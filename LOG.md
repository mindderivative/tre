# Log: M10 Phase 2 — Same-Tree Safety Checks for `set_context_menu`/`set_dock_handle` (§8)

Corresponds to `BUILD_TRACKER.md` M10 Phase 2. `Node.set_context_menu`
and `dock::set_dock_handle` gain the same `Rc::ptr_eq` same-tree guard
`Node.add_child`/`Window.begin_container_transform` already use.

## Investigation before writing code

- `Node.add_child`'s own real check (`node.rs`) is the exact template
  mirrored: `if !Rc::ptr_eq(&self.tree, &child.tree) { return
  Err(EngineError::ForeignNode.into()); }`.
- `Node.set_context_menu` had no check at all and returned plain `()`
  — widening to `PyResult<()>` is additive; confirmed via grep no
  Rust-level caller exists anywhere (only Python), so nothing at the
  Rust level breaks.
- `dock::set_dock_handle` is a free function taking raw `NodeId`s, not
  `PyRef<Node>` — it has no way to check same-tree itself. Its one real
  caller, `PyWindow.set_dock_handle` (`window.rs`), is where the check
  belongs — the same shape `Window.begin_container_transform`'s own
  two-`Node`-parameter check already uses.

## What happened

`Node.set_context_menu(&self, content: PyRef<'_, Node>) -> PyResult<
()>` — checks `Rc::ptr_eq(&self.tree, &content.tree)` first, returns
`EngineError::ForeignNode` if it fails. `PyWindow.set_dock_handle(&mut
self, handle: PyRef<'_, Node>, panel: PyRef<'_, Node>) -> PyResult<
()>` — checks both `handle`/`panel` against `self.tree` the same way.

New pytest tests: `Node.set_context_menu` with a `content` `Node` from
a different `Window` raises `ValueError` naming the foreign node
(matching `add_child`'s own real error message exactly);
`Window.set_dock_handle` with either `handle` or `panel` from a
different `Window` raises the same way. Existing `test_context_
menu.py`/`test_docking.py` tests kept passing unmodified — the real
regression check that legitimate, same-window usage is untouched.

Full `cargo test --workspace --release` clean (this phase touches no
`engine-core` — a pure `engine-py`-only widening, unchanged Rust test
counts from Phase 1), `cargo clippy --workspace --all-targets -- -D
warnings`, `cargo fmt --check` all clean. `maturin develop --release`
+ full `pytest tests/` (87 passed, up from 85, 1 skipped) and all
sixteen examples confirmed clean.
