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
use std::io::{Read as _, Write as _};

use anyhow::{Context, Result, anyhow};
use clap::{Parser, ValueEnum};
use rayon::prelude::*;
use serde_json::json;
use tracing::debug;
use tracing_subscriber::EnvFilter;

use crate::cli::{
	Cli, Commands, CompletionArgs, ConfigArgs, ConfigRunArgs, DoctorArgs, ExplainArgs, InitArgs, RulesArgs, RulesKind,
	RunArgs, SchemaArgs, ValidateArgs,
};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChangedFilesSource {
	Git,
	Explicit,
	BaseMissing,
}

impl ChangedFilesSource {
	const fn label(self) -> &'static str {
		match self {
			Self::Git => "git",
			Self::Explicit => "explicit",
			Self::BaseMissing => "base-missing",
		}
	}
}

#[derive(Debug, Clone)]
struct DoctorCheck {
	name: &'static str,
	level: DoctorLevel,
	summary: String,
	details: Vec<String>,
	hint: Option<String>,
}

fn main() {
	let cli = Cli::parse();

	let result = match cli.command.as_ref() {
		Some(Commands::Completion(args)) => completion_command(args),
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
		Some(Commands::Config(args)) => {
			init_tracing(args.debug);
			config_command(args)
		}
		Some(Commands::Rules(args)) => {
			init_tracing(args.debug);
			rules_command(args)
		}
		Some(Commands::Schema(args)) => {
			init_tracing(false);
			schema_command(args)
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

fn completion_command(args: &CompletionArgs) -> Result<()> {
	match &args.output {
		Some(path) => {
			let completion = Cli::completion_string(args.shell);
			if args.check {
				return check_completion_output(path, args.shell, &completion, args.json);
			}

			if let Some(parent) = path.parent()
				&& !parent.as_os_str().is_empty()
			{
				std::fs::create_dir_all(parent)
					.with_context(|| format!("failed to create completion output directory `{}`", parent.display()))?;
			}
			let mut file = std::fs::File::create(path)
				.with_context(|| format!("failed to create completion output file `{}`", path.display()))?;
			file.write_all(completion.as_bytes())
				.with_context(|| format!("failed to write completion output file `{}`", path.display()))?;
			file.flush()
				.with_context(|| format!("failed to flush completion output file `{}`", path.display()))?;
		}
		None => Cli::write_completion(args.shell, &mut std::io::stdout()),
	}

	Ok(())
}

fn check_completion_output(
	path: &std::path::Path,
	shell: clap_complete::Shell,
	expected_completion: &str,
	json_output: bool,
) -> Result<()> {
	let shell_label = completion_shell_label(shell);
	let existing_completion = match std::fs::read_to_string(path) {
		Ok(completion) => completion,
		Err(error) => {
			if json_output {
				println!(
					"{}",
					serde_json::to_string_pretty(&generated_file_check_json(
						path,
						Some(&shell_label),
						false,
						false,
						Some(&format!("failed to read completion file: {error}")),
					))?
				);
			}
			return Err(anyhow!("failed to read completion file `{}`: {error}", path.display()));
		}
	};
	let matches = existing_completion == expected_completion;
	if json_output {
		let error = if matches {
			None
		} else {
			Some("completion output is out of date")
		};
		println!(
			"{}",
			serde_json::to_string_pretty(&generated_file_check_json(
				path,
				Some(&shell_label),
				true,
				matches,
				error,
			))?
		);
	}
	if matches {
		if !json_output {
			println!("completion up to date: {}", path.display());
		}
		return Ok(());
	}

	Err(anyhow!(
		"completion out of date: {} (rerun `pullhook completion {} --output {}`)",
		path.display(),
		shell_label,
		path.display()
	))
}

fn run_legacy(cli: &RunArgs) -> Result<()> {
	if cli.pattern.is_none() && !cli.install {
		let error = anyhow!("missing required argument: use `--pattern <glob>`, `--install`, or the `run` subcommand");
		if cli.json {
			print_json_error(&error)?;
		}
		return Err(error);
	}
	ensure_json_without_debug(cli.json, cli.debug)?;

	let renderer = Renderer::new(effective_render_mode(cli.render, cli.no_color));
	let (_, repo) = discover_repo_from_cwd_for_output(cli.debug, cli.json)?;
	let repo_root = repo.root().to_path_buf();
	let run_config = result_for_output(resolve_run_config(cli, &repo_root), cli.json)?;
	let (changed_count, matched_files) = result_for_output(collect_matches(cli, &repo, &run_config), cli.json)?;
	let invocations = result_for_output(
		runner::prepare_invocations(run_config.command.as_deref(), run_config.script.as_deref())
			.context("failed to prepare command invocations"),
		cli.json,
	)?;
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
				None,
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
	let error = (failure_count > 0).then(|| format!("{failure_count} task(s) failed"));
	println!(
		"{}",
		serde_json::to_string_pretty(&legacy_run_json(
			cli,
			&run_config,
			changed_count,
			&matched_files,
			counts,
			&executions,
			error.as_deref(),
		))?
	);
	if let Some(error) = error {
		return Err(anyhow!(error));
	}

	Ok(())
}

fn run_config_command(args: &ConfigRunArgs) -> Result<()> {
	ensure_json_without_debug(args.json, args.debug)?;

	let renderer = Renderer::new(effective_render_mode(args.render, args.no_color));
	let (repo, repo_root, config) = load_config_from_cwd_for_output(args.debug, args.config.as_deref(), args.json)?;
	let explicit_changed_files = collect_explicit_changed_files_for_output(
		&args.changed_files,
		args.changed_files_file.as_deref(),
		args.changed_files_stdin,
		args.json,
	)?;
	let (changed_files, base_missing, changed_files_source) = resolve_config_changed_files_for_output(
		&repo,
		&config,
		args.base.as_deref(),
		&explicit_changed_files,
		args.debug,
		args.json,
	)?;
	let evaluation = filter_config_evaluation_for_output(
		evaluate_config(&config, &changed_files, base_missing, &repo_root)?,
		&args.rules,
		args.json,
	)?;

	if args.json {
		return run_config_command_json(
			args,
			&config,
			&changed_files,
			base_missing,
			changed_files_source,
			&evaluation,
			&repo_root,
		);
	}

	let matched_files = count_config_matched_files(&evaluation);

	if render_config_run_planning_only_output(args, &changed_files, base_missing, changed_files_source, &evaluation) {
		return ensure_required_config_match(args.require_match, &evaluation);
	}

	if !args.quiet {
		render_config_evaluation(&config, &evaluation, args.all_matches || args.dry_run, args.dry_run);
	}

	if args.dry_run {
		let planned_commands = count_planned_commands(&evaluation);
		renderer.render_dry_run_summary_stage(DryRunSummary {
			matched_files,
			task_dirs: planned_commands,
			planned_commands,
		});
		return ensure_required_config_match(args.require_match, &evaluation);
	}

	let counts = execute_config_entries(&renderer, config.on_failure, &evaluation, &repo_root, args)?;
	let failure_count = counts.failed + counts.interrupted;
	if !args.quiet || failure_count > 0 {
		renderer.render_summary_stage(Summary {
			matched_files,
			task_dirs: counts.task_dirs,
			passed: counts.passed,
			failed: counts.failed,
			interrupted: counts.interrupted,
		});
	}
	if failure_count > 0 {
		return Err(anyhow!("{failure_count} config rule(s) failed"));
	}

	ensure_required_config_match(args.require_match, &evaluation)
}

fn run_config_command_json(
	args: &ConfigRunArgs,
	config: &Config,
	changed_files: &[std::path::PathBuf],
	base_missing: bool,
	changed_files_source: ChangedFilesSource,
	evaluation: &[EvaluatedEntry],
	repo_root: &std::path::Path,
) -> Result<()> {
	if args.dry_run {
		let planned_commands = count_planned_commands(evaluation);
		let error = required_config_match_error(args.require_match, evaluation);
		let base = config_evaluation_json(
			config,
			changed_files,
			base_missing,
			changed_files_source,
			evaluation,
			error,
		);
		println!(
			"{}",
			serde_json::to_string_pretty(&config_dry_run_json(base, planned_commands))?
		);
		if let Some(error) = error {
			return Err(anyhow!(error));
		}
		return Ok(());
	}

	let (counts, executions) = execute_config_entries_json(config.on_failure, evaluation, repo_root, args)?;
	let failure_count = counts.failed + counts.interrupted;
	let failure_error = (failure_count > 0).then(|| format!("{failure_count} config rule(s) failed"));
	let require_match_error = required_config_match_error(args.require_match, evaluation);
	let error = failure_error.as_deref().or(require_match_error);
	let base = config_evaluation_json(
		config,
		changed_files,
		base_missing,
		changed_files_source,
		evaluation,
		error,
	);
	println!(
		"{}",
		serde_json::to_string_pretty(&config_run_json(base, evaluation, counts, executions))?
	);
	if let Some(error) = failure_error {
		return Err(anyhow!(error));
	}
	if let Some(error) = require_match_error {
		return Err(anyhow!(error));
	}
	Ok(())
}

fn render_config_run_planning_only_output(
	args: &ConfigRunArgs,
	changed_files: &[std::path::PathBuf],
	base_missing: bool,
	changed_files_source: ChangedFilesSource,
	evaluation: &[EvaluatedEntry],
) -> bool {
	if args.summary_only {
		render_config_evaluation_summary(changed_files, base_missing, changed_files_source, evaluation);
		return true;
	}

	if args.commands_only {
		render_config_evaluation_commands(evaluation);
		return true;
	}

	if args.changed_files_only {
		render_path_list(changed_files);
		return true;
	}

	if args.matched_files_only {
		render_config_evaluation_matched_files(evaluation);
		return true;
	}

	if args.matched_rules_only {
		render_config_evaluation_matched_rules(evaluation);
		return true;
	}

	false
}

fn ensure_required_config_match(require_match: bool, evaluation: &[EvaluatedEntry]) -> Result<()> {
	if let Some(error) = required_config_match_error(require_match, evaluation) {
		return Err(anyhow!(error));
	}

	Ok(())
}

fn required_config_match_error(require_match: bool, evaluation: &[EvaluatedEntry]) -> Option<&'static str> {
	if require_match && count_planned_commands(evaluation) == 0 {
		return Some("no config rules matched changed files");
	}

	None
}

fn explain_config_command(args: &ExplainArgs) -> Result<()> {
	ensure_json_without_debug(args.json, args.debug)?;

	let (repo, repo_root, config) = load_config_from_cwd_for_output(args.debug, args.config.as_deref(), args.json)?;
	let explicit_changed_files = collect_explicit_changed_files_for_output(
		&args.changed_files,
		args.changed_files_file.as_deref(),
		args.changed_files_stdin,
		args.json,
	)?;
	let (changed_files, base_missing, changed_files_source) = resolve_config_changed_files_for_output(
		&repo,
		&config,
		args.base.as_deref(),
		&explicit_changed_files,
		args.debug,
		args.json,
	)?;
	let evaluation = filter_config_evaluation_for_output(
		evaluate_config(&config, &changed_files, base_missing, &repo_root)?,
		&args.rules,
		args.json,
	)?;

	if args.json {
		let error = required_config_match_error(args.require_match, &evaluation);
		println!(
			"{}",
			serde_json::to_string_pretty(&config_evaluation_json(
				&config,
				&changed_files,
				base_missing,
				changed_files_source,
				&evaluation,
				error,
			))?
		);
		if let Some(error) = error {
			return Err(anyhow!(error));
		}
		return Ok(());
	}

	if args.summary_only {
		render_config_evaluation_summary(&changed_files, base_missing, changed_files_source, &evaluation);
		return ensure_required_config_match(args.require_match, &evaluation);
	}

	if args.commands_only {
		render_config_evaluation_commands(&evaluation);
		return ensure_required_config_match(args.require_match, &evaluation);
	}

	if args.changed_files_only {
		render_path_list(&changed_files);
		return ensure_required_config_match(args.require_match, &evaluation);
	}

	if args.matched_files_only {
		render_config_evaluation_matched_files(&evaluation);
		return ensure_required_config_match(args.require_match, &evaluation);
	}

	if args.matched_rules_only {
		render_config_evaluation_matched_rules(&evaluation);
		return ensure_required_config_match(args.require_match, &evaluation);
	}

	render_config_evaluation(&config, &evaluation, args.all_matches, false);
	ensure_required_config_match(args.require_match, &evaluation)
}

fn validate_config_command(args: &ValidateArgs) -> Result<()> {
	ensure_json_without_debug(args.json, args.debug)?;

	let renderer = Renderer::new(effective_render_mode(args.render, args.no_color));

	if args.json {
		let (cwd, repo) = discover_repo_from_cwd_for_output(args.debug, args.json)?;
		let repo_root = repo.root().to_path_buf();
		let path = match resolve_config_path(&cwd, &repo_root, args.config.as_deref()) {
			Ok(path) => path,
			Err(error) => {
				println!(
					"{}",
					serde_json::to_string_pretty(&config_validation_error_json(None, &error.to_string()))?
				);
				return Err(error);
			}
		};
		let config = match config::load(&path) {
			Ok(config) => config,
			Err(error) => {
				println!(
					"{}",
					serde_json::to_string_pretty(&config_validation_error_json(Some(&path), &error.to_string()))?
				);
				return Err(anyhow!("config invalid"));
			}
		};
		println!("{}", serde_json::to_string_pretty(&config_summary_json(&config))?);
		return Ok(());
	}

	let (_, _, config) = load_config_from_cwd(args.debug, args.config.as_deref())?;
	if args.quiet {
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
	ensure_json_without_debug(args.json, args.debug)?;

	let (_, repo) = discover_repo_from_cwd_for_output(args.debug, args.json)?;
	let repo_root = repo.root().to_path_buf();
	let checks = build_doctor_checks(&repo, &repo_root, args.config.as_deref());
	let blocking_error = checks.iter().any(|check| check.level == DoctorLevel::Error);
	let strict_warning = args.strict && checks.iter().any(|check| check.level == DoctorLevel::Warn);
	let error = if blocking_error {
		Some("doctor found blocking issues")
	} else if strict_warning {
		Some("doctor found warnings in strict mode")
	} else {
		None
	};

	if args.json {
		println!(
			"{}",
			serde_json::to_string_pretty(&doctor_report_json(&repo_root, &checks, error))?
		);
	} else if !args.quiet || checks.iter().any(|check| check.level != DoctorLevel::Ok) {
		render_doctor_checks(&checks, &repo_root);
	}

	if let Some(error) = error {
		return Err(anyhow!(error));
	}
	Ok(())
}

fn config_command(args: &ConfigArgs) -> Result<()> {
	ensure_json_without_debug(args.json, args.debug)?;
	if args.path_only && args.debug {
		return Err(anyhow!("--path-only cannot be used with --debug"));
	}

	let (cwd, repo) = discover_repo_from_cwd_for_output(args.debug, args.json)?;
	let repo_root = repo.root().to_path_buf();
	let path = resolve_config_path_for_output(&cwd, &repo_root, args.config.as_deref(), args.json)?;
	let format = config_format_from_path_for_output(&path, args.json)?;
	let explicit = args.config.is_some();
	let exists = path.is_file();
	if args.require_existing && !exists {
		if args.json {
			println!(
				"{}",
				serde_json::to_string_pretty(&json!({
					"status": "error",
					"path": path.display().to_string(),
					"format": format.label(),
					"exists": false,
					"explicit": explicit,
					"repoRoot": repo_root.display().to_string(),
					"error": "resolved config file does not exist",
				}))?
			);
		}
		return Err(anyhow!("resolved config file does not exist: {}", path.display()));
	}

	if args.path_only {
		println!("{}", path.display());
		return Ok(());
	}

	if args.json {
		println!(
			"{}",
			serde_json::to_string_pretty(&json!({
				"status": "ok",
				"path": path.display().to_string(),
				"format": format.label(),
				"exists": exists,
				"explicit": explicit,
				"repoRoot": repo_root.display().to_string(),
				"error": null,
			}))?
		);
		return Ok(());
	}

	let renderer = Renderer::new(effective_render_mode(args.render, args.no_color));
	renderer.render_message_stage(&format!("config: {}", path.display()));
	renderer.render_message_stage(&format!("format: {}", format.label()));
	renderer.render_message_stage(if exists { "exists: yes" } else { "exists: no" });
	renderer.render_message_stage(if explicit {
		"source: explicit"
	} else {
		"source: discovered"
	});
	Ok(())
}

fn rules_command(args: &RulesArgs) -> Result<()> {
	ensure_json_without_debug(args.json, args.debug)?;

	let renderer = Renderer::new(effective_render_mode(args.render, args.no_color));
	let (_, _, config) = load_config_from_cwd_for_output(args.debug, args.config.as_deref(), args.json)?;

	if args.json {
		println!(
			"{}",
			serde_json::to_string_pretty(&config_rules_json(&config, args.kind))?
		);
		return Ok(());
	}

	if args.names_only {
		for selector in collect_rule_selectors_for_kind(&config, args.kind) {
			println!("{selector}");
		}
		return Ok(());
	}

	if args.commands_only {
		for command in collect_config_rule_commands_for_kind(&config, args.kind) {
			println!("{command}");
		}
		return Ok(());
	}

	if args.patterns_only {
		for pattern in collect_config_rule_patterns_for_kind(&config, args.kind) {
			println!("{pattern}");
		}
		return Ok(());
	}

	renderer.render_message_stage(&format!("config: {}", config.path.display()));
	renderer.render_message_stage(&format!(
		"entries: {} | rules: {} | parallel groups: {}",
		config.entries.len(),
		count_config_rules_for_kind(&config, args.kind),
		count_config_groups_for_kind(&config, args.kind)
	));
	println!();
	println!("Rules");
	for entry in &config.entries {
		match entry {
			Entry::Rule(rule) => {
				if rules_kind_matches_rule(args.kind, rule) {
					render_config_rule_inventory(rule);
				}
			}
			Entry::Group(group) => render_config_group_inventory(group, args.kind),
		}
	}

	Ok(())
}

fn schema_command(args: &SchemaArgs) -> Result<()> {
	if let Some(path) = args.output.as_deref() {
		if args.check {
			return check_schema_output(path, args.json);
		}

		if let Some(parent) = path.parent()
			&& !parent.as_os_str().is_empty()
		{
			std::fs::create_dir_all(parent)
				.with_context(|| format!("failed to create schema directory `{}`", parent.display()))?;
		}
		std::fs::write(path, config::CONFIG_SCHEMA_JSON)
			.with_context(|| format!("failed to write schema `{}`", path.display()))?;
		return Ok(());
	}

	print!("{}", config::CONFIG_SCHEMA_JSON);
	Ok(())
}

fn check_schema_output(path: &std::path::Path, json_output: bool) -> Result<()> {
	let existing_schema = match std::fs::read_to_string(path) {
		Ok(schema) => schema,
		Err(error) => {
			if json_output {
				println!(
					"{}",
					serde_json::to_string_pretty(&schema_check_json(
						path,
						false,
						false,
						Some(&format!("failed to read schema file: {error}")),
					))?
				);
			}
			return Err(anyhow!("failed to read schema file `{}`: {error}", path.display()));
		}
	};
	let matches = existing_schema == config::CONFIG_SCHEMA_JSON;
	if json_output {
		let error = if matches {
			None
		} else {
			Some("schema output is out of date")
		};
		println!(
			"{}",
			serde_json::to_string_pretty(&schema_check_json(path, true, matches, error))?
		);
	}
	if matches {
		if !json_output {
			println!("schema up to date: {}", path.display());
		}
		return Ok(());
	}

	Err(anyhow!(
		"schema out of date: {} (rerun `pullhook schema --output {}`)",
		path.display(),
		path.display()
	))
}

fn ensure_json_without_debug(json_output: bool, debug_enabled: bool) -> Result<()> {
	if json_output && debug_enabled {
		let error = anyhow!("--json cannot be used with --debug");
		print_json_error(&error)?;
		return Err(error);
	}
	Ok(())
}

const fn effective_render_mode(render: RenderMode, no_color: bool) -> RenderMode {
	if no_color { RenderMode::Never } else { render }
}

fn init_config_command(args: &InitArgs) -> Result<()> {
	ensure_json_without_debug(args.json, args.debug)?;

	let requested_format = args.format.map_or(config::ConfigFormat::Json, Into::into);
	if args.stdout {
		print!("{}", requested_format.starter_config());
		return Ok(());
	}

	let (cwd, repo) = discover_repo_from_cwd_for_output(args.debug, args.json)?;
	let repo_root = repo.root();

	let renderer = Renderer::new(effective_render_mode(args.render, args.no_color));
	let path_and_format = args.output.as_deref().map_or_else(
		|| resolve_default_init_output(repo_root, args.format, requested_format, args.force),
		|output| resolve_init_output_path(&cwd, output, args.format),
	);
	let (path, format) = match path_and_format {
		Ok(resolved) => resolved,
		Err(error) => {
			if args.json {
				print_json_error(&error)?;
			}
			return Err(error);
		}
	};
	let exists = path.exists();

	if exists && !args.force {
		let error = format!(
			"refusing to overwrite existing file `{}`; rerun with `pullhook init --force`",
			path.display()
		);
		if args.json {
			println!(
				"{}",
				serde_json::to_string_pretty(&init_plan_json(
					&path,
					format,
					exists,
					args.force,
					args.dry_run,
					Some(&error)
				))?
			);
		}
		return Err(anyhow!(error));
	}

	if args.dry_run {
		if args.json {
			println!(
				"{}",
				serde_json::to_string_pretty(&init_plan_json(&path, format, exists, args.force, true, None))?
			);
		} else {
			renderer.render_message_stage(&format!(
				"would {} {}",
				if exists { "overwrite" } else { "create" },
				path.display()
			));
			renderer.render_message_stage(&format!("format: {}", format.label()));
		}
		return Ok(());
	}

	if let Some(parent) = path.parent()
		&& !parent.as_os_str().is_empty()
	{
		std::fs::create_dir_all(parent)
			.with_context(|| format!("failed to create config directory `{}`", parent.display()))?;
	}

	std::fs::write(&path, format.starter_config())
		.with_context(|| format!("failed to write config `{}`", path.display()))?;
	if args.json {
		println!(
			"{}",
			serde_json::to_string_pretty(&init_plan_json(&path, format, exists, args.force, false, None))?
		);
	} else {
		renderer.render_message_stage(&format!(
			"{} {}",
			if exists { "overwrote" } else { "created" },
			path.display()
		));
	}
	Ok(())
}

fn resolve_default_init_output(
	repo_root: &std::path::Path,
	format_arg: Option<cli::InitFormat>,
	requested_format: config::ConfigFormat,
	force: bool,
) -> Result<(std::path::PathBuf, config::ConfigFormat)> {
	let existing_path = config::discover(repo_root)?;
	let output = if let Some(path) = existing_path {
		let existing_format = config::ConfigFormat::from_path(&path)?;
		let format = format_arg.map_or(existing_format, Into::into);
		if force && format != existing_format {
			return Err(anyhow!(
				"existing config `{}` is {}; rerun without `--format` to overwrite it in place or remove it first",
				path.display(),
				existing_format.default_name()
			));
		}
		(path, format)
	} else {
		let path = repo_root.join(requested_format.default_name());
		(path, requested_format)
	};
	Ok(output)
}

fn resolve_init_output_path(
	cwd: &std::path::Path,
	output: &std::path::Path,
	format_arg: Option<cli::InitFormat>,
) -> Result<(std::path::PathBuf, config::ConfigFormat)> {
	let path = if output.is_absolute() {
		output.to_path_buf()
	} else {
		cwd.join(output)
	};
	let extension_format = config::ConfigFormat::from_path(&path)?;
	let format = format_arg.map_or(extension_format, Into::into);
	if format != extension_format {
		return Err(anyhow!(
			"output path `{}` uses {}; choose a matching `--format` or file extension",
			path.display(),
			extension_format.default_name()
		));
	}
	Ok((path, format))
}

fn discover_repo_from_cwd_for_output(debug_enabled: bool, json_output: bool) -> Result<(std::path::PathBuf, GitRepo)> {
	let cwd = std::env::current_dir().context("failed to read current working directory")?;
	match GitRepo::discover(&cwd, debug_enabled).context("failed to resolve repository root") {
		Ok(repo) => Ok((cwd, repo)),
		Err(error) => {
			if json_output {
				print_json_error(&error)?;
			}
			Err(error)
		}
	}
}

fn resolve_config_path_for_output(
	cwd: &std::path::Path,
	repo_root: &std::path::Path,
	explicit_config: Option<&std::path::Path>,
	json_output: bool,
) -> Result<std::path::PathBuf> {
	match resolve_config_path(cwd, repo_root, explicit_config) {
		Ok(path) => Ok(path),
		Err(error) => {
			if json_output {
				print_json_error(&error)?;
			}
			Err(error)
		}
	}
}

fn config_format_from_path_for_output(path: &std::path::Path, json_output: bool) -> Result<config::ConfigFormat> {
	match config::ConfigFormat::from_path(path) {
		Ok(format) => Ok(format),
		Err(error) => {
			let error = anyhow!(error);
			if json_output {
				print_json_error(&error)?;
			}
			Err(error)
		}
	}
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

fn load_config_from_cwd_for_output(
	debug_enabled: bool,
	explicit_config: Option<&std::path::Path>,
	json_output: bool,
) -> Result<(GitRepo, std::path::PathBuf, Config)> {
	match load_config_from_cwd(debug_enabled, explicit_config) {
		Ok(loaded) => Ok(loaded),
		Err(error) => {
			if json_output {
				print_json_error(&error)?;
			}
			Err(error)
		}
	}
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

fn collect_explicit_changed_files(
	changed_files: &[std::path::PathBuf],
	changed_files_file: Option<&std::path::Path>,
	read_stdin: bool,
) -> Result<Vec<std::path::PathBuf>> {
	let mut paths = changed_files.to_vec();
	if let Some(path) = changed_files_file {
		let input = std::fs::read_to_string(path)
			.with_context(|| format!("failed to read changed files from `{}`", path.display()))?;
		extend_changed_files_from_lines(&mut paths, &input);
	}
	if read_stdin {
		let mut input = String::new();
		std::io::stdin()
			.read_to_string(&mut input)
			.context("failed to read changed files from stdin")?;
		extend_changed_files_from_lines(&mut paths, &input);
	}
	Ok(dedupe_paths_preserving_order(paths))
}

fn collect_explicit_changed_files_for_output(
	changed_files: &[std::path::PathBuf],
	changed_files_file: Option<&std::path::Path>,
	read_stdin: bool,
	json_output: bool,
) -> Result<Vec<std::path::PathBuf>> {
	match collect_explicit_changed_files(changed_files, changed_files_file, read_stdin) {
		Ok(paths) => Ok(paths),
		Err(error) => {
			if json_output {
				print_json_error(&error)?;
			}
			Err(error)
		}
	}
}

fn extend_changed_files_from_lines(paths: &mut Vec<std::path::PathBuf>, input: &str) {
	paths.extend(
		input
			.lines()
			.map(str::trim)
			.filter(|line| !line.is_empty())
			.map(std::path::PathBuf::from),
	);
}

fn dedupe_paths_preserving_order(paths: Vec<std::path::PathBuf>) -> Vec<std::path::PathBuf> {
	let mut seen = BTreeSet::new();
	paths.into_iter().filter(|path| seen.insert(path.clone())).collect()
}

fn resolve_config_changed_files(
	repo: &GitRepo,
	config: &Config,
	base: Option<&str>,
	explicit_changed_files: &[std::path::PathBuf],
	debug_enabled: bool,
) -> Result<(Vec<std::path::PathBuf>, bool, ChangedFilesSource)> {
	if !explicit_changed_files.is_empty() {
		if debug_enabled {
			debug!(
				changed = explicit_changed_files.len(),
				"using explicit config changed files"
			);
		}
		return Ok((explicit_changed_files.to_vec(), false, ChangedFilesSource::Explicit));
	}

	match repo.resolve_base_and_changed_files(base, debug_enabled) {
		Ok((resolved_base, changed_files)) => {
			if debug_enabled {
				debug!(base = %resolved_base, changed = changed_files.len(), "resolved config changed files");
			}
			Ok((changed_files, false, ChangedFilesSource::Git))
		}
		Err(error)
			if config_allows_base_missing(config) && matches!(error, error::PullhookError::DiffBaseUnavailable) =>
		{
			if debug_enabled {
				debug!("diff base missing; evaluating runIfBaseMissing rules");
			}
			Ok((Vec::new(), true, ChangedFilesSource::BaseMissing))
		}
		Err(error) => Err(error).context("failed to resolve diff base or read changed files"),
	}
}

fn resolve_config_changed_files_for_output(
	repo: &GitRepo,
	config: &Config,
	base: Option<&str>,
	explicit_changed_files: &[std::path::PathBuf],
	debug_enabled: bool,
	json_output: bool,
) -> Result<(Vec<std::path::PathBuf>, bool, ChangedFilesSource)> {
	match resolve_config_changed_files(repo, config, base, explicit_changed_files, debug_enabled) {
		Ok(resolved) => Ok(resolved),
		Err(error) => {
			if json_output {
				print_json_error(&error)?;
			}
			Err(error)
		}
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
			format_unknown_selectors(&unknown, &available),
			available.join(", "),
		));
	}

	Ok(filtered)
}

fn filter_config_evaluation_for_output(
	evaluation: Vec<EvaluatedEntry>,
	selectors: &[String],
	json_output: bool,
) -> Result<Vec<EvaluatedEntry>> {
	match filter_config_evaluation(evaluation, selectors) {
		Ok(evaluation) => Ok(evaluation),
		Err(error) => {
			if json_output {
				print_json_error(&error)?;
			}
			Err(error)
		}
	}
}

fn result_for_output<T>(result: Result<T>, json_output: bool) -> Result<T> {
	match result {
		Ok(value) => Ok(value),
		Err(error) => {
			if json_output {
				print_json_error(&error)?;
			}
			Err(error)
		}
	}
}

fn print_json_error(error: &anyhow::Error) -> Result<()> {
	println!(
		"{}",
		serde_json::to_string_pretty(&json!({
			"status": "error",
			"error": error.to_string(),
		}))?
	);
	Ok(())
}

fn format_unknown_selectors(unknown: &[String], available: &[String]) -> String {
	unknown
		.iter()
		.map(|selector| {
			closest_selector(selector, available).map_or_else(
				|| selector.clone(),
				|suggestion| format!("{selector} (did you mean `{suggestion}`?)"),
			)
		})
		.collect::<Vec<_>>()
		.join(", ")
}

fn closest_selector<'a>(selector: &str, available: &'a [String]) -> Option<&'a str> {
	let max_distance = (selector.chars().count() / 3).max(2);
	available
		.iter()
		.map(|candidate| (edit_distance(selector, candidate), candidate.as_str()))
		.filter(|(distance, _)| *distance <= max_distance)
		.min_by(|(left_distance, left), (right_distance, right)| {
			left_distance.cmp(right_distance).then_with(|| left.cmp(right))
		})
		.map(|(_, candidate)| candidate)
}

fn edit_distance(left: &str, right: &str) -> usize {
	let right_chars = right.chars().collect::<Vec<_>>();
	let mut previous = (0..=right_chars.len()).collect::<Vec<_>>();
	let mut current = vec![0; right_chars.len() + 1];

	for (left_index, left_char) in left.chars().enumerate() {
		current[0] = left_index + 1;
		for (right_index, right_char) in right_chars.iter().enumerate() {
			let substitution_cost = usize::from(left_char != *right_char);
			current[right_index + 1] = (previous[right_index + 1] + 1)
				.min(current[right_index] + 1)
				.min(previous[right_index] + substitution_cost);
		}
		std::mem::swap(&mut previous, &mut current);
	}

	previous[right_chars.len()]
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

fn render_config_evaluation_summary(
	changed_files: &[std::path::PathBuf],
	base_missing: bool,
	changed_files_source: ChangedFilesSource,
	evaluation: &[EvaluatedEntry],
) {
	println!("changedFilesSource: {}", changed_files_source.label());
	println!("baseMissing: {base_missing}");
	println!("changedFiles: {}", changed_files.len());
	println!("matchedFiles: {}", count_config_matched_files(evaluation));
	println!("plannedCommands: {}", count_planned_commands(evaluation));
}

fn render_config_evaluation_commands(evaluation: &[EvaluatedEntry]) {
	for command in collect_planned_commands(evaluation) {
		println!("{command}");
	}
}

fn render_config_evaluation_matched_files(evaluation: &[EvaluatedEntry]) {
	render_path_list(collect_matched_files(evaluation));
}

fn render_config_evaluation_matched_rules(evaluation: &[EvaluatedEntry]) {
	for rule_name in collect_matched_rules(evaluation) {
		println!("{rule_name}");
	}
}

fn render_path_list(paths: impl IntoIterator<Item = impl AsRef<std::path::Path>>) {
	for path in paths {
		println!("{}", path.as_ref().display());
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
		hint: Some("run pullhook commands from anywhere inside this repository".to_owned()),
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
					hint: Some("run `pullhook explain --all-matches` to preview rule matches".to_owned()),
				}
			}
			Err(error) => DoctorCheck {
				name: "config",
				level: DoctorLevel::Error,
				summary: format!("config is invalid: {}", path.display()),
				details: vec![error.to_string()],
				hint: Some("run `pullhook validate` after editing the config".to_owned()),
			},
		},
		Err(error) if explicit_config.is_none() && error.to_string().contains("no pullhook config found") => {
			DoctorCheck {
				name: "config",
				level: DoctorLevel::Warn,
				summary: "no pullhook config found".to_owned(),
				details: vec!["run `pullhook init` to create pullhook.json".to_owned()],
				hint: Some("run `pullhook init` to create a starter config".to_owned()),
			}
		}
		Err(error) => DoctorCheck {
			name: "config",
			level: DoctorLevel::Error,
			summary: "config discovery failed".to_owned(),
			details: vec![error.to_string()],
			hint: Some("fix the config path or pass `--config <path>` explicitly".to_owned()),
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
			hint: Some("pass `--base <rev>` to compare against a specific revision".to_owned()),
		},
		Err(error::PullhookError::DiffBaseUnavailable) => DoctorCheck {
			name: "diff base",
			level: DoctorLevel::Warn,
			summary: "no automatic diff base available".to_owned(),
			details: vec!["use `--base <rev>` or rely on `runIfBaseMissing` rules".to_owned()],
			hint: Some("run with `--base <rev>` or add `runIfBaseMissing: true` to recovery rules".to_owned()),
		},
		Err(error) => DoctorCheck {
			name: "diff base",
			level: DoctorLevel::Error,
			summary: "failed to inspect git history".to_owned(),
			details: vec![error.to_string()],
			hint: Some("check git history or pass `--base <rev>` explicitly".to_owned()),
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
			hint: Some("use `install: true` for dependency-recovery rules".to_owned()),
		},
		Err(error::PullhookError::PackageManagerNotFound { .. }) => DoctorCheck {
			name: "install detection",
			level: DoctorLevel::Warn,
			summary: "no supported package manager files found".to_owned(),
			details: vec!["`pullhook --install` would not work in this repo yet".to_owned()],
			hint: Some("add a supported lockfile or use explicit `run` commands instead".to_owned()),
		},
		Err(error::PullhookError::AmbiguousPackageManagers { found }) => DoctorCheck {
			name: "install detection",
			level: DoctorLevel::Error,
			summary: "multiple package managers detected".to_owned(),
			details: vec![format!("found: {}", found.join(", "))],
			hint: Some("remove extra lockfiles so package-manager detection is unambiguous".to_owned()),
		},
		Err(error) => DoctorCheck {
			name: "install detection",
			level: DoctorLevel::Error,
			summary: "package-manager detection failed".to_owned(),
			details: vec![error.to_string()],
			hint: Some("fix package-manager files or avoid `install: true` rules".to_owned()),
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
		if let Some(hint) = &check.hint {
			println!("hint: {hint}");
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
		"hint": check.hint,
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

fn doctor_report_json(repo_root: &std::path::Path, checks: &[DoctorCheck], error: Option<&str>) -> serde_json::Value {
	json!({
		"status": if error.is_some() { "error" } else { "ok" },
		"repoRoot": repo_root.display().to_string(),
		"checks": checks.iter().map(doctor_check_json).collect::<Vec<_>>(),
		"summary": doctor_summary_json(checks),
		"error": error,
	})
}

fn config_summary_json(config: &Config) -> serde_json::Value {
	json!({
		"status": "ok",
		"valid": true,
		"path": config.path.display().to_string(),
		"onFailure": on_failure_label(config.on_failure),
		"entries": config.entries.len(),
		"rules": count_config_rules(config),
		"parallelGroups": count_config_groups(config),
		"error": null,
	})
}

fn config_validation_error_json(path: Option<&std::path::Path>, error: &str) -> serde_json::Value {
	json!({
		"status": "error",
		"valid": false,
		"path": path.map(|path| path.display().to_string()),
		"error": error,
	})
}

fn init_plan_json(
	path: &std::path::Path,
	format: config::ConfigFormat,
	existed: bool,
	force: bool,
	dry_run: bool,
	error: Option<&str>,
) -> serde_json::Value {
	json!({
		"status": if error.is_some() { "error" } else { "ok" },
		"path": path.display().to_string(),
		"format": format.label(),
		"existed": existed,
		"force": force,
		"dryRun": dry_run,
		"action": if existed { "overwrite" } else { "create" },
		"written": !dry_run && error.is_none(),
		"error": error,
	})
}

fn schema_check_json(path: &std::path::Path, exists: bool, matches: bool, error: Option<&str>) -> serde_json::Value {
	json!({
		"status": if error.is_some() { "error" } else { "ok" },
		"error": error,
		"path": path.display().to_string(),
		"exists": exists,
		"matches": matches,
	})
}

fn generated_file_check_json(
	path: &std::path::Path,
	shell: Option<&str>,
	exists: bool,
	matches: bool,
	error: Option<&str>,
) -> serde_json::Value {
	json!({
		"status": if error.is_some() { "error" } else { "ok" },
		"error": error,
		"path": path.display().to_string(),
		"shell": shell,
		"exists": exists,
		"matches": matches,
	})
}

fn completion_shell_label(shell: clap_complete::Shell) -> String {
	shell
		.to_possible_value()
		.map_or_else(|| "unknown".to_owned(), |value| value.get_name().to_owned())
}

fn config_rules_json(config: &Config, kind: RulesKind) -> serde_json::Value {
	json!({
		"status": "ok",
		"error": serde_json::Value::Null,
		"path": config.path.display().to_string(),
		"onFailure": on_failure_label(config.on_failure),
		"kind": rules_kind_label(kind),
		"selectors": collect_rule_selectors_for_kind(config, kind),
		"entries": config.entries.iter().filter_map(|entry| config_rule_inventory_json(entry, kind)).collect::<Vec<_>>(),
		"rules": count_config_rules_for_kind(config, kind),
		"parallelGroups": count_config_groups_for_kind(config, kind),
	})
}

fn config_evaluation_json(
	config: &Config,
	changed_files: &[std::path::PathBuf],
	base_missing: bool,
	changed_files_source: ChangedFilesSource,
	evaluation: &[EvaluatedEntry],
	error: Option<&str>,
) -> serde_json::Value {
	let entries = evaluation.iter().map(config_entry_json).collect::<Vec<_>>();
	let matched_files = collect_matched_files(evaluation)
		.into_iter()
		.map(|path| path.display().to_string())
		.collect::<Vec<_>>();

	json!({
		"status": if error.is_some() { "error" } else { "ok" },
		"error": error,
		"path": config.path.display().to_string(),
		"onFailure": on_failure_label(config.on_failure),
		"baseMissing": base_missing,
		"changedFilesSource": changed_files_source.label(),
		"changedFiles": changed_files.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
		"matchedFiles": matched_files,
		"entries": entries,
	})
}

fn config_dry_run_json(mut value: serde_json::Value, planned_commands: usize) -> serde_json::Value {
	if let Some(object) = value.as_object_mut() {
		object.insert("mode".to_owned(), json!("dry-run"));
		object.insert("plannedCommands".to_owned(), json!(planned_commands));
	}
	value
}

fn config_run_json(
	mut value: serde_json::Value,
	evaluation: &[EvaluatedEntry],
	counts: TaskCounters,
	executions: Vec<serde_json::Value>,
) -> serde_json::Value {
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
		"status": "ok",
		"error": serde_json::Value::Null,
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
	error: Option<&str>,
) -> serde_json::Value {
	json!({
		"status": if error.is_some() { "error" } else { "ok" },
		"error": error,
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

fn config_rule_inventory_json(entry: &Entry, kind: RulesKind) -> Option<serde_json::Value> {
	match entry {
		Entry::Rule(rule) if rules_kind_matches_rule(kind, rule) => Some(json!({
			"type": "rule",
			"name": rule.name,
			"kind": if rule.install { "install" } else { "run" },
			"changed": rule.changed.iter().map(config::Pattern::as_str).collect::<Vec<_>>(),
			"exclude": rule.exclude.iter().map(config::Pattern::as_str).collect::<Vec<_>>(),
			"command": rule.run,
			"runIfBaseMissing": rule.run_if_base_missing,
		})),
		Entry::Rule(_) => None,
		Entry::Group(group) if kind == RulesKind::All || kind == RulesKind::Group => Some(json!({
			"type": "group",
			"name": group.name,
			"jobs": group.jobs,
			"rules": group.rules.iter().map(|rule| json!({
				"name": rule.name,
				"kind": if rule.install { "install" } else { "run" },
				"changed": rule.changed.iter().map(config::Pattern::as_str).collect::<Vec<_>>(),
				"exclude": rule.exclude.iter().map(config::Pattern::as_str).collect::<Vec<_>>(),
				"command": rule.run,
				"runIfBaseMissing": rule.run_if_base_missing,
			})).collect::<Vec<_>>(),
		})),
		Entry::Group(group) => {
			let rules = group
				.rules
				.iter()
				.filter(|rule| rules_kind_matches_rule(kind, rule))
				.map(|rule| {
					json!({
						"name": rule.name,
						"kind": if rule.install { "install" } else { "run" },
						"changed": rule.changed.iter().map(config::Pattern::as_str).collect::<Vec<_>>(),
						"exclude": rule.exclude.iter().map(config::Pattern::as_str).collect::<Vec<_>>(),
						"command": rule.run,
						"runIfBaseMissing": rule.run_if_base_missing,
					})
				})
				.collect::<Vec<_>>();
			if rules.is_empty() {
				None
			} else {
				Some(json!({
					"type": "group",
					"name": group.name,
					"jobs": group.jobs,
					"rules": rules,
				}))
			}
		}
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

fn count_config_rules_for_kind(config: &Config, kind: RulesKind) -> usize {
	config
		.entries
		.iter()
		.map(|entry| match entry {
			Entry::Rule(rule) => usize::from(rules_kind_matches_rule(kind, rule)),
			Entry::Group(group) => group
				.rules
				.iter()
				.filter(|rule| rules_kind_matches_rule(kind, rule))
				.count(),
		})
		.sum()
}

fn collect_rule_selectors_for_kind(config: &Config, kind: RulesKind) -> Vec<String> {
	let mut selectors = BTreeSet::new();
	for entry in &config.entries {
		match entry {
			Entry::Rule(rule) => {
				if rules_kind_matches_rule(kind, rule) {
					selectors.insert(rule.name.clone());
				}
			}
			Entry::Group(group) => {
				if kind == RulesKind::All || kind == RulesKind::Group {
					selectors.insert(group.name.clone());
				}
				for rule in &group.rules {
					if rules_kind_matches_rule(kind, rule) {
						selectors.insert(rule.name.clone());
					}
				}
			}
		}
	}
	selectors.into_iter().collect()
}

fn collect_config_rule_commands_for_kind(config: &Config, kind: RulesKind) -> Vec<&str> {
	let mut commands = Vec::new();
	for entry in &config.entries {
		match entry {
			Entry::Rule(rule) => collect_config_rule_command_for_kind(rule, kind, &mut commands),
			Entry::Group(group) => {
				for rule in &group.rules {
					collect_config_rule_command_for_kind(rule, kind, &mut commands);
				}
			}
		}
	}
	commands
}

fn collect_config_rule_patterns_for_kind(config: &Config, kind: RulesKind) -> Vec<&str> {
	let mut patterns = Vec::new();
	for entry in &config.entries {
		match entry {
			Entry::Rule(rule) => collect_config_rule_patterns_for_rule_kind(rule, kind, &mut patterns),
			Entry::Group(group) => {
				for rule in &group.rules {
					collect_config_rule_patterns_for_rule_kind(rule, kind, &mut patterns);
				}
			}
		}
	}
	patterns
}

fn collect_config_rule_patterns_for_rule_kind<'a>(
	rule: &'a config::Rule,
	kind: RulesKind,
	patterns: &mut Vec<&'a str>,
) {
	if rules_kind_matches_rule(kind, rule) {
		patterns.extend(rule.changed.iter().map(config::Pattern::as_str));
	}
}

fn collect_config_rule_command_for_kind<'a>(rule: &'a config::Rule, kind: RulesKind, commands: &mut Vec<&'a str>) {
	if rules_kind_matches_rule(kind, rule)
		&& let Some(command) = rule.run.as_deref()
	{
		commands.push(command);
	}
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

fn render_config_group_inventory(group: &config::Group, kind: RulesKind) {
	if kind == RulesKind::Group {
		println!("[group] {}", group.name);
		match group.jobs {
			Some(jobs) => println!("jobs: {jobs}"),
			None => println!("jobs: default"),
		}
		println!();
		return;
	}

	let rules = group
		.rules
		.iter()
		.filter(|rule| rules_kind_matches_rule(kind, rule))
		.collect::<Vec<_>>();
	if rules.is_empty() {
		return;
	}

	println!("[group] {}", group.name);
	match group.jobs {
		Some(jobs) => println!("jobs: {jobs}"),
		None => println!("jobs: default"),
	}
	for rule in rules {
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

fn count_config_groups_for_kind(config: &Config, kind: RulesKind) -> usize {
	if kind != RulesKind::All && kind != RulesKind::Group {
		return 0;
	}
	count_config_groups(config)
}

const fn rules_kind_matches_rule(kind: RulesKind, rule: &config::Rule) -> bool {
	match kind {
		RulesKind::All | RulesKind::Rule => true,
		RulesKind::Group => false,
		RulesKind::Run => !rule.install,
		RulesKind::Install => rule.install,
	}
}

const fn rules_kind_label(kind: RulesKind) -> &'static str {
	match kind {
		RulesKind::All => "all",
		RulesKind::Rule => "rule",
		RulesKind::Group => "group",
		RulesKind::Run => "run",
		RulesKind::Install => "install",
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

fn collect_planned_commands(evaluation: &[EvaluatedEntry]) -> Vec<&str> {
	let mut commands = Vec::new();

	for entry in evaluation {
		match entry {
			EvaluatedEntry::Rule(rule) => collect_rule_command(rule, &mut commands),
			EvaluatedEntry::Group(group) => {
				for rule in &group.rules {
					collect_rule_command(rule, &mut commands);
				}
			}
		}
	}

	commands
}

fn collect_rule_command<'a>(rule: &'a EvaluatedRule, commands: &mut Vec<&'a str>) {
	if rule.should_run()
		&& let Some(command) = &rule.command
	{
		commands.push(command);
	}
}

fn collect_matched_rules(evaluation: &[EvaluatedEntry]) -> Vec<&str> {
	let mut rule_names = Vec::new();

	for entry in evaluation {
		match entry {
			EvaluatedEntry::Rule(rule) => collect_matched_rule(rule, &mut rule_names),
			EvaluatedEntry::Group(group) => {
				for rule in &group.rules {
					collect_matched_rule(rule, &mut rule_names);
				}
			}
		}
	}

	rule_names
}

fn collect_matched_rule<'a>(rule: &'a EvaluatedRule, rule_names: &mut Vec<&'a str>) {
	if rule.should_run() {
		rule_names.push(rule.rule.name.as_str());
	}
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
				let state = execute_config_rule(
					renderer,
					rule,
					repo_root,
					args.debug,
					effective_render_mode(args.render, args.no_color),
					args.quiet,
				);
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
				executions.push(rule_execution_json(
					rule,
					None,
					&result,
					repo_root,
					effective_render_mode(args.render, args.no_color),
				));
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
				executions.push(group_execution_json(
					group,
					&results,
					repo_root,
					effective_render_mode(args.render, args.no_color),
				));
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
	quiet: bool,
) -> runner::ResultState {
	let result = run_config_rule_task(rule, repo_root, debug_enabled);
	render_config_rule_result(renderer, rule, &result, repo_root, debug_enabled, render_mode, quiet);
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
		render_config_rule_result(
			renderer,
			rule,
			result,
			repo_root,
			args.debug,
			effective_render_mode(args.render, args.no_color),
			args.quiet,
		);
		counts.add_state(result.state);
	}

	if counts.has_non_success() {
		render_group_fail_text(group, effective_render_mode(args.render, args.no_color));
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
	quiet: bool,
) {
	let failed = result.state != runner::ResultState::Success;
	if quiet && !failed {
		return;
	}

	render_task_results(renderer, std::slice::from_ref(result), repo_root);
	report_debug_errors(debug_enabled, std::slice::from_ref(result));
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
