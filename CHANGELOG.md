# Changelog

All notable changes to this project are documented here.

Following [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2025-01-XX

Initial release of pullhook.

### Added

- Core CLI functionality for running commands when files change after `git pull`
- Native Git operations using `gix` library (no Git CLI dependency)
- Glob pattern matching with bash extglob support (`+(a|b)`, `*(a|b)`, `?(a|b)`, `@(a|b)`, `!(a|b)`)
- Parallel command execution with bounded concurrency via `rayon` (default: min(CPUs, 8))
- Shell word parsing for safe command execution
- Package manager detection (npm, yarn, pnpm, bun) via lock files and config files
- Diff base resolution with fallback chain:
  - Explicit `--base` flag
  - `HEAD@{1}` (reflog)
  - `ORIG_HEAD` (merge/pull)
  - `HEAD~1` (fallback for shallow clones)
- Tracing and logging with `tracing-subscriber` and environment filter
- Completion subcommand for generating shell completion scripts (bash, zsh, fish, elvish, powershell)
- Styled terminal output with compact layout and automatic TTY detection
- Automated multi-platform releases via `cargo-dist`:
  - macOS (aarch64, x86_64)
  - Linux (aarch64, x86_64)
  - Windows (aarch64, x86_64)
  - Homebrew tap
  - npm package
  - Shell and PowerShell installers
- MCP (Model Context Protocol) integration with `create-pull-request` command

### Changed

- Replaced Git CLI calls with native `gix` implementation for better performance and reliability
- Consolidated core logic and internal types for cleaner architecture
- Optimized diff resolution and task execution flow
- Unified debug and non-debug output paths for consistent behavior
- Simplified distribution and release workflow configuration

### Infrastructure

- Rust toolchain 1.93.1 with edition 2024
- CI/CD pipeline with GitHub Actions including:
  - Formatting checks (`cargo fmt`)
  - Linting (`cargo clippy` with all, pedantic, and nursery lints)
  - Testing (`cargo nextest`)
  - Security audits (`cargo audit`)
  - Dependency policy enforcement (`cargo deny`)
  - Dead code detection (`cargo shear`)
  - Secret scanning (`gitleaks`)
- Build caching with `sccache`
- Git hooks via `lefthook` (pre-commit and pre-push)
- Dependabot configuration for automated dependency updates (Cargo, GitHub Actions, npm)
- Comprehensive development tooling:
  - Biome for JSON/JSONC formatting
  - Tombi for TOML formatting
  - rumdl for Markdown formatting
  - lint-staged for pre-commit checks

### Security

- Secret scanning on commits and pull requests via `gitleaks`
- Dependency vulnerability monitoring via `cargo audit`
- Dependency license and policy enforcement via `cargo deny`
- Unsafe code forbidden via Rust lint configuration

[Unreleased]: https://github.com/howmanysmall/pullhook/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/howmanysmall/pullhook/releases/tag/v0.1.0
