# LOG — Archive BUILD_TRACKER.md (Milestones 1-50) + Known Gaps Refresh + Artifact Visual Change

- User: "Let's archive the current TRE Build Track up to Milestone 50.
  Give it a date and milestone range 1-50. Then start a new TRE Build
  Tracker at Milestone 50. This should give us some overlap. Make sure
  Known Gaps are updated with all actually known gaps. Link to the
  Archived Build Tracker for reference to Milestones 1-50." Plus a
  separate visual request: stack "Up Next"/"Just Finished"/"Known
  Gaps" vertically with expand/collapse on the published artifact.
- Real investigation before designing: mapped `BUILD_TRACKER.md`'s own
  line structure (front matter through line 257, `## Milestone 1` at
  258, `## Milestone 50` at 1330-1365, `## Milestone 51` at 1366,
  `## Future Work` at 1583); confirmed the front matter had ~250 lines
  of stacked, mostly-redundant historical "Just closed"/"Up next"
  pairs going back years; located the shared, cross-project
  `build-tracker` skill (`/home/phil/.claude/skills/build-tracker/`)
  and confirmed its own stated "never hand-edit a project's copy,
  change the skill's own copy and re-copy" convention (matching how
  "Fixed gaps" was added previously); read `generate_tracker_
  artifact.py`'s own parser logic directly, confirming Top Metrics
  rows and `## Milestone <N>` sections are matched independently (safe
  to trim either) and "Just closed"/"Up next" take the first match
  within the front matter (trimming to one pair is exactly what this
  expects); found `expandAll`/`collapseAll` already use a broad
  `querySelectorAll('details')`, so wrapping the 3 summary boxes in
  `<details>` needs zero JS changes.
- Three real design forks resolved via `AskUserQuestion`: archive file
  at the repo root (`BUILD_TRACKER_ARCHIVE_M1-M50.md`), not under
  `archive/` (a different kind of thing -- the retired TRE v1
  codebase); a plain committed markdown file, not a second published
  Artifact, keeping the "one tracker = one stable URL" convention
  intact; the live tracker's front matter trimmed to just the current
  pair, not the full historical stack.
- **Known Gaps audit**, cross-checking every M48-M56 "explicitly out
  of scope" note against live source, not trusted from its own prior
  mention: found one brand-new, not-yet-recorded gap from M56's own
  investigation (`Window`'s `select_all`/`press_key`/`type_text`/
  `copy`/`cut`/`resize` still read `self.tree`/`self.handlers`/`self.
  root` directly, bypassing `self.active`, unlike `click`/`hover`/
  `scroll`/`focus` and the M56-fixed `*_system_clipboard` trio);
  confirmed via `grep` that four more real candidates from M48/M52's
  own "out of scope" notes were still true and never promoted to
  "Known gaps": MD3 theming catalog loose ends (`add_tooltip`'s color,
  `build_menu`'s panel shape/elevation, `add_search_bar`'s hardcoded
  icon-button radius, `add_pagination`'s inconsistent icon-button
  theme key, `add_time_input_field`'s missing `is_set()` gate); layout
  API breadth (per-side padding/margin, flex-grow/shrink/basis,
  align-items/justify-content -- zero hits anywhere in `StyleSpec`/
  `node.rs`, confirmed via `grep`); styling API breadth (border kwargs
  only on `add_rect`, confirmed the only such constructor params in
  the whole catalog; no token-reference substitution in `StyleSpec`;
  no theme-driven typography). Re-confirmed the existing accessibility-
  client gap unchanged. Explicitly did **not** add deliberate,
  already-accepted tradeoffs as gaps (hook/handler pruning on node
  removal, `{{ }}` bindings not surviving `retheme()`, app-owned state
  not re-derived by `set_theme`) -- those are named design decisions
  elsewhere, not missing capability.

## What shipped

1. `BUILD_TRACKER_ARCHIVE_M1-M50.md` (repo root, new): dated
   2026-09-23, Top Metrics rows M1-50, the full current "Fixed gaps"
   list (kept as genuine historical context for this range), and
   Milestone 1 through 50's own sections copied verbatim -- relocated
   content, not rewritten. Deliberately no "Known gaps" section of its
   own (inherently a live-state concept); points to the live tracker
   instead.
2. `BUILD_TRACKER.md` rewritten: Top Metrics trimmed to M50-56 (M50
   repeated in full -- the requested overlap, so a reader starting
   from either file has real context for where the other picks up); a
   short link to the new archive right after Top Metrics; front
   matter trimmed to exactly the current M56 "Just closed"/"Up next"
   pair, with "Up next" refreshed to point at the real current state
   (the 5 known gaps, none scoped as next) rather than stale M54/M55-
   era pointer text; "Known gaps" replaced with the refreshed 5-bullet
   list from the audit above; "Fixed gaps" kept as the full,
   un-trimmed 20-item history (low-cost, collapsed by default, worth
   keeping complete in the actively-maintained file); Milestone 50
   through 56 sections retained verbatim; "Future Work" (`vello_
   hybrid`) section unchanged.
3. Visual change made in the shared skill's own copy first
   (`/home/phil/.claude/skills/build-tracker/generate_tracker_
   artifact.py`, never the project's copy directly, per the skill's
   own stated convention), then re-copied verbatim to `tools/generate_
   tracker_artifact.py` (confirmed byte-identical via `diff`
   afterward): `.metrics` changed from `grid-template-columns:
   repeat(3, 1fr)` to a single-column vertical stack; the three
   `<div class="metric">` blocks became `<details class="metric"
   open>` elements with a clickable `<summary>` (the existing colored
   dot + label + the same `CHEV_SMALL` chevron `details.fixed-gaps`
   already uses, mirrored for visual consistency, not invented fresh)
   and a bordered-top `.metric-body`. All three default `open` so
   nothing regressed on first load. **Real bug caught before shipping,
   not after:** the first draft referenced `{CHEV_SMALL}` directly
   inside `PAGE_TEMPLATE`, a plain (non-f) string later filled via
   `.format(**kwargs)` -- `CHEV_SMALL` was never one of those kwargs,
   which would have raised `KeyError: 'CHEV_SMALL'` at generation time.
   Fixed by adding `chev_small=CHEV_SMALL` to the `.format()` call and
   referencing the lowercase `{chev_small}` placeholder instead,
   matching every other kwarg in that same call. `expandAll`/
   `collapseAll` needed zero changes -- their existing broad
   `querySelectorAll('details')` already picks up the new elements.
   A new "Archiving a large tracker" section added to `SKILL.md`,
   documenting this exact pattern (file location/naming, what goes in
   the archive vs. the live file, the one-milestone-overlap
   convention, the Known Gaps re-audit discipline) for future reuse --
   the same precedent "Fixed gaps" itself set when it was added to the
   skill, not just this project's own copy.
4. Regenerated via the project's own now-updated `tools/generate_
   tracker_artifact.py`: `Parsed 7 milestones, 23 phases, 67 items, 5
   known gaps, 20 fixed gaps` -- exactly matches Milestones 50-56, a
   real sanity check that the split parsed correctly, not just "it ran
   without error." Spot-checked the generated HTML directly: the three
   boxes render as `<details>` with the new chevron/vertical-stack
   markup, and the Known Gaps list shows all 5 refreshed bullets.
   Republished to the existing artifact URL (`https://claude.ai/
   artifact/CaPkWjpd91oR7YFbcqC9ty`), not a new one.

## Status

**Complete.** No Rust or Python source touched -- documentation/
tooling-only, no test suite or build chain involved. Committing locally
now (new archive file, rewritten `BUILD_TRACKER.md`, `LOG.md`/
`PLAN.md`, `tools/generate_tracker_artifact.py`); push deferred pending
explicit user confirmation, per this session's own established,
unwavering convention.
