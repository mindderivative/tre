# Log: M12 Phase 2 — Python-Facing Size-Hint Callback (§11.7)

Corresponds to `BUILD_TRACKER.md` M12 Phase 2, closing M12 entirely.
`Window.add_virtual_list`/`set_virtual_list_window` gain a real way to
supply per-item heights from Python, wiring Phase 1's new `engine-core`
capability through to real app code.

## Investigation before writing code

`PyWindow.set_virtual_list_window` reads `item_extent` for a *second*,
distinct purpose beyond positioning (which `Tree::set_virtual_list_
window` itself now handles via `VirtualListState::offset_of`, Phase
1): each materialized item's own `Style.size.height`. Both values are
needed for `Variable` mode and are genuinely different (offset is
cumulative, height is per-item). All real call sites use keyword
arguments exclusively, confirmed via grep, so the Rust-side parameter
order was free to change. `Tree::set_virtual_list_window`'s own
closure runs while `&mut self`'s `Tree` is already held, so a per-item
height lookup has to be extracted as an owned snapshot before the
closure is built -- the same shape the pre-existing code already used
for its scalar `item_extent`.

## Design decision: eager, one-time resolution

A real per-item height callback is cheap (plain arithmetic, no `Node`
creation) but a real cumulative offset structurally requires knowing
every preceding item's own height. Rather than a stateful,
incrementally-extended lazy cache (real complexity ARCHITECTURE.md's
own one-line "size-hint callback" text doesn't ask for), this phase
resolves **eagerly, once, at `add_virtual_list` time**: `size_hint` is
called exactly `item_count` times immediately, building the complete
cumulative-offset table via `Tree::set_virtual_list_resolved_offsets`
before `add_virtual_list` ever returns -- a real, deliberate, stated
`O(item_count)` cost `Fixed`'s own zero-per-item-cost path never pays,
documented as such, not hidden.

## What happened

`add_virtual_list` gains `size_hint: Option<Py<PyAny>>`; `item_extent`
becomes `Option<f64>` (previously required). Exactly one of the two
must be given -- a real `PyValueError` otherwise (neither, or both).
When `size_hint` is given, it's resolved eagerly as described above,
building `ItemExtent::Variable` + the resolved cumulative-offset table.
`set_virtual_list_window` extracts a new, small, private `enum
ResolvedItemHeights { Fixed(f64), Variable(BTreeMap<usize, f64>) }`
snapshot before building its materializer closure; each item's own
real height for `Variable` is the difference between two adjacent
resolved cumulative offsets (both guaranteed present -- Phase 2's own
eager resolution always resolves every index through `item_count`).

New pytest coverage (`test_virtual_list.py`): `add_virtual_list(size_
hint=...)` returns a real `Node`; `size_hint` is called exactly once
per item, in order, at `add_virtual_list` time -- and *not* re-invoked
by a later `set_virtual_list_window` call (proven via call-tracking,
isolating exactly what each method itself contributes); passing
neither or both of `item_extent`/`size_hint` raises a clear
`ValueError`; a `size_hint` exception propagates as a real Python
error, matching `materialize`'s own established contract. The
definitive proof that resulting positions/heights are genuinely
non-uniform is Phase 1's own `engine-core` tests -- matching this
file's established "FFI wiring only" split; there's no direct
position/size getter on `Node` from Python for any list, `Fixed` or
`Variable`. Existing `Fixed`-mode tests all kept passing unmodified.

New `examples/variable_height_list.py`: a real, live 500-row list with
a genuinely non-uniform, repeating five-step row-height pattern,
scrolled for real -- pairing this phase's new capability with a real,
visible demonstration, matching this project's own established pattern
(`theme.py`, `animation_completion.py`, etc.).

Full `cargo test --workspace --release` (no `engine-core` change this
phase, `engine-py`-only)/`cargo clippy --workspace --all-targets -- -D
warnings` (one real `too_many_arguments` lint on the now-8-parameter
`add_virtual_list`, resolved with a scoped, justified `#[allow]` --
every real caller uses keywords exclusively, so a struct wouldn't
meaningfully improve ergonomics)/`cargo fmt --check` all clean.
`maturin develop --release` + full `pytest tests/` (100 passed, up
from 96, 1 skipped) and all seventeen examples (sixteen existing +
new `variable_height_list.py`) confirmed clean.

M12 (Variable-Height VirtualList Items) is now complete: both phases
done -- Phase 1 built the real `engine-core` primitive, Phase 2 wired
it through to real, working Python code with a real, non-uniform live
demonstration.
