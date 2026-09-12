# Text Editing

## `EditableText`

A real text-editing model: caret, selection, IME composition, and clipboard integration -- everything a text input field needs, independent of any particular widget toolkit.

**Real, disclosed scope limit:** caret/selection byte offsets are LTR-only-correct. Multi-line editing is real (see below) once `wrap_width` is set.

```python
font = tre.Font.system_cascade()
field = tre.EditableText(10, 10, "hello", font, 18.0, tre.rgba8(0, 0, 0, 255))
```

```
EditableText(x, y, text, font, px_size, fill_color, wrap_width=None)
```

Constructing sets the caret to the end of `text`. `wrap_width: float | None = None` -- `None` (default) keeps the editor single-line; `Some(width)` enables real multi-line editing.

### Fields

| Field | Type | Access | Notes |
|---|---|---|---|
| `x`, `y` | `float` | get/set | |
| `text` | `str` | get/set | |
| `font` | `Font` | get/set | |
| `px_size` | `float` | get/set | |
| `fill_color` | `int` | get/set | |
| `wrap_width` | `float \| None` | get/set | |
| `caret` | `int` | get only | always a valid UTF-8 char boundary |
| `selection_anchor` | `int \| None` | get only | selection spans `min(caret, selection_anchor)..max(caret, selection_anchor)` |
| `preedit` | `str` | get only | current IME composing text -- shown at the caret by `to_text()` but not part of `text` until a real `ImeCommit` event arrives |
| `ime_active` | `bool` | get only | |

### Editing

| Method | Signature | Notes |
|---|---|---|
| `insert` | `(text: str)` | Deletes any active selection, then inserts `text` at the caret, advancing the caret past it |
| `delete_selection` | `() -> bool` | Returns whether a selection was active |
| `delete_backward` | `()` | Deletes exactly one real character (not byte) |
| `set_caret` | `(byte_offset: int)` | Raises `ValueError` if not a valid UTF-8 char boundary |
| `set_selection` | `(anchor: int, caret: int)` | Raises `ValueError` if either offset isn't a char boundary |

### Clipboard integration

Each takes a `tre.Clipboard` (see [Desktop Integration](desktop-integration.md)) — construct one clipboard and pass it in.

| Method | Signature | Notes |
|---|---|---|
| `copy` | `(clipboard: Clipboard)` | Real no-op (not an error) when no selection is active. Raises whatever `clipboard.set_text` raises |
| `cut` | `(clipboard: Clipboard)` | If the clipboard write fails, `text` is left unchanged -- the delete never runs, so you never lose text to a write you can't observe succeeded |
| `paste` | `(clipboard: Clipboard)` | Raises whatever `clipboard.get_text` raises (e.g. an empty or non-text clipboard) |

### Caret movement

Every movement method takes `extend: bool = False`. `extend=False` always clears any active selection and just moves the caret. `extend=True` starts (or continues) a selection: the anchor is fixed the first time `extend=True` is used and never moves again until a plain, non-extending move clears it -- the standard Shift+arrow convention.

| Method | Signature | Convention |
|---|---|---|
| `move_caret_left` | `(extend=False) -> bool` | ← |
| `move_caret_right` | `(extend=False) -> bool` | → |
| `move_caret_word_left` | `(extend=False) -> bool` | Ctrl+← — real UAX #29 word-boundary segmentation |
| `move_caret_word_right` | `(extend=False) -> bool` | Ctrl+→ |
| `move_caret_up` | `(extend=False) -> bool` | ↑ — raises `TreError`. Preserves a "sticky column" pixel x position, recomputed fresh each call |
| `move_caret_down` | `(extend=False) -> bool` | ↓ — raises `TreError` |
| `move_caret_line_start` | `(extend=False) -> bool` | Home — raises `TreError` |
| `move_caret_line_end` | `(extend=False) -> bool` | End — raises `TreError`. Real trailing whitespace on the line is trimmed before measuring "end", so deliberately-typed trailing whitespace is skipped by `End` too (a disclosed v1 trade-off, avoiding a real tie with the next line's own start) |

Each `move_caret_*` returns whether the caret actually moved.

### Hit-testing and rendering

| Method | Signature | Notes |
|---|---|---|
| `hit_test` | `(x: float) -> int` | Single-line byte offset nearest pixel `x`. Raises `TreError` if shaping against the font fails |
| `hit_test_2d` | `(x: float, y: float) -> int` | 2D variant for multi-line text. Raises `TreError` |
| `line_count` | `() -> int` | Always `>= 1`, even for an empty string. Raises `TreError` |
| `selection_rects` | `() -> list[tuple[float, float, float, float]]` | One `(x, y, width, height)` rect per visual line the selection spans; empty list if no selection is active. Raises `TreError` |
| `to_text` | `() -> Text` | Builds a `Text` shape with the IME `preedit` spliced in at the caret -- render this, not `EditableText` itself |
| `handle_ime` | `(event: InputEvent)` | Feed it every event from `poll_events()` -- non-IME variants are a real no-op, so you don't need to pre-filter |

### A typical frame

```python
for event in renderer.poll_events():
    field.handle_ime(event)
    if isinstance(event, tre.InputEvent.KeyboardKey) and event.state == tre.ElementState.Pressed:
        ...  # your own key_code -> action mapping

registry = tre.ShapeRegistry()
registry.insert_text(field.to_text())
renderer.render(registry)
```