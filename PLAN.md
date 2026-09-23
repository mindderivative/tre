# PLAN — M67: Eliminate Redundant Full-Content Clones in `draw_field`/`hit_test_position`

*(Replaces the prior M66 plan in this file — M66 is complete, committed.
Last of four milestones scoped from the `/review-project` audit; see
this file's own Milestone 67 section in `BUILD_TRACKER.md` for the full
real investigation.)*

## Goal
`draw_field` unconditionally called `elide_folded_ranges(&state.content,
&state.folded_ranges)`, which itself always allocated a full content
copy even when nothing was folded — then cloned that already-fresh
result a second time on the common no-preedit path. Found by the
review's Performance lens; the independent verification pass found the
real cost was worse than first reported: a third, wholly wasted
`state.content.clone()` in `draw_field`'s no-preedit match arm, shadowed
and discarded before use.

## Real investigation
The identical `elide_folded_ranges(...)` + conditional `.clone()`
pattern also exists at `hit_test_position`, called on every click/hit-
test against a `TextField` — the fix needed to cover both call sites,
not just `draw_field`. Tracing `draw_field`'s control flow confirmed the
no-preedit match arm's `state.content.clone()` was always immediately
discarded and reassigned, pure dead work.

## Design (1 milestone, 2 phases)
1. Remove the dead clone; `Cow`-ify the folding path.
2. Tests, docs, verification.

## Status

**Complete, both phases.**

`elide_folded_ranges` widened from returning `String` to `Cow<'a, str>`,
with an early `Cow::Borrowed(content)` return (zero allocation) when
`folded_ranges` is empty, `Cow::Owned` only when something is actually
folded. `draw_field`'s first match restructured to return
`Option<String>` (`preedit_display`) instead of a placeholder value,
eliminating the dead `state.content.clone()` entirely rather than just
making it harder to reach. Both real call sites (`draw_field`,
`hit_test_position`) updated for the new `Cow`-typed return.

Tests: existing `hit_test_position`/selection/cursor test suite re-ran
and passed completely unchanged as the real regression bar, plus 1 new
test (`elide_folded_ranges_borrows_when_nothing_is_folded_and_owns_when
_something_is`) asserting the `Cow::Borrowed`/`Cow::Owned` variant
directly via `matches!`.

Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
clean; `cargo test --workspace --release` (`engine-render` 34, up from
33, +1; every other crate unchanged, including all pre-existing tests
in this same file passing completely unchanged — real, direct proof
this restructuring is a pure internal change); `maturin develop
--release`; `pytest tests/` 831 passed, 2 skipped, unchanged; every
example ran clean; `demo/showcase.py` all 5 phases, exit 0.
`BUILD_TRACKER.md` updated (Top Metrics, full Milestone 67 section,
Just-closed/Up-next refreshed), tracker regenerated (18 milestones/55
phases/137 items/3 known gaps/25 fixed gaps), artifact republished.
Committing locally now.

Next: nothing currently scoped. All four `/review-project` performance
findings (M64-M67) are closed. Further work is the user's to direct.
