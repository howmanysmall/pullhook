//! Unified non-debug output rendering and style controls.

use std::cell::Cell;
use std::env;
use std::fmt::Display;
use std::io::{self, IsTerminal};

use clap::ValueEnum;

use crate::runner::{InvocationOutput, ResultState};

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

	fn use_style(self) -> bool {
		match self {
			Self::Auto => {
				if env::var_os("NO_COLOR").is_some() || env_var_is_zero("CLICOLOR") {
					return false;
				}
				if env_var_is_set_and_non_zero("CLICOLOR_FORCE") {
					return true;
				}
				if env::var_os("TERM").is_some_and(|value| value == "dumb") {
					return false;
				}

				io::stdout().is_terminal() || io::stderr().is_terminal()
			}
			Self::Always => true,
			Self::Never => false,
		}
	}

	fn use_compact_layout(self) -> bool {
		match self {
			Self::Auto => io::stdout().is_terminal(),
			Self::Always => true,
			Self::Never => false,
		}
	}
}

fn env_var_is_zero(name: &str) -> bool {
	env::var_os(name).is_some_and(|value| value.to_string_lossy() == "0")
}

fn env_var_is_set_and_non_zero(name: &str) -> bool {
	env::var_os(name).is_some_and(|value| value.to_string_lossy() != "0")
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
	pub outcome: ResultState,
	/// Exit code when available.
	pub exit_code: Option<i32>,
}

/// Display payload for one task block.
#[derive(Debug, Clone, Copy)]
pub struct TaskBlock<'a> {
	/// Task directory relative to repository root.
	pub relative_cwd: &'a str,
	/// Commands executed in this task directory, in invocation order.
	pub commands: &'a [InvocationOutput],
	/// Final task status.
	pub outcome: ResultState,
}

/// Unified renderer.
#[derive(Debug, Clone)]
pub struct Renderer {
	tokens: StyleTokens,
	section_started: Cell<bool>,
	compact: bool,
}

impl Renderer {
	/// Construct a renderer for the configured mode.
	#[must_use]
	pub fn new(requested_mode: RenderMode) -> Self {
		let effective_mode = requested_mode.effective();
		let styled = effective_mode.use_style();
		let compact = effective_mode.use_compact_layout();
		let tokens = if styled {
			StyleTokens::styled()
		} else {
			StyleTokens::plain()
		};

		Self {
			tokens,
			section_started: Cell::new(false),
			compact,
		}
	}

	/// Render the prepare stage.
	pub fn render_prepare_stage(&self, pattern: &str) {
		if self.compact {
			let _ = pattern;
			println!(
				"{}{}{} {}pullhook{} ready",
				self.tokens.success_prefix,
				self.tokens.success_symbol,
				self.tokens.success_suffix,
				self.tokens.bold_prefix,
				self.tokens.bold_suffix
			);
			return;
		}

		self.start_section("Prepare");
		self.print_key_value("pattern", pattern);
	}

	/// Render changed-file discovery counters.
	pub fn render_discovery_stage(&self, changed_files: usize, matched_files: usize) {
		if self.compact {
			if matched_files > 0 {
				println!(
					"  {}{}{} Found {}{}{} relevant change(s) {}(changed: {}, matched: {}){}",
					self.tokens.info_prefix,
					self.tokens.info_symbol,
					self.tokens.info_suffix,
					self.tokens.bold_prefix,
					matched_files,
					self.tokens.bold_suffix,
					self.tokens.dim_prefix,
					changed_files,
					matched_files,
					self.tokens.dim_suffix
				);
			}
			return;
		}

		self.start_section("Discovery");
		self.print_key_value("changed", changed_files);
		self.print_key_value("matched", matched_files);
	}

	/// Render an optional user message stage.
	pub fn render_message_stage(&self, message: &str) {
		if self.compact {
			println!(
				"  {}{}{} {}{}{}",
				self.tokens.info_prefix,
				self.tokens.info_symbol,
				self.tokens.info_suffix,
				self.tokens.info_prefix,
				message,
				self.tokens.info_suffix
			);
			return;
		}

		self.start_section("Message");
		self.print_key_value("message", message);
	}

	/// Render no-match completion stage.
	pub fn render_no_match_stage(&self, pattern: &str, changed_files: usize, matched_files: usize) {
		if self.compact {
			println!(
				"  {}{}{} No relevant changes for {}{}{} {}(changed: {}, matched: {}){}",
				self.tokens.info_prefix,
				self.tokens.info_symbol,
				self.tokens.info_suffix,
				self.tokens.bold_prefix,
				pattern,
				self.tokens.bold_suffix,
				self.tokens.dim_prefix,
				changed_files,
				matched_files,
				self.tokens.dim_suffix
			);
			return;
		}

		self.start_section("Result");
		println!("{} no matching files found", self.tokens.warn_badge);
	}

	/// Render the dry-run stage header.
	pub fn render_dry_run_stage(&self) {
		if self.compact {
			println!(
				"  {}{}{} Dry run mode",
				self.tokens.warn_prefix, self.tokens.warn_symbol, self.tokens.warn_suffix
			);
			return;
		}

		self.start_section("Dry Run");
	}

	/// Render one dry-run command block.
	pub fn render_dry_run_block(&self, relative_cwd: &str, command: &str) {
		if self.compact {
			println!(
				"  {}{}{} {}{}{}",
				self.tokens.info_prefix,
				self.tokens.info_symbol,
				self.tokens.info_suffix,
				self.tokens.bold_prefix,
				relative_cwd,
				self.tokens.bold_suffix
			);
			println!("    {}{}{}", self.tokens.dim_prefix, command, self.tokens.dim_suffix);
			return;
		}

		self.print_directory(relative_cwd);
		self.print_command(command);
		println!("{} planned only", self.tokens.warn_badge);
	}

	/// Render task stage header.
	pub fn render_task_stage(&self) {
		if self.compact {
			return;
		}

		self.start_section("Tasks");
	}

	/// Render one task block.
	pub fn render_task_block(&self, block: TaskBlock<'_>) {
		if self.compact {
			let (prefix, symbol, suffix) = match block.outcome {
				ResultState::Success => (
					self.tokens.success_prefix,
					self.tokens.success_symbol,
					self.tokens.success_suffix,
				),
				ResultState::Failed | ResultState::SpawnError => (
					self.tokens.error_prefix,
					self.tokens.error_symbol,
					self.tokens.error_suffix,
				),
				ResultState::Interrupted => (
					self.tokens.warn_prefix,
					self.tokens.warn_symbol,
					self.tokens.warn_suffix,
				),
			};

			println!(
				"  {prefix}{symbol}{suffix} {}{}{}",
				self.tokens.bold_prefix, block.relative_cwd, self.tokens.bold_suffix
			);

			for command in block.commands {
				println!(
					"    {}{}{}",
					self.tokens.dim_prefix, command.command, self.tokens.dim_suffix
				);
				Self::print_indented_stdout(&command.stdout);
				Self::print_indented_stderr(&command.stderr);
			}

			return;
		}

		self.print_directory(block.relative_cwd);

		for command in block.commands {
			self.print_command(&command.command);

			if !command.stdout.is_empty() {
				print!("{}", command.stdout);
			}
			if !command.stderr.is_empty() {
				eprint!("{}", command.stderr);
			}
		}

		let (badge, status) = match block.outcome {
			ResultState::Success => (self.tokens.success_badge, "success"),
			ResultState::Failed => (self.tokens.error_badge, "failed"),
			ResultState::Interrupted => (self.tokens.warn_badge, "interrupted"),
			ResultState::SpawnError => (self.tokens.error_badge, "spawn_error"),
		};
		println!("{badge} {status}");
	}

	/// Render final summary stage.
	pub fn render_summary_stage(&self, summary: Summary) {
		if self.compact {
			if summary.task_dirs == 0 {
				return;
			}

			println!();
			if summary.failed == 0 && summary.interrupted == 0 {
				println!(
					"  {}{}{} {}{}{} task(s) completed successfully",
					self.tokens.success_prefix,
					self.tokens.success_symbol,
					self.tokens.success_suffix,
					self.tokens.success_prefix,
					summary.passed,
					self.tokens.success_suffix
				);
			} else if summary.failed == 0 {
				println!(
					"  {}{}{} {}{}{} task(s) interrupted",
					self.tokens.warn_prefix,
					self.tokens.warn_symbol,
					self.tokens.warn_suffix,
					self.tokens.warn_prefix,
					summary.interrupted,
					self.tokens.warn_suffix
				);
			} else {
				println!(
					"  {}{}{} {}{}{} task(s) failed, {} interrupted",
					self.tokens.error_prefix,
					self.tokens.error_symbol,
					self.tokens.error_suffix,
					self.tokens.error_prefix,
					summary.failed,
					self.tokens.error_suffix,
					summary.interrupted
				);
			}
			return;
		}

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
		if self.compact {
			println!(
				"\n  {}{}{} {}{}{} planned command(s) across {} task(s)",
				self.tokens.warn_prefix,
				self.tokens.warn_symbol,
				self.tokens.warn_suffix,
				self.tokens.warn_prefix,
				summary.planned_commands,
				self.tokens.warn_suffix,
				summary.task_dirs
			);
			return;
		}

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
			ResultState::Success => return,
			ResultState::Failed => (
				self.tokens.error_badge,
				"task failed",
				report.exit_code.map_or_else(
					|| "exit code unavailable".to_owned(),
					|code| format!("exit code {code}"),
				),
			),
			ResultState::Interrupted => (
				self.tokens.warn_badge,
				"task interrupted",
				"signal termination (no exit code)".to_owned(),
			),
			ResultState::SpawnError => (
				self.tokens.error_badge,
				"task failed to start",
				"spawn error".to_owned(),
			),
		};

		if self.compact {
			eprintln!(
				"  {}{}{} {headline}",
				self.tokens.error_prefix, self.tokens.error_symbol, self.tokens.error_suffix
			);
			eprintln!(
				"    {}cwd:{} {}",
				self.tokens.dim_prefix, self.tokens.dim_suffix, report.relative_cwd
			);
			eprintln!(
				"    {}command:{} {}",
				self.tokens.dim_prefix, self.tokens.dim_suffix, report.command
			);
			eprintln!(
				"    {}status:{} {status}",
				self.tokens.dim_prefix, self.tokens.dim_suffix
			);
			return;
		}

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

	fn print_indented_stdout(text: &str) {
		for line in text.lines() {
			println!("    │ {line}");
		}
	}

	fn print_indented_stderr(text: &str) {
		for line in text.lines() {
			eprintln!("    │ {line}");
		}
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
	success_symbol: &'static str,
	warn_symbol: &'static str,
	error_symbol: &'static str,
	info_symbol: &'static str,
	bold_prefix: &'static str,
	bold_suffix: &'static str,
	dim_prefix: &'static str,
	dim_suffix: &'static str,
	success_prefix: &'static str,
	success_suffix: &'static str,
	warn_prefix: &'static str,
	warn_suffix: &'static str,
	error_prefix: &'static str,
	error_suffix: &'static str,
	info_prefix: &'static str,
	info_suffix: &'static str,
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
			success_symbol: "[ok]",
			warn_symbol: "[warn]",
			error_symbol: "[error]",
			info_symbol: ">",
			bold_prefix: "",
			bold_suffix: "",
			dim_prefix: "",
			dim_suffix: "",
			success_prefix: "",
			success_suffix: "",
			warn_prefix: "",
			warn_suffix: "",
			error_prefix: "",
			error_suffix: "",
			info_prefix: "",
			info_suffix: "",
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
			success_symbol: "✔",
			warn_symbol: "⚠",
			error_symbol: "✖",
			info_symbol: "›",
			bold_prefix: "\x1b[1m",
			bold_suffix: "\x1b[0m",
			dim_prefix: "\x1b[2m",
			dim_suffix: "\x1b[0m",
			success_prefix: "\x1b[1;32m",
			success_suffix: "\x1b[0m",
			warn_prefix: "\x1b[1;33m",
			warn_suffix: "\x1b[0m",
			error_prefix: "\x1b[1;31m",
			error_suffix: "\x1b[0m",
			info_prefix: "\x1b[1;36m",
			info_suffix: "\x1b[0m",
		}
	}
}
