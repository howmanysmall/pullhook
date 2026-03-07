//! Package manager detection for `--install`.

use std::path::Path;

use crate::error::PullhookError;

struct PackageManagerSpec {
	name: &'static str,
	lock_files: &'static [&'static str],
	config_files: &'static [&'static str],
	watched_files: &'static [&'static str],
}

const NPM_SPEC: PackageManagerSpec = PackageManagerSpec {
	name: "npm",
	lock_files: &["package-lock.json"],
	config_files: &["package.json"],
	watched_files: &["package.json", "package-lock.json"],
};

const YARN_SPEC: PackageManagerSpec = PackageManagerSpec {
	name: "yarn",
	lock_files: &["yarn.lock"],
	config_files: &[],
	watched_files: &["package.json", "yarn.lock"],
};

const PNPM_SPEC: PackageManagerSpec = PackageManagerSpec {
	name: "pnpm",
	lock_files: &["pnpm-lock.yaml"],
	config_files: &[],
	watched_files: &["package.json", "pnpm-lock.yaml"],
};

const BUN_SPEC: PackageManagerSpec = PackageManagerSpec {
	name: "bun",
	lock_files: &["bun.lock", "bun.lockb"],
	config_files: &[],
	watched_files: &["package.json", "bun.lock", "bun.lockb"],
};

const DENO_SPEC: PackageManagerSpec = PackageManagerSpec {
	name: "deno",
	lock_files: &["deno.lock"],
	config_files: &["deno.json", "deno.jsonc"],
	watched_files: &["package.json", "deno.json", "deno.jsonc", "deno.lock"],
};

const VLT_SPEC: PackageManagerSpec = PackageManagerSpec {
	name: "vlt",
	lock_files: &["vlt-lock.json"],
	config_files: &[],
	watched_files: &["package.json", "vlt-lock.json"],
};

// TODO: add wally support?

const LOCKFILE_DETECTION_ORDER: [PackageManager; 6] = [
	PackageManager::Bun,
	PackageManager::Npm,
	PackageManager::Yarn,
	PackageManager::Pnpm,
	PackageManager::Deno,
	PackageManager::Vlt,
];

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
	const fn spec(self) -> &'static PackageManagerSpec {
		match self {
			Self::Npm => &NPM_SPEC,
			Self::Yarn => &YARN_SPEC,
			Self::Pnpm => &PNPM_SPEC,
			Self::Bun => &BUN_SPEC,
			Self::Deno => &DENO_SPEC,
			Self::Vlt => &VLT_SPEC,
		}
	}

	/// Binary name.
	#[must_use]
	pub const fn name(self) -> &'static str {
		self.spec().name
	}

	/// Lock files that uniquely identify the package manager.
	#[must_use]
	pub const fn lock_files(self) -> &'static [&'static str] {
		self.spec().lock_files
	}

	/// Config files used when no lock file is present.
	#[must_use]
	pub const fn config_files(self) -> &'static [&'static str] {
		self.spec().config_files
	}

	/// Files that should trigger `--install`.
	#[must_use]
	pub const fn watched_files(self) -> &'static [&'static str] {
		self.spec().watched_files
	}

	/// Install command used by `--install`.
	#[must_use]
	pub fn install_command(self) -> String {
		format!("{} install", self.name())
	}

	/// Pattern used by `--install`.
	#[must_use]
	pub fn install_pattern(self) -> String {
		format!("+({})", self.watched_files().join("|"))
	}
}

/// Detect the package manager for `--install`.
pub fn detect_package_manager(repo_root: &Path) -> Result<PackageManager, PullhookError> {
	let detected_by_lock: Vec<_> = LOCKFILE_DETECTION_ORDER
		.into_iter()
		.filter(|package_manager| any_file_exists(repo_root, package_manager.lock_files()))
		.collect();

	if detected_by_lock.len() > 1 {
		return Err(PullhookError::AmbiguousPackageManagers {
			found: detected_by_lock.into_iter().map(PackageManager::name).collect(),
		});
	}

	if let Some(found) = detected_by_lock.first().copied() {
		return Ok(found);
	}

	if any_file_exists(repo_root, PackageManager::Deno.config_files()) {
		return Ok(PackageManager::Deno);
	}

	if any_file_exists(repo_root, PackageManager::Npm.config_files()) {
		return Ok(PackageManager::Npm);
	}

	Err(PullhookError::PackageManagerNotFound {
		root: repo_root.display().to_string(),
	})
}

fn file_exists(root: &Path, name: &str) -> bool {
	root.join(name).is_file()
}

fn any_file_exists(root: &Path, names: &[&str]) -> bool {
	names.iter().any(|name| file_exists(root, name))
}

#[cfg(test)]
mod tests {
	use std::fs;

	use tempfile::tempdir;

	use super::{PackageManager, detect_package_manager};

	#[test]
	fn install_pattern_matches_current_npm_contract() {
		assert_eq!(
			PackageManager::Npm.install_pattern(),
			"+(package.json|package-lock.json)"
		);
	}

	#[test]
	fn install_pattern_matches_current_yarn_contract() {
		assert_eq!(PackageManager::Yarn.install_pattern(), "+(package.json|yarn.lock)");
	}

	#[test]
	fn install_pattern_matches_current_pnpm_contract() {
		assert_eq!(PackageManager::Pnpm.install_pattern(), "+(package.json|pnpm-lock.yaml)");
	}

	#[test]
	fn install_pattern_matches_current_bun_contract() {
		assert_eq!(
			PackageManager::Bun.install_pattern(),
			"+(package.json|bun.lock|bun.lockb)"
		);
	}

	#[test]
	fn install_pattern_matches_current_deno_contract() {
		assert_eq!(
			PackageManager::Deno.install_pattern(),
			"+(package.json|deno.json|deno.jsonc|deno.lock)"
		);
	}

	#[test]
	fn install_pattern_matches_current_vlt_contract() {
		assert_eq!(PackageManager::Vlt.install_pattern(), "+(package.json|vlt-lock.json)");
	}

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
