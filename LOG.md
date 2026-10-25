# LOG — Branch `0.5.x`: custom windowing to 0.5.7

## Status

**0.5.7 released (2026-10-17, PR #185, tag `v0.5.7`, PyPI `tesserae-engine==0.5.7`).**
0.5.6 was released 2026-10-09 (PR #167, tag `v0.5.6`).
Tracking lives in the GitHub project Tesserae Rendering Engine (the old
`BUILD_TRACKER_ARCHIVE_*.md` files are the pre-GitHub trackers). Open
issues: #47, #48 (macOS and Windows title bars on hardware), #91 (WGSL on
real GPUs: the Linux half is done; Windows, macOS and a video input are held),
#118 (baseline on Mac, Windows, low-end hardware), #132-#134 (touch, OS file
drag and drop, reduced-motion and contrast on hardware), #158 (a burst of
`wp_fifo_v1` barriers after a resize).

## After 0.5.7 (unreleased)

### Helper tools for the agent (2026-10-25)

Instructions moved to `AGENTS.md` (`CLAUDE.md` is `@AGENTS.md`). New in `tools/`: `check_agent_docs.py`
(keeps `CLAUDE.md` an import; run by `verify.sh`), `graphify_agents.py` (applies `graphify claude
install` to `AGENTS.md` and the hooks, after a graphify upgrade), `board.py` (board status, and card
moves with a guard on Done), `check_version.py` (the version agrees in `Cargo.toml`, `pyproject.toml`
and the four `Cargo.lock` entries; `--pypi` checks the version is free), and `status.sh` (one-screen
state). Tested against the real repo and board, and against broken fixtures; `board.py --comment`
was not exercised, since it posts publicly.

### Project skills reviewed and revised (2026-10-25)

The three skills added in `71adab8` (`pixel-test-triage`, `release-checklist`, `perf-profiling`)
were reviewed and revised. `perf-profiling` now lists the live measurements (`frame_stats()` stages
and `memory`, `profile_nodes`, `start_trace`), `TRE_GPU_MEMORY=performance` as a comparison setting,
and that `acquire_ms` under vsync is waiting, not work. `pixel-test-triage` sizes flaky re-runs by
failure rate (about 20 for a 1-in-6 flake) and says to report a test, branch or version that does not
exist. `release-checklist` pre-flight asks when the named branch or version is missing.

### KWin corner reservation re-checked (2026-10-25)

The Tesserae Shell ADR/request note listed "KWin shows no reservation for a corner surface" as
unresolved, with a late-measurement candidate. Re-run on the real KWin 6.8.1 session: a
`("top","right")` 120x28 surface with `exclusive_zone=28` and `exclusive_edge="top"` shortens a
maximised window from 1012 to 984 px, the same at 1, 3 and 6 s after the surface maps; the
`"right"` edge and no-edge cases of `test_a_corner_surface_reserves_space_on_the_edge_it_names`
also pass. The KWin skip is removed from that test, and the reference and hand-off no longer say
KWin lacks it. Cause of the first reading not found.

### Hand-off corrected (2026-10-25)

`docs/design/layer-window-shell-handoff.md` now says `PopupWindow` (#223) and small GPU memory
blocks (#219) exist, that the overlay menu is kept but is not the efficient route, and that
`PopupWindow` was not run on KWin.

### #223, `PopupWindow` (2026-10-25)

A window whose role is `xdg_popup`, parented to a `LayerWindow` (through
`zwlr_layer_surface_v1.get_popup`) or to another `PopupWindow`. The layer thread's `Role` enum is
`Layer | Popup{parent}`; the popup's `xdg_positioner` takes size, anchor rect, anchor, gravity,
constraint adjustment and offset; `grab` uses the serial of the last press/key/touch. `configure`
carries `x`, `y` (the compositor's placement after any flip), a dismissed popup closes with reason
`"dismissed"`, a missing or closed parent with `"failed"`. sctk's `delegate_xdg_shell!` wants a
`WindowHandler`, so `XdgWmBase` and the decoration manager have manual `Dispatch` impls.

Found while testing: the grab dismisses on a click outside the *client*, not on another surface of
the same client (that is the protocol), so a headless test that clicks on the app's own backdrop
window sees no `popup_done`. The live tests click the bare desktop. Verified on headless sway
(placement, flip above a bottom bar, outside-click dismissal, item click, submenu); not yet on KWin.

### #219, small GPU memory blocks by default (2026-10-25)

A window's device (and the headless snapshot device) now asks `wgpu` for `MemoryHints::MemoryUsage`
(blocks of 8 to 64 MiB) instead of the default 128 MiB blocks: `gpu_memory.rs`.
`TRE_GPU_MEMORY=performance` restores the old reservation. On the measured machine a small window
went from 211 to 31 MiB of video memory. Tests run for this change only: the unit test of the
mapping, two live tests that read a window's own memory from `/proc/self/fdinfo` (default under
100 MiB; the variable brings back more than 100 MiB extra), and the snapshot tests (they use the
other device site).

### #221, no kept frame for a window that redraws in full (2026-10-25)

`GpuState::new` takes the window's `partial_redraw` and makes the persistent target only when it is
on and the surface can be copied into (`copyable`, now kept on `GpuState`). `WindowRenderer::set_persistent`
makes or drops it live; each frame checks the setting, and `window.set(partial_redraw=...)` now marks the
tree dirty so an idle window makes the change. Measured: a 2400 x 1400 window 54.4 MiB with the
kept frame, 38.4 MiB without. Dropping it live did not lower the driver's count (AMD, 4 s). Tests: a
Rust pixel test (give back, draw in full, make again, partial exact) and a live fdinfo test.
`partial_redraw_active` now means "the surface allows it and the setting is on".

### #222, allocator totals in `frame_stats()` (2026-10-25)

`frame_stats()["memory"]` (and `StatsHandle.read()`) reports `allocated_bytes`, `reserved_bytes`,
`blocks` and the five `largest` allocations, from the window's `wgpu` allocator report. `FrameStats`
keeps a clone of the window's `Device` (set where the window's `GpuState` is made, for windows and
layer windows alike) and builds the report when the stats are read, so a frame costs nothing extra.
`None` before the window opens or on a backend without an allocator report. Tests: the empty
case, and a live window.

### #207, why a small window holds about 210 MB of VRAM (research, 2026-10-25)

Measured per process from `/proc/<pid>/fdinfo`: a 200 x 100 window 211 MiB VRAM and 73 MiB GTT, each
extra window about 193 MiB more. tre allocates under 1 MiB of it; the rest is `wgpu`'s allocator,
which on `request_device` reserves 192 MiB (device blocks of at least 128 MiB, host-visible ones
of 64 MiB; `MemoryHints::Performance`, the default) and tre makes a device per window. With
`MemoryHints::MemoryUsage` the same window is 31 MiB (a bar 18, two windows 44, a 4K overlay 177)
and a 900-box animated scene's frame time is unchanged (15.2 to 15.3 ms mean over three runs each).
At 4K the part that scales with pixels is the driver's four swapchain images plus tre's
persistent target (about 19 bytes a pixel); `desired_maximum_frame_latency` and the present modes
do not change it. One GPU (an AMD integrated GPU, RADV); other hardware unmeasured. Throwaway edits
and the probe program were reverted; kept: `tools/vram_probe.py` and `docs/design/vram-per-window.md`.
Findings are filed as Backlog issues; nothing built.

### #195 closed; the hand-off for Tesserae Shell (2026-10-25)

All six phases of #195 and the follow-ups #199, #200 and #201 are done and on `main`.
`docs/design/layer-window-shell-handoff.md` is the hand-off for the shell side: what shipped,
how each of the request's acceptance criteria stands (met, replaced, not tested), what differs
from the request, the compositor facts (KWin 6.8.1 does not reserve space for a corner surface),
the costs, and what was never tested. It also adds a live hot-plug test (`output_added`,
`output_updated` and `output_removed` fire on sway).

### #200, AccessKit tree for layer surfaces (2026-10-25)

A layer surface is now published to screen readers. `accesskit_winit` needs a `winit` window, so
each layer surface gets an `accesskit_unix::Adapter` of its own (Linux-only dependency, already in
the lock through `winit`, same features); the spike's question, whether the adapter assumes a
toplevel, is answered: it does not, and a bar appears as one more window of the application. The
adapter's handlers run on another thread, so they only post a `PlatformEvent::LayerAccess` and the
loop thread answers: `request_initial_tree` returns `None` and the loop calls `update_if_active`
with `build_access_update` (the closure windows use, keyed by the layer's minted id; held back
until the surface is open), the tree is refreshed after each layer frame, an `ActionRequest` goes
to `on_access_action` and asks for a frame, and the layer thread's keyboard focus event feeds
`update_window_focus_state`. Verified through AT-SPI with no change to the desktop's accessibility
settings: `tests/atspi_env.py` builds a private environment (two throwaway `dbus-daemon`s with no
service directories, a stand-in `org.a11y.Bus` made with `dbus-fast`, the real `at-spi2-registryd`)
and `busctl` plays the screen reader: the application, the bar's window and a `Save` push button
(role 43) appear, a `DoAction` click reaches the app, and a changed label shows up. Passes on sway
and KWin. Not tested: the window-focus state (a headless sway has no keyboard focus without a
virtual keyboard), a real screen reader (Orca), and screen positions (Wayland gives an application
none). The bar's window node has no name yet.

### #199, input-method (IME) preedit on layer surfaces (2026-10-24)

A `LayerWindow`'s text input now works with an input method. The layer thread binds
`zwp_text_input_manager_v3` (optional) and makes one text input for the seat (at start, not when
a keyboard appears: a headless compositor has no devices). The compositor's `enter` / `leave` say
which surface has the text-input focus; the frame pass sends the focused field's caret box
(logical px) and password flag through a new `LayerController::set_ime`, and the thread `enable`s,
sets the content type (password: `sensitive_data` and `password`), the cursor rectangle, and
`commit`s, or `disable`s. `preedit_string`, `commit_string` and `delete_surrounding_text` are held
until `done` (`layer_ime.rs`, a pure state machine, unit tested) and then become
`InputEvent::ImePreedit` and `TextInput`, as the winit path does: the old preview cleared, the
commit, the new preview. No input method is installed here and wiring one into KDE would change
the desktop's input settings, so the live tests use a **stand-in input method on a headless sway**
(`tests/wlr_input_method.py`, `pywayland`, bindings generated from the vendored input-method v2
XML; it also keeps a virtual keyboard, because a sway with no keyboard device gives no window the
keyboard focus): activation on focus, a preview shown, replaced and cleared, a commit landing as
text, and the password and plain content types all pass. Not run: a real input method (fcitx5,
ibus), KWin's path (its text-input needs a virtual-keyboard input method configured in KDE),
surrounding text (not reported; `delete_surrounding_text` is ignored).

### #201, exclusive_edge: tre binds the layer shell itself (2026-10-24)

`LayerWindow(exclusive_edge=...)` and `set(exclusive_edge=...)` now work: a corner-anchored surface
reserves its `exclusive_zone` on the edge it names (a clock at the top right with
`exclusive_edge="top"` shortens a maximised window; `"right"` narrows it). Option (a): tre binds
`zwlr_layer_shell_v1` at version 5 itself and makes its own `zwlr_layer_surface_v1`
(`layer_surface.rs`, on the `wayland-protocols-wlr` 0.3.12 already in the lock), replacing the
toolkit's `LayerShell`, `LayerSurface`, `LayerShellHandler` and `delegate_layer!` (the toolkit
shared with `winit` binds only up to 4): the bind, the `set_*` requests, the `ack_configure`
before the app hears of a `configure`, destroy on drop. A v4 compositor still opens the surface
and a `warning` event says the edge is ignored. The protocol cannot clear an edge, so
`set(exclusive_edge=None)` on a surface that has one raises. Live: sway (32 passed, 3 skipped,
the corner reserves on the named edge and nothing without one) and KWin (26 passed, 9 skipped).
**(Corrected 2026-10-25: KWin 6.8.1 does reserve for a corner surface, see the entry below; the
"no reservation" reading made here was wrong.)**

### KWin versions of the region, drag and menu live tests (2026-10-24)

The `input_region`, `drag_margin` and overlay-menu live tests now also run on KWin, through
`/dev/uinput` (`TRE_LIVE_INPUT=1`, moves the real pointer): `tests/uinput_device.py` has a
`Pointer` with the same interface as `wlr_pointer.Pointer` (and Escape), the tests pick the
backend by compositor, the window below is maximised (KWin floats a new window small), and the
drag targets are computed from the output's size. 12 of the 14 ran green on the first KWin run;
`[]` click-through failed once and passed on every rerun (the window below was not ready; the
test now waits a second). Real bug found: **a `drag_margin` drag did nothing on KWin**, because
an absolute pointer (a tablet, a VM's, a remote desktop's, a uinput absolute device) sends no
relative motion, and the drag only listened to it when the protocol exists. The drag now uses the
surface-local position until relative motion actually arrives. Still sway-only (they read pixels
with `grim` or need its output commands): transparent pixels, paste, unplug, 1.5x scale, margin
animation, the bottom-bar flip.

### #206, popups: the overlay route (6a, 2026-10-23)

Phase 6 of #195, stage 6a: a menu for a bar's button on a transparent full-screen overlay
`LayerWindow` (`input_region="auto"`, `keyboard="exclusive"`) with `show_layer` menus, mapped
only while the menu is open. No new API; one engine change: while a dismissible or modal layer
is open `input_region="auto"` takes the whole surface, since otherwise the outside click that
dismisses a menu fell through to the window below and the menu never closed
(`Tree::has_input_grab`). Live on headless sway (virtual pointer, `wtype`): dismiss on outside
click (consumed, not passed on), on Escape, item click, flip above a bottom bar, a submenu, a
click reaching the window below when no menu is open, and an open menu draws 0 idle frames.
Measured: about 212 MB VRAM per mapped window on a 720p output and about 333 MB on 4K (about
210 MB is a fixed per-window cost, the rest scales with pixels), released on close; the menu is on
screen about 140 ms after the click on 4K. Verdict: the overlay route works and `PopupWindow` is
not needed yet, if the overlay is created on demand. Guide: `docs/guide/menus-from-a-bar.md`;
example: `examples/layer_menu.py`.

### #205, margin animation and `drag_margin` (2026-10-20)

Phase 5 of #195: stage 5a, margin animation; stage 5b below.

Stage 5a: `LayerWindow.animate("margin_top" | "margin_right" |
"margin_bottom" | "margin_left", to, duration_ms, easing, on_complete)`. The four margins are
`Animated<f64>` values ticked in the window's frame pass; a changed rounded margin goes to the
layer thread as a margin-only command (one cheap commit, no other property re-sent). The loop
is woken by its alarm every 8 ms while a margin moves (no spinning, no buffer drawn). Same
easing and `on_complete` as `Node.animate`, `animation_end` on the root, retargeting from the
current value, `set(margin=...)` cancels and snaps, `advance(ms)` runs it headless. Frame passes
now tick to the instant the loop turn began (`engine_platform::turn_instant`), so windows
animating together read one time. Live on headless sway (read back with `grim`): a 0 to 300 px
slide in 800 ms is smooth (about 6 px per 16 ms), monotonic, and `on_complete` lands on time.

Stage 5b, `window_region="drag_margin"`: a primary press on such a node of a `LayerWindow` (same
exclusions as `"drag"`) tells the layer thread to follow the pointer. The deltas come from
`zwp_relative_pointer_v1` (exact however the surface lags; sctk already has the binding), or
from the surface-local position when the compositor lacks it (can overshoot a little). The
anchored edges' margins change, clamped to the output, one margin-only commit per input
frame, no Python; on release `moved` carries the final `margin` and the window's own margin
record and animation state follow it. A running margin animation stops where it is when a drag
starts. `drag_margin` raises on an ordinary window; `"drag"` still raises on a layer window.
Live on headless sway with a virtual pointer: the margins follow a drag exactly, and stop at the
far and near output edges. The surface-local fallback is not run live (sway has the protocol).

### #204, `LayerWindow.input_region` (2026-10-20)

Phase 4 of #195. `input_region` (constructor and `set()`): `None` (whole surface), `[]`
(nothing, so clicks fall through), rectangles in logical pixels (rounded outward, at most 256),
or `"auto"` (the shapes of drawn, hit-testable nodes). `click_through=True` is `input_region=[]`
on a `LayerWindow`; the two disagreeing raises. The layer thread builds a `wl_region` and
commits it at once, so an idle surface needs no frame; `"auto"` is recomputed after each
layout, only grows while something animates, and snaps exact when it ends. **Behaviour change
for every window:** hit testing now follows `corner_radius` / `corner_radii` (a click in a
clipped-away corner no longer hits the node; masks and `clip_children` are still ignored), so
what the compositor passes and what tre accepts agree. New: `Tree::input_region` (bands of 2 px
or more, at most 256 rectangles, never less than drawn). Tests: pure region and hit tests in
engine-core, headless Python tests; live click routing on sway needs `wlrctl` (not yet run).

### #203, live checks on KWin and sway (2026-10-20)

Ran the live LayerWindow suite on KWin and on a headless sway 1.12 (`WLR_BACKENDS=headless`,
wlroots). Found and fixed on sway: `configure` fired twice for one size (sway re-sends it;
now it fires only on a change); a bar's `closed` after an unplugged output said `compositor`
(the compositor closes the surface before it removes the output; the reason is now settled
after a roundtrip, so it says `output_removed`); `LayerWindow.get("scale_factor")` was 1.0
at any scale (now the surface's scale). `output=None` now picks the output at (0, 0), else the
first by name, instead of the compositor's alternating choice. New live tests: `output=None`
lands on the origin output, unplug, 1.5x scale, transparent pixels (`grim`), and paste (`wtype`, `wl-copy`),
all sway only. Paste was a real gap: the clipboard went through XWayland only, so on a session with
no X server a paste into any window did nothing (`read_clipboard()` was `None`); `arboard`'s
`wayland-data-control` feature now reads and writes it through the compositor (KWin and wlroots offer
it). Not yet run: touch, other GPUs, labwc, the fullscreen frame-callback check.

### #202, outputs as objects (2026-10-19)

Phase 3 of #195. `tre.Output` (frozen: name, description, x, y, width, height, scale_factor,
transform; equal and hashable by name), `App.outputs()`, and the events `output_added`,
`output_removed`, `output_updated` through `App.on` / `App.off`. Position and size are logical;
`scale_factor` is the effective mode-over-logical ratio. `outputs()` takes a fresh short-lived
connection each call (not a live list). Output events come from the layer thread, so they need
a compositor with the layer shell; outputs present at start are not "added". `App.simulate`
drives the handlers headless. `examples/outputs.py` runs on both a real session and headless.
Verified: `tools/verify.sh --cross` passes (1219 tests); live on KWin (HDMI-A-1, DP-1).

### #198, `LayerWindow` (2026-10-19)

Phase 2 of #195 (TRE-REQ-001 from Tesserae Shell), built on the spike's verdict (go on I2):
`tre.LayerWindow`, a `Window` whose Wayland role is a layer surface, for bars, panels, docks
and OSDs. Design in `docs/design/layer-window.md` ("As built" lists the differences), reference
in `docs/reference/layer-window.md`, guide in `docs/guide/bars-and-panels.md`, example
`examples/layer_bar.py`.

- *Platform* (`engine-platform`): `layer_config.rs` (the settings and the protocol's rules,
  validated with no compositor, 7 tests), `layer.rs` (the layer thread: a second event queue on
  `winit`'s `wl_display`; pointer, touch, keyboard, cursor shape, fractional scale, outputs by
  name), `layer_keys.rs` (the keymap through `xkbcommon-dl`, 3 tests), `layer_loop.rs` (the
  surfaces inside the loop, with ids minted from `WindowId::from(u64)`), `LayerHooks` for the
  host.
- *Python* (`engine-py`): `LayerWindow(namespace, *, layer, anchor, size, exclusive_zone, margin,
  keyboard, output)` extending `Window` (which gained `subclass`), `set`/`get`/`close`, the
  refused window keys, `UnsupportedPlatform`, `tre.supports("layer_shell")`, events `configure`,
  `output_changed`, `warning` and `closed` with a reason; the surface is made from raw
  handles (`GpuState` takes a `SurfaceSource`), drawn by `WindowRenderer` unchanged.
- *Checked on KDE Plasma 6.8 (RADV)*: a bar maps and a maximised window is exactly its
  `exclusive_zone` shorter; two bars (one per output) and an ordinary window run in one app; a
  bar added to a running app and closed; an idle bar draws 0 frames and uses 0.0% of a core; an
  unknown output raises from `run()` listing the real ones; with real pointer, scroll and keys
  (a virtual `uinput` device, `TRE_LIVE_INPUT=1`): enter, click, wheel, key and leave reach a
  node, `keyboard="none"` gets no keys, and a bar moved off the screen by a negative margin
  gets no pointer events.
- *Found on the way*: a first version crashed at exit because `winit` destroys its Wayland
  connection when the loop returns (shutdown now runs from `exiting`); and a layer frame
  asked for as "the OS requested it" never skipped an unchanged frame (52 frames in 5 idle
  seconds; now 0).
- *Tests*: `tests/test_layer_window.py` (35, headless), `tests/test_layer_window_live.py` (8;
  5 run on any Wayland session with the layer shell, 3 need `TRE_LIVE_INPUT=1` and
  `/dev/uinput`); `tools/verify.sh --cross` passes.
- *Not done*: `exclusive_edge` (protocol v5, the toolkit binds v4), IME and AccessKit on layer
  surfaces (follow-ups), `input_region`, outputs as objects, margin animation and drag, popups
  (phases 3 to 6 of #195), a 1.5x output, and a compositor without the layer shell (X11 and
  headless raise or build as documented).

### #196, the layer surface spike (2026-10-18 to 19)

Phase 1 of #195 (TRE-REQ-001 from Tesserae Shell). Verdict: **go, on I2**: layer
surfaces on a second event queue of the `wl_display` `winit` already owns. The full
findings are in `docs/design/layer-surface-spike.md`; the prototypes stay on the
unmerged branch `layer-surface-spike` (`spikes/layer-surface/`). Short version: a
layer surface maps and keeps its exclusive zone; `WindowRenderer` already draws on any
surface and drew a real tre scene on it; real pointer, scroll and keyboard events
reached `Tree::dispatch`; a `winit` window and a layer surface shared one connection
with no deadlock, both seats delivering every surface's events, so each side must
filter by surface; negative margins, a 1 px reserver and a `-1` painter behave as the
request needs on KDE Plasma 6.8. Not shown: touch, IME, cursor shape, AccessKit, a 1.5x
output and output removal.

### #132, touch: rotation, swipe, the slops (2026-10-18)

Built and tested with simulated fingers (as #113 was), the recognizer's arithmetic in
`engine-core/src/gesture.rs` (6 new unit tests) and the events in
`tests/test_touch_extras.py` (13).

- *Momentum:* nothing to add. A pan that lifts mid-flick already coasts the scroller
  under it (0.5.4: an exponential decay from the release velocity), and an app that
  takes the pan gets `velocity_x`/`velocity_y` on `ended` to do its own.
- *Rotation:* a `rotate` event (`rotation`, `rotation_delta` in radians, positive
  clockwise on the screen) from the same two fingers a pinch uses, once the line
  between them has turned past 0.12 rad (about 7 degrees); the first event carries
  the whole turn so far so the steps add up. A turn across the +/- pi branch cut is a
  small step. It comes with the pinch; a pure pinch sends none.
- *Swipe:* a `swipe` event (`direction` by the larger axis, `velocity_x`/`y`,
  `total_x`/`y`) right after a pan's `ended` when it lifted at 600 px/s or more after
  40 px or more. `GestureConfig` holds the three thresholds.
- *Slop:* `window.set(touch_slop=...)` (get too; default 10) sets the tap/long-press
  wander in layout pixels. Layout pixels are already device-independent (a scale
  factor of 2 does not shrink them), so the issue's "physical size" is only
  reachable with a monitor size, which winit 0.30 does not give; the setting is the
  app's lever for dense screens. Documented as a limit.
- *Touch-target slop:* `window.set(touch_target_slop=...)` (default 0, off). A touch
  that lands off every listening node (`click`, `pointer_down`, `tap`, `long_press`,
  `pan`, `swipe`, `pinch`, `rotate`, `touch_start`) goes to the nearest listening
  node whose layout box is within the slop; its touch events and the pointer press
  that stands in for the finger go there (the press at the nearest point of its box,
  while the finger stays within the slop), so a near miss clicks. A touch on a
  listening node is never pulled to another. The box is the layout box, before any
  transform. Off by default so nothing changes for existing apps.
- *Not run:* a real touch screen (none on the dev machine); Windows touch and the
  macOS trackpad pinch are #187 and #188.

### #133, OS file drag and drop on X11 (2026-10-18)

`tools/xdnd_source.py` is a scripted XDND (version 5) drag source over libX11 and
ctypes: a separate X client that drags files onto a named window the way a file
manager does, so the path is exercised with nobody holding the mouse. Against a live
tre window on XWayland (KDE Plasma, CachyOS):

- *Hover, cancel, drop:* a drag of two files is one `file_hover` with both paths, an
  abandoned drag (XdndLeave) is one `file_hover_cancel`, a drop is one `file_drop`
  with both paths; moving the drag over three points does not repeat the hover.
- *Position:* none. winit 0.30's X11 backend reads the XdndPosition messages itself
  and does not surface them, and the pointer does not move during a drag, so tre's
  position for a file event is the pointer's last, wherever it was before. A drop
  pointed at a node's middle reached the window's listener and not the node's.
  Documented in the events guide (window-level listeners are the way on X11); a fix
  needs the drag's own coordinates from winit or a raw XDND handler (filed as #189, a
  Backlog issue).
- *Tests:* `tests/test_file_drop_x11.py` (3; need an X display, skip without) covers
  the multi-file hover and drop as one event each, the cancel, and pins the position
  limit so it fails when it is fixed.
- Not run: Wayland (winit 0.30 delivers no file events there), and Windows and
  macOS, which are #187 and #188.

### #134, the OS reduced-motion and contrast preferences on Linux (2026-10-18)

KDE Plasma 6.8's portal (`xdg-desktop-portal-kde` 6.8.0, settings portal version 2)
publishes `org.freedesktop.appearance` `contrast` and `reduced-motion` (both 0 here,
and `window.get` agrees). Found by writing the test: the watcher listened only to the
appearance namespace, so on a portal older than those keys (GNOME's
`enable-animations` and `high-contrast`) the reads fell back but a live change never
arrived. It now watches every `SettingChanged`, and the GNOME keys count only where
the newer ones are absent, as in the reads.

`tests/test_os_preferences_live.py` drives a real window through a stand-in portal
(`tests/fake_portal.py`, PyGObject) on a throwaway `dbus-daemon`, so no desktop
setting is touched: the reads from a new-key portal, from a GNOME-only portal and
with no portal (`None`); live events from the new keys (an unchanged value says
nothing); live events from the GNOME keys (fails on the old watcher); and the GNOME
keys not overriding a portal that has the new ones. The event cases need a display,
the rest only the bus; all skip without `dbus-daemon` or PyGObject.

Not done: flipping the real setting in System Settings, which changes the
maintainer's desktop. The manual check is `python examples/accessibility_preferences.py
--watch`, then change the setting and watch the card; Windows and macOS are #187
and #188.

### #91, the shader check under X11 on the real GPU (2026-10-18)

Same machine as the Wayland checks (AMD Radeon 890M, RADV, Mesa 26.2.4, KDE Plasma
on CachyOS; llvmpipe the software adapter), run in an XWayland window
(`env -u WAYLAND_DISPLAY DISPLAY=:0`, the process maps libvulkan_radeon and the
xcb/X11 libraries). A native Plasma X11 session was not tried; XWayland is the X11
path Mesa uses here. `tools/check_shaders_live.py` is the new live script.

- *Pixel tests:* `shader_fill` 28/28 on RADV and on llvmpipe (offscreen, so the
  window system does not enter it).
- *The examples, RADV against llvmpipe under X11:* `shader_panel` 36.2% of pixels
  by one level, `shader_shine` 0.14%, nothing larger: the same as under Wayland.
  Both examples run live on RADV under X11 and exit clean.
- *Video input:* an `image` node whose pixels are replaced 30 times (the video
  case, see the painting guide) feeds a shader; every frame's snapshot shows that
  frame's colour (30 of 30).
- *Effect over children across resizes (X11):* equal to a direct draw (0 differing
  pixels) at 560x400, 700x460 and 820x520, forced by raising the minimum size.
- *CPU:* a still shader and an empty window idle at 0.0 to 0.2% of a core with no
  frames drawn; an animated one 3.3 to 3.9% at 60 frames a second, the same on X11
  and Wayland. (`App.run(max_frames=...)` runs flat out, so an idle measurement must
  not use it: it read 99% until that was removed.)
- *Banding:* the checklist wording does not fit the Julia example. Its colour steps
  are the shader's own integer escape count; `julia.wgsl` now says so.
- Not run: a real `gpu_stalled` or `gpu_lost` (can freeze the compositor), and a
  native X11 session. Windows (DX12) and macOS (Metal) are #187 and #188.

### #158: the burst of `wp_fifo_v1` barriers after a resize (2026-10-18)

Finding: not a bug. `RUST_LOG=engine_py=debug` now logs `resize pacing began`
(with the size) and `resize pacing settled`. A sweep of real size requests on KDE
Wayland (RADV, Mesa 26.2.4; the window's minimum size raised every 16 ms for 1.5 s,
traced with `WAYLAND_DEBUG=1`) showed one `began`, one `settled` 0.4 s after the
last resize, and one swapchain reconfigure at the restore. During the sweep there
were 0 barriers (Mailbox); at the restore three `set_barrier`/`wait_barrier`
commits landed 0.2 ms apart, then one every 16.7 ms: the new vsync swapchain's
images being presented as fast as the loop could fill them, until the fifo
throttles. That is the swapchain's frame depth, not the restore flipping back and
forth. The 12 in the Tesserae run is the same effect on a deeper queue or a busier
frame. A test pins that each sweep ends in exactly one restore and a resumed sweep
starts a fresh cycle. Not run: an interactive compositor-driven resize (no way to
inject pointer drags on Wayland here).

## 0.5.7 (released 2026-10-17)

Milestone 0.5.7: #168, #169, #175 to #179 and the Linux half of #91. It was built
as 0.5.6.1 and then 0.5.6.2, small releases with a fourth number, until it held
five features; it ships as 0.5.7 and the working labels were renamed (98 uses).
The `0.5.6 (#NNN)` stamps in the code are 0.5.6's own and were left alone.

**#169, step 1 (2026-10-10).** The fullscreen-ordering theory in the scope was
wrong: on COSMIC Wayland a plain window does not grow to a larger minimum
either, because winit's Wayland `request_inner_size` only resizes when the
compositor's last configure is not maximized, fullscreen or tiled, and
otherwise answers `Some(current size)`. `grow_to_minimum` took that `Some` for
"applied" and reported a resize, whose `Resized` ran it again: about 60 calls a
second while the window stayed below its minimum (0.05 s of CPU in 2 s,
against 0.00 with the fix). It now reports only when the size changed.

**#169, steps 2-4 (2026-10-10).** Why winit declined, decoded off the Wayland
socket: COSMIC's `xdg_toplevel.configure` for the window carries
`tiled_left, tiled_right, tiled_top, tiled_bottom` (then `activated`) even with
auto-tiling off, and winit never resizes a tiled window. COSMIC also did not
answer `set_min_size(400, 300)` with a larger configure, so on COSMIC a minimum
above the window's size is never reached. X11 (via XWayland) differs: a plain
window does grow to the minimum, and leaving fullscreen keeps the fullscreen
size (1920x1080) rather than restoring 300x200. So the live test's exact
`400x300` was wrong on two counts. It now asserts "at least the minimum", and
skips (a probe in a fresh interpreter) where the compositor holds the size.
The contract is written down in `docs/reference/window.md`. Passes on X11;
skips on COSMIC Wayland; KDE Wayland is untested (not available here).

## Releases

| Version | Date | What |
|---|---|---|
| 0.5.0 | 2026-09-30 | Custom windowing (#28): undecorated windows, window controls and state events, drag and resize regions, the window menu, macOS's overlay title bar, the Custom Title Bars guide |
| 0.5.0.1 | 2026-09-30 | A real click in a text field no longer panics (#51) |
| 0.5.1 | 2026-10-01 | WGSL shaders (fill, effect, sampling, animated; `tre.Shader`), `padding` on every kind, GPU health reporting, idle windows release the GIL (#92) |
| 0.5.2 | 2026-10-02 | Python 3.12 is the floor (wheels for 3.9-3.11 and PyPy dropped); docs reorganised, Rust and Python API pages |
| 0.5.3 | 2026-10-02 | A text node's explicit width is laid out rounded up (#98) |
| 0.5.4 | 2026-10-05 | The performance baseline and a round of speed work (damage walk, glyph cache, scroll reuse); transparent windows, blur, click-through; SVG nodes; gradients; spring easing; sticky positioning; custom cursors; selectable and linked text with accessibility runs |
| 0.5.5 | 2026-10-06 | A window being resized presents without a vsync barrier (#155); `configure_ms` frame stat |
| 0.5.6 | 2026-10-09 | Text input options and IME (#162, #165); animatable layout properties and `animation_end` (#161); `mask` (#164); accessibility states (#160); `Window.after`/`every` timers (#163); more `Window.set` keys, `center()`, and `add_window` while running (#159); `scroll_snap` and `snap_align` (#166); set-up from a fresh machine (`requirements-dev.txt`, `tools/verify.sh`, contributing guide), the project's Claude Code skills, and the intake routine write-up |

## 0.5.0 history (custom windowing, 2026-09-30)

Branch `0.5.0` off `main` at `a6f1853`. winit 0.30.13 was surveyed first:
decorations off everywhere; `drag_window` everywhere (right after a press);
`drag_resize_window` not on macOS; Windows undecorated shadow;
`show_window_menu`. M1 decided the design page (`d734290`); tracking then
moved to the GitHub project (milestone `0.5.0`, #31-#36, phases #37-#41).
M2 (#32, #37): undecorated windows, minimize/maximize/restore/close,
fullscreen, minimum size (grown to after every resize, for Wayland), icon,
platform, live-checked on KDE Wayland. M3 (#33, #38): drag regions, the
resize border, a double-click on the title bar, an opt-in window menu. M4
(#34, #39): macOS's overlay title bar. M5 (#35, #40): the example, docs and
upgrade page. M6 (#36, #41): the release. The step-by-step trail is in git
history for this file and in `BUILD_TRACKER_ARCHIVE_0.5.md`.

## 0.5.7, continued: the features

The rest of the 0.5.7 work: the features, and the repeat of the Linux GPU check
(#91). The fixes are in the first block, above; the Releases and 0.5.0 history
sections ended up between the two.

**#175, bitmap colour emoji (2026-10-11).** #168 left a bitmap emoji (CBDT on
Linux, `sbix` on macOS) drawing nothing, with three ways forward listed. None
was needed. glifo has two bitmap paths: the uncached one gives `vello_gpu` an
`ImageSource::Pixmap`, which it panics on (still so on upstream `main`,
2026-10-09); the atlas one (`atlas_cache(true)`) gives it an `OpaqueId`, which
it supports. #168 had tried glifo's `png` feature only without the atlas. With
`png` on and the glyph cache on, the CBDT emoji drew in colour (511 coloured
pixels of 683) in the first try; with the cache off it still panicked.

The fix: `png` on, `skrifa` as a direct dependency (the version glifo already
pulls in), and `atlas_for` in `text.rs`: a glyph run whose font has bitmap
strikes always draws through the atlas, at all four `fill_glyphs` sites (labels,
both text-field paths, the terminal); every other font follows `set_glyph_cache`
as before, so outline and COLR text is pixel-identical. The text-field sites had
no atlas call at all, so with `png` on an emoji typed into a field would have
panicked, not drawn nothing: they are covered by the test.

`colour_emoji_bitmap.rs` was the "draws nothing" pin; it now asserts colour with
the cache off (the case that panicked), the same pixels with it on, colour at
2x, colour in a plain and a selected text field, and that Latin text is
unchanged by registering the bitmap font. On this machine's real Noto Color
Emoji the `system_fonts` ink went from `emoji 184/0` to `184/937`. `sbix` is the
same code path but is unchecked without a Mac.

**#176 and #177, text that fits its content (2026-10-14).** The two issues had
one cause. A text node has no size of its own (`docs/guide/text.md` said so):
`compute_layout` ran taffy with no measure function, so an unsized text node is a
leaf of zero content size. #177's centred label was 0 wide and painted one word to
a line; #176's text with `padding=12` was a 24 px box, and an effect shader's
texture is the node's box, so it cropped 1003 bright pixels to 126 (with an
explicit size the effect pass handled padding correctly: 1003 against 1003). With
no padding the box is 0 wide and the effect is skipped, which hid it.

The decision was opt-in, so no existing layout moves: `width="fit-content"` and
`height="fit-content"` on a text node. It has to be a new value: an unset size
already reads back as `"auto"`, so "auto means fit" could not be told from "never
set". `engine-core`: `Node.fit_content` (`FitContent { width, height }`),
`Tree::set_text_measure` and `set_fit_content`, and `run_taffy`, which runs taffy
with a measure function (text leaves carry their own `NodeId` as taffy's node
context). A fit width is the text on one line, narrowed to the offered space when
it wraps (min-content is the longest word); a fit height is the wrapped text's,
at the width the node got; both rounded up as #98 does. `engine-py` installs
`window.measure_text`'s shaper as the measure, parses the value (any other value
clears it) and lays the node out again when its text, font or wrapping changes.
Stretch still fills the box, as in CSS, so #177's `stretch` row is 440 wide, one
line; centre and flex-start are the text's 299.

Tests: `engine-core/tests/fit_content.rs` (a stand-in measure: the three
alignments, wrapping, no-wrap, the legacy zero, a text change) and
`tests/test_text_fit_content.py` (against `measure_text`, a passthrough effect over
padding 0, 12 and 30 equal to a direct draw). `shader_shine.py` and
`shader_panel.py` use it and now assert their layout, so CI would catch this.
Not done: #176's second note (a passthrough effect over a box with children
differs by about 0.4% of pixels at 300x140), which this does not explain.

**#178, insets, mixed-unit sizes and a layout event (2026-10-14), from Tesserae
0.5.0.8 (#261).** All three of what it asked, none deferred.

*Insets.* `left`, `top`, `right`, `bottom` and `inset` on a node (`x` and `y` are
`left` and `top`). A taffy-level probe showed the engine already did the work: an
absolute child with `left 8, right 8` and a `auto` width is 284 in a 300 parent
(x=8), margins shrink it (276), percentages work (`10%`/`10%` is 240), a width
that is not `auto` beats the second inset (`100%` stays 300), and the insets are
from the parent's edge, not its padding. A relative node with `right` is offset
left, as in CSS. Only the property names were missing.

*Mixed units.* `width`/`height` take `"calc(100% - 16)"`, `"100% - 16"`, `"33% +
10"`. taffy 0.14 has a `calc` feature but `TaffyTree::resolve_calc_value` is a
stub that returns 0.0, so a calc size would silently be 0; the tree resolves it
itself (`Node.calc_size`, `Tree::set_calc_size`): the side starts `auto`, and
after taffy lays out the parent `settle_calc_sizes` writes
`percent x base + px` into the style and lays out again, up to 4 times. The base
is taffy's own for a plain percentage, measured: the parent's content box for a
node in the flow (280 inside `padding=10`), its padding box for an absolute one
(300). A side still changing after 4 passes has no fixed point (a row as wide as a
child that is the row less 16): it goes back to `auto` with a warning and is
listed by `take_calc_conflicts`, so it cannot drift every frame as Tesserae
feared. Reads back as `"calc(100% - 16px)"`; a lone percentage or pixel count is
the plain size. Not for `min_*`/`max_*`; in a grid it is taken of the container.
engine-core gains a `tracing` edge (already in the lock through engine-render).

*Layout event.* `node.on("layout", ...)`: `Tree::watch_layout` and
`take_layout_changes` compare a watched node's last computed size with the last
reported, as `take_scroll_changes` does, so any cause is seen (a `set`, a parent
resize, a calc size resolving once at its final size, new text). A node waits
until it has a computed layout (taffy hands out a zero placeholder otherwise,
which reported a bogus 0x0 in the first test run). `event.width`/`height`,
`old_value` (`None` first) and `new_value`; fired after layout in `advance` and
the frame loop, not from the `layout_x` getter; does not bubble.

Tests: `engine-core/tests/calc_size.rs` (10), `layout_watch.rs` (6),
`tests/test_layout_insets.py` (29), `test_calc_size.py` (24),
`test_layout_event.py` (11).

**#179, anchored layout (2026-10-15), from Tesserae 0.5.0.8 (#262).** Option 3
of the scope: the layer and an ordinary node share one placement model.

*Findings.* Layers already did what an in-tree node needs: `place_layers` runs
after layout, reads the anchor's window position, writes the layer's `inset` and
lays out again. An absolute node does not change its anchor's layout, so there is
no cycle with it and the same pass can place an ordinary node; positions go
through `composed_transform` (scroll offsets, scale and rotation included), so the
arithmetic stays right under a scrolling or transformed ancestor, which was
Tesserae's worry about doing it from Python.

*Model.* `Placement` gains `TopStart`, `TopEnd`, `BottomStart`, `BottomEnd` and
`Center`, which name a *point* of the anchor and put the node's centre there; the
four sides are as before. `Placement::origin` is the one piece of placement
arithmetic, and an `AnchorBox` is the bounds of the anchor's laid-out box through
its transform. `offset` moves the attachment point **in the anchor's own
coordinates** (a scaled anchor scales it): the user's reading, which I had first
described as a shift of the result; for an untransformed anchor they agree. A
point is never flipped or shifted to fit; `OverlayMeta.pinned` (`flip=False`)
stops a side doing it too. The default is unchanged, so every existing layer test
passed untouched.

*Layers.* `show_layer(..., offset=(dx, dy), flip=True)` and the five new names.

*In-tree.* `Node.anchored` (`Anchored { anchor, placement, offset }`),
`Tree::set_anchor` (makes the node absolute; `None` puts it back in the flow),
`anchor_cycle` (an anchor that is the node or inside it is refused), and a
`settle_anchored` pass beside the calc settle in `compute_layout`: it runs every
call, not only after a taffy pass, because an anchor moves with a scroll or a
transform without any layout; at most 4 passes (a chain needs one a link); two
nodes anchored to each other have no fixed point, so they are put back in the flow
with a warning (`take_anchor_conflicts`). The window-space target is mapped into
the parent's space with the inverse of its composed transform, less the node's own
scroll shift. Python: `anchor`, `placement` and `offset` properties (a node, a
layer-placement name, a pair); `check_anchor_changes` validates a `set` and a
`create` alike (the first version only checked `set`, which a test caught).
Deliberate gap from the proposal: an in-tree node has no `flip`, since it has no
bounding region to flip within; layers cover menus.

Tests: `engine-core/tests/anchored.rs` (9), `tests/test_layer_points.py` (12),
`tests/test_anchored_layout.py` (25).

**#91, the Linux real-GPU check, repeated on the CachyOS install (2026-10-16).**
The 2026-10-10 check (Pop!_OS, COSMIC) was done on the same hardware; this one is
on KDE Plasma Wayland with newer drivers: **AMD Radeon 890M (RADV STRIX1), Mesa
26.2.4**, kernel 7.2.10; the software adapter is llvmpipe (LLVM 23.1.1). The test
support code now names the adapter once per process and
`TRE_TEST_FORCE_FALLBACK=1` selects the software one (carried over from the
`gpu-check-91` branch, which also holds the first run's notes); from Python,
`VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.json` (or `lvp_icd.json`)
pins the adapter.

- *Pixel tests:* the whole `engine-render` suite, 324 passed, 0 failed, 12
  ignored, on RADV **and** on llvmpipe; `shader_fill` 28/28 on both. (#168's emoji
  failure of the first run is gone.)
- *The examples, RADV against llvmpipe, offscreen:* no pixel differs by more than
  one level (`shader_panel` 34 to 36% of pixels by one level, `shader_shine` 0.1
  to 0.2%). The fractal fills the rounded panel, clipped at the corners, with its
  label on one line; `shader_shine` shows its text (#176, #177). Both run live on
  RADV and exit clean.
- *The shine band crosses glyphs only where there is text:* measured against the
  same scene with no shader at eight clock times, 0 background pixels change at
  any of them, and glyph pixels brighten (2631 at t=1.0, 6038 at 1.5, 4566 at 2.0,
  326 at 2.5, 0 at 3.0, 3669 at 4.0).
- *Banding:* the fractal's flat colour steps are the shader's own integer
  escape-count colouring (`julia.wgsl`: `m = steps / 80.0`), the same on both
  adapters; the checklist's "no banding" does not fit it as written.
- *Inputs and effects, in a live window on RADV:* an image input shows the image;
  replacing its pixels repaints it; a shader reading another shader's output shows
  it and repaints too. An effect over a node with children, through 420x240,
  600x320, 300x180, 800x480 and back, matches a direct draw (0.00% of pixels, max
  difference 0) and is identical to its first render when the size returns. (#176's
  second note, 0.4% of pixels, did not reproduce in this scene.)
- *Idle (5 s, start-up included):* no shader 1.0% of a core and one frame; a still
  shader the same; an animated one 7.8% at 60 frames a second.
- *GPU health (#65):* `simulate("gpu_error")` and `simulate("gpu_lost")` reach
  their listeners, which is all the simulation promises. A real stall or device
  loss was **not** run: it can freeze the compositor. The kernel's GPU hang timeout
  here is amdgpu's default (10 s, recovery automatic).

Left under #91: Windows (DX12) and macOS (Metal); a video node as an input
(nothing here to feed one); and the real stall and loss checks.

## Verify on a fresh machine (2026-10-10)

`tools/verify.sh` was run on a new Pop!_OS 24.04 box (Wayland, Rust 1.98.0,
Python 3.12) after installing rustup and the system packages. rustfmt,
clippy, maturin, the generated `docs/api/python.md`, stubtest, mypy
`--strict` and `mkdocs --strict` pass. Rust tests: 704 passed, 14 ignored,
1 failed. pytest: 1021 passed, 1 skipped, 1 failed. Both failures are in
paths CI skips (it has no emoji font and no display), and neither comes from
0.5.6:

- **#168:** `engine-render` `system_fonts` — with Noto Color Emoji installed
  the emoji render with ink but no colour (`system_fonts.rs:113`).
- **#169:** `test_live_fullscreen_and_a_minimum_larger_than_the_window` —
  the minimum comes out 300x200, expected 400x300, on Wayland.

Both are in the Backlog. One set-up gap (#171): the guide's apt line
(`docs/contributing.md`) lacked `python3-dev`, without which the `engine-py`
tests fail to link (`unable to find library -lpython3.12`). Fixed on branch
`docs-python3-dev-171`; CI's package list needs no change, since its runners
ship the Python headers.

**#168, found (2026-10-10, 0.5.7).** The scope read the test's `off/on`
output backwards: with system fonts on, the emoji draws *nothing* (0 inked
pixels; 184 with them off is the missing-glyph box). The machine's Noto Color
Emoji is a CBDT bitmap font, and at the pinned Vello revision (`b408cd00`) a
bitmap glyph cannot be drawn on the GPU: glifo decodes bitmap glyphs only with
its `png` feature, and with it on `vello_gpu` panics ("pixmap image sources
are not supported by Vello GPU", `wgpu/mod.rs:705`), so the feature stays off
and the glyph is skipped. COLR emoji do draw in colour (new
`colour_emoji_colr.rs`, with a registered 5.6 KB subset, passes). A new
`colour_emoji_bitmap.rs` pins the bitmap limit (no colour, no panic) so it
cannot change silently. `system_fonts.rs` now checks emoji colour only where
the machine's font draws, and `docs/guide/text.md` no longer claims bitmap
emoji draw in colour (it said so; that was wrong) and says to register a COLR
font. macOS's sbix is a bitmap format too: expected blank, not checked. The
real fix is upstream (a bitmap-glyph path in `vello_gpu`) or our own drawing of
the strike as an image: left out then, and done as #175 (above).
