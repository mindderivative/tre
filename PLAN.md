# Plan: M13 Phase 2 — Real Content Navigation (§11.2, optionally §7.6)

Corresponds to `BUILD_TRACKER.md` M13 Phase 2's own scoping, closing
the milestone: a real `Node.remove()`, plus a real example
demonstrating navigation between two screens, optionally composed with
the already-real container-transform choreography.

## Investigation before writing code

- Confirmed via grep (already done during M13's own scoping, re-
  confirmed by direct read of `node.rs`'s full `#[pymethods] impl`):
  `Node.add_child` exists; nothing exposes `Tree::remove` to Python at
  all. `Tree::remove` (`tree.rs:271-289`, confirmed by direct read)
  recursively removes `id` and its whole subtree, unlinking it from its
  own parent's `children` first — exactly "replacing `content`'s own
  children," the one missing half ARCHITECTURE.md §11.2's own text
  names ("navigating means replacing that node's children, an
  ordinary, already-supported tree mutation").
- `Node.add_child`'s own real shape (`node.rs:323-332`) is the direct
  template: `Rc::ptr_eq` same-tree check first (not needed here —
  `remove` only ever touches `self.id`, already known to belong to
  this `Node`'s own `Tree`), then a plain `Tree` method call.
- `Window.begin_container_transform`/`end_container_transform` (§7.6,
  M7 Phase 5) are already real, already Python-facing — no new wiring
  needed to compose them with a real remove+add navigation; this phase
  only needs to *demonstrate* the composition in a real example, not
  build new machinery for it.

## Design

`crates/engine-py/src/node.rs`:

- New `Node.remove(&self)` — calls `Tree::remove(self.id)`, mirroring
  `add_child`'s own minimal shape. No return value: `Tree::remove`
  itself returns `bool` ("was it actually present"), but a `Node`
  handle Python already holds always refers to a real, present
  `NodeId` at the point `.remove()` is called (the same assumption
  `add_child`/every other `Node` method already makes) — a bare `bool`
  return with no real failure case to report would be dead API surface,
  not real information.

## Verification plan

- `cargo test --workspace --release`/clippy/fmt — `engine-py`-only, no
  `engine-core` change (`Tree::remove` already exists and is already
  tested).
- `maturin develop --release` + `pytest tests/`. New `test_node.py` (or
  extending an existing file if a better-fitting one exists — check
  first) coverage: `Node.remove()` genuinely detaches a node from its
  own real parent (a sibling still added afterward doesn't collide/
  isn't confused with the removed node); removing a node, then clicking
  where it used to be, no longer fires its old handler (the real
  functional proof it's genuinely gone from the live tree, matching
  this project's established "clicking it proves it" discipline).
- New `examples/navigation.py`: a real `AppShell` (Phase 1) with two
  "screens" built into `content`, navigating between them by removing
  the current screen's children and adding the next, run live through
  real frames. Run all examples — confirm clean exit, no panic (the
  same live-rendering check that caught Phase 1's own real bug).
