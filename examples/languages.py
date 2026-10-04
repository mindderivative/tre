#!/usr/bin/env python3
"""Languages: text in many scripts, and colour emoji, with `set_system_fonts`.

The bundled fonts cover Latin, Cyrillic, Greek and Arabic. `set_system_fonts(True)`
lets text fall back to the machine's installed fonts for the rest (CJK, Hebrew,
Indic, Thai, colour emoji); without it those glyphs draw as boxes. Hebrew and
Arabic lay out right to left. To get the same pixels on every machine, register
font files you ship with `tre.register_font` instead.

What shows depends on the fonts installed here; a machine without a CJK font
still shows boxes for it. Headless-CI-safe: renders `max_frames=60` and returns
quietly without a display. Pass `--watch` to keep the window open.
See docs/guide/text.md.
"""

import sys

import tre
from tre import App, Window

tre.set_system_fonts(True)

LINES = [
    "Hello, world — Привет — Γειά σου",
    "漢字かな  안녕하세요  नमस्ते  ไทย",
    "שלום עולם   مرحبا بالعالم",
    "😀 🎉 ❤️ 🚀",
]

window = Window(width=640, height=300, title="tre -- languages")
window.root.set(fill=(0xFF, 0xFF, 0xFF, 0xFF), padding=12, flex_direction="vertical", gap=6)
for line in LINES:
    window.root.add_child(
        window.create("text", text=line, font_family="Roboto", font_size=28,
                      width=610, height=48, fill=(0x1C, 0x1B, 0x1F, 0xFF))
    )

app = App()
app.add_window(window)
app.run(max_frames=None if "--watch" in sys.argv else 60)
print("languages.py: exited cleanly")
