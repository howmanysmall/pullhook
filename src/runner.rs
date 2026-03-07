//! Command execution and task scheduling.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

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

/// Execution state for an invocation or a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultState {
	/// Process exited with success status.
	Success,
	/// Process exited with a non-zero exit code.
	Failed,
	/// Process exited without an exit code (usually signal termination).
	Interrupted,
	/// Process failed to spawn.
	SpawnError,
}

/// Output from a single invocation.
#[derive(Debug, Clone)]
pub struct InvocationOutput {
	/// Display command text.
	pub command: String,
	/// Captured stdout (lossy-decoded, empty in debug mode).
	pub stdout: String,
	/// Captured stderr (lossy-decoded, empty in debug mode).
	pub stderr: String,
	/// Explicit invocation result state.
	pub state: ResultState,
	/// Exit code when available.
	pub exit_code: Option<i32>,
}

/// Execution result for a single task directory.
#[derive(Debug)]
pub struct TaskResult {
	/// Task current working directory.
	pub cwd: PathBuf,
	/// Outputs from invocations run in this task.
	pub outputs: Vec<InvocationOutput>,
	/// Explicit task result state.
	pub state: ResultState,
	/// Error produced by the first failed invocation, if any.
	pub error: Option<PullhookError>,
}

#[derive(Debug)]
struct InvocationExecution {
	output: InvocationOutput,
	error: Option<PullhookError>,
}

const NO_OUTPUT_CAPTURED: &str = "no output captured";
const NO_EXIT_CODE_CAPTURED: &str = "process terminated without an exit code";
const SEE_STREAMED_OUTPUT: &str = "see streamed output above";
const NO_EXIT_CODE_STREAMED_OUTPUT: &str = "process terminated without an exit code; see streamed output above";

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
	if tasks.len() <= 1 || jobs <= 1 {
		return Ok(tasks
			.iter()
			.map(|cwd| run_task(cwd, invocations, shell, debug_enabled))
			.collect());
	}

	let pool = rayon::ThreadPoolBuilder::new()
		.num_threads(jobs)
		.build()
		.map_err(|error| PullhookError::Message(error.to_string()))?;

	let results = pool.install(|| {
		tasks
			.par_iter()
			.map(|cwd| run_task(cwd, invocations, shell, debug_enabled))
			.collect::<Vec<_>>()
	});

	Ok(results)
}

/// Compute a display cwd relative to repository root.
#[must_use]
pub fn relative_cwd_label(cwd: &Path, repo_root: &Path) -> String {
	cwd.strip_prefix(repo_root)
		.ok()
		.and_then(|path| if path.as_os_str().is_empty() { None } else { Some(path) })
		.map_or_else(|| ".".to_owned(), |path| path.display().to_string())
}

fn run_task(cwd: &Path, invocations: &[Invocation], shell: bool, debug_enabled: bool) -> TaskResult {
	let mut outputs = Vec::new();

	for invocation in invocations {
		let InvocationExecution { output, error } = run_invocation(invocation, cwd, shell, debug_enabled);
		let state = output.state;
		outputs.push(output);

		if let Some(error) = error {
			return TaskResult {
				cwd: cwd.to_path_buf(),
				outputs,
				state,
				error: Some(error),
			};
		}
	}

	TaskResult {
		cwd: cwd.to_path_buf(),
		outputs,
		state: ResultState::Success,
		error: None,
	}
}

fn run_invocation(invocation: &Invocation, cwd: &Path, shell: bool, debug_enabled: bool) -> InvocationExecution {
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

fn run_command_direct(raw: &str, argv: &[String], cwd: &Path, debug_enabled: bool) -> InvocationExecution {
	let mut command = Command::new(&argv[0]);
	command.args(&argv[1..]).current_dir(cwd);

	run_process(command, raw, cwd, debug_enabled)
}

fn run_command_shell(raw: &str, cwd: &Path, debug_enabled: bool) -> InvocationExecution {
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

fn run_script(script: &str, cwd: &Path, debug_enabled: bool) -> InvocationExecution {
	let mut command = Command::new("npm");
	command.args(["run-script", script]).current_dir(cwd);

	run_process(command, &format!("npm run-script {script}"), cwd, debug_enabled)
}

fn run_process(command: Command, display_command: &str, cwd: &Path, debug_enabled: bool) -> InvocationExecution {
	if debug_enabled {
		return run_streaming_process(command, display_command, cwd);
	}

	run_captured_process(command, display_command, cwd)
}

fn run_streaming_process(mut command: Command, display_command: &str, cwd: &Path) -> InvocationExecution {
	debug!(
		command = display_command,
		cwd = %cwd.display(),
		"running invocation"
	);

	let status = match command.stdout(Stdio::inherit()).stderr(Stdio::inherit()).status() {
		Ok(status) => status,
		Err(source) => return spawn_error(display_command, cwd, source),
	};

	let (state, exit_code) = classify_exit_status(status);
	let details = match state {
		ResultState::Success => String::new(),
		ResultState::Interrupted => NO_EXIT_CODE_STREAMED_OUTPUT.to_owned(),
		ResultState::Failed | ResultState::SpawnError => SEE_STREAMED_OUTPUT.to_owned(),
	};

	finalize_execution(
		display_command,
		cwd,
		String::new(),
		String::new(),
		state,
		exit_code,
		details,
	)
}

fn run_captured_process(mut command: Command, display_command: &str, cwd: &Path) -> InvocationExecution {
	let output = match command.output() {
		Ok(output) => output,
		Err(source) => return spawn_error(display_command, cwd, source),
	};

	let stdout = normalize_output(&output.stdout);
	let stderr = normalize_output(&output.stderr);
	let (state, exit_code) = classify_exit_status(output.status);

	let fallback = if state == ResultState::Interrupted {
		NO_EXIT_CODE_CAPTURED
	} else {
		NO_OUTPUT_CAPTURED
	};
	let details = normalize_failure_details(&stdout, &stderr, fallback);

	finalize_execution(display_command, cwd, stdout, stderr, state, exit_code, details)
}

fn spawn_error(display_command: &str, cwd: &Path, source: std::io::Error) -> InvocationExecution {
	InvocationExecution {
		output: invocation_output(
			display_command,
			String::new(),
			String::new(),
			ResultState::SpawnError,
			None,
		),
		error: Some(PullhookError::CommandIo {
			command: display_command.to_owned(),
			cwd: cwd.display().to_string(),
			source,
		}),
	}
}

fn finalize_execution(
	display_command: &str,
	cwd: &Path,
	stdout: String,
	stderr: String,
	state: ResultState,
	exit_code: Option<i32>,
	details: String,
) -> InvocationExecution {
	let output = invocation_output(display_command, stdout, stderr, state, exit_code);
	if state == ResultState::Success {
		return InvocationExecution { output, error: None };
	}

	InvocationExecution {
		output,
		error: Some(command_failed(display_command, cwd, exit_code, details)),
	}
}

fn command_failed(display_command: &str, cwd: &Path, exit_code: Option<i32>, details: String) -> PullhookError {
	PullhookError::CommandFailed {
		command: display_command.to_owned(),
		cwd: cwd.display().to_string(),
		code: exit_code,
		status: format_exit_status(exit_code),
		details,
	}
}

fn invocation_output(
	display_command: &str,
	stdout: String,
	stderr: String,
	state: ResultState,
	exit_code: Option<i32>,
) -> InvocationOutput {
	InvocationOutput {
		command: display_command.to_owned(),
		stdout,
		stderr,
		state,
		exit_code,
	}
}

fn classify_exit_status(status: ExitStatus) -> (ResultState, Option<i32>) {
	let code = status.code();
	if status.success() {
		return (ResultState::Success, code);
	}

	if code.is_some() {
		return (ResultState::Failed, code);
	}

	(ResultState::Interrupted, None)
}

fn normalize_output(bytes: &[u8]) -> String {
	String::from_utf8_lossy(bytes).to_string()
}

fn normalize_failure_details(stdout: &str, stderr: &str, fallback: &str) -> String {
	if stderr.trim().is_empty() {
		if stdout.trim().is_empty() {
			fallback.to_owned()
		} else {
			stdout.trim().to_owned()
		}
	} else {
		stderr.trim().to_owned()
	}
}

fn format_exit_status(code: Option<i32>) -> String {
	code.map_or_else(
		|| "no exit code (terminated by signal)".to_owned(),
		|value| value.to_string(),
	)
}
