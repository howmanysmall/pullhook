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
  pullhook examples
  pullhook shells
  pullhook formats
  pullhook managers
  pullhook categories
  pullhook commands --json
  pullhook codes --json
  pullhook run --dry-run

Next steps:
  Use `pullhook init` to create a repo config.
  Use `pullhook examples` to see common workflows.
  Use `pullhook shells` to list completion targets.
  Use `pullhook formats` to list supported config formats.
  Use `pullhook managers` to list package-manager install detection.
  Use `pullhook categories` to inspect command categories.
  Use `pullhook explain --all-matches` to preview config rule matches.
  Use `pullhook commands` to inspect the command catalog.
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
  pullhook validate --path-only
  pullhook validate --json
  pullhook validate --config config/pullhook.custom.json";

const DOCTOR_AFTER_HELP: &str = "\
Examples:
  pullhook doctor
  pullhook doctor --quiet
  pullhook doctor --strict
  pullhook doctor --checks-only
  pullhook doctor --codes-only
  pullhook doctor --json
  pullhook doctor --config config/pullhook.custom.json";

const CONFIG_AFTER_HELP: &str = "\
Examples:
  pullhook config
  pullhook config --path-only
  pullhook config --format-only
  pullhook config --exists-only
  pullhook config --source-only
  pullhook config --require-existing --path-only
  pullhook config --json
  pullhook config --config config/pullhook.custom.json";

const RULES_AFTER_HELP: &str = "\
Examples:
  pullhook rules
  pullhook rules --kind install
  pullhook rules --search lint
  pullhook rules --search lint --count-only
  pullhook rules --count-only
  pullhook rules --names-only
  pullhook rules --commands-only
  pullhook rules --rule lint --commands-only
  pullhook rules --patterns-only
  pullhook rules --rule lint --patterns-only
  pullhook rules --exclude-patterns-only
  pullhook rules --rule lint --exclude-patterns-only
  pullhook rules --fail-text-only
  pullhook rules --rule lint --fail-text-only
  pullhook rules --json
  pullhook rules --rule lint --json
  pullhook rules --config config/pullhook.custom.json";

const SCHEMA_AFTER_HELP: &str = "\
Examples:
  pullhook schema
  pullhook schema --output .vscode/pullhook.schema.json
  pullhook schema --check --output .vscode/pullhook.schema.json
  pullhook schema --check --quiet --output .vscode/pullhook.schema.json
  pullhook schema --check --output .vscode/pullhook.schema.json --json";

const INIT_AFTER_HELP: &str = "\
Examples:
  pullhook init
  pullhook init --format yaml
  pullhook init --output config/pullhook.custom.json
  pullhook init --dry-run --path-only
  pullhook init --dry-run --format-only
  pullhook init --dry-run --action-only
  pullhook init --dry-run --json
  pullhook init --stdout
  pullhook init --force";

const COMPLETION_AFTER_HELP: &str = "\
Examples:
  pullhook shells
  pullhook completion bash > ~/.local/share/bash-completion/completions/pullhook
  pullhook completion zsh > ~/.zfunc/_pullhook
  pullhook completion fish --output ~/.config/fish/completions/pullhook.fish
  pullhook completion fish --check --output ~/.config/fish/completions/pullhook.fish
  pullhook completion fish --check --quiet --output ~/.config/fish/completions/pullhook.fish
  pullhook completion fish --check --output ~/.config/fish/completions/pullhook.fish --json";

const SHELLS_AFTER_HELP: &str = "\
Examples:
  pullhook shells
  pullhook shells --search fish
  pullhook shells --search fish --count-only
  pullhook shells --names-only
  pullhook shells --commands-only
  pullhook shells --descriptions-only
  pullhook shells --json";

const FORMATS_AFTER_HELP: &str = "\
Examples:
  pullhook formats
  pullhook formats --search yaml
  pullhook formats --search yaml --count-only
  pullhook formats --names-only
  pullhook formats --files-only
  pullhook formats --init-commands-only
  pullhook formats --descriptions-only
  pullhook formats --json";

const MANAGERS_AFTER_HELP: &str = "\
Examples:
  pullhook managers
  pullhook managers --search pnpm
  pullhook managers --search pnpm --count-only
  pullhook managers --names-only
  pullhook managers --patterns-only
  pullhook managers --commands-only
  pullhook managers --lock-files-only
  pullhook managers --config-files-only
  pullhook managers --watched-files-only
  pullhook managers --json";

const CATEGORIES_AFTER_HELP: &str = "\
Examples:
  pullhook categories
  pullhook categories --search workflow
  pullhook categories --search workflow --count-only
  pullhook categories --names-only
  pullhook categories --commands-only
  pullhook categories --example-commands-only
  pullhook categories --descriptions-only
  pullhook categories --json";

const CODES_AFTER_HELP: &str = "\
Examples:
  pullhook codes
  pullhook codes --kind doctor-check
  pullhook codes --surface run
  pullhook codes --search config
  pullhook codes --search config --count-only
  pullhook codes --kinds-only
  pullhook codes --surfaces-only
  pullhook codes --search config --descriptions-only
  pullhook codes --kind error --codes-only
  pullhook codes --json";

const COMMANDS_AFTER_HELP: &str = "\
Examples:
  pullhook commands
  pullhook commands --category diagnostic
  pullhook commands --category diagnostic --names-only
  pullhook commands --search config
  pullhook commands --search config --count-only
  pullhook commands --repo-only
  pullhook commands --standalone-only --names-only
  pullhook commands --categories-only
  pullhook commands --category workflow --example-commands-only
  pullhook commands --search config --summaries-only
  pullhook commands --markdown
  pullhook commands --json";

const EXAMPLES_AFTER_HELP: &str = "\
Examples:
  pullhook examples
  pullhook examples --command run
  pullhook examples --category workflow
  pullhook examples --search install
  pullhook examples --search install --count-only
  pullhook examples --search install --summaries-only
  pullhook examples --command run --commands-only
  pullhook examples --command rules --commands-only
  pullhook examples --command schema --commands-only
  pullhook examples --command examples --commands-only
  pullhook examples --category reference --commands-only
  pullhook examples --category reference --titles-only
  pullhook examples --command-names-only
  pullhook examples --categories-only
  pullhook examples --json";

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
	/// List supported shell completion targets.
	Shells(ShellsArgs),
	/// List supported config formats and filenames.
	Formats(FormatsArgs),
	/// List supported package-manager install detection.
	Managers(ManagersArgs),
	/// List command categories and their coverage.
	Categories(CategoriesArgs),
	/// Show common pullhook workflows and commands.
	Examples(ExamplesArgs),
	/// List pullhook commands for humans or automation.
	#[command(name = "commands")]
	CommandCatalog(CommandCatalogArgs),
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
	#[arg(long = "config", value_name = "path", help_heading = "Input options")]
	pub config: Option<PathBuf>,

	/// Override the git base revision.
	#[arg(long = "base", value_name = "rev", help_heading = "Input options")]
	pub base: Option<String>,

	/// Evaluate as if this file changed; repeat for multiple files.
	#[arg(
		long = "changed-file",
		value_name = "path",
		conflicts_with = "base",
		help_heading = "Input options"
	)]
	pub changed_files: Vec<PathBuf>,

	/// Read changed file paths from a newline-delimited file (`-` for stdin).
	#[arg(
		long = "changed-files-file",
		value_name = "path",
		conflicts_with = "base",
		help_heading = "Input options"
	)]
	pub changed_files_file: Option<PathBuf>,

	/// Read changed file paths from stdin, one path per line.
	#[arg(
		long = "changed-files-stdin",
		default_value_t = false,
		conflicts_with = "base",
		help_heading = "Input options"
	)]
	pub changed_files_stdin: bool,

	/// Max concurrent jobs for top-level work.
	#[arg(long = "jobs", value_name = "n", help_heading = "Execution options")]
	pub jobs: Option<NonZeroUsize>,

	/// Print planned commands and exit.
	#[arg(long = "dry-run", default_value_t = false, help_heading = "Execution options")]
	pub dry_run: bool,

	/// Print machine-readable JSON instead of text output.
	#[arg(long = "json", default_value_t = false, help_heading = "Output options")]
	pub json: bool,

	/// Suppress successful text output while still reporting failures.
	#[arg(
		long = "quiet",
		default_value_t = false,
		conflicts_with_all = ["json", "dry_run"],
		help_heading = "Output options"
	)]
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
		],
		help_heading = "Output options"
	)]
	pub summary_only: bool,

	/// Print only planned commands and exit without executing.
	#[arg(
		long = "commands-only",
		default_value_t = false,
		conflicts_with_all = ["json", "quiet", "changed_files_only", "matched_files_only", "matched_rules_only"],
		help_heading = "Output options"
	)]
	pub commands_only: bool,

	/// Print only resolved changed files and exit without executing.
	#[arg(
		long = "changed-files-only",
		default_value_t = false,
		conflicts_with_all = ["json", "quiet", "summary_only", "commands_only", "matched_files_only", "matched_rules_only"],
		help_heading = "Output options"
	)]
	pub changed_files_only: bool,

	/// Print only matched changed files and exit without executing.
	#[arg(
		long = "matched-files-only",
		default_value_t = false,
		conflicts_with_all = ["json", "quiet", "summary_only", "commands_only", "changed_files_only", "matched_rules_only"],
		help_heading = "Output options"
	)]
	pub matched_files_only: bool,

	/// Print only matched rule names and exit without executing.
	#[arg(
		long = "matched-rules-only",
		default_value_t = false,
		conflicts_with_all = ["json", "quiet", "summary_only", "commands_only", "changed_files_only", "matched_files_only"],
		help_heading = "Output options"
	)]
	pub matched_rules_only: bool,

	/// Show skipped rules as well as matched rules.
	#[arg(long = "all-matches", default_value_t = false, help_heading = "Rule selection")]
	pub all_matches: bool,

	/// Exit non-zero when no rules match changed files.
	#[arg(long = "require-match", default_value_t = false, help_heading = "Rule selection")]
	pub require_match: bool,

	/// Limit execution to one or more named rules or parallel groups.
	#[arg(long = "rule", value_name = "name", help_heading = "Rule selection")]
	pub rules: Vec<String>,

	/// Enable debug logging.
	#[arg(
		short = 'd',
		long = "debug",
		default_value_t = false,
		help_heading = "Display options"
	)]
	pub debug: bool,

	/// Control non-debug ANSI styling (`auto`, `always`, `never`).
	#[arg(
		long = "render",
		value_name = "mode",
		value_enum,
		default_value_t = RenderMode::Auto,
		help_heading = "Display options"
	)]
	pub render: RenderMode,

	/// Disable ANSI styling in non-debug output.
	#[arg(
		long = "no-color",
		default_value_t = false,
		conflicts_with = "render",
		help_heading = "Display options"
	)]
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
	#[arg(long = "config", value_name = "path", help_heading = "Input options")]
	pub config: Option<PathBuf>,

	/// Override the git base revision.
	#[arg(long = "base", value_name = "rev", help_heading = "Input options")]
	pub base: Option<String>,

	/// Evaluate as if this file changed; repeat for multiple files.
	#[arg(
		long = "changed-file",
		value_name = "path",
		conflicts_with = "base",
		help_heading = "Input options"
	)]
	pub changed_files: Vec<PathBuf>,

	/// Read changed file paths from a newline-delimited file (`-` for stdin).
	#[arg(
		long = "changed-files-file",
		value_name = "path",
		conflicts_with = "base",
		help_heading = "Input options"
	)]
	pub changed_files_file: Option<PathBuf>,

	/// Read changed file paths from stdin, one path per line.
	#[arg(
		long = "changed-files-stdin",
		default_value_t = false,
		conflicts_with = "base",
		help_heading = "Input options"
	)]
	pub changed_files_stdin: bool,

	/// Show skipped rules as well as matched rules.
	#[arg(long = "all-matches", default_value_t = false, help_heading = "Rule selection")]
	pub all_matches: bool,

	/// Exit non-zero when no rules match changed files.
	#[arg(long = "require-match", default_value_t = false, help_heading = "Rule selection")]
	pub require_match: bool,

	/// Limit output to one or more named rules or parallel groups.
	#[arg(long = "rule", value_name = "name", help_heading = "Rule selection")]
	pub rules: Vec<String>,

	/// Print only changed-file and planned-command counts.
	#[arg(
		long = "summary-only",
		default_value_t = false,
		conflicts_with_all = ["json", "commands_only", "changed_files_only", "matched_files_only", "matched_rules_only"],
		help_heading = "Output options"
	)]
	pub summary_only: bool,

	/// Print only planned commands, one per line.
	#[arg(
		long = "commands-only",
		default_value_t = false,
		conflicts_with_all = ["json", "summary_only", "changed_files_only", "matched_files_only", "matched_rules_only"],
		help_heading = "Output options"
	)]
	pub commands_only: bool,

	/// Print only resolved changed files, one per line.
	#[arg(
		long = "changed-files-only",
		default_value_t = false,
		conflicts_with_all = ["json", "summary_only", "commands_only", "matched_files_only", "matched_rules_only"],
		help_heading = "Output options"
	)]
	pub changed_files_only: bool,

	/// Print only matched changed files, one per line.
	#[arg(
		long = "matched-files-only",
		default_value_t = false,
		conflicts_with_all = ["json", "summary_only", "commands_only", "changed_files_only", "matched_rules_only"],
		help_heading = "Output options"
	)]
	pub matched_files_only: bool,

	/// Print only matched rule names, one per line.
	#[arg(
		long = "matched-rules-only",
		default_value_t = false,
		conflicts_with_all = ["json", "summary_only", "commands_only", "changed_files_only", "matched_files_only"],
		help_heading = "Output options"
	)]
	pub matched_rules_only: bool,

	/// Print machine-readable JSON instead of text output.
	#[arg(long = "json", default_value_t = false, help_heading = "Output options")]
	pub json: bool,

	/// Enable debug logging.
	#[arg(
		short = 'd',
		long = "debug",
		default_value_t = false,
		help_heading = "Display options"
	)]
	pub debug: bool,

	/// Control non-debug ANSI styling (`auto`, `always`, `never`).
	#[arg(
		long = "render",
		value_name = "mode",
		value_enum,
		default_value_t = RenderMode::Auto,
		help_heading = "Display options"
	)]
	pub render: RenderMode,

	/// Disable ANSI styling in non-debug output.
	#[arg(
		long = "no-color",
		default_value_t = false,
		conflicts_with = "render",
		help_heading = "Display options"
	)]
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
	#[arg(long = "config", value_name = "path", help_heading = "Input options")]
	pub config: Option<PathBuf>,

	/// Print machine-readable JSON instead of text output.
	#[arg(
		long = "json",
		default_value_t = false,
		conflicts_with_all = ["quiet", "path_only"],
		help_heading = "Output options"
	)]
	pub json: bool,

	/// Suppress successful text output.
	#[arg(
		long = "quiet",
		default_value_t = false,
		conflicts_with_all = ["json", "path_only"],
		help_heading = "Output options"
	)]
	pub quiet: bool,

	/// Print only the validated config path.
	#[arg(
		long = "path-only",
		default_value_t = false,
		conflicts_with_all = ["json", "quiet"],
		help_heading = "Output options"
	)]
	pub path_only: bool,

	/// Enable debug logging.
	#[arg(
		short = 'd',
		long = "debug",
		default_value_t = false,
		help_heading = "Display options"
	)]
	pub debug: bool,

	/// Control non-debug ANSI styling (`auto`, `always`, `never`).
	#[arg(
		long = "render",
		value_name = "mode",
		value_enum,
		default_value_t = RenderMode::Auto,
		help_heading = "Display options"
	)]
	pub render: RenderMode,

	/// Disable ANSI styling in non-debug output.
	#[arg(
		long = "no-color",
		default_value_t = false,
		conflicts_with = "render",
		help_heading = "Display options"
	)]
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
	#[arg(long = "config", value_name = "path", help_heading = "Input options")]
	pub config: Option<PathBuf>,

	/// Print JSON with filters and searchFields metadata.
	#[arg(
		long = "json",
		default_value_t = false,
		conflicts_with_all = ["quiet", "checks_only", "codes_only"],
		help_heading = "Output options"
	)]
	pub json: bool,

	/// Suppress text output when all checks pass.
	#[arg(
		long = "quiet",
		default_value_t = false,
		conflicts_with_all = ["json", "checks_only", "codes_only"],
		help_heading = "Output options"
	)]
	pub quiet: bool,

	/// Print only doctor check names, one per line.
	#[arg(
		long = "checks-only",
		default_value_t = false,
		conflicts_with_all = ["json", "quiet", "codes_only"],
		help_heading = "Output options"
	)]
	pub checks_only: bool,

	/// Print only doctor check codes, one per line.
	#[arg(
		long = "codes-only",
		default_value_t = false,
		conflicts_with_all = ["json", "quiet", "checks_only"],
		help_heading = "Output options"
	)]
	pub codes_only: bool,

	/// Exit non-zero on warnings as well as errors.
	#[arg(long = "strict", default_value_t = false, help_heading = "Check options")]
	pub strict: bool,

	/// Enable debug logging.
	#[arg(
		short = 'd',
		long = "debug",
		default_value_t = false,
		help_heading = "Display options"
	)]
	pub debug: bool,

	/// Control non-debug ANSI styling (`auto`, `always`, `never`).
	#[arg(
		long = "render",
		value_name = "mode",
		value_enum,
		default_value_t = RenderMode::Auto,
		help_heading = "Display options"
	)]
	pub render: RenderMode,

	/// Disable ANSI styling in non-debug output.
	#[arg(
		long = "no-color",
		default_value_t = false,
		conflicts_with = "render",
		help_heading = "Display options"
	)]
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
	#[arg(long = "config", value_name = "path", help_heading = "Input options")]
	pub config: Option<PathBuf>,

	/// Print only the resolved config path.
	#[arg(
		long = "path-only",
		default_value_t = false,
		conflicts_with_all = ["json", "format_only", "exists_only", "source_only"],
		help_heading = "Output options"
	)]
	pub path_only: bool,

	/// Print only the resolved config format.
	#[arg(
		long = "format-only",
		default_value_t = false,
		conflicts_with_all = ["json", "path_only", "exists_only", "source_only"],
		help_heading = "Output options"
	)]
	pub format_only: bool,

	/// Print only whether the resolved config file exists.
	#[arg(
		long = "exists-only",
		default_value_t = false,
		conflicts_with_all = ["json", "path_only", "format_only", "source_only"],
		help_heading = "Output options"
	)]
	pub exists_only: bool,

	/// Print only the config source (`discovered` or `explicit`).
	#[arg(
		long = "source-only",
		default_value_t = false,
		conflicts_with_all = ["json", "path_only", "format_only", "exists_only"],
		help_heading = "Output options"
	)]
	pub source_only: bool,

	/// Print machine-readable JSON instead of text output.
	#[arg(
		long = "json",
		default_value_t = false,
		conflicts_with_all = ["path_only", "format_only", "exists_only", "source_only"],
		help_heading = "Output options"
	)]
	pub json: bool,

	/// Exit non-zero if the resolved config file does not exist.
	#[arg(
		long = "require-existing",
		default_value_t = false,
		help_heading = "Resolution options"
	)]
	pub require_existing: bool,

	/// Enable debug logging.
	#[arg(
		short = 'd',
		long = "debug",
		default_value_t = false,
		help_heading = "Display options"
	)]
	pub debug: bool,

	/// Control non-debug ANSI styling (`auto`, `always`, `never`).
	#[arg(
		long = "render",
		value_name = "mode",
		value_enum,
		default_value_t = RenderMode::Auto,
		help_heading = "Display options"
	)]
	pub render: RenderMode,

	/// Disable ANSI styling in non-debug output.
	#[arg(
		long = "no-color",
		default_value_t = false,
		conflicts_with = "render",
		help_heading = "Display options"
	)]
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
	#[arg(
		long = "format",
		value_name = "format",
		value_enum,
		help_heading = "Generation options"
	)]
	pub format: Option<InitFormat>,

	/// Print the starter config to stdout instead of writing a file.
	#[arg(
		long = "stdout",
		default_value_t = false,
		conflicts_with_all = ["dry_run", "json"],
		help_heading = "Output options"
	)]
	pub stdout: bool,

	/// Write the starter config to an explicit path.
	#[arg(
		long = "output",
		value_name = "path",
		conflicts_with = "stdout",
		help_heading = "Output options"
	)]
	pub output: Option<PathBuf>,

	/// Print the init plan without writing a config file.
	#[arg(long = "dry-run", default_value_t = false, help_heading = "Output options")]
	pub dry_run: bool,

	/// Print machine-readable JSON instead of text output.
	#[arg(
		long = "json",
		default_value_t = false,
		conflicts_with_all = ["path_only", "format_only", "action_only"],
		help_heading = "Output options"
	)]
	pub json: bool,

	/// Print only the path that init would write.
	#[arg(
		long = "path-only",
		default_value_t = false,
		requires = "dry_run",
		conflicts_with_all = ["json", "stdout", "format_only", "action_only"],
		help_heading = "Output options"
	)]
	pub path_only: bool,

	/// Print only the config format that init would write.
	#[arg(
		long = "format-only",
		default_value_t = false,
		requires = "dry_run",
		conflicts_with_all = ["json", "stdout", "path_only", "action_only"],
		help_heading = "Output options"
	)]
	pub format_only: bool,

	/// Print only the action that init would take.
	#[arg(
		long = "action-only",
		default_value_t = false,
		requires = "dry_run",
		conflicts_with_all = ["json", "stdout", "path_only", "format_only"],
		help_heading = "Output options"
	)]
	pub action_only: bool,

	/// Overwrite an existing pullhook config file in place.
	#[arg(
		long = "force",
		default_value_t = false,
		conflicts_with = "stdout",
		help_heading = "Write options"
	)]
	pub force: bool,

	/// Enable debug logging.
	#[arg(
		short = 'd',
		long = "debug",
		default_value_t = false,
		help_heading = "Display options"
	)]
	pub debug: bool,

	/// Control non-debug ANSI styling (`auto`, `always`, `never`).
	#[arg(
		long = "render",
		value_name = "mode",
		value_enum,
		default_value_t = RenderMode::Auto,
		help_heading = "Display options"
	)]
	pub render: RenderMode,

	/// Disable ANSI styling in non-debug output.
	#[arg(
		long = "no-color",
		default_value_t = false,
		conflicts_with = "render",
		help_heading = "Display options"
	)]
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
	#[arg(long = "config", value_name = "path", help_heading = "Input options")]
	pub config: Option<PathBuf>,

	/// Print JSON with filters and searchFields metadata.
	#[arg(
		long = "json",
		default_value_t = false,
		conflicts_with_all = [
			"count_only",
			"names_only",
			"commands_only",
			"patterns_only",
			"exclude_patterns_only",
			"fail_text_only"
		],
		help_heading = "Output options"
	)]
	pub json: bool,

	/// Print only the number of matching rule selectors.
	#[arg(
		long = "count-only",
		default_value_t = false,
		conflicts_with_all = [
			"json",
			"names_only",
			"commands_only",
			"patterns_only",
			"exclude_patterns_only",
			"fail_text_only"
		],
		help_heading = "Output options"
	)]
	pub count_only: bool,

	/// Print only rule and group selector names, one per line.
	#[arg(
		long = "names-only",
		default_value_t = false,
		conflicts_with_all = [
			"json",
			"count_only",
			"commands_only",
			"patterns_only",
			"exclude_patterns_only",
			"fail_text_only"
		],
		help_heading = "Output options"
	)]
	pub names_only: bool,

	/// Print only configured run commands, one per line.
	#[arg(
		long = "commands-only",
		default_value_t = false,
		conflicts_with_all = [
			"json",
			"count_only",
			"names_only",
			"patterns_only",
			"exclude_patterns_only",
			"fail_text_only"
		],
		help_heading = "Output options"
	)]
	pub commands_only: bool,

	/// Print only configured changed-file patterns, one per line.
	#[arg(
		long = "patterns-only",
		default_value_t = false,
		conflicts_with_all = [
			"json",
			"count_only",
			"names_only",
			"commands_only",
			"exclude_patterns_only",
			"fail_text_only"
		],
		help_heading = "Output options"
	)]
	pub patterns_only: bool,

	/// Print only configured exclude patterns, one per line.
	#[arg(
		long = "exclude-patterns-only",
		default_value_t = false,
		conflicts_with_all = [
			"json",
			"count_only",
			"names_only",
			"commands_only",
			"patterns_only",
			"fail_text_only"
		],
		help_heading = "Output options"
	)]
	pub exclude_patterns_only: bool,

	/// Print only configured failText templates, one per line.
	#[arg(
		long = "fail-text-only",
		default_value_t = false,
		conflicts_with_all = [
			"json",
			"count_only",
			"names_only",
			"commands_only",
			"patterns_only",
			"exclude_patterns_only"
		],
		help_heading = "Output options"
	)]
	pub fail_text_only: bool,

	/// Limit inventory to all entries, leaf rules, groups, run rules, or install rules.
	#[arg(
		long = "kind",
		value_name = "kind",
		value_enum,
		default_value_t = RulesKind::All,
		help_heading = "Selection options"
	)]
	pub kind: RulesKind,

	/// Limit inventory to one or more named rules or parallel groups.
	#[arg(long = "rule", value_name = "name", help_heading = "Selection options")]
	pub rules: Vec<String>,

	/// Only list rules or groups whose inventory fields contain this text.
	#[arg(long = "search", value_name = "text", help_heading = "Selection options")]
	pub search: Option<String>,

	/// Enable debug logging.
	#[arg(
		short = 'd',
		long = "debug",
		default_value_t = false,
		help_heading = "Display options"
	)]
	pub debug: bool,

	/// Control non-debug ANSI styling (`auto`, `always`, `never`).
	#[arg(
		long = "render",
		value_name = "mode",
		value_enum,
		default_value_t = RenderMode::Auto,
		help_heading = "Display options"
	)]
	pub render: RenderMode,

	/// Disable ANSI styling in non-debug output.
	#[arg(
		long = "no-color",
		default_value_t = false,
		conflicts_with = "render",
		help_heading = "Display options"
	)]
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
	#[arg(long = "output", value_name = "path", help_heading = "Output options")]
	pub output: Option<PathBuf>,

	/// Check that the output file already matches the embedded schema.
	#[arg(
		long = "check",
		default_value_t = false,
		requires = "output",
		help_heading = "Check options"
	)]
	pub check: bool,

	/// Print machine-readable check results instead of text output.
	#[arg(
		long = "json",
		default_value_t = false,
		requires = "check",
		conflicts_with = "quiet",
		help_heading = "Output options"
	)]
	pub json: bool,

	/// Suppress successful check output.
	#[arg(
		long = "quiet",
		default_value_t = false,
		requires = "check",
		conflicts_with = "json",
		help_heading = "Output options"
	)]
	pub quiet: bool,
}

/// Arguments for `pullhook completion`.
#[derive(Debug, Clone, Args)]
#[command(after_help = COMPLETION_AFTER_HELP)]
pub struct CompletionArgs {
	/// Shell to generate completions for.
	pub shell: Shell,

	/// Write completions to a file instead of stdout.
	#[arg(long = "output", value_name = "path", help_heading = "Output options")]
	pub output: Option<PathBuf>,

	/// Check that the output file already matches the generated completions.
	#[arg(
		long = "check",
		default_value_t = false,
		requires = "output",
		help_heading = "Check options"
	)]
	pub check: bool,

	/// Print machine-readable check results instead of text output.
	#[arg(
		long = "json",
		default_value_t = false,
		requires = "check",
		conflicts_with = "quiet",
		help_heading = "Output options"
	)]
	pub json: bool,

	/// Suppress successful check output.
	#[arg(
		long = "quiet",
		default_value_t = false,
		requires = "check",
		conflicts_with = "json",
		help_heading = "Output options"
	)]
	pub quiet: bool,
}

/// Arguments for `pullhook shells`.
#[derive(Debug, Clone, Args)]
#[expect(
	clippy::struct_excessive_bools,
	reason = "clap line-output flags are clearer as independent switches"
)]
#[command(after_help = SHELLS_AFTER_HELP)]
pub struct ShellsArgs {
	/// Only list shells whose name, completion command, or description contains this text.
	#[arg(long = "search", value_name = "TEXT", help_heading = "Filter options")]
	pub search: Option<String>,

	/// Print JSON with filters and searchFields metadata.
	#[arg(
		long = "json",
		default_value_t = false,
		conflicts_with_all = ["count_only", "names_only", "commands_only", "descriptions_only"],
		help_heading = "Output options"
	)]
	pub json: bool,

	/// Print only the number of matching shells.
	#[arg(
		long = "count-only",
		default_value_t = false,
		conflicts_with_all = ["json", "names_only", "commands_only", "descriptions_only"],
		help_heading = "Output options"
	)]
	pub count_only: bool,

	/// Print only shell names, one per line.
	#[arg(
		long = "names-only",
		default_value_t = false,
		conflicts_with_all = ["json", "count_only", "commands_only", "descriptions_only"],
		help_heading = "Output options"
	)]
	pub names_only: bool,

	/// Print only completion commands, one per line.
	#[arg(
		long = "commands-only",
		default_value_t = false,
		conflicts_with_all = ["json", "count_only", "names_only", "descriptions_only"],
		help_heading = "Output options"
	)]
	pub commands_only: bool,

	/// Print only shell descriptions, one per line.
	#[arg(
		long = "descriptions-only",
		default_value_t = false,
		conflicts_with_all = ["json", "count_only", "names_only", "commands_only"],
		help_heading = "Output options"
	)]
	pub descriptions_only: bool,
}

/// Arguments for `pullhook formats`.
#[derive(Debug, Clone, Args)]
#[command(after_help = FORMATS_AFTER_HELP)]
pub struct FormatsArgs {
	/// Only list formats whose name, filenames, description, or init command contain this text.
	#[arg(long = "search", value_name = "TEXT", help_heading = "Filter options")]
	pub search: Option<String>,

	/// Print JSON with filters and searchFields metadata.
	#[arg(
		long = "json",
		default_value_t = false,
		conflicts_with_all = ["count_only", "names_only", "files_only", "init_commands_only", "descriptions_only"],
		help_heading = "Output options"
	)]
	pub json: bool,

	#[command(flatten)]
	pub output: FormatsLineOutputArgs,
}

/// Line-output mode arguments for `pullhook formats`.
#[derive(Debug, Clone, Args)]
#[expect(
	clippy::struct_excessive_bools,
	reason = "clap line-output flags are clearer as independent switches"
)]
pub struct FormatsLineOutputArgs {
	/// Print only the number of matching formats.
	#[arg(
		long = "count-only",
		default_value_t = false,
		conflicts_with_all = ["json", "names_only", "files_only", "init_commands_only", "descriptions_only"],
		help_heading = "Output options"
	)]
	pub count_only: bool,

	/// Print only config format names, one per line.
	#[arg(
		long = "names-only",
		default_value_t = false,
		conflicts_with_all = ["json", "count_only", "files_only", "init_commands_only", "descriptions_only"],
		help_heading = "Output options"
	)]
	pub names_only: bool,

	/// Print only default config filenames, one per line.
	#[arg(
		long = "files-only",
		default_value_t = false,
		conflicts_with_all = ["json", "count_only", "names_only", "init_commands_only", "descriptions_only"],
		help_heading = "Output options"
	)]
	pub files_only: bool,

	/// Print only config init commands, one per line.
	#[arg(
		long = "init-commands-only",
		default_value_t = false,
		conflicts_with_all = ["json", "count_only", "names_only", "files_only", "descriptions_only"],
		help_heading = "Output options"
	)]
	pub init_commands_only: bool,

	/// Print only config format descriptions, one per line.
	#[arg(
		long = "descriptions-only",
		default_value_t = false,
		conflicts_with_all = ["json", "count_only", "names_only", "files_only", "init_commands_only"],
		help_heading = "Output options"
	)]
	pub descriptions_only: bool,
}

/// Arguments for `pullhook managers`.
#[derive(Debug, Clone, Args)]
#[command(after_help = MANAGERS_AFTER_HELP)]
pub struct ManagersArgs {
	/// Only list package managers whose name, command, pattern, or watched files contain this text.
	#[arg(long = "search", value_name = "TEXT", help_heading = "Filter options")]
	pub search: Option<String>,

	/// Print JSON with filters and searchFields metadata.
	#[arg(
		long = "json",
		default_value_t = false,
		conflicts_with_all = [
			"count_only",
			"names_only",
			"patterns_only",
			"commands_only",
			"lock_files_only",
			"config_files_only",
			"watched_files_only"
		],
		help_heading = "Output options"
	)]
	pub json: bool,

	#[command(flatten)]
	pub output: ManagersLineOutputArgs,
}

/// Line-output mode arguments for `pullhook managers`.
#[derive(Debug, Clone, Args)]
#[expect(
	clippy::struct_excessive_bools,
	reason = "clap line-output flags are clearer as independent switches"
)]
pub struct ManagersLineOutputArgs {
	/// Print only the number of matching package managers.
	#[arg(
		long = "count-only",
		default_value_t = false,
		conflicts_with_all = [
			"json",
			"names_only",
			"patterns_only",
			"commands_only",
			"lock_files_only",
			"config_files_only",
			"watched_files_only"
		],
		help_heading = "Output options"
	)]
	pub count_only: bool,

	/// Print only package-manager names, one per line.
	#[arg(
		long = "names-only",
		default_value_t = false,
		conflicts_with_all = [
			"json",
			"count_only",
			"patterns_only",
			"commands_only",
			"lock_files_only",
			"config_files_only",
			"watched_files_only"
		],
		help_heading = "Output options"
	)]
	pub names_only: bool,

	/// Print only install detection patterns, one per line.
	#[arg(
		long = "patterns-only",
		default_value_t = false,
		conflicts_with_all = [
			"json",
			"count_only",
			"names_only",
			"commands_only",
			"lock_files_only",
			"config_files_only",
			"watched_files_only"
		],
		help_heading = "Output options"
	)]
	pub patterns_only: bool,

	/// Print only install commands, one per line.
	#[arg(
		long = "commands-only",
		default_value_t = false,
		conflicts_with_all = [
			"json",
			"count_only",
			"names_only",
			"patterns_only",
			"lock_files_only",
			"config_files_only",
			"watched_files_only"
		],
		help_heading = "Output options"
	)]
	pub commands_only: bool,

	/// Print only package-manager lock files, one per line.
	#[arg(
		long = "lock-files-only",
		default_value_t = false,
		conflicts_with_all = [
			"json",
			"count_only",
			"names_only",
			"patterns_only",
			"commands_only",
			"config_files_only",
			"watched_files_only"
		],
		help_heading = "Output options"
	)]
	pub lock_files_only: bool,

	/// Print only package-manager config files, one per line.
	#[arg(
		long = "config-files-only",
		default_value_t = false,
		conflicts_with_all = [
			"json",
			"count_only",
			"names_only",
			"patterns_only",
			"commands_only",
			"lock_files_only",
			"watched_files_only"
		],
		help_heading = "Output options"
	)]
	pub config_files_only: bool,

	/// Print only watched package-manager files, one per line.
	#[arg(
		long = "watched-files-only",
		default_value_t = false,
		conflicts_with_all = [
			"json",
			"count_only",
			"names_only",
			"patterns_only",
			"commands_only",
			"lock_files_only",
			"config_files_only"
		],
		help_heading = "Output options"
	)]
	pub watched_files_only: bool,
}

/// Arguments for `pullhook categories`.
#[derive(Debug, Clone, Args)]
#[expect(
	clippy::struct_excessive_bools,
	reason = "CLI line-output flags are clearer as independent booleans"
)]
#[command(after_help = CATEGORIES_AFTER_HELP)]
pub struct CategoriesArgs {
	/// Only list categories whose name or description contains this text.
	#[arg(long = "search", value_name = "TEXT", help_heading = "Filter options")]
	pub search: Option<String>,

	/// Print JSON with filters and searchFields metadata.
	#[arg(
		long = "json",
		default_value_t = false,
		conflicts_with_all = ["count_only", "names_only", "commands_only", "example_commands_only", "descriptions_only"],
		help_heading = "Output options"
	)]
	pub json: bool,

	/// Print only the number of matching categories.
	#[arg(
		long = "count-only",
		default_value_t = false,
		conflicts_with_all = ["json", "names_only", "commands_only", "example_commands_only", "descriptions_only"],
		help_heading = "Output options"
	)]
	pub count_only: bool,

	/// Print only category names, one per line.
	#[arg(
		long = "names-only",
		default_value_t = false,
		conflicts_with_all = ["json", "count_only", "commands_only", "example_commands_only", "descriptions_only"],
		help_heading = "Output options"
	)]
	pub names_only: bool,

	/// Print only command names for matching categories, one per line.
	#[arg(
		long = "commands-only",
		default_value_t = false,
		conflicts_with_all = ["json", "count_only", "names_only", "example_commands_only", "descriptions_only"],
		help_heading = "Output options"
	)]
	pub commands_only: bool,

	/// Print only example commands for matching categories, one per line.
	#[arg(
		long = "example-commands-only",
		default_value_t = false,
		conflicts_with_all = ["json", "count_only", "names_only", "commands_only", "descriptions_only"],
		help_heading = "Output options"
	)]
	pub example_commands_only: bool,

	/// Print only category descriptions, one per line.
	#[arg(
		long = "descriptions-only",
		default_value_t = false,
		conflicts_with_all = ["json", "count_only", "names_only", "commands_only", "example_commands_only"],
		help_heading = "Output options"
	)]
	pub descriptions_only: bool,
}

/// Arguments for `pullhook codes`.
#[derive(Debug, Clone, Args)]
#[command(after_help = CODES_AFTER_HELP)]
pub struct CodesArgs {
	/// Only list codes for a specific kind.
	#[arg(long = "kind", value_enum, help_heading = "Filter options")]
	pub kind: Option<CodeKind>,

	/// Only list codes whose surface contains this text.
	#[arg(long = "surface", value_name = "TEXT", help_heading = "Filter options")]
	pub surface: Option<String>,

	/// Only list codes whose code, surface, kind, or description contains this text.
	#[arg(long = "search", value_name = "TEXT", help_heading = "Filter options")]
	pub search: Option<String>,

	/// Print JSON with filters and searchFields metadata.
	#[arg(
		long = "json",
		default_value_t = false,
		conflicts_with_all = ["count_only", "codes_only", "surfaces_only", "kinds_only", "descriptions_only"],
		help_heading = "Output options"
	)]
	pub json: bool,

	#[command(flatten)]
	pub output: CodesLineOutputArgs,
}

/// Line-output mode arguments for `pullhook codes`.
#[derive(Debug, Clone, Args)]
pub struct CodesLineOutputArgs {
	#[command(flatten)]
	pub values: CodesValueOutputArgs,

	#[command(flatten)]
	pub facets: CodesFacetOutputArgs,
}

/// Value line-output mode arguments for `pullhook codes`.
#[derive(Debug, Clone, Args)]
pub struct CodesValueOutputArgs {
	/// Print only the number of matching codes.
	#[arg(
		long = "count-only",
		default_value_t = false,
		conflicts_with_all = ["json", "codes_only", "surfaces_only", "kinds_only", "descriptions_only"],
		help_heading = "Output options"
	)]
	pub count_only: bool,

	/// Print only stable codes, one per line.
	#[arg(
		long = "codes-only",
		default_value_t = false,
		conflicts_with_all = ["json", "count_only", "surfaces_only", "kinds_only", "descriptions_only"],
		help_heading = "Output options"
	)]
	pub codes_only: bool,

	/// Print only matching code descriptions, one per line.
	#[arg(
		long = "descriptions-only",
		default_value_t = false,
		conflicts_with_all = ["json", "count_only", "codes_only", "kinds_only", "surfaces_only"],
		help_heading = "Output options"
	)]
	pub descriptions_only: bool,
}

/// Facet line-output mode arguments for `pullhook codes`.
#[derive(Debug, Clone, Args)]
pub struct CodesFacetOutputArgs {
	/// Print only matching code kinds, one per line.
	#[arg(
		long = "kinds-only",
		default_value_t = false,
		conflicts_with_all = ["json", "count_only", "codes_only", "surfaces_only", "descriptions_only"],
		help_heading = "Output options"
	)]
	pub kinds_only: bool,

	/// Print only matching code surfaces, one per line.
	#[arg(
		long = "surfaces-only",
		default_value_t = false,
		conflicts_with_all = ["json", "count_only", "codes_only", "kinds_only", "descriptions_only"],
		help_heading = "Output options"
	)]
	pub surfaces_only: bool,
}

/// Arguments for `pullhook commands`.
#[derive(Debug, Clone, Args)]
#[command(after_help = COMMANDS_AFTER_HELP)]
pub struct CommandCatalogArgs {
	/// Only list commands for a specific category.
	#[arg(long = "category", value_enum, help_heading = "Filter options")]
	pub category: Option<CommandCategory>,

	/// Only list commands whose name, category, summary, or examples contain this text.
	#[arg(long = "search", value_name = "TEXT", help_heading = "Filter options")]
	pub search: Option<String>,

	#[command(flatten)]
	pub filters: CommandCatalogFilterArgs,

	/// Print JSON with filters and searchFields metadata.
	#[arg(
		long = "json",
		default_value_t = false,
		conflicts_with_all = [
			"markdown",
			"count_only",
			"names_only",
			"summaries_only",
			"categories_only",
			"example_commands_only"
		],
		help_heading = "Output options"
	)]
	pub json: bool,

	/// Print a Markdown command reference table.
	#[arg(
		long = "markdown",
		default_value_t = false,
		conflicts_with_all = ["json", "count_only", "names_only", "summaries_only", "categories_only", "example_commands_only"],
		help_heading = "Output options"
	)]
	pub markdown: bool,

	#[command(flatten)]
	pub output: CommandCatalogLineOutputArgs,
}

/// Line-output mode arguments for `pullhook commands`.
#[derive(Debug, Clone, Args)]
#[expect(
	clippy::struct_excessive_bools,
	reason = "clap line-output flags are clearer as independent switches"
)]
pub struct CommandCatalogLineOutputArgs {
	/// Print only the number of matching commands.
	#[arg(
		long = "count-only",
		default_value_t = false,
		conflicts_with_all = ["json", "markdown", "names_only", "summaries_only", "categories_only", "example_commands_only"],
		help_heading = "Output options"
	)]
	pub count_only: bool,

	/// Print only command names, one per line.
	#[arg(
		long = "names-only",
		default_value_t = false,
		conflicts_with_all = ["json", "markdown", "count_only", "summaries_only", "categories_only", "example_commands_only"],
		help_heading = "Output options"
	)]
	pub names_only: bool,

	/// Print only command summaries, one per line.
	#[arg(
		long = "summaries-only",
		default_value_t = false,
		conflicts_with_all = ["json", "markdown", "count_only", "names_only", "categories_only", "example_commands_only"],
		help_heading = "Output options"
	)]
	pub summaries_only: bool,

	/// Print only command categories, one per line.
	#[arg(
		long = "categories-only",
		default_value_t = false,
		conflicts_with_all = ["json", "markdown", "count_only", "names_only", "summaries_only", "example_commands_only"],
		help_heading = "Output options"
	)]
	pub categories_only: bool,

	/// Print example commands for matching commands, one per line.
	#[arg(
		long = "example-commands-only",
		default_value_t = false,
		conflicts_with_all = ["json", "markdown", "count_only", "names_only", "summaries_only", "categories_only"],
		help_heading = "Output options"
	)]
	pub example_commands_only: bool,
}

/// Filter arguments for `pullhook commands`.
#[derive(Debug, Clone, Args)]
pub struct CommandCatalogFilterArgs {
	/// Only list commands that require a git repository.
	#[arg(
		long = "repo-only",
		default_value_t = false,
		conflicts_with = "standalone_only",
		help_heading = "Filter options"
	)]
	pub repo_only: bool,

	/// Only list commands that can run outside a git repository.
	#[arg(
		long = "standalone-only",
		default_value_t = false,
		conflicts_with = "repo_only",
		help_heading = "Filter options"
	)]
	pub standalone_only: bool,
}

/// Arguments for `pullhook examples`.
#[derive(Debug, Clone, Args)]
#[command(after_help = EXAMPLES_AFTER_HELP)]
pub struct ExamplesArgs {
	/// Only list examples for a specific command.
	#[arg(long = "command", value_enum, help_heading = "Filter options")]
	pub command: Option<ExampleCommand>,

	/// Only list examples for a specific command category.
	#[arg(long = "category", value_enum, help_heading = "Filter options")]
	pub category: Option<CommandCategory>,

	/// Only list examples whose title, command, category, or summary contains this text.
	#[arg(long = "search", value_name = "TEXT", help_heading = "Filter options")]
	pub search: Option<String>,

	/// Print JSON with filters and searchFields metadata.
	#[arg(
		long = "json",
		default_value_t = false,
		conflicts_with_all = [
			"count_only",
			"commands_only",
			"command_names_only",
			"titles_only",
			"summaries_only",
			"categories_only"
		],
		help_heading = "Output options"
	)]
	pub json: bool,

	#[command(flatten)]
	pub output: ExamplesLineOutputArgs,
}

/// Line-output mode arguments for `pullhook examples`.
#[derive(Debug, Clone, Args)]
#[expect(
	clippy::struct_excessive_bools,
	reason = "clap line-output flags are clearer as independent switches"
)]
pub struct ExamplesLineOutputArgs {
	/// Print only the number of matching examples.
	#[arg(
		long = "count-only",
		default_value_t = false,
		conflicts_with_all = [
			"json",
			"commands_only",
			"command_names_only",
			"titles_only",
			"summaries_only",
			"categories_only"
		],
		help_heading = "Output options"
	)]
	pub count_only: bool,

	/// Print only example commands, one per line.
	#[arg(
		long = "commands-only",
		default_value_t = false,
		conflicts_with_all = [
			"json",
			"count_only",
			"command_names_only",
			"titles_only",
			"summaries_only",
			"categories_only"
		],
		help_heading = "Output options"
	)]
	pub commands_only: bool,

	/// Print only example command names, one per line.
	#[arg(
		long = "command-names-only",
		default_value_t = false,
		conflicts_with_all = ["json", "count_only", "commands_only", "titles_only", "summaries_only", "categories_only"],
		help_heading = "Output options"
	)]
	pub command_names_only: bool,

	/// Print only example titles, one per line.
	#[arg(
		long = "titles-only",
		default_value_t = false,
		conflicts_with_all = ["json", "count_only", "commands_only", "command_names_only", "summaries_only", "categories_only"],
		help_heading = "Output options"
	)]
	pub titles_only: bool,

	/// Print only example summaries, one per line.
	#[arg(
		long = "summaries-only",
		default_value_t = false,
		conflicts_with_all = ["json", "count_only", "commands_only", "command_names_only", "titles_only", "categories_only"],
		help_heading = "Output options"
	)]
	pub summaries_only: bool,

	/// Print only example categories, one per line.
	#[arg(
		long = "categories-only",
		default_value_t = false,
		conflicts_with_all = ["json", "count_only", "commands_only", "command_names_only", "titles_only", "summaries_only"],
		help_heading = "Output options"
	)]
	pub categories_only: bool,
}

/// Commands that have workflow examples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ExampleCommand {
	/// Legacy top-level one-off mode examples.
	Legacy,
	/// Config initialization examples.
	Init,
	/// Config explanation examples.
	Explain,
	/// Config execution examples.
	Run,
	/// Config validation examples.
	Validate,
	/// Repository diagnostic examples.
	Doctor,
	/// Config path discovery examples.
	Config,
	/// Config rule inventory examples.
	Rules,
	/// JSON Schema generation examples.
	Schema,
	/// Shell completion generation examples.
	Completion,
	/// Command catalog examples.
	Commands,
	/// Example workflow catalog examples.
	Examples,
	/// Shell completion target examples.
	Shells,
	/// Config format examples.
	Formats,
	/// Package-manager install detection examples.
	Managers,
	/// Command category examples.
	Categories,
	/// Status code catalog examples.
	Codes,
}

impl ExampleCommand {
	pub const fn label(self) -> &'static str {
		match self {
			Self::Legacy => "legacy",
			Self::Init => "init",
			Self::Explain => "explain",
			Self::Run => "run",
			Self::Validate => "validate",
			Self::Doctor => "doctor",
			Self::Config => "config",
			Self::Rules => "rules",
			Self::Schema => "schema",
			Self::Completion => "completion",
			Self::Commands => "commands",
			Self::Examples => "examples",
			Self::Shells => "shells",
			Self::Formats => "formats",
			Self::Managers => "managers",
			Self::Categories => "categories",
			Self::Codes => "codes",
		}
	}
}

/// Supported `pullhook commands` categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CommandCategory {
	/// Commands that evaluate or run configured workflows.
	Workflow,
	/// Commands that inspect repository or config state.
	Diagnostic,
	/// Commands that generate files or shell output.
	Generator,
	/// Commands that describe pullhook's own CLI surface.
	Reference,
}

impl CommandCategory {
	pub const fn label(self) -> &'static str {
		match self {
			Self::Workflow => "workflow",
			Self::Diagnostic => "diagnostic",
			Self::Generator => "generator",
			Self::Reference => "reference",
		}
	}
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
