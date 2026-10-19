# Layer surface spike (#196)

What the spike (phase 1 of [#195](https://github.com/mindderivative/tre/issues/195), TRE-REQ-001 from Tesserae Shell) found, on one machine: KDE Plasma 6.8 on Wayland, AMD Radeon 890M (RADV, Mesa 26.2.4), two 1920x1080 outputs at scale 1, `winit` 0.30.13, `wgpu` 30.0.1, `smithay-client-toolkit` 0.21.1, `wayland-client` 0.31. The prototypes (`spikes/layer-surface/`, branch `layer-surface-spike`, not merged) are throwaway; this note is what to keep.

## Verdict

**Go, on I2**: add layer surfaces to the process's existing Wayland connection (the `wl_display` `winit` already owns), on a second event queue. Nothing in the spike argues for I1 (a connection of its own), and nothing argues for stopping at TRE-REQ-002. I3 (replacing `winit`) stays out of scope.

I2 worked end to end: a `winit` window and a layer surface lived in one process on one `wl_display`, both mapped, both took input, and nothing deadlocked or crashed. The cost is not the connection; it is the second *input path* the request already warned about, and the spike measured how big it is (below).

## What was shown

| Step | Result |
|---|---|
| 1. Layer surface and `wgpu` | A `top`-layer surface (anchor top/left/right, 0x32, exclusive zone 32) maps on its own connection. The `wgpu` surface comes from the raw Wayland handles (`create_surface_unsafe`) and runs on RADV. Surface capabilities: formats include `Bgra8Unorm`/`Rgba8Unorm`, alpha `Opaque` and `PreMultiplied`, usages include `COPY_DST` (the persistent-target copy works). A maximised window is 32 px shorter with the bar up (1052 to 1020, and 1022 to 990 on the other output): the exclusive zone is respected. |
| 2. tre's pipeline | `WindowRenderer` (`engine-render`) drew a real tre scene (a container, a rounded rect, a text node) on the layer surface and read back correctly. It takes a device, queue, texture and a `Tree`: it is already window-agnostic. Only `GpuState::new` and `recreate_surface` in `engine-py/src/app.rs` assume a `winit` window (to create the surface). |
| 3. Input | Real events, through a virtual `uinput` device (the spike's only way to produce them): pointer enter/motion/press/release with exact surface-local positions, scroll (15 px a notch, as `Pixels`), keyboard text. They fed `engine-core`'s `InputEvent` and `Tree::dispatch`: hover changed, a click returned `Activated`. Keyboard interactivity behaves as the protocol says: `none` gets no key events, `on_demand` gets focus when the surface is clicked, `exclusive` takes focus on map. |
| 4. Shared connection (I2) | `winit`'s `HasDisplayHandle` gives the `wl_display`; `wayland-backend`'s `Backend::from_foreign_display` wraps it; a second `EventQueue` on it binds the layer shell. Both sides drew through one `wgpu` device. See "I2 findings" below. |
| 5. Idle and clocks | An idle layer surface drew **1 frame** in 10 s and used 0.0% of a core (a 10 Hz poll in the spike's loop). With frame callbacks chained it ran at **60 a second, steadily** (longest gap 18 ms), including while a fullscreen window was shown (see "Not shown"). Animation progress in tre is clock-driven (`Tree::tick_all(now)`), so it does not depend on callbacks; what must not depend on them is the loop's *wake-up*. |
| 6. KWin facts | A **negative margin** (`-32` top) is accepted with no protocol error and puts the surface above the screen edge (no pointer event reaches it at y 3 to 25). A **1 px surface with an empty input region and zone 32** keeps its zone (1052 to 1020). A **full-output surface with zone `-1`** reserves nothing, and alongside a 32 px bar the maximised height is still 32 px less. With no output given, the compositor picked one (it alternated between runs), so `LayerWindow` must bind an output itself. `wl_output` is v4 here, so connector names (`DP-1`, `HDMI-A-1`) come with the output; `zxdg_output_manager_v1` is v3. |
| 7. Fractional scale | `wp_fractional_scale_manager_v1` and `wp_viewporter` bind, and `get_fractional_scale`/`get_viewport` work on a layer surface. At scale 1 the compositor sends no `preferred_scale`. A real 1.5 output was not available and its display settings were not changed. |

## I2 findings

- **Mesa already does this.** Its Wayland window system integration makes its own event queues on the application's `wl_display` (`mesa vk display queue`, `mesa image count query`, and others, visible in `WAYLAND_DEBUG`), so several queues on one display beside `winit` is how the stack already runs.
- **Two seats, both deliver.** The layer side binds its own `wl_pointer` and `wl_keyboard`; the compositor sends enter/motion/focus events for *every* surface of the client to *every* pointer and keyboard object the client holds. The layer side saw `pointer enter` for `winit`'s window ("our surface: false"), and its keyboard saw `enter` for it too. `winit` ignored events for the layer surface without incident. So each side must **filter by surface**; that is easy (`event.surface == layer.wl_surface()`), but it is the design rule for the whole second input path.
- **Focus is correct.** With `on_demand`, clicking the bar moved keyboard focus to it: `winit` reported `focused: false`, the bar received the keys and `winit` received none.
- **The second queue needs somewhere to run.** `winit` 0.30 has no way to add a file descriptor to its Wayland loop. The spike dispatched the layer queue on a thread (`prepare_read` then `poll` then `read`, which libwayland coordinates with `winit`'s own reads without trouble). The tre-shaped version is the pattern `appearance::portal::watch` already uses: a thread dispatches the queue and posts events through the existing `EventLoopProxy` as `PlatformEvent`s, and the loop thread draws. `ack_configure` and the other requests are proxy calls and can be made from any thread.
- **Dispatching needs a lifetime story.** The layer state (surface, `wgpu` surface) has to be created, resized and dropped on the loop thread while the queue state lives on the helper thread; the spike put all of it on one thread. A design must decide who owns what.

## What has to be written (either route)

`engine-platform/src/lib.rs` is 1,718 lines and the `winit` input translation is the part a layer surface cannot reuse (29 `WindowEvent` arms). For the second path:

| Area | State |
|---|---|
| Pointer, scroll, buttons, keyboard text, key names, modifiers | Shown working in the spike (about 150 lines to translate to `InputEvent`). |
| Key repeat | Not provided: `sctk` offers it only through `calloop` (`get_keyboard_with_repeat`), a second event loop. tre needs its own repeat timer on the loop. |
| Cursor shape | Not tested. KWin offers `wp_cursor_shape_manager_v1`; it needs binding and a mapping from tre's cursor names. |
| Touch | The seat here has no touch capability, so nothing was tested. |
| Text input and IME | Not tested. Plain keys gave UTF-8 text; preedit needs `zwp_text_input_v3` (KWin offers it) for the layer surface, as `winit` does for its windows. |
| Clipboard | Unaffected by the surface role: `arboard` is built without its Wayland feature, so tre's clipboard already goes through XWayland for every window. |
| Accessibility | Not tested. `engine-platform` uses `accesskit_winit`, which wraps `accesskit_unix` (already built here); a layer surface would use `accesskit_unix` directly. Needs its own check. |
| Fractional scale | Objects work; 1.5 untested. The renderer already takes a display scale (`WindowRenderer::set_scale`). |

## Not shown

- **Visible on screen, by eye or screenshot.** `spectacle` did not capture the bar. The evidence that it maps is the protocol trace (`configure`, `ack_configure`, `attach`, `commit`), the first frame read back from the surface texture (a purple bar with the tre scene), the shortened maximised window, and pointer events at the bar's pixels.
- **Frame callbacks while covered.** They kept coming at 60 a second while a tre window was fullscreen, but that window may have gone to the other output; whether KWin throttles callbacks to a layer surface hidden under a fullscreen window on the *same* output is not shown.
- **Output removal (`closed`)**, a hot-plugged output, a 1.5x output, touch, IME, cursor shape, AccessKit, KWin's own map and unmap effects (not observable from a client), the first KWin release with the protocol (documentation, not checked), and wlroots compositors.
- **How the injected input was made.** A virtual absolute pointer and keyboard through `/dev/uinput` (`spikes/uinput.py`), with the maintainer's agreement; no real device was touched.

## Revised plan for phases 2 to 6

Sizes are against the spike, with I2 as the route.

| Phase | Work | Size |
|---|---|---|
| 2. `LayerWindow` core | The second-queue thread and proxy plumbing; `LayerWindow` constructor and `set()`; validation before any request; `configure` and `closed` events; `tre.supports("layer_shell")`; a mock backend so `simulate`, `advance` and `snapshot` run with no compositor; render through `WindowRenderer`; input translation, key repeat and cursor shape; the surface-filtering rule. | L |
| 3. Outputs | `app.outputs()`, `output_added` and `output_removed`, explicit output binding (the compositor's pick varies). `wl_output` v4 names. | S to M |
| 4. `input_region` | Empty and rectangle lists are one `wl_region` each (shown cheap); `"auto"` from hit-test shapes is the real work. | M |
| 5. Margin animation and `drag_margin` | Margins are plain requests (negative accepted); animation is tre's existing clock. | M |
| 6. `PopupWindow` | Separable; not touched by the spike. The full-screen overlay alternative is easier to try given a working layer surface. | M to L |

Open before phase 2: text input (IME), touch, AccessKit and a 1.5x output each need a check on the real implementation; none changes the verdict.
