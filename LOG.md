# LOG — M47: `VirtualList` Scrollbar Thumb + Build Tracker "Fixed Gaps"

- User's own governing instructions, two in one message: "Yes scope and
  built that" (the `VirtualList` scrollbar gap from the last audit) and
  "let's created a Fixed Gabs section with expand and collapse
  functionality... Add this to the template for the build tracker."
  Entered Plan Mode, did real investigation on both parts before
  writing a formal plan and getting approval.

## Part A — `VirtualList` scrollbar thumb

- Read `ScrollView`'s own identical M38 Phase 6 capability directly as
  the concrete precedent: `ScrollViewState::thumb_geometry`, `Tree::
  grabs_scroll_view_thumb`/`update_scroll_view_thumb_drag` (grab
  tolerance via `SCROLLBAR_GRAB_SLOP`, relative-delta drag anchor,
  direct never-eased scroll write), `engine-render::paint_scroll_view_
  thumb` (paint after children, shared scrollbar tokens already `pub`
  in `engine-core`).
- Real, load-bearing difference confirmed before writing code:
  `VirtualList` has no single measured child for "content extent" --
  reused its own `VirtualListState::total_extent()` (the same
  primitive `Tree::scroll_virtual_list_by`'s own clamping already uses,
  so wheel-scroll and thumb-drag can never disagree); `VirtualList` is
  vertical-only today, so the new code is a narrower single-axis
  version, not a horizontal-capable copy.
- `crates/engine-core/src/node.rs`: `VirtualListState` gained `thumb_
  drag_anchor: Option<(f64, f64)>` and `thumb_geometry(viewport_extent)
  -> (track, thumb, along)`.
- `crates/engine-core/src/tree.rs`: new `virtual_list_viewport_extent`/
  `grabs_virtual_list_thumb`/`update_virtual_list_thumb_drag`, wired
  into `PointerPressed`'s ancestor-walk (a second `NodeKind::
  VirtualList` check right after the existing `ScrollView` one) and
  `update_drag`'s `NodeKind` match.
- `crates/engine-render/src/lib.rs`: `paint_node` gained a `NodeKind::
  VirtualList` arm calling new `paint_virtual_list_thumb`, sharing the
  actual fill math with `paint_scroll_view_thumb` via a new small
  `fill_scrollbar_thumb` helper (factored out once a second real caller
  needed the identical `RoundedRect` fill).
- 6 new `engine-core` unit tests, mirroring `ScrollView`'s own test
  cluster exactly (same real numbers: 200x100 viewport, 400px content
  -> track 96, thumb 32). **Real test-fixture bug caught by actually
  running the tests:** the first `Variable`-extent drag test reused the
  identical tiny 30px-viewport fixture `scroll_virtual_list_by`'s own
  clamp test uses, which leaves zero real thumb travel once `SCROLLBAR_
  MIN_LENGTH` fills the whole track -- a real, correct no-op, but
  invalid for testing drag specifically; fixed with a larger, still
  genuinely non-uniform fixture with real drag travel.
- 2 new `engine-render` pixel-level tests (`virtual_list_scroll.rs`),
  mirroring `scroll_view.rs`'s own M38 Phase 6 tests. **A second real
  test-fixture bug caught by running it:** the shared `materialize`
  fixture paints items spanning the full viewport width, so the "no
  thumb" check point coincided with real opaque item content, not
  background -- fixed with a dedicated narrower-item fixture for just
  that test, mirroring `scroll_view.rs`'s own identical "use a
  transparent `Container`, not the shared opaque marker" fixture split.
- `examples/scrollable_list.py`'s doc comment updated: the new thumb
  paints automatically (no script change needed, its real 1,000-row
  list already exceeds its viewport); explicitly stated, matching
  `docking.py`/`resizable_panes.py`'s established honesty, that this
  engine has no synthetic Python-level "press at an arbitrary point,
  then move" primitive for *any* drag gesture (confirmed via grep
  before claiming this) -- the real thumb-drag itself stays proven at
  the Rust level, left for a human to try interactively.
- Full chain green: `cargo check`/`clippy -D warnings`/`fmt`, `cargo
  test --workspace --release` (`engine-core` 219, up from 213, +6;
  `engine-render` virtual_list_scroll suite 4, up from 2, +2),
  `maturin develop --release`, `pytest tests/` (629 passed, unchanged),
  all 82 examples, showcase demo.

## Part B — Build Tracker "Fixed Gaps" (the template)

- Discovered a real, already-existing `**Known gaps:**` convention in
  `~/.claude/skills/build-tracker/` (a reusable skill, not project-
  tracked in git -- confirmed via `git rev-parse --is-inside-work-tree`
  failing) with `generate_tracker_artifact.py`, `BUILD_TRACKER_TEMPLATE
  .md`, `SKILL.md`. Edited the skill's master copy first, then
  re-copied verbatim into `tools/`, per the skill's own explicit rule.
- `Tracker` gained `fixed_gaps: list[str]`; `_parse_bullet_list` shared
  by both "Known gaps"/"Fixed gaps"; new `render_fixed_gaps_section`
  wraps the list in a `<details class="fixed-gaps">` -- the same
  native pattern every milestone/phase already uses, so `expandAll`/
  `collapseAll` picks it up for free via the existing `querySelectorAll
  ('details')`, no new JS. New CSS block matching the existing metric-
  card/details styling. `main()`'s summary line gained the fixed-gap
  count.
- **Real bug found and fixed along the way, not anticipated during
  planning:** verified the change by regenerating this project's own
  much larger, real `BUILD_TRACKER.md` (not just a synthetic test) and
  found the published artifact's own "Just closed"/"Up next" highlight
  box had been showing text from roughly M31's own historical per-step
  note -- stale for over a dozen milestones. Root cause: `_grab()`
  searched the *whole file* and took the *last* match, an earlier fix
  (documented in SKILL.md's own "Lessons learned") that assumed fresh
  pairs are *appended*. This project's real, actually-used convention
  is the opposite -- a fresh pair is prepended at the very top, right
  after Top Metrics, every time a milestone closes -- and the file also
  accumulates many legitimate, older "Just closed" mentions deep inside
  individual milestone sections' own historical narrative (a step's
  own transient status note from when that milestone was being built),
  so "last match in the whole file" was silently finding one of those
  instead. Fixed by bounding the search to the real front matter
  (everything before the first `## Milestone <N>` heading) and taking
  the *first* match there.
- **A second real regression caught immediately by re-testing, not
  assumed fixed:** re-ran the skill's own `BUILD_TRACKER_TEMPLATE.md`
  after the front-matter fix and found it now returned an *empty*
  "Just closed"/"Up next" -- the template's own documented convention
  (a single pair at the true tail, after every milestone section) has
  nothing in the front matter to find. Fixed with a fallback: only
  when the front-matter search comes up empty, fall back to the
  original whole-file/last-match search -- correct for a single tail
  pair (there's only one to find either way). Re-verified both real
  conventions work: the template's own tail-pair convention, and this
  project's own front-matter-stack convention, side by side.
- `BUILD_TRACKER_TEMPLATE.md`: added a `**Fixed gaps:**` block modeling
  both a `**Fixed**` and a `**Resolved**` entry. `SKILL.md`: extended
  the "Exact format" section with the new grammar and the *convention*
  (move a closed bullet, don't strike it in place); added a maintenance
  -workflow step; rewrote the existing "Just closed"/"Up next" lesson
  to record the full two-bug arc (the original fix, and this session's
  correction of what that fix had assumed).

## Part C — Applied to this project's own `BUILD_TRACKER.md`

- Re-verified every one of the 19 existing "Known gaps" bullets against
  current source directly, not trusted from its own prior "Fixed" note
  -- confirmed `Node.add_child` (`crates/engine-py/src/node.rs:497`),
  `tracing::error!` wiring (`crates/engine-py/src/dispatch.rs`), and
  context-menu dismissal (`Tree::dismiss_on_outside_click`/`dismiss_
  escapable_overlays`) are all real, though none had ever actually been
  struck through in the original list despite being genuinely closed.
- Split into a trimmed "Known gaps" (2 items: handlers stay zero-
  argument with no real `Event` payload; no live accessibility client
  in this dev/CI environment) and a new "Fixed gaps" (19 entries,
  including the reframed PLAN.md/LOG.md-archiving bullet as "convention
  corrected, not a gap" and the new `VirtualList` scrollbar entry).
- Added the M47 milestone entry (Top Metrics row, "Just closed"
  paragraph, full `## Milestone 47` section with Phase 1/Step 1).
  **A real structural mistake caught immediately by re-parsing, not
  shipped:** first draft embedded a `### Phase 1` heading directly
  inside the "Just closed" narrative prose (before any `## Milestone`
  heading existed in the file at that point), which the parser would
  have silently ignored (no `current_milestone` set yet) -- moved to a
  proper `## Milestone 47` section in the correct position.
- Added a fresh "Up next: nothing currently scoped" entry -- a real,
  separate finding from the same front-matter investigation: this
  file's own "Up next" pointer had gone stale after roughly M17/M18
  and was never refreshed on any later closure, violating this
  project's own "never let this pair go stale" convention, silently,
  for about 30 milestones.
- `python tools/generate_tracker_artifact.py`: confirmed 47 milestones/
  139 phases/233 items/2 known gaps/19 fixed gaps (up from 46/138/232/
  --/--). Spot-checked the generated HTML directly for the correct
  "Just closed"/"Up next"/"Known gaps"/"Fixed gaps" content, not just
  the summary counts. Build Tracker artifact republished twice (once
  after the initial Fixed-Gaps migration, once after the front-matter
  parser fix) to the existing URL.

## Status

**M47 -- `VirtualList` Scrollbar Thumb -- is now fully complete, single
phase.** The Build Tracker template work (Parts B/C) is process/tooling
work on the tracker itself, folded into this same milestone's own
writeup rather than numbered separately, matching how this file has
never numbered its own maintenance passes as milestones. This closes
the milestone -- per the standing "push after a full milestone closes"
convention, a `git push` is now appropriate for the `tre` repo. The
`~/.claude/skills/build-tracker/` edits are not git-tracked (confirmed
directly) -- no push applicable there, the files are simply saved.
