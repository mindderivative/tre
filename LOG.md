# Log: M12 Phase 1 — Real Variable-Height Extent Support in `engine-core` (§11.7)

Corresponds to `BUILD_TRACKER.md` M12 Phase 1. `ItemExtent` gains a
real way to represent variable, per-item extents; the two uniform-
spacing call sites in `tree.rs` generalize to real per-item cumulative
sums.

## Investigation before writing code

`ItemExtent` (`node.rs`) had exactly one variant, `Fixed(f64)`; its own
doc comment already stated the intent since M8: "fixed, or a size-hint
callback for variable-height items." Two real production call sites in
`tree.rs` assumed uniform spacing: `set_virtual_list_window`'s own
materialization positioning (`idx as f64 * item_extent`) and the M8
Phase 3 scroll-clamp's own total-extent computation (`item_extent.
value() * item_count`). `engine-py`'s own `set_virtual_list_window`
additionally used the scalar `item_extent` for each materialized
item's real `Style.size.height` -- a distinct value from cumulative
offset, and out of this phase's own scope (Phase 2's concern; `engine-
core` never sets item size, only position). `engine-render`'s scroll-
offset handling is a pure paint-time translation, entirely independent
of `item_extent` -- confirmed via direct read, so no `engine-render`
change was needed.

Real design question: how should `engine-core` represent "item N's
real cumulative offset" for `Variable` mode, given §4's pyo3-agnostic
boundary rules out a callback living here? Considered storing raw
per-item heights and summing on every query (rejected: real repeated
work, and computing item N's offset this way requires every preceding
index resolved, which `engine-core` has no way to request). Chose
storing already-*cumulative offsets* directly (`BTreeMap<usize, f64>`,
keyed by index) -- `engine-core` never sums anything, only looks a
resolved value up. The key `item_count` (one past the last real item)
holds the real total content extent.

## What happened

`ItemExtent` gains `Variable` (a fieldless marker -- the real per-item
data lives on `VirtualListState`, not the enum). `VirtualListState`
gains `resolved_offsets: BTreeMap<usize, f64>` and two new methods,
the single shared source of truth both `tree.rs` call sites now use:
`offset_of(idx)` (item `idx`'s own real top-offset -- `idx *
item_extent` for `Fixed`, a real resolved lookup for `Variable`,
panicking with a clear message if unresolved) and `total_extent()`
(`item_count * item_extent` for `Fixed`, `offset_of(item_count)` for
`Variable`). `ItemExtent::value()` (no longer meaningful once a second
variant exists) is removed; its two callers rewritten in terms of the
new methods. New `Tree::set_virtual_list_resolved_offsets` is the real
way a caller (Phase 2, `engine-py`) supplies resolved offsets --
`engine-core` itself never calls it.

`engine-py`'s own `set_virtual_list_window` needed one small fix to
keep compiling: its `let ItemExtent::Fixed(v) = state.item_extent;`
was an irrefutable pattern only because `ItemExtent` had exactly one
variant. Made exhaustive with an `unreachable!` arm for `Variable`,
honestly reflecting that `add_virtual_list` still only ever constructs
`Fixed` until Phase 2 adds real Python-facing `Variable` support.

New `engine-core` tests: `offset_of`/`total_extent` for `Fixed` match
the exact pre-existing uniform formula (regression guard); for
`Variable`, a real, non-uniform set of resolved offsets (heights 10,
30, 15, 25 -- deliberately not an arithmetic sequence, so a passing
test can't be an accident of `Fixed`-shaped math still secretly
running underneath) produces the correct real cumulative offset per
index and the correct real total extent; `set_virtual_list_window`
with `Variable` extent positions materialized items at their own real,
non-uniform offsets; `scroll_virtual_list_by` with `Variable` extent
clamps against the real non-uniform total extent. A `#[should_panic]`
test proves querying an unresolved `Variable` index panics with a
clear message, not a silent wrong answer.

Full `cargo test --workspace --release` (`engine-core` 86, up from 82),
`cargo clippy --workspace --all-targets -- -D warnings` (one real
`needless_range_loop` lint caught and fixed), `cargo fmt --check` all
clean -- every prior test passed unmodified. `maturin develop
--release` + full `pytest tests/` (96 passed, 1 skipped, unchanged)
and all sixteen examples confirmed clean -- a pure regression check,
since this phase is `engine-core`-only and changes no observable
Python-facing behavior yet.
