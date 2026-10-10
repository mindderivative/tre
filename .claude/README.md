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

## Deliberately not here

- Personal settings and anything with credentials: `settings.local.json`, `~/.claude.json`,
  `.credentials.json`, histories, logs, and the session lock file.
- The skills Claude syncs from Anthropic (`~/.claude/skills/synced/`, such as pptx, docx and
  pdf): they come with their own licenses and are reinstalled by Claude itself.
- The maintainer's global `~/.claude/CLAUDE.md`: its policies are already in this repo's
  `CLAUDE.md`.
- Old agent worktrees under `.claude/worktrees/`.

To use these skills in other projects, copy `skills/<name>/` to `~/.claude/skills/` and
the command files to `~/.claude/commands/`.
