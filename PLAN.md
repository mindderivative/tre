# Plan: M12 Phase 2 — Python-Facing Size-Hint Callback (§11.7)

Corresponds to `BUILD_TRACKER.md` M12 Phase 2's own scoping, closing
the milestone: `Window.add_virtual_list`/`set_virtual_list_window`
gain a real way to supply per-item heights from Python, wiring Phase
1's new `engine-core` capability (`ItemExtent::Variable`,
`VirtualListState.resolved_offsets`, `Tree::set_virtual_list_
resolved_offsets`) through to real app code.

## Investigation before writing code

- `PyWindow.add_virtual_list` (`window.rs:763-797`, confirmed by direct
  read): `item_extent: f64` is currently a required parameter; the
  materialized `NodeKind::VirtualList(VirtualListState::new(item_count,
  ItemExtent::Fixed(item_extent)))` is built directly from it.
- `PyWindow.set_virtual_list_window` (`window.rs:818-880`) reads
  `item_extent` a *second* time, for a genuinely different purpose than
  positioning: `Style.size.height = length(item_extent as f32)` for
  each newly-materialized item (line 855) — its real on-screen height,
  not its cumulative offset (which `Tree::set_virtual_list_window`
  itself now computes via `VirtualListState::offset_of`, Phase 1). Both
  values are needed for `Variable` mode, and they're different: offset
  is cumulative, height is per-item.
- All real call sites (`examples/scrollable_list.py`, every `tests/
  test_virtual_list*.py` call) use `item_extent=...`/`materialize=...`
  exclusively as keyword arguments, confirmed via grep — no positional
  call anywhere, so the Rust-side parameter order is free to change
  without breaking any real caller.
- `Tree::set_virtual_list_window`'s own closure parameter (`impl FnMut
  (usize) -> (NodeKind, Style, PaintProperties)`) is called *while*
  `Tree::set_virtual_list_window` itself holds `&mut self` — the
  closure built in `window.rs` cannot also borrow `tree` from inside
  itself (the exact self-referential-borrow conflict the *existing*
  code already avoids by extracting `item_extent` as a plain scalar
  copy *before* building the closure). The same shape is needed for
  `Variable` mode: whatever per-item height lookup the closure needs
  must be extracted as an owned snapshot before the closure is built,
  not read live from `tree` inside it.

## Design decision: eager, one-time resolution, not lazy/incremental

A real per-item height callback (`size_hint(idx) -> float`) is
fundamentally different in cost class from `materialize` (which builds
a real `Node` — paint, layout, potential child structure): it's a
plain, cheap arithmetic call. Computing a *cumulative* offset for any
item, though, structurally requires knowing every preceding item's own
height — there's no way around this for correct, non-estimated
positioning. Rather than building a stateful, incrementally-extended
lazy cache (real complexity — cache invalidation, partial-resolution
bookkeeping — for a milestone that ARCHITECTURE.md itself describes in
one line, "a size-hint callback for variable-height items", with no
further spec), this phase resolves **eagerly, once, at `add_virtual_
list` time**: when `size_hint` is given, it's called exactly
`item_count` times immediately, building the complete real cumulative-
offset table up front via `Tree::set_virtual_list_resolved_offsets`.

This is a real, deliberate, stated tradeoff, not a hidden cost:
`Fixed` still creates zero real `Node`s beyond the visible window
(§11.7's own core claim, untouched); `Variable` additionally pays one
real, one-time `O(item_count)` Python-call cost at list-creation time
to know the shape of the data — cheap per call (no `Node` creation),
but real for very large lists, and honestly documented as such, not
silently deferred into a later surprise.

## Design

`crates/engine-py/src/window.rs`:

- `add_virtual_list` gains a new optional `size_hint:
  Option<Py<PyAny>>` parameter alongside `item_extent`, which itself
  becomes `Option<f64>` (previously required). Exactly one of the two
  must be given — `PyValueError` otherwise (neither given, or both).
  When `size_hint` is given: calls it once per index `0..item_count`,
  accumulating a real running sum into a `Vec<(usize, f64)>` of
  cumulative offsets (index `0` maps to offset `0.0`; index
  `item_count`, one past the last item, maps to the real total
  extent) — a callback exception propagates as a real `PyErr`
  immediately (via `?`), the same "raises as a real error" contract
  `materialize` already has. Builds `NodeKind::VirtualList(VirtualList
  State::new(item_count, ItemExtent::Variable))`, then calls `Tree::
  set_virtual_list_resolved_offsets` with the resolved table before
  returning the new `Node`.
- `set_virtual_list_window`: extracts a small, local, private
  `enum ResolvedItemHeights { Fixed(f64), Variable(BTreeMap<usize,
  f64>) }` snapshot from the list's own current state *before*
  building the materializer closure (mirroring the existing scalar-
  extraction shape exactly, just widened to also cover `Variable`).
  Inside the closure, each newly-materialized item's own real height
  is `Fixed(v) => v`, or for `Variable(offsets)`, `offsets[idx + 1] -
  offsets[idx]` (both guaranteed present — Phase 2's own eager
  resolution always resolves every index up to and including
  `item_count`) — used for that item's own `Style.size.height`,
  replacing the old uniform `item_extent as f32`.
- Doc comments on both methods corrected — `add_virtual_list`'s own
  currently states "only `ItemExtent::Fixed` is exposed -- see `engine_
  core::ItemExtent`'s own doc comment for why the variable-height
  variant isn't built yet," stale once this phase ships.

## Verification plan

- `cargo test --workspace --release`/clippy/fmt — this phase is
  `engine-py`-only; `engine-core`'s own `ItemExtent::Variable`/
  `resolved_offsets`/`offset_of`/`total_extent`/`set_virtual_list_
  resolved_offsets` (Phase 1) are reused entirely as-is, no changes.
- `maturin develop --release` + `pytest tests/`. New `test_virtual_
  list.py` coverage: `add_virtual_list(size_hint=...)` builds a real
  `Variable` list whose materialized items land at their own real,
  non-uniform positions and heights (the same kind of "clicking it
  there proves it really moved" functional proof this file's sibling
  tests already use, adapted for position/size instead of click
  routing — checked via `Window.click()` on items at their own real,
  distinct computed centers, since there's no direct position getter);
  `size_hint` is called exactly `item_count` times, once each, at
  `add_virtual_list` time, not lazily per `set_virtual_list_window`
  call; passing neither or both of `item_extent`/`size_hint` raises a
  clear `ValueError`; a `size_hint` callback raising propagates as a
  real Python error, matching `materialize`'s own established
  contract. Existing `Fixed`-mode tests (every one already in `test_
  virtual_list.py`/`test_virtual_list_benchmark.py`) must keep passing
  completely unmodified — the real regression check that `item_extent=
  ...` (unchanged, just now `Optional` on the Rust side) still works
  exactly as before.
- Run all examples (`examples/scrollable_list.py` at minimum, still
  `Fixed`-mode, unmodified) — confirm clean exit, no panic.
- Consider adding a small new example demonstrating a real `Variable`
  list (e.g. rows of varying text-derived height) if it meaningfully
  proves the feature works live, matching this project's own pattern
  of pairing a new capability with a real, visible demonstration —
  decide once the FFI-level implementation and its pytest coverage are
  solid, not before.
