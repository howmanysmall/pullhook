# Release Process

This project uses `cargo-release` to automate version updates, git tagging, and release commits. Releases are published via `cargo-dist` to GitHub Releases, Homebrew, npm, and shell/powershell installers.

## Prerequisites

Install `cargo-release`:

```bash
cargo install cargo-release
```

## Release Workflow

### 1. Update CHANGELOG.md

Manually update `CHANGELOG.md` to document the changes for the upcoming release. Follow [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) format.

Example:

```diff
## [Unreleased]

### Added
- (list of features)

## [0.2.0] - [2026-03-05]

### Added
- Add feature description here

- (commit, close, tag, push in one command)
```

### 2. Run the Release

From the `main` branch:

```bash
# Release with specific version
cargo release 0.2.0

# Or use semver keywords (major, minor, patch, rc, beta, alpha)
cargo release minor
cargo release patch
```

### 3. What Happens

The release process automatically:

1. ✅ Runs `./scripts/pre-commit.sh` (all quality checks)
2. ✅ Updates version in `Cargo.toml`
3. ✅ Verifies the build
4. ✅ Commits changes with message: `chore: Release pullhook version X.Y.Z`
5. ✅ Creates git tag `vX.Y.Z`
6. ✅ Pushes commit and tag to `origin`
7. 🚀 Triggers `cargo-dist` CI workflow to build and publish artifacts

## Artifacts Published

After CI completes, the following are automatically published:

- **GitHub Release** with binaries, archives, and checksums
- **Homebrew formula** to `howmanysmall/pullhook`
- **npm package** to `@pobammer/pullhook`
- **Shell installer**
- **PowerShell installer**
- **Updates** via built-in updater

## Configuration

Release behavior is configured in `[workspace.metadata.release]` in `Cargo.toml`:

- `publish = false` - Skips crates.io publishing (dist handles distribution)
- `allow-branch = ["main"]` - Restricts releases to main branch
- `verify = true` - Builds project before committing release
- `sign-commit = false` - No GPG signing
- `sign-tag = false` - No GPG signing

The generated [release workflow](./.github/workflows/release.yml) also sets `CARGO_BUILD_RUSTC_WRAPPER=""`.
Keep that override if the workflow is regenerated, because release runners do not install `sccache` but the repo-level cargo config enables it by default.

## Dry Run

To preview what a release will do without making changes:

```bash
cargo release patch
```

This will print the planned actions without executing them.

## Manual Release Steps

If `cargo-release` fails or you prefer manual control:

```bash
# 1. Update version in Cargo.toml
# 2. Update CHANGELOG.md
# 3. Commit changes
git commit -am "chore: Release pullhook version X.Y.Z"

# 4. Create tag
git tag vX.Y.Z

# 5. Push (atomic push ensures tag and commit arrive together)
git push --atomic origin main vX.Y.Z
```

## Troubleshooting

### Release fails on checks

If `./scripts/pre-commit.sh` fails, fix the issues and re-run the release.

### Already published version

If you see "already exists on crates.io", remember this project doesn't publish to crates.io (`publish = false`).

### Wrong branch

Releases must be run from `main`. Switch to main and try again.

## See Also

- [cargo-release documentation](https://github.com/crate-ci/cargo-release)
- [cargo-dist documentation](https://axodotdev.github.io/cargo-dist/)
