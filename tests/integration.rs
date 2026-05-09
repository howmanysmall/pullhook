//! Integration tests for pullhook CLI behavior.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Output};

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
	assert_eq!(value["summary"]["passed"], 0);
	assert_eq!(value["summary"]["failed"], 1);
	assert_eq!(value["results"][0]["state"], "failed");
	let stderr = stderr_text(&output);
	assert!(stderr.contains("1 task(s) failed"));
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
	let stderr = stderr_text(&output);
	assert!(stderr.contains("--json cannot be used with --debug"));
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
fn root_help_lists_common_examples() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["--help"]);

	assert!(output.status.success(), "root help should succeed");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("Examples:"));
	assert!(stdout.contains("pullhook --install --dry-run"));
	assert!(stdout.contains("pullhook init --format json"));
	assert!(stdout.contains("Use `pullhook explain --all-matches` to preview config rule matches."));
}

#[test]
fn run_help_lists_json_examples() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["run", "--help"]);

	assert!(output.status.success(), "run help should succeed");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("pullhook run --dry-run"));
	assert!(stdout.contains("pullhook run --json"));
	assert!(stdout.contains("pullhook run --config config/pullhook.custom.json --all-matches"));
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
fn init_refuses_to_overwrite_existing_config_without_force() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_file(repo_root, Path::new("pullhook.json"), "{\"rules\":[]}\n");

	let output = run_pullhook(repo_root, &["init", "--render", "never"]);

	assert!(!output.status.success(), "init should reject overwriting config");
	let stderr = stderr_text(&output);
	assert!(stderr.contains("pullhook config already exists"));
	assert!(stderr.contains("--force"));
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
	assert_eq!(value["valid"], true);
	assert_eq!(value["entries"], 2);
	assert_eq!(value["rules"], 2);
	assert_eq!(value["parallelGroups"], 1);
	assert_eq!(value["onFailure"], "stop");
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "validate --json should not write stderr");
}

#[test]
fn doctor_json_reports_repo_config_diff_base_and_install_detection() {
	let temp = setup_repo_with_root_manifest_change();
	let repo_root = temp.path();
	write_config_rule(repo_root, "rebuild root", "package-lock.json", "npm test");

	let output = run_pullhook(repo_root, &["doctor", "--json"]);

	assert!(output.status.success(), "doctor --json should succeed");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse doctor json");
	assert_eq!(value["summary"]["ok"], 4);
	assert_eq!(value["summary"]["warn"], 0);
	assert_eq!(value["summary"]["error"], 0);
	let checks = value["checks"].as_array().expect("checks array");
	assert_eq!(checks.len(), 4);
	assert_eq!(checks[0]["name"], "repository");
	assert_eq!(checks[1]["name"], "config");
	assert_eq!(checks[2]["name"], "diff base");
	assert_eq!(checks[3]["name"], "install detection");
	assert_eq!(checks[3]["summary"], "detected npm");
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "doctor --json should not write stderr");
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
	assert_eq!(value["summary"]["error"], 1);
	let checks = value["checks"].as_array().expect("checks array");
	assert_eq!(checks[1]["name"], "config");
	assert_eq!(checks[1]["level"], "error");
	let stderr = stderr_text(&output);
	assert!(stderr.contains("doctor found blocking issues"));
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
	assert_eq!(value["baseMissing"], false);
	assert_eq!(value["onFailure"], "stop");
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
	assert_eq!(value["mode"], "dry-run");
	assert_eq!(value["plannedCommands"], 1);
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
	assert_eq!(value["mode"], "run");
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
fn run_rejects_unknown_rule_selector() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "write marker", "packages/a/package-lock.json", "true");

	let output = run_pullhook(repo_root, &["run", "--dry-run", "--rule", "missing rule", "--json"]);

	assert!(!output.status.success(), "run should fail for an unknown rule selector");
	let stderr = stderr_text(&output);
	assert!(stderr.contains("unknown rule selector(s): missing rule"));
	assert!(stderr.contains("available: write marker"));
}

#[test]
fn run_json_rejects_debug_mode() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();
	write_config_rule(repo_root, "write marker", "packages/a/package-lock.json", "true");

	let output = run_pullhook(repo_root, &["run", "--json", "--debug"]);

	assert!(!output.status.success(), "run --json --debug should fail");
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
