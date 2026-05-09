//! Pullhook CLI entry point.

mod cli;
mod config;
mod error;
mod git;
mod matcher;
mod output;
mod pm;
mod runner;

use std::collections::BTreeSet;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use rayon::prelude::*;
use serde_json::json;
use tracing::debug;
use tracing_subscriber::EnvFilter;

use crate::cli::{Cli, Commands, ConfigRunArgs, DoctorArgs, ExplainArgs, InitArgs, RulesArgs, RunArgs, ValidateArgs};
use crate::config::{
	Config, Entry, EvaluatedEntry, EvaluatedGroup, EvaluatedRule, FailTextContext, OnFailure, Pattern,
};
use crate::git::GitRepo;
use crate::output::{DryRunSummary, NonSuccessReport, RenderMode, Renderer, Summary, TaskBlock};
use crate::pm::detect_package_manager;

#[derive(Debug, Clone)]
struct RunConfig {
	pattern: String,
	command: Option<String>,
	script: Option<String>,
	once: bool,
}

#[derive(Debug, Clone)]
struct InstallPlan {
	package_manager: &'static str,
	pattern: String,
	command: String,
}

#[derive(Debug, Clone)]
struct LegacyJsonContext {
	changed_count: usize,
	run_config: RunConfig,
	matched_files: Vec<std::path::PathBuf>,
	invocations: Vec<runner::Invocation>,
	tasks: Vec<std::path::PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DoctorLevel {
	Ok,
	Warn,
	Error,
}

impl DoctorLevel {
	const fn label(self) -> &'static str {
		match self {
			Self::Ok => "ok",
			Self::Warn => "warn",
			Self::Error => "error",
		}
	}
}

#[derive(Debug, Clone)]
struct DoctorCheck {
	name: &'static str,
	level: DoctorLevel,
	summary: String,
	details: Vec<String>,
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
		Some(Commands::Doctor(args)) => {
			init_tracing(args.debug);
			doctor_command(args)
		}
		Some(Commands::Rules(args)) => {
			init_tracing(args.debug);
			rules_command(args)
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
	if cli.json && cli.debug {
		return Err(anyhow!("--json cannot be used with --debug"));
	}

	let renderer = Renderer::new(cli.render);
	let cwd = std::env::current_dir().context("failed to read current working directory")?;
	let repo = GitRepo::discover(&cwd, cli.debug).context("failed to resolve repository root")?;
	let repo_root = repo.root().to_path_buf();
	let run_config = resolve_run_config(cli, &repo_root)?;
	let (changed_count, matched_files) = collect_matches(cli, &repo, &run_config)?;
	let invocations = runner::prepare_invocations(run_config.command.as_deref(), run_config.script.as_deref())
		.context("failed to prepare command invocations")?;
	let tasks = runner::build_task_dirs(&repo_root, &matched_files, run_config.once, cli.unique_cwd);

	if cli.json {
		return run_legacy_json(
			cli,
			LegacyJsonContext {
				changed_count,
				run_config,
				matched_files,
				invocations,
				tasks,
			},
			&repo_root,
		);
	}

	renderer.render_prepare_stage(&run_config.pattern);
	renderer.render_discovery_stage(changed_count, matched_files.len());

	if matched_files.is_empty() {
		renderer.render_no_match_stage(&run_config.pattern, changed_count, matched_files.len());
		return Ok(());
	}

	if let Some(message) = &cli.message {
		renderer.render_message_stage(message);
	}

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

fn run_legacy_json(cli: &RunArgs, context: LegacyJsonContext, repo_root: &std::path::Path) -> Result<()> {
	if cli.dry_run {
		let planned_commands = context.tasks.len() * context.invocations.len();
		println!(
			"{}",
			serde_json::to_string_pretty(&legacy_dry_run_json(cli, &context, planned_commands, repo_root))?
		);
		return Ok(());
	}

	let LegacyJsonContext {
		changed_count,
		run_config,
		matched_files,
		invocations,
		tasks,
	} = context;

	if matched_files.is_empty() || invocations.is_empty() {
		println!(
			"{}",
			serde_json::to_string_pretty(&legacy_run_json(
				cli,
				&run_config,
				changed_count,
				&matched_files,
				TaskCounters::default(),
				&[],
			))?
		);
		return Ok(());
	}

	let results = runner::run_tasks(&tasks, &invocations, cli.effective_jobs(), cli.shell, cli.debug)
		.context("failed to execute tasks")?;
	let counts = summarize_results(&results);
	let failure_count = counts.failed + counts.interrupted;
	let executions = results
		.iter()
		.map(|result| task_result_json(result, repo_root))
		.collect::<Vec<_>>();
	println!(
		"{}",
		serde_json::to_string_pretty(&legacy_run_json(
			cli,
			&run_config,
			changed_count,
			&matched_files,
			counts,
			&executions,
		))?
	);
	if failure_count > 0 {
		return Err(anyhow!("{failure_count} task(s) failed"));
	}

	Ok(())
}

fn run_config_command(args: &ConfigRunArgs) -> Result<()> {
	if args.json && args.debug {
		return Err(anyhow!("--json cannot be used with --debug"));
	}

	let renderer = Renderer::new(args.render);
	let (repo, repo_root, config) = load_config_from_cwd(args.debug, args.config.as_deref())?;
	let (changed_files, base_missing) =
		resolve_config_changed_files(&repo, &config, args.base.as_deref(), &args.changed_files, args.debug)?;
	let evaluation = filter_config_evaluation(
		evaluate_config(&config, &changed_files, base_missing, &repo_root)?,
		&args.rules,
	)?;
	let matched_files = count_config_matched_files(&evaluation);

	if args.json {
		if args.dry_run {
			let planned_commands = count_planned_commands(&evaluation);
			println!(
				"{}",
				serde_json::to_string_pretty(&config_dry_run_json(
					&config,
					&changed_files,
					base_missing,
					&evaluation,
					planned_commands,
				))?
			);
			return Ok(());
		}

		let (counts, executions) = execute_config_entries_json(config.on_failure, &evaluation, &repo_root, args)?;
		let failure_count = counts.failed + counts.interrupted;
		println!(
			"{}",
			serde_json::to_string_pretty(&config_run_json(
				&config,
				&changed_files,
				base_missing,
				&evaluation,
				counts,
				executions,
			))?
		);
		if failure_count > 0 {
			return Err(anyhow!("{failure_count} config rule(s) failed"));
		}
		return Ok(());
	}

	render_config_evaluation(&config, &evaluation, args.all_matches || args.dry_run, args.dry_run);

	if args.dry_run {
		let planned_commands = count_planned_commands(&evaluation);
		renderer.render_dry_run_summary_stage(DryRunSummary {
			matched_files,
			task_dirs: planned_commands,
			planned_commands,
		});
		return Ok(());
	}

	let counts = execute_config_entries(&renderer, config.on_failure, &evaluation, &repo_root, args)?;
	let failure_count = counts.failed + counts.interrupted;
	renderer.render_summary_stage(Summary {
		matched_files,
		task_dirs: counts.task_dirs,
		passed: counts.passed,
		failed: counts.failed,
		interrupted: counts.interrupted,
	});
	if failure_count > 0 {
		return Err(anyhow!("{failure_count} config rule(s) failed"));
	}

	Ok(())
}

fn explain_config_command(args: &ExplainArgs) -> Result<()> {
	let (repo, repo_root, config) = load_config_from_cwd(args.debug, args.config.as_deref())?;
	let (changed_files, base_missing) =
		resolve_config_changed_files(&repo, &config, args.base.as_deref(), &args.changed_files, args.debug)?;
	let evaluation = filter_config_evaluation(
		evaluate_config(&config, &changed_files, base_missing, &repo_root)?,
		&args.rules,
	)?;

	if args.json {
		println!(
			"{}",
			serde_json::to_string_pretty(&config_evaluation_json(
				&config,
				&changed_files,
				base_missing,
				&evaluation
			))?
		);
		return Ok(());
	}

	render_config_evaluation(&config, &evaluation, args.all_matches, false);
	Ok(())
}

fn validate_config_command(args: &ValidateArgs) -> Result<()> {
	let renderer = Renderer::new(args.render);
	let (_, _, config) = load_config_from_cwd(args.debug, args.config.as_deref())?;

	if args.json {
		println!("{}", serde_json::to_string_pretty(&config_summary_json(&config))?);
		return Ok(());
	}

	renderer.render_message_stage(&format!("config valid: {}", config.path.display()));
	renderer.render_message_stage(&format!(
		"entries: {} | rules: {} | parallel groups: {}",
		config.entries.len(),
		count_config_rules(&config),
		count_config_groups(&config)
	));
	Ok(())
}

fn doctor_command(args: &DoctorArgs) -> Result<()> {
	let cwd = std::env::current_dir().context("failed to read current working directory")?;
	let repo = GitRepo::discover(&cwd, args.debug).context("failed to resolve repository root")?;
	let repo_root = repo.root().to_path_buf();
	let checks = build_doctor_checks(&repo, &repo_root, args.config.as_deref());

	if args.json {
		println!(
			"{}",
			serde_json::to_string_pretty(&json!({
				"repoRoot": repo_root.display().to_string(),
				"checks": checks.iter().map(doctor_check_json).collect::<Vec<_>>(),
				"summary": doctor_summary_json(&checks),
			}))?
		);
	} else {
		render_doctor_checks(&checks, &repo_root);
	}

	if checks.iter().any(|check| check.level == DoctorLevel::Error) {
		return Err(anyhow!("doctor found blocking issues"));
	}

	Ok(())
}

fn rules_command(args: &RulesArgs) -> Result<()> {
	let renderer = Renderer::new(args.render);
	let (_, _, config) = load_config_from_cwd(args.debug, args.config.as_deref())?;

	if args.json {
		println!("{}", serde_json::to_string_pretty(&config_rules_json(&config))?);
		return Ok(());
	}

	renderer.render_message_stage(&format!("config: {}", config.path.display()));
	renderer.render_message_stage(&format!(
		"entries: {} | rules: {} | parallel groups: {}",
		config.entries.len(),
		count_config_rules(&config),
		count_config_groups(&config)
	));
	println!();
	println!("Rules");
	for entry in &config.entries {
		match entry {
			Entry::Rule(rule) => render_config_rule_inventory(rule),
			Entry::Group(group) => render_config_group_inventory(group),
		}
	}

	Ok(())
}

fn init_config_command(args: &InitArgs) -> Result<()> {
	let cwd = std::env::current_dir().context("failed to read current working directory")?;
	let repo = GitRepo::discover(&cwd, args.debug).context("failed to resolve repository root")?;
	let repo_root = repo.root();
	let requested_format = args.format.map_or(config::ConfigFormat::Json, Into::into);
	let existing_path = config::discover(repo_root)?;

	if args.stdout {
		print!("{}", requested_format.starter_config());
		return Ok(());
	}

	let renderer = Renderer::new(args.render);
	let (path, format) = match existing_path {
		Some(path) if !args.force => {
			return Err(anyhow!(
				"pullhook config already exists: {} (rerun with `pullhook init --force` to overwrite it)",
				path.display()
			));
		}
		Some(path) => {
			let existing_format = config::ConfigFormat::from_path(&path)?;
			let format = args.format.map_or(existing_format, Into::into);
			if format != existing_format {
				return Err(anyhow!(
					"existing config `{}` is {}; rerun without `--format` to overwrite it in place or remove it first",
					path.display(),
					existing_format.default_name()
				));
			}
			(path, format)
		}
		None => {
			let path = repo_root.join(requested_format.default_name());
			(path, requested_format)
		}
	};

	if path.exists() && !args.force {
		return Err(anyhow!(
			"refusing to overwrite existing file `{}`; rerun with `pullhook init --force`",
			path.display()
		));
	}

	std::fs::write(&path, format.starter_config())
		.with_context(|| format!("failed to write config `{}`", path.display()))?;
	renderer.render_message_stage(&format!("created {}", path.display()));
	Ok(())
}

fn load_config_from_cwd(
	debug_enabled: bool,
	explicit_config: Option<&std::path::Path>,
) -> Result<(GitRepo, std::path::PathBuf, Config)> {
	let cwd = std::env::current_dir().context("failed to read current working directory")?;
	let repo = GitRepo::discover(&cwd, debug_enabled).context("failed to resolve repository root")?;
	let repo_root = repo.root().to_path_buf();
	let path = resolve_config_path(&cwd, &repo_root, explicit_config)?;
	let config = config::load(&path)?;
	Ok((repo, repo_root, config))
}

fn resolve_config_path(
	cwd: &std::path::Path,
	repo_root: &std::path::Path,
	explicit_config: Option<&std::path::Path>,
) -> Result<std::path::PathBuf> {
	if let Some(path) = explicit_config {
		let resolved = if path.is_absolute() {
			path.to_path_buf()
		} else {
			cwd.join(path)
		};
		return Ok(resolved);
	}

	config::discover(repo_root)?.ok_or_else(|| {
		anyhow!(
			"no pullhook config found; run `pullhook init` to create {}",
			config::config_names()[0]
		)
	})
}

fn resolve_config_changed_files(
	repo: &GitRepo,
	config: &Config,
	base: Option<&str>,
	explicit_changed_files: &[std::path::PathBuf],
	debug_enabled: bool,
) -> Result<(Vec<std::path::PathBuf>, bool)> {
	if !explicit_changed_files.is_empty() {
		if debug_enabled {
			debug!(
				changed = explicit_changed_files.len(),
				"using explicit config changed files"
			);
		}
		return Ok((explicit_changed_files.to_vec(), false));
	}

	match repo.resolve_base_and_changed_files(base, debug_enabled) {
		Ok((resolved_base, changed_files)) => {
			if debug_enabled {
				debug!(base = %resolved_base, changed = changed_files.len(), "resolved config changed files");
			}
			Ok((changed_files, false))
		}
		Err(error)
			if config_allows_base_missing(config) && matches!(error, error::PullhookError::DiffBaseUnavailable) =>
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

		let resolved = resolve_install_plan(repo_root, "failed to detect package manager for config install rule")?;
		let command = resolved.command;
		let patterns = vec![Pattern::new(resolved.pattern)?];
		install_plan = Some((command.clone(), patterns.clone()));
		Ok((Some(command), patterns))
	})
	.map_err(Into::into)
}

fn filter_config_evaluation(evaluation: Vec<EvaluatedEntry>, selectors: &[String]) -> Result<Vec<EvaluatedEntry>> {
	if selectors.is_empty() {
		return Ok(evaluation);
	}

	let requested = selectors.iter().map(String::as_str).collect::<BTreeSet<_>>();
	let mut available = BTreeSet::new();
	let mut filtered = Vec::new();

	for entry in evaluation {
		match entry {
			EvaluatedEntry::Rule(rule) => {
				let rule_name = rule.rule.name.clone();
				available.insert(rule_name.clone());
				if requested.contains(rule_name.as_str()) {
					filtered.push(EvaluatedEntry::Rule(rule));
				}
			}
			EvaluatedEntry::Group(group) => {
				let group_name = group.group.name.clone();
				let group_selected = requested.contains(group_name.as_str());
				available.insert(group_name);

				if group_selected {
					for rule in &group.rules {
						available.insert(rule.rule.name.clone());
					}
					filtered.push(EvaluatedEntry::Group(group));
					continue;
				}

				let rules = group
					.rules
					.into_iter()
					.filter(|rule| {
						available.insert(rule.rule.name.clone());
						requested.contains(rule.rule.name.as_str())
					})
					.collect::<Vec<_>>();

				if !rules.is_empty() {
					filtered.push(EvaluatedEntry::Group(EvaluatedGroup {
						group: group.group,
						rules,
					}));
				}
			}
		}
	}

	let unknown = selectors
		.iter()
		.filter(|selector| !available.contains(selector.as_str()))
		.cloned()
		.collect::<Vec<_>>();

	if !unknown.is_empty() {
		let available = available.into_iter().collect::<Vec<_>>();
		return Err(anyhow!(
			"unknown rule selector(s): {} (available: {})",
			unknown.join(", "),
			available.join(", "),
		));
	}

	Ok(filtered)
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
					match group.group.jobs {
						Some(jobs) => println!("jobs: {jobs}"),
						None => println!("jobs: default"),
					}
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

fn build_doctor_checks(
	repo: &GitRepo,
	repo_root: &std::path::Path,
	explicit_config: Option<&std::path::Path>,
) -> Vec<DoctorCheck> {
	let cwd = std::env::current_dir().unwrap_or_else(|_| repo_root.to_path_buf());
	vec![
		doctor_repository_check(repo_root),
		doctor_config_check(&cwd, repo_root, explicit_config),
		doctor_diff_base_check(repo),
		doctor_install_check(repo_root),
	]
}

fn doctor_repository_check(repo_root: &std::path::Path) -> DoctorCheck {
	DoctorCheck {
		name: "repository",
		level: DoctorLevel::Ok,
		summary: format!("repo root resolved to {}", repo_root.display()),
		details: vec![format!("cwd scope is inside `{}`", repo_root.display())],
	}
}

fn doctor_config_check(
	cwd: &std::path::Path,
	repo_root: &std::path::Path,
	explicit_config: Option<&std::path::Path>,
) -> DoctorCheck {
	match resolve_config_path(cwd, repo_root, explicit_config) {
		Ok(path) => match config::load(&path) {
			Ok(config) => {
				let config_label = if explicit_config.is_some() {
					format!("loaded {} from --config", path.display())
				} else {
					format!("loaded {}", path.display())
				};
				DoctorCheck {
					name: "config",
					level: DoctorLevel::Ok,
					summary: config_label,
					details: vec![
						format!("entries: {}", config.entries.len()),
						format!("rules: {}", count_config_rules(&config)),
						format!("parallel groups: {}", count_config_groups(&config)),
					],
				}
			}
			Err(error) => DoctorCheck {
				name: "config",
				level: DoctorLevel::Error,
				summary: format!("config is invalid: {}", path.display()),
				details: vec![error.to_string()],
			},
		},
		Err(error) if explicit_config.is_none() && error.to_string().contains("no pullhook config found") => {
			DoctorCheck {
				name: "config",
				level: DoctorLevel::Warn,
				summary: "no pullhook config found".to_owned(),
				details: vec!["run `pullhook init` to create pullhook.json".to_owned()],
			}
		}
		Err(error) => DoctorCheck {
			name: "config",
			level: DoctorLevel::Error,
			summary: "config discovery failed".to_owned(),
			details: vec![error.to_string()],
		},
	}
}

fn doctor_diff_base_check(repo: &GitRepo) -> DoctorCheck {
	match repo.resolve_base_and_changed_files(None, false) {
		Ok((base, changed_files)) => DoctorCheck {
			name: "diff base",
			level: DoctorLevel::Ok,
			summary: format!("resolved {base}"),
			details: vec![format!("changed files: {}", changed_files.len())],
		},
		Err(error::PullhookError::DiffBaseUnavailable) => DoctorCheck {
			name: "diff base",
			level: DoctorLevel::Warn,
			summary: "no automatic diff base available".to_owned(),
			details: vec!["use `--base <rev>` or rely on `runIfBaseMissing` rules".to_owned()],
		},
		Err(error) => DoctorCheck {
			name: "diff base",
			level: DoctorLevel::Error,
			summary: "failed to inspect git history".to_owned(),
			details: vec![error.to_string()],
		},
	}
}

fn doctor_install_check(repo_root: &std::path::Path) -> DoctorCheck {
	match detect_package_manager(repo_root) {
		Ok(package_manager) => DoctorCheck {
			name: "install detection",
			level: DoctorLevel::Ok,
			summary: format!("detected {}", package_manager.name()),
			details: vec![
				format!("command: {}", package_manager.install_command()),
				format!("pattern: {}", package_manager.install_pattern()),
			],
		},
		Err(error::PullhookError::PackageManagerNotFound { .. }) => DoctorCheck {
			name: "install detection",
			level: DoctorLevel::Warn,
			summary: "no supported package manager files found".to_owned(),
			details: vec!["`pullhook --install` would not work in this repo yet".to_owned()],
		},
		Err(error::PullhookError::AmbiguousPackageManagers { found }) => DoctorCheck {
			name: "install detection",
			level: DoctorLevel::Error,
			summary: "multiple package managers detected".to_owned(),
			details: vec![format!("found: {}", found.join(", "))],
		},
		Err(error) => DoctorCheck {
			name: "install detection",
			level: DoctorLevel::Error,
			summary: "package-manager detection failed".to_owned(),
			details: vec![error.to_string()],
		},
	}
}

fn render_doctor_checks(checks: &[DoctorCheck], repo_root: &std::path::Path) {
	println!("Doctor");
	println!("repo: {}", repo_root.display());
	println!();

	for check in checks {
		println!("[{}] {}", check.level.label(), check.name);
		println!("summary: {}", check.summary);
		for detail in &check.details {
			println!("detail: {detail}");
		}
		println!();
	}

	let summary = doctor_summary_json(checks);
	println!("Summary");
	println!("ok: {}", summary["ok"]);
	println!("warn: {}", summary["warn"]);
	println!("error: {}", summary["error"]);
}

fn doctor_check_json(check: &DoctorCheck) -> serde_json::Value {
	json!({
		"name": check.name,
		"level": check.level.label(),
		"summary": check.summary,
		"details": check.details,
	})
}

fn doctor_summary_json(checks: &[DoctorCheck]) -> serde_json::Value {
	let ok = checks.iter().filter(|check| check.level == DoctorLevel::Ok).count();
	let warn = checks.iter().filter(|check| check.level == DoctorLevel::Warn).count();
	let error = checks.iter().filter(|check| check.level == DoctorLevel::Error).count();

	json!({
		"ok": ok,
		"warn": warn,
		"error": error,
	})
}

fn config_summary_json(config: &Config) -> serde_json::Value {
	json!({
		"valid": true,
		"path": config.path.display().to_string(),
		"onFailure": on_failure_label(config.on_failure),
		"entries": config.entries.len(),
		"rules": count_config_rules(config),
		"parallelGroups": count_config_groups(config),
	})
}

fn config_rules_json(config: &Config) -> serde_json::Value {
	json!({
		"path": config.path.display().to_string(),
		"onFailure": on_failure_label(config.on_failure),
		"entries": config.entries.iter().map(config_rule_inventory_json).collect::<Vec<_>>(),
		"rules": count_config_rules(config),
		"parallelGroups": count_config_groups(config),
	})
}

fn config_evaluation_json(
	config: &Config,
	changed_files: &[std::path::PathBuf],
	base_missing: bool,
	evaluation: &[EvaluatedEntry],
) -> serde_json::Value {
	let entries = evaluation.iter().map(config_entry_json).collect::<Vec<_>>();
	let matched_files = collect_matched_files(evaluation)
		.into_iter()
		.map(|path| path.display().to_string())
		.collect::<Vec<_>>();

	json!({
		"path": config.path.display().to_string(),
		"onFailure": on_failure_label(config.on_failure),
		"baseMissing": base_missing,
		"changedFiles": changed_files.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
		"matchedFiles": matched_files,
		"entries": entries,
	})
}

fn config_dry_run_json(
	config: &Config,
	changed_files: &[std::path::PathBuf],
	base_missing: bool,
	evaluation: &[EvaluatedEntry],
	planned_commands: usize,
) -> serde_json::Value {
	let mut value = config_evaluation_json(config, changed_files, base_missing, evaluation);
	if let Some(object) = value.as_object_mut() {
		object.insert("mode".to_owned(), json!("dry-run"));
		object.insert("plannedCommands".to_owned(), json!(planned_commands));
	}
	value
}

fn config_run_json(
	config: &Config,
	changed_files: &[std::path::PathBuf],
	base_missing: bool,
	evaluation: &[EvaluatedEntry],
	counts: TaskCounters,
	executions: Vec<serde_json::Value>,
) -> serde_json::Value {
	let mut value = config_evaluation_json(config, changed_files, base_missing, evaluation);
	if let Some(object) = value.as_object_mut() {
		object.insert("mode".to_owned(), json!("run"));
		object.insert(
			"summary".to_owned(),
			json!({
				"matchedFiles": count_config_matched_files(evaluation),
				"taskDirs": counts.task_dirs,
				"passed": counts.passed,
				"failed": counts.failed,
				"interrupted": counts.interrupted,
			}),
		);
		object.insert("executions".to_owned(), serde_json::Value::Array(executions));
	}
	value
}

fn legacy_dry_run_json(
	cli: &RunArgs,
	context: &LegacyJsonContext,
	planned_commands: usize,
	repo_root: &std::path::Path,
) -> serde_json::Value {
	let LegacyJsonContext {
		changed_count,
		run_config,
		matched_files,
		invocations,
		tasks,
	} = context;
	json!({
		"mode": "dry-run",
		"pattern": run_config.pattern,
		"command": run_config.command,
		"script": run_config.script,
		"message": cli.message,
		"once": run_config.once,
		"shell": cli.shell,
		"jobs": cli.effective_jobs(),
		"changedCount": changed_count,
		"matchedFiles": matched_files.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
		"tasks": tasks.iter().map(|cwd| runner::relative_cwd_label(cwd, repo_root)).collect::<Vec<_>>(),
		"invocations": invocations.iter().map(|invocation| invocation.display().into_owned()).collect::<Vec<_>>(),
		"plannedCommands": planned_commands,
	})
}

fn legacy_run_json(
	cli: &RunArgs,
	run_config: &RunConfig,
	changed_count: usize,
	matched_files: &[std::path::PathBuf],
	counts: TaskCounters,
	results: &[serde_json::Value],
) -> serde_json::Value {
	json!({
		"mode": "run",
		"pattern": run_config.pattern,
		"command": run_config.command,
		"script": run_config.script,
		"message": cli.message,
		"once": run_config.once,
		"shell": cli.shell,
		"jobs": cli.effective_jobs(),
		"changedCount": changed_count,
		"matchedFiles": matched_files.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
		"summary": {
			"matchedFiles": matched_files.len(),
			"taskDirs": counts.task_dirs,
			"passed": counts.passed,
			"failed": counts.failed,
			"interrupted": counts.interrupted,
		},
		"results": results,
	})
}

fn config_entry_json(entry: &EvaluatedEntry) -> serde_json::Value {
	match entry {
		EvaluatedEntry::Rule(rule) => config_rule_json(rule),
		EvaluatedEntry::Group(group) => json!({
			"type": "group",
			"name": group.group.name,
			"jobs": group.group.jobs,
			"status": if group.should_run() { "match" } else { "skip" },
			"rules": group.rules.iter().map(config_rule_json).collect::<Vec<_>>(),
		}),
	}
}

fn config_rule_inventory_json(entry: &Entry) -> serde_json::Value {
	match entry {
		Entry::Rule(rule) => json!({
			"type": "rule",
			"name": rule.name,
			"kind": if rule.install { "install" } else { "run" },
			"command": rule.run,
			"runIfBaseMissing": rule.run_if_base_missing,
		}),
		Entry::Group(group) => json!({
			"type": "group",
			"name": group.name,
			"jobs": group.jobs,
			"rules": group.rules.iter().map(|rule| json!({
				"name": rule.name,
				"kind": if rule.install { "install" } else { "run" },
				"command": rule.run,
				"runIfBaseMissing": rule.run_if_base_missing,
			})).collect::<Vec<_>>(),
		}),
	}
}

fn config_rule_json(rule: &EvaluatedRule) -> serde_json::Value {
	json!({
		"type": "rule",
		"name": rule.rule.name,
		"status": if rule.should_run() { "match" } else { "skip" },
		"matchCount": rule.matches.len(),
		"matches": rule.matches.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
		"command": rule.command,
		"install": rule.rule.install,
		"runIfBaseMissing": rule.rule.run_if_base_missing,
		"skipReason": rule.skip_reason,
	})
}

const fn on_failure_label(on_failure: OnFailure) -> &'static str {
	match on_failure {
		OnFailure::Stop => "stop",
		OnFailure::Continue => "continue",
	}
}

fn count_config_rules(config: &Config) -> usize {
	config
		.entries
		.iter()
		.map(|entry| match entry {
			Entry::Rule(_) => 1,
			Entry::Group(group) => group.rules.len(),
		})
		.sum()
}

fn render_config_rule_inventory(rule: &config::Rule) {
	println!("[rule] {}", rule.name);
	println!("kind: {}", if rule.install { "install" } else { "run" });
	if let Some(command) = &rule.run {
		println!("command: {command}");
	}
	if rule.run_if_base_missing {
		println!("runIfBaseMissing: true");
	}
	println!();
}

fn render_config_group_inventory(group: &config::Group) {
	println!("[group] {}", group.name);
	match group.jobs {
		Some(jobs) => println!("jobs: {jobs}"),
		None => println!("jobs: default"),
	}
	for rule in &group.rules {
		println!("- {}", rule.name);
		println!("  kind: {}", if rule.install { "install" } else { "run" });
		if let Some(command) = &rule.run {
			println!("  command: {command}");
		}
		if rule.run_if_base_missing {
			println!("  runIfBaseMissing: true");
		}
	}
	println!();
}

fn count_config_groups(config: &Config) -> usize {
	config
		.entries
		.iter()
		.filter(|entry| matches!(entry, Entry::Group(_)))
		.count()
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

fn count_config_matched_files(evaluation: &[EvaluatedEntry]) -> usize {
	collect_matched_files(evaluation).len()
}

fn collect_matched_files(evaluation: &[EvaluatedEntry]) -> BTreeSet<std::path::PathBuf> {
	let mut matched_files = BTreeSet::new();

	for entry in evaluation {
		match entry {
			EvaluatedEntry::Rule(rule) => collect_rule_matches(rule, &mut matched_files),
			EvaluatedEntry::Group(group) => {
				for rule in &group.rules {
					collect_rule_matches(rule, &mut matched_files);
				}
			}
		}
	}

	matched_files
}

fn collect_rule_matches(rule: &EvaluatedRule, matched_files: &mut BTreeSet<std::path::PathBuf>) {
	for path in &rule.matches {
		matched_files.insert(path.clone());
	}
}

fn execute_config_entries(
	renderer: &Renderer,
	on_failure: OnFailure,
	evaluation: &[EvaluatedEntry],
	repo_root: &std::path::Path,
	args: &ConfigRunArgs,
) -> Result<TaskCounters> {
	let mut counts = TaskCounters::default();

	for entry in evaluation {
		match entry {
			EvaluatedEntry::Rule(rule) => {
				if !rule.should_run() {
					continue;
				}
				let state = execute_config_rule(renderer, rule, repo_root, args.debug, args.render);
				counts.add_state(state);
			}
			EvaluatedEntry::Group(group) => {
				if !group.should_run() {
					continue;
				}
				counts.add(execute_config_group(renderer, group, repo_root, args)?);
			}
		}

		if counts.has_non_success() && on_failure == OnFailure::Stop {
			break;
		}
	}

	Ok(counts)
}

fn execute_config_entries_json(
	on_failure: OnFailure,
	evaluation: &[EvaluatedEntry],
	repo_root: &std::path::Path,
	args: &ConfigRunArgs,
) -> Result<(TaskCounters, Vec<serde_json::Value>)> {
	let mut counts = TaskCounters::default();
	let mut executions = Vec::new();

	for entry in evaluation {
		match entry {
			EvaluatedEntry::Rule(rule) => {
				if !rule.should_run() {
					continue;
				}
				let result = run_config_rule_task(rule, repo_root, args.debug);
				counts.add_state(result.state);
				executions.push(rule_execution_json(rule, None, &result, repo_root, args.render));
			}
			EvaluatedEntry::Group(group) => {
				if !group.should_run() {
					continue;
				}
				let results = run_config_group_tasks(group, repo_root, args)?;
				let mut group_counts = TaskCounters::default();
				for (_, result) in &results {
					group_counts.add_state(result.state);
				}
				counts.add(group_counts);
				executions.push(group_execution_json(group, &results, repo_root, args.render));
			}
		}

		if counts.has_non_success() && on_failure == OnFailure::Stop {
			break;
		}
	}

	Ok((counts, executions))
}

fn execute_config_rule(
	renderer: &Renderer,
	rule: &EvaluatedRule,
	repo_root: &std::path::Path,
	debug_enabled: bool,
	render_mode: RenderMode,
) -> runner::ResultState {
	let result = run_config_rule_task(rule, repo_root, debug_enabled);
	render_config_rule_result(renderer, rule, &result, repo_root, debug_enabled, render_mode);
	result.state
}

fn execute_config_group(
	renderer: &Renderer,
	group: &EvaluatedGroup,
	repo_root: &std::path::Path,
	args: &ConfigRunArgs,
) -> Result<TaskCounters> {
	let results = run_config_group_tasks(group, repo_root, args)?;

	let mut counts = TaskCounters::default();
	for (rule, result) in &results {
		render_config_rule_result(renderer, rule, result, repo_root, args.debug, args.render);
		counts.add_state(result.state);
	}

	if counts.has_non_success() {
		render_group_fail_text(group, args.render);
	}

	Ok(counts)
}

fn run_config_group_tasks<'a>(
	group: &'a EvaluatedGroup,
	repo_root: &std::path::Path,
	args: &ConfigRunArgs,
) -> Result<Vec<(&'a EvaluatedRule, runner::TaskResult)>> {
	let runnable: Vec<_> = group.rules.iter().filter(|rule| rule.should_run()).collect();
	let jobs = group.group.jobs.unwrap_or_else(|| args.effective_jobs()).max(1);

	if runnable.len() <= 1 || jobs <= 1 {
		return Ok(runnable
			.iter()
			.map(|rule| (*rule, run_config_rule_task(rule, repo_root, args.debug)))
			.collect::<Vec<_>>());
	}

	let pool = rayon::ThreadPoolBuilder::new()
		.num_threads(jobs)
		.build()
		.map_err(|error| anyhow!(error.to_string()))?;

	Ok(pool.install(|| {
		runnable
			.par_iter()
			.map(|rule| (*rule, run_config_rule_task(rule, repo_root, args.debug)))
			.collect::<Vec<_>>()
	}))
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
) {
	render_task_results(renderer, std::slice::from_ref(result), repo_root);
	report_debug_errors(debug_enabled, std::slice::from_ref(result));
	let failed = result.state != runner::ResultState::Success;
	if failed {
		render_rule_fail_text(rule, result, repo_root, render_mode);
	}
}

fn render_rule_fail_text(
	rule: &EvaluatedRule,
	result: &runner::TaskResult,
	repo_root: &std::path::Path,
	render_mode: RenderMode,
) {
	if let Some(message) = rule_fail_text_message(rule, result, repo_root, render_mode) {
		eprintln!("{message}");
	}
}

fn render_group_fail_text(group: &EvaluatedGroup, render_mode: RenderMode) {
	if let Some(message) = group_fail_text_message(group, render_mode) {
		eprintln!("{message}");
	}
}

fn rule_fail_text_message(
	rule: &EvaluatedRule,
	result: &runner::TaskResult,
	repo_root: &std::path::Path,
	render_mode: RenderMode,
) -> Option<String> {
	let fail_text = rule.rule.fail_text.as_ref()?;
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
	Some(fail_text.render(&context, render_mode))
}

fn group_fail_text_message(group: &EvaluatedGroup, render_mode: RenderMode) -> Option<String> {
	let fail_text = group.group.fail_text.as_ref()?;
	let context = FailTextContext {
		rule: &group.group.name,
		command: "parallel group",
		cwd: ".",
		exit_code: "1",
	};
	Some(fail_text.render(&context, render_mode))
}

fn group_execution_json(
	group: &EvaluatedGroup,
	results: &[(&EvaluatedRule, runner::TaskResult)],
	repo_root: &std::path::Path,
	render_mode: RenderMode,
) -> serde_json::Value {
	json!({
		"type": "group",
		"name": group.group.name,
		"jobs": group.group.jobs,
		"failText": group_fail_text_message(group, render_mode),
		"results": results
			.iter()
			.map(|(rule, result)| rule_execution_json(rule, Some(&group.group.name), result, repo_root, render_mode))
			.collect::<Vec<_>>(),
	})
}

fn rule_execution_json(
	rule: &EvaluatedRule,
	parent_group: Option<&str>,
	result: &runner::TaskResult,
	repo_root: &std::path::Path,
	render_mode: RenderMode,
) -> serde_json::Value {
	json!({
		"type": "rule",
		"name": rule.rule.name,
		"parentGroup": parent_group,
		"cwd": runner::relative_cwd_label(&result.cwd, repo_root),
		"state": result_state_label(result.state),
		"failText": rule_fail_text_message(rule, result, repo_root, render_mode),
		"error": result.error.as_ref().map(ToString::to_string),
		"outputs": result.outputs.iter().map(invocation_output_json).collect::<Vec<_>>(),
	})
}

fn invocation_output_json(output: &runner::InvocationOutput) -> serde_json::Value {
	json!({
		"command": output.command,
		"stdout": output.stdout,
		"stderr": output.stderr,
		"state": result_state_label(output.state),
		"exitCode": output.exit_code,
	})
}

fn task_result_json(result: &runner::TaskResult, repo_root: &std::path::Path) -> serde_json::Value {
	json!({
		"cwd": runner::relative_cwd_label(&result.cwd, repo_root),
		"state": result_state_label(result.state),
		"error": result.error.as_ref().map(ToString::to_string),
		"outputs": result.outputs.iter().map(invocation_output_json).collect::<Vec<_>>(),
	})
}

const fn result_state_label(state: runner::ResultState) -> &'static str {
	match state {
		runner::ResultState::Success => "success",
		runner::ResultState::Failed => "failed",
		runner::ResultState::Interrupted => "interrupted",
		runner::ResultState::SpawnError => "spawn_error",
	}
}

fn resolve_run_config(cli: &RunArgs, repo_root: &std::path::Path) -> Result<RunConfig> {
	let mut pattern = cli.pattern.clone().unwrap_or_default();
	let mut command = cli.command.clone();
	let script = cli.script.clone();
	let once = cli.effective_once();

	if cli.install {
		let resolved = resolve_install_plan(repo_root, "failed to detect package manager for --install")?;
		pattern = resolved.pattern;
		command = Some(resolved.command);

		if cli.debug {
			debug!(
				package_manager = resolved.package_manager,
				pattern = pattern,
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

fn resolve_install_plan(repo_root: &std::path::Path, error_context: &str) -> Result<InstallPlan, error::PullhookError> {
	let package_manager = detect_package_manager(repo_root)
		.map_err(|error| error::PullhookError::Message(format!("{error_context}: {error}")))?;

	Ok(InstallPlan {
		package_manager: package_manager.name(),
		pattern: package_manager.install_pattern(),
		command: package_manager.install_command(),
	})
}

fn collect_matches(cli: &RunArgs, repo: &GitRepo, run_config: &RunConfig) -> Result<(usize, Vec<std::path::PathBuf>)> {
	let matcher = matcher::compile(&run_config.pattern).context("failed to compile pattern")?;
	let (base, changed_count, matched_files) = repo
		.resolve_filtered_matches(cli.base.as_deref(), |path| matcher.is_match(path), cli.debug)
		.context("failed to resolve diff base or read changed files")?;

	if cli.debug {
		debug!(count = changed_count, "loaded changed files");
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

#[derive(Debug, Clone, Copy, Default)]
struct TaskCounters {
	task_dirs: usize,
	passed: usize,
	failed: usize,
	interrupted: usize,
}

impl TaskCounters {
	const fn add(&mut self, other: Self) {
		self.task_dirs += other.task_dirs;
		self.passed += other.passed;
		self.failed += other.failed;
		self.interrupted += other.interrupted;
	}

	const fn add_state(&mut self, state: runner::ResultState) {
		self.task_dirs += 1;
		match state {
			runner::ResultState::Success => self.passed += 1,
			runner::ResultState::Failed | runner::ResultState::SpawnError => self.failed += 1,
			runner::ResultState::Interrupted => self.interrupted += 1,
		}
	}

	const fn has_non_success(self) -> bool {
		self.failed > 0 || self.interrupted > 0
	}
}

fn summarize_results(results: &[runner::TaskResult]) -> TaskCounters {
	let mut counts = TaskCounters::default();

	for result in results {
		counts.add_state(result.state);
	}

	counts
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
