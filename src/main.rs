//! Pullhook CLI entry point.

mod cli;
mod config;
mod error;
mod git;
mod matcher;
mod output;
mod pm;
mod runner;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use rayon::prelude::*;
use tracing::debug;
use tracing_subscriber::EnvFilter;

use crate::cli::{Cli, Commands, ConfigRunArgs, ExplainArgs, InitArgs, RunArgs, ValidateArgs};
use crate::config::{
	Config, Entry, EvaluatedEntry, EvaluatedGroup, EvaluatedRule, FailTextContext, OnFailure, Pattern,
};
use crate::git::GitRepo;
use crate::output::{DryRunSummary, NonSuccessReport, RenderMode, Renderer, Summary, TaskBlock};
use crate::pm::{PackageManager, detect_package_manager};

#[derive(Debug, Clone)]
struct RunConfig {
	match_strategy: MatchStrategy,
	command: Option<String>,
	script: Option<String>,
	once: bool,
}

impl RunConfig {
	fn pattern(&self) -> &str {
		self.match_strategy.pattern()
	}
}

#[derive(Debug, Clone)]
enum MatchStrategy {
	Glob(String),
	Install { pattern: String },
}

impl MatchStrategy {
	fn from_package_manager(package_manager: PackageManager) -> Self {
		Self::Install {
			pattern: package_manager.install_pattern(),
		}
	}

	fn pattern(&self) -> &str {
		match self {
			Self::Glob(pattern) | Self::Install { pattern, .. } => pattern,
		}
	}
}

fn main() {
	let cli = Cli::parse();

	let result = match cli.command.as_ref() {
		Some(Commands::Completion { shell }) => {
			print_completion(*shell);
			return;
		}
		Some(Commands::Run(args)) => {
			init_tracing(args.debug);
			run_config_command(args)
		}
		Some(Commands::Explain(args)) => {
			init_tracing(args.debug);
			explain_config_command(args)
		}
		Some(Commands::Validate(args)) => {
			init_tracing(args.debug);
			validate_config_command(args)
		}
		Some(Commands::Init(args)) => {
			init_tracing(args.debug);
			init_config_command(args)
		}
		None => {
			init_tracing(cli.run.debug);
			run_legacy(&cli.run)
		}
	};

	if let Err(error) = result {
		eprintln!("error: {error:#}");
		std::process::exit(1);
	}
}

fn print_completion(shell: clap_complete::Shell) {
	Cli::print_completion(shell);
}

fn run_legacy(cli: &RunArgs) -> Result<()> {
	if cli.pattern.is_none() && !cli.install {
		return Err(anyhow!(
			"missing required argument: use `--pattern <glob>`, `--install`, or the `run` subcommand"
		));
	}

	let renderer = Renderer::new(cli.render);
	let cwd = std::env::current_dir().context("failed to read current working directory")?;
	let repo = GitRepo::discover(&cwd, cli.debug).context("failed to resolve repository root")?;
	let repo_root = repo.root().to_path_buf();
	let run_config = resolve_run_config(cli, &repo_root)?;

	renderer.render_prepare_stage(run_config.pattern());

	let (changed_count, matched_files) = collect_matches(cli, &repo, &run_config)?;

	renderer.render_discovery_stage(changed_count, matched_files.len());

	if matched_files.is_empty() {
		renderer.render_no_match_stage(run_config.pattern(), changed_count, matched_files.len());
		return Ok(());
	}

	if let Some(message) = &cli.message {
		renderer.render_message_stage(message);
	}

	let invocations = runner::prepare_invocations(run_config.command.as_deref(), run_config.script.as_deref())
		.context("failed to prepare command invocations")?;

	if invocations.is_empty() {
		renderer.render_summary_stage(Summary {
			matched_files: matched_files.len(),
			task_dirs: 0,
			passed: 0,
			failed: 0,
			interrupted: 0,
		});
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

fn run_config_command(args: &ConfigRunArgs) -> Result<()> {
	let renderer = Renderer::new(args.render);
	let (repo, repo_root, config) = load_config_from_cwd(args.debug)?;
	let (changed_files, base_missing) = resolve_config_changed_files(&repo, &config, args.base.as_deref(), args.debug)?;
	let evaluation = evaluate_config(&config, &changed_files, base_missing, &repo_root)?;

	render_config_evaluation(&config, &evaluation, args.all_matches || args.dry_run, args.dry_run);

	if args.dry_run {
		let planned_commands = count_planned_commands(&evaluation);
		renderer.render_dry_run_summary_stage(DryRunSummary {
			matched_files: changed_files.len(),
			task_dirs: planned_commands,
			planned_commands,
		});
		return Ok(());
	}

	let failed = execute_config_entries(&renderer, config.on_failure, &evaluation, &repo_root, args)?;
	let planned_commands = count_planned_commands(&evaluation);
	renderer.render_summary_stage(Summary {
		matched_files: changed_files.len(),
		task_dirs: planned_commands,
		passed: planned_commands.saturating_sub(failed),
		failed,
		interrupted: 0,
	});
	if failed > 0 {
		return Err(anyhow!("{failed} config rule(s) failed"));
	}

	Ok(())
}

fn explain_config_command(args: &ExplainArgs) -> Result<()> {
	let (repo, repo_root, config) = load_config_from_cwd(args.debug)?;
	let (changed_files, base_missing) = resolve_config_changed_files(&repo, &config, args.base.as_deref(), args.debug)?;
	let evaluation = evaluate_config(&config, &changed_files, base_missing, &repo_root)?;

	render_config_evaluation(&config, &evaluation, args.all_matches, false);
	Ok(())
}

fn validate_config_command(args: &ValidateArgs) -> Result<()> {
	let renderer = Renderer::new(args.render);
	let (_, _, config) = load_config_from_cwd(args.debug)?;

	renderer.render_message_stage(&format!("config valid: {}", config.path.display()));
	Ok(())
}

fn init_config_command(args: &InitArgs) -> Result<()> {
	let renderer = Renderer::new(args.render);
	let cwd = std::env::current_dir().context("failed to read current working directory")?;
	let repo = GitRepo::discover(&cwd, args.debug).context("failed to resolve repository root")?;
	let repo_root = repo.root();

	if let Some(path) = config::discover(repo_root)? {
		return Err(anyhow!("pullhook config already exists: {}", path.display()));
	}

	let path = repo_root.join("pullhook.json");
	std::fs::write(&path, config::STARTER_CONFIG)
		.with_context(|| format!("failed to write config `{}`", path.display()))?;
	renderer.render_message_stage(&format!("created {}", path.display()));
	Ok(())
}

fn load_config_from_cwd(debug_enabled: bool) -> Result<(GitRepo, std::path::PathBuf, Config)> {
	let cwd = std::env::current_dir().context("failed to read current working directory")?;
	let repo = GitRepo::discover(&cwd, debug_enabled).context("failed to resolve repository root")?;
	let repo_root = repo.root().to_path_buf();
	let path = config::discover(&repo_root)?.ok_or_else(|| {
		anyhow!(
			"no pullhook config found; run `pullhook init` to create {}",
			config::config_names()[0]
		)
	})?;
	let config = config::load(&path)?;
	Ok((repo, repo_root, config))
}

fn resolve_config_changed_files(
	repo: &GitRepo,
	config: &Config,
	base: Option<&str>,
	debug_enabled: bool,
) -> Result<(Vec<std::path::PathBuf>, bool)> {
	match repo.resolve_base_and_changed_files(base, debug_enabled) {
		Ok((resolved_base, changed_files)) => {
			if debug_enabled {
				debug!(base = %resolved_base, changed = changed_files.len(), "resolved config changed files");
			}
			Ok((changed_files, false))
		}
		Err(error)
			if config_allows_base_missing(config) && error.to_string().contains("unable to resolve diff base") =>
		{
			if debug_enabled {
				debug!("diff base missing; evaluating runIfBaseMissing rules");
			}
			Ok((Vec::new(), true))
		}
		Err(error) => Err(error).context("failed to resolve diff base or read changed files"),
	}
}

fn config_allows_base_missing(config: &Config) -> bool {
	config.entries.iter().any(|entry| match entry {
		Entry::Rule(rule) => rule.run_if_base_missing,
		Entry::Group(group) => group.rules.iter().any(|rule| rule.run_if_base_missing),
	})
}

fn evaluate_config(
	config: &Config,
	changed_files: &[std::path::PathBuf],
	base_missing: bool,
	repo_root: &std::path::Path,
) -> Result<Vec<EvaluatedEntry>> {
	let mut install_plan: Option<(String, Vec<Pattern>)> = None;

	config::evaluate(config, changed_files, base_missing, |_rule| {
		if let Some((command, patterns)) = &install_plan {
			return Ok((Some(command.clone()), patterns.clone()));
		}

		let package_manager = detect_package_manager(repo_root).map_err(|error| {
			error::PullhookError::Message(format!(
				"failed to detect package manager for config install rule: {error}"
			))
		})?;
		let command = package_manager.install_command();
		let patterns = vec![Pattern::new(package_manager.install_pattern())?];
		install_plan = Some((command.clone(), patterns.clone()));
		Ok((Some(command), patterns))
	})
	.map_err(Into::into)
}

fn render_config_evaluation(config: &Config, evaluation: &[EvaluatedEntry], all_matches: bool, dry_run: bool) {
	println!("Config");
	println!("path: {}", config.path.display());
	println!(
		"onFailure: {}",
		match config.on_failure {
			OnFailure::Stop => "stop",
			OnFailure::Continue => "continue",
		}
	);

	println!();
	if dry_run {
		println!("Dry Run");
	} else {
		println!("Rules");
	}

	for entry in evaluation {
		match entry {
			EvaluatedEntry::Rule(rule) => render_evaluated_rule(rule, all_matches),
			EvaluatedEntry::Group(group) => {
				if group.should_run() || all_matches {
					println!("group: {}", group.group.name);
					println!("jobs: {}", group.group.jobs.unwrap_or(1));
				}
				for rule in &group.rules {
					render_evaluated_rule(rule, all_matches);
				}
			}
		}
	}
}

fn render_evaluated_rule(rule: &EvaluatedRule, all_matches: bool) {
	if rule.should_run() {
		println!("[match] {}", rule.rule.name);
		println!("matches: {}", rule.matches.len());
		if let Some(command) = &rule.command {
			println!("command: {command}");
		}
		return;
	}

	if all_matches {
		println!("[skip] {}", rule.rule.name);
		println!("reason: {}", rule.skip_reason.as_deref().unwrap_or("not runnable"));
	}
}

fn count_planned_commands(evaluation: &[EvaluatedEntry]) -> usize {
	evaluation
		.iter()
		.map(|entry| match entry {
			EvaluatedEntry::Rule(rule) => usize::from(rule.should_run()),
			EvaluatedEntry::Group(group) => group.rules.iter().filter(|rule| rule.should_run()).count(),
		})
		.sum()
}

fn execute_config_entries(
	renderer: &Renderer,
	on_failure: OnFailure,
	evaluation: &[EvaluatedEntry],
	repo_root: &std::path::Path,
	args: &ConfigRunArgs,
) -> Result<usize> {
	let mut failed = 0usize;

	for entry in evaluation {
		match entry {
			EvaluatedEntry::Rule(rule) => {
				if !rule.should_run() {
					continue;
				}
				let rule_failed = execute_config_rule(renderer, rule, repo_root, args.debug, args.render);
				failed += usize::from(rule_failed);
			}
			EvaluatedEntry::Group(group) => {
				if !group.should_run() {
					continue;
				}
				let group_failed = execute_config_group(renderer, group, repo_root, args)?;
				failed += group_failed;
			}
		}

		if failed > 0 && on_failure == OnFailure::Stop {
			break;
		}
	}

	Ok(failed)
}

fn execute_config_rule(
	renderer: &Renderer,
	rule: &EvaluatedRule,
	repo_root: &std::path::Path,
	debug_enabled: bool,
	render_mode: RenderMode,
) -> bool {
	let result = run_config_rule_task(rule, repo_root, debug_enabled);
	render_config_rule_result(renderer, rule, &result, repo_root, debug_enabled, render_mode)
}

fn execute_config_group(
	renderer: &Renderer,
	group: &EvaluatedGroup,
	repo_root: &std::path::Path,
	args: &ConfigRunArgs,
) -> Result<usize> {
	let runnable: Vec<_> = group.rules.iter().filter(|rule| rule.should_run()).collect();
	let jobs = group.group.jobs.unwrap_or_else(|| args.effective_jobs()).max(1);
	let results = if runnable.len() <= 1 || jobs <= 1 {
		runnable
			.iter()
			.map(|rule| (*rule, run_config_rule_task(rule, repo_root, args.debug)))
			.collect::<Vec<_>>()
	} else {
		let pool = rayon::ThreadPoolBuilder::new()
			.num_threads(jobs)
			.build()
			.map_err(|error| anyhow!(error.to_string()))?;
		pool.install(|| {
			runnable
				.par_iter()
				.map(|rule| (*rule, run_config_rule_task(rule, repo_root, args.debug)))
				.collect::<Vec<_>>()
		})
	};

	let mut failed = 0usize;
	for (rule, result) in &results {
		if render_config_rule_result(renderer, rule, result, repo_root, args.debug, args.render) {
			failed += 1;
		}
	}

	if failed > 0 {
		render_group_fail_text(group, args.render);
	}

	Ok(failed)
}

fn run_config_rule_task(rule: &EvaluatedRule, repo_root: &std::path::Path, debug_enabled: bool) -> runner::TaskResult {
	let command = rule.command.as_deref().unwrap_or_default();
	let invocations = runner::prepare_invocations(Some(command), None);
	invocations.map_or_else(
		|error| runner::TaskResult {
			cwd: repo_root.to_path_buf(),
			outputs: Vec::new(),
			state: runner::ResultState::SpawnError,
			error: Some(error),
		},
		|invocations| runner::run_task_dir(repo_root, &invocations, false, debug_enabled),
	)
}

fn render_config_rule_result(
	renderer: &Renderer,
	rule: &EvaluatedRule,
	result: &runner::TaskResult,
	repo_root: &std::path::Path,
	debug_enabled: bool,
	render_mode: RenderMode,
) -> bool {
	render_task_results(renderer, std::slice::from_ref(result), repo_root);
	report_debug_errors(debug_enabled, std::slice::from_ref(result));
	let failed = result.state != runner::ResultState::Success;
	if failed {
		render_rule_fail_text(rule, result, repo_root, render_mode);
	}
	failed
}

fn render_rule_fail_text(
	rule: &EvaluatedRule,
	result: &runner::TaskResult,
	repo_root: &std::path::Path,
	render_mode: RenderMode,
) {
	let Some(fail_text) = &rule.rule.fail_text else {
		return;
	};
	let (command, exit_code) = result.outputs.last().map_or_else(
		|| ("", "unavailable".to_owned()),
		|output| {
			(
				output.command.as_str(),
				output
					.exit_code
					.map_or_else(|| "unavailable".to_owned(), |code| code.to_string()),
			)
		},
	);
	let cwd = runner::relative_cwd_label(&result.cwd, repo_root);
	let context = FailTextContext {
		rule: &rule.rule.name,
		command,
		cwd: &cwd,
		exit_code: &exit_code,
	};
	eprintln!("{}", fail_text.render(&context, render_mode));
}

fn render_group_fail_text(group: &EvaluatedGroup, render_mode: RenderMode) {
	let Some(fail_text) = &group.group.fail_text else {
		return;
	};
	let context = FailTextContext {
		rule: &group.group.name,
		command: "parallel group",
		cwd: ".",
		exit_code: "1",
	};
	eprintln!("{}", fail_text.render(&context, render_mode));
}

fn resolve_run_config(cli: &RunArgs, repo_root: &std::path::Path) -> Result<RunConfig> {
	let mut match_strategy = MatchStrategy::Glob(cli.pattern.clone().unwrap_or_default());
	let mut command = cli.command.clone();
	let script = cli.script.clone();
	let mut once = cli.effective_once();

	if cli.install {
		let package_manager =
			detect_package_manager(repo_root).context("failed to detect package manager for --install")?;
		match_strategy = MatchStrategy::from_package_manager(package_manager);
		command = Some(package_manager.install_command());
		once = true;

		if cli.debug {
			debug!(
				package_manager = package_manager.name(),
				pattern = match_strategy.pattern(),
				command = command.as_deref().unwrap_or_default(),
				"resolved --install settings"
			);
		}
	}

	Ok(RunConfig {
		match_strategy,
		command,
		script,
		once,
	})
}

fn collect_matches(cli: &RunArgs, repo: &GitRepo, run_config: &RunConfig) -> Result<(usize, Vec<std::path::PathBuf>)> {
	let (base, changed_count, matched_files) = match &run_config.match_strategy {
		MatchStrategy::Glob(pattern) => {
			let (base, changed_files) = repo
				.resolve_base_and_changed_files(cli.base.as_deref(), cli.debug)
				.context("failed to resolve diff base or read changed files")?;
			let changed_count = changed_files.len();

			if cli.debug {
				debug!(count = changed_count, "loaded changed files");
				for path in &changed_files {
					debug!(changed = %path.display(), "changed file");
				}
			}

			let matcher = matcher::compile(pattern).context("failed to compile pattern")?;
			let matched_files = changed_files
				.into_iter()
				.filter(|path| matcher.is_match(path))
				.collect();

			(base, changed_count, matched_files)
		}
		MatchStrategy::Install { pattern } => {
			let matcher = matcher::compile(pattern).context("failed to compile pattern")?;
			let (base, changed_count, matched_files) = repo
				.resolve_install_matches(cli.base.as_deref(), |path| matcher.is_match(path), cli.debug)
				.context("failed to resolve diff base or read changed files")?;
			(base, changed_count, matched_files)
		}
	};

	if cli.debug {
		debug!(%base, "resolved diff base");
		debug!(count = matched_files.len(), "matched changed files");
		for path in &matched_files {
			debug!(matched = %path.display(), "pattern match");
		}
	}

	Ok((changed_count, matched_files))
}

fn render_task_results(renderer: &Renderer, results: &[runner::TaskResult], repo_root: &std::path::Path) {
	renderer.render_task_stage();

	for result in results {
		let relative = runner::relative_cwd_label(&result.cwd, repo_root);

		renderer.render_task_block(TaskBlock {
			relative_cwd: &relative,
			commands: &result.outputs,
			outcome: result.state,
		});

		if result.state != runner::ResultState::Success {
			let (command, exit_code) = result.outputs.last().map_or(("<unknown>", None), |output| {
				(output.command.as_str(), output.exit_code)
			});
			renderer.render_non_success_report(NonSuccessReport {
				relative_cwd: &relative,
				command,
				outcome: result.state,
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
			renderer.render_dry_run_block(&relative, command.as_ref());
			planned_commands += 1;
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
