//! Integration tests for pullhook CLI behavior.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Output, Stdio};

use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn runs_command_per_matched_directory() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(
		repo_root,
		&[
			"--pattern",
			"packages/*/package-lock.json",
			"--command",
			"sh -c 'echo ran > .pullhook-marker'",
		],
	);

	assert!(output.status.success(), "pullhook should succeed");
	assert!(predicate::path::is_file().eval(&repo_root.join("packages/a/.pullhook-marker")));
	assert!(predicate::path::is_file().eval(&repo_root.join("packages/b/.pullhook-marker")));
}

#[test]
fn runs_command_once_in_repo_root() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(
		repo_root,
		&[
			"--pattern",
			"packages/*/package-lock.json",
			"--command",
			"sh -c 'echo ran > .pullhook-root-marker'",
			"--once",
		],
	);

	assert!(output.status.success(), "pullhook should succeed");
	assert!(predicate::path::is_file().eval(&repo_root.join(".pullhook-root-marker")));
	assert!(!predicate::path::is_file().eval(&repo_root.join("packages/a/.pullhook-root-marker")));
	assert!(!predicate::path::is_file().eval(&repo_root.join("packages/b/.pullhook-root-marker")));
}

#[test]
fn skips_execution_when_no_files_match() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(
		repo_root,
		&[
			"--pattern",
			"**/*.md",
			"--command",
			"sh -c 'echo ran > .pullhook-no-match-marker'",
		],
	);

	assert!(output.status.success(), "no matches should still succeed");
	assert!(!predicate::path::is_file().eval(&repo_root.join(".pullhook-no-match-marker")));
}

#[test]
fn install_ignores_nested_manifest_changes_that_do_not_match_install_pattern() {
	let temp = setup_repo_with_nested_manifest_change();
	let repo_root = temp.path();

	let output = run_pullhook_with_env(
		repo_root,
		&["--install", "--dry-run"],
		&[("PULLHOOK_RENDER_MODE", "never")],
	);

	assert!(output.status.success(), "--install --dry-run should succeed");

	let stdout = stdout_text(&output);
	assert!(stdout.contains("pattern: +(package.json|package-lock.json|npm-shrinkwrap.json)"));
	assert!(stdout.contains("matched: 0"));
	assert!(!stdout.contains("directory: ."));
	assert!(!stdout.contains("command: npm install"));
}

#[test]
fn install_json_reports_package_manager_detection_failure_details() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(repo_root, &["--install", "--dry-run", "--json"]);

	assert!(
		!output.status.success(),
		"--install --json should fail without root package manager files"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse install error json");
	assert_eq!(value["status"], "error");
	assert!(
		value["error"]
			.as_str()
			.expect("error")
			.contains("failed to detect package manager for --install")
	);
	let details = value["details"].as_array().expect("details array");
	assert!(details.iter().any(|detail| {
		detail
			.as_str()
			.expect("detail")
			.contains("no supported package manager files found")
	}));
	assert!(
		details
			.iter()
			.any(|detail| detail.as_str().expect("detail")
				== "add a supported package-manager lockfile at the repo root")
	);
	assert!(details.iter().any(|detail| {
		detail.as_str().expect("detail")
			== "or pass explicit `--pattern <glob>` and `--command <cmd>` instead of `--install`"
	}));
	assert_eq!(value["packageManagerError"]["kind"], "not_found");
	assert!(
		value["packageManagerError"]["root"]
			.as_str()
			.expect("package manager root")
			.ends_with(
				repo_root
					.file_name()
					.expect("temp dir name")
					.to_str()
					.expect("utf8 temp dir name")
			)
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("failed to detect package manager for --install"));
}

#[test]
fn install_json_reports_ambiguous_package_managers() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(repo_root, Path::new("bun.lock"), "");
	write_file(repo_root, Path::new("package-lock.json"), "{}\n");

	let output = run_pullhook(repo_root, &["--install", "--dry-run", "--json"]);

	assert!(
		!output.status.success(),
		"--install --json should fail when root lockfiles are ambiguous"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse install error json");
	assert_eq!(value["status"], "error");
	assert!(
		value["error"]
			.as_str()
			.expect("error")
			.contains("failed to detect package manager for --install")
	);
	assert_eq!(value["packageManagerError"]["kind"], "ambiguous");
	assert_eq!(value["packageManagerError"]["found"], serde_json::json!(["bun", "npm"]));
	let details = value["details"].as_array().expect("details array");
	assert!(details.iter().any(|detail| {
		detail
			.as_str()
			.expect("detail")
			.contains("multiple package managers detected")
	}));
	let stderr = stderr_text(&output);
	assert!(stderr.contains("failed to detect package manager for --install"));
}

#[test]
fn install_matches_repo_root_manifest_changes() {
	let temp = setup_repo_with_root_manifest_change();
	let repo_root = temp.path();

	let output = run_pullhook_with_env(
		repo_root,
		&["--install", "--dry-run"],
		&[("PULLHOOK_RENDER_MODE", "never")],
	);

	assert_install_dry_run_matches_repo_root(&output);
}

#[test]
fn install_runs_from_subdirectory_with_repo_root_discovery() {
	let temp = setup_repo_with_root_manifest_change();
	let repo_root = temp.path();

	let output = run_pullhook_with_env(
		&repo_root.join("packages/a"),
		&["--install", "--dry-run"],
		&[("PULLHOOK_RENDER_MODE", "never")],
	);

	assert_install_dry_run_matches_repo_root(&output);
}

#[test]
fn install_accepts_explicit_base() {
	let temp = setup_repo_with_root_manifest_change();
	let repo_root = temp.path();

	let output = run_pullhook_with_env(
		repo_root,
		&["--install", "--base", "HEAD~1", "--dry-run"],
		&[("PULLHOOK_RENDER_MODE", "never")],
	);

	assert_install_dry_run_matches_repo_root(&output);
}

#[test]
fn legacy_dry_run_json_reports_plan() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(
		repo_root,
		&[
			"--pattern",
			"packages/*/package-lock.json",
			"--command",
			"npm install",
			"--dry-run",
			"--json",
		],
	);

	assert!(output.status.success(), "legacy dry-run json should succeed");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse legacy dry-run json");
	assert_eq!(value["status"], "ok");
	assert_eq!(value["code"], serde_json::Value::Null);
	assert_eq!(value["error"], serde_json::Value::Null);
	assert_eq!(value["mode"], "dry-run");
	assert_eq!(value["pattern"], "packages/*/package-lock.json");
	assert_eq!(value["plannedCommands"], 2);
	assert_eq!(
		value["matchedFiles"],
		serde_json::json!(["packages/a/package-lock.json", "packages/b/package-lock.json"])
	);
	assert_eq!(value["tasks"], serde_json::json!(["packages/a", "packages/b"]));
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "legacy dry-run json should not write stderr");
}

#[test]
fn legacy_run_json_reports_execution_results() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(
		repo_root,
		&[
			"--pattern",
			"packages/*/package-lock.json",
			"--command",
			"sh -c 'echo legacy-stdout; echo legacy-stderr >&2'",
			"--once",
			"--json",
		],
	);

	assert!(output.status.success(), "legacy run json should succeed");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse legacy run json");
	assert_eq!(value["status"], "ok");
	assert_eq!(value["code"], serde_json::Value::Null);
	assert_eq!(value["error"], serde_json::Value::Null);
	assert_eq!(value["mode"], "run");
	assert_eq!(value["summary"]["passed"], 1);
	assert_eq!(value["summary"]["failed"], 0);
	let results = value["results"].as_array().expect("results array");
	assert_eq!(results.len(), 1);
	assert_eq!(results[0]["cwd"], ".");
	assert_eq!(results[0]["state"], "success");
	assert_eq!(results[0]["outputs"][0]["stdout"], "legacy-stdout\n");
	assert_eq!(results[0]["outputs"][0]["stderr"], "legacy-stderr\n");
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "legacy run json should not write stderr");
}

#[test]
fn legacy_run_json_reports_failures() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(
		repo_root,
		&[
			"--pattern",
			"packages/*/package-lock.json",
			"--command",
			"sh -c 'echo nope >&2; exit 7'",
			"--once",
			"--json",
		],
	);

	assert!(
		!output.status.success(),
		"legacy run json should fail on command failure"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse failed legacy run json");
	assert_eq!(value["status"], "error");
	assert_eq!(value["code"], "command_failed");
	assert_eq!(value["error"], "1 task(s) failed");
	assert_eq!(value["summary"]["passed"], 0);
	assert_eq!(value["summary"]["failed"], 1);
	assert_eq!(value["results"][0]["state"], "failed");
	let stderr = stderr_text(&output);
	assert!(stderr.contains("1 task(s) failed"));
}

#[test]
fn legacy_json_reports_missing_mode_as_json() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(repo_root, &["--json"]);

	assert!(!output.status.success(), "legacy --json should require a mode");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse missing mode json");
	assert_eq!(value["status"], "error");
	let error = value["error"].as_str().expect("error");
	assert!(error.contains("missing required argument"));
	assert_eq!(
		value["details"],
		serde_json::json!([
			"use `pullhook run` to execute configured rules from pullhook.json",
			"or pass `--pattern <glob>` with `--command <cmd>` for legacy top-level mode",
			"use `--install` only when the repo root has a supported package-manager file"
		])
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("missing required argument"));
}

#[test]
fn legacy_json_reports_repo_discovery_errors_as_json() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(
		temp.path(),
		&[
			"--pattern",
			"packages/*/package-lock.json",
			"--command",
			"true",
			"--json",
		],
	);

	assert!(!output.status.success(), "legacy --json should fail outside a git repo");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse repo discovery json");
	assert_eq!(value["status"], "error");
	assert_eq!(value["code"], "repository_not_found");
	assert_eq!(value["error"], "failed to resolve repository root");
	assert_eq!(value["repositoryError"]["kind"], "not_found");
	let reported_path = PathBuf::from(value["repositoryError"]["path"].as_str().expect("repository path"));
	assert_eq!(
		reported_path.canonicalize().expect("canonicalize reported path"),
		temp.path().canonicalize().expect("canonicalize temp path")
	);
	let details = value["details"].as_array().expect("details array");
	assert!(
		details
			.iter()
			.any(|detail| detail.as_str().expect("detail") == "rerun from inside a Git working tree")
	);
	assert!(details.iter().any(|detail| {
		detail.as_str().expect("detail") == "or initialize a repository with `git init` before running pullhook"
	}));
	let stderr = stderr_text(&output);
	assert!(stderr.contains("failed to resolve repository root"));
}

#[test]
fn legacy_json_reports_invalid_pattern_as_json() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(repo_root, &["--pattern", "{", "--command", "true", "--json"]);

	assert!(!output.status.success(), "legacy --json should fail for invalid glob");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse invalid pattern json");
	assert_eq!(value["status"], "error");
	let error = value["error"].as_str().expect("error");
	assert!(error.contains("failed to compile pattern"));
	assert_eq!(value["patternError"]["pattern"], "{");
	assert!(
		value["patternError"]["reason"]
			.as_str()
			.expect("pattern reason")
			.contains("unclosed")
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("failed to compile pattern"));
}

#[test]
fn legacy_json_reports_diff_base_errors_as_json() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(
		repo_root,
		&[
			"--pattern",
			"packages/*/package-lock.json",
			"--command",
			"true",
			"--base",
			"missing-base-ref",
			"--json",
		],
	);

	assert!(!output.status.success(), "legacy --json should fail for invalid base");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse legacy base error json");
	assert_eq!(value["status"], "error");
	assert_eq!(value["code"], "diff_base_revision_not_found");
	let error = value["error"].as_str().expect("error");
	assert!(error.contains("failed to resolve diff base or read changed files"));
	assert_eq!(value["diffBaseError"]["kind"], "revision_not_found");
	assert_eq!(value["diffBaseError"]["revision"], "missing-base-ref");
	let details = value["details"].as_array().expect("details array");
	assert!(details.iter().any(|detail| {
		detail
			.as_str()
			.expect("detail")
			.contains("check that `--base <rev>` names a commit")
	}));
	assert!(details.iter().any(|detail| {
		detail
			.as_str()
			.expect("detail")
			.contains("automatic diff-base fallback")
	}));
	let stderr = stderr_text(&output);
	assert!(stderr.contains("failed to resolve diff base or read changed files"));
}

#[test]
fn legacy_json_reports_command_parse_errors_as_json() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(
		repo_root,
		&[
			"--pattern",
			"packages/*/package-lock.json",
			"--command",
			"sh -c 'echo nope",
			"--json",
		],
	);

	assert!(
		!output.status.success(),
		"legacy --json should fail for invalid commands"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse command error json");
	assert_eq!(value["status"], "error");
	assert_eq!(value["error"], "failed to prepare command invocations");
	assert_eq!(value["commandParse"]["command"], "sh -c 'echo nope");
	assert!(
		value["commandParse"]["reason"]
			.as_str()
			.expect("parse reason")
			.contains("missing closing quote")
	);
	let details = value["details"].as_array().expect("details array");
	assert!(details.iter().any(|detail| {
		detail
			.as_str()
			.expect("detail")
			.contains("invalid command `sh -c 'echo nope`")
	}));
	let stderr = stderr_text(&output);
	assert!(stderr.contains("failed to prepare command invocations"));
	assert!(stderr.contains("invalid command `sh -c 'echo nope`"));
}

#[test]
fn legacy_run_json_rejects_debug_mode() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(
		repo_root,
		&[
			"--pattern",
			"packages/*/package-lock.json",
			"--command",
			"true",
			"--json",
			"--debug",
		],
	);

	assert!(!output.status.success(), "legacy run json should reject debug mode");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse debug conflict json");
	assert_eq!(value["status"], "error");
	assert_eq!(value["error"], "--json cannot be used with --debug");
	assert_eq!(
		value["details"],
		serde_json::json!([
			"rerun without `--debug` when a script needs JSON",
			"rerun without `--json` when you need debug traces"
		])
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("--json cannot be used with --debug"));
}

#[test]
fn config_json_commands_reject_debug_mode() {
	let temp = tempfile::tempdir().expect("create temp dir");
	let commands: &[&[&str]] = &[
		&["explain", "--json", "--debug"],
		&["validate", "--json", "--debug"],
		&["doctor", "--json", "--debug"],
		&["config", "--json", "--debug"],
		&["rules", "--json", "--debug"],
	];

	for args in commands {
		let output = run_pullhook(temp.path(), args);

		assert!(
			!output.status.success(),
			"`pullhook {}` should reject --json --debug",
			args.join(" ")
		);
		let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
			panic!(
				"`pullhook {}` should print JSON to stdout: {error}; stdout: {}",
				args.join(" "),
				stdout_text(&output)
			)
		});
		assert_eq!(value["status"], "error");
		assert_eq!(value["error"], "--json cannot be used with --debug");
		let stderr = stderr_text(&output);
		assert!(
			stderr.contains("--json cannot be used with --debug"),
			"`pullhook {}` stderr should explain the conflict",
			args.join(" ")
		);
	}
}

#[test]
fn config_mode_json_reports_repo_discovery_errors_as_json() {
	let temp = tempfile::tempdir().expect("create temp dir");
	let commands: &[&[&str]] = &[
		&["validate", "--json"],
		&["doctor", "--json"],
		&["config", "--json"],
		&["init", "--json"],
	];

	for args in commands {
		let output = run_pullhook(temp.path(), args);

		assert!(
			!output.status.success(),
			"`pullhook {}` should fail outside a git repo",
			args.join(" ")
		);
		let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
			panic!(
				"`pullhook {}` should print JSON to stdout: {error}; stdout: {}",
				args.join(" "),
				stdout_text(&output)
			)
		});
		assert_eq!(value["status"], "error");
		assert_eq!(value["error"], "failed to resolve repository root");
		let stderr = stderr_text(&output);
		assert!(stderr.contains("failed to resolve repository root"));
	}
}

#[test]
fn completion_command_succeeds_outside_git_repo() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["completion", "bash"]);

	assert!(
		output.status.success(),
		"completion command should succeed outside a git repo"
	);

	let stdout = stdout_text(&output);
	assert!(stdout.contains("_pullhook()"));
	assert!(stdout.contains("complete -F _pullhook -o bashdefault -o default pullhook"));

	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "completion command should not write stderr");
}

#[test]
fn completion_command_writes_output_file() {
	let temp = tempfile::tempdir().expect("create temp dir");
	let output_path = Path::new("completions/fish/pullhook.fish");

	let output = run_pullhook(
		temp.path(),
		&[
			"completion",
			"fish",
			"--output",
			output_path.to_str().expect("utf-8 path"),
		],
	);

	assert!(
		output.status.success(),
		"completion --output should succeed outside a git repo"
	);
	let stdout = stdout_text(&output);
	assert!(stdout.trim().is_empty(), "completion --output should not write stdout");
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "completion --output should not write stderr");
	let completion = fs::read_to_string(temp.path().join(output_path)).expect("read completion file");
	assert!(completion.contains("complete -c pullhook"));
	assert!(completion.contains("-l commands-only"));
	assert!(completion.contains("-a \"completion\""));
}

#[test]
fn completion_check_succeeds_when_output_is_current() {
	let temp = tempfile::tempdir().expect("create temp dir");
	let output_path = Path::new("completions/fish/pullhook.fish");
	let output_path_str = output_path.to_str().expect("utf-8 path");
	let write = run_pullhook(temp.path(), &["completion", "fish", "--output", output_path_str]);
	assert!(write.status.success(), "completion --output should seed check file");

	let output = run_pullhook(
		temp.path(),
		&["completion", "fish", "--check", "--output", output_path_str],
	);

	assert!(
		output.status.success(),
		"completion --check should pass for current completion"
	);
	let stdout = stdout_text(&output);
	assert!(stdout.contains("completion up to date"));
	assert!(stdout.contains(output_path_str));
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "completion --check should not write stderr");
}

#[test]
fn completion_check_fails_when_output_is_stale() {
	let temp = tempfile::tempdir().expect("create temp dir");
	let output_path = Path::new("completions/fish/pullhook.fish");
	let output_path_str = output_path.to_str().expect("utf-8 path");
	write_file(temp.path(), output_path, "complete -c pullhook -f\n");

	let output = run_pullhook(
		temp.path(),
		&["completion", "fish", "--check", "--output", output_path_str],
	);

	assert!(
		!output.status.success(),
		"completion --check should fail for stale completion"
	);
	assert!(
		stdout_text(&output).trim().is_empty(),
		"stale completion check should not write stdout"
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("completion out of date"));
	assert!(stderr.contains("pullhook completion fish --output completions/fish/pullhook.fish"));
}

#[test]
fn completion_check_json_reports_match_status() {
	let temp = tempfile::tempdir().expect("create temp dir");
	let output_path = Path::new("completions/fish/pullhook.fish");
	let output_path_str = output_path.to_str().expect("utf-8 path");
	write_file(temp.path(), output_path, "complete -c pullhook -f\n");

	let output = run_pullhook(
		temp.path(),
		&["completion", "fish", "--check", "--output", output_path_str, "--json"],
	);

	assert!(
		!output.status.success(),
		"completion --check --json should fail for stale completion"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse completion check json");
	assert_eq!(value["status"], "error");
	assert_eq!(value["code"], "completion_out_of_date");
	assert!(value["path"].as_str().expect("path").ends_with(output_path_str));
	assert_eq!(value["shell"], "fish");
	assert_eq!(value["exists"], true);
	assert_eq!(value["matches"], false);
	assert_eq!(value["error"], "completion output is out of date");
	let details = value["details"].as_array().expect("details array");
	assert!(details.iter().any(|detail| {
		detail
			.as_str()
			.expect("detail")
			.contains("pullhook completion fish --output completions/fish/pullhook.fish")
	}));
	let stderr = stderr_text(&output);
	assert!(stderr.contains("completion out of date"));
}

#[test]
fn completion_check_json_reports_up_to_date_status() {
	let temp = tempfile::tempdir().expect("create temp dir");
	let output_path = Path::new("completions/fish/pullhook.fish");
	let output_path_str = output_path.to_str().expect("utf-8 path");

	let write_output = run_pullhook(temp.path(), &["completion", "fish", "--output", output_path_str]);
	assert!(
		write_output.status.success(),
		"completion --output should write generated completion"
	);

	let output = run_pullhook(
		temp.path(),
		&["completion", "fish", "--check", "--output", output_path_str, "--json"],
	);

	assert!(
		output.status.success(),
		"completion --check --json should pass for current completion"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse completion check json");
	assert_eq!(value["status"], "ok");
	assert_eq!(value["code"], serde_json::Value::Null);
	assert_eq!(value["error"], serde_json::Value::Null);
	assert_eq!(value["shell"], "fish");
	assert_eq!(value["exists"], true);
	assert_eq!(value["matches"], true);
	assert_eq!(value["details"], serde_json::json!([]));
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"up-to-date completion check should not write stderr"
	);
}

#[test]
fn codes_text_lists_stable_json_codes() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["codes"]);

	assert!(output.status.success(), "codes should succeed outside a git repo");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("JSON status codes"));
	assert!(stdout.contains("ok responses use code: null"));
	assert!(stdout.contains("config_missing"));
	assert!(stdout.contains("no_rules_matched"));
	assert!(stdout.contains("schema_out_of_date"));
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "codes should not write stderr");
}

#[test]
fn codes_json_lists_stable_json_codes() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["codes", "--json"]);

	assert!(
		output.status.success(),
		"codes --json should succeed outside a git repo"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse codes json");
	assert_eq!(value["status"], "ok");
	assert_eq!(value["code"], serde_json::Value::Null);
	assert_eq!(value["successCode"], serde_json::Value::Null);
	let codes = value["codes"].as_array().expect("codes array");
	assert!(codes.iter().any(|entry| entry["code"] == "config_missing"));
	assert!(codes.iter().any(|entry| entry["code"] == "no_rules_matched"));
	assert!(codes.iter().any(|entry| entry["code"] == "schema_out_of_date"));
	assert_eq!(
		value["summary"]["codes"].as_u64().expect("code count"),
		codes.len() as u64
	);
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "codes --json should not write stderr");
}

#[test]
fn codes_kind_filter_limits_results() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["codes", "--kind", "doctor-check", "--json"]);

	assert!(
		output.status.success(),
		"codes --kind doctor-check --json should succeed"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse filtered codes json");
	assert_eq!(value["status"], "ok");
	assert_eq!(value["filters"]["kind"], "doctor-check");
	let codes = value["codes"].as_array().expect("codes array");
	assert!(!codes.is_empty(), "doctor-check filter should keep doctor codes");
	assert!(
		codes.iter().all(|entry| entry["kind"] == "doctor-check"),
		"doctor-check filter should exclude error codes"
	);
	assert!(codes.iter().any(|entry| entry["code"] == "config_ok"));
	assert!(
		!codes.iter().any(|entry| entry["code"] == "config_missing"),
		"config_missing is an error code, not a doctor-check code"
	);
	assert_eq!(
		value["summary"]["codes"].as_u64().expect("code count"),
		codes.len() as u64
	);
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"filtered codes --json should not write stderr"
	);
}

#[test]
fn commands_text_lists_cli_catalog() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["commands"]);

	assert!(output.status.success(), "commands should succeed outside a git repo");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("Pullhook commands"));
	assert!(stdout.contains("legacy one-off mode is available through top-level options"));
	assert!(stdout.contains("run [workflow]"));
	assert!(stdout.contains("codes [reference]"));
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "commands should not write stderr");
}

#[test]
fn commands_json_lists_cli_catalog() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["commands", "--json"]);

	assert!(
		output.status.success(),
		"commands --json should succeed outside a git repo"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse commands json");
	assert_eq!(value["status"], "ok");
	assert_eq!(value["code"], serde_json::Value::Null);
	let commands = value["commands"].as_array().expect("commands array");
	assert!(
		commands
			.iter()
			.any(|entry| entry["name"] == "run" && entry["requiresRepo"] == true)
	);
	assert!(
		commands
			.iter()
			.any(|entry| entry["name"] == "commands" && entry["category"] == "reference")
	);
	assert!(
		commands
			.iter()
			.any(|entry| entry["name"] == "codes" && entry["json"] == true)
	);
	assert_eq!(
		value["summary"]["commands"].as_u64().expect("command count"),
		commands.len() as u64
	);
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "commands --json should not write stderr");
}

#[test]
fn root_help_lists_common_examples() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["--help"]);

	assert!(output.status.success(), "root help should succeed");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("Examples:"));
	assert!(stdout.contains("pullhook --install --dry-run"));
	assert!(stdout.contains("pullhook init --format json"));
	assert!(stdout.contains("pullhook commands --json"));
	assert!(stdout.contains("pullhook codes --json"));
	assert!(stdout.contains("schema"));
	assert!(stdout.contains("codes"));
	assert!(stdout.contains("Legacy one-off options:"));
	assert!(stdout.contains("Use `pullhook explain --all-matches` to preview config rule matches."));
	assert!(stdout.contains("Use `pullhook commands` to inspect the command catalog."));
	assert!(stdout.contains("Use `pullhook codes` to inspect stable JSON status codes."));
}

#[test]
fn run_help_lists_json_examples() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["run", "--help"]);

	assert!(output.status.success(), "run help should succeed");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("pullhook run --dry-run"));
	assert!(stdout.contains("pullhook run --json"));
	assert!(stdout.contains("pullhook run --quiet"));
	assert!(stdout.contains("pullhook run --summary-only"));
	assert!(stdout.contains("pullhook run --commands-only"));
	assert!(stdout.contains("pullhook run --changed-files-only"));
	assert!(stdout.contains("pullhook run --matched-files-only"));
	assert!(stdout.contains("pullhook run --matched-rules-only"));
	assert!(stdout.contains("pullhook run --require-match --dry-run"));
	assert!(stdout.contains("git diff --name-only HEAD~1 | pullhook run --changed-files-file - --dry-run"));
	assert!(stdout.contains("pullhook run --config config/pullhook.custom.json --all-matches"));
	assert!(stdout.contains("Input options:"));
	assert!(stdout.contains("Execution options:"));
	assert!(stdout.contains("Output options:"));
	assert!(stdout.contains("Rule selection:"));
	assert!(stdout.contains("Display options:"));
	assert!(stdout.contains("--no-color"));
	assert!(stdout.contains("--quiet"));
	assert!(stdout.contains("--summary-only"));
	assert!(stdout.contains("--commands-only"));
	assert!(stdout.contains("--changed-files-only"));
	assert!(stdout.contains("--matched-files-only"));
	assert!(stdout.contains("--matched-rules-only"));
	assert!(stdout.contains("--require-match"));
}

#[test]
fn rules_help_lists_script_friendly_output_modes() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["rules", "--help"]);

	assert!(output.status.success(), "rules help should succeed");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("pullhook rules --names-only"));
	assert!(stdout.contains("pullhook rules --commands-only"));
	assert!(stdout.contains("pullhook rules --patterns-only"));
	assert!(stdout.contains("pullhook rules --rule lint --json"));
	assert!(stdout.contains("Input options:"));
	assert!(stdout.contains("Output options:"));
	assert!(stdout.contains("Selection options:"));
	assert!(stdout.contains("Display options:"));
	assert!(stdout.contains("--commands-only"));
	assert!(stdout.contains("--patterns-only"));
}

#[test]
fn diagnostic_help_groups_options_by_task() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let validate = run_pullhook(temp.path(), &["validate", "--help"]);
	assert!(validate.status.success(), "validate help should succeed");
	let validate_stdout = stdout_text(&validate);
	assert!(validate_stdout.contains("Input options:"));
	assert!(validate_stdout.contains("Output options:"));
	assert!(validate_stdout.contains("Display options:"));

	let doctor = run_pullhook(temp.path(), &["doctor", "--help"]);
	assert!(doctor.status.success(), "doctor help should succeed");
	let doctor_stdout = stdout_text(&doctor);
	assert!(doctor_stdout.contains("Input options:"));
	assert!(doctor_stdout.contains("Output options:"));
	assert!(doctor_stdout.contains("Check options:"));
	assert!(doctor_stdout.contains("Display options:"));

	let config = run_pullhook(temp.path(), &["config", "--help"]);
	assert!(config.status.success(), "config help should succeed");
	let config_stdout = stdout_text(&config);
	assert!(config_stdout.contains("Input options:"));
	assert!(config_stdout.contains("Output options:"));
	assert!(config_stdout.contains("Resolution options:"));
	assert!(config_stdout.contains("Display options:"));
}

#[test]
fn explain_help_lists_summary_only_example() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["explain", "--help"]);

	assert!(output.status.success(), "explain help should succeed");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("pullhook explain --summary-only"));
	assert!(stdout.contains("pullhook explain --commands-only"));
	assert!(stdout.contains("pullhook explain --changed-files-only"));
	assert!(stdout.contains("pullhook explain --matched-files-only"));
	assert!(stdout.contains("pullhook explain --matched-rules-only"));
	assert!(stdout.contains("pullhook explain --require-match"));
	assert!(stdout.contains("git diff --name-only HEAD~1 | pullhook explain --changed-files-file -"));
	assert!(stdout.contains("Input options:"));
	assert!(stdout.contains("Rule selection:"));
	assert!(stdout.contains("Output options:"));
	assert!(stdout.contains("Display options:"));
	assert!(stdout.contains("--summary-only"));
	assert!(stdout.contains("--commands-only"));
	assert!(stdout.contains("--changed-files-only"));
	assert!(stdout.contains("--matched-files-only"));
	assert!(stdout.contains("--matched-rules-only"));
	assert!(stdout.contains("--require-match"));
}

#[test]
fn no_color_conflicts_with_render_mode() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["run", "--no-color", "--render", "never"]);

	assert!(!output.status.success(), "--no-color should conflict with --render");
	let stderr = stderr_text(&output);
	assert!(stderr.contains("cannot be used with"));
	assert!(stderr.contains("--no-color"));
	assert!(stderr.contains("--render <mode>"));
}

#[test]
fn run_quiet_conflicts_with_json() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["run", "--quiet", "--json"]);

	assert!(!output.status.success(), "--quiet should conflict with --json");
	let stderr = stderr_text(&output);
	assert!(stderr.contains("cannot be used with"));
	assert!(stderr.contains("--quiet"));
	assert!(stderr.contains("--json"));
}

#[test]
fn run_commands_only_conflicts_with_other_output_modes() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let json_output = run_pullhook(temp.path(), &["run", "--commands-only", "--json"]);

	assert!(
		!json_output.status.success(),
		"run --commands-only should conflict with --json"
	);
	let json_stderr = stderr_text(&json_output);
	assert!(json_stderr.contains("cannot be used with"));
	assert!(json_stderr.contains("--commands-only"));
	assert!(json_stderr.contains("--json"));

	let quiet_output = run_pullhook(temp.path(), &["run", "--commands-only", "--quiet"]);

	assert!(
		!quiet_output.status.success(),
		"run --commands-only should conflict with --quiet"
	);
	let quiet_stderr = stderr_text(&quiet_output);
	assert!(quiet_stderr.contains("cannot be used with"));
	assert!(quiet_stderr.contains("--commands-only"));
	assert!(quiet_stderr.contains("--quiet"));

	let changed_files_output = run_pullhook(temp.path(), &["run", "--commands-only", "--changed-files-only"]);

	assert!(
		!changed_files_output.status.success(),
		"run --commands-only should conflict with --changed-files-only"
	);
	let changed_files_stderr = stderr_text(&changed_files_output);
	assert!(changed_files_stderr.contains("cannot be used with"));
	assert!(changed_files_stderr.contains("--commands-only"));
	assert!(changed_files_stderr.contains("--changed-files-only"));

	let matched_files_output = run_pullhook(temp.path(), &["run", "--commands-only", "--matched-files-only"]);

	assert!(
		!matched_files_output.status.success(),
		"run --commands-only should conflict with --matched-files-only"
	);
	let matched_files_stderr = stderr_text(&matched_files_output);
	assert!(matched_files_stderr.contains("cannot be used with"));
	assert!(matched_files_stderr.contains("--commands-only"));
	assert!(matched_files_stderr.contains("--matched-files-only"));

	let matched_rules_output = run_pullhook(temp.path(), &["run", "--commands-only", "--matched-rules-only"]);

	assert!(
		!matched_rules_output.status.success(),
		"run --commands-only should conflict with --matched-rules-only"
	);
	let matched_rules_stderr = stderr_text(&matched_rules_output);
	assert!(matched_rules_stderr.contains("cannot be used with"));
	assert!(matched_rules_stderr.contains("--commands-only"));
	assert!(matched_rules_stderr.contains("--matched-rules-only"));
}

#[test]
fn run_summary_only_conflicts_with_other_output_modes() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let json_output = run_pullhook(temp.path(), &["run", "--summary-only", "--json"]);

	assert!(
		!json_output.status.success(),
		"run --summary-only should conflict with --json"
	);
	let json_stderr = stderr_text(&json_output);
	assert!(json_stderr.contains("cannot be used with"));
	assert!(json_stderr.contains("--summary-only"));
	assert!(json_stderr.contains("--json"));

	let quiet_output = run_pullhook(temp.path(), &["run", "--summary-only", "--quiet"]);

	assert!(
		!quiet_output.status.success(),
		"run --summary-only should conflict with --quiet"
	);
	let quiet_stderr = stderr_text(&quiet_output);
	assert!(quiet_stderr.contains("cannot be used with"));
	assert!(quiet_stderr.contains("--summary-only"));
	assert!(quiet_stderr.contains("--quiet"));

	let commands_output = run_pullhook(temp.path(), &["run", "--summary-only", "--commands-only"]);

	assert!(
		!commands_output.status.success(),
		"run --summary-only should conflict with --commands-only"
	);
	let commands_stderr = stderr_text(&commands_output);
	assert!(commands_stderr.contains("cannot be used with"));
	assert!(commands_stderr.contains("--summary-only"));
	assert!(commands_stderr.contains("--commands-only"));

	let changed_files_output = run_pullhook(temp.path(), &["run", "--summary-only", "--changed-files-only"]);

	assert!(
		!changed_files_output.status.success(),
		"run --summary-only should conflict with --changed-files-only"
	);
	let changed_files_stderr = stderr_text(&changed_files_output);
	assert!(changed_files_stderr.contains("cannot be used with"));
	assert!(changed_files_stderr.contains("--summary-only"));
	assert!(changed_files_stderr.contains("--changed-files-only"));

	let matched_files_output = run_pullhook(temp.path(), &["run", "--summary-only", "--matched-files-only"]);

	assert!(
		!matched_files_output.status.success(),
		"run --summary-only should conflict with --matched-files-only"
	);
	let matched_files_stderr = stderr_text(&matched_files_output);
	assert!(matched_files_stderr.contains("cannot be used with"));
	assert!(matched_files_stderr.contains("--summary-only"));
	assert!(matched_files_stderr.contains("--matched-files-only"));

	let matched_rules_output = run_pullhook(temp.path(), &["run", "--summary-only", "--matched-rules-only"]);

	assert!(
		!matched_rules_output.status.success(),
		"run --summary-only should conflict with --matched-rules-only"
	);
	let matched_rules_stderr = stderr_text(&matched_rules_output);
	assert!(matched_rules_stderr.contains("cannot be used with"));
	assert!(matched_rules_stderr.contains("--summary-only"));
	assert!(matched_rules_stderr.contains("--matched-rules-only"));
}

#[test]
fn run_changed_files_only_conflicts_with_other_output_modes() {
	let temp = tempfile::tempdir().expect("create temp dir");
	let conflicting_modes: &[&[&str]] = &[
		&["run", "--changed-files-only", "--json"],
		&["run", "--changed-files-only", "--quiet"],
		&["run", "--changed-files-only", "--summary-only"],
		&["run", "--changed-files-only", "--commands-only"],
		&["run", "--changed-files-only", "--matched-files-only"],
		&["run", "--changed-files-only", "--matched-rules-only"],
	];

	for args in conflicting_modes {
		let output = run_pullhook(temp.path(), args);

		assert!(
			!output.status.success(),
			"run --changed-files-only should conflict for args {args:?}"
		);
		let stderr = stderr_text(&output);
		assert!(stderr.contains("cannot be used with"));
		assert!(stderr.contains("--changed-files-only"));
	}
}

#[test]
fn run_matched_files_only_conflicts_with_other_output_modes() {
	let temp = tempfile::tempdir().expect("create temp dir");
	let conflicting_modes: &[&[&str]] = &[
		&["run", "--matched-files-only", "--json"],
		&["run", "--matched-files-only", "--quiet"],
		&["run", "--matched-files-only", "--summary-only"],
		&["run", "--matched-files-only", "--commands-only"],
		&["run", "--matched-files-only", "--changed-files-only"],
		&["run", "--matched-files-only", "--matched-rules-only"],
	];

	for args in conflicting_modes {
		let output = run_pullhook(temp.path(), args);

		assert!(
			!output.status.success(),
			"run --matched-files-only should conflict for args {args:?}"
		);
		let stderr = stderr_text(&output);
		assert!(stderr.contains("cannot be used with"));
		assert!(stderr.contains("--matched-files-only"));
	}
}

#[test]
fn run_matched_rules_only_conflicts_with_other_output_modes() {
	let temp = tempfile::tempdir().expect("create temp dir");
	let conflicting_modes: &[&[&str]] = &[
		&["run", "--matched-rules-only", "--json"],
		&["run", "--matched-rules-only", "--quiet"],
		&["run", "--matched-rules-only", "--summary-only"],
		&["run", "--matched-rules-only", "--commands-only"],
		&["run", "--matched-rules-only", "--changed-files-only"],
		&["run", "--matched-rules-only", "--matched-files-only"],
	];

	for args in conflicting_modes {
		let output = run_pullhook(temp.path(), args);

		assert!(
			!output.status.success(),
			"run --matched-rules-only should conflict for args {args:?}"
		);
		let stderr = stderr_text(&output);
		assert!(stderr.contains("cannot be used with"));
		assert!(stderr.contains("--matched-rules-only"));
	}
}

#[test]
fn explain_summary_only_conflicts_with_json() {
	let temp = tempfile::tempdir().expect("create temp dir");
	let conflicting_modes: &[&[&str]] = &[
		&["explain", "--summary-only", "--json"],
		&["explain", "--summary-only", "--commands-only"],
		&["explain", "--summary-only", "--changed-files-only"],
		&["explain", "--summary-only", "--matched-files-only"],
		&["explain", "--summary-only", "--matched-rules-only"],
	];

	for args in conflicting_modes {
		let output = run_pullhook(temp.path(), args);

		assert!(
			!output.status.success(),
			"explain --summary-only should conflict for args {args:?}"
		);
		let stderr = stderr_text(&output);
		assert!(stderr.contains("cannot be used with"));
		assert!(stderr.contains("--summary-only"));
	}
}

#[test]
fn explain_commands_only_conflicts_with_other_output_modes() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let json_output = run_pullhook(temp.path(), &["explain", "--commands-only", "--json"]);

	assert!(
		!json_output.status.success(),
		"explain --commands-only should conflict with --json"
	);
	let json_stderr = stderr_text(&json_output);
	assert!(json_stderr.contains("cannot be used with"));
	assert!(json_stderr.contains("--commands-only"));
	assert!(json_stderr.contains("--json"));

	let summary_output = run_pullhook(temp.path(), &["explain", "--commands-only", "--summary-only"]);

	assert!(
		!summary_output.status.success(),
		"explain --commands-only should conflict with --summary-only"
	);
	let summary_stderr = stderr_text(&summary_output);
	assert!(summary_stderr.contains("cannot be used with"));
	assert!(summary_stderr.contains("--commands-only"));
	assert!(summary_stderr.contains("--summary-only"));

	let changed_files_output = run_pullhook(temp.path(), &["explain", "--commands-only", "--changed-files-only"]);

	assert!(
		!changed_files_output.status.success(),
		"explain --commands-only should conflict with --changed-files-only"
	);
	let changed_files_stderr = stderr_text(&changed_files_output);
	assert!(changed_files_stderr.contains("cannot be used with"));
	assert!(changed_files_stderr.contains("--commands-only"));
	assert!(changed_files_stderr.contains("--changed-files-only"));

	let matched_files_output = run_pullhook(temp.path(), &["explain", "--commands-only", "--matched-files-only"]);

	assert!(
		!matched_files_output.status.success(),
		"explain --commands-only should conflict with --matched-files-only"
	);
	let matched_files_stderr = stderr_text(&matched_files_output);
	assert!(matched_files_stderr.contains("cannot be used with"));
	assert!(matched_files_stderr.contains("--commands-only"));
	assert!(matched_files_stderr.contains("--matched-files-only"));

	let matched_rules_output = run_pullhook(temp.path(), &["explain", "--commands-only", "--matched-rules-only"]);

	assert!(
		!matched_rules_output.status.success(),
		"explain --commands-only should conflict with --matched-rules-only"
	);
	let matched_rules_stderr = stderr_text(&matched_rules_output);
	assert!(matched_rules_stderr.contains("cannot be used with"));
	assert!(matched_rules_stderr.contains("--commands-only"));
	assert!(matched_rules_stderr.contains("--matched-rules-only"));
}

#[test]
fn explain_changed_files_only_conflicts_with_other_output_modes() {
	let temp = tempfile::tempdir().expect("create temp dir");
	let conflicting_modes: &[&[&str]] = &[
		&["explain", "--changed-files-only", "--json"],
		&["explain", "--changed-files-only", "--summary-only"],
		&["explain", "--changed-files-only", "--commands-only"],
		&["explain", "--changed-files-only", "--matched-files-only"],
		&["explain", "--changed-files-only", "--matched-rules-only"],
	];

	for args in conflicting_modes {
		let output = run_pullhook(temp.path(), args);

		assert!(
			!output.status.success(),
			"`pullhook {}` should reject mixed output modes",
			args.join(" ")
		);
		let stderr = stderr_text(&output);
		assert!(stderr.contains("cannot be used with"));
		assert!(stderr.contains("--changed-files-only"));
	}
}

#[test]
fn explain_matched_files_only_conflicts_with_other_output_modes() {
	let temp = tempfile::tempdir().expect("create temp dir");
	let conflicting_modes: &[&[&str]] = &[
		&["explain", "--matched-files-only", "--json"],
		&["explain", "--matched-files-only", "--summary-only"],
		&["explain", "--matched-files-only", "--commands-only"],
		&["explain", "--matched-files-only", "--changed-files-only"],
		&["explain", "--matched-files-only", "--matched-rules-only"],
	];

	for args in conflicting_modes {
		let output = run_pullhook(temp.path(), args);

		assert!(
			!output.status.success(),
			"`pullhook {}` should reject mixed output modes",
			args.join(" ")
		);
		let stderr = stderr_text(&output);
		assert!(stderr.contains("cannot be used with"));
		assert!(stderr.contains("--matched-files-only"));
	}
}

#[test]
fn explain_matched_rules_only_conflicts_with_other_output_modes() {
	let temp = tempfile::tempdir().expect("create temp dir");
	let conflicting_modes: &[&[&str]] = &[
		&["explain", "--matched-rules-only", "--json"],
		&["explain", "--matched-rules-only", "--summary-only"],
		&["explain", "--matched-rules-only", "--commands-only"],
		&["explain", "--matched-rules-only", "--changed-files-only"],
		&["explain", "--matched-rules-only", "--matched-files-only"],
	];

	for args in conflicting_modes {
		let output = run_pullhook(temp.path(), args);

		assert!(
			!output.status.success(),
			"`pullhook {}` should reject mixed output modes",
			args.join(" ")
		);
		let stderr = stderr_text(&output);
		assert!(stderr.contains("cannot be used with"));
		assert!(stderr.contains("--matched-rules-only"));
	}
}

#[test]
fn config_path_only_conflicts_with_json() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["config", "--path-only", "--json"]);

	assert!(!output.status.success(), "--path-only should conflict with --json");
	let stderr = stderr_text(&output);
	assert!(stderr.contains("cannot be used with"));
	assert!(stderr.contains("--path-only"));
	assert!(stderr.contains("--json"));
}

#[test]
fn validate_quiet_conflicts_with_json() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["validate", "--quiet", "--json"]);

	assert!(!output.status.success(), "--quiet should conflict with --json");
	let stderr = stderr_text(&output);
	assert!(stderr.contains("cannot be used with"));
	assert!(stderr.contains("--quiet"));
	assert!(stderr.contains("--json"));
}

#[test]
fn doctor_quiet_conflicts_with_json() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["doctor", "--quiet", "--json"]);

	assert!(!output.status.success(), "--quiet should conflict with --json");
	let stderr = stderr_text(&output);
	assert!(stderr.contains("cannot be used with"));
	assert!(stderr.contains("--quiet"));
	assert!(stderr.contains("--json"));
}

#[test]
fn init_stdout_conflicts_with_force() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["init", "--stdout", "--force"]);

	assert!(!output.status.success(), "--stdout should conflict with --force");
	let stderr = stderr_text(&output);
	assert!(stderr.contains("cannot be used with"));
	assert!(stderr.contains("--stdout"));
	assert!(stderr.contains("--force"));
}

#[test]
fn init_stdout_conflicts_with_plan_output_modes() {
	let temp = tempfile::tempdir().expect("create temp dir");
	let conflicting_modes: &[&[&str]] = &[&["init", "--stdout", "--dry-run"], &["init", "--stdout", "--json"]];

	for args in conflicting_modes {
		let output = run_pullhook(temp.path(), args);

		assert!(!output.status.success(), "init --stdout should conflict for {args:?}");
		let stderr = stderr_text(&output);
		assert!(stderr.contains("cannot be used with"));
		assert!(stderr.contains("--stdout"));
	}
}

#[test]
fn rules_names_only_conflicts_with_json() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let json_output = run_pullhook(temp.path(), &["rules", "--names-only", "--json"]);

	assert!(
		!json_output.status.success(),
		"--names-only should conflict with --json"
	);
	let json_stderr = stderr_text(&json_output);
	assert!(json_stderr.contains("cannot be used with"));
	assert!(json_stderr.contains("--names-only"));
	assert!(json_stderr.contains("--json"));

	let patterns_output = run_pullhook(temp.path(), &["rules", "--names-only", "--patterns-only"]);

	assert!(
		!patterns_output.status.success(),
		"rules --names-only should conflict with --patterns-only"
	);
	let patterns_stderr = stderr_text(&patterns_output);
	assert!(patterns_stderr.contains("cannot be used with"));
	assert!(patterns_stderr.contains("--names-only"));
	assert!(patterns_stderr.contains("--patterns-only"));
}

#[test]
fn rules_commands_only_conflicts_with_other_output_modes() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let json_output = run_pullhook(temp.path(), &["rules", "--commands-only", "--json"]);

	assert!(
		!json_output.status.success(),
		"rules --commands-only should conflict with --json"
	);
	let json_stderr = stderr_text(&json_output);
	assert!(json_stderr.contains("cannot be used with"));
	assert!(json_stderr.contains("--commands-only"));
	assert!(json_stderr.contains("--json"));

	let names_output = run_pullhook(temp.path(), &["rules", "--commands-only", "--names-only"]);

	assert!(
		!names_output.status.success(),
		"rules --commands-only should conflict with --names-only"
	);
	let names_stderr = stderr_text(&names_output);
	assert!(names_stderr.contains("cannot be used with"));
	assert!(names_stderr.contains("--commands-only"));
	assert!(names_stderr.contains("--names-only"));

	let patterns_output = run_pullhook(temp.path(), &["rules", "--commands-only", "--patterns-only"]);

	assert!(
		!patterns_output.status.success(),
		"rules --commands-only should conflict with --patterns-only"
	);
	let patterns_stderr = stderr_text(&patterns_output);
	assert!(patterns_stderr.contains("cannot be used with"));
	assert!(patterns_stderr.contains("--commands-only"));
	assert!(patterns_stderr.contains("--patterns-only"));
}

#[test]
fn rules_patterns_only_conflicts_with_other_output_modes() {
	let temp = tempfile::tempdir().expect("create temp dir");
	let conflicting_modes: &[&[&str]] = &[
		&["rules", "--patterns-only", "--json"],
		&["rules", "--patterns-only", "--names-only"],
		&["rules", "--patterns-only", "--commands-only"],
	];

	for args in conflicting_modes {
		let output = run_pullhook(temp.path(), args);

		assert!(
			!output.status.success(),
			"rules --patterns-only should conflict for args {args:?}"
		);
		let stderr = stderr_text(&output);
		assert!(stderr.contains("cannot be used with"));
		assert!(stderr.contains("--patterns-only"));
	}
}

#[test]
fn init_help_lists_generation_examples() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["init", "--help"]);

	assert!(output.status.success(), "init help should succeed");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("pullhook init --stdout"));
	assert!(stdout.contains("pullhook init --force"));
	assert!(stdout.contains("pullhook init --format yaml"));
	assert!(stdout.contains("pullhook init --output config/pullhook.custom.json"));
	assert!(stdout.contains("pullhook init --dry-run --json"));
	assert!(stdout.contains("Generation options:"));
	assert!(stdout.contains("Output options:"));
	assert!(stdout.contains("Write options:"));
	assert!(stdout.contains("Display options:"));
	assert!(stdout.contains("--dry-run"));
	assert!(stdout.contains("--json"));
}

#[test]
fn utility_help_groups_options_by_task() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let schema = run_pullhook(temp.path(), &["schema", "--help"]);
	assert!(schema.status.success(), "schema help should succeed");
	let schema_stdout = stdout_text(&schema);
	assert!(schema_stdout.contains("Output options:"));
	assert!(schema_stdout.contains("Check options:"));

	let completion = run_pullhook(temp.path(), &["completion", "fish", "--help"]);
	assert!(completion.status.success(), "completion help should succeed");
	let completion_stdout = stdout_text(&completion);
	assert!(completion_stdout.contains("Output options:"));
	assert!(completion_stdout.contains("Check options:"));

	let codes = run_pullhook(temp.path(), &["codes", "--help"]);
	assert!(codes.status.success(), "codes help should succeed");
	let codes_stdout = stdout_text(&codes);
	assert!(codes_stdout.contains("Filter options:"));
	assert!(codes_stdout.contains("Output options:"));
}

#[test]
fn schema_prints_json_schema() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["schema"]);

	assert!(output.status.success(), "schema command should succeed");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse schema json");
	assert_eq!(value["$id"], "https://pullhook.dev/schema.json");
	assert_eq!(value["properties"]["rules"]["type"], "array");
	assert_eq!(value["$defs"]["rule"]["properties"]["run"]["type"], "string");
	assert_eq!(
		value["$defs"]["rule"]["oneOf"][0]["not"]["properties"]["install"]["const"],
		true
	);
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "schema should not write stderr");
}

#[test]
fn schema_can_write_to_output_file() {
	let temp = tempfile::tempdir().expect("create temp dir");
	let output_path = temp.path().join(".vscode/pullhook.schema.json");

	let output = run_pullhook(temp.path(), &["schema", "--output", ".vscode/pullhook.schema.json"]);

	assert!(output.status.success(), "schema --output should succeed");
	assert!(
		stdout_text(&output).trim().is_empty(),
		"schema --output should not write stdout"
	);
	let schema = fs::read_to_string(output_path).expect("read schema output");
	let value: serde_json::Value = serde_json::from_str(&schema).expect("parse written schema");
	assert_eq!(value["$id"], "https://pullhook.dev/schema.json");
}

#[test]
fn schema_check_succeeds_when_output_is_current() {
	let temp = tempfile::tempdir().expect("create temp dir");
	let output_path = temp.path().join(".vscode/pullhook.schema.json");
	let write = run_pullhook(temp.path(), &["schema", "--output", ".vscode/pullhook.schema.json"]);
	assert!(write.status.success(), "schema --output should seed check file");

	let output = run_pullhook(
		temp.path(),
		&["schema", "--check", "--output", ".vscode/pullhook.schema.json"],
	);

	assert!(output.status.success(), "schema --check should pass for current schema");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("schema up to date"));
	assert!(stdout.contains(".vscode/pullhook.schema.json"));
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "schema --check should not write stderr");
	assert!(predicate::path::is_file().eval(&output_path));
}

#[test]
fn schema_check_fails_when_output_is_stale() {
	let temp = tempfile::tempdir().expect("create temp dir");
	write_file(temp.path(), Path::new(".vscode/pullhook.schema.json"), "{}\n");

	let output = run_pullhook(
		temp.path(),
		&["schema", "--check", "--output", ".vscode/pullhook.schema.json"],
	);

	assert!(!output.status.success(), "schema --check should fail for stale schema");
	assert!(
		stdout_text(&output).trim().is_empty(),
		"stale schema check should not write stdout"
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("schema out of date"));
	assert!(stderr.contains("pullhook schema --output .vscode/pullhook.schema.json"));
}

#[test]
fn schema_check_json_reports_match_status() {
	let temp = tempfile::tempdir().expect("create temp dir");
	write_file(temp.path(), Path::new(".vscode/pullhook.schema.json"), "{}\n");

	let output = run_pullhook(
		temp.path(),
		&[
			"schema",
			"--check",
			"--output",
			".vscode/pullhook.schema.json",
			"--json",
		],
	);

	assert!(
		!output.status.success(),
		"schema --check --json should fail for stale schema"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse schema check json");
	assert_eq!(value["status"], "error");
	assert_eq!(value["code"], "schema_out_of_date");
	assert!(
		value["path"]
			.as_str()
			.expect("path")
			.ends_with(".vscode/pullhook.schema.json")
	);
	assert_eq!(value["exists"], true);
	assert_eq!(value["matches"], false);
	assert_eq!(value["error"], "schema output is out of date");
	let details = value["details"].as_array().expect("details array");
	assert!(details.iter().any(|detail| {
		detail
			.as_str()
			.expect("detail")
			.contains("pullhook schema --output .vscode/pullhook.schema.json")
	}));
	let stderr = stderr_text(&output);
	assert!(stderr.contains("schema out of date"));
}

#[test]
fn schema_check_json_reports_up_to_date_status() {
	let temp = tempfile::tempdir().expect("create temp dir");
	let schema_path = ".vscode/pullhook.schema.json";

	let write_output = run_pullhook(temp.path(), &["schema", "--output", schema_path]);
	assert!(write_output.status.success(), "schema --output should write schema");

	let output = run_pullhook(temp.path(), &["schema", "--check", "--output", schema_path, "--json"]);

	assert!(
		output.status.success(),
		"schema --check --json should pass for current schema"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse schema check json");
	assert_eq!(value["status"], "ok");
	assert_eq!(value["code"], serde_json::Value::Null);
	assert_eq!(value["error"], serde_json::Value::Null);
	assert_eq!(value["exists"], true);
	assert_eq!(value["matches"], true);
	assert_eq!(value["details"], serde_json::json!([]));
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"up-to-date schema check should not write stderr"
	);
}

#[test]
fn completion_command_rejects_run_arguments() {
	let output = run_pullhook(Path::new("."), &["--install", "completion", "bash"]);

	assert!(!output.status.success(), "mixed run and completion args should fail");

	let stdout = stdout_text(&output);
	let stderr = stderr_text(&output);
	assert!(
		stdout.trim().is_empty(),
		"argument parsing failure should not write stdout"
	);
	assert!(stderr.contains("cannot be used with"));
	assert!(stderr.contains("completion"));
}

#[test]
fn init_creates_starter_config() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook_with_env(
		repo_root,
		&["init", "--render", "never"],
		&[("PULLHOOK_RENDER_MODE", "never")],
	);

	assert!(output.status.success(), "init should succeed");
	assert!(predicate::path::is_file().eval(&repo_root.join("pullhook.json")));
	let config = fs::read_to_string(repo_root.join("pullhook.json")).expect("read config");
	assert!(config.contains("\"rules\""));
	assert!(config.contains("\"install\""));
}

#[test]
fn init_can_generate_yaml_starter_config() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(repo_root, &["init", "--format", "yaml", "--render", "never"]);

	assert!(output.status.success(), "yaml init should succeed");
	assert!(predicate::path::is_file().eval(&repo_root.join("pullhook.yaml")));
	assert!(!predicate::path::is_file().eval(&repo_root.join("pullhook.json")));
	let config = fs::read_to_string(repo_root.join("pullhook.yaml")).expect("read yaml config");
	assert!(config.contains("rules:"));
	assert!(config.contains("install: true"));
}

#[test]
fn init_can_write_starter_config_to_explicit_output_path() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(
		repo_root,
		&["init", "--output", "config/pullhook.custom.json", "--render", "never"],
	);

	assert!(output.status.success(), "explicit output init should succeed");
	let config_path = repo_root.join("config/pullhook.custom.json");
	assert!(predicate::path::is_file().eval(&config_path));
	let config = fs::read_to_string(config_path).expect("read config");
	assert!(config.contains("\"rules\""));
	assert!(config.contains("\"install\""));
	assert!(!predicate::path::is_file().eval(&repo_root.join("pullhook.json")));
}

#[test]
fn init_dry_run_previews_default_config_without_writing_file() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(repo_root, &["init", "--dry-run", "--render", "never"]);

	assert!(output.status.success(), "init --dry-run should succeed");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("would create"));
	assert!(stdout.contains("pullhook.json"));
	assert!(stdout.contains("format: json"));
	assert!(!predicate::path::is_file().eval(&repo_root.join("pullhook.json")));
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "init --dry-run should not write stderr");
}

#[test]
fn init_dry_run_json_reports_plan_without_writing_file() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(repo_root, &["init", "--format", "yaml", "--dry-run", "--json"]);

	assert!(output.status.success(), "init --dry-run --json should succeed");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse init json");
	assert_eq!(value["status"], "ok");
	assert_eq!(value["code"], serde_json::Value::Null);
	assert!(value["path"].as_str().expect("path").ends_with("pullhook.yaml"));
	assert_eq!(value["format"], "yaml");
	assert_eq!(value["existed"], false);
	assert_eq!(value["force"], false);
	assert_eq!(value["dryRun"], true);
	assert_eq!(value["action"], "create");
	assert_eq!(value["written"], false);
	assert_eq!(value["error"], serde_json::Value::Null);
	let details = value["details"].as_array().expect("details array");
	assert!(
		details
			.iter()
			.any(|detail| { detail.as_str().expect("detail").contains("pullhook init --output") })
	);
	assert!(
		details
			.iter()
			.any(|detail| { detail.as_str().expect("detail").contains("--format yaml") })
	);
	assert!(!predicate::path::is_file().eval(&repo_root.join("pullhook.yaml")));
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"init --dry-run --json should not write stderr"
	);
}

#[test]
fn init_json_reports_created_config() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(repo_root, &["init", "--json"]);

	assert!(output.status.success(), "init --json should succeed");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse init json");
	assert_eq!(value["status"], "ok");
	assert_eq!(value["code"], serde_json::Value::Null);
	assert!(value["path"].as_str().expect("path").ends_with("pullhook.json"));
	assert_eq!(value["format"], "json");
	assert_eq!(value["action"], "create");
	assert_eq!(value["written"], true);
	assert_eq!(value["details"], serde_json::json!([]));
	assert!(predicate::path::is_file().eval(&repo_root.join("pullhook.json")));
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "init --json should not write stderr");
}

#[test]
fn init_stdout_prints_requested_format_without_writing_file() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(repo_root, &["init", "--format", "jsonc", "--stdout"]);

	assert!(output.status.success(), "stdout init should succeed");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("// pullhook runs each rule once from the repo root"));
	assert!(stdout.contains("\"install\": true"));
	assert!(!predicate::path::is_file().eval(&repo_root.join("pullhook.json")));
	assert!(!predicate::path::is_file().eval(&repo_root.join("pullhook.jsonc")));
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "stdout init should not write stderr");
}

#[test]
fn init_stdout_succeeds_outside_git_repo() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["init", "--stdout"]);

	assert!(output.status.success(), "stdout init should not require a git repo");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("\"rules\""));
	assert!(!predicate::path::is_file().eval(&temp.path().join("pullhook.json")));
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "stdout init should not write stderr");
}

#[test]
fn config_json_reports_discovered_config_path_without_validating_contents() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(repo_root, Path::new("pullhook.json"), "{not valid json}\n");

	let output = run_pullhook(repo_root, &["config", "--json"]);

	assert!(
		output.status.success(),
		"config --json should not require a valid config body"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse config json");
	assert_eq!(value["status"], "ok");
	assert_eq!(value["code"], serde_json::Value::Null);
	assert!(value["path"].as_str().expect("path").ends_with("pullhook.json"));
	assert_eq!(value["format"], "json");
	assert_eq!(value["exists"], true);
	assert_eq!(value["explicit"], false);
	assert_eq!(value["source"], "discovered");
	assert_eq!(value["error"], serde_json::Value::Null);
	assert!(
		value["repoRoot"]
			.as_str()
			.expect("repo root")
			.ends_with(repo_root.file_name().unwrap().to_str().unwrap())
	);
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "config --json should not write stderr");
}

#[test]
fn config_path_only_reports_discovered_config_path() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(repo_root, Path::new("pullhook.json"), "{not valid json}\n");

	let output = run_pullhook(repo_root, &["config", "--path-only"]);

	assert!(
		output.status.success(),
		"config --path-only should not require a valid config body"
	);
	let stdout = stdout_text(&output);
	assert!(stdout.trim_end().ends_with("pullhook.json"));
	assert_eq!(stdout.lines().count(), 1);
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "config --path-only should not write stderr");
}

#[test]
fn config_text_reports_explicit_missing_config_path() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(
		repo_root,
		&["config", "--config", "config/pullhook.custom.yaml", "--render", "never"],
	);

	assert!(
		output.status.success(),
		"config should describe explicit paths before they exist"
	);
	let stdout = stdout_text(&output);
	assert!(stdout.contains("config/pullhook.custom.yaml"));
	assert!(stdout.contains("format: yaml"));
	assert!(stdout.contains("exists: no"));
	assert!(stdout.contains("source: explicit"));
}

#[test]
fn config_require_existing_rejects_explicit_missing_config_path() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(
		repo_root,
		&[
			"config",
			"--config",
			"config/pullhook.custom.yaml",
			"--require-existing",
			"--path-only",
		],
	);

	assert!(
		!output.status.success(),
		"config --require-existing should reject missing paths"
	);
	let stdout = stdout_text(&output);
	assert!(
		stdout.trim().is_empty(),
		"failed path-only config should not write stdout"
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("resolved config file does not exist"));
	assert!(stderr.contains("config/pullhook.custom.yaml"));
}

#[test]
fn config_require_existing_json_reports_missing_config_path_as_json() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(
		repo_root,
		&[
			"config",
			"--config",
			"config/pullhook.custom.yaml",
			"--require-existing",
			"--json",
		],
	);

	assert!(
		!output.status.success(),
		"config --require-existing --json should reject missing paths"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse config error json");
	assert_eq!(value["status"], "error");
	assert_eq!(value["code"], "config_path_missing");
	assert_eq!(value["format"], "yaml");
	assert_eq!(value["exists"], false);
	assert_eq!(value["explicit"], true);
	assert_eq!(value["source"], "explicit");
	assert_eq!(value["error"], "resolved config file does not exist");
	let details = value["details"].as_array().expect("details array");
	assert!(
		details
			.iter()
			.any(|detail| { detail.as_str().expect("detail").contains("config/pullhook.custom.yaml") })
	);
	assert!(
		details
			.iter()
			.any(|detail| { detail.as_str().expect("detail").contains("pullhook init --output") })
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("resolved config file does not exist"));
}

#[test]
fn config_json_reports_unsupported_config_extension_as_json() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(repo_root, &["config", "--config", "pullhook.json5", "--json"]);

	assert!(
		!output.status.success(),
		"config --json should reject unsupported config extensions as JSON"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse config error json");
	assert_eq!(value["status"], "error");
	assert!(
		value["error"]
			.as_str()
			.expect("error")
			.contains("JSON5 configs are not supported")
	);
	assert_eq!(value["configPathError"]["kind"], "unsupported_format");
	let reported_path = PathBuf::from(value["configPathError"]["path"].as_str().expect("config path"));
	assert_eq!(
		reported_path
			.parent()
			.expect("reported parent")
			.canonicalize()
			.expect("canonicalize reported parent"),
		repo_root.canonicalize().expect("canonicalize repo root")
	);
	assert_eq!(
		reported_path.file_name().and_then(|name| name.to_str()),
		Some("pullhook.json5")
	);
	assert_eq!(value["configPathError"]["extension"], "json5");
	assert_eq!(
		value["configPathError"]["reason"],
		"JSON5 configs are not supported; use `pullhook.json` or `pullhook.jsonc`"
	);
	assert_eq!(
		value["configPathError"]["supported"],
		serde_json::json!([
			"pullhook.json",
			"pullhook.jsonc",
			"pullhook.yaml",
			"pullhook.toml",
			".pullhook.json",
			".pullhook.jsonc",
			".pullhook.yaml",
			".pullhook.toml"
		])
	);
	assert_eq!(
		value["details"],
		serde_json::json!([
			"supported config files are `pullhook.json`, `pullhook.jsonc`, `pullhook.yaml`, and `.pullhook.toml`",
			"use `pullhook init --output <path>` to create a supported config file"
		])
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("JSON5 configs are not supported"));
}

#[test]
fn init_refuses_to_overwrite_existing_config_without_force() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(repo_root, Path::new("pullhook.json"), "{\"rules\":[]}\n");

	let output = run_pullhook(repo_root, &["init", "--render", "never"]);

	assert!(!output.status.success(), "init should reject overwriting config");
	let stderr = stderr_text(&output);
	assert!(stderr.contains("refusing to overwrite existing file"));
	assert!(stderr.contains("--force"));
}

#[test]
fn init_json_reports_existing_config_error() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(repo_root, Path::new("pullhook.json"), "{\"rules\":[]}\n");

	let output = run_pullhook(repo_root, &["init", "--json"]);

	assert!(!output.status.success(), "init --json should reject overwriting config");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse init json");
	assert_eq!(value["status"], "error");
	assert_eq!(value["code"], "init_refusing_overwrite");
	assert!(value["path"].as_str().expect("path").ends_with("pullhook.json"));
	assert_eq!(value["format"], "json");
	assert_eq!(value["existed"], true);
	assert_eq!(value["force"], false);
	assert_eq!(value["dryRun"], false);
	assert_eq!(value["action"], "overwrite");
	assert_eq!(value["written"], false);
	assert!(
		value["error"]
			.as_str()
			.expect("error")
			.contains("refusing to overwrite")
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("refusing to overwrite existing file"));
}

#[test]
fn init_refuses_to_overwrite_explicit_output_without_force() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(repo_root, Path::new("config/pullhook.custom.json"), "{}\n");

	let output = run_pullhook(
		repo_root,
		&["init", "--output", "config/pullhook.custom.json", "--render", "never"],
	);

	assert!(!output.status.success(), "explicit output init should refuse overwrite");
	let stderr = stderr_text(&output);
	assert!(stderr.contains("refusing to overwrite existing file"));
	assert!(stderr.contains("pullhook init --force"));
}

#[test]
fn init_rejects_mismatched_explicit_format_and_output_extension() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(
		repo_root,
		&["init", "--format", "yaml", "--output", "config/pullhook.custom.json"],
	);

	assert!(
		!output.status.success(),
		"explicit output init should reject mismatched format"
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("uses pullhook.json"));
	assert!(stderr.contains("matching `--format`"));
}

#[test]
fn init_json_reports_mismatched_explicit_format_and_output_extension() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(
		repo_root,
		&[
			"init",
			"--format",
			"yaml",
			"--output",
			"config/pullhook.custom.json",
			"--json",
		],
	);

	assert!(
		!output.status.success(),
		"init --json should reject mismatched format as JSON"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse init error json");
	assert_eq!(value["status"], "error");
	let error = value["error"].as_str().expect("error");
	assert!(error.contains("uses pullhook.json"));
	assert!(error.contains("matching `--format`"));
	let stderr = stderr_text(&output);
	assert!(stderr.contains("uses pullhook.json"));
}

#[test]
fn init_force_overwrites_existing_config_in_place() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(repo_root, Path::new("pullhook.json"), "{\"rules\":[]}\n");

	let output = run_pullhook(repo_root, &["init", "--force", "--render", "never"]);

	assert!(output.status.success(), "forced init should succeed");
	let config = fs::read_to_string(repo_root.join("pullhook.json")).expect("read config");
	assert!(config.contains("\"install\": true"));
	assert!(config.contains("\"onFailure\": \"stop\""));
}

#[test]
fn init_force_json_reports_overwritten_config() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(repo_root, Path::new("pullhook.json"), "{\"rules\":[]}\n");

	let output = run_pullhook(repo_root, &["init", "--force", "--json"]);

	assert!(output.status.success(), "init --force --json should succeed");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse init json");
	assert_eq!(value["status"], "ok");
	assert!(value["path"].as_str().expect("path").ends_with("pullhook.json"));
	assert_eq!(value["format"], "json");
	assert_eq!(value["existed"], true);
	assert_eq!(value["force"], true);
	assert_eq!(value["action"], "overwrite");
	assert_eq!(value["written"], true);
	let config = fs::read_to_string(repo_root.join("pullhook.json")).expect("read config");
	assert!(config.contains("\"install\": true"));
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "init --force --json should not write stderr");
}

#[test]
fn init_force_rejects_format_change_when_config_already_exists() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(repo_root, Path::new("pullhook.json"), "{\"rules\":[]}\n");

	let output = run_pullhook(repo_root, &["init", "--force", "--format", "yaml", "--render", "never"]);

	assert!(
		!output.status.success(),
		"forced init should reject format changes in place"
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("existing config"));
	assert!(stderr.contains("rerun without `--format`"));
}

#[test]
fn validate_reports_invalid_fail_text() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "bad copy",
      "changed": "**/*.rs",
      "run": "cargo test",
      "failText": "{sparkle nope}"
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["validate", "--render", "never"]);

	assert!(!output.status.success(), "invalid config should fail");
	let stderr = stderr_text(&output);
	assert!(stderr.contains("unknown style `sparkle`"));
	assert!(stderr.contains("pullhook.json"));
}

#[test]
fn validate_quiet_suppresses_success_output() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "write marker", "packages/a/package-lock.json", "true");

	let output = run_pullhook(repo_root, &["validate", "--quiet"]);

	assert!(output.status.success(), "validate --quiet should succeed");
	let stdout = stdout_text(&output);
	assert!(stdout.trim().is_empty(), "quiet validate should not write stdout");
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "quiet validate should not write stderr");
}

#[test]
fn validate_json_reports_config_summary() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "install dependencies",
      "install": true
    },
    {
      "name": "parallel checks",
      "parallel": [
        {
          "name": "typecheck",
          "changed": "packages/*/package-lock.json",
          "run": "cargo test"
        }
      ]
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["validate", "--json"]);

	assert!(output.status.success(), "validate --json should succeed");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse validate json");
	assert_eq!(value["status"], "ok");
	assert_eq!(value["code"], serde_json::Value::Null);
	assert_eq!(value["valid"], true);
	assert_eq!(value["entries"], 2);
	assert_eq!(value["rules"], 2);
	assert_eq!(value["parallelGroups"], 1);
	assert_eq!(value["onFailure"], "stop");
	assert_eq!(value["error"], serde_json::Value::Null);
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "validate --json should not write stderr");
}

#[test]
fn validate_json_reports_invalid_config_as_json() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "bad copy",
      "changed": "**/*.rs",
      "run": "cargo test",
      "failText": "{sparkle nope}"
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["validate", "--json"]);

	assert!(!output.status.success(), "invalid config should fail");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse validate json");
	assert_eq!(value["status"], "error");
	assert_eq!(value["code"], "config_validation");
	assert_eq!(value["valid"], false);
	assert!(value["path"].as_str().expect("path").ends_with("pullhook.json"));
	assert!(
		value["error"]
			.as_str()
			.expect("error")
			.contains("unknown style `sparkle`")
	);
	let details = value["details"].as_array().expect("details array");
	assert!(details.iter().any(|detail| {
		detail
			.as_str()
			.expect("detail")
			.contains("rules[0]: invalid `failText`: unknown style `sparkle`")
	}));
	assert_eq!(value["validationErrors"], value["details"]);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("config invalid"));
}

#[test]
fn validate_json_reports_config_parse_errors_as_json() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(repo_root, Path::new("pullhook.json"), r#"{ "rules": [ "#);

	let output = run_pullhook(repo_root, &["validate", "--json"]);

	assert!(!output.status.success(), "unparseable config should fail");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse validate json");
	assert_eq!(value["status"], "error");
	assert_eq!(value["code"], "config_missing");
	assert_eq!(value["valid"], false);
	assert!(value["path"].as_str().expect("path").ends_with("pullhook.json"));
	assert!(
		value["error"]
			.as_str()
			.expect("error")
			.contains("failed to parse config")
	);
	assert!(
		value["parseError"]["path"]
			.as_str()
			.expect("parse path")
			.ends_with("pullhook.json")
	);
	assert!(!value["parseError"]["reason"].as_str().expect("parse reason").is_empty());
	let details = value["details"].as_array().expect("details array");
	assert!(
		details
			.iter()
			.any(|detail| detail.as_str().expect("detail") == value["parseError"]["reason"])
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("config invalid"));
}

#[test]
fn validate_json_reports_missing_config_as_json() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(repo_root, &["validate", "--json"]);

	assert!(!output.status.success(), "missing config should fail");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse validate json");
	assert_eq!(value["status"], "error");
	assert_eq!(value["valid"], false);
	assert!(value["path"].is_null());
	assert!(
		value["error"]
			.as_str()
			.expect("error")
			.contains("no pullhook config found")
	);
	assert_eq!(value["configDiscoveryError"]["kind"], "missing");
	let reported_root = PathBuf::from(
		value["configDiscoveryError"]["repoRoot"]
			.as_str()
			.expect("config repo root"),
	);
	assert_eq!(
		reported_root.canonicalize().expect("canonicalize reported root"),
		repo_root.canonicalize().expect("canonicalize repo root")
	);
	assert_eq!(value["configDiscoveryError"]["defaultConfig"], "pullhook.json");
	assert_eq!(
		value["details"],
		serde_json::json!([
			"run `pullhook init` to create pullhook.json",
			"use `--config <path>` to point at a custom config file"
		])
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("no pullhook config found"));
}

#[test]
fn run_json_reports_missing_config_as_json() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(repo_root, &["run", "--json"]);

	assert!(!output.status.success(), "run --json should fail without config");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse run error json");
	assert_eq!(value["status"], "error");
	assert!(
		value["error"]
			.as_str()
			.expect("error")
			.contains("no pullhook config found")
	);
	assert_eq!(value["configDiscoveryError"]["kind"], "missing");
	let reported_root = PathBuf::from(
		value["configDiscoveryError"]["repoRoot"]
			.as_str()
			.expect("config repo root"),
	);
	assert_eq!(
		reported_root.canonicalize().expect("canonicalize reported root"),
		repo_root.canonicalize().expect("canonicalize repo root")
	);
	assert_eq!(value["configDiscoveryError"]["defaultConfig"], "pullhook.json");
	assert_eq!(
		value["details"],
		serde_json::json!([
			"run `pullhook init` to create pullhook.json",
			"use `--config <path>` to point at a custom config file"
		])
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("no pullhook config found"));
}

#[test]
fn explain_json_reports_missing_config_as_json() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(repo_root, &["explain", "--json"]);

	assert!(!output.status.success(), "explain --json should fail without config");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse explain error json");
	assert_eq!(value["status"], "error");
	assert!(
		value["error"]
			.as_str()
			.expect("error")
			.contains("no pullhook config found")
	);
	assert_eq!(
		value["details"],
		serde_json::json!([
			"run `pullhook init` to create pullhook.json",
			"use `--config <path>` to point at a custom config file"
		])
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("no pullhook config found"));
}

#[test]
fn run_json_reports_diff_base_errors_as_json() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "write marker", "packages/a/package-lock.json", "true");

	let output = run_pullhook(repo_root, &["run", "--base", "missing-base-ref", "--json"]);

	assert!(!output.status.success(), "run --json should fail for an invalid base");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse run base error json");
	assert_eq!(value["status"], "error");
	let error = value["error"].as_str().expect("error");
	assert!(error.contains("failed to resolve diff base or read changed files"));
	assert_eq!(value["diffBaseError"]["kind"], "revision_not_found");
	assert_eq!(value["diffBaseError"]["revision"], "missing-base-ref");
	let details = value["details"].as_array().expect("details array");
	assert!(details.iter().any(|detail| {
		detail
			.as_str()
			.expect("detail")
			.contains("check that `--base <rev>` names a commit")
	}));
	assert!(details.iter().any(|detail| {
		detail
			.as_str()
			.expect("detail")
			.contains("automatic diff-base fallback")
	}));
	let stderr = stderr_text(&output);
	assert!(stderr.contains("failed to resolve diff base or read changed files"));
}

#[test]
fn explain_json_reports_diff_base_errors_as_json() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "write marker", "packages/a/package-lock.json", "true");

	let output = run_pullhook(repo_root, &["explain", "--base", "missing-base-ref", "--json"]);

	assert!(
		!output.status.success(),
		"explain --json should fail for an invalid base"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse explain base error json");
	assert_eq!(value["status"], "error");
	let error = value["error"].as_str().expect("error");
	assert!(error.contains("failed to resolve diff base or read changed files"));
	assert_eq!(value["diffBaseError"]["kind"], "revision_not_found");
	assert_eq!(value["diffBaseError"]["revision"], "missing-base-ref");
	let stderr = stderr_text(&output);
	assert!(stderr.contains("failed to resolve diff base or read changed files"));
}

#[test]
fn rules_json_reports_missing_config_as_json() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(repo_root, &["rules", "--json"]);

	assert!(!output.status.success(), "rules --json should fail without config");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse rules error json");
	assert_eq!(value["status"], "error");
	assert_eq!(value["code"], "config_missing");
	assert!(
		value["error"]
			.as_str()
			.expect("error")
			.contains("no pullhook config found")
	);
	assert_eq!(
		value["details"],
		serde_json::json!([
			"run `pullhook init` to create pullhook.json",
			"use `--config <path>` to point at a custom config file"
		])
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("no pullhook config found"));
}

#[test]
fn validate_json_rejects_duplicate_selector_names() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "checks",
      "parallel": [
        {
          "name": "lint",
          "changed": "packages/a/package-lock.json",
          "run": "cargo test -p lint"
        }
      ]
    },
    {
      "name": "lint",
      "changed": "packages/a/package-lock.json",
      "run": "cargo test -p lint-again"
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["validate", "--json"]);

	assert!(!output.status.success(), "duplicate selector names should fail");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse validate json");
	assert_eq!(value["valid"], false);
	let error = value["error"].as_str().expect("error");
	assert!(error.contains("rule selector names must be unique"));
	assert!(error.contains("duplicate selector(s): `lint`"));
}

#[test]
fn doctor_json_reports_repo_config_diff_base_and_install_detection() {
	let temp = setup_repo_with_root_manifest_change();
	let repo_root = temp.path();
	write_config_rule(repo_root, "rebuild root", "package-lock.json", "npm test");

	let output = run_pullhook(repo_root, &["doctor", "--json"]);

	assert!(output.status.success(), "doctor --json should succeed");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse doctor json");
	assert_eq!(value["status"], "ok");
	assert_eq!(value["code"], serde_json::Value::Null);
	assert_eq!(value["error"], serde_json::Value::Null);
	assert_eq!(value["strict"], false);
	assert_eq!(value["summary"]["ok"], 4);
	assert_eq!(value["summary"]["warn"], 0);
	assert_eq!(value["summary"]["error"], 0);
	assert_eq!(value["summary"]["allOk"], true);
	assert_eq!(value["summary"]["hasWarnings"], false);
	assert_eq!(value["summary"]["hasErrors"], false);
	let checks = value["checks"].as_array().expect("checks array");
	assert_eq!(checks.len(), 4);
	assert_eq!(checks[0]["name"], "repository");
	assert_eq!(checks[1]["name"], "config");
	assert_eq!(checks[2]["name"], "diff base");
	assert_eq!(checks[3]["name"], "install detection");
	assert_eq!(checks[3]["summary"], "detected npm");
	assert_eq!(
		checks[1]["hint"],
		"run `pullhook explain --all-matches` to preview rule matches"
	);
	assert_eq!(checks[3]["hint"], "use `install: true` for dependency-recovery rules");
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "doctor --json should not write stderr");
}

#[test]
fn doctor_quiet_suppresses_all_ok_output() {
	let temp = setup_repo_with_root_manifest_change();
	let repo_root = temp.path();
	write_config_rule(repo_root, "rebuild root", "package-lock.json", "npm test");

	let output = run_pullhook(repo_root, &["doctor", "--quiet"]);

	assert!(
		output.status.success(),
		"doctor --quiet should succeed when all checks pass"
	);
	let stdout = stdout_text(&output);
	assert!(stdout.trim().is_empty(), "quiet all-ok doctor should not write stdout");
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "quiet all-ok doctor should not write stderr");
}

#[test]
fn doctor_quiet_still_reports_warnings() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(repo_root, &["doctor", "--quiet", "--render", "never"]);

	assert!(
		output.status.success(),
		"doctor warnings should stay non-blocking without --strict"
	);
	let stdout = stdout_text(&output);
	assert!(stdout.contains("Doctor"));
	assert!(stdout.contains("[warn] config"));
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"non-strict doctor warnings should not write stderr"
	);
}

#[test]
fn doctor_strict_fails_on_warnings() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(repo_root, &["doctor", "--strict", "--json"]);

	assert!(!output.status.success(), "doctor --strict should fail on warnings");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse doctor json");
	assert_eq!(value["status"], "error");
	assert_eq!(value["code"], "doctor_warnings");
	assert_eq!(value["error"], "doctor found warnings in strict mode");
	assert_eq!(value["strict"], true);
	let warn_count = value["summary"]["warn"].as_u64().expect("warn count");
	assert!(warn_count >= 1, "strict doctor should report at least one warning");
	assert_eq!(value["summary"]["error"], 0);
	assert_eq!(value["summary"]["allOk"], false);
	assert_eq!(value["summary"]["hasWarnings"], true);
	assert_eq!(value["summary"]["hasErrors"], false);
	let checks = value["checks"].as_array().expect("checks array");
	assert_eq!(checks[1]["name"], "config");
	assert_eq!(checks[1]["code"], "config_missing");
	assert_eq!(checks[1]["level"], "warn");
	assert_eq!(checks[2]["code"], "diff_base_ok");
	assert_eq!(checks[3]["code"], "package_manager_missing");
	let stderr = stderr_text(&output);
	assert!(stderr.contains("doctor found warnings in strict mode"));
}

#[test]
fn doctor_json_fails_when_config_is_invalid() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "bad copy",
      "changed": "**/*.rs",
      "run": "cargo test",
      "failText": "{sparkle nope}"
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["doctor", "--json"]);

	assert!(!output.status.success(), "doctor should fail for invalid config");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse doctor json");
	assert_eq!(value["status"], "error");
	assert_eq!(value["code"], "doctor_blocking_issues");
	assert_eq!(value["error"], "doctor found blocking issues");
	assert_eq!(value["summary"]["error"], 1);
	assert_eq!(value["summary"]["allOk"], false);
	assert_eq!(value["summary"]["hasErrors"], true);
	let checks = value["checks"].as_array().expect("checks array");
	assert_eq!(checks[1]["name"], "config");
	assert_eq!(checks[1]["code"], "config_invalid");
	assert_eq!(checks[1]["level"], "error");
	assert_eq!(checks[1]["hint"], "run `pullhook validate` after editing the config");
	let details = checks[1]["details"].as_array().expect("config details");
	assert!(details.iter().any(|detail| {
		detail
			.as_str()
			.expect("detail")
			.contains("rules[0]: invalid `failText`: unknown style `sparkle`")
	}));
	let stderr = stderr_text(&output);
	assert!(stderr.contains("doctor found blocking issues"));
}

#[test]
fn rules_json_reports_rule_inventory() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "install dependencies",
      "install": true
    },
    {
      "name": "checks",
      "parallel": [
        {
          "name": "lint",
          "changed": "packages/a/package-lock.json",
          "run": "cargo test -p lint"
        }
      ]
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["rules", "--json"]);

	assert!(output.status.success(), "rules --json should succeed");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse rules json");
	assert_eq!(value["status"], "ok");
	assert_eq!(value["code"], serde_json::Value::Null);
	assert_eq!(value["error"], serde_json::Value::Null);
	assert_eq!(value["rules"], 2);
	assert_eq!(value["parallelGroups"], 1);
	assert_eq!(value["summary"]["entries"], 2);
	assert_eq!(value["summary"]["rules"], 2);
	assert_eq!(value["summary"]["parallelGroups"], 1);
	assert_eq!(value["summary"]["selectors"], 3);
	assert_eq!(value["summary"]["commands"], 1);
	assert_eq!(value["summary"]["patterns"], 1);
	assert_eq!(
		value["selectors"],
		serde_json::json!(["checks", "install dependencies", "lint"])
	);
	assert_eq!(value["commands"], serde_json::json!(["cargo test -p lint"]));
	assert_eq!(value["patterns"], serde_json::json!(["packages/a/package-lock.json"]));
	let entries = value["entries"].as_array().expect("entries array");
	assert_eq!(entries.len(), 2);
	assert_eq!(entries[0]["type"], "rule");
	assert_eq!(entries[0]["name"], "install dependencies");
	assert_eq!(entries[0]["kind"], "install");
	assert_eq!(entries[0]["changed"], serde_json::json!([]));
	assert_eq!(entries[0]["exclude"], serde_json::json!([]));
	assert_eq!(entries[1]["type"], "group");
	assert_eq!(entries[1]["name"], "checks");
	assert_eq!(entries[1]["rules"][0]["name"], "lint");
	assert_eq!(
		entries[1]["rules"][0]["changed"],
		serde_json::json!(["packages/a/package-lock.json"])
	);
	assert_eq!(entries[1]["rules"][0]["exclude"], serde_json::json!([]));
}

#[test]
fn rules_json_filters_inventory_by_kind() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "install dependencies",
      "install": true
    },
    {
      "name": "checks",
      "parallel": [
        {
          "name": "lint",
          "changed": "packages/a/package-lock.json",
          "run": "cargo test -p lint"
        }
      ]
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["rules", "--kind", "install", "--json"]);

	assert!(output.status.success(), "rules --kind install --json should succeed");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse rules json");
	assert_eq!(value["status"], "ok");
	assert_eq!(value["error"], serde_json::Value::Null);
	assert_eq!(value["kind"], "install");
	assert_eq!(value["rules"], 1);
	assert_eq!(value["parallelGroups"], 0);
	assert_eq!(value["selectors"], serde_json::json!(["install dependencies"]));
	assert_eq!(value["commands"], serde_json::json!([]));
	assert_eq!(value["patterns"], serde_json::json!([]));
	let entries = value["entries"].as_array().expect("entries array");
	assert_eq!(entries.len(), 1);
	assert_eq!(entries[0]["name"], "install dependencies");
	assert_eq!(entries[0]["kind"], "install");
}

#[test]
fn rules_json_filters_inventory_by_rule_selector() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "install dependencies",
      "install": true
    },
    {
      "name": "checks",
      "parallel": [
        {
          "name": "lint",
          "changed": "packages/a/package-lock.json",
          "run": "cargo test -p lint"
        },
        {
          "name": "typecheck",
          "changed": "packages/b/package-lock.json",
          "run": "cargo test -p typecheck"
        }
      ]
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["rules", "--rule", "lint", "--json"]);

	assert!(output.status.success(), "rules --rule lint --json should succeed");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse rules json");
	assert_eq!(value["status"], "ok");
	assert_eq!(value["selectors"], serde_json::json!(["lint"]));
	assert_eq!(value["commands"], serde_json::json!(["cargo test -p lint"]));
	assert_eq!(value["patterns"], serde_json::json!(["packages/a/package-lock.json"]));
	assert_eq!(value["rules"], 1);
	assert_eq!(value["parallelGroups"], 1);
	let entries = value["entries"].as_array().expect("entries array");
	assert_eq!(entries.len(), 1);
	assert_eq!(entries[0]["type"], "group");
	assert_eq!(entries[0]["name"], "checks");
	let rules = entries[0]["rules"].as_array().expect("group rules");
	assert_eq!(rules.len(), 1);
	assert_eq!(rules[0]["name"], "lint");
}

#[test]
fn rules_json_rejects_unknown_rule_selector_as_json() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(
		repo_root,
		"typecheck",
		"packages/a/package-lock.json",
		"cargo test -p typecheck",
	);

	let output = run_pullhook(repo_root, &["rules", "--rule", "typcheck", "--json"]);

	assert!(!output.status.success(), "rules should fail for an unknown selector");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse selector error json");
	assert_eq!(value["status"], "error");
	let error = value["error"].as_str().expect("error message");
	assert!(error.contains("unknown rule selector(s): typcheck (did you mean `typecheck`?)"));
	assert_eq!(value["unknownSelectors"], serde_json::json!(["typcheck"]));
	assert_eq!(value["availableSelectors"], serde_json::json!(["typecheck"]));
	assert_eq!(
		value["suggestions"],
		serde_json::json!([
			{
				"selector": "typcheck",
				"suggestion": "typecheck"
			}
		])
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("unknown rule selector(s): typcheck (did you mean `typecheck`?)"));
}

#[test]
fn rules_names_only_prints_rule_selectors() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "checks",
      "parallel": [
        {
          "name": "lint",
          "changed": "packages/a/package-lock.json",
          "run": "cargo test -p lint"
        },
        {
          "name": "typecheck",
          "changed": "packages/a/package-lock.json",
          "run": "cargo test -p typecheck"
        }
      ]
    },
    {
      "name": "install dependencies",
      "install": true
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["rules", "--names-only"]);

	assert!(output.status.success(), "rules --names-only should succeed");
	let stdout = stdout_text(&output);
	assert_eq!(stdout, "checks\ninstall dependencies\nlint\ntypecheck\n");
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "rules --names-only should not write stderr");
}

#[test]
fn rules_names_only_filters_by_kind() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "install dependencies",
      "install": true
    },
    {
      "name": "checks",
      "parallel": [
        {
          "name": "lint",
          "changed": "packages/a/package-lock.json",
          "run": "cargo test -p lint"
        },
        {
          "name": "typecheck",
          "changed": "packages/a/package-lock.json",
          "run": "cargo test -p typecheck"
        }
      ]
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["rules", "--kind", "run", "--names-only"]);

	assert!(output.status.success(), "rules --kind run --names-only should succeed");
	let stdout = stdout_text(&output);
	assert_eq!(stdout, "lint\ntypecheck\n");
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"rules --kind run --names-only should not write stderr"
	);
}

#[test]
fn rules_names_only_respects_rule_selector_filter() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "checks",
      "parallel": [
        {
          "name": "lint",
          "changed": "packages/a/package-lock.json",
          "run": "cargo test -p lint"
        },
        {
          "name": "typecheck",
          "changed": "packages/a/package-lock.json",
          "run": "cargo test -p typecheck"
        }
      ]
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["rules", "--rule", "lint", "--names-only"]);

	assert!(output.status.success(), "rules --rule lint --names-only should succeed");
	let stdout = stdout_text(&output);
	assert_eq!(stdout, "lint\n");
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"rules --rule lint --names-only should not write stderr"
	);
}

#[test]
fn rules_commands_only_prints_configured_run_commands() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "install dependencies",
      "install": true
    },
    {
      "name": "format",
      "changed": "packages/a/package-lock.json",
      "run": "cargo fmt --all"
    },
    {
      "name": "checks",
      "parallel": [
        {
          "name": "lint",
          "changed": "packages/a/package-lock.json",
          "run": "cargo clippy --all-targets"
        },
        {
          "name": "test",
          "changed": "packages/a/package-lock.json",
          "run": "cargo nextest run"
        }
      ]
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["rules", "--commands-only"]);

	assert!(output.status.success(), "rules --commands-only should succeed");
	let stdout = stdout_text(&output);
	assert_eq!(
		stdout,
		"cargo fmt --all\ncargo clippy --all-targets\ncargo nextest run\n"
	);
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"rules --commands-only should not write stderr"
	);
}

#[test]
fn rules_commands_only_respects_kind_filter() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "install dependencies",
      "install": true
    },
    {
      "name": "format",
      "changed": "packages/a/package-lock.json",
      "run": "cargo fmt --all"
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["rules", "--kind", "install", "--commands-only"]);

	assert!(
		output.status.success(),
		"rules --kind install --commands-only should succeed"
	);
	let stdout = stdout_text(&output);
	assert!(
		stdout.trim().is_empty(),
		"install rules without configured run commands should print no commands"
	);
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"rules --kind install --commands-only should not write stderr"
	);
}

#[test]
fn rules_commands_only_respects_rule_selector_filter() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "format",
      "changed": "packages/a/package-lock.json",
      "run": "cargo fmt --all"
    },
    {
      "name": "checks",
      "parallel": [
        {
          "name": "lint",
          "changed": "packages/a/package-lock.json",
          "run": "cargo clippy --all-targets"
        },
        {
          "name": "test",
          "changed": "packages/a/package-lock.json",
          "run": "cargo nextest run"
        }
      ]
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["rules", "--rule", "lint", "--commands-only"]);

	assert!(
		output.status.success(),
		"rules --rule lint --commands-only should succeed"
	);
	let stdout = stdout_text(&output);
	assert_eq!(stdout, "cargo clippy --all-targets\n");
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"rules --rule lint --commands-only should not write stderr"
	);
}

#[test]
fn rules_patterns_only_prints_configured_changed_patterns() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "install dependencies",
      "install": true
    },
    {
      "name": "format",
      "changed": "packages/a/package-lock.json",
      "run": "cargo fmt --all"
    },
    {
      "name": "checks",
      "parallel": [
        {
          "name": "lint",
          "changed": "packages/b/package-lock.json",
          "run": "cargo clippy --all-targets"
        },
        {
          "name": "test",
          "changed": ["src/**/*.rs", "tests/**/*.rs"],
          "run": "cargo nextest run"
        }
      ]
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["rules", "--patterns-only"]);

	assert!(output.status.success(), "rules --patterns-only should succeed");
	let stdout = stdout_text(&output);
	assert_eq!(
		stdout,
		"packages/a/package-lock.json\npackages/b/package-lock.json\nsrc/**/*.rs\ntests/**/*.rs\n"
	);
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"rules --patterns-only should not write stderr"
	);
}

#[test]
fn rules_patterns_only_respects_kind_filter() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "install dependencies",
      "install": true
    },
    {
      "name": "format",
      "changed": "packages/a/package-lock.json",
      "run": "cargo fmt --all"
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["rules", "--kind", "install", "--patterns-only"]);

	assert!(
		output.status.success(),
		"rules --kind install --patterns-only should succeed"
	);
	let stdout = stdout_text(&output);
	assert!(
		stdout.trim().is_empty(),
		"install rules without configured changed patterns should print no patterns"
	);
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"rules --kind install --patterns-only should not write stderr"
	);
}

#[test]
fn rules_patterns_only_respects_rule_selector_filter() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "format",
      "changed": "packages/a/package-lock.json",
      "run": "cargo fmt --all"
    },
    {
      "name": "checks",
      "parallel": [
        {
          "name": "lint",
          "changed": "packages/b/package-lock.json",
          "run": "cargo clippy --all-targets"
        },
        {
          "name": "test",
          "changed": ["src/**/*.rs", "tests/**/*.rs"],
          "run": "cargo nextest run"
        }
      ]
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["rules", "--rule", "test", "--patterns-only"]);

	assert!(
		output.status.success(),
		"rules --rule test --patterns-only should succeed"
	);
	let stdout = stdout_text(&output);
	assert_eq!(stdout, "src/**/*.rs\ntests/**/*.rs\n");
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"rules --rule test --patterns-only should not write stderr"
	);
}

#[test]
fn rules_text_lists_group_members() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "checks",
      "parallel": [
        {
          "name": "lint",
          "changed": "packages/a/package-lock.json",
          "run": "cargo test -p lint"
        },
        {
          "name": "typecheck",
          "changed": "packages/a/package-lock.json",
          "run": "cargo test -p typecheck"
        }
      ]
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["rules", "--render", "never"]);

	assert!(output.status.success(), "rules should succeed");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("config:"));
	assert!(stdout.contains("Rules"));
	assert!(stdout.contains("[group] checks"));
	assert!(stdout.contains("- lint"));
	assert!(stdout.contains("- typecheck"));
	assert!(stdout.contains("command: cargo test -p lint"));
}

#[test]
fn rules_text_filters_groups_by_kind() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "checks",
      "parallel": [
        {
          "name": "lint",
          "changed": "packages/a/package-lock.json",
          "run": "cargo test -p lint"
        }
      ]
    },
    {
      "name": "install dependencies",
      "install": true
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["rules", "--kind", "group", "--render", "never"]);

	assert!(output.status.success(), "rules --kind group should succeed");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("rules: 0 | parallel groups: 1"));
	assert!(stdout.contains("[group] checks"));
	assert!(!stdout.contains("[rule] install dependencies"));
	assert!(!stdout.contains("- lint"));
}

#[test]
fn validate_accepts_relative_explicit_config_path_from_subdirectory() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("config/pullhook.custom.json"),
		r#"{
  "rules": [
    {
      "name": "rebuild package a",
      "changed": "packages/a/package-lock.json",
      "run": "cargo test -p package-a"
    }
  ]
}
"#,
	);

	let output = run_pullhook(
		&repo_root.join("packages/a"),
		&["validate", "--config", "../../config/pullhook.custom.json"],
	);

	assert!(output.status.success(), "validate should accept explicit config path");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("config valid"));
	assert!(stdout.contains("entries: 1 | rules: 1 | parallel groups: 0"));
}

#[test]
fn run_dry_run_uses_explicit_config_path_when_default_config_is_missing() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("config/pullhook.custom.json"),
		r#"{
  "rules": [
    {
      "name": "rebuild package a",
      "changed": "packages/a/package-lock.json",
      "run": "cargo test -p package-a"
    }
  ]
}
"#,
	);

	let output = run_pullhook(
		repo_root,
		&["run", "--config", "config/pullhook.custom.json", "--dry-run", "--json"],
	);

	assert!(output.status.success(), "run should use explicit config path");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse run json");
	assert_eq!(value["plannedCommands"], 1);
	assert_eq!(value["entries"][0]["name"], "rebuild package a");
	assert_eq!(value["entries"][0]["status"], "match");
}

#[test]
fn explain_json_reports_matches_and_skips() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "rebuild package a",
      "changed": "packages/a/package-lock.json",
      "run": "cargo test -p package-a"
    },
    {
      "name": "skip markdown",
      "changed": "**/*.md",
      "run": "cargo test"
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["explain", "--all-matches", "--json"]);

	assert!(output.status.success(), "explain --json should succeed");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse explain json");
	assert_eq!(value["status"], "ok");
	assert_eq!(value["code"], serde_json::Value::Null);
	assert_eq!(value["error"], serde_json::Value::Null);
	assert_eq!(value["baseMissing"], false);
	assert_eq!(value["onFailure"], "stop");
	assert_eq!(value["summary"]["changedFilesSource"], "git");
	assert_eq!(value["summary"]["baseMissing"], false);
	assert_eq!(value["summary"]["changedFiles"], 2);
	assert_eq!(value["summary"]["matchedFiles"], 1);
	assert_eq!(value["summary"]["matchedRules"], 1);
	assert_eq!(value["summary"]["plannedCommands"], 1);
	assert_eq!(
		value["matchedFiles"],
		serde_json::json!(["packages/a/package-lock.json"])
	);
	let entries = value["entries"].as_array().expect("entries array");
	assert_eq!(entries.len(), 2);
	assert_eq!(entries[0]["name"], "rebuild package a");
	assert_eq!(entries[0]["status"], "match");
	assert_eq!(entries[0]["command"], "cargo test -p package-a");
	assert_eq!(entries[1]["name"], "skip markdown");
	assert_eq!(entries[1]["status"], "skip");
	assert_eq!(entries[1]["skipReason"], "no matching changed files");
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "explain --json should not write stderr");
}

#[test]
fn explain_summary_only_reports_compact_plan_counts() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "rebuild package a",
      "changed": "packages/a/package-lock.json",
      "run": "cargo test -p package-a"
    },
    {
      "name": "skip markdown",
      "changed": "**/*.md",
      "run": "cargo test"
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["explain", "--summary-only"]);

	assert!(output.status.success(), "explain --summary-only should succeed");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("changedFilesSource: git"));
	assert!(stdout.contains("baseMissing: false"));
	assert!(stdout.contains("changedFiles: 2"));
	assert!(stdout.contains("matchedFiles: 1"));
	assert!(stdout.contains("plannedCommands: 1"));
	assert!(
		!stdout.contains("[match]"),
		"summary-only output should not include rule blocks"
	);
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"explain --summary-only should not write stderr"
	);
}

#[test]
fn explain_commands_only_reports_planned_commands() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "rebuild package a",
      "changed": "packages/a/package-lock.json",
      "run": "cargo test -p package-a"
    },
    {
      "name": "skip markdown",
      "changed": "**/*.md",
      "run": "cargo test"
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["explain", "--commands-only"]);

	assert!(output.status.success(), "explain --commands-only should succeed");
	let stdout = stdout_text(&output);
	assert_eq!(stdout.trim(), "cargo test -p package-a");
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"explain --commands-only should not write stderr"
	);
}

#[test]
fn explain_changed_files_only_reports_resolved_changed_files() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(
		repo_root,
		"package a",
		"packages/a/package-lock.json",
		"cargo test -p package-a",
	);

	let output = run_pullhook(repo_root, &["explain", "--changed-files-only"]);

	assert!(output.status.success(), "explain --changed-files-only should succeed");
	let stdout = stdout_text(&output);
	assert_eq!(stdout, "packages/a/package-lock.json\npackages/b/package-lock.json\n");
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"explain --changed-files-only should not write stderr"
	);
}

#[test]
fn explain_matched_files_only_reports_unique_matched_files() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "package a",
      "changed": "packages/a/package-lock.json",
      "run": "cargo test -p package-a"
    },
    {
      "name": "all packages",
      "changed": "packages/*/package-lock.json",
      "run": "cargo test --workspace"
    },
    {
      "name": "skip markdown",
      "changed": "**/*.md",
      "run": "cargo test"
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["explain", "--matched-files-only"]);

	assert!(output.status.success(), "explain --matched-files-only should succeed");
	let stdout = stdout_text(&output);
	assert_eq!(stdout, "packages/a/package-lock.json\npackages/b/package-lock.json\n");
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"explain --matched-files-only should not write stderr"
	);
}

#[test]
fn explain_matched_rules_only_reports_matching_rule_names() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "package a",
      "changed": "packages/a/package-lock.json",
      "run": "cargo test -p package-a"
    },
    {
      "name": "all packages",
      "changed": "packages/*/package-lock.json",
      "run": "cargo test --workspace"
    },
    {
      "name": "skip markdown",
      "changed": "**/*.md",
      "run": "cargo test"
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["explain", "--matched-rules-only"]);

	assert!(output.status.success(), "explain --matched-rules-only should succeed");
	let stdout = stdout_text(&output);
	assert_eq!(stdout, "package a\nall packages\n");
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"explain --matched-rules-only should not write stderr"
	);
}

#[test]
fn explain_require_match_fails_after_printing_empty_summary() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "skip markdown", "**/*.md", "cargo test");

	let output = run_pullhook(repo_root, &["explain", "--summary-only", "--require-match"]);

	assert!(
		!output.status.success(),
		"explain --require-match should fail when no rules match"
	);
	let stdout = stdout_text(&output);
	assert!(stdout.contains("changedFiles: 2"));
	assert!(stdout.contains("matchedFiles: 0"));
	assert!(stdout.contains("plannedCommands: 0"));
	let stderr = stderr_text(&output);
	assert!(stderr.contains("no config rules matched changed files"));
}

#[test]
fn explain_require_match_fails_after_printing_empty_json_plan() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "skip markdown", "**/*.md", "cargo test");

	let output = run_pullhook(repo_root, &["explain", "--json", "--require-match"]);

	assert!(
		!output.status.success(),
		"explain --json --require-match should fail when no rules match"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse explain json");
	assert_eq!(value["status"], "error");
	assert_eq!(value["code"], "no_rules_matched");
	assert_eq!(value["error"], "no config rules matched changed files");
	assert_eq!(value["matchedFiles"], serde_json::json!([]));
	let stderr = stderr_text(&output);
	assert!(stderr.contains("no config rules matched changed files"));
}

#[test]
fn explain_require_match_succeeds_when_a_rule_matches() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(
		repo_root,
		"package a",
		"packages/a/package-lock.json",
		"cargo test -p package-a",
	);

	let output = run_pullhook(repo_root, &["explain", "--matched-rules-only", "--require-match"]);

	assert!(
		output.status.success(),
		"explain --require-match should succeed when a rule matches"
	);
	let stdout = stdout_text(&output);
	assert_eq!(stdout, "package a\n");
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"successful explain --require-match should not write stderr"
	);
}

#[test]
fn explain_json_filters_to_requested_rule() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "rebuild package a",
      "changed": "packages/a/package-lock.json",
      "run": "cargo test -p package-a"
    },
    {
      "name": "skip markdown",
      "changed": "**/*.md",
      "run": "cargo test"
    }
  ]
}
"#,
	);

	let output = run_pullhook(
		repo_root,
		&["explain", "--all-matches", "--rule", "skip markdown", "--json"],
	);

	assert!(output.status.success(), "explain with --rule should succeed");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse explain json");
	let entries = value["entries"].as_array().expect("entries array");
	assert_eq!(entries.len(), 1);
	assert_eq!(entries[0]["name"], "skip markdown");
	assert_eq!(entries[0]["status"], "skip");
	assert_eq!(entries[0]["skipReason"], "no matching changed files");
}

#[test]
fn explain_json_uses_explicit_changed_files() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "docs check", "docs/*.md", "cargo test -p docs");

	let output = run_pullhook(repo_root, &["explain", "--changed-file", "docs/guide.md", "--json"]);

	assert!(output.status.success(), "explain with --changed-file should succeed");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse explain json");
	assert_eq!(value["changedFilesSource"], "explicit");
	assert_eq!(value["changedFiles"], serde_json::json!(["docs/guide.md"]));
	assert_eq!(value["matchedFiles"], serde_json::json!(["docs/guide.md"]));
	assert_eq!(value["entries"][0]["status"], "match");
	assert_eq!(value["entries"][0]["command"], "cargo test -p docs");
}

#[test]
fn explain_json_reads_changed_files_from_stdin() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "docs check", "docs/*.md", "cargo test -p docs");

	let output = run_pullhook_with_stdin(
		repo_root,
		&["explain", "--changed-files-stdin", "--json"],
		"docs/guide.md\n\n",
	);

	assert!(
		output.status.success(),
		"explain with --changed-files-stdin should succeed"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse explain json");
	assert_eq!(value["changedFiles"], serde_json::json!(["docs/guide.md"]));
	assert_eq!(value["matchedFiles"], serde_json::json!(["docs/guide.md"]));
	assert_eq!(value["entries"][0]["status"], "match");
}

#[test]
fn explain_json_reads_changed_files_from_file() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "docs check", "docs/*.md", "cargo test -p docs");
	write_file(repo_root, Path::new(".pullhook-changed"), "docs/guide.md\n\n");

	let output = run_pullhook(
		repo_root,
		&["explain", "--changed-files-file", ".pullhook-changed", "--json"],
	);

	assert!(
		output.status.success(),
		"explain with --changed-files-file should succeed"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse explain json");
	assert_eq!(value["changedFiles"], serde_json::json!(["docs/guide.md"]));
	assert_eq!(value["matchedFiles"], serde_json::json!(["docs/guide.md"]));
	assert_eq!(value["entries"][0]["status"], "match");
}

#[test]
fn explain_json_reads_changed_files_file_dash_from_stdin() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "docs check", "docs/*.md", "cargo test -p docs");

	let output = run_pullhook_with_stdin(
		repo_root,
		&["explain", "--changed-files-file", "-", "--json"],
		"docs/guide.md\n\n",
	);

	assert!(
		output.status.success(),
		"explain with --changed-files-file - should read stdin"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse explain json");
	assert_eq!(value["changedFiles"], serde_json::json!(["docs/guide.md"]));
	assert_eq!(value["matchedFiles"], serde_json::json!(["docs/guide.md"]));
	assert_eq!(value["entries"][0]["status"], "match");
}

#[test]
fn explain_json_reports_missing_changed_files_file_as_json() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "docs check", "docs/*.md", "cargo test -p docs");

	let output = run_pullhook(
		repo_root,
		&["explain", "--changed-files-file", ".pullhook-missing", "--json"],
	);

	assert!(
		!output.status.success(),
		"explain should fail when changed files file is missing"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse changed-files error json");
	assert_eq!(value["status"], "error");
	let error = value["error"].as_str().expect("error message");
	assert!(error.contains("failed to read changed files from `.pullhook-missing`"));
	assert_eq!(value["changedFilesFile"], ".pullhook-missing");
	let details = value["details"].as_array().expect("details array");
	assert!(details.iter().any(|detail| {
		detail
			.as_str()
			.expect("detail")
			.contains("check that `.pullhook-missing` exists and is readable")
	}));
	assert!(
		details
			.iter()
			.any(|detail| { detail.as_str().expect("detail").contains("--changed-files-file -") })
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("failed to read changed files from `.pullhook-missing`"));
}

#[test]
fn run_dry_run_json_reports_planned_commands() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "rebuild package a",
      "changed": "packages/a/package-lock.json",
      "run": "cargo test -p package-a"
    },
    {
      "name": "skip markdown",
      "changed": "**/*.md",
      "run": "cargo test"
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["run", "--dry-run", "--json"]);

	assert!(output.status.success(), "run --dry-run --json should succeed");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse run json");
	assert_eq!(value["status"], "ok");
	assert_eq!(value["code"], serde_json::Value::Null);
	assert_eq!(value["error"], serde_json::Value::Null);
	assert_eq!(value["mode"], "dry-run");
	assert_eq!(value["changedFilesSource"], "git");
	assert_eq!(value["plannedCommands"], 1);
	assert_eq!(value["summary"]["changedFilesSource"], "git");
	assert_eq!(value["summary"]["baseMissing"], false);
	assert_eq!(value["summary"]["changedFiles"], 2);
	assert_eq!(value["summary"]["matchedFiles"], 1);
	assert_eq!(value["summary"]["matchedRules"], 1);
	assert_eq!(value["summary"]["plannedCommands"], 1);
	assert_eq!(
		value["matchedFiles"],
		serde_json::json!(["packages/a/package-lock.json"])
	);
	let entries = value["entries"].as_array().expect("entries array");
	assert_eq!(entries[0]["status"], "match");
	assert_eq!(entries[1]["status"], "skip");
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "run --dry-run --json should not write stderr");
}

#[test]
fn run_commands_only_reports_planned_commands_without_execution() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "write marker",
      "changed": "packages/a/package-lock.json",
      "run": "touch marker.txt"
    },
    {
      "name": "skip markdown",
      "changed": "**/*.md",
      "run": "touch skipped.txt"
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["run", "--commands-only"]);

	assert!(output.status.success(), "run --commands-only should succeed");
	let stdout = stdout_text(&output);
	assert_eq!(stdout.trim(), "touch marker.txt");
	assert!(
		!repo_root.join("marker.txt").exists(),
		"commands-only should not execute planned commands"
	);
	assert!(
		!repo_root.join("skipped.txt").exists(),
		"commands-only should not execute skipped commands"
	);
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "run --commands-only should not write stderr");
}

#[test]
fn run_changed_files_only_reports_resolved_changed_files_without_execution() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(
		repo_root,
		"write marker",
		"packages/a/package-lock.json",
		"touch marker.txt",
	);

	let output = run_pullhook(repo_root, &["run", "--changed-files-only"]);

	assert!(output.status.success(), "run --changed-files-only should succeed");
	let stdout = stdout_text(&output);
	assert_eq!(stdout, "packages/a/package-lock.json\npackages/b/package-lock.json\n");
	assert!(
		!repo_root.join("marker.txt").exists(),
		"changed-files-only should not execute planned commands"
	);
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"run --changed-files-only should not write stderr"
	);
}

#[test]
fn run_matched_files_only_reports_unique_matched_files_without_execution() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "write package a",
      "changed": "packages/a/package-lock.json",
      "run": "touch marker.txt"
    },
    {
      "name": "write all packages",
      "changed": "packages/*/package-lock.json",
      "run": "touch other-marker.txt"
    },
    {
      "name": "skip markdown",
      "changed": "**/*.md",
      "run": "touch skipped.txt"
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["run", "--matched-files-only"]);

	assert!(output.status.success(), "run --matched-files-only should succeed");
	let stdout = stdout_text(&output);
	assert_eq!(stdout, "packages/a/package-lock.json\npackages/b/package-lock.json\n");
	assert!(
		!repo_root.join("marker.txt").exists(),
		"matched-files-only should not execute planned commands"
	);
	assert!(
		!repo_root.join("other-marker.txt").exists(),
		"matched-files-only should not execute planned commands"
	);
	assert!(
		!repo_root.join("skipped.txt").exists(),
		"matched-files-only should not execute skipped commands"
	);
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"run --matched-files-only should not write stderr"
	);
}

#[test]
fn run_matched_rules_only_reports_matching_rule_names_without_execution() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "write package a",
      "changed": "packages/a/package-lock.json",
      "run": "touch marker.txt"
    },
    {
      "name": "write all packages",
      "changed": "packages/*/package-lock.json",
      "run": "touch other-marker.txt"
    },
    {
      "name": "skip markdown",
      "changed": "**/*.md",
      "run": "touch skipped.txt"
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["run", "--matched-rules-only"]);

	assert!(output.status.success(), "run --matched-rules-only should succeed");
	let stdout = stdout_text(&output);
	assert_eq!(stdout, "write package a\nwrite all packages\n");
	assert!(
		!repo_root.join("marker.txt").exists(),
		"matched-rules-only should not execute planned commands"
	);
	assert!(
		!repo_root.join("other-marker.txt").exists(),
		"matched-rules-only should not execute planned commands"
	);
	assert!(
		!repo_root.join("skipped.txt").exists(),
		"matched-rules-only should not execute skipped commands"
	);
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"run --matched-rules-only should not write stderr"
	);
}

#[test]
fn run_require_match_fails_after_printing_empty_json_plan() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "skip markdown", "**/*.md", "touch skipped.txt");

	let output = run_pullhook(repo_root, &["run", "--dry-run", "--json", "--require-match"]);

	assert!(
		!output.status.success(),
		"run --require-match should fail when no rules match"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse run json");
	assert_eq!(value["status"], "error");
	assert_eq!(value["code"], "no_rules_matched");
	assert_eq!(value["error"], "no config rules matched changed files");
	assert_eq!(value["plannedCommands"], 0);
	assert_eq!(value["matchedFiles"], serde_json::json!([]));
	assert!(
		!repo_root.join("skipped.txt").exists(),
		"require-match dry-run should not execute skipped commands"
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("no config rules matched changed files"));
}

#[test]
fn run_require_match_succeeds_when_a_rule_matches_without_execution() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(
		repo_root,
		"write marker",
		"packages/a/package-lock.json",
		"touch marker.txt",
	);

	let output = run_pullhook(repo_root, &["run", "--matched-rules-only", "--require-match"]);

	assert!(
		output.status.success(),
		"run --require-match should succeed when a rule matches"
	);
	let stdout = stdout_text(&output);
	assert_eq!(stdout, "write marker\n");
	assert!(
		!repo_root.join("marker.txt").exists(),
		"matched-rules-only should not execute planned commands"
	);
	let stderr = stderr_text(&output);
	assert!(
		stderr.trim().is_empty(),
		"successful run --require-match should not write stderr"
	);
}

#[test]
fn run_summary_only_reports_plan_counts_without_execution() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "write marker",
      "changed": "packages/a/package-lock.json",
      "run": "touch marker.txt"
    },
    {
      "name": "skip markdown",
      "changed": "**/*.md",
      "run": "touch skipped.txt"
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["run", "--summary-only"]);

	assert!(output.status.success(), "run --summary-only should succeed");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("changedFilesSource: git"));
	assert!(stdout.contains("baseMissing: false"));
	assert!(stdout.contains("changedFiles: 2"));
	assert!(stdout.contains("matchedFiles: 1"));
	assert!(stdout.contains("plannedCommands: 1"));
	assert!(
		!repo_root.join("marker.txt").exists(),
		"summary-only should not execute planned commands"
	);
	assert!(
		!repo_root.join("skipped.txt").exists(),
		"summary-only should not execute skipped commands"
	);
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "run --summary-only should not write stderr");
}

#[test]
fn run_dry_run_json_uses_explicit_changed_files() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "docs check", "docs/*.md", "cargo test -p docs");

	let output = run_pullhook(
		repo_root,
		&["run", "--dry-run", "--changed-file", "docs/guide.md", "--json"],
	);

	assert!(
		output.status.success(),
		"run --dry-run with --changed-file should succeed"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse run json");
	assert_eq!(value["mode"], "dry-run");
	assert_eq!(value["plannedCommands"], 1);
	assert_eq!(value["changedFiles"], serde_json::json!(["docs/guide.md"]));
	assert_eq!(value["entries"][0]["status"], "match");
}

#[test]
fn run_dry_run_json_reads_changed_files_from_stdin() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "docs check",
      "changed": "docs/*.md",
      "run": "cargo test -p docs"
    },
    {
      "name": "config check",
      "changed": "config/*.json",
      "run": "cargo test -p config"
    }
  ]
}
"#,
	);

	let output = run_pullhook_with_stdin(
		repo_root,
		&[
			"run",
			"--dry-run",
			"--changed-file",
			"config/app.json",
			"--changed-files-stdin",
			"--json",
		],
		"docs/guide.md\n",
	);

	assert!(
		output.status.success(),
		"run --dry-run with stdin changed files should succeed"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse run json");
	assert_eq!(value["plannedCommands"], 2);
	assert_eq!(
		value["changedFiles"],
		serde_json::json!(["config/app.json", "docs/guide.md"])
	);
	assert_eq!(value["entries"][0]["status"], "match");
	assert_eq!(value["entries"][1]["status"], "match");
}

#[test]
fn run_dry_run_json_reads_changed_files_from_file() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "docs check", "docs/*.md", "cargo test -p docs");
	write_file(repo_root, Path::new(".pullhook-changed"), "docs/guide.md\n\n");

	let output = run_pullhook(
		repo_root,
		&[
			"run",
			"--dry-run",
			"--changed-files-file",
			".pullhook-changed",
			"--json",
		],
	);

	assert!(
		output.status.success(),
		"run --dry-run with --changed-files-file should succeed"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse run json");
	assert_eq!(value["plannedCommands"], 1);
	assert_eq!(value["changedFiles"], serde_json::json!(["docs/guide.md"]));
	assert_eq!(value["entries"][0]["status"], "match");
}

#[test]
fn run_dry_run_json_reads_changed_files_file_dash_from_stdin() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "docs check", "docs/*.md", "cargo test -p docs");

	let output = run_pullhook_with_stdin(
		repo_root,
		&["run", "--dry-run", "--changed-files-file", "-", "--json"],
		"docs/guide.md\n\n",
	);

	assert!(
		output.status.success(),
		"run --dry-run with --changed-files-file - should read stdin"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse run json");
	assert_eq!(value["plannedCommands"], 1);
	assert_eq!(value["changedFiles"], serde_json::json!(["docs/guide.md"]));
	assert_eq!(value["entries"][0]["status"], "match");
}

#[test]
fn run_json_reports_missing_changed_files_file_as_json() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "docs check", "docs/*.md", "cargo test -p docs");

	let output = run_pullhook(
		repo_root,
		&[
			"run",
			"--dry-run",
			"--changed-files-file",
			".pullhook-missing",
			"--json",
		],
	);

	assert!(
		!output.status.success(),
		"run should fail when changed files file is missing"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse changed-files error json");
	assert_eq!(value["status"], "error");
	let error = value["error"].as_str().expect("error message");
	assert!(error.contains("failed to read changed files from `.pullhook-missing`"));
	assert_eq!(value["changedFilesFile"], ".pullhook-missing");
	let details = value["details"].as_array().expect("details array");
	assert!(details.iter().any(|detail| {
		detail
			.as_str()
			.expect("detail")
			.contains("check that `.pullhook-missing` exists and is readable")
	}));
	assert!(
		details
			.iter()
			.any(|detail| { detail.as_str().expect("detail").contains("--changed-files-file -") })
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("failed to read changed files from `.pullhook-missing`"));
}

#[test]
fn run_dry_run_json_dedupes_explicit_changed_files() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "docs check", "docs/*.md", "cargo test -p docs");
	write_file(repo_root, Path::new(".pullhook-changed"), "docs/guide.md\n");

	let output = run_pullhook_with_stdin(
		repo_root,
		&[
			"run",
			"--dry-run",
			"--changed-file",
			"docs/guide.md",
			"--changed-files-file",
			".pullhook-changed",
			"--changed-files-stdin",
			"--json",
		],
		"docs/guide.md\n",
	);

	assert!(
		output.status.success(),
		"run --dry-run should dedupe repeated explicit changed files"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse run json");
	assert_eq!(value["changedFiles"], serde_json::json!(["docs/guide.md"]));
	assert_eq!(value["matchedFiles"], serde_json::json!(["docs/guide.md"]));
	assert_eq!(value["entries"][0]["matches"], serde_json::json!(["docs/guide.md"]));
}

#[test]
fn run_rejects_changed_file_with_base() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "docs check", "docs/*.md", "cargo test -p docs");

	let output = run_pullhook(
		repo_root,
		&[
			"run",
			"--dry-run",
			"--changed-file",
			"docs/guide.md",
			"--base",
			"HEAD~1",
		],
	);

	assert!(!output.status.success(), "run should reject --changed-file with --base");
	let stderr = stderr_text(&output);
	assert!(stderr.contains("cannot be used with"));
	assert!(stderr.contains("--changed-file <path>"));
	assert!(stderr.contains("--base <rev>"));
}

#[test]
fn run_rejects_changed_files_stdin_with_base() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "docs check", "docs/*.md", "cargo test -p docs");

	let output = run_pullhook_with_stdin(
		repo_root,
		&["run", "--dry-run", "--changed-files-stdin", "--base", "HEAD~1"],
		"docs/guide.md\n",
	);

	assert!(
		!output.status.success(),
		"run should reject --changed-files-stdin with --base"
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("cannot be used with"));
	assert!(stderr.contains("--changed-files-stdin"));
	assert!(stderr.contains("--base <rev>"));
}

#[test]
fn run_rejects_changed_files_file_with_base() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "docs check", "docs/*.md", "cargo test -p docs");
	write_file(repo_root, Path::new(".pullhook-changed"), "docs/guide.md\n");

	let output = run_pullhook(
		repo_root,
		&[
			"run",
			"--dry-run",
			"--changed-files-file",
			".pullhook-changed",
			"--base",
			"HEAD~1",
		],
	);

	assert!(
		!output.status.success(),
		"run should reject --changed-files-file with --base"
	);
	let stderr = stderr_text(&output);
	assert!(stderr.contains("cannot be used with"));
	assert!(stderr.contains("--changed-files-file <path>"));
	assert!(stderr.contains("--base <rev>"));
}

#[test]
fn run_dry_run_json_filters_parallel_group_rules() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "checks",
      "parallel": [
        {
          "name": "lint",
          "changed": "packages/a/package-lock.json",
          "run": "cargo test -p lint"
        },
        {
          "name": "typecheck",
          "changed": "packages/a/package-lock.json",
          "run": "cargo test -p typecheck"
        }
      ]
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["run", "--dry-run", "--rule", "typecheck", "--json"]);

	assert!(output.status.success(), "run --dry-run with --rule should succeed");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse run json");
	assert_eq!(value["plannedCommands"], 1);
	let entries = value["entries"].as_array().expect("entries array");
	assert_eq!(entries.len(), 1);
	assert_eq!(entries[0]["type"], "group");
	let rules = entries[0]["rules"].as_array().expect("group rules");
	assert_eq!(rules.len(), 1);
	assert_eq!(rules[0]["name"], "typecheck");
	assert_eq!(rules[0]["status"], "match");
}

#[test]
fn run_json_reports_execution_results() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(
		repo_root,
		"write marker",
		"packages/a/package-lock.json",
		"sh -c 'echo ok-stdout; echo ok-stderr >&2'",
	);

	let output = run_pullhook(repo_root, &["run", "--json"]);

	assert!(output.status.success(), "run --json should succeed");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse run json");
	assert_eq!(value["status"], "ok");
	assert_eq!(value["code"], serde_json::Value::Null);
	assert_eq!(value["error"], serde_json::Value::Null);
	assert_eq!(value["mode"], "run");
	assert_eq!(value["summary"]["changedFilesSource"], "git");
	assert_eq!(value["summary"]["baseMissing"], false);
	assert_eq!(value["summary"]["changedFiles"], 2);
	assert_eq!(value["summary"]["matchedFiles"], 1);
	assert_eq!(value["summary"]["matchedRules"], 1);
	assert_eq!(value["summary"]["plannedCommands"], 1);
	assert_eq!(value["summary"]["taskDirs"], 1);
	assert_eq!(value["summary"]["passed"], 1);
	assert_eq!(value["summary"]["failed"], 0);
	let executions = value["executions"].as_array().expect("executions array");
	assert_eq!(executions.len(), 1);
	assert_eq!(executions[0]["name"], "write marker");
	assert_eq!(executions[0]["state"], "success");
	assert_eq!(executions[0]["outputs"][0]["stdout"], "ok-stdout\n");
	assert_eq!(executions[0]["outputs"][0]["stderr"], "ok-stderr\n");
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "run --json should not write stderr");
}

#[test]
fn run_json_reports_failure_results() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "onFailure": "continue",
  "rules": [
    {
      "name": "fail first",
      "changed": "packages/a/package-lock.json",
      "run": "sh -c 'echo nope >&2; exit 7'",
      "failText": "{red.bold {rule} failed}"
    },
    {
      "name": "run second",
      "changed": "packages/b/package-lock.json",
      "run": "sh -c 'echo later'"
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["run", "--json"]);

	assert!(!output.status.success(), "run --json should fail when a rule fails");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse failed run json");
	assert_eq!(value["status"], "error");
	assert_eq!(value["code"], "config_rule_failed");
	assert_eq!(value["error"], "1 config rule(s) failed");
	assert_eq!(value["summary"]["changedFiles"], 2);
	assert_eq!(value["summary"]["matchedFiles"], 2);
	assert_eq!(value["summary"]["matchedRules"], 2);
	assert_eq!(value["summary"]["plannedCommands"], 2);
	assert_eq!(value["summary"]["taskDirs"], 2);
	assert_eq!(value["summary"]["passed"], 1);
	assert_eq!(value["summary"]["failed"], 1);
	let executions = value["executions"].as_array().expect("executions array");
	assert_eq!(executions.len(), 2);
	assert_eq!(executions[0]["state"], "failed");
	assert_eq!(executions[0]["failText"], "fail first failed");
	assert_eq!(executions[1]["state"], "success");
	let stderr = stderr_text(&output);
	assert!(stderr.contains("1 config rule(s) failed"));
}

#[test]
fn run_quiet_suppresses_successful_text_output() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "write marker", "packages/a/package-lock.json", "true");

	let output = run_pullhook(repo_root, &["run", "--quiet"]);

	assert!(output.status.success(), "quiet run should succeed");
	let stdout = stdout_text(&output);
	let stderr = stderr_text(&output);
	assert!(stdout.trim().is_empty(), "quiet successful run should not write stdout");
	assert!(stderr.trim().is_empty(), "quiet successful run should not write stderr");
}

#[test]
fn run_quiet_reports_failures() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "fail marker",
      "changed": "packages/a/package-lock.json",
      "run": "false",
      "failText": "{rule} failed"
    }
  ]
}
"#,
	);

	let output = run_pullhook_with_env(repo_root, &["run", "--quiet"], &[("PULLHOOK_RENDER_MODE", "never")]);

	assert!(!output.status.success(), "quiet run should still fail");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("Tasks"));
	assert!(stdout.contains("[error] failed"));
	assert!(stdout.contains("Summary"));
	assert!(stdout.contains("failed: 1"));
	let stderr = stderr_text(&output);
	assert!(stderr.contains("fail marker failed"));
	assert!(stderr.contains("1 config rule(s) failed"));
}

#[test]
fn run_rejects_unknown_rule_selector() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "write marker", "packages/a/package-lock.json", "true");

	let output = run_pullhook(repo_root, &["run", "--dry-run", "--rule", "missing rule", "--json"]);

	assert!(!output.status.success(), "run should fail for an unknown rule selector");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse selector error json");
	assert_eq!(value["status"], "error");
	let error = value["error"].as_str().expect("error message");
	assert!(error.contains("unknown rule selector(s): missing rule"));
	assert!(error.contains("available: write marker"));
	assert_eq!(value["unknownSelectors"], serde_json::json!(["missing rule"]));
	assert_eq!(value["availableSelectors"], serde_json::json!(["write marker"]));
	assert_eq!(value["suggestions"], serde_json::json!([]));
	let stderr = stderr_text(&output);
	assert!(stderr.contains("unknown rule selector(s): missing rule"));
	assert!(stderr.contains("available: write marker"));
}

#[test]
fn explain_json_rejects_unknown_rule_selector_as_json() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "write marker", "packages/a/package-lock.json", "true");

	let output = run_pullhook(repo_root, &["explain", "--rule", "missing rule", "--json"]);

	assert!(
		!output.status.success(),
		"explain should fail for an unknown rule selector"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse selector error json");
	assert_eq!(value["status"], "error");
	let error = value["error"].as_str().expect("error message");
	assert!(error.contains("unknown rule selector(s): missing rule"));
	assert!(error.contains("available: write marker"));
	assert_eq!(value["unknownSelectors"], serde_json::json!(["missing rule"]));
	assert_eq!(value["availableSelectors"], serde_json::json!(["write marker"]));
	assert_eq!(value["suggestions"], serde_json::json!([]));
	let stderr = stderr_text(&output);
	assert!(stderr.contains("unknown rule selector(s): missing rule"));
}

#[test]
fn run_suggests_nearby_rule_selector_for_typos() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "typecheck", "packages/a/package-lock.json", "true");

	let output = run_pullhook(repo_root, &["run", "--dry-run", "--rule", "typcheck"]);

	assert!(!output.status.success(), "run should fail for a mistyped selector");
	let stderr = stderr_text(&output);
	assert!(stderr.contains("unknown rule selector(s): typcheck"));
	assert!(stderr.contains("did you mean `typecheck`?"));
	assert!(stderr.contains("available: typecheck"));
}

#[test]
fn run_json_rejects_debug_mode() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "write marker", "packages/a/package-lock.json", "true");

	let output = run_pullhook(repo_root, &["run", "--json", "--debug"]);

	assert!(!output.status.success(), "run --json --debug should fail");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse debug conflict json");
	assert_eq!(value["status"], "error");
	assert_eq!(value["error"], "--json cannot be used with --debug");
	let stderr = stderr_text(&output);
	assert!(stderr.contains("--json cannot be used with --debug"));
}

#[test]
fn config_validate_and_dry_run_reject_malformed_run_command() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "bad command",
      "changed": "packages/*/package-lock.json",
      "run": "sh -c 'echo nope"
    }
  ]
}
"#,
	);

	let validate = run_pullhook(repo_root, &["validate", "--render", "never"]);
	assert!(
		!validate.status.success(),
		"malformed run command should fail validation"
	);
	assert_malformed_run_command(&validate);

	let dry_run = run_pullhook(repo_root, &["run", "--dry-run", "--render", "never"]);
	assert!(!dry_run.status.success(), "malformed run command should fail dry run");
	assert_malformed_run_command(&dry_run);
}

#[test]
fn config_dry_run_plans_matching_rule_without_execution() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(
		repo_root,
		"write marker",
		"packages/*/package-lock.json",
		"sh -c 'echo ran > .pullhook-config-marker'",
	);

	let output = run_pullhook(repo_root, &["run", "--dry-run", "--render", "never"]);

	assert!(output.status.success(), "dry run should succeed");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("[match] write marker"));
	assert!(stdout.contains("command: sh -c 'echo ran > .pullhook-config-marker'"));
	assert!(!predicate::path::is_file().eval(&repo_root.join(".pullhook-config-marker")));
}

#[test]
fn config_no_match_reports_zero_matched_files() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(
		repo_root,
		"write marker",
		"**/*.md",
		"sh -c 'echo ran > .pullhook-config-no-match-marker'",
	);

	let dry_run = run_pullhook(repo_root, &["run", "--dry-run", "--render", "never"]);
	assert!(dry_run.status.success(), "no-match dry run should succeed");
	let dry_run_stdout = stdout_text(&dry_run);
	assert!(dry_run_stdout.contains("matched files: 0"));
	assert!(dry_run_stdout.contains("planned commands: 0"));

	let run = run_pullhook(repo_root, &["run", "--render", "never"]);
	assert!(run.status.success(), "no-match run should succeed");
	assert!(!predicate::path::is_file().eval(&repo_root.join(".pullhook-config-no-match-marker")));
	let run_stdout = stdout_text(&run);
	assert!(run_stdout.contains("matched files: 0"));
	assert!(run_stdout.contains("task dirs: 0"));
}

#[test]
fn config_run_executes_rule_once_from_repo_root() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(
		repo_root,
		"write marker",
		"packages/*/package-lock.json",
		"sh -c 'echo ran > .pullhook-config-marker'",
	);

	let output = run_pullhook(repo_root, &["run", "--render", "never"]);

	assert!(output.status.success(), "config run should succeed");
	assert!(predicate::path::is_file().eval(&repo_root.join(".pullhook-config-marker")));
	assert!(!predicate::path::is_file().eval(&repo_root.join("packages/a/.pullhook-config-marker")));
}

#[test]
fn config_without_run_if_base_missing_fails_when_diff_base_is_missing() {
	let temp = setup_repo_without_diff_base();
	let repo_root = temp.path();
	write_config_rule(
		repo_root,
		"write marker",
		"packages/*/package-lock.json",
		"sh -c 'echo ran > .pullhook-base-missing-marker'",
	);

	let output = run_pullhook(repo_root, &["run", "--render", "never"]);

	assert!(!output.status.success(), "missing diff base should fail without opt-in");
	assert!(!predicate::path::is_file().eval(&repo_root.join(".pullhook-base-missing-marker")));
	let stderr = stderr_text(&output);
	assert!(stderr.contains("unable to resolve diff base"));
}

#[test]
fn config_run_if_base_missing_runs_rule_without_diff_base() {
	let temp = setup_repo_without_diff_base();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "recover without base",
      "changed": "packages/*/package-lock.json",
      "runIfBaseMissing": true,
      "run": "sh -c 'echo ran > .pullhook-base-missing-marker'"
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["run", "--render", "never"]);

	assert!(
		output.status.success(),
		"runIfBaseMissing rule should run without a diff base"
	);
	assert!(predicate::path::is_file().eval(&repo_root.join(".pullhook-base-missing-marker")));
	let stdout = stdout_text(&output);
	assert!(stdout.contains("[match] recover without base"));
	assert!(stdout.contains("matched files: 1"));
}

#[test]
fn config_run_if_base_missing_json_reports_changed_file_source() {
	let temp = setup_repo_without_diff_base();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "recover without base",
      "changed": "packages/*/package-lock.json",
      "runIfBaseMissing": true,
      "run": "true"
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["run", "--dry-run", "--json"]);

	assert!(
		output.status.success(),
		"runIfBaseMissing dry run should succeed without a diff base"
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse run json");
	assert_eq!(value["baseMissing"], true);
	assert_eq!(value["changedFilesSource"], "base-missing");
	assert_eq!(value["entries"][0]["matches"], serde_json::json!(["."]));
}

#[test]
fn config_parallel_run_if_base_missing_runs_opted_in_child() {
	let temp = setup_repo_without_diff_base();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "parallel base recovery",
      "parallel": [
        {
          "name": "skip without opt-in",
          "changed": "packages/*/package-lock.json",
          "run": "sh -c 'echo skipped > .pullhook-base-missing-skipped'"
        },
        {
          "name": "recover without base",
          "changed": "packages/*/package-lock.json",
          "runIfBaseMissing": true,
          "run": "sh -c 'echo ran > .pullhook-base-missing-parallel'"
        }
      ]
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["run", "--render", "never"]);

	assert!(
		output.status.success(),
		"parallel runIfBaseMissing child should run without a diff base"
	);
	assert!(predicate::path::is_file().eval(&repo_root.join(".pullhook-base-missing-parallel")));
	assert!(!predicate::path::is_file().eval(&repo_root.join(".pullhook-base-missing-skipped")));
}

#[test]
fn config_on_failure_continue_runs_later_rules() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "onFailure": "continue",
  "rules": [
    {
      "name": "fail first",
      "changed": "packages/*/package-lock.json",
      "run": "sh -c 'exit 7'",
      "failText": "{red.bold {rule} failed}"
    },
    {
      "name": "run second",
      "changed": "packages/*/package-lock.json",
      "run": "sh -c 'echo ran > .pullhook-continue-marker'"
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["run", "--render", "never"]);

	assert!(!output.status.success(), "failed rule should exit non-zero");
	assert!(predicate::path::is_file().eval(&repo_root.join(".pullhook-continue-marker")));
	let stderr = stderr_text(&output);
	assert!(stderr.contains("fail first failed"));
}

#[test]
fn config_run_reports_interrupted_rules_separately_from_failures() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(
		repo_root,
		"interrupt",
		"packages/*/package-lock.json",
		"sh -c 'kill -TERM $$'",
	);

	let output = run_pullhook(repo_root, &["run", "--render", "never"]);

	assert!(!output.status.success(), "interrupted rule should exit non-zero");

	let stdout = stdout_text(&output);
	assert!(stdout.contains("failed: 0"));
	assert!(stdout.contains("interrupted: 1"));
	assert!(stdout.contains("[warn] 1 task(s) interrupted"));

	let stderr = stderr_text(&output);
	assert!(stderr.contains("[warn] task interrupted"));
	assert!(stderr.contains("error: 1 config rule(s) failed"));
}

#[test]
fn config_install_rule_reuses_package_manager_detection() {
	let temp = setup_repo_with_root_manifest_change();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "install dependencies",
      "install": true
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["run", "--dry-run", "--render", "never"]);

	assert!(output.status.success(), "install dry run should succeed");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("[match] install dependencies"));
	assert!(stdout.contains("command: npm install"));
}

#[test]
fn config_parallel_group_runs_matched_rules_from_repo_root() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "parallel checks",
      "jobs": 2,
      "parallel": [
        {
          "name": "write first marker",
          "changed": "packages/*/package-lock.json",
          "run": "sh -c 'echo first > .pullhook-parallel-first'"
        },
        {
          "name": "write second marker",
          "changed": "packages/*/package-lock.json",
          "run": "sh -c 'echo second > .pullhook-parallel-second'"
        }
      ]
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["run", "--render", "never"]);

	assert!(output.status.success(), "parallel group should succeed");
	assert!(predicate::path::is_file().eval(&repo_root.join(".pullhook-parallel-first")));
	assert!(predicate::path::is_file().eval(&repo_root.join(".pullhook-parallel-second")));
	assert!(!predicate::path::is_file().eval(&repo_root.join("packages/a/.pullhook-parallel-first")));
	assert!(!predicate::path::is_file().eval(&repo_root.join("packages/a/.pullhook-parallel-second")));
}

#[test]
fn config_parallel_group_reports_default_jobs_when_omitted() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "parallel checks",
      "parallel": [
        {
          "name": "write first marker",
          "changed": "packages/*/package-lock.json",
          "run": "sh -c 'echo first > .pullhook-parallel-first'"
        },
        {
          "name": "write second marker",
          "changed": "packages/*/package-lock.json",
          "run": "sh -c 'echo second > .pullhook-parallel-second'"
        }
      ]
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["run", "--dry-run", "--render", "never"]);

	assert!(output.status.success(), "parallel dry run should succeed");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("group: parallel checks"));
	assert!(stdout.contains("jobs: default"));
}

#[test]
fn validate_rejects_nested_parallel_groups() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		r#"{
  "rules": [
    {
      "name": "outer",
      "parallel": [
        {
          "name": "inner",
          "parallel": [
            {
              "name": "leaf",
              "changed": "packages/*/package-lock.json",
              "run": "true"
            }
          ]
        }
      ]
    }
  ]
}
"#,
	);

	let output = run_pullhook(repo_root, &["validate", "--render", "never"]);

	assert!(!output.status.success(), "nested parallel group should fail");
	let stderr = stderr_text(&output);
	assert!(stderr.contains("nested parallel groups are not supported"));
}

fn run_pullhook(repo_root: &Path, args: &[&str]) -> Output {
	run_pullhook_with_env(repo_root, args, &[])
}

fn run_pullhook_with_env(repo_root: &Path, args: &[&str], envs: &[(&str, &str)]) -> Output {
	let mut command = ProcessCommand::new(assert_cmd::cargo::cargo_bin!("pullhook"));
	command.current_dir(repo_root).args(args);

	for &(key, value) in envs {
		command.env(key, value);
	}

	command.output().expect("command runs")
}

fn run_pullhook_with_stdin(repo_root: &Path, args: &[&str], stdin: &str) -> Output {
	let mut child = ProcessCommand::new(assert_cmd::cargo::cargo_bin!("pullhook"))
		.current_dir(repo_root)
		.args(args)
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.expect("command starts");

	child
		.stdin
		.as_mut()
		.expect("stdin is piped")
		.write_all(stdin.as_bytes())
		.expect("stdin writes");

	child.wait_with_output().expect("command runs")
}

fn stdout_text(output: &Output) -> String {
	String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr_text(output: &Output) -> String {
	String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_install_dry_run_matches_repo_root(output: &Output) {
	assert!(output.status.success(), "install dry run should succeed");

	let stdout = stdout_text(output);
	assert!(stdout.contains("matched: 1"));
	assert!(stdout.contains("directory: ."));
	assert!(stdout.contains("command: npm install"));
}

fn assert_malformed_run_command(output: &Output) {
	let stdout = stdout_text(output);
	assert!(
		stdout.trim().is_empty(),
		"malformed config should fail before writing stdout:\n{stdout}"
	);

	let stderr = stderr_text(output);
	assert!(stderr.contains("invalid `run` command"));
	assert!(stderr.contains("invalid command `sh -c 'echo nope`"));
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

fn setup_repo_without_diff_base() -> TempDir {
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

	run_git(repo_root, &["add", "."]);
	run_git(repo_root, &["commit", "-m", "initial"]);
	clear_diff_base_files(repo_root);

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

fn clear_diff_base_files(repo_root: &Path) {
	let _ = fs::remove_file(repo_root.join(".git/logs/HEAD"));
	let _ = fs::remove_dir_all(repo_root.join(".git/logs/refs"));
	let _ = fs::remove_file(repo_root.join(".git/ORIG_HEAD"));
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

fn write_config_rule(repo_root: &Path, name: &str, changed: &str, run: &str) {
	write_file(
		repo_root,
		Path::new("pullhook.json"),
		&format!(
			r#"{{
  "rules": [
    {{
      "name": "{name}",
      "changed": "{changed}",
      "run": "{run}"
    }}
  ]
}}
"#
		),
	);
}

fn run_git(repo_root: &Path, args: &[&str]) {
	let status = ProcessCommand::new("git")
		.current_dir(repo_root)
		.args(args)
		.status()
		.expect("run git command");

	assert!(status.success(), "git command failed: git {}", args.join(" "));
}
