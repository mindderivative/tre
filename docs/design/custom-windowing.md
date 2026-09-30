# Custom windowing (0.5.0)

!!! note "Decided (2026-09-30) — being built"
    This is 0.5.0's M1 design for [issue #28](https://github.com/mindderivative/tre/issues/28),
    reviewed by Tesserae and decided by the project owner: every
    recommendation accepted, with Tesserae's refinements, and five
    [additions](#additions-from-the-review). Each question below ends with
    its **Decided** line. Built so far: M2's undecorated windows,
    controls, state and events, fullscreen, minimum size, icon, and
    platform ([#37](https://github.com/mindderivative/tre/issues/37)); M3–M5
    build the rest.

## The goal

A window can drop the operating system's title bar and borders, so the
framework draws its own: an icon, a title, minimize, maximize, and close
buttons, and whatever else an application wants up there — a search field,
tabs, extra buttons. The same window then looks the same on every platform.

Issue #28 splits the work the way `tre` splits everything: mechanism in the
engine, look in the framework.

| Tesserae Engine (`tre`) | The framework (Tesserae) |
| --- | --- |
| Turns the native decorations off | Draws the title bar, its buttons, and any border |
| Moves the window when the title bar is dragged | Marks which nodes are the title bar |
| Resizes the window from its edges | Says how wide the resize edges are |
| Minimizes, maximizes, restores, and closes on request | Calls those from its buttons |
| Reports maximized, minimized, and active (focused) | Swaps the maximize/restore icon, dims an inactive bar |

## What `winit` allows

Checked in `winit` 0.30.13's source, which `tre` pins:

- `with_decorations(false)` and `set_decorations` turn decorations off on
  every desktop platform, at creation or live.
- `drag_window()` moves the window on X11, Wayland, Windows, and macOS, but
  only "if the left mouse button was pressed immediately before" the call.
  So `tre` has to start the drag on the press itself — from a region the
  framework marked beforehand — not from a Python callback afterwards.
- `drag_resize_window(direction)` resizes from an edge or corner everywhere
  **except macOS**, where it always fails. An undecorated macOS window
  can't be resized by the user at all.
- macOS has its own answer: a transparent title bar over full-size content
  (`with_titlebar_transparent`, `with_fullsize_content_view`,
  `with_title_hidden`). The content fills the window, the native
  traffic-light buttons stay, and so do resizing, rounded corners, and the
  shadow.
- Windows drops an undecorated window's shadow unless
  `with_undecorated_shadow(true)` is set, and offers the native title-bar
  menu through `show_window_menu(position)`.
- `set_minimized`, `set_maximized`, `is_minimized`, and `is_maximized` are
  there; a window gaining or losing focus arrives as `WindowEvent::Focused`.

What `tre` has today: windows are made in one place (`engine-platform`'s
`WindowAttributes`, which sets only a title and size); nodes already have a
`cursor` property with every resize shape; nothing handles window focus;
and there's no double-click detection.

## The API

### An undecorated window

```python
window = Window(width=960, height=640, title="Notes", decorations=False)
window.set(decorations=True)       # live, either way
window.get("decorations")          # False
```

`title` still names the window to the OS — the taskbar, window switcher,
and screen readers — though nothing draws it but the framework.

### Window controls

```python
window.minimize()
window.maximize()
window.restore()                   # from maximized or minimized
window.close()                     # as if the user closed it (see Q6)
```

### State and events

```python
window.get("maximized")            # True or False
window.get("minimized")
window.get("active")               # whether the window has focus

window.on("maximized", lambda e: swap_icon(e.maximized))
window.on("active", lambda e: dim_title_bar(not e.active))
```

`winit` sends no "maximized" event, so `tre` checks `is_maximized()` after
each resize and fires `maximized` when it changes. `active` comes from
`WindowEvent::Focused`. `window.simulate("active", active=False)` and
`simulate("maximized", ...)` make both testable without a display, like
the existing window events.

### The title bar: a drag region

```python
bar = window.create("box", height=36, window_region="drag")
```

A node property, `window_region`, marks a node as the window's title bar.
A primary press that lands in it starts `drag_window()` at once, so the
window follows the pointer until the button is released. What happens to
the buttons and fields inside the bar is Q1.

### The border: a resize width

```python
window.set(resize_border=6)        # pixels along each edge, 0 for none
```

A window property, not nodes, because issue #28 describes the border as
"these pixels along the root view sides": while the window is undecorated
and not maximized, a press within `resize_border` pixels of an edge starts
`drag_resize_window()` in that edge's or corner's direction, and the
pointer shows the matching resize cursor there. It's hit-tested before any
node. See Q2 for the alternative.

## Platforms

| Platform | Undecorated | Drag | Resize | Notes |
| --- | --- | --- | --- | --- |
| Linux, Wayland | Yes | Yes | Yes | The compositor draws nothing; everything is the framework's |
| Linux, X11 | Yes | Yes | Yes | Moves and resizes through the window manager |
| Windows | Yes | Yes (snaps to edges) | Yes | `tre` sets the undecorated shadow; Windows 11's snap-layout flyout over a maximize button isn't reachable through `winit` |
| macOS | See Q5 | Yes | Not undecorated | The transparent title bar keeps native resizing |

## Accessibility

The framework's title-bar buttons are ordinary nodes with roles
(`"button"`, a label), as now. The OS still reads the window's `title`.
Nothing changes for AccessKit.

## Decisions

Each question lists the options offered, the recommendation marked
**(recommended)**, and ends with what was decided.

**Q1. Buttons and fields inside the title bar.** A press on the close
button or a search field must not drag the window.

- **(recommended)** Automatic: a press drags only when it lands on the
  drag region itself or on nodes inside it that aren't interactive — not
  focusable and with no pointer or click listener. So a button or field
  just works, and `window_region="none"` on a node forces it either way.
- Explicit, as Electron does: everything inside a drag region drags unless
  marked `window_region="none"`.

**Decided:** automatic, with Tesserae's tighter rule. A node is interactive
if it's focusable, has a `click` listener, or holds the pointer capture;
hover listeners and `pointer_down` alone don't count, so a tooltip's anchor
and a context menu's secondary-press listener still drag. Only the nodes
from the one pressed up to the drag region are checked, never above it (a
root listening for the mouse's side buttons doesn't count).
`window_region="none"` turns dragging off for a node, and `"drag"` on a
child turns it back on.

**Q2. How the resize edges are marked.**

- **(recommended)** A window property, `resize_border`, as above: one
  number, and nothing to lay out.
- Edge nodes: the framework places thin nodes along the sides with
  `window_region="resize_n"` and so on — more control (different widths, a
  gap for a corner widget), more work for every framework.

**Decided:** `resize_border`. A press taken for resizing never reaches the
nodes under it, so a splitter or scrollbar at the edge doesn't also start
its own drag. The border lies inside the content, and the docs say so, so
the framework can inset its content by that much.

**Q3. Double-clicking the title bar maximizes.** Windows, GNOME, and KDE
do this by default; macOS follows a system setting.

- **(recommended)** `tre` does it: a second primary press on a drag region
  within 500 ms toggles maximize.
- The framework does it, timing presses itself and calling `maximize()`.

**Decided:** `tre` does it, counting only presses the Q1 rule would drag
with, and timing them with the system's double-click time where the
platform gives one (Windows, macOS), 500 ms elsewhere. On macOS it follows
the user's title-bar double-click setting.

**Q4. The Windows title-bar menu.** Right-clicking a native title bar on
Windows shows Restore, Move, Size, Minimize, Maximize, Close.

- **(recommended)** A secondary press on a drag region shows it on Windows
  (`show_window_menu`), and does nothing elsewhere; `pointer_down` still
  reaches the framework's listeners, so it can show its own instead.
- Leave it to the framework.

**Decided:** yes, with an opt-out, `window.set(system_menu=False)`, for a
framework that shows its own menu on the title bar. On Windows, Alt+Space
opens it too, since an undecorated window loses that.

**Q5. macOS.** An undecorated macOS window can't be resized.

- **(recommended)** On macOS, `decorations=False` makes the title bar
  transparent over full-size content, keeping the traffic lights and
  resizing. `window.get("titlebar_inset")` gives the height and the
  traffic lights' width, so the framework lays its bar out beside them.
  The framework's own minimize/maximize/close are then hidden on macOS —
  its choice.
- Truly borderless on macOS too: the same look everywhere, but no resizing
  and no traffic lights.

**Decided:** the transparent title bar, plus: `window.get("native_controls")`
says whether the traffic lights are shown, rather than leaving it to be
read from a non-zero inset; a window event fires when `titlebar_inset`
changes (fullscreen hides the traffic lights, making it 0); and
`window_region="drag"` works in a bar taller than the native strip.

**Q6. `window.close()`.**

- **(recommended)** It fires `close_requested` first, like the user closing
  the window, so an app's "save changes?" check still runs; the window
  closes unless a listener cancels.
- It closes at once.

**Decided:** it fires `close_requested` first.

**Q7. The state events.**

- **(recommended)** Two events, `maximized` and `active`, each carrying its
  new value.
- One `state` event carrying `maximized`, `minimized`, and `active`
  together.

**Decided:** two events, `maximized` and `active`. No minimized event:
nothing needs one.

## Additions from the review

Tesserae's review found five things a real title bar needs; all were
accepted.

1. **`pointer_cancel`.** `winit` swallows the button release after
   `drag_window()` or `drag_resize_window()` on some platforms, which would
   leave a framework's pressed state or pointer capture stuck. When `tre`
   starts a move or resize, it sends `pointer_cancel` to the pressed node
   and releases any capture.
2. **Fullscreen.** `window.set(fullscreen=True)` and
   `get("fullscreen")`. In fullscreen the drag region and resize border do
   nothing, and the macOS inset is 0.
3. **A window icon.** `window.set(icon=(rgba, width, height))`, for the
   taskbar and window switcher. `winit` sets it on Windows and X11;
   Wayland and macOS take the icon from the app's desktop file or bundle.
4. **A minimum size.** `window.set(min_width=..., min_height=...)`, so
   resizing can't crush the title bar's buttons.
5. **The platform.** `window.get("platform")`: `"wayland"`, `"x11"`,
   `"windows"`, or `"macos"`, since snapping and shadows differ.

## Milestones

As decided:

- **M2** — `decorations`, the window controls, the state and its events,
  fullscreen, the icon, the minimum size, and the platform.
- **M3** — `window_region`, `resize_border`, cursors, `pointer_cancel`,
  double-click to maximize, and the Windows menu with its opt-out.
- **M4** — macOS's transparent title bar, `titlebar_inset`,
  `native_controls`, and the inset event.
- **M5** — `examples/custom_titlebar.py`, a guide page, and manual checks on
  KDE Wayland and X11 (macOS and Windows by CI only).
