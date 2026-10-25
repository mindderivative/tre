# LayerWindow: what shipped for Tesserae Shell (TRE-REQ-001)

A hand-off for the Tesserae Shell side, written when issue #195 closed (2026-10-25) and
**corrected the same day** after the VRAM research (#207): `PopupWindow` (#223) and small GPU memory
blocks (#219) were added, and the overlay route for menus is no longer the recommendation. It says
what is in tre now (`PopupWindow` and #219 are on their own branches until merged to `main`), how each acceptance criterion of the request stands, what differs
from the request, and what was not tested. Everything here is **unreleased**: the
last release is 0.5.7; the next version number is not chosen. The reference is
[LayerWindow](../reference/layer-window.md) and [Outputs](../reference/outputs.md); the guides are
[Bars and panels](../guide/bars-and-panels.md) and [Menus from a bar](../guide/menus-from-a-bar.md).

## What you can build now

```python
bar = LayerWindow("tesserae-shell.bar", layer="top", anchor=("top", "left", "right"),
                  size=(0, 32), exclusive_zone=32, keyboard="on_demand", output=output)
```

- **`LayerWindow`**, a `Window` subclass: `layer`, `anchor`, `size`, `exclusive_zone`,
  `exclusive_edge`, `margin`, `keyboard`, `output`, `input_region` (`None`, `[]`, rectangles or
  `"auto"`), `click_through`; live through `set()`; `get()`, `close()`, `animate()`. Events:
  `configure`, `closed` (reason `"compositor"`, `"output_removed"` or `"app"`), `output_changed`,
  `warning`, `moved`, plus the window events.
- **Outputs:** `App.outputs()` and `Output` (name, description, logical position and size, scale
  factor, transform); `App.on("output_added" | "output_removed" | "output_updated")`.
- **Capability:** `tre.supports("layer_shell")`; `UnsupportedPlatform` where there is none (X11,
  GNOME's Mutter, Windows, macOS). With no display at all a `LayerWindow` can be built and driven
  headless (`simulate`, `advance`, `snapshot`), so shell logic is testable in CI.
- **Patterns:** a bar (one edge, or three, reserving space); a screen frame (a painter surface
  plus 1 px reservers, with `input_region="auto"` for the ring); a movable widget
  (`window_region="drag_margin"`, then save the `moved` margin); a slide-in panel
  (`animate("margin_top", -32, 200)`); a menu or flyout from a bar (a **`PopupWindow`**, below).
- **`PopupWindow`** (#223), a `Window` subclass whose Wayland role is `xdg_popup`, parented to a
  `LayerWindow` or to another `PopupWindow` (a submenu): `size`, `anchor_rect` (the button, in the
  parent's coordinates), `anchor`, `gravity`, `constraint_adjustment`, `offset`, `grab`. The
  compositor places it and flips or slides it to stay on screen; `configure` reports the final
  `x`, `y`. With `grab=True` a click outside the application, or Escape, closes it with `closed`
  reason `"dismissed"`. A click on another window of the same application reaches that window (the
  protocol's rule), so a menu that should close on it listens and calls `close()`. It costs a bar's
  worth of memory, not an output's. See [`PopupWindow`](../reference/layer-window.md#popupwindow).
- **Small GPU memory blocks** (#219): a window's device asks for small blocks by default, so a
  window holds tens of MiB of video memory, not 210. `TRE_GPU_MEMORY=performance` restores the old
  blocks. See [App](../reference/app.md) and [VRAM per window](vram-per-window.md).

## How it differs from the request

- **`PopupWindow` is built (#223), after a first hand-off that said it was not.** Menus first went
  the overlay route (#206): a full-screen transparent `LayerWindow` on the `overlay` layer with
  `input_region="auto"` and `keyboard="exclusive"`, showing the menu with `show_layer`. It works
  and is kept (it is the only route to the engine's own `show_layer` layers and tooltips across a
  whole output), but it is **not the efficient way to build a menu**: it maps an output-sized
  surface, and the engine flips the menu inside the output without knowing other panels' reserved
  areas. Build menus with `PopupWindow`; keep the overlay for what needs it.
- **`layer=` keeps its name** (decided on #195). If the shell needs its own name for it, use
  `shell_layer=` on the shell's side.
- **`output=None` is deterministic:** the output at position (0, 0), else the first by name (the
  compositor's own choice alternates between runs). Pass an explicit `Output` for every
  per-monitor surface anyway.
- **Targets KWin and wlroots compositors** (decided on #195). Tested on KWin 6.8.1 and sway 1.12.
- **Unsupported platforms raise** `UnsupportedPlatform` rather than degrade, as recommended.
- **`animate` moves margins only** (the four `margin_*`), as recommended; size and
  `exclusive_zone` are step changes. `set(margin=...)` while one runs cancels it.
- **`click_through=True`** is `input_region=[]` on a `LayerWindow`.
- **`input_region="auto"` follows animation** by only growing while something animates, then
  snapping exact; while a dismissible or modal layer is open it takes the whole surface.
- **Hit testing now follows `corner_radius` for every window** (a click in a clipped-away corner
  no longer hits the node), which `"auto"` needs. Masks and `clip_children` are still ignored.
  This is a behaviour change for existing apps too.
- **The note's question about masks:** box hit testing did not follow rounded corners or masks
  before; it follows rounded corners now, not masks.
- **`exclusive_edge`** is built (tre binds layer-shell v5 itself). A compositor with only v4
  still opens the surface and a `warning` event says the edge is ignored.
- **Drag deltas** come from the relative-pointer protocol, with a fallback to the surface-local
  position when the protocol is missing or the pointer is absolute (a tablet, a VM, a remote
  desktop); the fallback can overshoot a little while a commit is in flight.

## Compositor facts

- **Corner reservation works on KWin 6.8.1 and on sway** (corrected 2026-10-25). The first
  hand-off said KWin ignored `exclusive_edge` for a corner surface. That was wrong: with a
  `("top", "right")` surface, `exclusive_zone=28` and `exclusive_edge="top"`, a maximised window
  on KWin shrinks by 28 px (1012 to 984) at 1, 3 and 6 seconds; with `"right"` it narrows
  instead; with no edge nothing is reserved. The earlier "no reservation" reading is not
  reproduced and its cause is unknown (a timing or set-up fault in the first test run is the
  likeliest, not a KWin gap). Do not report it to KDE. The live test now runs on KWin.
- Negative margins are accepted and honoured on KWin (the spike).
- Output removal closes a bar with `closed` reason `"output_removed"` (live-tested on sway).
- A headless sway with no keyboard device gives no window keyboard focus (the tests add a virtual
  keyboard); a real session has one.

## Acceptance criteria

| Criterion | Status |
|---|---|
| Bar on KWin; a maximised window does not overlap | Met, live on KWin 6.8.1 (the request names 6.7) and sway |
| `set()` of zone or anchor in one commit, no wrong-size configure | One commit by construction; the absence of a wrong-size configure was not measured |
| Bad `size` raises before any request | Met (headless) |
| Unplug fires `closed` `"output_removed"`; the app continues | Met on sway with a real unplug; KWin not run |
| `output=None` binds the same output every run | Met on KWin and sway |
| Transparent bar over the desktop | Met on sway (pixels read back); KWin not run |
| Paste into a focused text field | Met on sway, with no X server; KWin not run |
| `outputs()` and hot-plug events | Met (KWin and sway for `outputs()`; added, updated and removed fire on sway) |
| `input_region=[]` passes clicks through | Met on KWin and sway |
| `"auto"` with a circle: inside hits, the box corner passes | Met on KWin and sway |
| Screen frame: reservers keep windows inside; ring takes clicks | Ring clicks met on KWin and sway; the four reservers plus painter together were not tested |
| `drag` refused; `drag_margin` moves a widget and fires `moved` | Met on KWin and sway |
| `animate("margin_top", ...)` runs in the engine; `advance(200)` finishes it headless | Met (headless; live slide on sway; the live check used a positive slide) |
| Two layer windows animating read the same timestamp | Met (`turn_instant`; headless test) |
| `animation_end` fires with frame callbacks stopped | Not tested; clock-driven by design, with an 8 ms alarm |
| Fade-out close keeps the surface mapped until the animation ends | Built (`close_requested` can be cancelled), not tested as a sequence |
| `reduced_motion` on a `LayerWindow` | Not checked |
| `on_demand` gets keys, `"none"` never does | Met on KWin (real injected keys) |
| `PopupWindow` opens, flips, dismisses | Met on sway (headless, with live tests): placement, flip above a bottom bar, outside-click dismissal, item click, submenu. **Not run on KWin**; Escape dismissal was not checked on a keyboard-less headless sway. The overlay route (dismissal on outside click and Escape on KWin and sway) remains as the alternative |
| Crisp text at 1.0 and 1.5 scale | The surface follows 1.5x (`scale_factor` reads 1.5 on sway); crispness was not inspected |
| `supports("layer_shell")` false and `UnsupportedPlatform` without the protocol | Met by the availability rule in headless tests; GNOME's Mutter was not run |

## Also done, beyond the criteria

- **Input methods (IME)** work on a layer surface's text input through `zwp_text_input_v3`: the
  preview and the commit, with the caret rectangle sent so a candidate window can be placed.
  Tested with a stand-in input method on sway; **no real input method (fcitx5, ibus) was run**,
  KWin's text-input path was not, and where a candidate window actually appears was not checked.
- **Screen readers:** a layer surface is published through AT-SPI as one more window of the
  application, with its nodes, actions and focus. Verified in a private AT-SPI environment with
  `busctl` as the reader; **Orca was not run**. A bar's window node has no accessible name yet.
- **Keyboard:** named keys, text, key repeat, clipboard shortcuts, compose and dead keys.

## Costs and limits to plan around

- **Video memory.** The research (#207) found the fixed cost was wgpu's 128 MiB device blocks, not
  the window. With the small blocks now default (#219) a window holds tens of MiB, plus about 16
  bytes per pixel (a full-screen overlay at 4K holds about 120 MB more than at 720p). Measured on an
  AMD RADV GPU only. A popup, being menu-sized, costs a bar's worth. Each window still has its own
  GPU device; sharing one across windows is a separate, larger item in the research page.
- Wayland cannot commit two surfaces atomically, so surfaces animating together can differ by up
  to a frame on screen.
- Under Wayland an application cannot know where its surfaces are on screen, so a screen reader
  gets the tree but no position.
- `exclusive_edge` cannot be cleared once set (the protocol has no request for it).

## Not tested anywhere

Touch (no hardware); non-AMD GPUs; labwc and other wlroots compositors besides sway; GNOME
(there is no layer shell); Windows and macOS (layer surfaces are Linux-only); a real v4-only
layer-shell compositor.

## Where each part is

| | tre issue | PR |
|---|---|---|
| Spike (design verdict, go on a second event queue) | #196 | #197 |
| `LayerWindow` core | #198 | #208 |
| Outputs | #202 | #209 |
| Live checks, sway fixes, paste without X, `output=None` | #203 | #210 |
| `input_region`, hit testing and `corner_radius` | #204 | #211 |
| Margin animation, `drag_margin` | #205 | #212 |
| Menus from a bar (the overlay route) | #206 | #213 |
| VRAM per window (research) | #207 | on branch `vram-research-207` |
| Small GPU memory blocks | #219 | on branch `gpu-memory-hints-219` |
| `PopupWindow` | #223 | on branch `popup-window-223` |
| KWin versions of the region, drag and menu tests | | #214 |
| `exclusive_edge` | #201 | #215 |
| IME | #199 | #216 |
| Screen readers | #200 | #217 |

