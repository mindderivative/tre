# PLAN — M65: Incremental Existence Tracking for `compute_layout`'s Per-Kind Scans

*(Replaces the prior M64 plan in this file — M64 is complete, committed.
Second of four milestones scoped from the `/review-project` audit; see
this file's own Milestone 65 section in `BUILD_TRACKER.md` for the full
real investigation.)*

## Goal
`Tree::compute_layout` unconditionally ran four full-tree scans (one
per `Carousel`/`ButtonGroup`-reflow/`ScrollView`/`VirtualList`) on
every dirty frame just to discover whether any node of that kind
existed at all. Found and adversarially verified by the review's
Performance lens.

## Real investigation
Each of the four `sync_*_layouts` functions began with a
`self.nodes.iter().filter(...).collect::<Vec<NodeId>>()` scan purely to
answer a yes/no existence question. `ARCHITECTURE.md`'s own stated
rationale ("Taffy's own internal caching makes that a cheap no-op")
covers only Taffy's own pass, not these four scans running before Taffy
is ever invoked. Real finding confirmed before implementing:
`PaintProperties.button_group_reflow` is set exactly once, at
construction time inside `add_button_group`, never mutated on an
already-inserted node anywhere in the codebase (confirmed via grep) --
so the fix needed only two real choke points, `Tree::insert` and
`Tree::remove`.

## Design (1 milestone, 2 phases)
1. Per-kind existence counters.
2. Tests, docs, verification.

## Status

**Complete, both phases.**

Four new `Tree` fields (`carousel_count`/`button_group_reflow_count`/
`scroll_view_count`/`virtual_list_count`), incremented in `Tree::insert`
when the inserted node's own kind/flag matches, decremented in `Tree::
remove` -- `remove` is recursive, so the decrement fires once per real
removed node (top-level or reached only through a parent's own
recursive removal), reading the node's own kind/flag while it's still
borrowed, before `self.nodes.remove(id)` invalidates it. Each of the
four `sync_*_layouts` functions gated on its own counter being non-zero
before the existing scan -- the pre-existing `if <scan>.is_empty() {
return false; }` check removed entirely (not just made unreachable),
since the counter already guarantees a match exists whenever the scan
actually runs.

Tests: 2 new Rust unit tests -- all four counters track insert/remove
correctly for every real kind and stay untouched by an unrelated plain
node; a `Carousel` removed only as a side effect of an ancestor's own
recursive removal still decrements correctly.

Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
clean; `cargo test --workspace --release` (`engine-core` 229, up from
227, +2; every other crate unchanged, including the 2 pre-existing
`sync_button_group_layouts_*` behavioral tests passing unchanged --
real proof the counter-gated paths changed nothing about actual
behavior); `maturin develop --release`; `pytest tests/` 831 passed,
unchanged -- pure internal caching, no new Python-facing surface, the
identical precedent M64 already established; every example ran clean;
`demo/showcase.py` all 5 phases, exit 0. `BUILD_TRACKER.md` updated
(Top Metrics, full Milestone 65 section, Just-closed/Up-next
refreshed), tracker regenerated (18 milestones/55 phases/137 items/3
known gaps/25 fixed gaps), artifact republished. Committing locally
now.

Next: M66 (zero-allocation cache-hit path in `shaped_layout`).
