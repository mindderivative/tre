# LOG — M32 Phase 2: Window Resize Handling

- Confirmed via direct grep that `engine-platform`'s own `WindowEvent`
  match had no `Resized` arm at all -- fell through the existing
  `_ => {}` catch-all, matching this phase's own stated gap exactly.
- Added `InputEvent::Resized { width: f32, height: f32 }`
  (`engine-core::input.rs`), window-client-pixel space, no DPI
  conversion (matching `PointerMoved`'s own established precedent).
- Traced every real reader of window size before writing any code:
  `PyWindow.width`/`height` (many interactive `add_*` factory methods'
  own `compute_layout` calls) and `WindowRuntime.width`/`height`
  (`engine-py::app.rs`, a *separate*, non-shared `u32` copy used for
  the real per-frame `compute_layout`/`build_tree_scene`/`RenderSize`
  calls) -- confirmed these are genuinely two different struct
  instances, not aliased state, a real architectural fact that shaped
  this phase's own honest scope limit (see below).
- Unlike `ThemeChanged` (plumbing only -- `engine-core` has no MD3
  knowledge), gave `Tree::dispatch` a real, direct handling arm for
  `Resized`: resizing a node's own layout box is a pure taffy concern
  `engine-core` fully owns already.
- **Real bug caught by this phase's own first test, not predicted in
  advance:** an initial draft mutated `node.layout_style` directly via
  `self.get_mut(root)`. The new unit test (`resized_grows_the_roots_
  own_layout_box_and_a_fresh_layout_reflects_it`) failed on its very
  first run: after the resize, a fresh `compute_layout` still reported
  the OLD size (100.0 instead of 300.0). Root-caused via direct read of
  `Tree::set_layout_style`'s own doc comment: "the only place after
  `insert` that's allowed to touch `layout_style`" -- taffy keeps its
  own internal copy of every node's style (handed over once at
  `insert`) and never reads `Node::layout_style` back out on its own,
  so a direct field mutation silently desyncs the two. Fixed by
  reading the current style, cloning it, mutating just `.size`, and
  calling the real, existing `set_layout_style` instead. Re-ran the
  same test -- passed.
- `engine-platform::window_event` translates `WindowEvent::Resized`
  into the new `InputEvent`, requests a redraw -- the identical
  "translate the raw event, let engine-py decide what it means" split
  `ThemeChanged` already established.
- Investigated whether `FrameRenderer`/`vello_hybrid::Renderer` need
  reconstruction on a resize, by direct source read of the vendored
  `vello_hybrid = "0.2.0"`: `Renderer::render` already calls a private
  `maybe_update_config_buffer` every real frame, which recreates its
  own internal depth texture whenever the `RenderSize` passed to
  `render` genuinely differs from the previous call -- real, existing
  resize-safety this phase only needed to rely on, confirmed via
  direct source read rather than assumed.
- Added `GpuState::resize` (`engine-py::app.rs`): stores the original
  `SurfaceConfiguration` as a new field (`surface_config`, kept around
  specifically so `resize` can reconfigure with the identical real
  `usage`/`present_mode`/`alpha_mode`/`view_formats` the adapter chose
  at construction, not re-derive them), mutates its own `width`/
  `height`, calls `surface.configure` again -- the textbook real wgpu
  resize recipe. Skips 0-sized dimensions (a real wgpu panic otherwise,
  a real transient value some platforms report while minimizing).
- `on_input`'s own new `InputEvent::Resized` arm updates `runtime.
  width`/`height` and calls `GpuState::resize` -- `Tree::dispatch`
  (called unconditionally just above, for every real `InputEvent`)
  already resized the root's own box by this point.
- Added `Window.resize(width, height)`: the synthetic, no-live-window-
  needed testing entry point, the identical real pattern `click`/
  `hover` already establish (a real resize has nowhere else to
  originate outside a live window either). Updates `self.width`/
  `height` directly (this object's own fields, unlike `WindowRuntime`'s
  separate copy) and dispatches the real `InputEvent::Resized`.
- Full Rust verification chain green after the one real fix above:
  `cargo check`/`clippy -D warnings`/`fmt --check`/`cargo test
  --release` all clean (`engine-core` 166, up from 165).
- Rebuilt the Python extension. Ran a real, direct empirical script
  before writing any pytest: `Window.resize` doesn't raise (growing and
  shrinking both checked), and a click dispatched at a rect after a
  real resize still reaches its own handler.
- Added `tests/test_resize.py` (4 new tests, checked for a filename
  collision first: none) and `examples/resize.py` (checked for a
  collision first: none) -- both exercise the synthetic path only,
  with an honest module-doc note on why the live winit-driven path
  isn't independently exercised (no way to script a real OS window
  resize in this headless testing setup).
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --release` (`engine-core` +1 unit test that caught
  a real bug), `maturin develop --release`, `pytest tests/` 511
  passed/1 skipped (4 new, up from 507, zero regressions), all 70
  examples (including the new `examples/resize.py`) and the showcase
  demo re-run clean, `mypy --strict` clean against `examples/resize.py`.
- **Real, stated v1 limit, documented directly, not glossed over:** a
  live OS-driven resize does *not* reach `PyWindow`'s own `width`/
  `height` fields (a separate, non-shared copy from `WindowRuntime`'s)
  -- an app that calls an interactive `add_*` factory method (Dialog,
  Snackbar, Side Sheet, Navigation Drawer, all of which read `self.
  width`/`height` for their own layout) from a live click handler
  after a real resize still sizes against the window's construction-
  time dimensions. Making `PyWindow`'s own fields genuinely shared,
  live state is a real, separate, deeper architectural change, out of
  this phase's scope. Also real, stated: a genuine, *live* OS-driven
  resize isn't independently exercised by an automated test here (no
  way to script an actual window-manager resize in this headless
  setup) -- verified by code review plus `vello_hybrid`'s own existing,
  confirmed resize-safety mechanism instead.
- Updated `BUILD_TRACKER.md` (Top Metrics row, Phase 2 heading and Step
  1) -- verified the parser's own reported item count before/after,
  regenerated and republished the Build Tracker artifact.
