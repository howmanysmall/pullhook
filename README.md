# `pullhook`

[![CI](https://github.com/howmanysmall/pullhook/actions/workflows/ci.yaml/badge.svg)](https://github.com/howmanysmall/pullhook/actions/workflows/ci.yaml)
[![Release](https://github.com/howmanysmall/pullhook/actions/workflows/release.yml/badge.svg)](https://github.com/howmanysmall/pullhook/actions/workflows/release.yml)

`pullhook` runs commands when files changed by `git pull` match a glob pattern.

It keeps the familiar `git-pull-run` workflow, with additive improvements:

- resilient diff base fallback (`HEAD@{1}` -> `ORIG_HEAD`)
- safer command execution (no shell unless `--shell`)
- bounded parallel jobs (`--jobs`)
- dry-run previews (`--dry-run`)
- per-directory dedupe (`--unique-cwd`)

## Install

### From Source

```bash
cargo install --path .
```

### `cargo-binstall`

```bash
cargo binstall pullhook
```

### Homebrew

```bash
brew install howmanysmall/pullhook/pullhook
```

## Build

```bash
cargo build
cargo build --release
```

## Hook Setup (`post-merge`)

Create `.git/hooks/post-merge`:

```bash
#!/usr/bin/env sh
pullhook --install --message "Dependency files changed. Running install..."
```

Then make it executable:

```bash
chmod +x .git/hooks/post-merge
```

## Usage

```text
Usage: pullhook [OPTIONS]
       pullhook <COMMAND>

Commands:
  run         Run configured pullhook rules
  explain     Explain which configured rules match changed files
  validate    Validate the pullhook config file
  doctor      Inspect repository and config readiness
  init        Create a starter pullhook config file
  completion  Generate shell completion scripts
  help        Print this message or the help of the given subcommand(s)

Options:
  -p, --pattern <glob>      Pattern to match files
  -c, --command <command>   Execute command for each matched file
  -s, --script <script>     Execute npm script for each matched file
  -i, --install             Detect package manager and run install
  -m, --message <message>   Message to print once when matches are found
  -d, --debug               Enable debug logging
      --render <mode>       Control non-debug ANSI styling: auto, always, or never
  -o, --once                Run once in repo root
      --base <rev>          Override diff base revision
      --jobs <n>            Max concurrent jobs
      --shell               Run --command through shell
      --dry-run             Print planned commands and exit
      --unique-cwd          De-dupe per-match working directories
  -h, --help                Print help
  -V, --version             Print version
```

## Examples

Run install when `package-lock.json` changed:

```bash
pullhook --pattern "package-lock.json" --command "npm install"
```

Run once from repo root:

```bash
pullhook --pattern "packages/*/package-lock.json" --command "npm install" --once
```

Auto-detect package manager and install:

```bash
pullhook --install
```

Preview commands without executing:

```bash
pullhook --pattern "**/*.rs" --command "cargo test" --dry-run
```

Generate shell completions:

```bash
pullhook completion bash > ~/.local/share/bash-completion/completions/pullhook
pullhook completion zsh > "${fpath[1]}/_pullhook"
pullhook completion fish > ~/.config/fish/completions/pullhook.fish
```

Limit parallel work:

```bash
pullhook --pattern "packages/*/package-lock.json" --command "npm install" --jobs 4
```

## Config Mode

Config mode is for repo-local post-pull recovery rules. It runs named commands once from the repository
root when files changed by a pull or merge match configured globs.

Create a starter config:

```bash
pullhook init
```

`pullhook init` creates `pullhook.json` by default. You can also scaffold another supported format
or preview the starter config before writing it:

```bash
pullhook init --format yaml
pullhook init --format jsonc --stdout
pullhook init --force
```

`--force` only overwrites the existing config in place. It will not silently switch an existing repo
from `pullhook.json` to `pullhook.yaml`.

Config discovery supports exactly one of:

- `pullhook.json`
- `pullhook.jsonc`
- `pullhook.yaml`
- `pullhook.toml`
- `.pullhook.json`
- `.pullhook.jsonc`
- `.pullhook.yaml`
- `.pullhook.toml`

JSON5 and `.yml` are intentionally unsupported.

Example:

```json
{
  "$schema": "https://pullhook.dev/schema.json",
  "onFailure": "stop",
  "rules": [
    {
      "name": "install dependencies",
      "install": true
    },
    {
      "name": "build generated assets",
      "changed": ["src/generated/**", "package.json"],
      "exclude": "src/generated/README.md",
      "run": "npm run build:generated",
      "failText": "{red.bold {rule} failed}. Try {cyan {command}} from {cwd}."
    },
    {
      "name": "independent checks",
      "jobs": 2,
      "parallel": [
        {
          "name": "typecheck",
          "changed": ["src/**/*.ts", "tsconfig.json"],
          "run": "npm run typecheck"
        },
        {
          "name": "unit tests",
          "changed": ["src/**/*.rs", "Cargo.toml"],
          "run": "cargo test"
        }
      ]
    }
  ]
}
```

Run configured rules:

```bash
pullhook validate
pullhook validate --json
pullhook doctor
pullhook doctor --json
pullhook explain
pullhook explain --all-matches --json
pullhook run --dry-run --json
pullhook run --dry-run
pullhook run
```

Rules match when any `changed` pattern matches and no `exclude` pattern matches. Top-level entries stop
on the first failure by default; set `"onFailure": "continue"` to run later top-level entries while still
exiting non-zero at the end.

`install: true` reuses the same package-manager detection as `pullhook --install` and watches the same
package-manager files. Config-mode commands always run from the repository root.

`failText` supports fixed placeholders and Chalk-like style blocks:

- Placeholders: `{rule}`, `{command}`, `{cwd}`, `{exitCode}`
- Style examples: `{bold text}`, `{red.bold text}`, `{bgRed.white.bold text}`

Styles respect `--render auto|always|never`; placeholders render in every mode.

`validate --json` emits a compact config summary for scripts. `doctor` checks repo discovery, config
health, diff-base availability, and `--install` detection in one pass. `explain --json` emits the
evaluated rule plan, including changed files, matched files, commands, and skip reasons. `run --dry-run --json`
emits the same plan plus `plannedCommands`, which is handy for CI or editor integrations.

## `--install` Detection

`pullhook --install` detects package manager files from repo root:

- `npm`: `package-lock.json` or fallback `package.json`
- `yarn`: `yarn.lock`
- `pnpm`: `pnpm-lock.yaml`
- `bun`: `bun.lock` or `bun.lockb`
- `aube`: `aube-lock.yaml`
- `deno`: `deno.lock`, `deno.json`, or `deno.jsonc`
- `vlt`: `vlt-lock.json`

If conflicting lockfiles are present, `pullhook` errors and asks for explicit `--pattern`/`--command`.

`aube` also watches the common JavaScript lockfiles, since changing any of them can require a fresh `aube install`.

## Output Examples (`--render never`)

These examples show deterministic plain output without ANSI styling.

Success:

```text
Prepare
pattern: packages/a/package-lock.json

Discovery
changed: 2
matched: 1

Tasks
directory: packages/a
command: npm install
[ok] success

Summary
matched files: 1
task dirs: 1
passed: 1
failed: 0
interrupted: 0
[ok] all tasks passed
```

No change:

```text
Prepare
pattern: **/*.md

Discovery
changed: 2
matched: 0

Result
[warn] no matching files found
```

Dry run:

```text
Prepare
pattern: packages/*/package-lock.json

Discovery
changed: 2
matched: 2

Dry Run
directory: packages/a
command: npm install
[warn] planned only
directory: packages/b
command: npm install
[warn] planned only

Summary
matched files: 2
task dirs: 2
planned commands: 2
executed commands: 0
[warn] dry run only: 2 command(s) planned, 0 executed
```

Failure:

`stdout`

```text
Prepare
pattern: packages/a/package-lock.json

Discovery
changed: 2
matched: 1

Tasks
directory: packages/a
command: sh -c 'echo fail-stdout; echo fail-stderr >&2; exit 7'
fail-stdout
[error] failed

Summary
matched files: 1
task dirs: 1
passed: 0
failed: 1
interrupted: 0
[error] 1 task(s) failed
```

`stderr`

```text
fail-stderr
[error] task failed
cwd: packages/a
command: sh -c 'echo fail-stdout; echo fail-stderr >&2; exit 7'
status: exit code 7
error: 1 task(s) failed
```

## Development

Run tests:

```bash
cargo nextest run
```

Run quality checks:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo audit
cargo deny check
cargo shear
```

## License

MIT
