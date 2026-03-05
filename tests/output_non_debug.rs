//! Integration tests for non-debug output contract lines, ordering, and stream routing.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Output};

use tempfile::TempDir;

#[test]
fn non_debug_no_match_contract_lines_and_streams() {
	let temp = setup_repo_with_two_changed_lockfiles();
	let output = run_pullhook(
		temp.path(),
		&["--pattern", "**/*.md", "--command", "sh -c 'echo should-not-run'"],
	);

	assert!(output.status.success(), "no-match run should succeed");

	let stdout = stdout_text(&output);
	let stderr = stderr_text(&output);

	assert_in_order(
		&stdout,
		&[
			"Prepare",
			"pattern: **/*.md",
			"Discovery",
			"changed: 2",
			"matched: 0",
			"Result",
			"[warn] no matching files found",
		],
	);
	assert!(stderr.is_empty(), "no-match stderr should be empty:\n{stderr}");
}

#[test]
fn non_debug_single_match_routes_stdout_and_stderr_separately() {
	let temp = setup_repo_with_two_changed_lockfiles();
	let command = "sh -c 'echo single-stdout; echo single-stderr >&2'";
	let output = run_pullhook(
		temp.path(),
		&["--pattern", "packages/a/package-lock.json", "--command", command],
	);

	assert!(output.status.success(), "single-match run should succeed");

	let stdout = stdout_text(&output);
	let stderr = stderr_text(&output);
	let command_line = format!("command: {command}");

	assert_in_order(
		&stdout,
		&[
			"Prepare",
			"pattern: packages/a/package-lock.json",
			"Discovery",
			"changed: 2",
			"matched: 1",
			"Tasks",
			"directory: packages/a",
			command_line.as_str(),
			"single-stdout",
			"[ok] success",
			"Summary",
			"matched files: 1",
			"task dirs: 1",
			"passed: 1",
			"failed: 0",
			"interrupted: 0",
			"[ok] all tasks passed",
		],
	);
	assert_eq!(
		non_empty_lines(&stderr),
		vec!["single-stderr"],
		"single-match stderr should only contain command stderr"
	);
	assert!(
		count_exact_lines(&stdout, "single-stderr") == 0,
		"stderr content must not appear in stdout:\n{stdout}"
	);
	assert!(
		count_exact_lines(&stderr, "single-stdout") == 0,
		"stdout content must not appear in stderr:\n{stderr}"
	);
}

#[test]
fn non_debug_multi_match_preserves_task_order() {
	let temp = setup_repo_with_two_changed_lockfiles();
	let command = "sh -c 'echo multi-stdout; echo multi-stderr >&2'";
	let output = run_pullhook(
		temp.path(),
		&["--pattern", "packages/*/package-lock.json", "--command", command],
	);

	assert!(output.status.success(), "multi-match run should succeed");

	let stdout = stdout_text(&output);
	let stderr = stderr_text(&output);
	let command_line = format!("command: {command}");

	assert_in_order(
		&stdout,
		&[
			"Prepare",
			"pattern: packages/*/package-lock.json",
			"Discovery",
			"changed: 2",
			"matched: 2",
			"Tasks",
			"directory: packages/a",
			command_line.as_str(),
			"multi-stdout",
			"[ok] success",
			"directory: packages/b",
			command_line.as_str(),
			"multi-stdout",
			"[ok] success",
			"Summary",
			"matched files: 2",
			"task dirs: 2",
			"passed: 2",
			"failed: 0",
			"interrupted: 0",
			"[ok] all tasks passed",
		],
	);

	let a_idx = stdout.find("directory: packages/a").expect("packages/a block missing");
	let b_idx = stdout.find("directory: packages/b").expect("packages/b block missing");
	assert!(a_idx < b_idx, "task blocks should be ordered a -> b:\n{stdout}");
	assert_eq!(
		count_occurrences(&stderr, "multi-stderr"),
		2,
		"expected two stderr lines (one per task):\n{stderr}"
	);
}

#[test]
fn non_debug_once_collapses_to_single_root_task() {
	let temp = setup_repo_with_two_changed_lockfiles();
	let command = "sh -c 'echo once-stdout; echo once-stderr >&2'";
	let output = run_pullhook(
		temp.path(),
		&[
			"--pattern",
			"packages/*/package-lock.json",
			"--command",
			command,
			"--once",
		],
	);

	assert!(output.status.success(), "--once run should succeed");

	let stdout = stdout_text(&output);
	let stderr = stderr_text(&output);
	let command_line = format!("command: {command}");

	assert_in_order(
		&stdout,
		&[
			"Prepare",
			"pattern: packages/*/package-lock.json",
			"Discovery",
			"changed: 2",
			"matched: 2",
			"Tasks",
			"directory: .",
			command_line.as_str(),
			"once-stdout",
			"[ok] success",
			"Summary",
			"matched files: 2",
			"task dirs: 1",
			"passed: 1",
			"failed: 0",
			"interrupted: 0",
			"[ok] all tasks passed",
		],
	);
	assert_eq!(
		count_occurrences(&stdout, "directory: "),
		1,
		"--once should print exactly one task directory:\n{stdout}"
	);
	assert!(
		!stdout.contains("directory: packages/"),
		"--once should not include per-package directories:\n{stdout}"
	);
	assert_eq!(
		non_empty_lines(&stderr),
		vec!["once-stderr"],
		"--once stderr should only contain command stderr"
	);
}

#[test]
fn non_debug_dry_run_prints_planned_blocks_only() {
	let temp = setup_repo_with_two_changed_lockfiles();
	let command = "sh -c 'echo ran > .pullhook-dry-run-marker'";
	let output = run_pullhook(
		temp.path(),
		&[
			"--pattern",
			"packages/*/package-lock.json",
			"--command",
			command,
			"--dry-run",
		],
	);

	assert!(output.status.success(), "--dry-run run should succeed");

	let stdout = stdout_text(&output);
	let stderr = stderr_text(&output);
	let command_line = format!("command: {command}");

	assert_in_order(
		&stdout,
		&[
			"Prepare",
			"pattern: packages/*/package-lock.json",
			"Discovery",
			"changed: 2",
			"matched: 2",
			"Dry Run",
			"directory: packages/a",
			command_line.as_str(),
			"[warn] planned only",
			"directory: packages/b",
			command_line.as_str(),
			"[warn] planned only",
			"Summary",
			"matched files: 2",
			"task dirs: 2",
			"planned commands: 2",
			"executed commands: 0",
			"[warn] dry run only: 2 command(s) planned",
		],
	);
	assert_eq!(
		count_occurrences(&stdout, "[warn] planned only"),
		2,
		"--dry-run should print one planned marker per task block"
	);
	assert!(stderr.is_empty(), "--dry-run stderr should be empty:\n{stderr}");
	assert!(
		!temp.path().join("packages/a/.pullhook-dry-run-marker").exists(),
		"--dry-run should not execute command in packages/a"
	);
	assert!(
		!temp.path().join("packages/b/.pullhook-dry-run-marker").exists(),
		"--dry-run should not execute command in packages/b"
	);
}

#[test]
fn non_debug_failure_reports_task_failure_on_stderr() {
	let temp = setup_repo_with_two_changed_lockfiles();
	let command = "sh -c 'echo fail-stdout; echo fail-stderr >&2; exit 7'";
	let output = run_pullhook(
		temp.path(),
		&["--pattern", "packages/a/package-lock.json", "--command", command],
	);

	assert!(!output.status.success(), "failure run should exit non-zero");

	let stdout = stdout_text(&output);
	let stderr = stderr_text(&output);
	let command_line = format!("command: {command}");

	assert_in_order(
		&stdout,
		&[
			"Prepare",
			"pattern: packages/a/package-lock.json",
			"Discovery",
			"changed: 2",
			"matched: 1",
			"Tasks",
			"directory: packages/a",
			command_line.as_str(),
			"fail-stdout",
			"[error] failed",
			"Summary",
			"matched files: 1",
			"task dirs: 1",
			"passed: 0",
			"failed: 1",
			"interrupted: 0",
			"[error] 1 task(s) failed",
		],
	);
	assert_in_order(
		&stderr,
		&[
			"fail-stderr",
			"[error] task failed",
			"cwd: packages/a",
			command_line.as_str(),
			"status: exit code 7",
			"error: 1 task(s) failed",
		],
	);
	assert!(
		count_exact_lines(&stdout, "fail-stderr") == 0,
		"command stderr must not be routed to stdout:\n{stdout}"
	);
}

#[test]
fn non_debug_interrupted_reports_interrupted_state() {
	let temp = setup_repo_with_two_changed_lockfiles();
	let command = "sh -c 'kill -TERM $$'";
	let output = run_pullhook(
		temp.path(),
		&["--pattern", "packages/a/package-lock.json", "--command", command],
	);

	assert!(!output.status.success(), "interrupted run should exit non-zero");

	let stdout = stdout_text(&output);
	let stderr = stderr_text(&output);
	let command_line = format!("command: {command}");

	assert_in_order(
		&stdout,
		&[
			"Prepare",
			"pattern: packages/a/package-lock.json",
			"Discovery",
			"changed: 2",
			"matched: 1",
			"Tasks",
			"directory: packages/a",
			command_line.as_str(),
			"[warn] interrupted",
			"Summary",
			"matched files: 1",
			"task dirs: 1",
			"passed: 0",
			"failed: 0",
			"interrupted: 1",
			"[warn] 1 task(s) interrupted",
		],
	);
	assert_in_order(
		&stderr,
		&[
			"[warn] task interrupted",
			"cwd: packages/a",
			command_line.as_str(),
			"status: signal termination (no exit code)",
			"error: 1 task(s) failed",
		],
	);
}

#[test]
fn non_debug_spawn_error_reports_spawn_error_state() {
	let temp = setup_repo_with_two_changed_lockfiles();
	let command = "definitely-not-a-real-command-pullhook";
	let output = run_pullhook(
		temp.path(),
		&["--pattern", "packages/a/package-lock.json", "--command", command],
	);

	assert!(!output.status.success(), "spawn-error run should exit non-zero");

	let stdout = stdout_text(&output);
	let stderr = stderr_text(&output);
	let command_line = format!("command: {command}");

	assert_in_order(
		&stdout,
		&[
			"Prepare",
			"pattern: packages/a/package-lock.json",
			"Discovery",
			"changed: 2",
			"matched: 1",
			"Tasks",
			"directory: packages/a",
			command_line.as_str(),
			"[error] spawn_error",
			"Summary",
			"matched files: 1",
			"task dirs: 1",
			"passed: 0",
			"failed: 1",
			"interrupted: 0",
			"[error] 1 task(s) failed",
		],
	);
	assert_in_order(
		&stderr,
		&[
			"[error] task failed to start",
			"cwd: packages/a",
			command_line.as_str(),
			"status: spawn error",
			"error: 1 task(s) failed",
		],
	);
}

#[test]
fn non_debug_non_utf8_output_is_lossy_decoded_without_panic() {
	let temp = setup_repo_with_two_changed_lockfiles();
	let command = "sh -c 'printf \"\\377\\376nonutf-stdout\\n\"; printf \"\\377\\376nonutf-stderr\\n\" >&2'";
	let output = run_pullhook(
		temp.path(),
		&["--pattern", "packages/a/package-lock.json", "--command", command],
	);

	assert!(output.status.success(), "non-UTF8 run should still succeed");

	let stdout = stdout_text(&output);
	let stderr = stderr_text(&output);

	assert_in_order(
		&stdout,
		&[
			"Prepare",
			"pattern: packages/a/package-lock.json",
			"Discovery",
			"changed: 2",
			"matched: 1",
			"Tasks",
			"directory: packages/a",
			"nonutf-stdout",
			"[ok] success",
			"Summary",
			"matched files: 1",
			"task dirs: 1",
			"passed: 1",
			"failed: 0",
			"interrupted: 0",
			"[ok] all tasks passed",
		],
	);
	assert!(
		stdout.contains('\u{FFFD}'),
		"stdout should contain lossy replacement chars:\n{stdout}"
	);
	assert!(
		stderr.contains("nonutf-stderr"),
		"stderr should contain non-UTF8 payload text:\n{stderr}"
	);
	assert!(
		stderr.contains('\u{FFFD}'),
		"stderr should contain lossy replacement chars:\n{stderr}"
	);
}

#[test]
fn non_debug_empty_output_non_zero_still_reports_deterministic_failure_lines() {
	let temp = setup_repo_with_two_changed_lockfiles();
	let command = "sh -c 'exit 9'";
	let output = run_pullhook(
		temp.path(),
		&["--pattern", "packages/a/package-lock.json", "--command", command],
	);

	assert!(
		!output.status.success(),
		"empty-output non-zero run should exit non-zero"
	);

	let stdout = stdout_text(&output);
	let stderr = stderr_text(&output);
	let command_line = format!("command: {command}");

	assert_in_order(
		&stdout,
		&[
			"Prepare",
			"pattern: packages/a/package-lock.json",
			"Discovery",
			"changed: 2",
			"matched: 1",
			"Tasks",
			"directory: packages/a",
			command_line.as_str(),
			"[error] failed",
			"Summary",
			"matched files: 1",
			"task dirs: 1",
			"passed: 0",
			"failed: 1",
			"interrupted: 0",
			"[error] 1 task(s) failed",
		],
	);
	assert_in_order(
		&stderr,
		&[
			"[error] task failed",
			"cwd: packages/a",
			command_line.as_str(),
			"status: exit code 9",
			"error: 1 task(s) failed",
		],
	);
}

fn run_pullhook(repo_root: &Path, args: &[&str]) -> Output {
	ProcessCommand::new(assert_cmd::cargo::cargo_bin!("pullhook"))
		.current_dir(repo_root)
		.args(["--render", "never"])
		.args(args)
		.output()
		.expect("run pullhook")
}

fn stdout_text(output: &Output) -> String {
	String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_text(output: &Output) -> String {
	String::from_utf8_lossy(&output.stderr).to_string()
}

fn non_empty_lines(text: &str) -> Vec<&str> {
	text.lines().filter(|line| !line.trim().is_empty()).collect()
}

fn count_occurrences(haystack: &str, needle: &str) -> usize {
	haystack.match_indices(needle).count()
}

fn count_exact_lines(haystack: &str, needle: &str) -> usize {
	haystack.lines().filter(|line| *line == needle).count()
}

fn assert_in_order(haystack: &str, ordered_fragments: &[&str]) {
	let mut offset = 0usize;

	for fragment in ordered_fragments {
		let remaining = &haystack[offset..];
		let relative = remaining
			.find(fragment)
			.unwrap_or_else(|| panic!("missing fragment `{fragment}` in output:\n{haystack}"));
		offset = offset.saturating_add(relative + fragment.len());
	}
}

fn setup_repo_with_two_changed_lockfiles() -> TempDir {
	let temp = tempfile::tempdir().expect("create temp dir");
	let repo_root = temp.path();

	run_git(repo_root, &["init"]);
	run_git(repo_root, &["config", "user.email", "pullhook@example.com"]);
	run_git(repo_root, &["config", "user.name", "Pullhook Test"]);

	write_file(
		repo_root,
		Path::new("packages/a/package-lock.json"),
		"{\"name\":\"a\",\"version\":1}\n",
	);
	write_file(
		repo_root,
		Path::new("packages/b/package-lock.json"),
		"{\"name\":\"b\",\"version\":1}\n",
	);

	run_git(repo_root, &["add", "."]);
	run_git(repo_root, &["commit", "-m", "initial"]);

	write_file(
		repo_root,
		Path::new("packages/a/package-lock.json"),
		"{\"name\":\"a\",\"version\":2}\n",
	);
	write_file(
		repo_root,
		Path::new("packages/b/package-lock.json"),
		"{\"name\":\"b\",\"version\":2}\n",
	);

	run_git(repo_root, &["add", "."]);
	run_git(repo_root, &["commit", "-m", "update locks"]);

	temp
}

fn write_file(repo_root: &Path, relative_path: &Path, contents: &str) {
	let path: PathBuf = repo_root.join(relative_path);
	if let Some(parent) = path.parent() {
		fs::create_dir_all(parent).expect("create parent directories");
	}

	fs::write(path, contents).expect("write file");
}

fn run_git(repo_root: &Path, args: &[&str]) {
	let status = ProcessCommand::new("git")
		.current_dir(repo_root)
		.args(args)
		.status()
		.expect("run git command");

	assert!(status.success(), "git command failed: git {}", args.join(" "));
}
