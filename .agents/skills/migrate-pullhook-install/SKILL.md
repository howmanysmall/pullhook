---
name: migrate-pullhook-install
description: >
  Migrate legacy pullhook hook usage from `pullhook --install`, `pullhook -i`, or old
  `--pattern`/`--command` post-merge commands to config-driven `pullhook.json` plus
  `pullhook run`. Use when converting hook snippets, README/setup docs, repo templates,
  or scripts away from legacy pullhook CLI flags into configuration.
---

# Migrate Pullhook Install

Use this skill to convert old hook commands into config mode. Keep the task narrow: migration guidance and
edits only, not broad config-mode design.

## Migration Workflow

1. Find legacy usage:

   ```bash
   rg -n "pullhook (.*--install|.*-i\\b|.*--pattern|.*--command)|post-merge" .
   ```

2. If the old command is `pullhook --install` or `pullhook -i`, create or update `pullhook.json`:

   ```json
   {
     "$schema": "https://pullhook.dev/schema.json",
     "onFailure": "stop",
     "rules": [
       {
         "name": "install dependencies",
         "install": true
       }
     ]
   }
   ```

3. Replace the hook command with:

   ```sh
   pullhook run
   ```

4. If migrating old `--pattern <glob> --command <command>` usage, convert it to:

   ```json
   {
     "name": "short action name",
     "changed": "<glob>",
     "run": "<command>"
   }
   ```

5. Validate the migration:

   ```bash
   pullhook validate
   pullhook run --dry-run --render never
   ```

## Rules of Thumb

- Prefer `pullhook.json` for new migrations unless an existing supported config already exists.
- Do not create `.yml` or JSON5 configs.
- Do not combine `install: true` with `changed` or `run`; `install: true` owns package-manager detection.
- Do not preserve `--message` as a config field; config mode has no direct message equivalent. Use the rule name
  for normal output, and only use `failText` if migrating failure guidance.
- Do not add TUI, `--interactive`, `tui` aliases, or terminal UI dependencies.
- Do not rewrite unrelated config-mode internals while doing a migration.
