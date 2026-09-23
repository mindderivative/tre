# PLAN — M66: Zero-Allocation Cache-Hit Path in `shaped_layout`

*(Replaces the prior M65 plan in this file — M65 is complete,
committed. Third of four milestones scoped from the `/review-project`
audit; see this file's own Milestone 66 section in `BUILD_TRACKER.md`
for the full real investigation.)*

## Goal
`TextRenderer::shaped_layout` -- the one real, working text-shaping
cache in this codebase -- unconditionally built an owned `LayoutCacheKey`
(a content string clone, a syntax-span vec clone) *before* the
staleness check that decides whether a cache hit even needs it. Found
and adversarially verified by the review's Performance lens.

## Real investigation
Called once per visible `Text`/`Link` node (via `draw`) and once per
`TextField` (via `draw_field`) on every dirty-frame scene rebuild, so
the intended-cheap common case (an unchanged node whose shaping is
genuinely being reused) still paid a full clone just to build a
comparison key that got discarded once the comparison confirmed
nothing changed.

## Design (1 milestone, 2 phases)
1. Compare-before-clone.
2. Tests, docs, verification.

## Status

**Complete, both phases.**

`shaped_layout` restructured so the staleness check compares the
*borrowed* new inputs directly against the cached key's own fields
(`cached.key.content != content`, `cached.key.spans != spans`, etc. --
`String`/`Vec` both compare against a borrowed `&str`/`&[T]` via std's
own blanket `PartialEq` impls, no allocation needed). The owned
`LayoutCacheKey` (and the reshape itself) now only ever constructed
inside the real stale/miss branch, at `layout_cache.insert` time -- the
exact same "compare borrowed inputs before allocating" technique M64's
own new `shaped_terminal_run` was already written with from the start,
now applied back to the original code it was deliberately written to
not repeat.

Tests: 1 new Rust unit test, real per-field regression coverage for the
hand-written comparison -- changes exactly one of the 9 real shaping
inputs at a time, keeping every other one identical, and confirms the
cached key's own field actually updated after each change. This is the
concrete risk a hand-written comparison has that a derived `PartialEq`
doesn't: a field silently missing from it would stop invalidating on
that one input, with nothing else catching it.

Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
clean; `cargo test --workspace --release` (`engine-render` 33, up from
32, +1; every other crate unchanged, including all 32 pre-existing
tests in this same file passing completely unchanged -- real, direct
proof this is a pure internal restructuring, not a behavior change);
`maturin develop --release`; `pytest tests/` 831 passed, unchanged;
every example ran clean; `demo/showcase.py` all 5 phases, exit 0.
`BUILD_TRACKER.md` updated (Top Metrics, full Milestone 66 section,
Just-closed/Up-next refreshed), tracker regenerated (18 milestones/55
phases/137 items/3 known gaps/25 fixed gaps), artifact republished.
Committing locally now.

Next: M67 (`draw_field`/`hit_test_position`'s redundant full-content
clones during code folding) -- the last of the four `/review-project`
performance findings.
