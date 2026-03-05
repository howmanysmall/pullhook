//! Pullhook CLI entry point.

mod cli;
mod error;
mod git;
mod matcher;
mod output;
mod pm;
mod runner;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use tracing::debug;
use tracing_subscriber::EnvFilter;

use crate::cli::Cli;
use crate::output::{DryRunSummary, NonSuccessReport, Renderer, Summary, TaskBlock, TaskCommand, TaskOutcome};
use crate::pm::detect_package_manager;

#[derive(Debug, Clone)]
struct RunConfig {
	pattern: String,
	command: Option<String>,
	script: Option<String>,
	once: bool,
	install_matchers: Option<Vec<String>>,
}

#[derive(Debug)]
struct MatchSet {
	changed_count: usize,
	matched_files: Vec<std::path::PathBuf>,
}

fn main() {
	let cli = Cli::parse();
	init_tracing(cli.debug);

	if let Err(error) = run(&cli) {
		eprintln!("error: {error:#}");
		std::process::exit(1);
	}
}

fn run(cli: &Cli) -> Result<()> {
	let renderer = Renderer::new(cli.render);
	let repo_root = resolve_repo_root(cli.debug)?;
	let run_config = resolve_run_config(cli, &repo_root)?;

	renderer.render_prepare_stage(&run_config.pattern);

	let MatchSet {
		changed_count,
		matched_files,
	} = collect_matches(cli, &run_config)?;

	renderer.render_discovery_stage(changed_count, matched_files.len());

	if matched_files.is_empty() {
		renderer.render_no_match_stage(&run_config.pattern, changed_count, matched_files.len());
		return Ok(());
	}

	if let Some(message) = &cli.message {
		render_message(&renderer, message);
	}

	let invocations = runner::prepare_invocations(run_config.command.as_deref(), run_config.script.as_deref())
		.context("failed to prepare command invocations")?;

	if invocations.is_empty() {
		render_empty_summary(&renderer, matched_files.len());
		return Ok(());
	}

	let tasks = runner::build_task_dirs(&repo_root, &matched_files, run_config.once, cli.unique_cwd);

	if cli.dry_run {
		let planned_commands = print_dry_run(&renderer, &tasks, &invocations, &repo_root);
		renderer.render_dry_run_summary_stage(DryRunSummary {
			matched_files: matched_files.len(),
			task_dirs: tasks.len(),
			planned_commands,
		});
		return Ok(());
	}

	let results = runner::run_tasks(&tasks, &invocations, cli.effective_jobs(), cli.shell, cli.debug)
		.context("failed to execute tasks")?;

	render_task_results(&renderer, &results, &repo_root);

	report_debug_errors(cli.debug, &results);
	let counts = summarize_results(&results);
	let failure_count = counts.failed + counts.interrupted;
	render_summary(&renderer, matched_files.len(), counts);

	if failure_count > 0 {
		return Err(anyhow!("{failure_count} task(s) failed"));
	}

	Ok(())
}

fn resolve_run_config(cli: &Cli, repo_root: &std::path::Path) -> Result<RunConfig> {
	let mut pattern = cli.pattern.clone().unwrap_or_default();
	let mut command = cli.command.clone();
	let script = cli.script.clone();
	let mut once = cli.effective_once();
	let mut install_matchers = None;

	if cli.install {
		let package_manager =
			detect_package_manager(repo_root).context("failed to detect package manager for --install")?;
		let install_pattern = package_manager.install_pattern();
		install_pattern.clone_into(&mut pattern);
		command = Some(package_manager.install_command());
		once = true;
		install_matchers = Some(parse_install_matchers(install_pattern));

		if cli.debug {
			debug!(
				package_manager = package_manager.name(),
				pattern,
				command = command.as_deref().unwrap_or_default(),
				"resolved --install settings"
			);
		}
	}

	Ok(RunConfig {
		pattern,
		command,
		script,
		once,
		install_matchers,
	})
}

fn parse_install_matchers(pattern: &str) -> Vec<String> {
	const PREFIX: &str = "+(";

	if let Some(inner) = pattern.strip_prefix(PREFIX).and_then(|value| value.strip_suffix(')')) {
		return inner
			.split('|')
			.filter(|part| !part.is_empty())
			.map(ToOwned::to_owned)
			.collect();
	}

	vec![pattern.to_owned()]
}

fn collect_matches(cli: &Cli, run_config: &RunConfig) -> Result<MatchSet> {
	let (base, changed_files) = git::resolve_base_and_changed_files(cli.base.as_deref(), cli.debug)
		.context("failed to resolve diff base or read changed files")?;
	let changed_count = changed_files.len();

	if cli.debug {
		debug!(%base, "resolved diff base");
		debug!(count = changed_count, "loaded changed files");
		for path in &changed_files {
			debug!(changed = %path.display(), "changed file");
		}
	}

	let matched_files: Vec<_> = if let Some(install_matchers) = &run_config.install_matchers {
		changed_files
			.into_iter()
			.filter(|path| {
				path.file_name()
					.and_then(std::ffi::OsStr::to_str)
					.is_some_and(|name| install_matchers.iter().any(|candidate| candidate == name))
			})
			.collect()
	} else {
		let matcher = matcher::compile(&run_config.pattern).context("failed to compile pattern")?;
		changed_files
			.into_iter()
			.filter(|path| matcher.is_match(path))
			.collect()
	};

	if cli.debug {
		debug!(count = matched_files.len(), "matched changed files");
		for path in &matched_files {
			debug!(matched = %path.display(), "pattern match");
		}
	}

	Ok(MatchSet {
		changed_count,
		matched_files,
	})
}

fn resolve_repo_root(debug_enabled: bool) -> Result<std::path::PathBuf> {
	let cwd = std::env::current_dir().context("failed to read current working directory")?;
	if cwd.join(".git").exists() {
		if debug_enabled {
			debug!(cwd = %cwd.display(), "using current working directory as repository root");
		}
		return Ok(cwd);
	}

	git::repo_root(debug_enabled).context("failed to resolve repository root")
}

fn render_message(renderer: &Renderer, message: &str) {
	renderer.render_message_stage(message);
}

fn render_empty_summary(renderer: &Renderer, matched_files: usize) {
	renderer.render_summary_stage(Summary {
		matched_files,
		task_dirs: 0,
		passed: 0,
		failed: 0,
		interrupted: 0,
	});
}

fn render_task_results(renderer: &Renderer, results: &[runner::TaskResult], repo_root: &std::path::Path) {
	renderer.render_task_stage();

	for result in results {
		let relative = runner::relative_cwd_label(&result.cwd, repo_root);
		let commands: Vec<_> = result
			.outputs
			.iter()
			.map(|output| TaskCommand {
				command: &output.command,
				stdout: &output.stdout,
				stderr: &output.stderr,
			})
			.collect();
		let outcome = map_task_outcome(result.state);

		renderer.render_task_block(TaskBlock {
			relative_cwd: &relative,
			commands: &commands,
			outcome,
		});

		if outcome != TaskOutcome::Success {
			let (command, exit_code) = result.outputs.last().map_or(("<unknown>", None), |output| {
				(output.command.as_str(), output.exit_code)
			});
			renderer.render_non_success_report(NonSuccessReport {
				relative_cwd: &relative,
				command,
				outcome,
				exit_code,
			});
		}
	}
}

fn report_debug_errors(debug_enabled: bool, results: &[runner::TaskResult]) {
	if !debug_enabled {
		return;
	}

	for result in results {
		if result.state != runner::ResultState::Success
			&& let Some(error) = &result.error
		{
			eprintln!("error in {}: {error}", result.cwd.display());
		}
	}
}

fn render_summary(renderer: &Renderer, matched_files: usize, counts: TaskCounters) {
	renderer.render_summary_stage(Summary {
		matched_files,
		task_dirs: counts.task_dirs,
		passed: counts.passed,
		failed: counts.failed,
		interrupted: counts.interrupted,
	});
}

const fn map_task_outcome(state: runner::ResultState) -> TaskOutcome {
	match state {
		runner::ResultState::Success => TaskOutcome::Success,
		runner::ResultState::Failed => TaskOutcome::Failed,
		runner::ResultState::Interrupted => TaskOutcome::Interrupted,
		runner::ResultState::SpawnError => TaskOutcome::SpawnError,
	}
}

#[derive(Debug, Clone, Copy)]
struct TaskCounters {
	task_dirs: usize,
	passed: usize,
	failed: usize,
	interrupted: usize,
}

fn summarize_results(results: &[runner::TaskResult]) -> TaskCounters {
	let mut passed = 0usize;
	let mut failed = 0usize;
	let mut interrupted = 0usize;

	for result in results {
		match result.state {
			runner::ResultState::Success => passed += 1,
			runner::ResultState::Failed | runner::ResultState::SpawnError => failed += 1,
			runner::ResultState::Interrupted => interrupted += 1,
		}
	}

	TaskCounters {
		task_dirs: results.len(),
		passed,
		failed,
		interrupted,
	}
}

fn print_dry_run(
	renderer: &Renderer,
	tasks: &[std::path::PathBuf],
	invocations: &[runner::Invocation],
	repo_root: &std::path::Path,
) -> usize {
	renderer.render_dry_run_stage();
	let mut planned_commands = 0usize;

	for cwd in tasks {
		let relative = runner::relative_cwd_label(cwd, repo_root);

		for invocation in invocations {
			let command = invocation.display();
			renderer.render_dry_run_block(&relative, &command);
			planned_commands = planned_commands.saturating_add(1);
		}
	}

	planned_commands
}

fn init_tracing(debug_enabled: bool) {
	if !debug_enabled && std::env::var_os("RUST_LOG").is_none() {
		return;
	}

	let fallback = if debug_enabled { "debug" } else { "error" };
	let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(fallback));

	tracing_subscriber::fmt()
		.with_env_filter(filter)
		.with_target(debug_enabled)
		.with_level(debug_enabled)
		.without_time()
		.init();
}
