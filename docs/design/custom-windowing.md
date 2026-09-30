# Custom windowing (0.5.0)

!!! warning "Proposal — awaiting decisions"
    This is 0.5.0's M1 design for [issue #28](https://github.com/mindderivative/tre/issues/28),
    written for the project owner and for Tesserae to review. Nothing here
    exists yet. The [open questions](#open-questions) each carry a
    recommendation; once they're decided, this page records the answers and
    M2–M5 follow it.

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

## Proposed API

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

## Open questions

Each has a recommendation, marked **(recommended)**.

**Q1. Buttons and fields inside the title bar.** A press on the close
button or a search field must not drag the window.

- **(recommended)** Automatic: a press drags only when it lands on the
  drag region itself or on nodes inside it that aren't interactive — not
  focusable and with no pointer or click listener. So a button or field
  just works, and `window_region="none"` on a node forces it either way.
- Explicit, as Electron does: everything inside a drag region drags unless
  marked `window_region="none"`.

**Q2. How the resize edges are marked.**

- **(recommended)** A window property, `resize_border`, as above: one
  number, and nothing to lay out.
- Edge nodes: the framework places thin nodes along the sides with
  `window_region="resize_n"` and so on — more control (different widths, a
  gap for a corner widget), more work for every framework.

**Q3. Double-clicking the title bar maximizes.** Windows, GNOME, and KDE
do this by default; macOS follows a system setting.

- **(recommended)** `tre` does it: a second primary press on a drag region
  within 500 ms toggles maximize.
- The framework does it, timing presses itself and calling `maximize()`.

**Q4. The Windows title-bar menu.** Right-clicking a native title bar on
Windows shows Restore, Move, Size, Minimize, Maximize, Close.

- **(recommended)** A secondary press on a drag region shows it on Windows
  (`show_window_menu`), and does nothing elsewhere; `pointer_down` still
  reaches the framework's listeners, so it can show its own instead.
- Leave it to the framework.

**Q5. macOS.** An undecorated macOS window can't be resized.

- **(recommended)** On macOS, `decorations=False` makes the title bar
  transparent over full-size content, keeping the traffic lights and
  resizing. `window.get("titlebar_inset")` gives the height and the
  traffic lights' width, so the framework lays its bar out beside them.
  The framework's own minimize/maximize/close are then hidden on macOS —
  its choice.
- Truly borderless on macOS too: the same look everywhere, but no resizing
  and no traffic lights.

**Q6. `window.close()`.**

- **(recommended)** It fires `close_requested` first, like the user closing
  the window, so an app's "save changes?" check still runs; the window
  closes unless a listener cancels.
- It closes at once.

**Q7. The state events.**

- **(recommended)** Two events, `maximized` and `active`, each carrying its
  new value.
- One `state` event carrying `maximized`, `minimized`, and `active`
  together.

## Milestones

Once these are decided (M1 Step 2):

- **M2** — `decorations`, the window controls, and the state and events.
- **M3** — `window_region`, `resize_border`, cursors, and Q3/Q4 if kept.
- **M4** — macOS, as Q5 decides.
- **M5** — `examples/custom_titlebar.py`, a guide page, and manual checks on
  KDE Wayland and X11 (macOS and Windows by CI only).
