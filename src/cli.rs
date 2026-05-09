//! CLI parsing and argument helpers.

use std::num::NonZeroUsize;

use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{Shell, generate};

use crate::config::ConfigFormat;
use crate::output::RenderMode;

/// Pullhook command line arguments.
#[derive(Debug, Clone, Parser)]
#[command(name = "pullhook")]
#[command(about = "Run commands when files change after git pull")]
#[command(version)]
#[command(args_conflicts_with_subcommands = true)]
#[command(subcommand_negates_reqs = true)]
#[command(propagate_version = true)]
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
	/// Create a starter pullhook config file.
	Init(InitArgs),
	/// Generate shell completion scripts.
	Completion {
		/// Shell to generate completions for.
		shell: Shell,
	},
}

/// Arguments for the default pullhook execution flow.
#[derive(Debug, Clone, Args)]
#[expect(
	clippy::struct_excessive_bools,
	reason = "CLI flags are naturally represented as independent booleans"
)]
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

	/// Dedupe directories before per-match execution.
	#[arg(long = "unique-cwd", default_value_t = false)]
	pub unique_cwd: bool,
}

/// Arguments for `pullhook run`.
#[derive(Debug, Clone, Args)]
pub struct ConfigRunArgs {
	/// Override the git base revision.
	#[arg(long = "base", value_name = "rev")]
	pub base: Option<String>,

	/// Max concurrent jobs for top-level work.
	#[arg(long = "jobs", value_name = "n")]
	pub jobs: Option<NonZeroUsize>,

	/// Print planned commands and exit.
	#[arg(long = "dry-run", default_value_t = false)]
	pub dry_run: bool,

	/// Show skipped rules as well as matched rules.
	#[arg(long = "all-matches", default_value_t = false)]
	pub all_matches: bool,

	/// Enable debug logging.
	#[arg(short = 'd', long = "debug", default_value_t = false)]
	pub debug: bool,

	/// Control non-debug ANSI styling (`auto`, `always`, `never`).
	#[arg(long = "render", value_name = "mode", value_enum, default_value_t = RenderMode::Auto)]
	pub render: RenderMode,
}

/// Arguments for `pullhook explain`.
#[derive(Debug, Clone, Args)]
pub struct ExplainArgs {
	/// Override the git base revision.
	#[arg(long = "base", value_name = "rev")]
	pub base: Option<String>,

	/// Show skipped rules as well as matched rules.
	#[arg(long = "all-matches", default_value_t = false)]
	pub all_matches: bool,

	/// Print machine-readable JSON instead of text output.
	#[arg(long = "json", default_value_t = false)]
	pub json: bool,

	/// Enable debug logging.
	#[arg(short = 'd', long = "debug", default_value_t = false)]
	pub debug: bool,

	/// Control non-debug ANSI styling (`auto`, `always`, `never`).
	#[arg(long = "render", value_name = "mode", value_enum, default_value_t = RenderMode::Auto)]
	pub render: RenderMode,
}

/// Arguments for `pullhook validate`.
#[derive(Debug, Clone, Args)]
pub struct ValidateArgs {
	/// Print machine-readable JSON instead of text output.
	#[arg(long = "json", default_value_t = false)]
	pub json: bool,

	/// Enable debug logging.
	#[arg(short = 'd', long = "debug", default_value_t = false)]
	pub debug: bool,

	/// Control non-debug ANSI styling (`auto`, `always`, `never`).
	#[arg(long = "render", value_name = "mode", value_enum, default_value_t = RenderMode::Auto)]
	pub render: RenderMode,
}

/// Arguments for `pullhook init`.
#[derive(Debug, Clone, Args)]
pub struct InitArgs {
	/// Config format to generate.
	#[arg(long = "format", value_name = "format", value_enum)]
	pub format: Option<InitFormat>,

	/// Print the starter config to stdout instead of writing a file.
	#[arg(long = "stdout", default_value_t = false)]
	pub stdout: bool,

	/// Overwrite an existing pullhook config file in place.
	#[arg(long = "force", default_value_t = false)]
	pub force: bool,

	/// Enable debug logging.
	#[arg(short = 'd', long = "debug", default_value_t = false)]
	pub debug: bool,

	/// Control non-debug ANSI styling (`auto`, `always`, `never`).
	#[arg(long = "render", value_name = "mode", value_enum, default_value_t = RenderMode::Auto)]
	pub render: RenderMode,
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
	/// Write shell completions to stdout.
	pub fn print_completion(shell: Shell) {
		generate(shell, &mut Self::command(), "pullhook", &mut std::io::stdout());
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

		assert!(matches!(cli.command, Some(Commands::Completion { shell: Shell::Bash })));
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
