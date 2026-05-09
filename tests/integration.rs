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
		let stderr = stderr_text(&output);
		assert!(
			stderr.contains("--json cannot be used with --debug"),
			"`pullhook {}` stderr should explain the conflict",
			args.join(" ")
		);
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
fn root_help_lists_common_examples() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["--help"]);

	assert!(output.status.success(), "root help should succeed");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("Examples:"));
	assert!(stdout.contains("pullhook --install --dry-run"));
	assert!(stdout.contains("pullhook init --format json"));
	assert!(stdout.contains("schema"));
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
	assert!(stdout.contains("pullhook run --quiet"));
	assert!(stdout.contains("pullhook run --config config/pullhook.custom.json --all-matches"));
	assert!(stdout.contains("--no-color"));
	assert!(stdout.contains("--quiet"));
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
fn init_help_lists_generation_examples() {
	let temp = tempfile::tempdir().expect("create temp dir");

	let output = run_pullhook(temp.path(), &["init", "--help"]);

	assert!(output.status.success(), "init help should succeed");
	let stdout = stdout_text(&output);
	assert!(stdout.contains("pullhook init --stdout"));
	assert!(stdout.contains("pullhook init --force"));
	assert!(stdout.contains("pullhook init --format yaml"));
	assert!(stdout.contains("pullhook init --output config/pullhook.custom.json"));
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
	assert!(value["path"].as_str().expect("path").ends_with("pullhook.json"));
	assert_eq!(value["format"], "json");
	assert_eq!(value["exists"], true);
	assert_eq!(value["explicit"], false);
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
	assert_eq!(value["valid"], false);
	assert!(value["path"].as_str().expect("path").ends_with("pullhook.json"));
	assert!(
		value["error"]
			.as_str()
			.expect("error")
			.contains("unknown style `sparkle`")
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
	assert_eq!(value["valid"], false);
	assert!(value["path"].is_null());
	assert!(
		value["error"]
			.as_str()
			.expect("error")
			.contains("no pullhook config found")
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
	let stderr = stderr_text(&output);
	assert!(stderr.contains("no pullhook config found"));
}

#[test]
fn rules_json_reports_missing_config_as_json() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(repo_root, &["rules", "--json"]);

	assert!(!output.status.success(), "rules --json should fail without config");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse rules error json");
	assert_eq!(value["status"], "error");
	assert!(
		value["error"]
			.as_str()
			.expect("error")
			.contains("no pullhook config found")
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
	assert_eq!(
		checks[1]["hint"],
		"run `pullhook explain --all-matches` to preview rule matches"
	);
	assert_eq!(checks[3]["hint"], "use `install: true` for dependency-recovery rules");
	let stderr = stderr_text(&output);
	assert!(stderr.trim().is_empty(), "doctor --json should not write stderr");
}

#[test]
fn doctor_strict_fails_on_warnings() {
	let temp = setup_repo_with_merge();
	let repo_root = temp.path();

	let output = run_pullhook(repo_root, &["doctor", "--strict", "--json"]);

	assert!(!output.status.success(), "doctor --strict should fail on warnings");
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse doctor json");
	let warn_count = value["summary"]["warn"].as_u64().expect("warn count");
	assert!(warn_count >= 1, "strict doctor should report at least one warning");
	assert_eq!(value["summary"]["error"], 0);
	let checks = value["checks"].as_array().expect("checks array");
	assert_eq!(checks[1]["name"], "config");
	assert_eq!(checks[1]["level"], "warn");
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
	assert_eq!(value["summary"]["error"], 1);
	let checks = value["checks"].as_array().expect("checks array");
	assert_eq!(checks[1]["name"], "config");
	assert_eq!(checks[1]["level"], "error");
	assert_eq!(checks[1]["hint"], "run `pullhook validate` after editing the config");
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
	assert_eq!(value["rules"], 2);
	assert_eq!(value["parallelGroups"], 1);
	assert_eq!(
		value["selectors"],
		serde_json::json!(["checks", "install dependencies", "lint"])
	);
	let entries = value["entries"].as_array().expect("entries array");
	assert_eq!(entries.len(), 2);
	assert_eq!(entries[0]["type"], "rule");
	assert_eq!(entries[0]["name"], "install dependencies");
	assert_eq!(entries[0]["kind"], "install");
	assert_eq!(entries[1]["type"], "group");
	assert_eq!(entries[1]["name"], "checks");
	assert_eq!(entries[1]["rules"][0]["name"], "lint");
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
	assert_eq!(value["kind"], "install");
	assert_eq!(value["rules"], 1);
	assert_eq!(value["parallelGroups"], 0);
	assert_eq!(value["selectors"], serde_json::json!(["install dependencies"]));
	let entries = value["entries"].as_array().expect("entries array");
	assert_eq!(entries.len(), 1);
	assert_eq!(entries[0]["name"], "install dependencies");
	assert_eq!(entries[0]["kind"], "install");
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
	assert_eq!(value["mode"], "dry-run");
	assert_eq!(value["changedFilesSource"], "git");
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
