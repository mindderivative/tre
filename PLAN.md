# PLAN — Archive BUILD_TRACKER.md (Milestones 1-50) + Known Gaps Refresh + Artifact Visual Change

*(Replaces the prior M56 plan in this file — M56 is complete, committed,
and pushed; its own full writeup lives permanently in `BUILD_TRACKER.md`
's own Milestone 56 section. This is a separate, non-milestone
maintenance task on the tracker/tooling itself.)*

## Goal
`BUILD_TRACKER.md` had grown to 56 milestones / ~1600 lines, with a
front-matter section that had itself accumulated ~250 lines of stacked,
mostly-redundant historical "Just closed"/"Up next" pairs. User:
"Let's archive the current TRE Build Track up to Milestone 50. Give it
a date and milestone range 1-50. Then start a new TRE Build Tracker at
Milestone 50. This should give us some overlap. Make sure Known Gaps
are updated with all actually known gaps. Link to the Archived Build
Tracker for reference to Milestones 1-50." Plus a smaller visual
request: the "Up Next"/"Just Finished"/"Known Gaps" boxes on the
published artifact should stack vertically with expand/collapse,
instead of the prior fixed 3-column grid.

## Three real design forks, resolved via `AskUserQuestion`
1. Archive file lives at the repo root: `BUILD_TRACKER_ARCHIVE_M1-M50.md`
   (not under `archive/`, which holds the retired TRE v1 codebase — a
   different kind of thing).
2. Plain committed markdown file, not a second published Artifact —
   keeps the "one tracker = one stable artifact URL" convention intact.
3. The live tracker's front matter is trimmed to just the current
   pair — older stacked pairs' substance already lives in each
   Milestone's own write-up, and (for M1-50) in the archive file.

## Real investigation
`BUILD_TRACKER.md`'s own line map: front matter (Top Metrics, Known/
Fixed gaps, ~250 lines of stacked historical narrative) through line
257; `## Milestone 1` at 258; `## Milestone 50` at 1330-1365;
`## Milestone 51` at 1366; `## Future Work` (not milestone-numbered)
at 1583. The shared, cross-project `build-tracker` skill lives at
`/home/phil/.claude/skills/build-tracker/` — its own stated convention
is to never hand-edit a project's copy of `generate_tracker_
artifact.py`, only the skill's own copy, then re-copy. Parser facts
confirmed by reading the script directly: Top Metrics rows and
`## Milestone <N>` sections are matched independently per-row/per-
section (safe to trim either); "Known gaps"/"Fixed gaps" take the
first occurrence in the file; "Just closed"/"Up next" take the first
match within the front matter — trimming to one pair is exactly what
this logic expects. The 3-box HTML was three plain `<div class=
"metric">` blocks; `expandAll`/`collapseAll` already use a broad
`querySelectorAll('details')`, so wrapping them in `<details>` needed
zero JS changes.

**Known Gaps audit** (cross-checked every M48-M56 "explicitly out of
scope" note against live source, confirmed each still real, not
assumed): found one brand-new, not-yet-recorded gap from M56's own
investigation (`Window`'s `select_all`/`press_key`/`type_text`/`copy`/
`cut`/`resize` still bypass `self.active`, unlike `click`/`hover`/
`scroll`/`focus` and the M56-fixed clipboard trio); confirmed four more
real, still-open candidates named-but-never-promoted from M48/M52's own
"out of scope" notes (MD3 theming catalog loose ends; layout API
breadth; styling API breadth); confirmed the existing accessibility-
client gap unchanged. Explicitly did *not* add deliberate, already-
accepted tradeoffs (handler/hook pruning on node removal, `{{ }}`
bindings not surviving `retheme()`, app-owned state not re-derived by
`set_theme`) — those aren't missing capability, just design decisions
already named elsewhere.

## Status

**Complete.**

1. `BUILD_TRACKER_ARCHIVE_M1-M50.md` created at the repo root: dated
   2026-09-23, Top Metrics rows M1-50, the full current "Fixed gaps"
   list, and Milestone 1 through 50's own sections copied verbatim
   (relocated, not rewritten). No "Known gaps" section of its own —
   points to the live file instead.
2. `BUILD_TRACKER.md` rewritten: Top Metrics now M50-56 (M50 repeated
   as the requested overlap), a link to the new archive right after
   Top Metrics, front matter trimmed to exactly the current M56 "Just
   closed"/"Up next" pair (refreshed to reflect the real current
   state), "Known gaps" replaced with the refreshed 5-bullet list,
   "Fixed gaps" kept as the full, un-trimmed 20-item history, Milestone
   50 through 56 sections retained verbatim, "Future Work" section
   unchanged.
3. Visual change made in the shared skill's own copy first
   (`/home/phil/.claude/skills/build-tracker/generate_tracker_
   artifact.py`), then re-copied verbatim to `tools/generate_tracker_
   artifact.py` (confirmed byte-identical via `diff`): the three top
   boxes are now `<details class="metric" open>` elements with a
   chevron summary mirroring `details.fixed-gaps`'s existing look,
   `.metrics` changed from a 3-column grid to a vertical stack, zero JS
   changes needed (`expandAll`/`collapseAll`'s existing `querySelectorAll
   ('details')` already covers the new elements). A new "Archiving a
   large tracker" section added to `SKILL.md`, documenting this exact
   pattern for future reuse — the same precedent "Fixed gaps" itself
   set when it was added to the skill, not just this project.
4. Regenerated (`Parsed 7 milestones, 23 phases, 67 items, 5 known
   gaps, 20 fixed gaps` — matches M50-56 exactly) and republished to
   the existing artifact URL
   (`https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty`).

No Rust or Python source touched — documentation/tooling-only, no test
suite or build chain involved. `git add` the new archive file, the
rewritten `BUILD_TRACKER.md`, `LOG.md`/`PLAN.md`, and `tools/generate_
tracker_artifact.py` (never `CLAUDE.md`, never `-A`); local commit per
standing policy; push only after separate explicit confirmation.
