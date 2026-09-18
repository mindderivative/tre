# MD3 Components

`tre` implements a handful of real Material Design 3 components directly
in the engine — not as Python-side wrapper logic, but as native
`NodeKind` variants with their own state, dispatch, and painting.

## Checkbox

```python
box = window.add_checkbox(background=(0x67, 0x50, 0xA4, 0xFF), width=24, height=24, checked=False)
box.set_on_click(lambda: box.set_checked(not box.get_checked()))
box.animate("check_progress", 1.0, duration_ms=150)
```

- `background` is the box's own fill color.
- `checked` seeds the initial boxed/unboxed state.
- The engine never flips `checked` itself on click — by design, checked
  state depends on what the app's data means, so the app's own `on_click`
  handler calls `set_checked` explicitly, typically alongside an
  `animate("check_progress", ...)` call for the visual consequence.
- `node.set_checked(value)` — plain, non-animated write; also fires a
  `Change` event.
- `node.get_checked()` — read back the current `checked` value.
- `node.animate("check_progress", 0.0 | 1.0, duration_ms)` — the
  checkmark's own draw animation, independent of `checked` itself.

## Slider

```python
slider = window.add_slider(background=(0x03, 0xDA, 0xC6, 0xFF), width=200, height=32, value=0.3)
slider.set_on_change(lambda: print("new value:", slider.get("thumb_position")))
```

- `value` seeds `thumb_position`, clamped to `0.0..=1.0`.
- Drag-to-set is entirely built into the engine's own input dispatch — no
  Python wiring needed for that half.
- Arrow-key increments work once the slider has keyboard focus (it opts
  into `Role::Slider` + `Action::Focus` at construction, so it's
  Tab-reachable from the start).
- `node.animate("thumb_position", value, duration_ms)` for an app-
  triggered eased move (e.g. a keyboard nudge), distinct from the drag
  path above.
- `node.set_on_change(...)` fires when a drag genuinely ends.

## TextField

```python
field = window.add_text_field(
    background=(0xEE, 0xEE, 0xEE, 0xFF),
    width=280,
    height=48,
    content="",
    font_family="Roboto",
    font_weight=400.0,
    font_size=16.0,
)
```

- Real keyboard editing, mouse click-to-position and drag-to-select, IME
  composition, and clipboard integration.
- Opts into `Role::TextInput` + `Action::Focus` at construction.
- `node.set_text(content)` — plain write, resets the cursor to the new
  content's end, fires `Change`.
- `node.get_text()` — read back the current content.
- `node.set_on_change(...)` fires on any edit (typing, cut, paste,
  backspace/delete).

## Image

```python
picture = window.add_image("logo.png", width=200, height=120, fit="cover")
```

- Loads and decodes a real file from disk (`png`/`jpeg`) at call time and
  uploads it as a GPU texture.
- `fit` is one of `"cover"`, `"contain"`, `"fill"` (default `"fill"`).
- Raises `OSError` if the file can't be read or decoded.
- Has no `background` parameter — there's no meaningful "behind the
  content" color for a node whose entire content is a loaded image.

## Icon

```python
icon = window.add_icon("settings", color=(0x1C, 0x1B, 0x1F, 0xFF), size=24)
```

- One square `size` (Material Symbols icons are uniformly square by
  design), rendered as a vector fill in `color`.
- Currently curated icon names: `home`, `search`, `menu`, `close`,
  `check`, `arrow_back`, `add`, `settings` — a small, deliberately
  additive starter set, not the full Material Symbols library. An
  unknown name raises `ValueError` listing the real known set.
- Has no `background` parameter, the same reasoning as `Image`.

## Splitter

```python
left = window.add_rect(background=(0xEE, 0xEE, 0xEE, 0xFF), width=200, height=300)
splitter = window.add_splitter(background=(0xCC, 0xCC, 0xCC, 0xFF), width=8, height=300, initial_position=0.5)
right = window.add_rect(background=(0xDD, 0xDD, 0xDD, 0xFF), width=200, height=300)
```

Add a splitter between two panes (in call order — left pane, splitter,
right pane) inside a `Window`'s own root row to get a resizable-pane
layout, with no separate "pane container" concept needed. Drag handling
is entirely built into the engine's own input dispatch.

## Theming components together

`window.set_theme(seed, dark=False)` builds a full MD3 dynamic color
scheme from one seed color and immediately re-tints every already-
`enable_interaction()`-enabled node's ripple/hover state layer, plus
every checkbox mark / slider track / text field's default text color
created afterward. See [Theming & Accessibility](theming-and-accessibility.md).
