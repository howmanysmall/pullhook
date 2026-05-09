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
  config      Show the resolved pullhook config path
  rules       List configured rule and group names
  schema      Print or write the pullhook JSON Schema
  init        Create a starter pullhook config file
  completion  Generate shell completion scripts
  help        Print this message or the help of the given subcommand(s)

Options:
  -p, --pattern <glob>      Pattern to match files
  -c, --command <command>   Execute command for each matched file
  -s, --script <script>     Execute npm script for each matched file
  -i, --install             Detect package manager and run install
  -m, --message <message>   Print message if any matches are found
  -d, --debug               Enable debug logging
      --render <mode>       Control non-debug ANSI styling: auto, always, or never
      --no-color            Disable ANSI styling in non-debug output
  -o, --once                Run command once in repo root if any match
      --base <rev>          Override the git base revision
      --jobs <n>            Max concurrent jobs
      --shell               Run --command via a shell
      --dry-run             Print planned commands and exit
      --json                Print machine-readable JSON instead of text output
      --unique-cwd          Dedupe directories before per-match execution
  -h, --help                Print help
  -V, --version             Print version
```

The live `--help` output also includes examples and next-step hints for the root command and the main subcommands.

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
pullhook --pattern "**/*.rs" --command "cargo test" --dry-run --json
pullhook rules
pullhook rules --kind install
pullhook rules --rule lint --json
pullhook rules --commands-only
pullhook rules --rule lint --commands-only
pullhook rules --patterns-only
pullhook rules --rule lint --patterns-only
pullhook schema --output .vscode/pullhook.schema.json
pullhook schema --check --output .vscode/pullhook.schema.json
pullhook explain --changed-file packages/a/package-lock.json
git diff --name-only HEAD~1 | pullhook explain --changed-files-file -
pullhook explain --summary-only
pullhook explain --commands-only
pullhook explain --changed-files-only
pullhook explain --matched-files-only
pullhook explain --matched-rules-only
pullhook run --summary-only
pullhook run --commands-only
pullhook run --changed-files-only
pullhook run --matched-files-only
pullhook run --matched-rules-only
pullhook run --require-match --dry-run
pullhook run --changed-file packages/a/package-lock.json --dry-run
git diff --name-only HEAD~1 | pullhook run --changed-files-file - --dry-run
git diff --name-only HEAD~1 | pullhook run --changed-files-stdin --dry-run
pullhook run --rule lint --dry-run
```

Generate shell completions:

```bash
pullhook completion bash > ~/.local/share/bash-completion/completions/pullhook
pullhook completion zsh > "${fpath[1]}/_pullhook"
pullhook completion fish > ~/.config/fish/completions/pullhook.fish
pullhook completion fish --output ~/.config/fish/completions/pullhook.fish
pullhook completion fish --check --output ~/.config/fish/completions/pullhook.fish
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
or preview the starter config before writing it. `--stdout` works outside a git repo because it never writes:

```bash
pullhook init --format yaml
pullhook init --output config/pullhook.custom.json
pullhook init --dry-run --json
pullhook init --format jsonc --stdout
pullhook init --force
```

`--force` only overwrites the existing config in place. It will not silently switch an existing repo
from `pullhook.json` to `pullhook.yaml`.
Use `init --dry-run` to preview the target path and format without writing a config. Add `--json` when setup
scripts need the same plan as structured data with top-level `status`, `error`, and `details` fields.
Use `--output <path>` when you want to scaffold a custom config path for later use with `--config <path>`;
the output extension chooses the file format unless you also pass a matching `--format`.

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
pullhook validate --config config/pullhook.ci.json
pullhook config
pullhook config --path-only
pullhook config --require-existing --path-only
pullhook config --json
pullhook validate
pullhook validate --quiet
pullhook validate --json
pullhook doctor
pullhook doctor --quiet
pullhook doctor --strict
pullhook doctor --json
pullhook schema
pullhook schema --output .vscode/pullhook.schema.json
pullhook explain
pullhook explain --all-matches --json
pullhook run --json
pullhook run --dry-run --json
pullhook run --dry-run
pullhook run --quiet
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

Styles respect `--render auto|always|never`; use `--no-color` as a shortcut for plain non-debug output.
Placeholders render in every mode.

`pullhook config` shows the config path and format that pullhook will use without parsing the file.
Use `pullhook config --path-only` when a script needs the resolved path as one clean line.
Add `--require-existing` when that script should fail instead of returning a planned-but-missing config path.
`config --json` includes `status`, `source`, and `error` fields for the same automation-friendly result shape used by other JSON commands.
Standard JSON errors also include `details` when there is a deeper cause chain, so scripts can show the short error while still keeping the useful diagnostic text.
Missing config JSON errors include setup details for `pullhook init` and `--config <path>`.
`pullhook schema` prints the config JSON Schema, and `pullhook schema --output <path>` writes it for editor setup.
Use `schema --check --output <path>` in CI when a checked-in schema file must stay current.
`schema --check --json` and `completion <shell> --check --json` include top-level `status`, `error`, and
`details` fields with the rerun command when generated output is stale.
`validate --json` emits a compact config summary for scripts and still prints structured JSON when the
config is invalid, including `status`, `error`, and `details` fields. Use `validate --quiet` when CI only needs the exit code. `doctor` checks repo discovery, config health, diff-base availability, and `--install`
detection in one pass, with a short hint for each check. Use `doctor --strict` when CI should fail on warnings.
Use `doctor --quiet` to suppress all-ok text output without hiding warnings or errors.
`doctor --json` includes `status`, `strict`, `error`, and summary booleans so automation can read the result without scraping stderr.
`explain --json` emits the evaluated rule plan,
including changed files, their source (`git`, `explicit`, or `base-missing`), matched files, commands, and
skip reasons. `explain --json`, `run --dry-run --json`, and `run --json` include top-level `status` and
`error` fields; `run --json` also adds real execution results, captured stdout/stderr, and a final summary.
Diff-base JSON errors include details for invalid `--base <rev>` values and the automatic fallback path.
Their JSON output also includes a `summary` object with changed-file source, matched-file, matched-rule, and
planned-command counts so scripts do not need to walk every entry.
Use `explain --summary-only` when you only need changed-file, matched-file, and planned-command counts.
Use `explain --commands-only` when another script should receive only the planned command lines.
Use `explain --changed-files-only` when a script needs the resolved changed-file paths without parsing JSON.
Use `explain --matched-files-only` when a script needs the matched changed-file paths without parsing JSON.
Use `explain --matched-rules-only` when a script needs the rule names that would run without parsing JSON.
Use `run --summary-only` for the same clean plan counts from the execution subcommand; it exits before running anything.
Use `run --commands-only` for the same clean command list from the execution subcommand; it exits before running anything.
Use `run --changed-files-only` for the same clean changed-file list from the execution subcommand; it exits before running anything.
Use `run --matched-files-only` for the same clean matched-file list from the execution subcommand; it exits before running anything.
Use `run --matched-rules-only` for the same clean matched-rule list from the execution subcommand; it exits before running anything.
Add `--require-match` to `explain` or `run` when an empty plan should fail the command after printing its normal output.
`run --dry-run --json` emits the same plan plus `plannedCommands`, which is handy for CI or editor integrations.
Use `run --quiet` when successful text output would be noise; failures still print the failed task, any `failText`,
and the final summary.

Legacy top-level mode also supports `--json`, including the same top-level `status`, `error`, and `details`
fields for setup failures, live execution results, and dry-run plans.
Legacy command-parse JSON errors include a `commandParse` object with the rejected command and parser reason.

Use `pullhook rules` to list configured rule and parallel group names before targeting a large config.
`rules --json` includes top-level `status` and `error` fields like the other JSON commands, plus
script-friendly `selectors`, `commands`, and `patterns` arrays with summary counts.
Unknown selector JSON errors include `unknownSelectors`, `availableSelectors`, and `suggestions`, so scripts
do not need to scrape the human error message.
Use `pullhook rules --names-only` when a script or completion helper only needs valid selector names.
Use `pullhook rules --commands-only` when a script only needs configured `run` command lines without evaluating changed files.
Use `pullhook rules --patterns-only` when a script needs the configured `changed` globs without evaluating changed files.
Combine either one with `--rule <name>` to print command lines or globs for one rule or parallel group.
Use `pullhook rules --kind install`, `--kind run`, or `--kind group` to narrow inventory output.
Use `--rule <name>` with `rules`, `run`, or `explain` to focus on specific rule names or parallel groups in large configs.
Repeat it to target more than one selector, for example `pullhook run --rule lint --rule typecheck`.
Rule and group names share one selector namespace, so each configured name must be unique.
Use `--changed-file <path>` with `run` or `explain` to evaluate against explicit paths instead of the git diff;
repeat it to simulate several changed files. Use `--changed-files-file <path>` or `--changed-files-stdin` when
a script already has a newline-delimited file list, such as `git diff --name-only`. Pass `--changed-files-file -`
to read that list from stdin.
Missing changed-files file JSON errors include `changedFilesFile` and recovery details, so callers do not need
to scrape the error string for the path.

Use `--config <path>` with `run`, `explain`, `validate`, `doctor`, or `rules` when you want to point at a
specific config file instead of repo-root discovery. Pair it with `pullhook init --output <path>` to create
that custom file.

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
