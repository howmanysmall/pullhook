# pullhook Output Contract (Parity + Polish)

## Purpose

Define deterministic, testable non-debug output behavior for `pullhook` after hard cutover to a `git-pull-run`-like staged UX with improved visual quality.

## Global Rules

- Output order is deterministic and follows task index order, never completion timing order.
- Non-debug output is stable for assertions and excludes variable timing values.
- Copy style is sentence case, concise, and operationally clear.
- Pluralization is explicit (`1 file`, `2 files`; `1 task`, `2 tasks`).
- Spacing rhythm:
  - One blank line between major sections.
  - No trailing spaces.
  - No double blank lines.
- Stream routing is strict (see table below).

## Render Modes

- `auto`: styled output only when TTY is detected.
- `always`: force styled output.
- `never`: force plain output (ASCII-safe, no ANSI control sequences).

## Non-TTY / Plain Fallback Rules

- No ANSI escape sequences.
- Use plain labels (`[OK]`, `[WARN]`, `[ERROR]`) instead of colored badges.
- Keep section/line ordering identical to styled mode.

## Staged Flow (Non-Debug)

1. Preparation line
2. Changed-files discovery line
3. Optional message (if `--message` is set and matches found)
4. Task execution blocks (or dry-run blocks)
5. Final summary line

### Stage Line Requirements

- Preparation line appears once per run.
- Discovery line appears once and includes changed count and matched count.
- Optional message appears once and only when there is at least one match.
- No-match path prints completion summary and exits success without task blocks.

## Task Block Requirements

For each task directory, print in this order:

1. Directory context label
2. Command label
3. Captured command output (stdout then stderr content)
4. Terminal status line for the task (`success`, `failed`, `interrupted`, `spawn_error`)

## Dry-Run Block Requirements

- Uses same visual language and ordering as live task blocks.
- Prints planned command and target directory without execution output.
- Includes dry-run summary line with planned task count.

## Summary Requirements

Final summary includes:

- matched file count
- task directory count
- success count
- failure count
- interrupted count

Failure/interrupt summary must remain deterministic and include command context in associated error lines.

## Stream Routing Contract

| Line Type | Stream |
| --- | --- |
| Preparation/discovery/message lines | stdout |
| Directory and command labels | stdout |
| Captured command stdout | stdout |
| Captured command stderr content | stderr |
| Per-task failure/error detail | stderr |
| Final success summary | stdout |
| Final non-success summary note | stderr |
| Fatal orchestration error prefix (`error: ...`) | stderr |

## Edge Case Contract

- Spawn failure: task state `spawn_error`; include cwd + command context.
- Signal termination/no exit code: task state `interrupted`; summary increments interrupted.
- Non-UTF8 process output: lossy-decoded consistently; no panics.
- Non-zero exit with empty output: emit deterministic fallback detail string.

## Test-Mappable Checklist

- [ ] No-match run prints preparation + discovery + success summary, no task blocks.
- [ ] Single-match run prints one task block and success summary.
- [ ] Multi-match run prints blocks in deterministic task order.
- [ ] `--once` run prints single root task block with correct counts.
- [ ] `--dry-run` run prints dry-run blocks and dry-run summary only.
- [ ] Failure run prints stderr failure detail and non-zero summary path.
- [ ] Interrupted run increments interrupted count and uses interrupted status copy.
- [ ] Spawn-error run prints deterministic spawn failure detail.
- [ ] Render mode `never` contains no ANSI escape sequences.
- [ ] Stream routing assertions validate stdout/stderr placement per table.
