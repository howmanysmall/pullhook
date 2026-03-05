//! Command execution and task scheduling.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use rayon::prelude::*;
use tracing::debug;

use crate::error::PullhookError;

/// A runnable invocation.
#[derive(Debug, Clone)]
pub enum Invocation {
	/// User-supplied command from `--command`.
	Command {
		/// Original command string.
		raw: String,
		/// Parsed argv.
		argv: Vec<String>,
	},
	/// Script invocation from `--script`.
	Script {
		/// Script name.
		script: String,
	},
}

impl Invocation {
	#[must_use]
	pub fn display(&self) -> String {
		match self {
			Self::Command { raw, .. } => raw.clone(),
			Self::Script { script } => format!("npm run-script {script}"),
		}
	}
}

/// Output from a single invocation.
#[derive(Debug, Clone)]
pub struct InvocationOutput {
	/// Display command text.
	pub command: String,
	/// Captured stdout (empty in debug mode).
	pub stdout: String,
	/// Captured stderr (empty in debug mode).
	pub stderr: String,
}

/// Execution result for a single task directory.
#[derive(Debug)]
pub struct TaskResult {
	/// Task current working directory.
	pub cwd: PathBuf,
	/// Outputs from invocations run in this task.
	pub outputs: Vec<InvocationOutput>,
	/// Error produced by the first failed invocation, if any.
	pub error: Option<PullhookError>,
}

/// Build invocation list in deterministic order.
pub fn prepare_invocations(command: Option<&str>, script: Option<&str>) -> Result<Vec<Invocation>, PullhookError> {
	let mut invocations = Vec::new();

	if let Some(command_text) = command {
		let argv = parse_command(command_text)?;
		invocations.push(Invocation::Command {
			raw: command_text.to_owned(),
			argv,
		});
	}

	if let Some(script_name) = script {
		invocations.push(Invocation::Script {
			script: script_name.to_owned(),
		});
	}

	Ok(invocations)
}

/// Parse a command string into argv without invoking a shell.
pub fn parse_command(command: &str) -> Result<Vec<String>, PullhookError> {
	let parsed = shell_words::split(command).map_err(|error| PullhookError::CommandParse {
		command: command.to_owned(),
		reason: error.to_string(),
	})?;

	if parsed.is_empty() {
		return Err(PullhookError::CommandParse {
			command: command.to_owned(),
			reason: "command cannot be empty".to_owned(),
		});
	}

	Ok(parsed)
}

/// Build task directories from matched file paths.
#[must_use]
pub fn build_task_dirs(repo_root: &Path, matched_paths: &[PathBuf], once: bool, unique_cwd: bool) -> Vec<PathBuf> {
	if once {
		return vec![repo_root.to_path_buf()];
	}

	let mut task_dirs = Vec::new();
	for relative_path in matched_paths {
		let relative_dir = relative_path.parent().unwrap_or_else(|| Path::new(""));
		task_dirs.push(repo_root.join(relative_dir));
	}

	if !unique_cwd {
		return task_dirs;
	}

	let mut seen = HashSet::new();
	task_dirs.into_iter().filter(|path| seen.insert(path.clone())).collect()
}

/// Execute tasks in parallel and return results in task order.
pub fn run_tasks(
	tasks: &[PathBuf],
	invocations: &[Invocation],
	jobs: usize,
	shell: bool,
	debug_enabled: bool,
) -> Result<Vec<TaskResult>, PullhookError> {
	let pool = rayon::ThreadPoolBuilder::new()
		.num_threads(jobs)
		.build()
		.map_err(|error| PullhookError::Message(error.to_string()))?;

	let mut indexed = pool.install(|| {
		tasks
			.par_iter()
			.enumerate()
			.map(|(index, cwd)| (index, run_task(cwd, invocations, shell, debug_enabled)))
			.collect::<Vec<_>>()
	});

	indexed.sort_by_key(|(index, _)| *index);
	Ok(indexed.into_iter().map(|(_, result)| result).collect())
}

/// Print grouped outputs for non-debug mode.
pub fn print_grouped_results(results: &[TaskResult], repo_root: &Path, debug_enabled: bool) {
	if debug_enabled {
		return;
	}

	for result in results {
		let relative_cwd = result
			.cwd
			.strip_prefix(repo_root)
			.ok()
			.and_then(|path| if path.as_os_str().is_empty() { None } else { Some(path) })
			.map_or_else(|| ".".to_owned(), |path| path.display().to_string());

		println!("=== {relative_cwd} ===");

		for output in &result.outputs {
			println!("$ {}", output.command);
			if !output.stdout.is_empty() {
				print!("{}", output.stdout);
			}
			if !output.stderr.is_empty() {
				eprint!("{}", output.stderr);
			}
		}

		if let Some(error) = &result.error {
			eprintln!("error: {error}");
		}
	}
}

fn run_task(cwd: &Path, invocations: &[Invocation], shell: bool, debug_enabled: bool) -> TaskResult {
	let mut outputs = Vec::new();

	for invocation in invocations {
		match run_invocation(invocation, cwd, shell, debug_enabled) {
			Ok(output) => outputs.push(output),
			Err(error) => {
				return TaskResult {
					cwd: cwd.to_path_buf(),
					outputs,
					error: Some(error),
				};
			}
		}
	}

	TaskResult {
		cwd: cwd.to_path_buf(),
		outputs,
		error: None,
	}
}

fn run_invocation(
	invocation: &Invocation,
	cwd: &Path,
	shell: bool,
	debug_enabled: bool,
) -> Result<InvocationOutput, PullhookError> {
	match invocation {
		Invocation::Command { raw, argv } => {
			if shell {
				run_command_shell(raw, cwd, debug_enabled)
			} else {
				run_command_direct(raw, argv, cwd, debug_enabled)
			}
		}
		Invocation::Script { script } => run_script(script, cwd, debug_enabled),
	}
}

fn run_command_direct(
	raw: &str,
	argv: &[String],
	cwd: &Path,
	debug_enabled: bool,
) -> Result<InvocationOutput, PullhookError> {
	let mut command = Command::new(&argv[0]);
	command.args(&argv[1..]).current_dir(cwd);

	run_process(command, raw, cwd, debug_enabled)
}

fn run_command_shell(raw: &str, cwd: &Path, debug_enabled: bool) -> Result<InvocationOutput, PullhookError> {
	let mut command = if cfg!(windows) {
		let mut command = Command::new("cmd");
		command.arg("/C").arg(raw);
		command
	} else {
		let mut command = Command::new("sh");
		command.arg("-c").arg(raw);
		command
	};

	command.current_dir(cwd);
	run_process(command, raw, cwd, debug_enabled)
}

fn run_script(script: &str, cwd: &Path, debug_enabled: bool) -> Result<InvocationOutput, PullhookError> {
	let mut command = Command::new("npm");
	command.args(["run-script", script]).current_dir(cwd);

	run_process(command, &format!("npm run-script {script}"), cwd, debug_enabled)
}

fn run_process(
	mut command: Command,
	display_command: &str,
	cwd: &Path,
	debug_enabled: bool,
) -> Result<InvocationOutput, PullhookError> {
	if debug_enabled {
		debug!(
			command = display_command,
			cwd = %cwd.display(),
			"running invocation"
		);
		let status = command
			.stdout(Stdio::inherit())
			.stderr(Stdio::inherit())
			.status()
			.map_err(|source| PullhookError::CommandIo {
				command: display_command.to_owned(),
				cwd: cwd.display().to_string(),
				source,
			})?;

		if status.success() {
			return Ok(InvocationOutput {
				command: display_command.to_owned(),
				stdout: String::new(),
				stderr: String::new(),
			});
		}

		return Err(PullhookError::CommandFailed {
			command: display_command.to_owned(),
			cwd: cwd.display().to_string(),
			code: status.code().unwrap_or(-1),
			details: "see streamed output above".to_owned(),
		});
	}

	let output = command.output().map_err(|source| PullhookError::CommandIo {
		command: display_command.to_owned(),
		cwd: cwd.display().to_string(),
		source,
	})?;

	let stdout = String::from_utf8_lossy(&output.stdout).to_string();
	let stderr = String::from_utf8_lossy(&output.stderr).to_string();

	if output.status.success() {
		return Ok(InvocationOutput {
			command: display_command.to_owned(),
			stdout,
			stderr,
		});
	}

	Err(PullhookError::CommandFailed {
		command: display_command.to_owned(),
		cwd: cwd.display().to_string(),
		code: output.status.code().unwrap_or(-1),
		details: if stderr.trim().is_empty() {
			if stdout.trim().is_empty() {
				"no output captured".to_owned()
			} else {
				stdout.trim().to_owned()
			}
		} else {
			stderr.trim().to_owned()
		},
	})
}
