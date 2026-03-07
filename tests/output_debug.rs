//! Debug-mode output tests for streamed diagnostics behavior.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use tempfile::TempDir;

#[test]
fn debug_mode_streams_outputs_and_keeps_renderer_output() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(
		repo_root,
		&[
			"--debug",
			"--pattern",
			"packages/*/package-lock.json",
			"--command",
			"sh -c 'echo debug-stream-stdout; echo debug-stream-stderr >&2'",
		],
	);

	assert!(output.status.success(), "debug run should succeed");

	let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
	let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

	assert_eq!(
		count_exact_lines(&stdout, "debug-stream-stdout"),
		2,
		"stdout should be streamed once per matched task",
	);
	assert_eq!(
		count_exact_lines(&stderr, "debug-stream-stderr"),
		2,
		"stderr should be streamed once per matched task",
	);
	assert_eq!(
		count_occurrences(&stdout, "loaded changed files"),
		1,
		"changed-files diagnostic should be printed once",
	);
	assert_eq!(
		count_occurrences(&stdout, "matched changed files"),
		1,
		"matched-files diagnostic should be printed once",
	);
	assert_eq!(
		count_occurrences(&stdout, "running invocation"),
		2,
		"invocation diagnostic should be printed once per task",
	);

	assert_includes_renderer_contract_text(&stdout);
}

#[test]
fn debug_mode_failure_reports_once_with_renderer_failure_copy() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(
		repo_root,
		&[
			"--debug",
			"--once",
			"--pattern",
			"packages/*/package-lock.json",
			"--command",
			"sh -c 'echo debug-failure-stderr >&2; exit 7'",
		],
	);

	assert!(!output.status.success(), "failing debug run should return non-zero");

	let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
	let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

	assert_eq!(
		count_exact_lines(&stderr, "debug-failure-stderr"),
		1,
		"failing command stderr should remain streamed once",
	);
	assert_eq!(
		count_occurrences(&stdout, "running invocation"),
		1,
		"single --once invocation should emit one invocation diagnostic",
	);
	assert_eq!(
		count_occurrences(&stderr, "error in "),
		1,
		"debug failure line should be emitted once",
	);
	assert_eq!(
		count_occurrences(&stderr, "error: 1 task(s) failed"),
		1,
		"top-level failure summary should be emitted once",
	);

	assert!(
		stderr.contains("cwd:"),
		"renderer failure detail labels should appear in debug mode",
	);
	assert!(
		stderr.contains("task failed")
			|| stderr.contains("task interrupted")
			|| stderr.contains("task failed to start"),
		"stderr should include renderer failure report headline for non-success runs",
	);

	assert_includes_renderer_contract_text(&stdout);
}

fn assert_includes_renderer_contract_text(stdout: &str) {
	for marker in ["Prepare", "Discovery", "Summary", "directory:", "command:"] {
		assert!(
			stdout.contains(marker),
			"stdout should contain renderer marker `{marker}` in debug mode",
		);
	}

	assert!(
		!stdout.contains("=== "),
		"debug mode should not print grouped legacy task headers",
	);
}

fn count_occurrences(haystack: &str, needle: &str) -> usize {
	haystack.match_indices(needle).count()
}

fn count_exact_lines(haystack: &str, needle: &str) -> usize {
	haystack.lines().filter(|line| *line == needle).count()
}

fn run_pullhook(repo_root: &Path, args: &[&str]) -> std::process::Output {
	ProcessCommand::new(assert_cmd::cargo::cargo_bin!("pullhook"))
		.current_dir(repo_root)
		.env("RUST_LOG", "debug")
		.env("PULLHOOK_RENDER_MODE", "never")
		.args(args)
		.output()
		.expect("run pullhook command")
}

fn setup_repo_with_merge() -> TempDir {
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

	let branch = current_branch(repo_root);
	run_git(repo_root, &["checkout", "-b", "feature/update-locks"]);

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
	run_git(repo_root, &["checkout", &branch]);
	run_git(
		repo_root,
		&["merge", "--no-ff", "feature/update-locks", "-m", "merge feature"],
	);

	temp
}

fn current_branch(repo_root: &Path) -> String {
	let output = ProcessCommand::new("git")
		.current_dir(repo_root)
		.args(["branch", "--show-current"])
		.output()
		.expect("read current branch");

	assert!(output.status.success(), "failed to detect current branch");
	String::from_utf8_lossy(&output.stdout).trim().to_owned()
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
