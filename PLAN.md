# PLAN — M32 Phase 2: Window Resize Handling

## Goal
Close the real, stated v1 gap M29's own trailer named: "nothing
resizes any node's box when its window resizes." A real
`WindowEvent::Resized` must genuinely resize the root node's own
layout box and reconfigure the real GPU surface, not silently no-op.

## Steps
1. Confirmed via direct grep that `WindowEvent::Resized` had no arm at
   all in `engine-platform`'s own match -- fell through the existing
   `_ => {}` catch-all.
2. Added `InputEvent::Resized { width, height }` (`engine-core::
   input.rs`) in window-client-pixel space, no DPI conversion, matching
   `PointerMoved`'s own established convention.
3. Unlike `ThemeChanged` (plumbing only, deferred to `engine-py` since
   `engine-core` has no MD3 knowledge), `Tree::dispatch` handles
   `Resized` directly: a window resize is a pure taffy concern
   `engine-core` fully owns. Uses the existing `Tree::set_layout_style`
   (not a direct field mutation) to keep taffy's own internal copy in
   sync.
4. `engine-platform::window_event` translates `WindowEvent::Resized`
   into the new `InputEvent`, requests a redraw -- the same "translate
   the raw event, let engine-py decide what it means" split
   `ThemeChanged` already established.
5. Investigated whether `FrameRenderer`/`vello_hybrid::Renderer` need
   reconstruction on resize: direct source read of the vendored
   `vello_hybrid = "0.2.0"` confirmed `Renderer::render` already calls
   a private `maybe_update_config_buffer` every frame, recreating its
   own internal depth texture whenever `RenderSize` genuinely differs
   from the previous call -- real, existing resize-safety, no new
   reconstruction needed.
6. Added `GpuState::resize` (`engine-py::app.rs`): the textbook wgpu
   resize recipe -- mutate a stored `SurfaceConfiguration`'s own
   `width`/`height`, then `surface.configure` again. Skips 0-sized
   dimensions (a real wgpu panic otherwise, a real transient value on
   some platforms while minimizing).
7. `on_input`'s own `InputEvent::Resized` arm updates `runtime.width`/
   `height` (u32, read by every per-frame `compute_layout`/
   `build_tree_scene`/`RenderSize` call already) and calls `GpuState::
   resize`. `Tree::dispatch` (called unconditionally just above for
   every real `InputEvent`) already resized the root's own box.
8. Added `Window.resize(width, height)`: the synthetic, no-live-window-
   needed testing entry point, the identical pattern `click`/`hover`
   already establish -- updates `self.width`/`height` too (this
   object's own fields, unlike `WindowRuntime`'s separate copy the live
   path touches) and dispatches the real `InputEvent::Resized`.
9. Real Rust unit test caught a real bug on the first run: an initial
   draft mutated `node.layout_style` directly, bypassing
   `Tree::set_layout_style` -- a fresh `compute_layout` after the
   resize still reported the stale size, because taffy keeps its own
   internal copy of every node's style, never reading `Node::
   layout_style` back out on its own. Fixed by going through the real,
   existing `set_layout_style` method instead.
10. Real, direct empirical script before pytest: `Window.resize`
    doesn't raise, a click dispatched after it still reaches its own
    handler.
11. Added `tests/test_resize.py` (4 tests) and `examples/resize.py`.
12. Full verification chain: cargo check/clippy/fmt/test, maturin
    develop, pytest (full suite), all 70 examples, showcase demo,
    mypy --strict.
13. **Real, stated v1 limit, not glossed over:** a live OS-driven
    resize reaches `WindowRuntime`'s own `width`/`height` (governing
    per-frame rendering) and the tree's own root box, but *not*
    `PyWindow`'s own `width`/`height` fields -- a separate, non-shared
    copy read by every interactive `add_*` factory method
    (Dialog/Snackbar/Side Sheet/Navigation Drawer/etc.) for their own
    layout. An app that calls one of those from a live click handler
    after a real resize still sizes against the window's construction-
    time dimensions. Making `PyWindow`'s own fields genuinely shared,
    live state is a real, separate, deeper change, out of this phase's
    scope. Also real, stated: exercising a *live* OS-driven resize end
    to end isn't reachable through this project's existing headless
    testing surface (no way to script an actual window-manager resize),
    so this path is verified by code review plus `vello_hybrid`'s own
    existing resize-safety mechanism, not a dedicated empirical test.
14. Update `BUILD_TRACKER.md`, regenerate + republish the artifact,
    update memory, commit.

## Status
Complete. All steps done; full verification chain green (`engine-core`
gains 1 new unit test that caught and drove a real fix, `pytest tests/`
511 passed/1 skipped, up from 507, all 70 examples, showcase demo,
mypy --strict clean).
