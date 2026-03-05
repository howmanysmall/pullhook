# pullhook

[![CI](https://github.com/howmanysmall/pullhook/actions/workflows/ci.yaml/badge.svg)](https://github.com/howmanysmall/pullhook/actions/workflows/ci.yaml)
[![Release](https://github.com/howmanysmall/pullhook/actions/workflows/release.yml/badge.svg)](https://github.com/howmanysmall/pullhook/actions/workflows/release.yml)

Run commands when files change after git pull.

## Installation

### From Source

```bash
cargo install --path .
```

### Using cargo-binstall

```bash
cargo binstall pullhook
```

### Using Homebrew (macOS/Linux)

```bash
brew install howmanysmall/pullhook/pullhook
```

## Development Setup

### Prerequisites

- Rust 1.93.1 (via `rust-toolchain.toml`)
- [Bun](https://bun.sh) (for JS tooling)
- [Rokit](https://github.com/rojo-rbx/rokit) (for gitleaks/lefthook)

### Initial Setup

1. Clone the repository:

   ```bash
   git clone https://github.com/howmanysmall/pullhook.git
   cd pullhook
   ```

2. Install Rokit tools:

   ```bash
   rokit install
   ```

3. Install Node dependencies:

   ```bash
   bun install
   ```

4. Install Git hooks:

   ```bash
   lefthook install
   ```

5. Install Rust development tools:

   ```bash
   cargo binstall cargo-nextest cargo-audit cargo-deny cargo-shear cargo-bloat cargo-insta cargo-zigbuild
   ```

### Local Development Workflow

#### Running tests

```bash
cargo test          # standard runner
cargo nextest run   # faster, better output
```

#### Code quality checks

Run these before pushing (or use the hooks):

```bash
cargo fmt --all --check     # format check
cargo clippy --all-targets -- -D warnings  # linting
cargo audit                 # security audit
cargo deny check             # dependency policy
cargo shear --check         # dead code detection
```

#### Pre-commit hooks

Lefthook runs these automatically before each commit:

- JavaScript/JSON/TOML formatting (Biome, tombi)
- Markdown linting (rumdl)
- Secret scanning (gitleaks)
- Rust formatting check
- Clippy linting
- Security audit
- Dependency policy check
- Dead code detection

#### Pre-push hooks

`cargo nextest run` runs automatically before pushing.

### CI/CD

#### CI workflow (`.github/workflows/ci.yaml`)

Required checks on Ubuntu (PRs fail if these don't pass):

- Format verification
- Clippy linting with `-D warnings`
- Test suite via cargo-nextest
- Security audit
- Dependency policy
- Dead code detection
- Secret scanning
- Documentation checks

Informational checks (won't block):

- Binary size analysis (cargo-bloat)
- Cross-compilation smoke test (cargo-zigbuild)

#### Release workflow (`.github/workflows/release.yml`)

Triggered on version tags (`v*.*.*`). Builds and publishes multi-platform binaries to GitHub Releases, Homebrew, and npm (@pobammer/pullhook).

### Tooling

| Tool | Purpose | Version |
|------|---------|---------|
| `cargo-nextest` | Fast test runner | 0.9.129 |
| `cargo-audit` | Security vulnerability scanner | 0.22.1 |
| `cargo-deny` | Dependency policy enforcement | 0.19.0 |
| `cargo-shear` | Unused dependency detection | 1.9.1 |
| `cargo-bloat` | Binary size analysis | 0.12.1 |
| `cargo-insta` | Snapshot testing | 1.46.3 |
| `cargo-zigbuild` | Zig-based cross-compilation | 0.22.1 |
| `lefthook` | Git hooks manager | 2.1.2 |
| `gitleaks` | Secret scanner | 8.30.0 |

### Configuration Files

| File | Purpose |
|------|---------|
| `rust-toolchain.toml` | Pin Rust version to 1.93.1 |
| `.cargo/config.toml` | Build optimizations (mold linker, LTO settings) |
| `deny.toml` | License/advisory/source policy for dependencies |
| `lefthook.yaml` | Git hook configuration |
| `Cargo.toml` | Rust lint rules (`clippy::all`, `clippy::pedantic`, `clippy::nursery`) |

## Contributing

1. Make sure pre-commit checks pass locally
2. Run `cargo nextest run` before pushing
3. Open a PR; CI validates all checks
4. Keep test coverage for new features

## License

MIT

## Troubleshooting

### Gitleaks False Positives

If gitleaks flags non-sensitive data, add an allowlist entry to `.gitleaks.toml` (create if missing):

```toml
[[allowlist]]
description = "Explanation of why this is safe"
regexes = ['^pattern-to-allow$']
```

### Cargo Deny License Issues

If a new dependency has an unexpected license:

1. Verify the license is acceptable
2. Add to `allow` list in `deny.toml` under `[licenses]`

### Dependency Version Conflicts

Run `cargo tree -d` to identify duplicate versions, then use `[patch]` sections in `Cargo.toml` if needed.
