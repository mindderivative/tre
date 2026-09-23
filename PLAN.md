# PLAN — M60: Styling API Breadth I: Border Kwargs Across the Catalog

*(Replaces the prior M61 plan in this file — M61 is complete, committed.
Fourth of six milestones from the approved M57-M62 plan to actually
close, though dispatched before M61 and M62; see `/home/phil/.claude/
plans/reflective-sleeping-falcon.md` for the full roadmap.)*

## Goal
The first, most mechanical of the 3 genuinely separate pieces the
"Scope the following Known Gaps" investigation found bundled in one
"styling API breadth" bullet (see M61/M62). `add_rect`'s own existing
`border_color`/`border_width` kwargs were the only place in the whole
58-entry catalog a caller could set a border at all.

## Real investigation
`add_rect`'s own pattern (conditionally overwriting `paint.border_
color`/`border_width`) is mechanical to extend -- `PaintProperties` is
universal across every `NodeKind`. But `paint_node` (`engine-render`)
only strokes a border for `NodeKind::Rect | Splitter | LoadingIndicator`
-- only the ~35-40 factories whose primary node is `NodeKind::Rect` are
in scope; factories returning `Text`/`Container`/`ScrollView`/`Carousel`
would silently no-op without also extending `paint_node` (out of
scope); ~12-14 factories with their own dedicated `NodeKind` state
(`Checkbox`/`Slider`/etc.) raise a genuine design question, also out of
scope.

## Why dispatched to a background agent
Sheer mechanical repetition -- the same real 4-line pattern applied to
~32 near-identical factories, no custom retheme-hook logic needed per
factory (unlike M52's own catalog work). Dispatched to a general-purpose
agent in an isolated git worktree with a very detailed prompt (exact
pattern, explicit exclusion list, the identical verification-chain
requirements every other milestone this session has, explicit "do NOT
touch BUILD_TRACKER.md/commit anything" boundary) so the main session
could do M61's design-heavier work in parallel instead of idling on
mechanical edits.

## Status

**Complete, both phases.**

1: `border_color`/`border_width` optional kwargs added to 32 factories
beyond the pre-existing `add_rect`, each verified against its own real
`tree.insert(NodeKind::...)` call site by the agent, not guessed from
name -- full list in this file's own `BUILD_TRACKER.md` Milestone 60
section. Multi-node-return factories handled by real judgment: `Vec
<Node>` returns apply the border uniformly (no distinguishable "first"
peer); tuple returns apply it only to the first/primary element;
`add_side_sheet`/`add_navigation_drawer` apply it to whichever real node
is actually returned in each branch; `add_split_button` forwards into
its own internal `add_button` call.

2: 6 new pytest tests. `python/tre/_core.pyi` updated for all 32
factories, cross-checked programmatically against the real Rust source.

**Merge note:** the agent's own worktree branched from a commit before
M57/M58/M59/M61 landed on `main`. Its own uncommitted 3-file changeset
(`window_factory.rs`/`_core.pyi`/`test_live_style.py`) was extracted as
a patch and applied onto current `main` via `git apply -3` (a real
3-way merge using the shared base blob) -- 2 real conflicts surfaced in
`window_factory.rs`, both in factories M58 had *also* touched
(`add_tooltip`'s theme-aware color resolution, `add_pagination`'s
`"icon_button"`-key unification): resolved by hand, keeping both M58's
theme fixes and M60's new border kwargs together, not one overwriting
the other. The full verification chain was then re-run from scratch
against the merged result, not just trusted from the agent's own
pre-merge run.

Full chain green (post-merge): `cargo check`/`clippy -D warnings`/
`fmt --check` clean; `cargo test --workspace --release` (227
engine-core, 21 engine-md3, 74 engine-spec, all matching the M61
baseline, zero regressions from the merge); `maturin develop --release`;
`pytest tests/` (824 passed, up from 818, +6, 2 skipped unchanged); all
examples; `demo/showcase.py` all 5 phases, exit 0. `BUILD_TRACKER.md`
updated (Top Metrics, full Milestone 60 section, Just-closed/Up-next
refreshed), tracker regenerated (13 milestones/46 phases/114 items/2
known gaps/24 fixed gaps), artifact republished. Committing locally now.

Next: M62 (styling API breadth III: typography theming). Real scope
finding surfaced while starting M62's own investigation: the approved
plan's "role -> family/weight/size/line-height" type scale can't
actually apply line-height anywhere -- `TextState`/`TextSpec` have no
such field, and no line-height concept exists in the render/layout
pipeline at all. Asked the user via `AskUserQuestion`; they chose to
also add real `line_height` plumbing to `TextState`/the text-layout
path this milestone, widening M62 beyond pure theme data into genuine
new engine-core/engine-render capability -- see the updated Milestone 62
section in `BUILD_TRACKER.md` for the revised phase breakdown.
