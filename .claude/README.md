# Claude Code files for this project

Claude Code reads project-level skills from `.claude/skills/` and slash commands from
`.claude/commands/` when it works in this repo, so a fresh clone has them. They are the
maintainer's own, copied here so they survive a lost machine.

## Skills

- **`research-mode`**: the maintainer's definition of "optimization, performance,
  efficiency, feature, and research mode" in their own words, and the measure-first method
  that worked for `tre`: baseline and inventory, findings as ranked Backlog issues, nothing
  built until approved.
- **`build-tracker`**: the `BUILD_TRACKER.md` format and the script that renders it as an
  interactive page (`generate_tracker_artifact.py`, the same script as
  `tools/generate_tracker_artifact.py`). `tre` used it up to 0.4; from 0.5.0 progress is
  tracked on the GitHub project instead (see `docs/contributing.md`), so it is kept for
  reference and for the archived trackers (`BUILD_TRACKER_ARCHIVE_*.md`).

## Commands

- **`/review-project`**: a multi-agent code review (performance, architecture, security,
  modernization lenses) that fixes what needs no decision and reports the rest.
- **`/create-tracker`**: drafts a hierarchical build tracker (Milestones, Phases, Stages,
  Steps).

## `synced-skills/`: skills Claude syncs from Anthropic

Only the ones licensed for redistribution are copied: `mcp-builder` and `skill-creator`
(Apache License 2.0, each with its `LICENSE.txt`), plus `manifest.json`, the list of every
skill the account syncs (name, description, source, date), so the rest can be re-synced by
Claude itself on a new machine. Copy a skill directory to `~/.claude/skills/` to use it.

## Deliberately not here

- The maintainer's personal Claude Code settings, global instructions and anything with
  credentials or identifiers: `settings.local.json`, `~/.claude/settings.json`,
  `~/.claude/CLAUDE.md`, `~/.claude.json`, `.credentials.json`, histories, logs, and the
  session lock file. This repository is public. Its policies are already in this repo's
  `CLAUDE.md`; sign in and set up the rest again on a new machine.
- The other synced skills (`pptx`, `docx`, `pdf`, `xlsx`, and the ones with no license file):
  the first four are marked "All rights reserved" and forbid copying or redistribution
  outside the Claude services. Claude re-syncs them.
- Old agent worktrees under `.claude/worktrees/`: a stale snapshot, not a git repository,
  and already covered by this repo's history.

To use the project's own skills in other projects, copy `skills/<name>/` to
`~/.claude/skills/` and the command files to `~/.claude/commands/`.
