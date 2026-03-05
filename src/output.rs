//! Unified non-debug output rendering and style controls.

use std::cell::Cell;
use std::env;
use std::fmt::Display;
use std::io::{self, IsTerminal};

use clap::ValueEnum;

/// Environment override for render mode, primarily for deterministic tests.
pub const RENDER_MODE_ENV: &str = "PULLHOOK_RENDER_MODE";

/// Non-debug render mode.
#[derive(Debug, Clone, Copy, Eq, PartialEq, ValueEnum)]
pub enum RenderMode {
	/// Respect terminal detection.
	Auto,
	/// Force ANSI styling.
	Always,
	/// Force plain output.
	Never,
}

impl RenderMode {
	#[must_use]
	fn effective(self) -> Self {
		env::var(RENDER_MODE_ENV)
			.ok()
			.and_then(|value| Self::from_env_value(&value))
			.unwrap_or(self)
	}

	fn from_env_value(value: &str) -> Option<Self> {
		match value.trim().to_ascii_lowercase().as_str() {
			"auto" => Some(Self::Auto),
			"always" => Some(Self::Always),
			"never" => Some(Self::Never),
			_ => None,
		}
	}

	#[must_use]
	fn use_style(self) -> bool {
		match self {
			Self::Auto => io::stdout().is_terminal() && io::stderr().is_terminal(),
			Self::Always => true,
			Self::Never => false,
		}
	}
}

/// Deterministic summary payload for the final stage.
#[derive(Debug, Clone, Copy)]
pub struct Summary {
	/// Number of matched changed files.
	pub matched_files: usize,
	/// Number of task directories considered.
	pub task_dirs: usize,
	/// Number of task directories that passed.
	pub passed: usize,
	/// Number of task directories that failed (including spawn failures).
	pub failed: usize,
	/// Number of task directories interrupted without an exit code.
	pub interrupted: usize,
}

/// Deterministic dry-run summary payload for the final stage.
#[derive(Debug, Clone, Copy)]
pub struct DryRunSummary {
	/// Number of matched changed files.
	pub matched_files: usize,
	/// Number of task directories considered.
	pub task_dirs: usize,
	/// Number of commands that would run.
	pub planned_commands: usize,
}

/// Deterministic non-success task report payload.
#[derive(Debug, Clone, Copy)]
pub struct NonSuccessReport<'a> {
	/// Task directory relative to repository root.
	pub relative_cwd: &'a str,
	/// Failed or interrupted command text.
	pub command: &'a str,
	/// Final task outcome.
	pub outcome: TaskOutcome,
	/// Exit code when available.
	pub exit_code: Option<i32>,
}

/// Logical task result for task-block rendering.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TaskOutcome {
	/// The task succeeded.
	Success,
	/// The task failed with a non-zero exit code.
	Failed,
	/// The task terminated without an exit code.
	Interrupted,
	/// The task failed before process spawn.
	SpawnError,
}

/// Display payload for one command rendered within a task block.
#[derive(Debug, Clone, Copy)]
pub struct TaskCommand<'a> {
	/// Display command.
	pub command: &'a str,
	/// Captured stdout text.
	pub stdout: &'a str,
	/// Captured stderr text.
	pub stderr: &'a str,
}

/// Display payload for one task block.
#[derive(Debug, Clone, Copy)]
pub struct TaskBlock<'a> {
	/// Task directory relative to repository root.
	pub relative_cwd: &'a str,
	/// Commands executed in this task directory, in invocation order.
	pub commands: &'a [TaskCommand<'a>],
	/// Final task status.
	pub outcome: TaskOutcome,
}

/// Unified non-debug renderer.
#[derive(Debug, Clone)]
pub struct Renderer {
	tokens: StyleTokens,
	section_started: Cell<bool>,
}

impl Renderer {
	/// Construct a renderer for the configured mode.
	#[must_use]
	pub fn new(requested_mode: RenderMode) -> Self {
		let effective_mode = requested_mode.effective();
		let tokens = if effective_mode.use_style() {
			StyleTokens::styled()
		} else {
			StyleTokens::plain()
		};

		Self {
			tokens,
			section_started: Cell::new(false),
		}
	}

	/// Render the prepare stage.
	pub fn render_prepare_stage(&self, pattern: &str) {
		self.start_section("Prepare");
		self.print_key_value("pattern", pattern);
	}

	/// Render changed-file discovery counters.
	pub fn render_discovery_stage(&self, changed_files: usize, matched_files: usize) {
		self.start_section("Discovery");
		self.print_key_value("changed", changed_files);
		self.print_key_value("matched", matched_files);
	}

	/// Render an optional user message stage.
	pub fn render_message_stage(&self, message: &str) {
		self.start_section("Message");
		self.print_key_value("message", message);
	}

	/// Render no-match completion stage.
	pub fn render_no_match_stage(&self) {
		self.start_section("Result");
		println!("{} no matching files found", self.tokens.warn_badge);
	}

	/// Render the dry-run stage header.
	pub fn render_dry_run_stage(&self) {
		self.start_section("Dry Run");
	}

	/// Render one dry-run command block.
	pub fn render_dry_run_block(&self, relative_cwd: &str, command: &str) {
		self.print_directory(relative_cwd);
		self.print_command(command);
		println!("{} planned only", self.tokens.warn_badge);
	}

	/// Render task stage header.
	pub fn render_task_stage(&self) {
		self.start_section("Tasks");
	}

	/// Render one task block.
	pub fn render_task_block(&self, block: TaskBlock<'_>) {
		self.print_directory(block.relative_cwd);

		for command in block.commands {
			self.print_command(command.command);

			if !command.stdout.is_empty() {
				print!("{}", command.stdout);
			}
			if !command.stderr.is_empty() {
				eprint!("{}", command.stderr);
			}
		}

		let (badge, status) = match block.outcome {
			TaskOutcome::Success => (self.tokens.success_badge, "success"),
			TaskOutcome::Failed => (self.tokens.error_badge, "failed"),
			TaskOutcome::Interrupted => (self.tokens.warn_badge, "interrupted"),
			TaskOutcome::SpawnError => (self.tokens.error_badge, "spawn_error"),
		};
		println!("{badge} {status}");
	}

	/// Render final summary stage.
	pub fn render_summary_stage(&self, summary: Summary) {
		self.start_section("Summary");
		self.print_key_value("matched files", summary.matched_files);
		self.print_key_value("task dirs", summary.task_dirs);
		self.print_key_value("passed", summary.passed);
		self.print_key_value("failed", summary.failed);
		self.print_key_value("interrupted", summary.interrupted);

		if summary.failed == 0 && summary.interrupted == 0 {
			println!("{} all tasks passed", self.tokens.success_badge);
		} else if summary.failed > 0 && summary.interrupted == 0 {
			println!("{} {} task(s) failed", self.tokens.error_badge, summary.failed);
		} else if summary.failed == 0 {
			println!("{} {} task(s) interrupted", self.tokens.warn_badge, summary.interrupted);
		} else {
			println!(
				"{} {} task(s) failed, {} interrupted",
				self.tokens.error_badge, summary.failed, summary.interrupted
			);
		}
	}

	/// Render final dry-run summary stage.
	pub fn render_dry_run_summary_stage(&self, summary: DryRunSummary) {
		self.start_section("Summary");
		self.print_key_value("matched files", summary.matched_files);
		self.print_key_value("task dirs", summary.task_dirs);
		self.print_key_value("planned commands", summary.planned_commands);
		self.print_key_value("executed commands", 0);
		println!(
			"{} dry run only: {} command(s) planned, 0 executed",
			self.tokens.warn_badge, summary.planned_commands
		);
	}

	/// Render deterministic non-success details for one task.
	pub fn render_non_success_report(&self, report: NonSuccessReport<'_>) {
		let (badge, headline, status) = match report.outcome {
			TaskOutcome::Success => return,
			TaskOutcome::Failed => (
				self.tokens.error_badge,
				"task failed",
				report.exit_code.map_or_else(
					|| "exit code unavailable".to_owned(),
					|code| format!("exit code {code}"),
				),
			),
			TaskOutcome::Interrupted => (
				self.tokens.warn_badge,
				"task interrupted",
				"signal termination (no exit code)".to_owned(),
			),
			TaskOutcome::SpawnError => (
				self.tokens.error_badge,
				"task failed to start",
				"spawn error".to_owned(),
			),
		};

		eprintln!("{badge} {headline}");
		eprintln!(
			"{}cwd{}: {}",
			self.tokens.label_prefix, self.tokens.label_suffix, report.relative_cwd
		);
		eprintln!(
			"{}command{}: {}",
			self.tokens.label_prefix, self.tokens.label_suffix, report.command
		);
		eprintln!(
			"{}status{}: {status}",
			self.tokens.label_prefix, self.tokens.label_suffix
		);
	}

	fn start_section(&self, title: &str) {
		if self.section_started.replace(true) {
			println!();
		}
		println!("{}{}{}", self.tokens.heading_prefix, title, self.tokens.heading_suffix);
	}

	fn print_key_value(&self, label: &str, value: impl Display) {
		println!(
			"{}{}{}: {value}",
			self.tokens.label_prefix, label, self.tokens.label_suffix
		);
	}

	fn print_directory(&self, relative_cwd: &str) {
		println!(
			"{}directory{}: {relative_cwd}",
			self.tokens.dir_label_prefix, self.tokens.dir_label_suffix
		);
	}

	fn print_command(&self, command: &str) {
		println!(
			"{}command{}: {command}",
			self.tokens.cmd_label_prefix, self.tokens.cmd_label_suffix
		);
	}
}

#[derive(Debug, Clone, Copy)]
struct StyleTokens {
	heading_prefix: &'static str,
	heading_suffix: &'static str,
	label_prefix: &'static str,
	label_suffix: &'static str,
	dir_label_prefix: &'static str,
	dir_label_suffix: &'static str,
	cmd_label_prefix: &'static str,
	cmd_label_suffix: &'static str,
	success_badge: &'static str,
	warn_badge: &'static str,
	error_badge: &'static str,
}

impl StyleTokens {
	const fn plain() -> Self {
		Self {
			heading_prefix: "",
			heading_suffix: "",
			label_prefix: "",
			label_suffix: "",
			dir_label_prefix: "",
			dir_label_suffix: "",
			cmd_label_prefix: "",
			cmd_label_suffix: "",
			success_badge: "[ok]",
			warn_badge: "[warn]",
			error_badge: "[error]",
		}
	}

	const fn styled() -> Self {
		Self {
			heading_prefix: "\x1b[1;36m",
			heading_suffix: "\x1b[0m",
			label_prefix: "\x1b[1m",
			label_suffix: "\x1b[0m",
			dir_label_prefix: "\x1b[1;35m",
			dir_label_suffix: "\x1b[0m",
			cmd_label_prefix: "\x1b[1;34m",
			cmd_label_suffix: "\x1b[0m",
			success_badge: "\x1b[1;32m[ok]\x1b[0m",
			warn_badge: "\x1b[1;33m[warn]\x1b[0m",
			error_badge: "\x1b[1;31m[error]\x1b[0m",
		}
	}
}
