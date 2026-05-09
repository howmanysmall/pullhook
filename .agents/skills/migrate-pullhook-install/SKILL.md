---
name: migrate-pullhook-install
description: >
  Migrate legacy pullhook hook usage from `pullhook --install`, `pullhook -i`, or old
  `--pattern`/`--command`/`--script` post-merge commands to config-driven `pullhook.json` plus
  `pullhook run`. Use when converting hook snippets, README/setup docs, repo templates,
  or scripts away from legacy pullhook CLI flags into configuration.
domain:
  file_patterns:
    - ".git/hooks/*"
    - "*.sh"
    - "*.md"
    - "Makefile"
  watch_for:
    - pattern: "pullhook (--install|-i\\b)"
      category: install-migration
      type: anti-pattern
    - pattern: "pullhook --pattern"
      category: pattern-migration
      type: anti-pattern
    - pattern: "pullhook --script"
      category: script-migration
      type: anti-pattern
    - pattern: "pullhook run"
      category: config-mode
      type: exemplar
---

# Migrate Pullhook Install

Use this skill to convert old hook commands into config mode. Keep the task narrow: migration guidance and
edits only, not broad config-mode design.

## Principles
<!-- 🔒 LOCKED: Agent cannot modify -->

1. **Bootstrap with `pullhook init`**: Never create config files by hand unless a config already exists.
   `pullhook init` writes the canonical starter and errors when a config already exists.
2. **One config file only**: Multiple config files at the repo root cause a hard error. Never create a
   second config alongside an existing one.
3. **`install: true` is exclusive**: An install rule must not include `changed`, `exclude`, or `run`.
   These fields are mutually exclusive with `install: true` at both the schema and runtime level.
4. **Parallel groups own their level**: A `parallel` group entry must not define `changed`, `run`,
   `install`, or `runIfBaseMissing`. Child rules inside `parallel` own those fields.
5. **No nested groups**: `parallel` groups cannot be nested. Only one level of grouping is supported.
6. **No `--message` in config**: Config mode has no message field. Use the rule `name` for labelling;
   use `failText` only when migrating explicit failure guidance.
7. **`.yml` and JSON5 are rejected**: pullhook actively errors on both. Use `.yaml` for YAML configs.

## Migration Workflow

1. Find legacy usage:

   ```bash
   rg -n "pullhook\s+(--install|-i\b|--pattern|--command|--script)|post-merge" .
   ```

2. Bootstrap the config if one does not already exist:

   ```bash
   pullhook init
   ```

   `pullhook init` creates `pullhook.json` in the repo root. If a config already exists it will error —
   skip this step and edit the existing file instead.

3. If the old command is `pullhook --install` or `pullhook -i`, ensure the config contains an install
   rule. The starter config from `pullhook init` already includes one:

   ```json
   {
     "$schema": "https://raw.githubusercontent.com/howmanysmall/pullhook/main/schema.json",
     "onFailure": "stop",
     "rules": [
       {
         "name": "install dependencies",
         "install": true
       }
     ]
   }
   ```

4. Replace the hook command with:

   ```sh
   pullhook run
   ```

5. If migrating old `--pattern <glob> --command <command>` usage, add a rule:

   ```json
   {
     "name": "short action name",
     "changed": "<glob>",
     "run": "<command>"
   }
   ```

   `changed` accepts a single string or an array of strings. Both forms are valid:

   ```json
   "changed": "package-lock.json"
   "changed": ["package-lock.json", "yarn.lock"]
   ```

6. If migrating old `--pattern <glob> --script <script>` usage (npm script shorthand), expand it
   explicitly — config mode has no `script` shorthand:

   ```json
   {
     "name": "short action name",
     "changed": "<glob>",
     "run": "npm run <script>"
   }
   ```

7. Validate the migration:

   ```bash
   pullhook validate
   pullhook explain
   pullhook run --dry-run
   ```

## Config Field Reference

| Field | Scope | Notes |
|---|---|---|
| `name` | rule, group | Required. Appears in output and `failText` as `{rule}`. |
| `changed` | rule | String or array. Required unless `install: true`. |
| `exclude` | rule | String or array. Files matching are excluded from the match set. |
| `run` | rule | Command string. Required unless `install: true`. |
| `install` | rule | `true` only. Mutually exclusive with `changed`, `exclude`, and `run`. |
| `runIfBaseMissing` | rule | Run the rule even when no diff base can be resolved (shallow clones, fresh CI checkouts). |
| `failText` | rule, group | Template shown on failure. Placeholders: `{rule}`, `{command}`, `{cwd}`, `{exitCode}`. Supports Chalk-like style blocks: `{red.bold text}`. Styling respects `--render`. |
| `onFailure` | top-level | `"stop"` (default) or `"continue"`. |
| `parallel` | group | Array of rules run concurrently. Cannot co-exist with rule fields at the group level. |
| `jobs` | group | Max concurrency for a `parallel` group. Defaults to the top-level `--jobs` value. |

Supported config file names (discovery order):

```text
pullhook.json  pullhook.jsonc  pullhook.yaml  pullhook.toml
.pullhook.json  .pullhook.jsonc  .pullhook.yaml  .pullhook.toml
```

JSON5 (`.json5`) and YAML with the `.yml` extension are explicitly unsupported and cause a hard error.

## Rules of Thumb

- Use `pullhook init` to create the config; do not write it by hand.
- Prefer `pullhook.json` for new migrations unless an existing supported config format already exists.
- Never create a second config file alongside an existing one; pullhook errors on ambiguity.
- Do not combine `install: true` with `changed`, `exclude`, or `run`; they are mutually exclusive.
- Do not add `changed`, `run`, `install`, or `runIfBaseMissing` fields to a group entry (the one with
  `parallel`); those belong on the child rules inside `parallel`.
- Do not nest `parallel` groups; only one level of grouping is supported.
- Do not create `.yml` or JSON5 configs; use `.yaml` for YAML and `.json`/`.jsonc` for JSON.
- Do not preserve `--message` as a config field; use the rule `name` for normal output, and only use
  `failText` if migrating explicit failure guidance.
- Do not add TUI, `--interactive`, `tui` aliases, or terminal UI dependencies.
- Do not rewrite unrelated config-mode internals while doing a migration.

## Project Learnings
<!-- 🤖 AUTO-UPDATED: Agent writes learnings here -->

_Updated automatically as agent works on projects._
