# LOG — M65: Incremental Existence Tracking for `compute_layout`'s Per-Kind Scans

- Found by the same `/review-project` Performance-lens pass that
  surfaced M64, adversarially verified before scoping. `Tree::
  compute_layout` unconditionally called `sync_carousel_layouts`/
  `sync_button_group_layouts`/`sync_scroll_view_layouts`/`sync_virtual_
  list_layouts` every time it ran -- i.e. every dirty frame, during any
  interaction or animation -- and each of the four began with its own
  full `self.nodes.iter().filter(...).collect::<Vec<NodeId>>()` scan
  purely to discover whether any node of that one kind existed at all,
  before doing anything real. `ARCHITECTURE.md`'s own stated rationale
  ("Taffy's own internal caching makes that a cheap no-op") covers only
  Taffy's own pass, not these four scans that ran *before* Taffy was
  ever invoked.
- Real investigation finding, confirmed via direct source read before
  writing any code: `PaintProperties.button_group_reflow` (the flag
  `sync_button_group_layouts` filters on, since `ButtonGroup` isn't its
  own `NodeKind`) is set exactly once, at construction time inside
  `add_button_group` (`engine-py::window_factory.rs`) -- never mutated
  on an already-inserted node anywhere in the codebase (confirmed via
  grep across every crate) -- so an incremental counter needed only two
  real choke points, `Tree::insert` and `Tree::remove`, not a wider set
  of mutation sites to hook.

## What shipped (single milestone, both phases)

1. Four new `Tree` fields: `carousel_count`, `button_group_reflow_
   count`, `scroll_view_count`, `virtual_list_count`. Incremented in
   `Tree::insert` by matching the inserted node's own `kind` (for
   `Carousel`/`ScrollView`/`VirtualList`) and checking `paint.button_
   group_reflow.is_some()`. Decremented in `Tree::remove` the identical
   way -- `remove` is recursive (it removes children before the node
   itself), so the decrement logic reads the node's own real kind/flag
   while `node` is still borrowed from `self.nodes.get(id)`, before
   `self.nodes.remove(id)` invalidates it and before the recursive
   `self.remove(child)` calls run -- each of which independently
   re-enters this same function and does its own decrement for its own
   child, so a subtree removed only as a side effect of an ancestor's
   own removal is still tracked correctly.
2. Each of the four `sync_*_layouts` functions gated on its own counter
   being non-zero *before* the existing full-tree scan -- turning the
   common (no-node-of-this-kind) case from an O(n) scan-and-allocate
   into a real O(1) integer comparison. The pre-existing `if <scan_
   result>.is_empty() { return false; }` check inside each function was
   removed entirely, not merely made dead code -- the counter already
   guarantees the scan will find at least one real match whenever it
   actually runs, so the redundant post-scan check added nothing.
- Tests: 2 new Rust unit tests. `existence_counters_track_insert_and_
  remove_for_every_real_kind` inserts one real node of each tracked
  kind (`Carousel`, `ScrollView`, `VirtualList`, a `button_group_
  reflow`-flagged `Container`), confirms every counter reads exactly
  1, confirms inserting an unrelated plain node moves none of them,
  then removes each and confirms every counter returns to 0.
  `existence_counters_decrement_correctly_through_recursive_removal`
  builds a real 3-level chain (root -> middle -> Carousel), removes
  only `root`, and confirms `carousel_count` still correctly drops to
  0 even though `Carousel` was never removed directly -- the real path
  a naive "decrement only in the direct top-level call" implementation
  would have missed.
- `BUILD_TRACKER.md`: full Milestone 65 section, Top Metrics row at
  100%, Just-closed/Up-next refreshed to point at M66/M67 as the
  remaining backlog. Tracker regenerated (18 milestones/55 phases/137
  items/3 known gaps/25 fixed gaps), artifact republished.
- Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean; `cargo test --workspace --release` (`engine-core` 229, up
  from 227, +2; every other crate's own count unchanged -- notably
  including the 2 pre-existing `sync_button_group_layouts_leaves_
  widths_unchanged_when_nothing_is_pressed`/`sync_button_group_layouts_
  grows_the_pressed_child_and_shrinks_its_real_neighbors` behavioral
  tests, which pass completely unchanged -- real, direct proof the
  counter-gated paths still do exactly what they did before whenever a
  real match genuinely exists, not just "compiles"); `maturin develop
  --release`; `pytest tests/` 831 passed, unchanged from the pre-
  milestone baseline -- pure internal Rust-side caching/bookkeeping
  with no new Python-facing surface at all, the identical "Rust-tested
  only" precedent M64 already established for this same kind of
  change; every file in `examples/` ran clean (specifically re-checked
  every carousel/scroll/virtual-list/button-group-touching example and
  pytest test by name); `demo/showcase.py` (all 5 phases, exit 0).

## Status

**M65 is complete, both phases.** The second of the four `/review-
project` performance findings is closed with zero regression to any
existing test, example, or the showcase demo, and with a real
behavioral proof (the two pre-existing button-group tests passing
unchanged) that gating the scans behind a counter didn't quietly
change what happens on the path where a real match exists. Committing
locally now; push deferred pending explicit user confirmation. Next:
M66 (the text-shaping cache's own allocate-before-checking-the-cache
pattern in `shaped_layout`).
