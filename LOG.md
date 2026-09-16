# Log: M4 Phase 9 (final M4 phase) — Docking Drag-to-Rearrange (§11.4)

Corresponds to `BUILD_TRACKER.md` M4 Phase 9 — the last M4 phase.
`DockLayout`/`DockZone`/`Tree::apply_active_tab` are real since M3 step
15 Stage B, but moving a whole panel *between* zones (not resizing,
not switching a zone's own active tab) had never been wired to
anything, and no Python-facing docking API existed at all.

## Investigation before writing code

Re-read §11.4 in full, then read `dock.rs`, `Tree::apply_active_tab`,
and `docking.rs`'s own 3-zone pixel test end to end before designing
anything.

- **No Python-facing docking API exists at all** — confirmed via grep,
  the same class of prerequisite gap M4 Phase 7 found for overlays.
- **Docking has no dedicated `NodeKind`.** A "zone" is an ordinary node
  the app builds itself; `DockLayout` is pure external bookkeeping
  `Tree` never stores. Which nodes are "handles"/"panels"/"zones" are
  exactly the meaning-dependent facts §2 Design Principle 6 says
  `engine-core` has no business knowing — confirmed this phase needed
  *zero* new `engine-core` code: `Tree::hit_test`, `Tree::
  apply_active_tab`, and the already-`pub` `Node::parent` field are
  exactly what's needed. The whole drag-to-rearrange orchestration
  lives in a new `engine-py` module.
- **Real bug found by reading `apply_active_tab` closely, before
  writing anything:** it only checks whether the *target* container
  already lists the active panel as a child — never whether the panel
  is still attached elsewhere first. Calling it naively while moving a
  panel between zones would `add_child` an already-attached node,
  corrupting the tree — the exact "`add_child` has no dedup" class of
  bug M4 Phase 7 already found once for `open_overlay`.
- **`open_overlay` can't support a drop-zone highlight.** Its `inset`
  is hardcoded to place content *below* its anchor (a dropdown-menu
  placement, confirmed by reading the actual computation) — it can't
  cover a target zone's own bounds. Real, additive `engine-core` work
  to add that placement mode would be disproportionate to this phase's
  actual core claim. **Scope narrowed: no highlight overlay** — named
  explicitly, not silently dropped. One real consequence: nothing needs
  to track *which* zone is under the pointer mid-drag, only the drop
  point once at release — no `PointerMoved`-time callback for docking
  at all, just press (start) and release (end).

## What happened

New `crates/engine-py/src/dock.rs`: `DockState { layout: DockLayout,
containers: Vec<(DockSide, NodeId)>, handles: HashMap<NodeId, NodeId>,
dragging: Option<NodeId> }`, shared as `Rc<RefCell<DockState>>` on
`PyWindow` (mirrors `handlers`/`context_menus`'s own sharing shape — no
`Py<PyAny>` involved, so no GC-traversal obligation either).
`parse_dock_side` mirrors `press_key`'s own string-vocabulary pattern.

New `Window` methods: `add_dock_zone(side, container, size)`,
`dock_panel(side, panel)` (real initial setup, reused verbatim by the
drag mechanism's own "attach into the new zone" step), `set_active_tab
(side, index)`, `set_dock_handle(handle, panel)`, and the real
no-live-window-needed test entry points `start_panel_drag(handle)`/
`drop_panel_at(x, y)`, mirroring `.click()`/`.hover()`/`.right_click()`
exactly. Real `winit` wiring: `app.rs`'s `on_input` closure inspects
the *raw* `InputEvent` directly (not `DispatchOutcome` — "which node is
a handle" isn't something `Tree::dispatch` has any reason to report)
and calls the same `start_drag`/`end_drag_at` on a real primary-button
press/release.

## Real bug found and fixed by actually running the example, not just pytest

`examples/docking.py`'s first run panicked immediately at `app.run()`:
`"TreeUpdate includes duplicate child #..."` — a real `accesskit`
validation failure. Root cause: `dock_panel`'s initial setup called
`apply_active_tab` without first detaching the panel from its existing
parent (`add_rect` attaches every new node to the window's root
immediately) — the exact bug class named above, just not yet applied
to `dock_panel` itself when it was first written. No pytest test
caught this, since none of them render a real frame or call
`build_access_update` — only the live example actually exercises
`accesskit`'s own tree validation. Fixed by detaching the panel from
its current parent first, guarded by the same containment check
`apply_active_tab` itself already uses. A concrete reminder of why this
project's own discipline runs real examples, not just unit/pytest
suites, before calling a feature done.

## Verification

7 new pytest tests in `test_docking.py`: a real drag moves a panel
between zones (proven functionally — clicking the panel in its new
zone fires its own handler, which only works if it's really attached
and laid out); starting a drag on an unregistered node is a safe
no-op; dropping outside any zone cancels without moving anything;
dropping back into the same zone is a no-op; releasing with no drag in
progress doesn't raise; `set_active_tab` really switches which panel is
attached; an unknown dock side raises `ValueError`. All passed after
the one real fix above. New `examples/docking.py`, matching the
established per-phase pattern and honestly stating what needs a human.

```
$ cargo build --workspace                                    # clean
$ cargo clippy --workspace --all-targets -- -D warnings       # clean
$ cargo fmt --check                                           # clean
$ cargo test --workspace                                      # all green, unchanged (no new engine-core code)

$ maturin develop
$ python -m pytest tests/ -v
60 passed, 1 skipped (the TRE_RUN_BENCHMARK-gated benchmark)

$ python examples/*.py    # all six exit cleanly, including the newly fixed docking.py
```

## M4 is now complete

All 9 phases done. Real pointer/keyboard `InputEvent` dispatch,
hit-testing, assistive-technology action dispatch, splitter-drag,
`View`'s declarative handlers, real ripple/hover, `EventKind` +
`HoverEnter`/`HoverExit`, right-click context menus, scroll-wheel
plumbing, and docking drag-to-rearrange are all real, tested, and wired
end to end. Real, stated-not-silent gaps carried forward (not this
phase's to close): overlay dismissal (`dismiss_on_outside_click`/
`dismiss_on_escape`), no drop-zone highlight visual, scroll input not
yet wired to `VirtualList`'s still-missing real scrollable viewport,
`AppHandler` remains confirmed dead code. Milestone 5 (transform
composition, `NodeKind::Canvas`, custom hit-testing) remains scoped and
untouched.
