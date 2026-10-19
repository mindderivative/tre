# LayerWindow: the design (#198)

Phase 2 of [#195](https://github.com/mindderivative/tre/issues/195) (TRE-REQ-001 from Tesserae Shell). **Decided 2026-10-19, built (#198).** The [reference](../reference/layer-window.md) and the [guide](../guide/bars-and-panels.md) say how it works; this page keeps the reasoning, and "As built" at the end lists where the build differs from the plan. It rests on the spike's findings (`layer-surface-spike.md`, #196): layer surfaces on a second event queue of the `wl_display` `winit` owns. Where the maintainer has to choose, the choice is marked **Decision** with a recommendation.

## What it is

A `LayerWindow` is a `Window` whose Wayland role is `zwlr_layer_shell_v1` instead of `xdg_toplevel`: the compositor places it (an edge, a stacking layer, a size, margins), not the user, and it can reserve screen space. It is how a bar, a panel, a dock, an OSD or a wallpaper is made. tre draws and drives it with the same pipeline as any window.

```python
bar = LayerWindow(
    namespace="tesserae-shell.bar",     # the protocol's namespace: what the surface is for
    layer="top",                        # "background" | "bottom" | "top" | "overlay"
    anchor=("top", "left", "right"),
    size=(0, 32),                       # 0 stretches between opposite anchors
    exclusive_zone=32,                  # pixels to reserve; 0; -1 (ignore other zones)
    margin=(0, 0, 0, 0),                # top, right, bottom, left
    keyboard="none",                    # "none" | "on_demand" | "exclusive"
    output="HDMI-A-1",                  # a connector name; None lets the compositor pick
)
app.add_window(bar)
```

Everything on `Window` that makes sense still works: `root`, `create`, `show_layer`, `after`, `every`, `on`, `get`, `set` for the shared keys, `simulate`, `advance`, `snapshot`, `close`. The constructor takes `namespace` and `output` for good; `layer`, `anchor`, `size`, `exclusive_zone`, `exclusive_edge`, `margin` and `keyboard` change live through `set()`, in one atomic commit.

### Refused, loudly

| Where | What raises |
|---|---|
| Constructor or `set()` | `size` with a 0 axis but not anchored to both opposite edges; an anchor outside the four edges; an `exclusive_edge` not among the anchors; an unknown `layer` or `keyboard` name. Raised in Python, naming the key, before any request reaches the compositor. |
| `set()` | `decorations`, `maximized`, `minimized`, `fullscreen`, `resize_border`, `system_menu`, a window-title bar: they mean nothing to a surface the compositor places. |
| Attaching a node | `window_region="drag"`: it calls `drag_window()`, which moves a toplevel. |
| Constructor | `UnsupportedPlatform` where there is no layer shell (X11, GNOME's Mutter, Windows, macOS). |

A positive `exclusive_zone` with an anchor set the protocol treats as zero is accepted by the protocol, so it produces a warning event, not an error.

### Events

`configure` (`width`, `height`: the compositor assigned a size; tre acks it and resizes the target; `resize` also fires), `closed` (`reason`: `"compositor"`, `"output_removed"` or `"app"`; after the first two the window is dead and the shell makes a new one) and `output_changed` (the output's name; the `Output` object is phase 3). A buffer is never attached before the first configure is acked: tre owns that sequence.

`tre.supports("layer_shell")` is `False` where the role does not exist.

## How it fits the code

The spike measured the seams; none is large.

- **The Python type.** `Window` is a plain `#[pyclass(name = "Window")]` (`PyWindow`). A probe showed that adding `subclass` to it and a `#[pyclass(extends = PyWindow)]` `LayerWindow` compiles and works: `isinstance(layer, Window)`, `root`/`create`/`advance`/`snapshot`, and `App.add_window` (which takes `Py<PyWindow>`) accepts the subclass. The probe was reverted. `LayerWindow` itself would not be subclassable from Python.
- **The platform loop.** `engine-platform` keys every window by `winit`'s `WindowId` (31 uses in `lib.rs`; 3 in `engine-py`, in one file) and opens them through `PlatformEvent::OpenWindow` and `winit`'s `create_window`. A layer surface has no `WindowId`. The change is a small id type in `engine-platform` that is either a `winit` id or a layer surface's, and a second branch where windows are opened.
- **The connection.** The `wl_display` comes from `winit`'s `ActiveEventLoop` (it implements `HasDisplayHandle`); a second `EventQueue` on it, wrapped with `wayland-backend`'s `from_foreign_display`, binds the layer shell. A thread dispatches that queue and posts what it hears through the existing `EventLoopProxy`, the way `appearance::portal::watch` does for the settings portal; drawing and the `wgpu` surface stay on the loop thread. Requests (`ack_configure`, `set_margin`, and the rest) are proxy calls and can be made from either thread.
- **Drawing.** `WindowRenderer` is already independent of the window; only `GpuState::new` and `recreate_surface` in `engine-py/src/app.rs` take a `winit` window, to make the surface. A layer surface builds its `wgpu` surface from raw handles (`create_surface_unsafe`), as the spike did, and the renderer is used unchanged.
- **Input.** A second seat delivers pointer, button, scroll and key events to the layer side. Both seats receive every surface's events, so every handler first checks the event's surface is its own. Key repeat is tre's own timer (the toolkit's needs a second event loop); the cursor comes from `wp_cursor_shape_v1`; touch is written to the protocol and untestable on the dev machine; IME (`zwp_text_input_v3`) comes last and may be its own issue.
- **Headless.** A `Window` with no OS window already runs `simulate`, `advance` and `snapshot` with no display. A `LayerWindow` constructed where there is no layer shell would raise, so the headless path needs a way to build one for tests: the constructor takes the validation and state without the surface, and the surface exists only once `App.run()` opens it on a compositor that has the role.

## Decisions (the maintainer accepted all five recommendations, 2026-10-19)

1. **A subclass of `Window`, or one `Window` with a role.** *Recommendation: the subclass (shown to work).* It keeps `Window`'s signature unchanged and `LayerWindow`'s extra keys honest; a role flag would put layer-only keys on every window and make most `set()` errors a runtime role check.
2. **Raise `UnsupportedPlatform`, or degrade to a plain window.** *Recommendation: raise.* A bar that quietly lacks its exclusive zone is a worse failure than a clear error; the shell can build its own fallback from `Window`.
3. **The name `layer=`.** *Recommendation: keep the protocol's word* (the shell's other tools use it) and define both senses in the glossary: tre's in-window `show_layer` overlay, and the compositor's stacking layer.
4. **AccessKit and IME in this phase, or follow-ups.** *Recommendation: follow-ups for both,* each as its own issue with its own check (`accesskit_unix` for AccessKit; `zwp_text_input_v3` for IME). The core is usable without them.
5. **`output=` as a connector name now, or wait for phase 3's `Output` objects.** *Recommendation: a name string now.* The compositor's pick varied between spike runs, so a bar that can only be placed by the compositor is not useful, and a name needs only `wl_output` v4, which KWin offers.

## Steps (from #198)

1. This page and the decisions above. 2. The platform layer (second queue, thread, id type). 3. Surface and render. 4. Input. 5. Validation and events. 6. Capability and headless. 7. Tests (headless pytest; live KWin tests that skip elsewhere, checking the zone by maximising a tre window, keyboard modes, and input through a virtual `uinput` device when writable). 8. Docs, a runnable example, LOG.

## As built

Where the build differs from, or settled, what the sections above said.

- **Ids.** A layer surface has no `winit` `WindowId`, and instead of a new id type in every
  callback (the plan above), each surface is given a `WindowId` minted from the high-bit range
  of the `u64`s `winit`'s `WindowId` converts from. The ids `winit` makes on every platform are
  pointers or small numbers and never set the top bit. The loop's callbacks kept their
  signatures and a layer surface is one more window to them (`engine-platform/src/layer_loop.rs`).
- **No `smithay-client-toolkit` keyboard.** Its keyboard module needs the `xkbcommon` crate,
  which links libxkbcommon when built; a wheel must not depend on that library to import. The
  keymap is handled with `xkbcommon-dl` (already in the build through `winit`; it loads the
  library at run time), and key repeat is the layer thread's own timer. `sctk` 0.19 is the
  version already in the lock.
- **Shutdown order.** `winit` destroys its Wayland connection when the loop returns, so the
  layer side is shut down from `ApplicationHandler::exiting`: the host drops its `wgpu`
  surfaces first, the layer thread then destroys the Wayland surfaces and ends. A surface the
  compositor closes is destroyed only after the loop has dropped its `wgpu` surface.
- **Availability.** `LayerWindow(...)` raises `UnsupportedPlatform` where there is a display but
  no layer shell. With no display at all (`WAYLAND_DISPLAY` and `DISPLAY` both unset) it is
  built headless, so a CI run can drive one with `simulate`, `advance` and `snapshot`, and
  opening it needs a compositor. This is how decision 2 and the headless requirement fit.
- **`exclusive_edge`** is validated but refused: the protocol request is from version 5 and the
  toolkit binds up to version 4. A follow-up if a corner-anchored reserving surface is needed.
- **`output=`** is a connector name resolved through `wl_output` v4; `output_changed` carries
  the name. The `Output` objects and hot-plug events stay in phase 3.
- **Defaults.** The root has no padding or gap; layout is logical with the surface's scale
  (`dpi_scaling` on); the surface is transparent.
- **Not done in this phase:** IME (input-method preedit) and AccessKit on layer surfaces, each
  its own follow-up as decided; the frame-callback-driven redraw (a surface draws when something
  changed, paced by the swapchain's present, as a window).
