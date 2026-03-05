//! Package manager detection for `--install`.

use std::path::Path;

use crate::error::PullhookError;

/// Supported package managers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
	/// npm
	Npm,
	/// yarn
	Yarn,
	/// pnpm
	Pnpm,
	/// bun
	Bun,
	/// deno
	Deno,
	/// vlt
	Vlt,
}

impl PackageManager {
	/// Binary name.
	#[must_use]
	pub const fn name(self) -> &'static str {
		match self {
			Self::Npm => "npm",
			Self::Yarn => "yarn",
			Self::Pnpm => "pnpm",
			Self::Bun => "bun",
			Self::Deno => "deno",
			Self::Vlt => "vlt",
		}
	}

	/// Install command used by `--install`.
	#[must_use]
	pub fn install_command(self) -> String {
		format!("{} install", self.name())
	}

	/// Pattern used by `--install`.
	#[must_use]
	pub const fn install_pattern(self) -> &'static str {
		match self {
			Self::Npm => "+(package.json|package-lock.json)",
			Self::Yarn => "+(package.json|yarn.lock)",
			Self::Pnpm => "+(package.json|pnpm-lock.yaml)",
			Self::Bun => "+(package.json|bun.lock|bun.lockb)",
			Self::Deno => "+(package.json|deno.json|deno.jsonc|deno.lock)",
			Self::Vlt => "+(package.json|vlt-lock.json)",
		}
	}
}

/// Detect the package manager for `--install`.
pub fn detect_package_manager(repo_root: &Path) -> Result<PackageManager, PullhookError> {
	let mut detected_by_lock = Vec::new();

	if file_exists(repo_root, "bun.lock") || file_exists(repo_root, "bun.lockb") {
		detected_by_lock.push(PackageManager::Bun);
	}
	if file_exists(repo_root, "package-lock.json") {
		detected_by_lock.push(PackageManager::Npm);
	}
	if file_exists(repo_root, "yarn.lock") {
		detected_by_lock.push(PackageManager::Yarn);
	}
	if file_exists(repo_root, "pnpm-lock.yaml") {
		detected_by_lock.push(PackageManager::Pnpm);
	}
	if file_exists(repo_root, "deno.lock") {
		detected_by_lock.push(PackageManager::Deno);
	}
	if file_exists(repo_root, "vlt-lock.json") {
		detected_by_lock.push(PackageManager::Vlt);
	}

	if detected_by_lock.len() > 1 {
		return Err(PullhookError::AmbiguousPackageManagers {
			found: detected_by_lock.into_iter().map(PackageManager::name).collect(),
		});
	}

	if let Some(found) = detected_by_lock.first().copied() {
		return Ok(found);
	}

	if file_exists(repo_root, "deno.json") || file_exists(repo_root, "deno.jsonc") {
		return Ok(PackageManager::Deno);
	}

	if file_exists(repo_root, "package.json") {
		return Ok(PackageManager::Npm);
	}

	Err(PullhookError::PackageManagerNotFound {
		root: repo_root.display().to_string(),
	})
}

fn file_exists(root: &Path, name: &str) -> bool {
	root.join(name).is_file()
}

#[cfg(test)]
mod tests {
	use std::fs;

	use tempfile::tempdir;

	use super::{PackageManager, detect_package_manager};

	#[test]
	fn detects_npm_from_lock_file() {
		let dir = tempdir().expect("tempdir");
		fs::write(dir.path().join("package-lock.json"), "{}").expect("write lock file");
		assert_eq!(detect_package_manager(dir.path()).expect("detect"), PackageManager::Npm);
	}

	#[test]
	fn detects_yarn_from_lock_file() {
		let dir = tempdir().expect("tempdir");
		fs::write(dir.path().join("yarn.lock"), "").expect("write lock file");
		assert_eq!(
			detect_package_manager(dir.path()).expect("detect"),
			PackageManager::Yarn
		);
	}

	#[test]
	fn detects_pnpm_from_lock_file() {
		let dir = tempdir().expect("tempdir");
		fs::write(dir.path().join("pnpm-lock.yaml"), "").expect("write lock file");
		assert_eq!(
			detect_package_manager(dir.path()).expect("detect"),
			PackageManager::Pnpm
		);
	}

	#[test]
	fn detects_bun_from_lockb_file() {
		let dir = tempdir().expect("tempdir");
		fs::write(dir.path().join("bun.lockb"), "").expect("write lock file");
		assert_eq!(detect_package_manager(dir.path()).expect("detect"), PackageManager::Bun);
	}

	#[test]
	fn detects_deno_from_config_file() {
		let dir = tempdir().expect("tempdir");
		fs::write(dir.path().join("deno.json"), "{}").expect("write deno config");
		assert_eq!(
			detect_package_manager(dir.path()).expect("detect"),
			PackageManager::Deno
		);
	}

	#[test]
	fn detects_vlt_from_lock_file() {
		let dir = tempdir().expect("tempdir");
		fs::write(dir.path().join("vlt-lock.json"), "{}").expect("write lock file");
		assert_eq!(detect_package_manager(dir.path()).expect("detect"), PackageManager::Vlt);
	}

	#[test]
	fn falls_back_to_npm_with_package_json_only() {
		let dir = tempdir().expect("tempdir");
		fs::write(dir.path().join("package.json"), "{}").expect("write package json");
		assert_eq!(detect_package_manager(dir.path()).expect("detect"), PackageManager::Npm);
	}

	#[test]
	fn errors_on_ambiguous_lock_files() {
		let dir = tempdir().expect("tempdir");
		fs::write(dir.path().join("package-lock.json"), "{}").expect("write npm lock file");
		fs::write(dir.path().join("yarn.lock"), "").expect("write yarn lock file");

		let error = detect_package_manager(dir.path()).expect_err("must be ambiguous");
		let message = error.to_string();
		assert!(message.contains("multiple package managers"));
	}

	#[test]
	fn errors_when_no_files_are_present() {
		let dir = tempdir().expect("tempdir");
		let error = detect_package_manager(dir.path()).expect_err("must fail");
		let message = error.to_string();
		assert!(message.contains("no supported package manager files"));
	}
}
