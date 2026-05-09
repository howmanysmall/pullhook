//! CLI parsing and argument helpers.

use std::num::NonZeroUsize;
use std::path::PathBuf;

use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{Shell, generate};

use crate::config::ConfigFormat;
use crate::output::RenderMode;

const ROOT_AFTER_HELP: &str = "\
Examples:
  pullhook --pattern \"**/*.rs\" --command \"cargo test\"
  pullhook --install --dry-run
  pullhook init --format json
  pullhook codes --json
  pullhook run --dry-run

Next steps:
  Use `pullhook init` to create a repo config.
  Use `pullhook explain --all-matches` to preview config rule matches.
  Use `pullhook codes` to inspect stable JSON status codes.";

const LEGACY_RUN_AFTER_HELP: &str = "\
Examples:
  pullhook --pattern \"packages/*/package-lock.json\" --command \"npm install\"
  pullhook --pattern \"**/*.rs\" --command \"cargo test\" --once
  pullhook --pattern \"**/*.json\" --command \"prettier --write\" --dry-run --json";

const CONFIG_RUN_AFTER_HELP: &str = "\
Examples:
  pullhook run
  pullhook run --dry-run
  pullhook run --json
  pullhook run --quiet
  pullhook run --summary-only
  pullhook run --commands-only
  pullhook run --changed-files-only
  pullhook run --matched-files-only
  pullhook run --matched-rules-only
  pullhook run --require-match --dry-run
  pullhook run --changed-file packages/a/package-lock.json --dry-run
  pullhook run --changed-files-file .pullhook-changed --dry-run
  git diff --name-only HEAD~1 | pullhook run --changed-files-file - --dry-run
  git diff --name-only HEAD~1 | pullhook run --changed-files-stdin --dry-run
  pullhook run --rule lint --rule typecheck
  pullhook run --config config/pullhook.custom.json --all-matches";

const EXPLAIN_AFTER_HELP: &str = "\
Examples:
  pullhook explain
  pullhook explain --all-matches
  pullhook explain --changed-file packages/a/package-lock.json
  pullhook explain --changed-files-file .pullhook-changed
  git diff --name-only HEAD~1 | pullhook explain --changed-files-file -
  git diff --name-only HEAD~1 | pullhook explain --changed-files-stdin
  pullhook explain --rule lint --all-matches
  pullhook explain --summary-only
  pullhook explain --commands-only
  pullhook explain --changed-files-only
  pullhook explain --matched-files-only
  pullhook explain --matched-rules-only
  pullhook explain --require-match
  pullhook explain --json";

const VALIDATE_AFTER_HELP: &str = "\
Examples:
  pullhook validate
  pullhook validate --quiet
  pullhook validate --json
  pullhook validate --config config/pullhook.custom.json";

const DOCTOR_AFTER_HELP: &str = "\
Examples:
  pullhook doctor
  pullhook doctor --quiet
  pullhook doctor --strict
  pullhook doctor --json
  pullhook doctor --config config/pullhook.custom.json";

const CONFIG_AFTER_HELP: &str = "\
Examples:
  pullhook config
  pullhook config --path-only
  pullhook config --require-existing --path-only
  pullhook config --json
  pullhook config --config config/pullhook.custom.json";

const RULES_AFTER_HELP: &str = "\
Examples:
  pullhook rules
  pullhook rules --kind install
  pullhook rules --names-only
  pullhook rules --commands-only
  pullhook rules --rule lint --commands-only
  pullhook rules --patterns-only
  pullhook rules --rule lint --patterns-only
  pullhook rules --json
  pullhook rules --rule lint --json
  pullhook rules --config config/pullhook.custom.json";

const SCHEMA_AFTER_HELP: &str = "\
Examples:
  pullhook schema
  pullhook schema --output .vscode/pullhook.schema.json
  pullhook schema --check --output .vscode/pullhook.schema.json
  pullhook schema --check --output .vscode/pullhook.schema.json --json";

const INIT_AFTER_HELP: &str = "\
Examples:
  pullhook init
  pullhook init --format yaml
  pullhook init --output config/pullhook.custom.json
  pullhook init --dry-run --json
  pullhook init --stdout
  pullhook init --force";

const COMPLETION_AFTER_HELP: &str = "\
Examples:
  pullhook completion bash > ~/.local/share/bash-completion/completions/pullhook
  pullhook completion zsh > ~/.zfunc/_pullhook
  pullhook completion fish --output ~/.config/fish/completions/pullhook.fish
  pullhook completion fish --check --output ~/.config/fish/completions/pullhook.fish
  pullhook completion fish --check --output ~/.config/fish/completions/pullhook.fish --json";

const CODES_AFTER_HELP: &str = "\
Examples:
  pullhook codes
  pullhook codes --kind doctor-check
  pullhook codes --json";

/// Pullhook command line arguments.
#[derive(Debug, Clone, Parser)]
#[command(name = "pullhook")]
#[command(about = "Run commands when files change after git pull")]
#[command(version)]
#[command(args_conflicts_with_subcommands = true)]
#[command(subcommand_negates_reqs = true)]
#[command(propagate_version = true)]
#[command(after_help = ROOT_AFTER_HELP)]
pub struct Cli {
	#[command(flatten)]
	pub run: RunArgs,

	#[command(subcommand)]
	pub command: Option<Commands>,
}

/// Non-run command variants.
#[derive(Debug, Clone, Subcommand)]
pub enum Commands {
	/// Run configured pullhook rules.
	Run(ConfigRunArgs),
	/// Explain which configured rules match changed files.
	Explain(ExplainArgs),
	/// Validate the pullhook config file.
	Validate(ValidateArgs),
	/// Inspect repository and config readiness.
	Doctor(DoctorArgs),
	/// Show the resolved pullhook config path.
	Config(ConfigArgs),
	/// List configured rule and group names.
	Rules(RulesArgs),
	/// Print or write the pullhook JSON Schema.
	Schema(SchemaArgs),
	/// Create a starter pullhook config file.
	Init(InitArgs),
	/// Generate shell completion scripts.
	Completion(CompletionArgs),
	/// List stable JSON status codes for automation.
	Codes(CodesArgs),
}

/// Arguments for the default pullhook execution flow.
#[derive(Debug, Clone, Args)]
#[expect(
	clippy::struct_excessive_bools,
	reason = "CLI flags are naturally represented as independent booleans"
)]
#[command(after_help = LEGACY_RUN_AFTER_HELP)]
#[command(next_help_heading = "Legacy one-off options")]
pub struct RunArgs {
	/// Pattern to match files.
	#[arg(short = 'p', long = "pattern", value_name = "glob")]
	#[arg(conflicts_with = "install")]
	pub pattern: Option<String>,

	/// Execute command for each matched file.
	#[arg(short = 'c', long = "command", value_name = "command")]
	#[arg(conflicts_with = "install")]
	pub command: Option<String>,

	/// Execute npm script for each matched file.
	#[arg(short = 's', long = "script", value_name = "script")]
	pub script: Option<String>,

	/// Detect package manager and run install.
	#[arg(short = 'i', long = "install")]
	#[arg(conflicts_with_all = ["pattern", "command"])]
	pub install: bool,

	/// Print message if any matches are found.
	#[arg(short = 'm', long = "message", value_name = "message")]
	pub message: Option<String>,

	/// Enable debug logging.
	#[arg(short = 'd', long = "debug", default_value_t = false)]
	pub debug: bool,

	/// Control non-debug ANSI styling (`auto`, `always`, `never`).
	#[arg(long = "render", value_name = "mode", value_enum, default_value_t = RenderMode::Auto)]
	pub render: RenderMode,

	/// Disable ANSI styling in non-debug output.
	#[arg(long = "no-color", default_value_t = false, conflicts_with = "render")]
	pub no_color: bool,

	/// Run command once in repo root if any match.
	#[arg(short = 'o', long = "once", default_value_t = false)]
	pub once: bool,

	/// Override the git base revision.
	#[arg(long = "base", value_name = "rev")]
	pub base: Option<String>,

	/// Max concurrent jobs.
	#[arg(long = "jobs", value_name = "n")]
	pub jobs: Option<NonZeroUsize>,

	/// Run --command via a shell.
	#[arg(long = "shell", default_value_t = false)]
	pub shell: bool,

	/// Print planned commands and exit.
	#[arg(long = "dry-run", default_value_t = false)]
	pub dry_run: bool,

	/// Print machine-readable JSON instead of text output.
	#[arg(long = "json", default_value_t = false)]
	pub json: bool,

	/// Dedupe directories before per-match execution.
	#[arg(long = "unique-cwd", default_value_t = false)]
	pub unique_cwd: bool,
}

/// Arguments for `pullhook run`.
#[derive(Debug, Clone, Args)]
#[expect(
	clippy::struct_excessive_bools,
	reason = "CLI flags are naturally represented as independent booleans"
)]
#[command(after_help = CONFIG_RUN_AFTER_HELP)]
pub struct ConfigRunArgs {
	/// Load config from an explicit path instead of repo-root discovery.
	#[arg(long = "config", value_name = "path")]
	pub config: Option<PathBuf>,

	/// Override the git base revision.
	#[arg(long = "base", value_name = "rev")]
	pub base: Option<String>,

	/// Evaluate as if this file changed; repeat for multiple files.
	#[arg(long = "changed-file", value_name = "path", conflicts_with = "base")]
	pub changed_files: Vec<PathBuf>,

	/// Read changed file paths from a newline-delimited file (`-` for stdin).
	#[arg(long = "changed-files-file", value_name = "path", conflicts_with = "base")]
	pub changed_files_file: Option<PathBuf>,

	/// Read changed file paths from stdin, one path per line.
	#[arg(long = "changed-files-stdin", default_value_t = false, conflicts_with = "base")]
	pub changed_files_stdin: bool,

	/// Max concurrent jobs for top-level work.
	#[arg(long = "jobs", value_name = "n")]
	pub jobs: Option<NonZeroUsize>,

	/// Print planned commands and exit.
	#[arg(long = "dry-run", default_value_t = false)]
	pub dry_run: bool,

	/// Print machine-readable JSON instead of text output.
	#[arg(long = "json", default_value_t = false)]
	pub json: bool,

	/// Suppress successful text output while still reporting failures.
	#[arg(long = "quiet", default_value_t = false, conflicts_with_all = ["json", "dry_run"])]
	pub quiet: bool,

	/// Print only changed-file and planned-command counts and exit without executing.
	#[arg(
		long = "summary-only",
		default_value_t = false,
		conflicts_with_all = [
			"json",
			"quiet",
			"commands_only",
			"changed_files_only",
			"matched_files_only",
			"matched_rules_only"
		]
	)]
	pub summary_only: bool,

	/// Print only planned commands and exit without executing.
	#[arg(
		long = "commands-only",
		default_value_t = false,
		conflicts_with_all = ["json", "quiet", "changed_files_only", "matched_files_only", "matched_rules_only"]
	)]
	pub commands_only: bool,

	/// Print only resolved changed files and exit without executing.
	#[arg(
		long = "changed-files-only",
		default_value_t = false,
		conflicts_with_all = ["json", "quiet", "summary_only", "commands_only", "matched_files_only", "matched_rules_only"]
	)]
	pub changed_files_only: bool,

	/// Print only matched changed files and exit without executing.
	#[arg(
		long = "matched-files-only",
		default_value_t = false,
		conflicts_with_all = ["json", "quiet", "summary_only", "commands_only", "changed_files_only", "matched_rules_only"]
	)]
	pub matched_files_only: bool,

	/// Print only matched rule names and exit without executing.
	#[arg(
		long = "matched-rules-only",
		default_value_t = false,
		conflicts_with_all = ["json", "quiet", "summary_only", "commands_only", "changed_files_only", "matched_files_only"]
	)]
	pub matched_rules_only: bool,

	/// Show skipped rules as well as matched rules.
	#[arg(long = "all-matches", default_value_t = false)]
	pub all_matches: bool,

	/// Exit non-zero when no rules match changed files.
	#[arg(long = "require-match", default_value_t = false)]
	pub require_match: bool,

	/// Limit execution to one or more named rules or parallel groups.
	#[arg(long = "rule", value_name = "name")]
	pub rules: Vec<String>,

	/// Enable debug logging.
	#[arg(short = 'd', long = "debug", default_value_t = false)]
	pub debug: bool,

	/// Control non-debug ANSI styling (`auto`, `always`, `never`).
	#[arg(long = "render", value_name = "mode", value_enum, default_value_t = RenderMode::Auto)]
	pub render: RenderMode,

	/// Disable ANSI styling in non-debug output.
	#[arg(long = "no-color", default_value_t = false, conflicts_with = "render")]
	pub no_color: bool,
}

/// Arguments for `pullhook explain`.
#[expect(
	clippy::struct_excessive_bools,
	reason = "CLI flags are naturally represented as independent booleans"
)]
#[derive(Debug, Clone, Args)]
#[command(after_help = EXPLAIN_AFTER_HELP)]
pub struct ExplainArgs {
	/// Load config from an explicit path instead of repo-root discovery.
	#[arg(long = "config", value_name = "path")]
	pub config: Option<PathBuf>,

	/// Override the git base revision.
	#[arg(long = "base", value_name = "rev")]
	pub base: Option<String>,

	/// Evaluate as if this file changed; repeat for multiple files.
	#[arg(long = "changed-file", value_name = "path", conflicts_with = "base")]
	pub changed_files: Vec<PathBuf>,

	/// Read changed file paths from a newline-delimited file (`-` for stdin).
	#[arg(long = "changed-files-file", value_name = "path", conflicts_with = "base")]
	pub changed_files_file: Option<PathBuf>,

	/// Read changed file paths from stdin, one path per line.
	#[arg(long = "changed-files-stdin", default_value_t = false, conflicts_with = "base")]
	pub changed_files_stdin: bool,

	/// Show skipped rules as well as matched rules.
	#[arg(long = "all-matches", default_value_t = false)]
	pub all_matches: bool,

	/// Exit non-zero when no rules match changed files.
	#[arg(long = "require-match", default_value_t = false)]
	pub require_match: bool,

	/// Limit output to one or more named rules or parallel groups.
	#[arg(long = "rule", value_name = "name")]
	pub rules: Vec<String>,

	/// Print only changed-file and planned-command counts.
	#[arg(
		long = "summary-only",
		default_value_t = false,
		conflicts_with_all = ["json", "commands_only", "changed_files_only", "matched_files_only", "matched_rules_only"]
	)]
	pub summary_only: bool,

	/// Print only planned commands, one per line.
	#[arg(
		long = "commands-only",
		default_value_t = false,
		conflicts_with_all = ["json", "summary_only", "changed_files_only", "matched_files_only", "matched_rules_only"]
	)]
	pub commands_only: bool,

	/// Print only resolved changed files, one per line.
	#[arg(
		long = "changed-files-only",
		default_value_t = false,
		conflicts_with_all = ["json", "summary_only", "commands_only", "matched_files_only", "matched_rules_only"]
	)]
	pub changed_files_only: bool,

	/// Print only matched changed files, one per line.
	#[arg(
		long = "matched-files-only",
		default_value_t = false,
		conflicts_with_all = ["json", "summary_only", "commands_only", "changed_files_only", "matched_rules_only"]
	)]
	pub matched_files_only: bool,

	/// Print only matched rule names, one per line.
	#[arg(
		long = "matched-rules-only",
		default_value_t = false,
		conflicts_with_all = ["json", "summary_only", "commands_only", "changed_files_only", "matched_files_only"]
	)]
	pub matched_rules_only: bool,

	/// Print machine-readable JSON instead of text output.
	#[arg(long = "json", default_value_t = false)]
	pub json: bool,

	/// Enable debug logging.
	#[arg(short = 'd', long = "debug", default_value_t = false)]
	pub debug: bool,

	/// Control non-debug ANSI styling (`auto`, `always`, `never`).
	#[arg(long = "render", value_name = "mode", value_enum, default_value_t = RenderMode::Auto)]
	pub render: RenderMode,

	/// Disable ANSI styling in non-debug output.
	#[arg(long = "no-color", default_value_t = false, conflicts_with = "render")]
	pub no_color: bool,
}

/// Arguments for `pullhook validate`.
#[derive(Debug, Clone, Args)]
#[expect(
	clippy::struct_excessive_bools,
	reason = "CLI flags are naturally represented as independent booleans"
)]
#[command(after_help = VALIDATE_AFTER_HELP)]
pub struct ValidateArgs {
	/// Load config from an explicit path instead of repo-root discovery.
	#[arg(long = "config", value_name = "path")]
	pub config: Option<PathBuf>,

	/// Print machine-readable JSON instead of text output.
	#[arg(long = "json", default_value_t = false)]
	pub json: bool,

	/// Suppress successful text output.
	#[arg(long = "quiet", default_value_t = false, conflicts_with = "json")]
	pub quiet: bool,

	/// Enable debug logging.
	#[arg(short = 'd', long = "debug", default_value_t = false)]
	pub debug: bool,

	/// Control non-debug ANSI styling (`auto`, `always`, `never`).
	#[arg(long = "render", value_name = "mode", value_enum, default_value_t = RenderMode::Auto)]
	pub render: RenderMode,

	/// Disable ANSI styling in non-debug output.
	#[arg(long = "no-color", default_value_t = false, conflicts_with = "render")]
	pub no_color: bool,
}

/// Arguments for `pullhook doctor`.
#[derive(Debug, Clone, Args)]
#[expect(
	clippy::struct_excessive_bools,
	reason = "CLI flags are naturally represented as independent booleans"
)]
#[command(after_help = DOCTOR_AFTER_HELP)]
pub struct DoctorArgs {
	/// Load config from an explicit path instead of repo-root discovery.
	#[arg(long = "config", value_name = "path")]
	pub config: Option<PathBuf>,

	/// Print machine-readable JSON instead of text output.
	#[arg(long = "json", default_value_t = false)]
	pub json: bool,

	/// Suppress text output when all checks pass.
	#[arg(long = "quiet", default_value_t = false, conflicts_with = "json")]
	pub quiet: bool,

	/// Exit non-zero on warnings as well as errors.
	#[arg(long = "strict", default_value_t = false)]
	pub strict: bool,

	/// Enable debug logging.
	#[arg(short = 'd', long = "debug", default_value_t = false)]
	pub debug: bool,

	/// Control non-debug ANSI styling (`auto`, `always`, `never`).
	#[arg(long = "render", value_name = "mode", value_enum, default_value_t = RenderMode::Auto)]
	pub render: RenderMode,

	/// Disable ANSI styling in non-debug output.
	#[arg(long = "no-color", default_value_t = false, conflicts_with = "render")]
	pub no_color: bool,
}

/// Arguments for `pullhook config`.
#[derive(Debug, Clone, Args)]
#[expect(
	clippy::struct_excessive_bools,
	reason = "CLI flags are naturally represented as independent booleans"
)]
#[command(after_help = CONFIG_AFTER_HELP)]
pub struct ConfigArgs {
	/// Load config from an explicit path instead of repo-root discovery.
	#[arg(long = "config", value_name = "path")]
	pub config: Option<PathBuf>,

	/// Print only the resolved config path.
	#[arg(long = "path-only", default_value_t = false, conflicts_with = "json")]
	pub path_only: bool,

	/// Print machine-readable JSON instead of text output.
	#[arg(long = "json", default_value_t = false)]
	pub json: bool,

	/// Exit non-zero if the resolved config file does not exist.
	#[arg(long = "require-existing", default_value_t = false)]
	pub require_existing: bool,

	/// Enable debug logging.
	#[arg(short = 'd', long = "debug", default_value_t = false)]
	pub debug: bool,

	/// Control non-debug ANSI styling (`auto`, `always`, `never`).
	#[arg(long = "render", value_name = "mode", value_enum, default_value_t = RenderMode::Auto)]
	pub render: RenderMode,

	/// Disable ANSI styling in non-debug output.
	#[arg(long = "no-color", default_value_t = false, conflicts_with = "render")]
	pub no_color: bool,
}

/// Arguments for `pullhook init`.
#[derive(Debug, Clone, Args)]
#[expect(
	clippy::struct_excessive_bools,
	reason = "CLI flags are naturally represented as independent booleans"
)]
#[command(after_help = INIT_AFTER_HELP)]
pub struct InitArgs {
	/// Config format to generate.
	#[arg(long = "format", value_name = "format", value_enum)]
	pub format: Option<InitFormat>,

	/// Print the starter config to stdout instead of writing a file.
	#[arg(long = "stdout", default_value_t = false, conflicts_with_all = ["dry_run", "json"])]
	pub stdout: bool,

	/// Write the starter config to an explicit path.
	#[arg(long = "output", value_name = "path", conflicts_with = "stdout")]
	pub output: Option<PathBuf>,

	/// Print the init plan without writing a config file.
	#[arg(long = "dry-run", default_value_t = false)]
	pub dry_run: bool,

	/// Print machine-readable JSON instead of text output.
	#[arg(long = "json", default_value_t = false)]
	pub json: bool,

	/// Overwrite an existing pullhook config file in place.
	#[arg(long = "force", default_value_t = false, conflicts_with = "stdout")]
	pub force: bool,

	/// Enable debug logging.
	#[arg(short = 'd', long = "debug", default_value_t = false)]
	pub debug: bool,

	/// Control non-debug ANSI styling (`auto`, `always`, `never`).
	#[arg(long = "render", value_name = "mode", value_enum, default_value_t = RenderMode::Auto)]
	pub render: RenderMode,

	/// Disable ANSI styling in non-debug output.
	#[arg(long = "no-color", default_value_t = false, conflicts_with = "render")]
	pub no_color: bool,
}

/// Arguments for `pullhook rules`.
#[derive(Debug, Clone, Args)]
#[expect(
	clippy::struct_excessive_bools,
	reason = "CLI flags are naturally represented as independent booleans"
)]
#[command(after_help = RULES_AFTER_HELP)]
pub struct RulesArgs {
	/// Load config from an explicit path instead of repo-root discovery.
	#[arg(long = "config", value_name = "path")]
	pub config: Option<PathBuf>,

	/// Print machine-readable JSON instead of text output.
	#[arg(long = "json", default_value_t = false)]
	pub json: bool,

	/// Print only rule and group selector names, one per line.
	#[arg(long = "names-only", default_value_t = false, conflicts_with_all = ["json", "commands_only", "patterns_only"])]
	pub names_only: bool,

	/// Print only configured run commands, one per line.
	#[arg(long = "commands-only", default_value_t = false, conflicts_with_all = ["json", "names_only", "patterns_only"])]
	pub commands_only: bool,

	/// Print only configured changed-file patterns, one per line.
	#[arg(long = "patterns-only", default_value_t = false, conflicts_with_all = ["json", "names_only", "commands_only"])]
	pub patterns_only: bool,

	/// Limit inventory to all entries, leaf rules, groups, run rules, or install rules.
	#[arg(long = "kind", value_name = "kind", value_enum, default_value_t = RulesKind::All)]
	pub kind: RulesKind,

	/// Limit inventory to one or more named rules or parallel groups.
	#[arg(long = "rule", value_name = "name")]
	pub rules: Vec<String>,

	/// Enable debug logging.
	#[arg(short = 'd', long = "debug", default_value_t = false)]
	pub debug: bool,

	/// Control non-debug ANSI styling (`auto`, `always`, `never`).
	#[arg(long = "render", value_name = "mode", value_enum, default_value_t = RenderMode::Auto)]
	pub render: RenderMode,

	/// Disable ANSI styling in non-debug output.
	#[arg(long = "no-color", default_value_t = false, conflicts_with = "render")]
	pub no_color: bool,
}

/// Rule inventory filter for `pullhook rules`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RulesKind {
	/// Show groups and leaf rules.
	All,
	/// Show only leaf rules.
	Rule,
	/// Show only parallel group selectors.
	Group,
	/// Show only leaf rules that run commands.
	Run,
	/// Show only leaf rules that run package-manager install.
	Install,
}

/// Arguments for `pullhook schema`.
#[derive(Debug, Clone, Args)]
#[command(after_help = SCHEMA_AFTER_HELP)]
pub struct SchemaArgs {
	/// Write the schema to a file instead of stdout.
	#[arg(long = "output", value_name = "path")]
	pub output: Option<PathBuf>,

	/// Check that the output file already matches the embedded schema.
	#[arg(long = "check", default_value_t = false, requires = "output")]
	pub check: bool,

	/// Print machine-readable check results instead of text output.
	#[arg(long = "json", default_value_t = false, requires = "check")]
	pub json: bool,
}

/// Arguments for `pullhook completion`.
#[derive(Debug, Clone, Args)]
#[command(after_help = COMPLETION_AFTER_HELP)]
pub struct CompletionArgs {
	/// Shell to generate completions for.
	pub shell: Shell,

	/// Write completions to a file instead of stdout.
	#[arg(long = "output", value_name = "path")]
	pub output: Option<PathBuf>,

	/// Check that the output file already matches the generated completions.
	#[arg(long = "check", default_value_t = false, requires = "output")]
	pub check: bool,

	/// Print machine-readable check results instead of text output.
	#[arg(long = "json", default_value_t = false, requires = "check")]
	pub json: bool,
}

/// Arguments for `pullhook codes`.
#[derive(Debug, Clone, Args)]
#[command(after_help = CODES_AFTER_HELP)]
pub struct CodesArgs {
	/// Only list codes for a specific kind.
	#[arg(long = "kind", value_enum)]
	pub kind: Option<CodeKind>,

	/// Print machine-readable JSON instead of text output.
	#[arg(long = "json", default_value_t = false)]
	pub json: bool,
}

/// Kinds of stable JSON status codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CodeKind {
	/// Top-level error response code.
	Error,
	/// Per-check doctor status code.
	DoctorCheck,
}

impl CodeKind {
	pub const fn label(self) -> &'static str {
		match self {
			Self::Error => "error",
			Self::DoctorCheck => "doctor-check",
		}
	}
}

/// Supported starter config formats for `pullhook init`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum InitFormat {
	/// JSON (`pullhook.json`).
	Json,
	/// JSON with comments (`pullhook.jsonc`).
	Jsonc,
	/// YAML (`pullhook.yaml`).
	Yaml,
	/// TOML (`pullhook.toml`).
	Toml,
}

impl From<InitFormat> for ConfigFormat {
	fn from(value: InitFormat) -> Self {
		match value {
			InitFormat::Json => Self::Json,
			InitFormat::Jsonc => Self::Jsonc,
			InitFormat::Yaml => Self::Yaml,
			InitFormat::Toml => Self::Toml,
		}
	}
}

impl Cli {
	/// Write shell completions to the provided writer.
	pub fn write_completion<W: std::io::Write>(shell: Shell, writer: &mut W) {
		generate(shell, &mut Self::command(), "pullhook", writer);
	}

	/// Render shell completions to a string.
	pub fn completion_string(shell: Shell) -> String {
		let mut output = Vec::new();
		Self::write_completion(shell, &mut output);
		String::from_utf8_lossy(&output).into_owned()
	}
}

impl RunArgs {
	/// Compute the effective `--once` mode.
	#[must_use]
	pub const fn effective_once(&self) -> bool {
		self.once || self.install
	}

	/// Compute the effective jobs value.
	#[must_use]
	pub fn effective_jobs(&self) -> usize {
		self.jobs.map_or_else(default_jobs, NonZeroUsize::get)
	}
}

impl ConfigRunArgs {
	/// Compute the effective jobs value.
	#[must_use]
	pub fn effective_jobs(&self) -> usize {
		self.jobs.map_or_else(default_jobs, NonZeroUsize::get)
	}
}

fn default_jobs() -> usize {
	std::thread::available_parallelism().map_or(1, NonZeroUsize::get).min(8)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn clap_configuration_is_valid() {
		Cli::command().debug_assert();
	}

	#[test]
	fn completion_subcommand_skips_run_requirements() {
		let cli = Cli::try_parse_from(["pullhook", "completion", "bash"]).expect("completion parses");

		assert!(matches!(cli.command, Some(Commands::Completion(args)) if args.shell == Shell::Bash));
		assert!(cli.run.pattern.is_none());
	}

	#[test]
	fn run_subcommand_parses_without_legacy_pattern() {
		let cli = Cli::try_parse_from(["pullhook", "run", "--dry-run"]).expect("run parses");

		assert!(matches!(cli.command, Some(Commands::Run(args)) if args.dry_run));
	}

	#[test]
	fn run_args_conflict_with_completion_subcommand() {
		let error = Cli::try_parse_from(["pullhook", "--install", "completion", "bash"]).expect_err("mixed args fail");

		assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
	}
}
