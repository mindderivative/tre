# Upgrading to 0.4.x

No name was removed or renamed in 0.4.x: code written for 0.3.5 runs
unchanged. A few behaviors changed, and each is listed here with the release
that changed it, in case your code or tests relied on the old one.

## 0.4.0

- **Partial redraw is on.** A window repaints only what changed each frame.
  Nothing in your code has to say what changed; `window.set(partial_redraw=False)`
  goes back to full redraws, and `window.get("partial_redraw_active")` says
  whether an open window really redraws partially. See
  [Painting](guide/painting.md#redrawing).
- **`App.run()` raises `RuntimeError`** when a window's GPU can't be set up
  (no adapter, no device, or a surface the adapter can't drive), where the
  process used to exit or panic. No display at all still returns `None`.
- **`Window(0, h)` and `Window(w, 0)` raise `ValueError`.**
- **Images larger than `MAX_IMAGE_DIMENSION`** on a side are rejected with
  `ValueError`.
- **Building from source needs Rust 1.90** or newer.

## 0.4.1

- **The mouse's side buttons are reported.** They arrive as `pointer_down`
  and `pointer_up` with `event.button` set to `"back"` or `"forward"`, and,
  like the middle button, make no `click` and move no focus. They used to be
  dropped.
- **`window.set(show_damage=True)`** tints each frame's redrawn areas, for
  debugging partial redraw.

## 0.4.2

- **Keys scroll.** The arrow keys, Page Up/Down, and Home/End scroll the
  nearest scroll view around the focused node, unless that node uses the key:
  a `key_down` listener anywhere from the focused node up to the scroll view
  keeps every key, and a text input keeps all but Page Up and Page Down. A
  widget that reads keys through `key_down` is unaffected. See
  [Keyboard scrolling](guide/nodes-and-layout.md#keyboard-scrolling).
- **Focus reveals.** Focusing a node — by Tab, a click, `node.focus()`, or an
  assistive technology — scrolls the scroll views around it just enough to
  show it, and Tesserae Engine now answers an assistive technology's
  `scroll_into_view` request itself. A test that focuses a node below the
  fold and then checks a scroll offset will see it move.
- **New:** `node.scroll_into_view()`, the `scroll` event, CSS Grid layout
  (`display="grid"`), and the Page Up and Page Down keys, which a terminal
  receives as `\x1b[5~` and `\x1b[6~`.

## 0.4.3

- **Shortcuts don't scroll.** An arrow, Page Up/Down, Home, or End pressed
  with Ctrl, Alt, or Meta held no longer scrolls the scroll view around the
  focused node, so it's free for a shortcut such as Ctrl+Page Down. Shift
  still scrolls.
- **A bare number is a one-track list.** `grid_auto_rows=96` (and the other
  three track-list properties) now means `[96]`, reading back as `"96"`,
  where it used to raise `ValueError`.
- **`scroll_offset` is clamped when it's set.** `set(scroll_offset=...)` past
  the end reads back as the end at once and fires one `scroll` event, where
  it used to read back as set until the next layout clamped it and fired a
  second. `animate` to a point past the end eases to the real end. A view
  created with `scroll_offset` fires nothing on its first frame, and its
  first `scroll` event's `old_value` is that offset, not 0.
