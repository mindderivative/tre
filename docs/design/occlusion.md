# Occlusion: hidden windows

Work for issue #246 (0.5.10), measured 2026-11-02 on headless sway with llvmpipe and on KWin 6.8 (a real session) with
AMD RADV. It started as "a looping SVG animation keeps drawing when nobody can see it" and turned into a finding
about the whole app.

## What was wrong

A window the compositor is not showing (another workspace, minimized, a layer surface under a fullscreen window)
used to cost almost no CPU, because its vsync present blocks until the compositor sends a frame callback. **The
blocked call held the Python GIL and the single thread that runs every window**, so with one hidden window the
app froze: a second, visible window stopped animating (identical screenshots), and a Python thread printed
nothing for 50 s. `present_mode="low_latency"` avoids the block and spins every core (about 380%), so it is not a fix.

tess_re never called winit's `Window::pre_present_notify()`. Without it winit keeps delivering `RedrawRequested` while the
compositor withholds frame callbacks, so tess_re kept drawing into a swapchain that never drained.

## What changed

| Part | Change |
| --- | --- |
| Windows | `pre_present_notify()` before `present`. winit then holds a window's next redraw until its frame callback; `ControlFlow::Wait` replaces `Poll` on Wayland (`PerWindow::polls`, one rule for windows and layers). |
| Layer surfaces | No winit window, so `LayerController::request_frame/frame_pending` ask `wl_surface.frame` themselves; the layer thread answers in `CompositorHandler::frame` (`LayerEvent::FrameDone`) and `run_layer_frames` skips a surface with a callback out. A frame request made while the callback was out is kept, not dropped. |
| Resize | A resize-paced frame (#155) skips the gate: it presents without a vsync barrier on purpose. |
| X11, macOS | winit `Occluded` becomes the window state and event `occluded`; with `pause_when_occluded` (default on) an occluded window skips prepare, draw and present and asks for no more frames. A minimized window reads `occluded` too. |

## Measured

| Case | Result |
| --- | --- |
| sway + llvmpipe, window on another workspace | about 0 CPU ticks in 5 s (visible: 129 in 4 s) and **the other windows keep running** (before: frozen) |
| sway, a bar under a fullscreen window | the app keeps its heartbeat (before: 0 in 3 s); the bar draws again when uncovered |
| KWin + RADV, a minimized window | 23 frames in 8 s with the change against 255 on main; the visible second window runs 60/s instead of 32/s |
| KWin + RADV, window on another virtual desktop | 0.4-0.6% of a core against 3.1% |
| KWin + RADV, a bar under a fullscreen window | KWin keeps showing it, so it keeps drawing (498 frames in 8 s, as visible) |

The freeze itself **reproduces on sway/wlroots only**. On KWin, `main` did not stall: a minimized or other-desktop
window's present did not block, so it kept drawing (255 frames in 8 s for the minimized one) instead of freezing the
app, and a bar under a fullscreen window is still shown and still gets frame callbacks. With the change a minimized window
there drops to 23 frames, so KWin does stop calling back once tess_re waits for the callback; why `main`'s present did not block
there was not investigated. Counting `frame_stats()["frames"]` per phase settled what CPU% alone got wrong.

## Known gaps

- While every window of an app is hidden, timers, `call_soon` and `on_complete` wait for one to be shown, because they
  run in a window's frame (#247).
- `present_mode="immediate"`/`"low_latency"` no longer outrun the compositor on Wayland (a resize still does).
- The `occluded` path on X11 and macOS follows winit's documentation and was not run (XWayland under sway sends none).
- A fullscreen window counts about 480 frames/s on KWin, also on `main` (#248).
