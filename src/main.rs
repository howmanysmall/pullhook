//! Pullhook CLI entry point.

mod cli;
mod error;
mod git;
mod matcher;
mod pm;
mod runner;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;

use crate::cli::Cli;
use crate::pm::detect_package_manager;

fn main() {
	let cli = Cli::parse();
	init_tracing(cli.debug);

	if let Err(error) = run(&cli) {
		eprintln!("error: {error:#}");
		std::process::exit(1);
	}
}

fn run(cli: &Cli) -> Result<()> {
	let repo_root = git::repo_root(cli.debug).context("failed to resolve repository root")?;

	let mut pattern = cli.pattern.clone().unwrap_or_default();
	let mut command = cli.command.clone();
	let script = cli.script.clone();
	let mut once = cli.effective_once();

	if cli.install {
		let package_manager =
			detect_package_manager(&repo_root).context("failed to detect package manager for --install")?;
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

	let base = git::resolve_base(cli.base.as_deref(), cli.debug).context("failed to resolve diff base")?;
	let changed_files = git::changed_files(&base, cli.debug).context("failed to read changed files")?;

	if cli.debug {
		debug!(count = changed_files.len(), "loaded changed files");
		for path in &changed_files {
			debug!(changed = %path.display(), "changed file");
		}
	}

	let matcher = matcher::compile(&pattern).context("failed to compile pattern")?;
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

	if matched_files.is_empty() {
		info!("no matching files found");
		return Ok(());
	}

	if let Some(message) = &cli.message {
		println!("{message}");
	}

	let invocations = runner::prepare_invocations(command.as_deref(), script.as_deref())
		.context("failed to prepare command invocations")?;

	if invocations.is_empty() {
		return Ok(());
	}

	let tasks = runner::build_task_dirs(&repo_root, &matched_files, once, cli.unique_cwd);

	if cli.dry_run {
		print_dry_run(&tasks, &invocations, &repo_root);
		return Ok(());
	}

	let results = runner::run_tasks(&tasks, &invocations, cli.effective_jobs(), cli.shell, cli.debug)
		.context("failed to execute tasks")?;

	runner::print_grouped_results(&results, &repo_root, cli.debug);

	let mut failure_count = 0usize;
	for result in &results {
		if let Some(error) = &result.error {
			failure_count += 1;
			if cli.debug {
				eprintln!("error in {}: {error}", result.cwd.display());
			}
		}
	}

	if failure_count > 0 {
		return Err(anyhow!("{failure_count} task(s) failed"));
	}

	Ok(())
}

fn print_dry_run(tasks: &[std::path::PathBuf], invocations: &[runner::Invocation], repo_root: &std::path::Path) {
	for cwd in tasks {
		let relative = cwd
			.strip_prefix(repo_root)
			.ok()
			.and_then(|path| if path.as_os_str().is_empty() { None } else { Some(path) })
			.map_or_else(|| ".".to_owned(), |path| path.display().to_string());

		for invocation in invocations {
			println!("Would run: {} (cwd: {relative})", invocation.display());
		}
	}
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
