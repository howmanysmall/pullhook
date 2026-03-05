# pullhook

[![CI](https://github.com/howmanysmall/pullhook/actions/workflows/ci.yaml/badge.svg)](https://github.com/howmanysmall/pullhook/actions/workflows/ci.yaml)
[![Release](https://github.com/howmanysmall/pullhook/actions/workflows/release.yml/badge.svg)](https://github.com/howmanysmall/pullhook/actions/workflows/release.yml)

`pullhook` runs commands when files changed by `git pull` match a glob pattern.

It keeps the familiar `git-pull-run` workflow, with additive improvements:

- resilient diff base fallback (`HEAD@{1}` -> `ORIG_HEAD`)
- safer command execution (no shell unless `--shell`)
- bounded parallel jobs (`--jobs`)
- dry-run previews (`--dry-run`)
- per-directory de-dupe (`--unique-cwd`)

## Install

### From source

```bash
cargo install --path .
```

### cargo-binstall

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

## Hook setup (`post-merge`)

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

Options:
  -p, --pattern <glob>      Pattern to match files (required unless --install)
  -c, --command <command>   Command to run for each match
  -s, --script <script>     Script to run as `npm run-script <script>`
  -i, --install             Detect package manager and run install (implies --once)
  -m, --message <message>   Message to print once when matches are found
  -d, --debug               Enable debug logging
  -o, --once                Run once in repo root
      --base <rev>          Override diff base revision
      --jobs <n>            Max parallel jobs (default: min(CPUs, 8))
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

Limit parallel work:

```bash
pullhook --pattern "packages/*/package-lock.json" --command "npm install" --jobs 4
```

## `--install` detection

`pullhook --install` detects package manager files from repo root:

- npm: `package-lock.json` or fallback `package.json`
- yarn: `yarn.lock`
- pnpm: `pnpm-lock.yaml`
- bun: `bun.lock` or `bun.lockb`
- deno: `deno.lock`, `deno.json`, or `deno.jsonc`
- vlt: `vlt-lock.json`

If conflicting lock files are present, `pullhook` errors and asks for explicit `--pattern`/`--command`.

## Output examples (`--render never`)

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
