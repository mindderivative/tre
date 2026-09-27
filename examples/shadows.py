#!/usr/bin/env python3
"""Layered shadows (M95): five cards, each animating in to one of MD3's
elevation levels -- written as the `shadows` lists a framework passes,
since `tre` 0.3.5 has no elevation levels of its own (the table is in
`docs/design/legacy-behavior.md`). A key shadow over an ambient one,
CSS `box-shadow`'s model: `(color, offset_x, offset_y, blur, spread)`.

Headless-CI-safe: it renders `max_frames=180` (3s at 60fps) and exits.
The pixel proof that shadows paint is `crates/engine-render/tests/
m95_paint.rs`.
"""

from tre import App, Window

KEY, AMBIENT = (0, 0, 0, 77), (0, 0, 0, 38)
LEVELS = [
    [(KEY, 0, 1, 2, 0), (AMBIENT, 0, 1, 3, 1)],
    [(KEY, 0, 1, 2, 0), (AMBIENT, 0, 2, 6, 2)],
    [(KEY, 0, 1, 3, 0), (AMBIENT, 0, 4, 8, 3)],
    [(KEY, 0, 2, 3, 0), (AMBIENT, 0, 6, 10, 4)],
    [(KEY, 0, 4, 4, 0), (AMBIENT, 0, 8, 12, 6)],
]
FLAT = [((0, 0, 0, 0), 0, 0, 0, 0), ((0, 0, 0, 0), 0, 0, 0, 0)]

window = Window(width=420, height=160, title="tre -- shadows")
window.root.set(fill=(0xF4, 0xF4, 0xF4, 0xFF))

for shadows in LEVELS:
    card = window.create(
        "box", width=56, height=56, fill=(0xFF, 0xFF, 0xFF, 0xFF), corner_radius=8, shadows=FLAT
    )
    window.root.add_child(card)
    card.animate("shadows", shadows, duration_ms=800)

app = App()
app.add_window(window)
app.run(max_frames=180)
print("shadows.py: exited cleanly after 180 frames")
