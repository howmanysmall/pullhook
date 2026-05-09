//! Glob matching utilities.

use std::borrow::Cow;
use std::path::Path;

use globset::{GlobBuilder, GlobMatcher};

use crate::error::PullhookError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExtglobOperator {
	Plus,
	Star,
	Question,
	At,
	Bang,
}

impl ExtglobOperator {
	const fn from_prefix_byte(byte: u8) -> Option<Self> {
		match byte {
			b'+' => Some(Self::Plus),
			b'*' => Some(Self::Star),
			b'?' => Some(Self::Question),
			b'@' => Some(Self::At),
			b'!' => Some(Self::Bang),
			_ => None,
		}
	}

	const fn token(self) -> &'static str {
		match self {
			Self::Plus => "+(...)",
			Self::Star => "*(...)",
			Self::Question => "?(...)",
			Self::At => "@(...)",
			Self::Bang => "!(...)",
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExtglobGroup {
	operator: ExtglobOperator,
	start: usize,
	end: usize,
}

#[derive(Debug, Default)]
struct ExpandedPatterns {
	includes: Vec<String>,
	excludes: Vec<String>,
}

impl ExpandedPatterns {
	const fn include(patterns: Vec<String>) -> Self {
		Self {
			includes: patterns,
			excludes: Vec::new(),
		}
	}

	fn extend(&mut self, other: Self) {
		self.includes.extend(other.includes);
		self.excludes.extend(other.excludes);
	}
}

/// Compiled pattern matcher.
#[derive(Debug, Clone)]
pub struct Matcher {
	includes: Vec<GlobMatcher>,
	excludes: Vec<GlobMatcher>,
}

impl Matcher {
	/// Returns true when any matcher accepts `path`.
	#[must_use]
	pub fn is_match(&self, path: &Path) -> bool {
		let normalized = normalize_path(path);
		let path = normalized.as_ref();
		self.includes.iter().any(|matcher| matcher.is_match(path))
			&& !self.excludes.iter().any(|matcher| matcher.is_match(path))
	}
}

fn compile_patterns(patterns: Vec<String>, original: &str) -> Result<Vec<GlobMatcher>, PullhookError> {
	patterns
		.into_iter()
		.map(|expanded_pattern| {
			GlobBuilder::new(&expanded_pattern)
				.literal_separator(true)
				.backslash_escape(true)
				.build()
				.map(|glob| glob.compile_matcher())
				.map_err(|error| PullhookError::Pattern {
					pattern: original.to_owned(),
					reason: error.to_string(),
				})
		})
		.collect()
}

/// Compile a pattern with support for pullhook's extglob shim.
///
/// Pullhook expands extglob groups into finite `globset` patterns:
/// - `+(a|b)` and `@(a|b)` match one listed option.
/// - `?(a|b)` and `*(a|b)` match no option or one listed option.
/// - `!(a|b)` matches the surrounding wildcard shape, excluding listed options.
///
/// Nested groups are rejected so the expansion stays predictable.
pub fn compile(pattern: &str) -> Result<Matcher, PullhookError> {
	let expanded = expand_extglob_groups(pattern)?;
	let includes = compile_patterns(expanded.includes, pattern)?;
	let excludes = compile_patterns(expanded.excludes, pattern)?;

	Ok(Matcher { includes, excludes })
}

fn expand_extglob_groups(pattern: &str) -> Result<ExpandedPatterns, PullhookError> {
	let Some(group) = find_extglob_group(pattern)? else {
		return Ok(ExpandedPatterns::include(vec![pattern.to_owned()]));
	};

	let inner_start = group.start + 2;
	let inner = &pattern[inner_start..group.end];
	if inner.is_empty() {
		return Err(PullhookError::Pattern {
			pattern: pattern.to_owned(),
			reason: "extglob cannot be empty".to_owned(),
		});
	}

	if has_unescaped_paren(inner) {
		return Err(PullhookError::Pattern {
			pattern: pattern.to_owned(),
			reason: "nested extglob is not supported".to_owned(),
		});
	}

	let options = split_unescaped(inner, '|');
	let prefix = &pattern[..group.start];
	let suffix = &pattern[group.end + 1..];

	match group.operator {
		ExtglobOperator::Plus | ExtglobOperator::At => expand_required_extglob(pattern, prefix, suffix, &options),
		ExtglobOperator::Question | ExtglobOperator::Star => expand_optional_extglob(pattern, prefix, suffix, &options),
		ExtglobOperator::Bang => expand_negated_extglob(pattern, prefix, suffix, &options),
	}
}

fn expand_required_extglob(
	pattern: &str,
	prefix: &str,
	suffix: &str,
	options: &[&str],
) -> Result<ExpandedPatterns, PullhookError> {
	let mut expanded = ExpandedPatterns::default();
	for option in options {
		expanded.extend(expand_option(pattern, prefix, option, suffix)?);
	}
	Ok(expanded)
}

fn expand_optional_extglob(
	pattern: &str,
	prefix: &str,
	suffix: &str,
	options: &[&str],
) -> Result<ExpandedPatterns, PullhookError> {
	let mut expanded = expand_extglob_groups(&format!("{prefix}{suffix}"))?;
	for option in options {
		expanded.extend(expand_option(pattern, prefix, option, suffix)?);
	}
	Ok(expanded)
}

fn expand_negated_extglob(
	pattern: &str,
	prefix: &str,
	suffix: &str,
	options: &[&str],
) -> Result<ExpandedPatterns, PullhookError> {
	let mut expanded = expand_extglob_groups(&format!("{prefix}*{suffix}"))?;
	for option in options {
		let option_patterns = expand_option(pattern, prefix, option, suffix)?;
		expanded.excludes.extend(option_patterns.includes);
		expanded.excludes.extend(option_patterns.excludes);
	}
	Ok(expanded)
}

fn expand_option(pattern: &str, prefix: &str, option: &str, suffix: &str) -> Result<ExpandedPatterns, PullhookError> {
	if option.is_empty() {
		return Err(PullhookError::Pattern {
			pattern: pattern.to_owned(),
			reason: "extglob option cannot be empty".to_owned(),
		});
	}

	expand_extglob_groups(&format!("{prefix}{option}{suffix}"))
}

fn find_extglob_group(pattern: &str) -> Result<Option<ExtglobGroup>, PullhookError> {
	let bytes = pattern.as_bytes();
	let mut index = 0;

	while index + 1 < bytes.len() {
		if let Some(operator) = ExtglobOperator::from_prefix_byte(bytes[index]) {
			if bytes[index + 1] != b'(' || is_escaped(pattern, index) {
				index += 1;
				continue;
			}

			let mut cursor = index + 2;
			while cursor < bytes.len() {
				if bytes[cursor] == b')' && !is_escaped(pattern, cursor) {
					return Ok(Some(ExtglobGroup {
						operator,
						start: index,
						end: cursor,
					}));
				}
				cursor += 1;
			}

			return Err(PullhookError::Pattern {
				pattern: pattern.to_owned(),
				reason: format!("extglob operator `{}` is missing a closing `)`", operator.token()),
			});
		}

		index += 1;
	}

	Ok(None)
}

fn has_unescaped_paren(value: &str) -> bool {
	value
		.char_indices()
		.any(|(index, ch)| (ch == '(' || ch == ')') && !is_escaped(value, index))
}

fn split_unescaped(value: &str, separator: char) -> Vec<&str> {
	let mut parts = Vec::new();
	let mut start = 0;

	for (index, ch) in value.char_indices() {
		if ch == separator && !is_escaped(value, index) {
			parts.push(&value[start..index]);
			start = index + ch.len_utf8();
		}
	}

	parts.push(&value[start..]);
	parts
}

fn is_escaped(value: &str, index: usize) -> bool {
	if index == 0 {
		return false;
	}

	let bytes = value.as_bytes();
	let mut backslashes = 0;
	let mut cursor = index;

	while cursor > 0 {
		cursor -= 1;
		if bytes[cursor] == b'\\' {
			backslashes += 1;
		} else {
			break;
		}
	}

	backslashes % 2 == 1
}

fn normalize_path(path: &Path) -> Cow<'_, str> {
	let normalized = path.to_string_lossy();
	if normalized.contains('\\') {
		return Cow::Owned(normalized.replace('\\', "/"));
	}

	normalized
}

#[cfg(test)]
mod tests {
	use super::compile;
	use std::borrow::Cow;
	use std::path::Path;

	use crate::matcher::normalize_path;

	#[test]
	fn matches_basic_glob() {
		let matcher = compile("**/*.rs").expect("pattern compiles");
		assert!(matcher.is_match(Path::new("src/main.rs")));
		assert!(!matcher.is_match(Path::new("Cargo.toml")));
	}

	#[test]
	fn matches_plus_extglob_options() {
		let matcher = compile("+(package.json|package-lock.json)").expect("pattern compiles");
		assert!(matcher.is_match(Path::new("package.json")));
		assert!(matcher.is_match(Path::new("package-lock.json")));
		assert!(!matcher.is_match(Path::new("yarn.lock")));
	}

	#[test]
	fn matches_plus_extglob_with_prefix() {
		let matcher = compile("packages/*/+(package.json|package-lock.json)").expect("pattern compiles");
		assert!(matcher.is_match(Path::new("packages/a/package.json")));
		assert!(matcher.is_match(Path::new("packages/a/package-lock.json")));
		assert!(!matcher.is_match(Path::new("packages/a/yarn.lock")));
	}

	#[test]
	fn matches_at_extglob_options() {
		let matcher = compile("@(package.json|package-lock.json)").expect("pattern compiles");
		assert!(matcher.is_match(Path::new("package.json")));
		assert!(matcher.is_match(Path::new("package-lock.json")));
		assert!(!matcher.is_match(Path::new("yarn.lock")));
	}

	#[test]
	fn matches_question_extglob_when_option_is_missing() {
		let matcher = compile("package?(.json)").expect("pattern compiles");
		assert!(matcher.is_match(Path::new("package")));
		assert!(matcher.is_match(Path::new("package.json")));
		assert!(!matcher.is_match(Path::new("package-lock.json")));
	}

	#[test]
	fn matches_star_extglob_when_option_is_missing() {
		let matcher = compile("package*(.json)").expect("pattern compiles");
		assert!(matcher.is_match(Path::new("package")));
		assert!(matcher.is_match(Path::new("package.json")));
		assert!(!matcher.is_match(Path::new("package-lock.json")));
	}

	#[test]
	fn matches_bang_extglob_by_excluding_options() {
		let matcher = compile("!(package.json|package-lock.json)").expect("pattern compiles");
		assert!(matcher.is_match(Path::new("yarn.lock")));
		assert!(!matcher.is_match(Path::new("package.json")));
		assert!(!matcher.is_match(Path::new("package-lock.json")));
	}

	#[test]
	fn escaped_plus_is_literal() {
		let matcher = compile(r"\+(a|b)").expect("pattern compiles");
		assert!(matcher.is_match(Path::new("+(a|b)")));
		assert!(!matcher.is_match(Path::new("a")));
	}

	#[test]
	fn nested_extglob_is_rejected() {
		let error = compile("+(a|+(b|c))").expect_err("nested extglob should fail");
		let message = error.to_string();
		assert!(message.contains("nested extglob"));
	}

	#[test]
	fn empty_question_extglob_is_rejected() {
		let error = compile("?()").expect_err("empty extglob should fail");
		let message = error.to_string();
		assert!(message.contains("extglob cannot be empty"));
	}

	#[test]
	fn unclosed_extglob_is_rejected() {
		let error = compile("+(a|b").expect_err("unclosed extglob should fail");
		let message = error.to_string();
		assert!(message.contains("missing a closing `)`"));
	}

	#[test]
	fn normalize_path_borrows_when_no_separator_rewrite_is_needed() {
		assert!(matches!(
			normalize_path(Path::new("src/main.rs")),
			Cow::Borrowed("src/main.rs")
		));
	}

	#[test]
	fn normalize_path_rewrites_windows_separators_when_present() {
		let normalized = normalize_path(Path::new(r"src\main.rs"));
		assert!(matches!(normalized, Cow::Owned(_)));
		assert_eq!(normalized, "src/main.rs");
	}
}
