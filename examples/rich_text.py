#!/usr/bin/env python3
"""Rich text: styled spans in one text node, and text you can select and copy.

`spans` styles byte ranges of a text node (colour, weight, italic, underline,
strikethrough). `selectable=True` lets a press and drag select the text, and
Ctrl+C copies it.

Headless-CI-safe: renders `max_frames=60` and returns quietly without a display.
Pass `--watch` to keep the window open and try selecting the second paragraph.
See docs/guide/text.md.
"""

import sys

from tre import App, Window

INK = (0x1C, 0x1B, 0x1F, 0xFF)
RED = (0xB3, 0x26, 0x1E, 0xFF)
LINK = (0x0B, 0x57, 0xD0, 0xFF)

window = Window(width=560, height=240, title="tre -- rich text")
window.root.set(fill=(0xFF, 0xFB, 0xFE, 0xFF), padding=16, flex_direction="vertical", gap=12)

price = "Sale: $12 $9, ends Friday. Read the terms."
label = window.create("text", text=price, font_size=22, fill=INK, width=520, height=34)


def span_of(part: str, **style):
    """`part`'s byte range in `price`, styled (spans are UTF-8 byte offsets)."""
    start = len(price[: price.index(part)].encode())
    return (start, start + len(part.encode()), style)


label.set(spans=[
    span_of("Sale:", weight=500),
    span_of("$12", strikethrough=True, color=(0x79, 0x74, 0x7E, 0xFF)),
    span_of("$9", color=RED, weight=500),
    span_of("Read the terms", underline=True, color=LINK),
])

paragraph = window.create(
    "text", font_size=18, fill=INK, width=520, height=120, selectable=True,
    text="Press and drag across this paragraph to select it, then press Ctrl+C. "
         "Selected text shows the text colour behind it, and a press anywhere else "
         "clears the selection.",
)
for node in (label, paragraph):
    window.root.add_child(node)

app = App()
app.add_window(window)
app.run(max_frames=None if "--watch" in sys.argv else 60)
print("rich_text.py: exited cleanly")
