# Log: M9 Phase 1 — Real Completion-Handle Plumbing Through the Central Tick

Corresponds to `BUILD_TRACKER.md` M9 Phase 1. `Animated<T>::tick`
surfaces a just-completed `on_complete` handle, threaded through
`PaintProperties`/`InteractionState`/`RippleState` up to `Tree::
tick_all`, which now returns the real set of handles that finished each
tick instead of just a bare `bool`. A new `animate_to_with_completion`
sibling method actually attaches a real handle; `animate_to` itself is
unchanged.

## Investigation before writing code

- `CompletionHandle(pub u64)` already derives `Clone, Copy, Debug,
  PartialEq, Eq, Hash` — cheap to copy out of a borrow.
- `Animated::tick`'s existing body already reads `anim.to.clone()`
  before `self.active = None` clears the borrow away — confirmed this
  already compiles today, so reading `anim.on_complete` (also `Copy`)
  the same way needed no new borrow-checker workaround.
- Full call-site enumeration via grep before editing: `Animated::tick`
  is called from `PaintProperties::tick` (6 fields), `InteractionState
  ::tick` (`hover_opacity`/`focus_ring`), `RippleState::tick`
  (`radius`/`opacity`, via `SmallVec::retain`'s `&mut T` closure), and
  `Tree::set_splitter_position`'s own inline call — 10 real call sites,
  all inside `engine-core`, none missed.
- `Tree::tick_all`'s own real callers that bind the old bare-`bool`
  return needed updating (`engine-core::tree`'s own tests); callers
  that discard it (`engine-py::app.rs`'s per-frame loop, `view.rs`,
  most Rust pixel tests) compiled unchanged.
- `Animated::animate_to`'s own real signature has no `on_complete`
  parameter — added a new, additive sibling method instead of a 5th
  parameter, so none of its ~15+ existing call sites needed touching.

## What happened

`Animated<T>::tick(&mut self, now, completed: &mut Vec<CompletionHandle
>) -> bool` — pushes `anim.on_complete` into `completed` on the exact
tick an animation finishes, before `self.active` is cleared. New
`Animated<T>::animate_to_with_completion(to, duration, curve, now,
on_complete)` — identical to `animate_to` except it actually sets
`on_complete: Some(...)`.

`PaintProperties::tick`/`InteractionState::tick`/`RippleState::tick`
each gained the same `completed` parameter, threaded uniformly into
every inner `.tick` call. `Tree::tick_all` now returns `(bool, Vec<
CompletionHandle>)`, collecting one shared `Vec` across the whole
per-node walk. `set_splitter_position`'s own inline tick call gets a
throwaway `Vec::new()` — kind-specific fields stay outside the central
completion queue, the same M8 Phase 2 precedent already established for
`Tree::tick_all` itself.

New tests: `animate_to_with_completion` reports its handle exactly on
the completing tick, never early, never twice on a later tick; plain
`animate_to` never reports a completion (a true no-op for the existing,
unchanged default path); `InteractionState::tick` genuinely threads a
real completion up from `hover_opacity`. Every existing test that bound
`tick_all`'s old return value updated to destructure the new tuple (2
sites in `tree.rs`); two downstream `engine-render` tests
(`animated_rect.rs`, `transform_composition.rs`) updated for
`Animated::tick`'s new signature.

Full `cargo test --workspace --release` clean (`engine-core` gains 3
new tests: 69 → 72 — every prior test passed unmodified), `cargo
clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`
all clean. This phase adds no `engine-py`/Python-facing API — `maturin
develop --release` + full `pytest tests/` (79 passed, 1 skipped,
unaffected) and all fifteen examples confirmed clean, a pure regression
check proving the `Tree::tick_all` signature change compiles cleanly
through `engine-py` even though nothing there consumes the new return
value yet.
