# Log: M6 Phase 1 — `Node.add_child` (real cycle rejection)

Corresponds to `BUILD_TRACKER.md` M6 Phase 1. `ARCHITECTURE.md` §8
sketches this exactly: `fn add_child(&self, child: &PyNode) ->
PyResult<()>` rejecting an ancestor-as-child cycle with a
`PyValueError`, and `EngineError::CycleRejected`. This phase makes it
real.

## Investigation before writing code

- **`Tree::add_child`'s ~80 existing call sites all treat it as
  infallible** (confirmed via grep) — every one of them already knows
  structurally it can't form a cycle. Changing its signature would have
  rippled through all of them for zero real benefit. **Scope narrowed,
  correctly:** a new, separate `Tree::try_add_child` is the checked
  entry point for the one caller that genuinely can't make that
  guarantee — Python's own `Node.add_child` — leaving `add_child` and
  every existing caller completely untouched.
- **A second, real, closely-related corruption risk found by reading
  `add_child` closely, not assumed:** it has no dedup. Attaching an
  already-attached `child` under a new `parent` pushes a second parent
  pointer without detaching the old one first — the exact "`add_child`
  has no dedup" bug class M4 Phase 7 (overlay) and M4 Phase 9 (docking)
  each already found and fixed once, for their own specific caller.
  `Node.add_child` is the first *general-purpose* reparenting entry
  point, and the one most likely to hit it a third time (a real app
  moving a node between containers). Fixed the same way both prior
  instances were: detach from the current parent first (`Tree::detach`,
  reusing the existing mechanism, not a new one).
- **A third, real risk found before writing any Python code:** `NodeId`
  is a `slotmap` generational key, unique only *within* the `Tree` that
  minted it — confirmed directly, `slotmap` gives no cross-map identity
  guarantee. `Node.add_child` is the first Python-facing method that
  hands another node's id to `taffy` for a real structural mutation
  (`set_context_menu`/`set_dock_handle` also take a second `Node`, but
  only ever store its id in a side map, never touch `taffy` with it).
  A cross-`Window` call could alias an unrelated real node in the
  wrong `Tree` and hand a foreign `taffy::NodeId` to this `Tree`'s own
  `taffy::TaffyTree` — real corruption risk, not just a wrong result.
  `set_context_menu`/`set_dock_handle` don't guard against this today
  (confirmed via grep, no `Rc::ptr_eq` anywhere in the codebase) — a
  real, pre-existing gap, not this phase's to fix retroactively. For
  `add_child` specifically, the risk justified a real, minimal guard:
  `Rc::ptr_eq(&self.tree, &child.tree)`, checked before either `Tree`
  is ever touched.

## What happened

`engine-core/src/tree.rs`: new `Tree::try_add_child(&mut self, parent,
child) -> bool` — walks up from `parent` via `Node::parent` links
(the same walk `absolute_position` already uses) checking for `child`;
rejects (returns `false`, no mutation) on a cycle. On no cycle: detaches
`child` from its current parent if `Some` (`Tree::detach`), then calls
the existing, untouched `add_child`.

`engine-py/src/error.rs`: `EngineError::CycleRejected` (message
verbatim from §8's own sketch) and `EngineError::ForeignNode` (the
cross-`Window` case above), both to `PyValueError`.

`engine-py/src/node.rs`: `Node.add_child(&self, child: PyRef<'_,
Node>) -> PyResult<()>` — `Rc::ptr_eq` check first, then
`try_add_child`, translating a rejection into `CycleRejected`.

Three new `engine-core` unit tests (self-cycle, real multi-level
ancestor cycle, re-parenting an already-attached node moves it rather
than duplicating it) — all passed on the first run, each asserting the
*tree* ended up correct, not just the return value. New
`tests/test_add_child.py` (5 tests): a real attach, both cycle shapes,
re-parenting, and the cross-`Window` rejection — all passed on the
first run.

Full `cargo test --workspace --release` clean (`engine-core` 50 tests,
up from 47), `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --check` all clean. `maturin develop --release` + full
`pytest tests/` (73 passed, up from 68, 1 skipped) and all eight
examples confirmed clean.
