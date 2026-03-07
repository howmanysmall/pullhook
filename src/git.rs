//! Git operations used by pullhook.

use std::path::{Path, PathBuf};

use gix::bstr::BStr;
use tracing::debug;

use crate::error::PullhookError;

/// Native git repository handle.
#[derive(Debug, Clone)]
pub struct GitRepo {
	repo: gix::ThreadSafeRepository,
	root: PathBuf,
}

#[derive(Debug)]
struct ResolvedBase<'repo> {
	name: String,
	tree: gix::Tree<'repo>,
}

impl GitRepo {
	/// Discover a repository from the current working directory while honoring git env overrides.
	pub fn discover(current_dir: &Path, debug_enabled: bool) -> Result<Self, PullhookError> {
		let repo = gix::ThreadSafeRepository::discover_with_environment_overrides(current_dir).map_err(|source| {
			PullhookError::GitOpen {
				path: current_dir.display().to_string(),
				source: Box::new(source),
			}
		})?;
		let local = repo.to_thread_local();
		let root = local
			.workdir()
			.map(Path::to_path_buf)
			.ok_or_else(|| PullhookError::Message("bare repositories are not supported".to_owned()))?;

		if debug_enabled {
			if root == current_dir {
				debug!(cwd = %root.display(), "using current working directory as repository root");
			} else {
				debug!(
					cwd = %current_dir.display(),
					repo_root = %root.display(),
					"resolved repository root from current working directory"
				);
			}
		}

		Ok(Self { repo, root })
	}

	/// Repository root containing checked-out files.
	#[must_use]
	pub fn root(&self) -> &Path {
		&self.root
	}

	/// Resolve base and return all changed files.
	pub fn resolve_base_and_changed_files(
		&self,
		explicit: Option<&str>,
		debug_enabled: bool,
	) -> Result<(String, Vec<PathBuf>), PullhookError> {
		let repo = self.repo.to_thread_local();
		let base = resolve_base(&repo, explicit, debug_enabled)?;
		let changes = diff_changes(&repo, &base)?;
		let files = changes
			.into_iter()
			.map(|change| relative_path_from_bstr(change.location()))
			.collect();
		Ok((base.name, files))
	}

	/// Resolve base and collect install matches without materializing all changed paths.
	pub fn resolve_install_matches<F>(
		&self,
		explicit: Option<&str>,
		mut is_match: F,
		debug_enabled: bool,
	) -> Result<(String, usize, Vec<PathBuf>), PullhookError>
	where
		F: FnMut(&Path) -> bool,
	{
		let repo = self.repo.to_thread_local();
		let base = resolve_base(&repo, explicit, debug_enabled)?;
		let changes = diff_changes(&repo, &base)?;
		let mut changed_count = 0usize;
		let mut matched_files = Vec::new();

		for change in changes {
			let path = relative_path_from_bstr(change.location());
			changed_count = changed_count.saturating_add(1);

			if debug_enabled {
				debug!(changed = %path.display(), "changed file");
			}

			if is_match(&path) {
				matched_files.push(path);
			}
		}

		Ok((base.name, changed_count, matched_files))
	}
}

fn resolve_base<'repo>(
	repo: &'repo gix::Repository,
	explicit: Option<&str>,
	debug_enabled: bool,
) -> Result<ResolvedBase<'repo>, PullhookError> {
	if let Some(base) = explicit {
		let tree = try_resolve_tree(repo, base)?
			.ok_or_else(|| PullhookError::Message(format!("base revision `{base}` could not be resolved")))?;

		if debug_enabled {
			debug!(%base, "using explicit base revision");
		}
		return Ok(ResolvedBase {
			name: base.to_owned(),
			tree,
		});
	}

	for candidate in ["HEAD@{1}", "ORIG_HEAD", "HEAD~1"] {
		let Some(tree) = try_resolve_tree(repo, candidate)? else {
			continue;
		};

		if debug_enabled {
			debug!(base = candidate, "selected diff base");
		}
		return Ok(ResolvedBase {
			name: candidate.to_owned(),
			tree,
		});
	}

	Err(PullhookError::Message(
		"unable to resolve diff base; use --base <rev> to override".to_owned(),
	))
}

fn try_resolve_tree<'repo>(
	repo: &'repo gix::Repository,
	revision: &str,
) -> Result<Option<gix::Tree<'repo>>, PullhookError> {
	let Ok(spec) = repo.rev_parse(revision) else {
		return Ok(None);
	};
	let id = spec
		.single()
		.ok_or_else(|| PullhookError::Message(format!("base revision `{revision}` could not be resolved")))?;
	let object = id.object().map_err(|source| PullhookError::GitRevision {
		revision: revision.to_owned(),
		source: Box::new(source),
	})?;

	object
		.peel_to_tree()
		.map(Some)
		.map_err(|source| PullhookError::GitRevision {
			revision: revision.to_owned(),
			source: Box::new(source),
		})
}

fn diff_changes(
	repo: &gix::Repository,
	base: &ResolvedBase<'_>,
) -> Result<Vec<gix::object::tree::diff::ChangeDetached>, PullhookError> {
	let head_tree = repo.head_tree().map_err(|source| PullhookError::GitDiff {
		base: base.name.clone(),
		source: Box::new(source),
	})?;

	repo.diff_tree_to_tree(Some(&base.tree), Some(&head_tree), None)
		.map(|changes| {
			changes
				.into_iter()
				.filter(|change| !change.entry_mode().is_tree())
				.collect()
		})
		.map_err(|source| PullhookError::GitDiff {
			base: base.name.clone(),
			source: Box::new(source),
		})
}

fn relative_path_from_bstr(path: &BStr) -> PathBuf {
	PathBuf::from(String::from_utf8_lossy(path.as_ref()).into_owned())
}

#[cfg(test)]
mod tests {
	use std::fs;
	use std::path::{Path, PathBuf};
	use std::process::Command as ProcessCommand;

	use tempfile::TempDir;

	use crate::error::PullhookError;
	use crate::matcher;

	use super::GitRepo;

	#[test]
	fn discovers_repo_root_from_subdirectory() {
		let temp = setup_repo_with_merge();
		let repo_root = temp.path();
		let subdirectory = repo_root.join("packages/a");

		let repo = GitRepo::discover(&subdirectory, false).expect("discover repo");

		assert_eq!(repo.root(), repo_root);
	}

	#[test]
	fn resolves_head_reflog_base_first() {
		let temp = setup_repo_with_merge();
		let repo = GitRepo::discover(temp.path(), false).expect("discover repo");

		let (base, changed_files) = repo
			.resolve_base_and_changed_files(None, false)
			.expect("resolve changed files");

		assert_eq!(base, "HEAD@{1}");
		assert_eq!(
			changed_files,
			vec![
				PathBuf::from("packages/a/package-lock.json"),
				PathBuf::from("packages/b/package-lock.json"),
			]
		);
	}

	#[test]
	fn falls_back_to_orig_head_when_head_reflog_is_unavailable() {
		let temp = setup_repo_with_merge();
		let repo_root = temp.path();
		clear_head_reflog(repo_root);
		let repo = GitRepo::discover(repo_root, false).expect("discover repo");

		let (base, changed_files) = repo
			.resolve_base_and_changed_files(None, false)
			.expect("resolve changed files");

		assert_eq!(base, "ORIG_HEAD");
		assert_eq!(changed_files.len(), 2);
	}

	#[test]
	fn falls_back_to_head_parent_when_reflog_and_orig_head_are_unavailable() {
		let temp = setup_repo_with_merge();
		let repo_root = temp.path();
		clear_head_reflog(repo_root);
		let _ = fs::remove_file(repo_root.join(".git/ORIG_HEAD"));
		let repo = GitRepo::discover(repo_root, false).expect("discover repo");

		let (base, changed_files) = repo
			.resolve_base_and_changed_files(None, false)
			.expect("resolve changed files");

		assert_eq!(base, "HEAD~1");
		assert_eq!(changed_files.len(), 2);
	}

	#[test]
	fn invalid_explicit_base_returns_user_message() {
		let temp = setup_repo_with_merge();
		let repo = GitRepo::discover(temp.path(), false).expect("discover repo");

		let error = repo
			.resolve_base_and_changed_files(Some("definitely-not-a-ref"), false)
			.expect_err("invalid base should fail");

		assert!(matches!(
			error,
			PullhookError::Message(message)
			if message == "base revision `definitely-not-a-ref` could not be resolved"
		));
	}

	#[test]
	fn install_fast_path_uses_full_relative_path_matching() {
		let temp = setup_repo_with_nested_manifest_change();
		let repo = GitRepo::discover(temp.path(), false).expect("discover repo");
		let matcher = matcher::compile("+(package.json|package-lock.json)").expect("compile matcher");

		let (base, changed_count, matched_files) = repo
			.resolve_install_matches(None, |path| matcher.is_match(path), false)
			.expect("resolve install matches");

		assert_eq!(base, "HEAD@{1}");
		assert_eq!(changed_count, 1);
		assert!(matched_files.is_empty());
	}

	#[test]
	fn install_fast_path_matches_repo_root_manifest_changes() {
		let temp = setup_repo_with_root_manifest_change();
		let repo = GitRepo::discover(temp.path(), false).expect("discover repo");
		let matcher = matcher::compile("+(package.json|package-lock.json)").expect("compile matcher");

		let (base, changed_count, matched_files) = repo
			.resolve_install_matches(None, |path| matcher.is_match(path), false)
			.expect("resolve install matches");

		assert_eq!(base, "HEAD@{1}");
		assert_eq!(changed_count, 1);
		assert_eq!(matched_files, vec![PathBuf::from("package-lock.json")]);
	}

	fn clear_head_reflog(repo_root: &Path) {
		let branch = current_branch(repo_root);
		let _ = fs::remove_file(repo_root.join(".git/logs/HEAD"));
		let _ = fs::remove_file(repo_root.join(".git/logs/refs/heads").join(branch));
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

	fn setup_repo_with_root_manifest_change() -> TempDir {
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
		let path = repo_root.join(relative_path);
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
}
