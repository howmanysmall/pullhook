//! CLI parsing and argument helpers.

use std::num::NonZeroUsize;

use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};

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
	#[arg(required_unless_present = "install")]
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
	fn run_args_conflict_with_completion_subcommand() {
		let error = Cli::try_parse_from(["pullhook", "--install", "completion", "bash"]).expect_err("mixed args fail");

		assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
	}
}
