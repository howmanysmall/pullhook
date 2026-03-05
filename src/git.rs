//! Git operations used by pullhook.

use std::path::PathBuf;
use std::process::Command;

use tracing::debug;

use crate::error::PullhookError;

/// Resolve the repository root with `git rev-parse --show-toplevel`.
pub fn repo_root(debug_enabled: bool) -> Result<PathBuf, PullhookError> {
	let output = run_git(["rev-parse", "--show-toplevel"], debug_enabled)?;
	Ok(PathBuf::from(output.trim()))
}

/// Resolve base and return changed files in one pass.
pub fn resolve_base_and_changed_files(
	explicit: Option<&str>,
	debug_enabled: bool,
) -> Result<(String, Vec<PathBuf>), PullhookError> {
	if let Some(base) = explicit {
		if !revision_exists(base, debug_enabled)? {
			return Err(PullhookError::Message(format!(
				"base revision `{base}` could not be resolved"
			)));
		}

		let files = changed_files(base, debug_enabled)?;
		if debug_enabled {
			debug!(%base, "using explicit base revision");
		}
		return Ok((base.to_owned(), files));
	}

	for candidate in ["HEAD@{1}", "ORIG_HEAD", "HEAD~1"] {
		match changed_files(candidate, debug_enabled) {
			Ok(files) => {
				if debug_enabled {
					debug!(base = candidate, "selected diff base");
				}
				return Ok((candidate.to_owned(), files));
			}
			Err(PullhookError::GitCommand { .. }) => {}
			Err(error) => return Err(error),
		}
	}

	Err(PullhookError::Message(
		"unable to resolve diff base; use --base <rev> to override".to_owned(),
	))
}

/// Read changed files using `git diff --name-only <base> HEAD`.
pub fn changed_files(base: &str, debug_enabled: bool) -> Result<Vec<PathBuf>, PullhookError> {
	let output = run_git(["diff", "--name-only", base, "HEAD"], debug_enabled)?;

	Ok(output
		.lines()
		.map(str::trim)
		.filter(|line| !line.is_empty())
		.map(PathBuf::from)
		.collect())
}

fn revision_exists(rev: &str, debug_enabled: bool) -> Result<bool, PullhookError> {
	let args = ["rev-parse", "--verify", "--quiet", rev];
	let status = run_git_status(args, debug_enabled)?;
	Ok(status)
}

fn run_git<const N: usize>(args: [&str; N], debug_enabled: bool) -> Result<String, PullhookError> {
	if debug_enabled {
		debug!(command = format!("git {}", args.join(" ")), "running git command");
	}

	let output = Command::new("git")
		.args(args)
		.output()
		.map_err(|source| PullhookError::GitIo {
			command: format!("git {}", args.join(" ")),
			source,
		})?;

	if output.status.success() {
		let stdout = String::from_utf8_lossy(&output.stdout).to_string();
		return Ok(stdout);
	}

	Err(PullhookError::GitCommand {
		command: format!("git {}", args.join(" ")),
		stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
	})
}

fn run_git_status<const N: usize>(args: [&str; N], debug_enabled: bool) -> Result<bool, PullhookError> {
	if debug_enabled {
		debug!(command = format!("git {}", args.join(" ")), "running git probe command");
	}

	let output = Command::new("git")
		.args(args)
		.output()
		.map_err(|source| PullhookError::GitIo {
			command: format!("git {}", args.join(" ")),
			source,
		})?;

	Ok(output.status.success())
}
