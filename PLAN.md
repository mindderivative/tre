# PLAN — M33 Phase 2: Live, Shared PyWindow Dimensions

## Goal
Close the real, stated v1 limit M32 Phase 2 left open: a live,
winit-driven window resize updated `WindowRuntime`'s own `width`/
`height` but never reached `PyWindow`'s own fields, so an interactive
`add_*` factory method called after a real resize (e.g. from a live
click handler) still sized against the window's construction-time
dimensions. This closes M33 itself, both phases.

## Steps
1. Confirmed via grep before designing anything: `self.width`/
   `self.height` are read at 34 real call sites across `window_
   factory.rs`/`window_input.rs`/`window_docking.rs`/`window_virtual_
   canvas.rs` (a wider real blast radius than the earlier 27-site
   estimate, which missed the latter two files).
2. Added `window::SharedSize = Rc<Cell<u32>>` -- the identical real
   sharing shape `SharedTheme`/`HandlerMap`/`context_menus`/`dock`
   already use, `Cell` instead of `RefCell` since `u32` is `Copy` and
   every real access is a plain get/set, never a borrow that could
   outlive one statement.
3. Changed `PyWindow.width`/`height` from plain `u32` to `SharedSize`.
   `PyWindow::new`'s own public Python-facing signature is unchanged
   (still takes plain `u32` args) -- only the internal storage type
   changed.
4. Fixed all 34 real call sites the compiler found (not a guess --
   `cargo check`'s own error list was the real, exhaustive worklist):
   `self.width as f32` -> `self.width.get() as f32` (and the `height`
   sibling) at every real read site; `Window.resize`'s own `self.width
   = width` -> `self.width.set(width)`.
5. Widened `WindowSetup`/`WindowRuntime` (`app.rs`) to hold the
   identical `SharedSize` too -- extracted via `.clone()` (a cheap
   `Rc` clone sharing the same cell), not a `u32` copy, so `App::run`'s
   own live `InputEvent::Resized` handler's `.set()` call is now
   immediately visible on `PyWindow`'s own fields too. Fixed the
   remaining real call sites needing a plain `u32` (`GpuState::new`,
   `RenderSize`, `WindowConfig`, `build_tree_scene`'s own `u16` args)
   with `.get()`.
6. Updated two stale doc comments (`window_input.rs`'s own `resize`
   method, `app.rs`'s own `InputEvent::Resized` arm) that explicitly
   named the real v1 limit this phase closes -- both corrected to
   state the real fix directly, not left describing a gap that no
   longer exists.
7. Real, direct empirical script before pytest: confirmed the
   synthetic `Window.resize()` path plus a subsequent interactive
   `add_dialog`/`open_dialog` call still works correctly after a
   resize -- a real regression check, since the live winit-resize-
   reaching-`PyWindow` claim itself can't be scripted (no way to
   trigger a genuine OS window resize event, the same established
   limit this whole project already accepts for winit-only behavior).
8. Extended `tests/test_resize.py` with a new test proving the exact
   real scenario the prior phase's own doc comment named as broken:
   an interactive `add_dialog` call after a real resize still builds
   and opens correctly.
9. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all 71 examples, showcase demo.
10. Update `BUILD_TRACKER.md` (closing Phase 2 and M33 itself),
    regenerate + republish the artifact, update memory, commit, push
    (a full milestone closing).

## Status
Complete. All steps done; full verification chain green (`pytest
tests/` 527 passed/1 skipped, up from 526, all 71 examples, showcase
demo). **M33 -- Real Window/Terminal Resize Propagation is now fully
complete, both phases.**
