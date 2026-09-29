#!/usr/bin/env python3
"""Grid layout: a settings form whose labels and fields line up in two
columns, over a gallery whose cells wrap to fit the width.

`display="grid"` lays children out in rows and columns. The form's columns
are `"auto 1fr"`: as wide as the longest label, then the rest -- so every
field starts at the same x, whatever its label. The gallery's
`"repeat(auto_fill, minmax(96, 1fr))"` fits as many 96-pixel-or-wider cells
as the width allows and shares out what's left; the last tile spans two
columns. The script checks that geometry before any frame renders.

Headless-CI-safe: it checks the layout, renders `max_frames=60`, and
exits; `App.run()` returns quietly where no display is reachable.
See docs/guide/nodes-and-layout.md.
"""

from tre import App, Window

SURFACE = (0xFE, 0xF7, 0xFF, 0xFF)
FIELD = (0xF3, 0xED, 0xF7, 0xFF)
TILE = (0x67, 0x50, 0xA4, 0xFF)

window = Window(width=480, height=360, title="tre -- grid")
window.root.set(flex_direction="vertical", padding=16, gap=16, fill=SURFACE)

form = window.create("box", display="grid", width="100%",
                     grid_template_columns="auto 1fr", row_gap=8, column_gap=12,
                     align_items="center")
fields = []
for label in ("Name", "Email address", "City"):
    form.add_child(window.create("text", text=label, font_size=14, height=20))
    field = window.create("text_input", text="", height=28, fill=FIELD)
    form.add_child(field)
    fields.append(field)

gallery = window.create("box", display="grid", width="100%", gap=8,
                        grid_template_columns="repeat(auto_fill, minmax(96, 1fr))")
tiles = [window.create("box", height=56, corner_radius=8, fill=TILE) for _ in range(7)]
for tile in tiles:
    gallery.add_child(tile)
tiles[-1].set(grid_column="span 2")

window.root.add_child(form)
window.root.add_child(gallery)

# Every field starts at the same x, and is equally wide.
assert len({f.get("layout_x") for f in fields}) == 1
assert len({f.get("layout_width") for f in fields}) == 1
# 448 pixels fit four columns of at least 96 with 8-pixel gaps.
first_row = [t for t in tiles if t.get("layout_y") == tiles[0].get("layout_y")]
assert len(first_row) == 4, len(first_row)
assert tiles[-1].get("layout_width") > tiles[0].get("layout_width") * 2
print("grid.py: checks passed")

app = App()
app.add_window(window)
app.run(max_frames=60)
print("grid.py: exited cleanly")
