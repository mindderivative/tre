# Text

Text is shaped with [parley](https://github.com/linebender/parley):
ligatures, kerning, bidirectional text, and fallback between faces are
handled for you. Three kinds show it: `"text"` for labels,
`"text_input"` for editing, and `"terminal"` for a shell.

## Text nodes

```python
title = window.create("text", text="Quarterly report", font_size=22,
                      font_weight=500, fill=(0x1D, 0x1B, 0x20, 0xFF),
                      width=280, height=28)
```

The styling properties are `font_family`, `font_size` (pixels),
`font_weight` (1–1000), `font_style` (`"normal"` or `"italic"`),
`letter_spacing`, `line_height` (a multiple of the size), and `text_align`
(`"start"`, `"center"`, `"end"`). Text wraps at word boundaries within the
node's width; `wrap="none"` keeps each paragraph on one line, `max_lines`
caps the lines shown, and `overflow="ellipsis"` ends a cut line with "…".

## Sizing text to its content

A text node has no size of its own — without a `width` it's zero wide. Size
it with `window.measure_text`, which lays the text out exactly as a text node
with the same properties paints it:

```python
style = dict(font_size=14, max_lines=1, overflow="ellipsis")
width, height = window.measure_text("A long list item title", max_width=180, **style)
label = window.create("text", text="A long list item title",
                      width=min(width, 180), height=height, **style)
```

That's how a button hugs its label, or a chip fits its text.

## Fonts

`tre` bundles Roboto (regular and medium), Noto Sans Arabic, and Hack Nerd
Font Mono, and never loads system fonts, so text renders the same on every
machine. `tre.MONOSPACE_FONT_FAMILY` names the monospace face, which
terminals use — use it for anything that must line up with one, such as an
editor's line numbers.

Any other font is bytes you load and register:

```python
from pathlib import Path
import tre

families = tre.register_font(Path("fonts/Inter-Regular.ttf").read_bytes())
# ["Inter"] -- the name to use as font_family
```

Registration is for the whole process: every window sees the font, and one
already running picks it up on its next frame. Until a family is registered,
a `font_family` naming it falls back to a bundled face.

## Text input

A `"text_input"` is editable text with a caret, selection, clipboard, and
input-method support. It paints no box of its own — put it in a `"box"` for
a field's background and border:

```python
field_box = window.create("box", width=240, height=40, padding=8,
                          corner_radius=4, stroke_color=(0x79, 0x74, 0x7E, 0xFF),
                          stroke_width=1)
field = window.create("text_input", placeholder="Email", width=224, height=24)
field_box.add_child(field)
field.on("change", lambda e: validate(e.new_value))
```

| Property | Does |
| --- | --- |
| `text` | the content; setting it fires no `change` |
| `placeholder`, `placeholder_fill` | the hint shown while it's empty |
| `multiline` | Enter inserts a newline |
| `selection` | `(start, end)` as UTF-8 byte offsets; equal ends are a caret |
| `obscured` | a password field: bullets, and no copy or cut |
| `caret_color`, `selection_fill` | the caret and selection colors |
| `syntax_spans` | `[(start, end, color), ...]` byte ranges to color |
| `folded_ranges` | `[(start, end), ...]` byte ranges drawn as "…" |
| `show_whitespace` | draw spaces and tabs as marks |

A code editor is a multiline input in the monospace face, with syntax spans
from your highlighter:

```python
editor = window.create("text_input", text=source, multiline=True,
                       font_family=tre.MONOSPACE_FONT_FAMILY, font_size=13,
                       width=600, height=400)
editor.set(syntax_spans=[(0, 3, (0xB5, 0x3F, 0x8C, 0xFF))])  # "def"
```

Its events — `input` and `change` — are on
[Events and Input](events-and-input.md#text-input).

## Terminal

A `"terminal"` runs a shell on a pseudo-terminal and emulates its screen:

```python
term = window.create("terminal", shell="/bin/bash", cols=80, rows=24,
                     scrollback_lines=5000, font_size=13)
```

Its box follows its grid and font. While focused it takes every key: text
and named keys go to the shell, and Ctrl+*letter* sends that control byte, so
Ctrl+C interrupts. Ctrl+Shift+C copies its selection, the wheel scrolls its
history, and `set(cols=, rows=)` resizes the grid and tells the shell.
`get("text")` reads the visible grid, one line per row, and `selection` is
`(start_row, start_col, end_row, end_col)`. The `palette` recolors it — any of
`ansi` (16 colors), `foreground`, `background`, `cursor`, and `selection`:

```python
term.set(palette={"background": (0x1E, 0x1E, 0x1E, 0xFF),
                  "foreground": (0xD4, 0xD4, 0xD4, 0xFF)})
```
