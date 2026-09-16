# Plan: M6 Phase 1 — `Node.add_child` (real cycle rejection)

## Context

`ARCHITECTURE.md` §8 sketches this exactly: `fn add_child(&self, child:
&PyNode) -> PyResult<()>` ("rejects a child that is an ancestor of self
with a `PyValueError`, not a silent cycle") and an `EngineError::
CycleRejected` variant with the message "cannot add a node as a child
of its own descendant." Confirmed directly: `engine-core::Tree::
add_child` has no cycle check at all today, and `engine-py::Node` has
no `add_child` method at all — the gap named when M6 was scoped.

## Investigation before writing code

- **`Tree::add_child`'s own call sites (~80, via grep) all treat it as
  infallible.** Changing its signature would ripple through every
  internal caller — `engine-core`'s own logic, every pixel-readback
  test, `engine-spec::reconcile`/`build`, `engine-py::window.rs` — none
  of which need checking (each already knows structurally it can't form
  a cycle: attaching a freshly-inserted node, or a docking/reconcile
  reparent already proven disjoint). **Scope narrowed, correctly:** a
  new, separate `Tree::try_add_child` is the checked entry point for the
  one caller that genuinely can't make that guarantee — Python's own
  `Node.add_child` — leaving `add_child` and its ~80 existing callers
  completely untouched.
- **A cycle is exactly: `child` is `parent` itself, or `child` is
  already an ancestor of `parent`.** Detected by walking up from
  `parent` via `Node::parent` links (the same walk `absolute_position`
  already uses) and checking whether `child` appears in that chain —
  `parent == child` is caught for free as the walk's very first
  iteration.
- **A second, real, closely-related corruption risk found by reading
  `add_child` closely, not assumed:** it has no dedup — attaching an
  already-attached `child` under a new `parent` pushes a second parent
  pointer into `taffy`/`children` without first detaching the old one,
  the *exact* "`add_child` has no dedup" bug class M4 Phase 7 (overlay)
  and M4 Phase 9 (docking) already each found and fixed once, for their
  own specific callers. `Node.add_child` is the first *general-purpose*,
  arbitrary-reparenting entry point — the single call site most likely
  to trigger this a third time (a real app moving a node from one
  container to another). Fixed the same way both prior instances were:
  detach from the current parent first (`Tree::detach`, reusing the
  existing mechanism, no new one) if `child` already has one.
- **A third, real risk found before writing any Python code:** `NodeId`
  is a `slotmap` generational key, unique only *within* the `Tree` that
  minted it — confirmed directly, `slotmap` gives no cross-map identity
  guarantee. `Node.add_child(child)` is the first Python-facing method
  that takes *another Node* as a mutation target for a real tree
  structural change (`set_context_menu`/`set_dock_handle` already take
  a second `Node` too, but only ever store its id in a side map — never
  hand it to `taffy`). If `self`/`child` belong to different `Window`s
  (different `Tree`s), `child`'s `NodeId` could alias an unrelated real
  node in `self`'s own `Tree`, and handing a foreign `taffy::NodeId` to
  this `Tree`'s own `taffy::TaffyTree` risks corrupting or panicking
  it — worse than a merely-wrong id. `set_context_menu`/`set_dock_handle`
  don't guard against this today (confirmed via grep, no `Rc::ptr_eq`
  anywhere in the codebase) — a real, pre-existing gap, but not this
  phase's to fix retroactively. For `add_child` specifically, the real
  `taffy`-corruption risk justifies a real, minimal guard: `engine-py`
  already holds `Rc<RefCell<Tree>>` per `Node` (cloned from its owning
  `Window`), so `Rc::ptr_eq(&self.tree, &child.tree)` is a cheap,
  already-available check, made *before* either `Tree` is ever touched.

## Approach

1. **`engine-core/src/tree.rs`**: new `Tree::try_add_child(&mut self,
   parent: NodeId, child: NodeId) -> bool` — walks up from `parent`
   checking for `child`; on a cycle, returns `false`, no mutation. On no
   cycle: detaches `child` from its current parent if `Some`, then calls
   the existing `add_child(parent, child)`, returns `true`. `add_child`
   itself is untouched.
2. **`engine-py/src/error.rs`**: `EngineError::CycleRejected` (verbatim
   message from `ARCHITECTURE.md` §8) and `EngineError::ForeignNode`
   ("this Node belongs to a different Window's Tree" — the real,
   `Rc::ptr_eq`-detected cross-tree case above), both to `PyValueError`.
3. **`engine-py/src/node.rs`**: `Node.add_child(&self, child: PyRef<'_,
   Node>) -> PyResult<()>` — checks `Rc::ptr_eq` first, then calls
   `try_add_child`, translating `false` into `CycleRejected`.
4. **New tests**: `engine-core` unit tests (self-cycle, a real multi-
   level ancestor cycle, and the "already attached elsewhere" auto-
   detach case, proving the *tree* — not just the return value — ends
   up correct). New `tests/test_add_child.py`: a real attach, a rejected
   self/ancestor cycle raises `ValueError`, a cross-window `add_child`
   raises `ValueError`, and re-parenting an already-attached node moves
   it (not duplicates it).

## Files to touch

- `crates/engine-core/src/tree.rs` — `try_add_child` + tests.
- `crates/engine-py/src/error.rs` — `CycleRejected`/`ForeignNode`.
- `crates/engine-py/src/node.rs` — `Node.add_child`.
- `tests/test_add_child.py` — new.

## Verification

- `cargo test --workspace`, `cargo clippy --workspace --all-targets --
  -D warnings`, `cargo fmt --check`.
- `maturin develop && python -m pytest tests/ -v` plus all examples run.
