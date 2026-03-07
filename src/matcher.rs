//! Glob matching utilities.

use std::borrow::Cow;
use std::path::Path;

use globset::{GlobBuilder, GlobMatcher};

use crate::error::PullhookError;

/// Compiled pattern matcher.
#[derive(Debug)]
pub struct Matcher {
	matchers: Vec<GlobMatcher>,
}

impl Matcher {
	/// Returns true when any matcher accepts `path`.
	#[must_use]
	pub fn is_match(&self, path: &Path) -> bool {
		let normalized = normalize_path(path);
		self.matchers
			.iter()
			.any(|matcher| matcher.is_match(normalized.as_ref()))
	}
}

/// Compile a pattern with support for `+(a|b)` extglob expansion.
pub fn compile(pattern: &str) -> Result<Matcher, PullhookError> {
	let expanded = expand_plus_extglob(pattern)?;

	let mut matchers = Vec::with_capacity(expanded.len());
	for expanded_pattern in expanded {
		let glob = GlobBuilder::new(&expanded_pattern)
			.literal_separator(true)
			.backslash_escape(true)
			.build()
			.map_err(|error| PullhookError::Pattern {
				pattern: pattern.to_owned(),
				reason: error.to_string(),
			})?;

		matchers.push(glob.compile_matcher());
	}

	Ok(Matcher { matchers })
}

fn expand_plus_extglob(pattern: &str) -> Result<Vec<String>, PullhookError> {
	let Some((start, end)) = find_plus_group(pattern) else {
		return Ok(vec![pattern.to_owned()]);
	};

	let inner_start = start + 2;
	let inner = &pattern[inner_start..end];

	if has_unescaped_paren(inner) {
		return Err(PullhookError::Pattern {
			pattern: pattern.to_owned(),
			reason: "nested extglob is not supported".to_owned(),
		});
	}

	let options = split_unescaped(inner, '|');
	if options.is_empty() {
		return Err(PullhookError::Pattern {
			pattern: pattern.to_owned(),
			reason: "extglob cannot be empty".to_owned(),
		});
	}

	let prefix = &pattern[..start];
	let suffix = &pattern[end + 1..];

	let mut expanded = Vec::new();
	for option in options {
		if option.is_empty() {
			return Err(PullhookError::Pattern {
				pattern: pattern.to_owned(),
				reason: "extglob option cannot be empty".to_owned(),
			});
		}

		let candidate = format!("{prefix}{option}{suffix}");
		expanded.extend(expand_plus_extglob(&candidate)?);
	}

	Ok(expanded)
}

fn find_plus_group(pattern: &str) -> Option<(usize, usize)> {
	let bytes = pattern.as_bytes();
	let mut index = 0;

	while index + 1 < bytes.len() {
		if bytes[index] == b'+' && bytes[index + 1] == b'(' && !is_escaped(pattern, index) {
			let mut cursor = index + 2;
			while cursor < bytes.len() {
				if bytes[cursor] == b')' && !is_escaped(pattern, cursor) {
					return Some((index, cursor));
				}
				cursor += 1;
			}
			return None;
		}
		index += 1;
	}

	None
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
