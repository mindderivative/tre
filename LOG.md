# Log: M27 Phase 1 — Shell & Navigation Scaffold

New `demo/showcase.py` (a fresh top-level location, not `examples/` --
`examples/` is explicitly "one real mechanism" per script; this is the
opposite, a consolidated multi-screen app growing one real screen per
phase). `Window.build_shell(menu_bar=...)` for the chrome; a persistent
left nav rail and a screen area both attached to `content`, laying out
side by side for free (`content`'s own `Style::default()` is a real
`Flex Row` in taffy 0.14.0 -- confirmed directly in its source before
relying on it, not assumed). Navigation reuses `examples/navigation.py`'s
own already-proven remove-old/build-new screen-swap pattern verbatim,
not a new mechanism. Phase 1's own two screens are placeholders --
Phases 2-4 replace them with real content via the same `SCREENS`
registry.

**Two real, connected bugs found only by actually running the demo end
to end, not assumed -- the same discipline this project holds itself
to throughout:**

1. **Handler arity.** `Node.set_on_click`/etc. call the registered
   Python callback with **zero** arguments (confirmed in `dispatch.rs`),
   but the first draft's nav-button handler was `lambda e, k=key: ...`
   -- `TypeError: missing 1 required positional argument`, caught and
   logged non-fatally by the engine's own "an uncaught handler
   exception is non-fatal" policy, so it failed *silently* rather than
   crashing. Fixed the demo's own handler. **Found the identical real
   bug already shipped in the docs**: `docs/getting-started.md`'s own
   `on_click(event):`, `docs/guide/components.md`'s two `lambda e: ...`,
   and `docs/guide/imperative-api.md`'s four `lambda event: ...` --
   every one would silently no-op for a real reader who copy-pasted
   them. Reproduced the exact failure with a standalone script before
   fixing, then reproduced success after. Fixed all six.

2. **A genuine, structural `&mut self` over-restriction in `engine-py::
   PyWindow`.** `Window.click`/`hover`/`scroll`/`right_click`/
   `press_key`/`type_text`/`cut`/`paste`/`begin_container_transform`/
   `end_container_transform`/`redraw_canvas`/`set_virtual_list_window`
   and every real docking method (`add_dock_zone`/`dock_panel`/
   `set_active_tab`/`set_dock_handle`/`set_drop_zone_highlight`/
   `drag_panel_over`/`start_panel_drag`/`drop_panel_at`) were all
   declared `&mut self` in Rust despite every one of them touching only
   interior-mutable state (`Rc<RefCell<Tree>>`/`Rc<RefCell<DockState>>`/
   read-only fields) -- none of them ever needed exclusive access,
   confirmed by reading each body directly before changing anything.
   PyO3 enforces Rust's aliasing rules on `#[pyclass]` instances at
   runtime: a `&mut self` method holds an exclusive borrow on the
   Python object for its whole call, including while it synchronously
   invokes a registered Python callback -- so a `window.click()`-
   triggered handler that itself called back into the *same* `window`
   object (e.g. `window.add_rect(...)`, a completely ordinary,
   realistic pattern any real click-driven UI needs) panicked with
   `RuntimeError: Already mutably borrowed`. Converted all 19 real
   methods to `&self` (a mechanical, scripted, verified-per-method
   change -- confirmed via direct read that none of them write to the
   two fields that genuinely still need `&mut self`: `materializers`/
   `canvas_draws`, both plain `HashMap`s `add_virtual_list`/`add_canvas`
   insert into, the *only* two `PyWindow` fields not already behind a
   `RefCell`). `add_virtual_list`/`add_canvas` themselves correctly stay
   `&mut self` -- a real, honestly-stated, narrower residual limitation:
   a handler still can't call those two specifically from within
   another `PyWindow` method's own call stack, which would need those
   two fields moved behind their own `RefCell` too, real, separate,
   larger work not needed by anything today.

Full `cargo test --workspace --release` (144 `engine-core` + every
other crate's suite, all unmodified and passing -- widening `&self`
can't break a test that never relied on exclusivity)/clippy
`-D warnings`/fmt clean on the first run after the fix. `maturin
develop --release` + `pytest tests/` (187 passed, unchanged, 1
pre-existing skip) and all 33 pre-existing examples plus the new
`demo/showcase.py` confirmed clean with the real display. `mkdocs
build --strict` clean after the six doc fixes.

M27 Phase 1 — Shell & Navigation Scaffold is now complete. M27 itself
continues with Phase 2 (MD3 component & theming gallery screen).
