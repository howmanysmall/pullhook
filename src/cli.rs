//! CLI parsing and argument helpers.

use std::num::NonZeroUsize;

use clap::Parser;

/// Pullhook command line arguments.
#[derive(Debug, Clone, Parser)]
#[command(name = "pullhook")]
#[command(about = "Run commands when files change after git pull")]
#[command(version)]
#[expect(
	clippy::struct_excessive_bools,
	reason = "CLI flags are naturally represented as independent booleans"
)]
pub struct Cli {
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
