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
  shells      List supported shell completion targets
  formats     List supported config formats and filenames
  managers    List supported package-manager install detection
  categories  List command categories and their coverage
  examples    Show common pullhook workflows and commands
  commands    List pullhook commands for humans or automation
  codes       List stable JSON status codes for automation
  help        Print this message or the help of the given subcommand(s)

Options:
  -h, --help                Print help
  -V, --version             Print version

Legacy one-off options:
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
```

The live `--help` output also groups subcommand options by task, such as input, output, rule selection,
checks, and display settings.

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
pullhook rules --exclude-patterns-only
pullhook rules --rule lint --exclude-patterns-only
pullhook rules --fail-text-only
pullhook rules --rule lint --fail-text-only
pullhook schema --output .vscode/pullhook.schema.json
pullhook schema --check --output .vscode/pullhook.schema.json
pullhook schema --check --quiet --output .vscode/pullhook.schema.json
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
pullhook shells
pullhook shells --names-only
pullhook shells --descriptions-only
pullhook completion bash > ~/.local/share/bash-completion/completions/pullhook
pullhook completion zsh > "${fpath[1]}/_pullhook"
pullhook completion fish > ~/.config/fish/completions/pullhook.fish
pullhook completion fish --output ~/.config/fish/completions/pullhook.fish
pullhook completion fish --check --output ~/.config/fish/completions/pullhook.fish
pullhook completion fish --check --quiet --output ~/.config/fish/completions/pullhook.fish
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
pullhook init --dry-run --path-only
pullhook init --dry-run --format-only
pullhook init --dry-run --action-only
pullhook init --dry-run --json
pullhook init --format jsonc --stdout
pullhook init --force
```

`--force` only overwrites the existing config in place. It will not silently switch an existing repo
from `pullhook.json` to `pullhook.yaml`.
Use `init --dry-run` to preview the target path and format without writing a config. Add `--json` when setup
scripts need the same plan as structured data with top-level `status`, stable `code`, `error`, and `details` fields.
Use `init --dry-run --path-only`, `--format-only`, or `--action-only` when a script only needs one field from that plan.
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
pullhook config --format-only
pullhook config --exists-only
pullhook config --source-only
pullhook config --require-existing --path-only
pullhook config --json
pullhook validate
pullhook validate --quiet
pullhook validate --path-only
pullhook validate --json
pullhook doctor
pullhook doctor --quiet
pullhook doctor --strict
pullhook doctor --checks-only
pullhook doctor --codes-only
pullhook doctor --json
pullhook shells
pullhook shells --search fish
pullhook shells --names-only
pullhook shells --commands-only
pullhook shells --descriptions-only
pullhook shells --json
pullhook formats
pullhook formats --search yaml
pullhook formats --files-only
pullhook formats --init-commands-only
pullhook formats --descriptions-only
pullhook formats --json
pullhook managers
pullhook managers --search pnpm
pullhook managers --patterns-only
pullhook managers --commands-only
pullhook managers --lock-files-only
pullhook managers --config-files-only
pullhook managers --watched-files-only
pullhook managers --json
pullhook categories
pullhook categories --search workflow
pullhook categories --names-only
pullhook categories --commands-only
pullhook categories --example-commands-only
pullhook categories --descriptions-only
pullhook categories --json
pullhook examples
pullhook examples --category reference
pullhook examples --category reference --commands-only
pullhook examples --category reference --titles-only
pullhook examples --search install
pullhook examples --search install --summaries-only
pullhook examples --command run
pullhook examples --command run --commands-only
pullhook examples --command schema --commands-only
pullhook examples --command-names-only
pullhook examples --categories-only
pullhook examples --json
pullhook commands
pullhook commands --category diagnostic
pullhook commands --category reference --names-only
pullhook commands --search config
pullhook commands --repo-only
pullhook commands --standalone-only --names-only
pullhook commands --categories-only
pullhook commands --category workflow --example-commands-only
pullhook commands --search config --summaries-only
pullhook commands --json
pullhook codes
pullhook codes --kind doctor-check
pullhook codes --surface run
pullhook codes --search config
pullhook codes --kinds-only
pullhook codes --surfaces-only
pullhook codes --search config --descriptions-only
pullhook codes --kind doctor-check --codes-only
pullhook codes --json
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
Use `pullhook config --format-only`, `--exists-only`, or `--source-only` when a script only needs the resolved format, file-existence status, or discovery source.
Add `--require-existing` when that script should fail instead of returning a planned-but-missing config path.
`config --json` includes `status`, stable `code`, `source`, and `error` fields for the same automation-friendly result shape used by other JSON commands.
Standard JSON errors also include `details` when there is a deeper cause chain, so scripts can show the short error while still keeping the useful diagnostic text.
Missing config JSON errors include setup details for `pullhook init` and `--config <path>`.
Unsupported config-path JSON errors list the supported config filenames and an `init --output` recovery command.
`pullhook schema` prints the config JSON Schema, and `pullhook schema --output <path>` writes it for editor setup.
Use `schema --check --output <path>` in CI when a checked-in schema file must stay current.
Add `--quiet` when successful schema or completion checks should only report through their exit code.
`schema --check --json` and `completion <shell> --check --json` include top-level `status`, stable `code`, `error`, and
`details` fields with the rerun command when generated output is stale.
Use `pullhook shells` to inspect supported shell completion targets in text form, or `pullhook shells --json`
when another tool needs shell names and generation commands without scraping help text.
Add `--search <text>` to match shell names, completion commands, or descriptions case-insensitively.
Use `pullhook shells --names-only` when a script only needs supported shell names.
Use `pullhook shells --commands-only` when a script only needs shell completion commands.
Use `pullhook shells --descriptions-only` when a script only needs short shell descriptions.
Use `pullhook formats` to inspect supported config formats and discovery filenames.
Add `--search <text>` to match format names, config filenames, descriptions, or init commands.
Use `pullhook formats --files-only` when a script only needs the config filenames pullhook discovers.
Use `pullhook formats --init-commands-only` when a script only needs starter config commands.
Use `pullhook formats --descriptions-only` when a script only needs short format descriptions.
Use `pullhook managers` to inspect package-manager detection files and install commands.
Add `--search <text>` to match package-manager names, install commands, detection patterns, or watched files.
Use `pullhook managers --patterns-only` when a script only needs install detection patterns.
Use `pullhook managers --commands-only` when a script only needs package-manager install commands.
Use `pullhook managers --lock-files-only` when a script only needs the lock files pullhook checks first.
Use `pullhook managers --config-files-only` when a script only needs fallback package-manager config files.
Use `pullhook managers --watched-files-only` when a script only needs the deduped files that can trigger installs.
`validate --json` emits a compact config summary for scripts and still prints structured JSON when the
config is invalid, including `status`, stable `code`, `error`, `details`, and `validationErrors` fields. Config parse failures also include a `parseError` object with the config path and parser reason. Use `validate --quiet` when CI only needs the exit code. Use `validate --path-only` when a script needs the validated config path as one clean line. `doctor` checks repo discovery, config health, diff-base availability, and `--install`
detection in one pass, with a short hint for each check. Use `doctor --strict` when CI should fail on warnings.
Use `doctor --quiet` to suppress all-ok text output without hiding warnings or errors.
Use `doctor --checks-only` or `doctor --codes-only` when scripts only need the check names or stable check codes.
`doctor --json` includes `status`, stable top-level and per-check `code` values, `strict`, `error`, and summary booleans so automation can read the result without scraping stderr.
Unsupported config-path JSON errors include a `configPathError` object with the rejected path, extension, reason, and supported config filenames.
Missing config JSON errors include a `configDiscoveryError` object with the searched repo root and default config filename.
`explain --json` emits the evaluated rule plan,
including changed files, their source (`git`, `explicit`, or `base-missing`), matched files, commands, and
skip reasons. `explain --json`, `run --dry-run --json`, and `run --json` include top-level `status` and
stable `code` and `error` fields; `run --json` also adds real execution results, captured stdout/stderr, and a final summary.
Diff-base JSON errors include details for invalid `--base <rev>` values and the automatic fallback path,
plus a `diffBaseError` object with the failing revision or diff-base failure kind.
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

Legacy top-level mode also supports `--json`, including the same top-level `status`, stable `code`, `error`, and `details`
fields for setup failures, live execution results, and dry-run plans.
Repository-discovery JSON errors include recovery details for running inside or initializing a Git repo,
plus a `repositoryError` object with the searched path.
Missing-mode legacy JSON errors explain the config-mode path and the explicit legacy `--pattern`/`--command` fallback.
Pattern JSON errors include a `patternError` object with the rejected glob and parser reason.
Legacy command-parse JSON errors include a `commandParse` object with the rejected command and parser reason.
Use `pullhook examples` to inspect common workflows in text form, or `pullhook examples --json` when another
tool needs example commands without scraping docs.
Add `--command run`, `--command explain`, `--command validate`, `--command doctor`, `--command config`,
`--command init`, `--command schema`, `--command completion`, `--command commands`, `--command shells`,
`--command formats`, `--command managers`, `--command codes`, or `--command legacy` to focus the example list.
Use `pullhook examples --category reference` to narrow examples by command category.
Add `--search <text>` to match example titles, commands, categories, or summaries case-insensitively.
Use `pullhook examples --commands-only` when a script or completion helper only needs example command lines.
Use `pullhook examples --command-names-only` when a script only needs the commands covered by matching examples.
Use `pullhook examples --titles-only` when a script or completion helper only needs example titles.
Use `pullhook examples --summaries-only` when a script or command palette only needs short descriptions.
Use `pullhook examples --categories-only` when a script only needs the matching example categories.
Use `pullhook categories` to inspect command categories with command and example counts.
Add `--search <text>` to match category names or descriptions case-insensitively.
Use `pullhook categories --descriptions-only` when a script or command palette only needs category descriptions.
Use `pullhook categories --commands-only` when a script needs command names for matching categories.
Use `pullhook categories --example-commands-only` when a script needs example command lines for matching categories.
Use `pullhook commands` to inspect the command catalog in text form, or `pullhook commands --json` when another
tool needs supported commands, categories, repo requirements, and example invocations without scraping help text.
Add `--category workflow`, `--category diagnostic`, `--category generator`, or `--category reference` to narrow
the catalog.
Add `--search <text>` to match command names, categories, or summaries case-insensitively.
Use `--repo-only` or `--standalone-only` to split commands by whether they need to run inside a Git repository.
Use `pullhook commands --names-only` when a script or completion helper only needs command names.
Use `pullhook commands --summaries-only` when a script or command palette only needs command descriptions.
Use `pullhook commands --categories-only` when a script only needs the matching command categories.
Use `pullhook commands --example-commands-only` when a script needs example invocations for the matching commands.
Use `pullhook codes` to inspect the stable code catalog in text form, or `pullhook codes --json` when another
tool needs the catalog without scraping docs. Add `--kind error` or `--kind doctor-check` to narrow the list.
Add `--surface run`, `--surface doctor`, or another case-insensitive surface fragment to focus codes by command area.
Add `--search <text>` to match codes, surfaces, kinds, or descriptions case-insensitively.
Use `pullhook codes --codes-only` when a script or completion helper only needs stable code strings.
Use `pullhook codes --kinds-only` when a script needs the distinct code kinds.
Use `pullhook codes --surfaces-only` when a script needs the distinct code surfaces.
Use `pullhook codes --descriptions-only` when a script or command palette only needs code descriptions.

Use `pullhook rules` to list configured rule and parallel group names before targeting a large config.
`rules --json` includes top-level `status`, stable `code`, and `error` fields like the other JSON commands, plus
script-friendly `selectors`, `commands`, `patterns`, `excludePatterns`, and `failText` arrays with summary counts.
Unknown selector JSON errors include `unknownSelectors`, `availableSelectors`, and `suggestions`, so scripts
do not need to scrape the human error message.
Use `pullhook rules --names-only` when a script or completion helper only needs valid selector names.
Use `pullhook rules --commands-only` when a script only needs configured `run` command lines without evaluating changed files.
Use `pullhook rules --patterns-only` when a script needs the configured `changed` globs without evaluating changed files.
Use `pullhook rules --exclude-patterns-only` when a script needs configured `exclude` globs without evaluating changed files.
Use `pullhook rules --fail-text-only` when a script needs configured `failText` templates without executing rules.
Combine these line-output modes with `--rule <name>` to print values for one rule or parallel group.
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

Use `pullhook managers --json` to get the same package-manager install contract in a stable, machine-readable shape.
Use `pullhook managers --search <text>` with `--json`, `--names-only`, `--patterns-only`, or `--commands-only` to narrow the catalog.
If conflicting lockfiles are present, `pullhook` errors and asks for explicit `--pattern`/`--command`.
`--install --json` detection errors include recovery details for missing or ambiguous repo-root package-manager files.
They also include a `packageManagerError` object with either the searched repo root or the ambiguous package-manager names.

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
