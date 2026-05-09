//! Config-file discovery, parsing, validation, and rule evaluation.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::PullhookError;
use crate::matcher;
use crate::output::RenderMode;

const CONFIG_NAMES: &[&str] = &[
	"pullhook.json",
	"pullhook.jsonc",
	"pullhook.yaml",
	"pullhook.toml",
	".pullhook.json",
	".pullhook.jsonc",
	".pullhook.yaml",
	".pullhook.toml",
];

const UNSUPPORTED_CONFIG_NAMES: &[&str] = &["pullhook.json5", "pullhook.yml", ".pullhook.json5", ".pullhook.yml"];

const STYLES: &[&str] = &[
	"black",
	"red",
	"green",
	"yellow",
	"blue",
	"magenta",
	"cyan",
	"white",
	"gray",
	"grey",
	"bgBlack",
	"bgRed",
	"bgGreen",
	"bgYellow",
	"bgBlue",
	"bgMagenta",
	"bgCyan",
	"bgWhite",
	"bold",
	"dim",
	"italic",
	"underline",
];

/// Starter config written by `pullhook init`.
pub const STARTER_CONFIG: &str = r#"{
  "$schema": "https://pullhook.dev/schema.json",
  "onFailure": "stop",
  "rules": [
    {
      "name": "install dependencies",
      "install": true
    }
  ]
}
"#;

/// Config file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
	/// JSON.
	Json,
	/// JSON with comments.
	Jsonc,
	/// YAML using `.yaml`.
	Yaml,
	/// TOML.
	Toml,
}

impl ConfigFormat {
	fn from_path(path: &Path) -> Result<Self, PullhookError> {
		match path.extension().and_then(|extension| extension.to_str()) {
			Some("json") => Ok(Self::Json),
			Some("jsonc") => Ok(Self::Jsonc),
			Some("yaml") => Ok(Self::Yaml),
			Some("toml") => Ok(Self::Toml),
			Some("json5") => Err(PullhookError::Message(
				"JSON5 configs are not supported; use `pullhook.json` or `pullhook.jsonc`".to_owned(),
			)),
			Some("yml") => Err(PullhookError::Message(
				"`*.yml` configs are not supported; use `pullhook.yaml`".to_owned(),
			)),
			Some(extension) => Err(PullhookError::Message(format!(
				"unsupported config extension `.{extension}`"
			))),
			None => Err(PullhookError::Message("config path has no extension".to_owned())),
		}
	}
}

/// Loaded and validated config.
#[derive(Debug, Clone)]
pub struct Config {
	/// Source file path.
	pub path: PathBuf,
	/// Top-level failure policy.
	pub on_failure: OnFailure,
	/// Top-level entries.
	pub entries: Vec<Entry>,
}

/// Top-level failure policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum OnFailure {
	/// Stop after first failed top-level entry.
	#[default]
	Stop,
	/// Continue later top-level entries and exit non-zero at the end.
	Continue,
}

/// A top-level config entry.
#[derive(Debug, Clone)]
pub enum Entry {
	/// Single rule.
	Rule(Rule),
	/// Parallel rule group.
	Group(Group),
}

/// Rule command definition.
#[derive(Debug, Clone)]
pub struct Rule {
	/// Rule name.
	pub name: String,
	/// Changed globs.
	pub changed: Vec<Pattern>,
	/// Excluded globs.
	pub exclude: Vec<Pattern>,
	/// Command to run from repo root.
	pub run: Option<String>,
	/// Whether this rule runs package-manager install.
	pub install: bool,
	/// Run when no diff base can be resolved.
	pub run_if_base_missing: bool,
	/// Optional failure text.
	pub fail_text: Option<FailText>,
}

/// A validated glob pattern and its compiled matcher.
#[derive(Debug, Clone)]
pub struct Pattern {
	matcher: matcher::Matcher,
}

impl Pattern {
	/// Compile a user-facing pattern once.
	pub fn new(raw: impl Into<String>) -> Result<Self, PullhookError> {
		let raw = raw.into();
		let matcher = matcher::compile(&raw)?;
		Ok(Self { matcher })
	}

	fn is_match(&self, path: &Path) -> bool {
		self.matcher.is_match(path)
	}
}

/// Parallel group definition.
#[derive(Debug, Clone)]
pub struct Group {
	/// Group name.
	pub name: String,
	/// Max group concurrency.
	pub jobs: Option<usize>,
	/// Child rules.
	pub rules: Vec<Rule>,
	/// Optional group failure text.
	pub fail_text: Option<FailText>,
}

/// Validated fail-text template.
#[derive(Debug, Clone)]
pub struct FailText {
	raw: String,
}

impl FailText {
	/// Render without ANSI styling.
	#[must_use]
	pub fn render_plain(&self, context: &FailTextContext<'_>) -> String {
		render_template(&self.raw, context, false)
	}

	/// Render with the selected render mode.
	#[must_use]
	pub fn render(&self, context: &FailTextContext<'_>, render_mode: RenderMode) -> String {
		if render_mode.effective().use_style() {
			render_template(&self.raw, context, true)
		} else {
			self.render_plain(context)
		}
	}
}

/// Runtime values available to `failText`.
#[derive(Debug, Clone, Copy)]
pub struct FailTextContext<'a> {
	/// Rule or group name.
	pub rule: &'a str,
	/// Command text.
	pub command: &'a str,
	/// Current working directory label.
	pub cwd: &'a str,
	/// Exit code label.
	pub exit_code: &'a str,
}

/// Evaluated config entry.
#[derive(Debug, Clone)]
pub enum EvaluatedEntry {
	/// Evaluated single rule.
	Rule(EvaluatedRule),
	/// Evaluated parallel group.
	Group(EvaluatedGroup),
}

/// Evaluated single rule.
#[derive(Debug, Clone)]
pub struct EvaluatedRule {
	/// Source rule.
	pub rule: Rule,
	/// Matched changed files.
	pub matches: Vec<PathBuf>,
	/// Command planned for execution.
	pub command: Option<String>,
	/// Skip reason.
	pub skip_reason: Option<String>,
}

impl EvaluatedRule {
	/// Whether this rule should execute.
	#[must_use]
	pub const fn should_run(&self) -> bool {
		self.skip_reason.is_none() && self.command.is_some() && !self.matches.is_empty()
	}
}

/// Evaluated parallel group.
#[derive(Debug, Clone)]
pub struct EvaluatedGroup {
	/// Source group.
	pub group: Group,
	/// Evaluated child rules.
	pub rules: Vec<EvaluatedRule>,
}

impl EvaluatedGroup {
	/// Whether at least one child should run.
	#[must_use]
	pub fn should_run(&self) -> bool {
		self.rules.iter().any(EvaluatedRule::should_run)
	}
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawConfig {
	#[serde(default, rename = "$schema")]
	_schema: Option<String>,
	#[serde(default)]
	on_failure: OnFailure,
	#[serde(default)]
	rules: Vec<RawEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawEntry {
	name: Option<String>,
	changed: Option<PatternList>,
	exclude: Option<PatternList>,
	run: Option<String>,
	#[serde(default)]
	install: bool,
	#[serde(default)]
	run_if_base_missing: bool,
	fail_text: Option<String>,
	jobs: Option<usize>,
	parallel: Option<Vec<Self>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PatternList {
	One(String),
	Many(Vec<String>),
}

impl PatternList {
	fn into_vec(self) -> Vec<String> {
		match self {
			Self::One(value) => vec![value],
			Self::Many(values) => values,
		}
	}
}

/// Return all supported config names in discovery order.
#[must_use]
pub const fn config_names() -> &'static [&'static str] {
	CONFIG_NAMES
}

/// Discover exactly one supported config file from the repo root.
pub fn discover(repo_root: &Path) -> Result<Option<PathBuf>, PullhookError> {
	let found: Vec<_> = CONFIG_NAMES
		.iter()
		.map(|name| repo_root.join(name))
		.filter(|path| path.is_file())
		.collect();

	match found.as_slice() {
		[] => discover_unsupported_config(repo_root),
		[path] => Ok(Some(path.clone())),
		_ => Err(PullhookError::Message(format!(
			"multiple pullhook config files found: {}",
			found
				.iter()
				.map(|path| path.file_name().unwrap_or_default().to_string_lossy())
				.collect::<Vec<_>>()
				.join(", ")
		))),
	}
}

fn discover_unsupported_config(repo_root: &Path) -> Result<Option<PathBuf>, PullhookError> {
	UNSUPPORTED_CONFIG_NAMES
		.iter()
		.map(|name| repo_root.join(name))
		.find(|path| path.is_file())
		.map_or(Ok(None), |path| ConfigFormat::from_path(&path).map(|_| None))
}

/// Load and validate a config file.
pub fn load(path: &Path) -> Result<Config, PullhookError> {
	let text = std::fs::read_to_string(path)
		.map_err(|source| PullhookError::Message(format!("failed to read config `{}`: {source}", path.display())))?;
	let format = ConfigFormat::from_path(path)?;
	let raw = parse_raw(path, &text, format)?;
	normalize_config(path, raw)
}

/// Evaluate config entries against changed files.
pub fn evaluate(
	config: &Config,
	changed_files: &[PathBuf],
	base_missing: bool,
	mut install_plan: impl FnMut(&Rule) -> Result<(Option<String>, Vec<Pattern>), PullhookError>,
) -> Result<Vec<EvaluatedEntry>, PullhookError> {
	let mut evaluated = Vec::with_capacity(config.entries.len());
	for entry in &config.entries {
		evaluated.push(match entry {
			Entry::Rule(rule) => {
				evaluate_rule(rule, changed_files, base_missing, &mut install_plan).map(EvaluatedEntry::Rule)?
			}
			Entry::Group(group) => {
				let mut rules = Vec::with_capacity(group.rules.len());
				for rule in &group.rules {
					rules.push(evaluate_rule(rule, changed_files, base_missing, &mut install_plan)?);
				}
				EvaluatedEntry::Group(EvaluatedGroup {
					group: group.clone(),
					rules,
				})
			}
		});
	}

	Ok(evaluated)
}

fn parse_raw(path: &Path, text: &str, format: ConfigFormat) -> Result<RawConfig, PullhookError> {
	match format {
		ConfigFormat::Json => serde_json::from_str(text).map_err(|error| config_parse_error(path, error)),
		ConfigFormat::Jsonc => {
			let value =
				jsonc_parser::parse_to_serde_value::<serde_json::Value>(text, &jsonc_parser::ParseOptions::default())
					.map_err(|error| config_parse_error(path, error))?;
			serde_json::from_value(value).map_err(|error| config_parse_error(path, error))
		}
		ConfigFormat::Yaml => serde_yaml::from_str(text).map_err(|error| config_parse_error(path, error)),
		ConfigFormat::Toml => toml::from_str(text).map_err(|error| config_parse_error(path, error)),
	}
}

fn config_parse_error(path: &Path, error: impl std::fmt::Display) -> PullhookError {
	PullhookError::ConfigParse {
		path: path.display().to_string(),
		reason: error.to_string(),
	}
}

fn normalize_config(path: &Path, raw: RawConfig) -> Result<Config, PullhookError> {
	let mut errors = Vec::new();
	if raw.rules.is_empty() {
		errors.push("rules must contain at least one entry".to_owned());
	}

	let mut entries = Vec::new();
	for (index, raw_entry) in raw.rules.into_iter().enumerate() {
		let label = format!("rules[{index}]");
		if raw_entry.parallel.is_some() {
			if let Some(group) = normalize_group(raw_entry, &label, &mut errors) {
				entries.push(Entry::Group(group));
			}
		} else if let Some(rule) = normalize_rule(raw_entry, &label, &mut errors) {
			entries.push(Entry::Rule(rule));
		}
	}

	if !errors.is_empty() {
		return Err(PullhookError::ConfigValidation {
			path: path.display().to_string(),
			details: errors.join("\n"),
		});
	}

	Ok(Config {
		path: path.to_path_buf(),
		on_failure: raw.on_failure,
		entries,
	})
}

fn normalize_group(raw: RawEntry, label: &str, errors: &mut Vec<String>) -> Option<Group> {
	let name = normalize_name(raw.name, label, errors);
	let children = raw.parallel.unwrap_or_default();
	if children.is_empty() {
		errors.push(format!("{label}: `parallel` must contain at least one rule"));
	}
	if raw.changed.is_some() || raw.exclude.is_some() || raw.run.is_some() || raw.install || raw.run_if_base_missing {
		errors.push(format!(
			"{label}: parallel groups cannot also define rule fields (`changed`, `exclude`, `run`, `install`, `runIfBaseMissing`)"
		));
	}
	if raw.jobs == Some(0) {
		errors.push(format!("{label}: `jobs` must be greater than zero"));
	}

	let rules = children
		.into_iter()
		.enumerate()
		.filter_map(|(index, child)| {
			let child_label = format!("{label}.parallel[{index}]");
			if child.parallel.is_some() {
				errors.push(format!("{child_label}: nested parallel groups are not supported"));
				None
			} else {
				normalize_rule(child, &child_label, errors)
			}
		})
		.collect();

	let fail_text = normalize_fail_text(raw.fail_text, label, errors);

	name.map(|name| Group {
		name,
		jobs: raw.jobs,
		rules,
		fail_text,
	})
}

fn normalize_rule(raw: RawEntry, label: &str, errors: &mut Vec<String>) -> Option<Rule> {
	let name = normalize_name(raw.name, label, errors);
	if raw.jobs.is_some() {
		errors.push(format!("{label}: `jobs` is only valid on parallel groups"));
	}
	let has_changed = raw.changed.is_some();
	let changed = if raw.install {
		Vec::new()
	} else {
		normalize_patterns(raw.changed, label, "`changed`", errors)
	};
	let exclude = normalize_optional_patterns(raw.exclude, label, "`exclude`", errors);
	if raw.run.as_ref().is_some_and(|value| value.trim().is_empty()) {
		errors.push(format!("{label}: `run` cannot be empty"));
	}
	if raw.run.is_some() && raw.install {
		errors.push(format!("{label}: `run` and `install` cannot be used together"));
	}
	if raw.run.is_none() && !raw.install {
		errors.push(format!("{label}: define either `run` or `install: true`"));
	}
	if raw.install && has_changed {
		errors.push(format!(
			"{label}: `install: true` uses package-manager watched files and cannot define `changed`"
		));
	}
	let fail_text = normalize_fail_text(raw.fail_text, label, errors);

	name.map(|name| Rule {
		name,
		changed,
		exclude,
		run: raw.run.map(|value| value.trim().to_owned()),
		install: raw.install,
		run_if_base_missing: raw.run_if_base_missing,
		fail_text,
	})
}

fn normalize_name(name: Option<String>, label: &str, errors: &mut Vec<String>) -> Option<String> {
	match name.map(|value| value.trim().to_owned()) {
		Some(value) if !value.is_empty() => Some(value),
		_ => {
			errors.push(format!("{label}: `name` is required"));
			None
		}
	}
}

fn normalize_patterns(
	patterns: Option<PatternList>,
	label: &str,
	field: &str,
	errors: &mut Vec<String>,
) -> Vec<Pattern> {
	let Some(patterns) = patterns else {
		errors.push(format!("{label}: {field} is required"));
		return Vec::new();
	};

	let values = normalize_pattern_values(patterns.into_vec(), label, field, errors);
	if values.is_empty() {
		errors.push(format!("{label}: {field} must contain at least one pattern"));
	}
	values
}

fn normalize_optional_patterns(
	patterns: Option<PatternList>,
	label: &str,
	field: &str,
	errors: &mut Vec<String>,
) -> Vec<Pattern> {
	patterns
		.map(|patterns| normalize_pattern_values(patterns.into_vec(), label, field, errors))
		.unwrap_or_default()
}

fn normalize_pattern_values(values: Vec<String>, label: &str, field: &str, errors: &mut Vec<String>) -> Vec<Pattern> {
	values
		.into_iter()
		.filter_map(|value| {
			let value = value.trim().to_owned();
			if value.is_empty() {
				errors.push(format!("{label}: {field} cannot contain empty patterns"));
				return None;
			}
			match Pattern::new(value.clone()) {
				Ok(pattern) => Some(pattern),
				Err(error) => {
					errors.push(format!("{label}: invalid {field} pattern `{value}`: {error}"));
					None
				}
			}
		})
		.collect()
}

fn normalize_fail_text(raw: Option<String>, label: &str, errors: &mut Vec<String>) -> Option<FailText> {
	let value = raw?;
	if value.trim().is_empty() {
		errors.push(format!("{label}: `failText` cannot be empty"));
	}
	if let Err(error) = validate_template(&value) {
		errors.push(format!("{label}: invalid `failText`: {error}"));
	}
	Some(FailText { raw: value })
}

fn evaluate_rule(
	rule: &Rule,
	changed_files: &[PathBuf],
	base_missing: bool,
	install_plan: &mut impl FnMut(&Rule) -> Result<(Option<String>, Vec<Pattern>), PullhookError>,
) -> Result<EvaluatedRule, PullhookError> {
	let (command, changed_patterns) = if rule.install {
		install_plan(rule)?
	} else {
		(rule.run.clone(), rule.changed.clone())
	};

	let matches = if base_missing && rule.run_if_base_missing {
		vec![PathBuf::from(".")]
	} else {
		match_rule_paths(&changed_patterns, &rule.exclude, changed_files)
	};

	let skip_reason = if matches.is_empty() {
		Some("no matching changed files".to_owned())
	} else if command.is_none() {
		Some("no command".to_owned())
	} else {
		None
	};

	Ok(EvaluatedRule {
		rule: rule.clone(),
		matches,
		command,
		skip_reason,
	})
}

/// Match changed files for a rule.
pub fn match_rule_paths(changed: &[Pattern], exclude: &[Pattern], changed_files: &[PathBuf]) -> Vec<PathBuf> {
	changed_files
		.iter()
		.filter(|path| changed.iter().any(|pattern| pattern.is_match(path)))
		.filter(|path| !exclude.iter().any(|pattern| pattern.is_match(path)))
		.cloned()
		.collect()
}

fn validate_template(template: &str) -> Result<(), String> {
	let _ = render_template_checked(
		template,
		&FailTextContext {
			rule: "rule",
			command: "command",
			cwd: ".",
			exit_code: "1",
		},
		false,
		true,
	)?;
	Ok(())
}

fn render_template(template: &str, context: &FailTextContext<'_>, styled: bool) -> String {
	render_template_checked(template, context, styled, false).unwrap_or_else(|_| template.to_owned())
}

fn render_template_checked(
	template: &str,
	context: &FailTextContext<'_>,
	styled: bool,
	validate_only: bool,
) -> Result<String, String> {
	let mut output = String::new();
	let mut cursor = 0usize;
	while let Some(start_offset) = template[cursor..].find('{') {
		let start = cursor + start_offset;
		let literal = &template[cursor..start];
		if literal.contains('}') {
			return Err("unmatched `}`".to_owned());
		}
		output.push_str(literal);
		let end = find_matching_brace(template, start).ok_or_else(|| "unclosed `{`".to_owned())?;
		let block = &template[start + 1..end];
		let rendered = render_block(block, context, styled, validate_only)?;
		output.push_str(&rendered);
		cursor = end + 1;
	}
	let tail = &template[cursor..];
	if tail.contains('}') {
		return Err("unmatched `}`".to_owned());
	}
	output.push_str(tail);

	Ok(output)
}

fn find_matching_brace(template: &str, start: usize) -> Option<usize> {
	let mut depth = 0usize;
	for (offset, ch) in template[start..].char_indices() {
		match ch {
			'{' => depth += 1,
			'}' => {
				depth = depth.saturating_sub(1);
				if depth == 0 {
					return Some(start + offset);
				}
			}
			_ => {}
		}
	}
	None
}

fn render_block(
	block: &str,
	context: &FailTextContext<'_>,
	styled: bool,
	validate_only: bool,
) -> Result<String, String> {
	match block {
		"rule" => return Ok(context.rule.to_owned()),
		"command" => return Ok(context.command.to_owned()),
		"cwd" => return Ok(context.cwd.to_owned()),
		"exitCode" => return Ok(context.exit_code.to_owned()),
		_ => {}
	}

	let Some((style_chain, text)) = block.split_once(char::is_whitespace) else {
		return Err(format!("unknown placeholder or style block `{block}`"));
	};
	if style_chain.is_empty() {
		return Err("style block is missing a style chain".to_owned());
	}
	let text = text.trim_start();
	if text.is_empty() {
		return Err(format!("style block `{style_chain}` has no text"));
	}
	for style in style_chain.split('.') {
		if !STYLES.contains(&style) {
			return Err(format!("unknown style `{style}`"));
		}
	}

	let rendered = render_template_checked(text, context, styled, validate_only)?;
	if validate_only || !styled {
		return Ok(rendered);
	}

	Ok(format!("{}{}\x1b[0m", ansi_prefix(style_chain), rendered))
}

fn ansi_prefix(style_chain: &str) -> String {
	let mut prefix = String::new();
	for style in style_chain.split('.') {
		let code = match style {
			"black" => "30",
			"red" => "31",
			"green" => "32",
			"yellow" => "33",
			"blue" => "34",
			"magenta" => "35",
			"cyan" => "36",
			"white" => "37",
			"gray" | "grey" => "90",
			"bgBlack" => "40",
			"bgRed" => "41",
			"bgGreen" => "42",
			"bgYellow" => "43",
			"bgBlue" => "44",
			"bgMagenta" => "45",
			"bgCyan" => "46",
			"bgWhite" => "47",
			"bold" => "1",
			"dim" => "2",
			"italic" => "3",
			"underline" => "4",
			_ => continue,
		};
		let _ = write!(prefix, "\x1b[{code}m");
	}
	prefix
}

#[cfg(test)]
mod tests {
	use std::fs;
	use std::path::PathBuf;

	use tempfile::tempdir;

	use super::*;

	#[test]
	fn discovers_single_config() {
		let dir = tempdir().expect("tempdir");
		fs::write(dir.path().join("pullhook.json"), "{}").expect("write config");

		let path = discover(dir.path()).expect("discover").expect("config");

		assert_eq!(path.file_name().and_then(|name| name.to_str()), Some("pullhook.json"));
	}

	#[test]
	fn errors_on_multiple_configs() {
		let dir = tempdir().expect("tempdir");
		fs::write(dir.path().join("pullhook.json"), "{}").expect("write config");
		fs::write(dir.path().join(".pullhook.toml"), "").expect("write config");

		let error = discover(dir.path()).expect_err("multiple configs fail");

		assert!(error.to_string().contains("multiple pullhook config files"));
	}

	#[test]
	fn rejects_yml_extension() {
		let error = ConfigFormat::from_path(Path::new("pullhook.yml")).expect_err("yml fails");

		assert!(error.to_string().contains("*.yml"));
	}

	#[test]
	fn discover_rejects_unsupported_config_names() {
		let dir = tempdir().expect("tempdir");
		fs::write(dir.path().join("pullhook.yml"), "rules: []").expect("write unsupported config");

		let error = discover(dir.path()).expect_err("unsupported config fails");

		assert!(error.to_string().contains("*.yml"));
	}

	#[test]
	fn load_accepts_schema_metadata() {
		let dir = tempdir().expect("tempdir");
		let path = dir.path().join("pullhook.json");
		fs::write(
			&path,
			r#"{
  "$schema": "https://pullhook.dev/schema.json",
  "rules": [
    {
      "name": "build",
      "changed": "src/**/*.rs",
      "run": "cargo test"
    }
  ]
}
"#,
		)
		.expect("write config");

		let config = load(&path).expect("load config");

		assert_eq!(config.entries.len(), 1);
	}

	#[test]
	fn load_rejects_unknown_top_level_fields() {
		let dir = tempdir().expect("tempdir");
		let path = dir.path().join("pullhook.json");
		fs::write(
			&path,
			r#"{
  "rules": [
    {
      "name": "build",
      "changed": "src/**/*.rs",
      "run": "cargo test"
    }
  ],
  "onSucces": "ship it"
}
"#,
		)
		.expect("write config");

		let error = load(&path).expect_err("unknown top-level field fails");

		assert!(error.to_string().contains("unknown field"));
		assert!(error.to_string().contains("onSucces"));
	}

	#[test]
	fn load_rejects_unknown_rule_fields() {
		let dir = tempdir().expect("tempdir");
		let path = dir.path().join("pullhook.json");
		fs::write(
			&path,
			r#"{
  "rules": [
    {
      "name": "build",
      "changed": "src/**/*.rs",
      "run": "cargo test",
      "excldue": "target/**"
    }
  ]
}
"#,
		)
		.expect("write config");

		let error = load(&path).expect_err("unknown rule field fails");

		assert!(error.to_string().contains("unknown field"));
		assert!(error.to_string().contains("excldue"));
	}

	#[test]
	fn validates_fail_text() {
		let fail_text = normalize_fail_text(
			Some("{red.bold {rule} failed}: {command} in {cwd} ({exitCode})".to_owned()),
			"rule",
			&mut Vec::new(),
		)
		.expect("fail text");

		let rendered = fail_text.render_plain(&FailTextContext {
			rule: "build",
			command: "cargo test",
			cwd: ".",
			exit_code: "1",
		});

		assert_eq!(rendered, "build failed: cargo test in . (1)");
	}

	#[test]
	fn invalid_fail_text_style_errors() {
		let mut errors = Vec::new();
		let _ = normalize_fail_text(Some("{sparkle nope}".to_owned()), "rule", &mut errors);

		assert!(errors.iter().any(|error| error.contains("unknown style `sparkle`")));
	}

	#[test]
	fn invalid_fail_text_unmatched_closing_brace_errors_before_later_block() {
		let mut errors = Vec::new();
		let _ = normalize_fail_text(Some("oops } then {rule}".to_owned()), "rule", &mut errors);

		assert!(errors.iter().any(|error| error.contains("unmatched `}`")));
	}

	#[test]
	fn matches_changed_and_excludes() {
		let changed = vec![Pattern::new("src/**/*.rs").expect("changed pattern")];
		let exclude = vec![Pattern::new("src/generated/**").expect("exclude pattern")];
		let files = vec![
			PathBuf::from("src/main.rs"),
			PathBuf::from("src/generated/code.rs"),
			PathBuf::from("README.md"),
		];

		let matched = match_rule_paths(&changed, &exclude, &files);

		assert_eq!(matched, vec![PathBuf::from("src/main.rs")]);
	}
}
