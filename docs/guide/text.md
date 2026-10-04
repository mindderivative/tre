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

Layout rounds every box to whole pixels, so a text node's explicit `width`
(and `min_width`, `max_width`) is laid out **rounded up**: a width the text fits
in still fits. Given the `66.43` that `measure_text` returned, the node is 67
wide and the text stays on one line; rounded down to 66, it would have wrapped.
`get("width")` still reads back what you set, and `layout_width` is the whole
pixel above it. Percentage and `auto` widths, and every other kind of node
(boxes included), are rounded as before.

## Fonts

Tesserae Engine bundles Roboto (regular and medium), Noto Sans Arabic, and Hack Nerd
Font Mono, and by default never loads system fonts, so text renders the same on every
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

## Other languages and emoji

The bundled fonts cover Latin, Cyrillic, Greek and Arabic. Hebrew, CJK, Indic
scripts, Thai, colour emoji and the rest have no bundled font, so text in them
draws as missing-glyph boxes or nothing. There are two ways to supply them.

**Use the machine's fonts (0.5.4).** One switch lets text fall back to whatever
is installed, for exactly the glyphs the bundled and registered fonts lack:

```python
tre.set_system_fonts(True)     # for every window in this process, live
```

A node whose text mixes scripts then needs no special handling: with
`font_family="Roboto"`, Latin comes from Roboto and the rest from a system font
chosen for its script, so `"Hello 漢字 😀 שלום"` shows all of it. Colour emoji
(including the COLR and bitmap formats) draw in colour, and a right-to-left
paragraph (Arabic, Hebrew) lays out right to left and aligns its start on the
right. The switch is off by default because it makes text depend on the machine:
the same app shows different pixels where different fonts are installed, and
where no installed font has a script it still shows boxes. Text the bundled
fonts cover is unaffected either way, and turning the switch off again returns
to the exact same frames.

**Ship the fonts with the app.** For the same pixels everywhere, register
font files you bundle, such as subsets of [Noto](https://fonts.google.com/noto):

```python
from importlib.resources import files
import tre

for name in ("NotoSansJP-Regular.otf", "NotoSansHebrew-Regular.ttf", "NotoEmoji-Regular.ttf"):
    tre.register_font(files("myapp.fonts").joinpath(name).read_bytes())
```

Registered fonts take part in fallback too: a glyph that `font_family` lacks is
taken from any registered font that has it, so one call per script is enough and
no node needs to name them. Subset a font to the glyphs you need
(`pyftsubset`) to keep an app small; a full CJK font is many megabytes.

## Rich text and selectable text

One text node can mix styles. `spans` styles ranges of its content (0.5.4),
given as UTF-8 byte offsets, as a text input's `selection` and `syntax_spans`
are:

```python
label = window.create("text", text="Sale: $12 $9, ends Friday", font_size=18, width=300)
label.set(spans=[
    (0, 5, {"weight": 500}),                              # "Sale:"
    (6, 9, {"strikethrough": True}),                       # "$12"
    (10, 12, {"color": (0xB3, 0x26, 0x1E, 0xFF), "weight": 500}),
    (20, 26, {"italic": True, "underline": True}),
])
```

A style may set any of `color`, `weight` (an OpenType weight, 100 to 950),
`italic`, `underline` and `strikethrough`; what it leaves out stays the node's
own, and a later span wins where two overlap. Offsets past the text, or inside a
multi-byte character, are clamped, so a span survives a shorter `text`. Spans
change the shape of the text (a heavier weight is wider), so the node's width
is yours to size, as it is for any text. Underline and strikethrough follow the
text's own font metrics and are drawn in the span's colour.

`selectable=True` lets the user select the text with the pointer and copy it
(0.5.4):

```python
help_text.set(selectable=True)          # press and drag selects; Ctrl+C copies
help_text.get("selection")              # (start, end) or None; equal ends are a caret
help_text.set(selection=(0, 5))         # select from code; None clears
```

Selected text shows the text colour at 30% behind it. One text holds a
selection at a time, and a press anywhere else clears it. A selectable text
claims the pointer events over its box, which plain text never does (so a label
inside a button doesn't swallow the button's clicks): turn it on for text that
stands alone. A text input's own selection is separate, and Copy takes the
focused input's selection first. Static text exposes no selection to screen
readers yet.

## Text input

A `"text_input"` is editable text with a caret, selection, clipboard, and
input-method support. Its `fill` is the text color, so it paints no background
of its own — put it in a `"box"` for a field's background (its own
`stroke_color`, `stroke_width`, and `corner_radius` draw and round a border, and
its own `padding` insets its text, 0.5.1).
Of the styling properties above it takes
`font_family`, `font_size`, and `font_weight`; the rest are for text nodes
only:

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
