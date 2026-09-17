# Plan: M9 Phase 1 — Real Completion-Handle Plumbing Through the Central Tick

Corresponds to `BUILD_TRACKER.md` M9 Phase 1's own scoping: `Animated<T>
::tick` surfaces a just-completed `on_complete` handle, threaded
through `PaintProperties`/`InteractionState`/`RippleState` up to
`Tree::tick_all`, which returns the real set of handles that finished
this tick instead of just a bare `bool`; a new `animate_to_with_
completion` sibling method actually attaches a real handle, leaving
`animate_to` itself unchanged.

## Investigation before writing code

- `CompletionHandle(pub u64)` already derives `Clone, Copy, Debug,
  PartialEq, Eq, Hash` (confirmed by reading `animation.rs` directly)
  — cheap to copy out of a borrow, no new derive needed.
- `Animated::tick`'s current body already reads `anim.to.clone()`
  *before* `self.active = None` clears the borrowed `ActiveAnimation`
  away — confirmed this already compiles today, meaning NLL ends the
  `anim` borrow at its last real use, not at the end of the `if` block.
  Reading `anim.on_complete` (a `Copy` field) the same way, into a local
  before `self.active = None`, needs no new borrow-checker workaround.
- Full call-site enumeration via grep, so every one gets updated, none
  missed: `Animated::tick` is called from `PaintProperties::tick` (6
  fields), `InteractionState::tick` (`hover_opacity`/`focus_ring`) and
  `RippleState::tick` (`radius`/`opacity`, called via `SmallVec::
  retain`'s own `&mut T` closure), and `Tree::set_splitter_position`'s
  own single inline `state.position.tick(now)` call (kind-specific,
  ticked manually per M8 Phase 2's own confirmed finding that `Tree::
  tick_all` never reaches it) — 10 real call sites total, all inside
  `engine-core`.
- `Tree::tick_all`'s own real signature returns a bare `bool` today.
  Real callers that *bind* it need updating (`crates/engine-core/src/
  tree.rs`'s own tests, several `let still_active = tree.tick_all(...)`
  sites, confirmed via grep); callers that call it and discard the
  result (`engine-py::app.rs`'s own per-frame loop, `engine-py::
  view.rs`, most Rust pixel tests across `engine-render`/`engine-md3`)
  don't break on a richer return type, though `app.rs`'s own loop is
  exactly where Phase 2 will need to start consuming the new
  completions — out of this phase's own scope (engine-core only, no
  `engine-py` changes yet), but confirmed not to break by the signature
  change alone.
- `set_splitter_position`'s own inline `state.position.tick(now)` call
  needs a `completed` argument too, once `Animated::tick`'s own
  signature changes globally — its own real completions are discarded
  there (`SplitterState.position` is kind-specific, ticked outside
  `Tree::tick_all`'s own real completion-collection path, matching M8
  Phase 2's own precedent that kind-specific fields don't participate
  in whatever `Tree`-level central mechanism exists) — a real, stated
  scope boundary, not silently dropped functionality (nothing ever
  attaches `on_complete` to a splitter's own position animation today,
  confirmed via grep).
- `Animated::animate_to`'s own real signature (`to, duration, curve,
  now`) has no `on_complete` parameter — adding a *new*, additive
  sibling method (`animate_to_with_completion`) rather than adding a
  5th parameter avoids touching any of `animate_to`'s own ~15+ existing
  call sites across `engine-core`/`engine-md3`/`engine-py` (confirmed
  via grep), matching this codebase's own repeated "additive, not a
  breaking signature change" discipline.

## Design

- `Animated<T>::tick(&mut self, now: Instant, completed: &mut Vec<
  CompletionHandle>) -> bool` — on real completion, if `anim.
  on_complete` is `Some(handle)`, pushes it into `completed` before
  clearing `self.active`. Still-active and never-was-active paths are
  byte-for-byte unchanged aside from the new parameter simply not being
  touched.
- `Animated<T>::animate_to_with_completion(&mut self, to: T, duration:
  Duration, curve: MotionCurve, now: Instant, on_complete:
  CompletionHandle)` — identical body to `animate_to`, except `on_
  complete: Some(on_complete)` instead of `None`. `animate_to` itself
  stays untouched; the two share no code duplication risk worth a
  refactor at this size (four lines each).
- `PaintProperties::tick`/`InteractionState::tick`/`RippleState::tick`
  each gain the same `completed: &mut Vec<CompletionHandle>` parameter,
  threading it into every inner `.tick(now, completed)` call
  unconditionally — including fields no real caller has ever attached
  `on_complete` to yet (uniform plumbing, not selectively wired, so a
  future property gains real completion support for free the moment a
  caller starts using `animate_to_with_completion` on it).
- `Tree::tick_all(&mut self, now: Instant) -> (bool, Vec<
  CompletionHandle>)` — collects one shared `Vec` across the whole
  per-node walk, passing it into each node's `paint.tick`/`interaction.
  tick` calls, returning `(any_active, completed)`. `set_
  splitter_position`'s own inline tick call gets its own throwaway
  `Vec::new()` (discarded), matching the stated scope boundary above.

## Verification plan

- `cargo test --workspace --release`/clippy/fmt — every existing test
  that binds `tick_all`'s old bare-`bool` return updated to destructure
  the new tuple; tests that discard it need no change. New tests:
  `Animated::tick` reports a real `CompletionHandle` exactly on the
  tick where an animation genuinely finishes, never early and never a
  second time on a later tick that's already inactive; an animation
  with no `on_complete` (the plain `animate_to` path) reports nothing,
  a true no-op matching every prior test's own expectations; `Tree::
  tick_all` correctly aggregates completions from multiple nodes ticked
  in the same call.
- No `engine-py`/Python-facing API exists yet for this phase (Phase 2's
  own scope) — `maturin develop --release` + `pytest tests/` + all
  examples is a pure regression check, confirming the `Tree::tick_all`
  signature change compiles cleanly through `engine-py` even though
  nothing there uses the new return value yet.
