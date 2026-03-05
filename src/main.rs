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
	let non_debug_output = !cli.debug;
	let repo_root = git::repo_root(cli.debug).context("failed to resolve repository root")?;
	let run_config = resolve_run_config(cli, &repo_root)?;

	if non_debug_output {
		renderer.render_prepare_stage(&run_config.pattern);
	}

	let MatchSet {
		changed_count,
		matched_files,
	} = collect_matches(cli, &run_config.pattern)?;

	if non_debug_output {
		renderer.render_discovery_stage(changed_count, matched_files.len());
	}

	if matched_files.is_empty() {
		if non_debug_output {
			renderer.render_no_match_stage();
		}
		return Ok(());
	}

	if let Some(message) = &cli.message {
		render_message(&renderer, non_debug_output, message);
	}

	let invocations = runner::prepare_invocations(run_config.command.as_deref(), run_config.script.as_deref())
		.context("failed to prepare command invocations")?;

	if invocations.is_empty() {
		render_empty_summary(&renderer, non_debug_output, matched_files.len());
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

	if non_debug_output {
		render_task_results(&renderer, &results, &repo_root);
	} else {
		runner::print_grouped_results(&results, &repo_root, cli.debug);
	}

	report_debug_errors(cli.debug, &results);
	let counts = summarize_results(&results);
	let failure_count = counts.failed + counts.interrupted;
	render_summary(&renderer, non_debug_output, matched_files.len(), counts);

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

	if cli.install {
		let package_manager =
			detect_package_manager(repo_root).context("failed to detect package manager for --install")?;
		package_manager.install_pattern().clone_into(&mut pattern);
		command = Some(package_manager.install_command());
		once = true;

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
	})
}

fn collect_matches(cli: &Cli, pattern: &str) -> Result<MatchSet> {
	let base = git::resolve_base(cli.base.as_deref(), cli.debug).context("failed to resolve diff base")?;
	let changed_files = git::changed_files(&base, cli.debug).context("failed to read changed files")?;
	let changed_count = changed_files.len();

	if cli.debug {
		debug!(count = changed_count, "loaded changed files");
		for path in &changed_files {
			debug!(changed = %path.display(), "changed file");
		}
	}

	let matcher = matcher::compile(pattern).context("failed to compile pattern")?;
	let matched_files: Vec<_> = changed_files
		.into_iter()
		.filter(|path| matcher.is_match(path))
		.collect();

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

fn render_message(renderer: &Renderer, non_debug_output: bool, message: &str) {
	if non_debug_output {
		renderer.render_message_stage(message);
	} else {
		println!("{message}");
	}
}

fn render_empty_summary(renderer: &Renderer, non_debug_output: bool, matched_files: usize) {
	if !non_debug_output {
		return;
	}

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

fn render_summary(renderer: &Renderer, non_debug_output: bool, matched_files: usize, counts: TaskCounters) {
	if !non_debug_output {
		return;
	}

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
	let fallback = if debug_enabled { "debug" } else { "error" };
	let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(fallback));

	let formatter = tracing_subscriber::fmt()
		.with_env_filter(filter)
		.with_target(debug_enabled)
		.with_level(debug_enabled);

	if debug_enabled {
		formatter.init();
	} else {
		formatter.without_time().init();
	}
}
