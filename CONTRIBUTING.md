# Contributing To `pullhook`

Thank you for your interest in contributing to **pullhook**! This guide will help you get started.

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) 1.93.1 (managed via `rust-toolchain.toml`)
- [mise](https://mise.jdx.dev/) for tool version management
- [Node.js](https://nodejs.org/) 25+ (for commit linting and JS tooling)

### Setup

1. Clone the repository:

   ```bash
   git clone https://github.com/howmanysmall/pullhook.git
   cd pullhook
   ```

2. Install dependencies via mise:

   ```bash
   mise install
   ```

3. Install JS tooling:

   ```bash
   aube install
   ```

## Development Commands

### Building

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo install --path .   # Install locally
```

### Testing

```bash
cargo nextest run                    # Run all tests
cargo nextest run -E 'test(name)'    # Run specific test
```

### Linting & Quality Checks

Run all checks before pushing:

```bash
bash ./scripts/pre-commit.sh
```

Or run individually:

```bash
cargo fmt --all --check              # Format check
cargo clippy --all-targets -- -D warnings  # Lint
cargo audit                          # Security audit
cargo deny check                     # License/advisory check
cargo shear                          # Unused dependency check
```

## Commit Convention

This project uses **Conventional Commits** enforced by `commitlint`. All commit
messages must follow this format:

```text
<type>(<scope>): <subject>

<body>

<footer>
```

### Allowed Types

| Type       | Description                                                      |
| ---------- | ---------------------------------------------------------------- |
| `feat`     | A new feature                                                    |
| `fix`      | A bug fix                                                        |
| `refactor` | A code change that neither fixes a bug nor adds a feature        |
| `docs`     | Documentation only changes                                       |
| `style`    | Changes that do not affect the meaning of the code (formatting, etc) |
| `test`     | Adding missing tests or correcting existing tests                |
| `chore`    | Maintenance tasks, CI config, tooling changes                    |
| `perf`     | A code change that improves performance                          |

### Rules

- **Header**: 50–72 characters, imperative mood, no trailing period
- **Subject**: Lowercase after the colon, no trailing period
- **Scope**: Optional, but if provided must be 2–30 characters, lowercase
- **Body**: Optional, but if provided must start with a blank line and wrap at 72 characters
- **Footer**: Optional, but if provided must start with a blank line and wrap at 72 characters
- **Breaking Changes**: Use `!` suffix after type/scope AND include a `BREAKING CHANGE:` footer

### Examples

✅ **Valid commits:**

```text
feat(cli): add --install flag for automatic package manager detection

When --install is provided, detect the package manager and configure
the project accordingly. This implies --once behavior.
```

```text
fix(runner): handle shell-word parsing edge cases

Shell-word parsing now correctly handles escaped quotes and nested
strings. This prevents command execution failures in edge cases.
```

```text
feat(api)!: redesign changed file detection

BREAKING CHANGE: The changed file detection algorithm now uses a
different diff base resolution chain. Users relying on the previous
behavior should update their workflows.
```

❌ **Invalid commits:**

```text
# Missing type
add new feature

# Wrong type (build, ci, revert not allowed)
ci: update workflow config

# Trailing period
feat: add new feature.

# Scope too short
feat(a): something

# Uppercase subject
feat: Add new feature
```

## Pull Requests

1. Create a feature branch from `main`
2. Make your changes following the commit convention above
3. Run all quality checks: `bash ./scripts/pre-commit.sh`
4. Push your branch and open a pull request
5. Ensure all CI checks pass

### CI Requirements

All PRs must pass these required checks:

- `fmt` — Code formatting
- `clippy` — Rust lints
- `nextest` — Tests
- `audit` — Security audit
- `deny` — License/advisory policy
- `shear` — Unused dependencies
- `gitleaks` — Secret detection
- `docs` — Documentation build

## Code Standards

- **Error Handling**: Use typed errors via `thiserror` in `error.rs`. Reserve `anyhow::Result` for top-level only.
- **Safety**: `unsafe` code is forbidden.
- **Formatting**: Follow `rustfmt.toml` rules (hard tabs, 120 width, 2024 edition rules).
- **Linting**: All `clippy::all`, `clippy::pedantic`, and `clippy::nursery` warnings must be addressed.

## Releases

Releases are managed by `cargo-release` and `cargo-dist`. Only maintainers can publish releases.

```bash
# Install cargo-release
cargo install cargo-release

# Cut a release (after updating CHANGELOG.md)
cargo release 0.2.0
```

Artifacts are published to:

- GitHub Releases
- Homebrew (`howmanysmall/pullhook`)
- npm (`@pobammer/pullhook`)
- Shell/PowerShell installers

## Questions?

Open an issue for bugs or feature requests. For questions, start a discussion on
GitHub.
