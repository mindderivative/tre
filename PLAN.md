# Plan: M10 Phase 2 — Same-Tree Safety Checks for `set_context_menu`/`set_dock_handle` (§8)

Corresponds to `BUILD_TRACKER.md` M10 Phase 2's own scoping:
`Node.set_context_menu` and `dock::set_dock_handle` gain the same
`Rc::ptr_eq` same-tree guard `Node.add_child`/`Window.
begin_container_transform` already use.

## Investigation before writing code

- `Node.add_child`'s own real check (`node.rs`): `if !Rc::ptr_eq(&self.
  tree, &child.tree) { return Err(EngineError::ForeignNode.into()); }`
  — the exact template to mirror, confirmed by direct re-read.
- `Node.set_context_menu` (`node.rs`) currently has no check at all and
  returns plain `()`, not `PyResult<()>` — widening its return type to
  `PyResult<()>` is additive (Python already handles any method
  potentially raising; a caller that never triggers the new error path
  sees no behavior change).
- `dock::set_dock_handle` (`dock.rs`) is a free function taking raw
  `NodeId`s, not `PyRef<Node>` — it has no way to check same-tree
  itself. Its one real caller, `PyWindow.set_dock_handle` (`window.rs`)
  — the same shape `Window.begin_container_transform` already uses for
  its own two `Node` parameters — is where the check belongs, mirrored
  from there directly.
- Confirmed via grep: no Rust-level caller of either method anywhere in
  the codebase (only Python, via `tests/`/`examples/`) — widening both
  return types to `PyResult<()>` breaks nothing at the Rust level.

## Design

- `Node.set_context_menu(&self, content: PyRef<'_, Node>) -> PyResult<
  ()>` — checks `Rc::ptr_eq(&self.tree, &content.tree)` first, returns
  `EngineError::ForeignNode` if it fails, otherwise proceeds exactly as
  before.
- `PyWindow.set_dock_handle(&mut self, handle: PyRef<'_, Node>, panel:
  PyRef<'_, Node>) -> PyResult<()>` — checks `Rc::ptr_eq(&self.tree,
  &handle.tree)` and `Rc::ptr_eq(&self.tree, &panel.tree)`, returns
  `EngineError::ForeignNode` if either fails, otherwise calls `dock::
  set_dock_handle` exactly as before.

## Verification plan

- `cargo test --workspace --release`/clippy/fmt — this phase touches
  no `engine-core`, a pure `engine-py`-only widening.
- `maturin develop --release` + `pytest tests/` + all examples. New
  pytest tests: `Node.set_context_menu` with a `content` `Node` from a
  *different* `Window` raises a real `ValueError` naming the foreign
  node (matching `add_child`'s own real error message/type exactly);
  `Window.set_dock_handle` with either `handle` or `panel` from a
  different `Window` raises the same way. Existing tests (`test_
  context_menu.py`, `test_docking.py`) must keep passing unmodified —
  the real regression check that legitimate, same-window usage is
  untouched.
