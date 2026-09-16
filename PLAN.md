# Plan: M4 Phase 9 (final M4 phase) — Docking Drag-to-Rearrange (§11.4)

## Context

`DockLayout`/`DockZone`/`Tree::apply_active_tab` are real since M3 step
15 Stage B (proven by `crates/engine-render/tests/docking.rs`'s own
3-zone pixel test), but moving a whole panel *between* zones — as
opposed to resizing (M4 Phase 3) or switching a zone's active tab
(already real) — has never been wired to anything, and no Python-facing
docking API exists at all yet.

## Investigation before writing code

Re-read §11.4 in full, then read `dock.rs`, `Tree::apply_active_tab`,
and `docking.rs`'s own test end to end before designing anything.

- **No Python-facing docking API exists at all** — confirmed via grep:
  `DockLayout`/`DockZone`/`apply_active_tab` are referenced only in one
  doc comment across `engine-py`, never actually wrapped. Like M4 Phase
  7's own overlay discovery, this phase's real prerequisite is bigger
  than "wire drag-to-rearrange" alone.
- **Docking has no dedicated `NodeKind`.** A "zone" is just an ordinary
  `Rect`/`Container` node the app builds itself; "panels" are ordinary
  content nodes; `DockLayout`/`DockZone` are pure external bookkeeping
  the *caller* owns and passes into `apply_active_tab` by reference —
  `Tree` itself never stores a `DockLayout`. This means "which node is
  a panel drag handle" and "which node is a zone" are exactly the kind
  of meaning-dependent facts Design Principle 6 says `engine-core` has
  no business knowing — the whole drag-to-rearrange orchestration
  belongs in `engine-py`, not `engine-core`. Confirmed this needs zero
  new `engine-core` code: `Tree::hit_test`, `Tree::apply_active_tab`,
  and the already-`pub` `Node::parent` field are exactly what's needed.
- **Real bug found by reading `apply_active_tab` closely before
  writing anything:** it only checks whether the *target* container
  already lists the active panel as a child — it never checks whether
  the panel is still attached to a *different* (e.g. old) parent
  first. Calling it naively while moving a panel between zones would
  call `add_child` while the panel is still attached elsewhere,
  corrupting the tree — the exact same "`add_child` has no dedup" class
  of bug M4 Phase 7 already found once for `open_overlay`. The real
  fix: explicitly `Tree::detach` the panel from its *old* zone's
  container first, guarded by the same containment check
  `apply_active_tab` itself already uses, before ever touching the new
  zone.
- **`open_overlay`'s real positioning can't support a drop-zone
  highlight.** Its `inset` is hardcoded to place content *below* its
  anchor (a dropdown-menu placement) — confirmed by reading the actual
  `inset` computation, not assumed. A drop-zone highlight needs to
  cover the *target zone's own bounds* exactly, a different placement
  mode `open_overlay` doesn't offer today. Real, additive engine-core
  work to add that would be disproportionate to this phase's actual
  core claim. **Scope narrowed: no highlight overlay this phase** —
  the real, functionally complete deliverable is "a real drag gesture
  provably moves a panel from one zone to another"; the visual
  highlight during the drag is a separate UX nicety, not manufactured
  here. Named explicitly, not silently dropped.
- **Consequence of skipping the highlight:** nothing needs to track
  *which* zone is under the pointer while the drag is in progress —
  only the drop *point*, once, at release. This drops any need for a
  `PointerMoved`-time callback for docking at all, keeping the real
  mechanism to exactly two moments: press (on a registered handle,
  starts the drag) and release (hit-tests the drop point, finds the
  enclosing zone by walking `Node::parent` links, and reparents).

## Approach

1. **New `engine-py/src/dock.rs`**: `DockState { layout: DockLayout,
   containers: Vec<(DockSide, NodeId)>, handles: HashMap<NodeId,
   NodeId>, dragging: Option<NodeId> }`, shared as `Rc<RefCell<
   DockState>>` on `PyWindow` (mirrors `handlers`/`context_menus`'
   sharing shape). `parse_dock_side(&str)` mirrors `press_key`'s own
   string-vocabulary pattern (`"left"/"right"/"top"/"bottom"/
   "center"`).
2. **`Window` methods**: `add_dock_zone(side, container, size)`,
   `dock_panel(side, panel)` (real initial setup — records the panel
   and calls `apply_active_tab`), `set_active_tab(side, index)`,
   `set_dock_handle(handle, panel)`.
3. **Shared drag orchestration** (module-private, used by both the
   real `winit`-driven path and the Python test entry points): `fn
   start_drag(dock, node_id) -> bool` (registers `dragging` if `node_id`
   is a known handle); `fn end_drag_at(dock, tree, root, position)`
   (hit-tests `position`, walks `Node::parent` to find an enclosing
   registered zone, and — if it's a real, different-from-source target
   — detaches the panel from its old zone's container first (the real
   fix above), fixes up the old zone's `active_tab`, reparents into the
   new zone via `apply_active_tab`; always clears `dragging`).
4. **Test entry points**: `Window.start_panel_drag(handle)`/
   `Window.drop_panel_at(x, y)`, the same no-live-window-needed pattern
   `.click()`/`.hover()`/`.right_click()` already established.
5. **Real `winit` wiring**: `app.rs`'s `on_input` closure additionally
   calls `start_drag` on a real `PointerPressed(Primary)` hit and
   `end_drag_at` on a real `PointerReleased(Primary)`, alongside (not
   instead of) the existing `run_dispatch_outcome`/`open_context_menu`
   calls on the same raw event — mirroring exactly how those two
   already coexist on one event today.

## Files to touch

- `crates/engine-py/src/dock.rs` — new.
- `crates/engine-py/src/window.rs` — `dock` field, new methods,
  `__traverse__`/`__clear__` unaffected (no `Py<PyAny>` involved, same
  reasoning as `context_menus`).
- `crates/engine-py/src/app.rs` — `WindowSetup`/`WindowRuntime` gain
  `dock`, `on_input` closure wired.
- `crates/engine-py/src/lib.rs` — register the new module.
- `tests/test_docking.py` — new.
- `examples/docking.py` — new, matching the established per-phase
  example pattern.

## Verification

- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo fmt --check` (no `engine-core` changes expected, so
  no new Rust unit tests this phase beyond what already exists).
- `maturin develop && python -m pytest tests/ -v` plus all examples run.
