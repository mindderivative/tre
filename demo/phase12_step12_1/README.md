# Demo: Phase 12 Step 12.1 -- Real Input Events + Windowed Rendering (`tre-python`)

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase12_step12_1
../../.venv/bin/python demo.py
```

Needs a real, live Wayland or X11 session (`DISPLAY`/`WAYLAND_DISPLAY` set)
and real GPU hardware -- this demo opens a real on-screen window, not a
headless surface.

**What this proves.** The first section of Phase 12's plan to make
`tre-python` a complete `pySilver` backend, not just a pretty offscreen
renderer: `tre_python.WindowedRenderer` (new) opens a real OS window via
`tre_platform::PlatformConnection`, submits real GPU frames directly to
that window's own `VulkanSwapchain` (no byte readback -- that stays
`HeadlessRenderer`'s job), and surfaces real input/lifecycle events via
`renderer.poll_events() -> list[tre_python.InputEvent]`. `demo.py`:

- Creates a real window, builds a `ShapeRegistry` with a red rectangle,
  green circle, and blue hexagon (reusing the same shape classes
  `HeadlessRenderer` already binds), and renders 120 real frames directly
  to the window's swapchain.
- Confirms a real `InputEvent.Resized` comes back through `poll_events()`
  -- proof the whole path (compositor -> `PlatformConnection` ->
  `tre_engine::InputEvent` -> `tre.InputEvent`) is live, not stubbed.
- Exercises every real window-lifecycle method (`set_title`,
  `is_minimized`, `is_maximized`) without error.
- Creates a *second* real window sharing the same `VulkanDevice`
  (`multi_window.rs`'s own proven pattern), renders into it, and closes
  it -- multi-window is real from day one, not artificially restricted to
  one window.

**Two real constraints found only by building this** (see
IMPLEMENTATION.md's Phase 12 Step 12.1 write-up for the full account):

- PyO3 0.27's "complex enum" support rejects a plain unit variant mixed
  with a data-carrying variant in the same enum. `MouseButton.Left`/
  `Right`/`Middle` had to become empty-tuple variants (`Left()`, not bare
  `Left`) to compile alongside `Other(u16)`.
- `WindowedRenderer` cannot be `Send` -- it owns a real
  `PlatformConnection`, which wraps winit's X11/Wayland backends (raw
  `Rc`/`RefCell`/FFI handles only safe to touch from their creating
  thread, a real OS constraint). Marked `#[pyclass(unsendable)]`, and
  `render()`'s GIL-released closure was restructured to never touch
  `self.connection` at all (a non-`Send` capture fails `Python::detach`'s
  own `Ungil` bound) -- swapchain recreation on a stale-swapchain retry
  happens back on the GIL-holding thread between `detach` calls instead.

**Full workspace verification performed:**

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets --
  -D warnings` / `cargo build --workspace --all-targets` / `cargo test
  --workspace` -- all clean in debug.
- In `--release`, two pre-existing `tre-engine` tests fail
  (`gpu_style::style_index_param_rejects_a_misaligned_offset_in_debug_builds`,
  `flatten_into_still_enforces_the_balance_assertion_on_the_reused_path`)
  -- both rely on a `debug_assert!` that's a no-op in release builds, a
  real, disclosed pre-existing gap unrelated to this step (confirmed via
  `git status` showing zero uncommitted changes to `tre-engine`); flagged
  as separate follow-up work rather than fixed here.
- `demo.py` run against a real live compositor with real GPU hardware:
  exits 0, prints the real `Resized` event observed, and completes 150
  total real on-screen frames across two windows with zero exceptions.

**Not yet done, deliberately deferred to Phase 12's remaining sections:**
text rendering, gradient/texture fill, and the `Canvas` redesign around
clip/layer/accessibility semantics.
