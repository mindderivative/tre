# Plan: M12 Phase 1 — Real Variable-Height Extent Support in `engine-core` (§11.7)

Corresponds to `BUILD_TRACKER.md` M12 Phase 1's own scoping: `ItemExtent`
gains a real way to represent variable, per-item extents; `VirtualListState`'s
materialization math and the M8 Phase 3 scroll-clamping math both
generalize to real per-item cumulative sums.

## Investigation before writing code

- `ItemExtent` (`crates/engine-core/src/node.rs:137-147`) has exactly
  one variant, `Fixed(f64)`, confirmed by direct read; its own doc
  comment already states the intent verbatim: "fixed, or a size-hint
  callback for variable-height items."
- Two real production call sites assume uniform spacing, confirmed by
  direct read of `tree.rs`:
  - `set_virtual_list_window` (line 816): `top: length((idx as f64 *
    item_extent) as f32)` — every materialized item's own absolute
    `inset.top` is `idx * item_extent`.
  - `scroll_virtual_list_by` (line 850): `content_extent =
    state.item_extent.value() * state.item_count as f64` — the M8
    Phase 3 scroll-clamp's own total-extent computation.
- `engine-py`'s own `set_virtual_list_window` (`window.rs:832-842`)
  additionally uses the scalar `item_extent` for a *third*, distinct
  purpose: each materialized item's own `Style.size.height =
  length(item_extent as f32)` (line 855) — its real on-screen height,
  not its position. This is a real, separate value from the cumulative
  offset (Phase 2's own concern; `engine-core` doesn't set item size at
  all, only position, so this doesn't affect this phase's own design).
- `engine-render`'s own scroll-offset handling (`lib.rs:520`,
  `Affine::translate((0.0, -state.scroll_offset.current))`) is a pure
  paint-time translation applied uniformly to the whole `VirtualList`
  subtree, entirely independent of `item_extent` — confirmed via direct
  read, so **no `engine-render` change is needed this phase**: as long
  as each materialized item's own `inset.top` already holds its correct
  real cumulative offset, the existing scroll translation composes
  correctly regardless of whether spacing is uniform or variable.
- **Real design question investigated before choosing a shape:** how
  should `engine-core` represent "item N's real cumulative offset" for
  `Variable` mode, given §4's own pyo3-agnostic boundary rules out
  storing a callback here (the same real reason `materialize` itself
  lives in `engine-py`, not `engine-core`)? Two shapes were considered:
  1. Store raw per-item *heights* (`BTreeMap<usize, f64>`) and have
     `engine-core` sum them on every `offset_of`/`total_extent` call.
     Rejected: summing a growing map on every call is real, avoidable
     repeated work, and — more importantly — computing item `idx`'s
     offset this way requires *every* preceding index's height to
     already be resolved and present, which `engine-core` itself has no
     way to enforce or request (it can't call back into Python to
     resolve a missing one).
  2. Store already-*cumulative offsets* directly (`BTreeMap<usize,
     f64>`, keyed by index, value = that item's own real top-offset) —
     chosen. `engine-core` never sums anything itself; it only looks a
     resolved value up, `O(log n)` via `BTreeMap`. The special key
     `item_count` (one past the last real item — the same "one past
     the end" convention a `Range` already uses) holds the real total
     content extent for `total_extent()`. Whoever populates this map
     (Phase 2, in `engine-py`) owns the actual cumulative-sum
     computation — matching this codebase's own established "`engine-
     core` carries inert state, an `engine-py` method is what drives
     it" shape (`materialized`, `context_menus`, `dock`, all the same
     pattern) rather than teaching `engine-core` a new kind of active
     computation.
- `ItemExtent::value()` (the sole method on the type today) has no
  sensible single-`f64` meaning once a second variant exists — it's
  removed outright; its two production callers are rewritten in terms
  of the new `VirtualListState::offset_of`/`total_extent` methods
  instead (confirmed via grep: `value()` has no other callers anywhere
  in the workspace beyond those two, both being rewritten).

## Design

`crates/engine-core/src/node.rs`:

- `ItemExtent` gains `Variable` (a fieldless marker — the real per-item
  data lives on `VirtualListState`, not the enum itself, since only
  `VirtualListState` has a `Tree`-mutation path to populate it):
  ```rust
  pub enum ItemExtent {
      Fixed(f64),
      Variable,
  }
  ```
- `VirtualListState` gains:
  ```rust
  /// Real, resolved cumulative offsets for `Variable`-extent lists --
  /// index `idx` maps to item `idx`'s own real top-offset (not its
  /// height). Always empty for `Fixed` (whose offsets are computed
  /// directly). The key `item_count` (one past the last real item)
  /// holds the real total content extent. `engine-core` never computes
  /// a cumulative sum itself -- only looks a resolved value up; §4's
  /// own pyo3-agnostic boundary is why the actual per-item size-hint
  /// resolution lives in `engine-py` (Phase 2), not here.
  pub resolved_offsets: BTreeMap<usize, f64>,
  ```
- Two new `VirtualListState` methods, the single shared source of truth
  both `tree.rs` call sites use:
  ```rust
  /// Item `idx`'s own real top-offset. Panics if `item_extent` is
  /// `Variable` and `idx` isn't yet resolved -- an internal bookkeeping
  /// bug (the caller must resolve an index before positioning it), the
  /// same "internal bug, not a runtime condition" contract `set_
  /// splitter_position` already uses for its own malformed-call panics.
  pub fn offset_of(&self, idx: usize) -> f64 { ... }

  /// The real total content extent -- `item_count * item_extent` for
  /// `Fixed`, or `offset_of(item_count)` for `Variable`.
  pub fn total_extent(&self) -> f64 { ... }
  ```

`crates/engine-core/src/tree.rs`:

- `set_virtual_list_window`: the `top: length((idx as f64 *
  item_extent) as f32)` line becomes `top: length(state.offset_of(idx)
  as f32)`, computed via a fresh, short-lived immutable borrow of
  `self.nodes[list]` inside the loop (matching the borrow shape the
  rest of this method already uses to interleave reads and the mutating
  `self.insert`/`self.add_child` calls) rather than extracting a single
  scalar up front, since the offset now genuinely varies per index.
- `scroll_virtual_list_by`: `state.item_extent.value() *
  state.item_count as f64` becomes `state.total_extent()`.
- New `Tree::set_virtual_list_resolved_offsets(&mut self, list: NodeId,
  offsets: impl IntoIterator<Item = (usize, f64)>)` — the real way a
  caller supplies resolved cumulative offsets; merges into `state.
  resolved_offsets` via `extend`. This is Phase 2's own entry point
  into this phase's new storage — `engine-core` itself never calls it,
  the same "primitive first, wiring second" split M10 Phase 3/M11
  Phase 1 both already used.

## Verification plan

- `cargo test --workspace --release`/clippy/fmt. New `engine-core`
  tests: `offset_of`/`total_extent` for `Fixed` match the pre-existing
  uniform formula exactly (regression guard); for `Variable`, a real,
  non-uniform set of resolved offsets (e.g. rows of heights 10, 30, 15)
  produces the correct real cumulative offset for each index and the
  correct real total extent; `set_virtual_list_window` with `Variable`
  extent positions materialized items at their own real, non-uniform
  offsets (not the old uniform formula); `scroll_virtual_list_by` with
  `Variable` extent clamps against the real non-uniform total extent,
  not `item_extent * item_count`. A `#[should_panic]` test proves
  querying an unresolved `Variable` index panics with a clear message,
  not a silent wrong answer.
- Existing `Fixed`-extent tests (materialization positioning, scroll
  clamping) must keep passing completely unmodified — the real
  regression check that this phase didn't change `Fixed`'s own behavior
  at all.
- `maturin develop --release` + `pytest tests/` + all examples — this
  phase is `engine-core`-only, so this is a pure regression check
  (nothing in `engine-py`/Python changes until Phase 2).
