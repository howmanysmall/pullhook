//! Integration tests for pullhook CLI behavior.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn runs_command_per_matched_directory() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let status = ProcessCommand::new(assert_cmd::cargo::cargo_bin!("pullhook"))
		.current_dir(repo_root)
		.args([
			"--pattern",
			"packages/*/package-lock.json",
			"--command",
			"sh -c 'echo ran > .pullhook-marker'",
		])
		.status()
		.expect("command runs");

	assert!(status.success(), "pullhook should succeed");
	assert!(predicate::path::is_file().eval(&repo_root.join("packages/a/.pullhook-marker")));
	assert!(predicate::path::is_file().eval(&repo_root.join("packages/b/.pullhook-marker")));
}

#[test]
fn runs_command_once_in_repo_root() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let status = ProcessCommand::new(assert_cmd::cargo::cargo_bin!("pullhook"))
		.current_dir(repo_root)
		.args([
			"--pattern",
			"packages/*/package-lock.json",
			"--command",
			"sh -c 'echo ran > .pullhook-root-marker'",
			"--once",
		])
		.status()
		.expect("command runs");

	assert!(status.success(), "pullhook should succeed");
	assert!(predicate::path::is_file().eval(&repo_root.join(".pullhook-root-marker")));
	assert!(!predicate::path::is_file().eval(&repo_root.join("packages/a/.pullhook-root-marker")));
	assert!(!predicate::path::is_file().eval(&repo_root.join("packages/b/.pullhook-root-marker")));
}

#[test]
fn skips_execution_when_no_files_match() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let status = ProcessCommand::new(assert_cmd::cargo::cargo_bin!("pullhook"))
		.current_dir(repo_root)
		.args([
			"--pattern",
			"**/*.md",
			"--command",
			"sh -c 'echo ran > .pullhook-no-match-marker'",
		])
		.status()
		.expect("command runs");

	assert!(status.success(), "no matches should still succeed");
	assert!(!predicate::path::is_file().eval(&repo_root.join(".pullhook-no-match-marker")));
}

#[test]
fn install_ignores_nested_manifest_changes_that_do_not_match_install_pattern() {
	let temp = setup_repo_with_nested_manifest_change();
	let repo_root = temp.path();

	let output = ProcessCommand::new(assert_cmd::cargo::cargo_bin!("pullhook"))
		.current_dir(repo_root)
		.env("PULLHOOK_RENDER_MODE", "never")
		.args(["--install", "--dry-run"])
		.output()
		.expect("command runs");

	assert!(output.status.success(), "--install --dry-run should succeed");

	let stdout = String::from_utf8_lossy(&output.stdout);
	assert!(stdout.contains("pattern: +(package.json|package-lock.json)"));
	assert!(stdout.contains("matched: 0"));
	assert!(!stdout.contains("directory: ."));
	assert!(!stdout.contains("command: npm install"));
}

#[test]
fn install_matches_repo_root_manifest_changes() {
	let temp = setup_repo_with_root_manifest_change();
	let repo_root = temp.path();

	let output = ProcessCommand::new(assert_cmd::cargo::cargo_bin!("pullhook"))
		.current_dir(repo_root)
		.env("PULLHOOK_RENDER_MODE", "never")
		.args(["--install", "--dry-run"])
		.output()
		.expect("command runs");

	assert!(output.status.success(), "--install --dry-run should succeed");

	let stdout = String::from_utf8_lossy(&output.stdout);
	assert!(stdout.contains("matched: 1"));
	assert!(stdout.contains("directory: ."));
	assert!(stdout.contains("command: npm install"));
}

#[test]
fn install_runs_from_subdirectory_with_repo_root_discovery() {
	let temp = setup_repo_with_root_manifest_change();
	let repo_root = temp.path();

	let output = ProcessCommand::new(assert_cmd::cargo::cargo_bin!("pullhook"))
		.current_dir(repo_root.join("packages/a"))
		.env("PULLHOOK_RENDER_MODE", "never")
		.args(["--install", "--dry-run"])
		.output()
		.expect("command runs");

	assert!(output.status.success(), "--install from subdirectory should succeed");

	let stdout = String::from_utf8_lossy(&output.stdout);
	assert!(stdout.contains("matched: 1"));
	assert!(stdout.contains("directory: ."));
	assert!(stdout.contains("command: npm install"));
}

#[test]
fn install_accepts_explicit_base() {
	let temp = setup_repo_with_root_manifest_change();
	let repo_root = temp.path();

	let output = ProcessCommand::new(assert_cmd::cargo::cargo_bin!("pullhook"))
		.current_dir(repo_root)
		.env("PULLHOOK_RENDER_MODE", "never")
		.args(["--install", "--base", "HEAD~1", "--dry-run"])
		.output()
		.expect("command runs");

	assert!(output.status.success(), "--install with explicit base should succeed");

	let stdout = String::from_utf8_lossy(&output.stdout);
	assert!(stdout.contains("matched: 1"));
	assert!(stdout.contains("directory: ."));
	assert!(stdout.contains("command: npm install"));
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

fn setup_repo_with_root_manifest_change() -> TempDir {
	let temp = tempfile::tempdir().expect("create temp dir");
	let repo_root = temp.path();

	fs::create_dir_all(repo_root.join("packages/a")).expect("create nested package directory");
	run_git(repo_root, &["init"]);
	run_git(repo_root, &["config", "user.email", "pullhook@example.com"]);
	run_git(repo_root, &["config", "user.name", "Pullhook Test"]);

	write_file(repo_root, Path::new("package.json"), "{\"name\":\"root\"}\n");
	write_file(
		repo_root,
		Path::new("package-lock.json"),
		"{\"name\":\"root\",\"lockfileVersion\":3}\n",
	);

	run_git(repo_root, &["add", "."]);
	run_git(repo_root, &["commit", "-m", "initial"]);

	let branch = current_branch(repo_root);
	run_git(repo_root, &["checkout", "-b", "feature/update-lockfile"]);

	write_file(
		repo_root,
		Path::new("package-lock.json"),
		"{\"name\":\"root\",\"lockfileVersion\":4}\n",
	);

	run_git(repo_root, &["add", "."]);
	run_git(repo_root, &["commit", "-m", "update root lockfile"]);
	run_git(repo_root, &["checkout", &branch]);
	run_git(
		repo_root,
		&["merge", "--no-ff", "feature/update-lockfile", "-m", "merge feature"],
	);

	temp
}

fn setup_repo_with_nested_manifest_change() -> TempDir {
	let temp = tempfile::tempdir().expect("create temp dir");
	let repo_root = temp.path();

	run_git(repo_root, &["init"]);
	run_git(repo_root, &["config", "user.email", "pullhook@example.com"]);
	run_git(repo_root, &["config", "user.name", "Pullhook Test"]);

	write_file(repo_root, Path::new("package.json"), "{\"name\":\"root\"}\n");
	write_file(
		repo_root,
		Path::new("package-lock.json"),
		"{\"name\":\"root\",\"lockfileVersion\":3}\n",
	);
	write_file(
		repo_root,
		Path::new("packages/a/package.json"),
		"{\"name\":\"a\",\"version\":\"1.0.0\"}\n",
	);

	run_git(repo_root, &["add", "."]);
	run_git(repo_root, &["commit", "-m", "initial"]);

	let branch = current_branch(repo_root);
	run_git(repo_root, &["checkout", "-b", "feature/update-manifest"]);

	write_file(
		repo_root,
		Path::new("packages/a/package.json"),
		"{\"name\":\"a\",\"version\":\"2.0.0\"}\n",
	);

	run_git(repo_root, &["add", "."]);
	run_git(repo_root, &["commit", "-m", "update nested package"]);
	run_git(repo_root, &["checkout", &branch]);
	run_git(
		repo_root,
		&["merge", "--no-ff", "feature/update-manifest", "-m", "merge feature"],
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
