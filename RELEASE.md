# Release Process

This project uses `cargo-release` to automate version updates, git tagging, and release commits. Releases are published
via `cargo-dist` to GitHub Releases, Homebrew, npm, and shell/powershell installers.

## Quick Release (recommended)

The easiest way to release is the bundled script:

```bash
./scripts/release.sh patch
./scripts/release.sh minor
./scripts/release.sh major
./scripts/release.sh 2.0.0
```

The script handles **everything**:

- ✅ Checks you're on `main`
- ✅ Checks the working tree is clean
- ✅ Asks for confirmation with current → target version
- ✅ Runs all quality checks (`./scripts/pre-commit.sh`)
- ✅ Bumps version, commits, tags, and pushes
- ✅ Prints a link to monitor CI artifact builds

## Prerequisites

Install `cargo-release`:

```bash
cargo install cargo-release
```

## Manual Release

If you prefer to run `cargo-release` directly:

```bash
# Release with specific version
cargo release 0.2.0 --execute

# Or use semver keywords (major, minor, patch, rc, beta, alpha)
cargo release minor --execute
cargo release patch --execute
```

## What Happens

Whether you use the script or run `cargo-release` directly, the release process automatically:

1. ✅ Runs `./scripts/pre-commit.sh` (all quality checks)
2. ✅ Updates version in `Cargo.toml`
3. ✅ Verifies the build
4. ✅ Commits changes with message: `chore: release {{version}}`
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

The generated [release workflow](./.github/workflows/release.yml) should be kept fully cargo-dist-managed.
Do not hand-edit `release.yml`; regenerate it through cargo-dist instead.

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
