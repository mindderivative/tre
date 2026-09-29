#!/usr/bin/env python3
"""A keyed list reordered in place: each row is kept in a dict by its
key, and `insert_child(index, row)` moves rows into a new order. A moved
node keeps everything about it -- its handle, listeners, keyboard focus,
and any running animation -- which is what a framework's list diffing
needs so that sorting a list never resets the row being edited.

The script sorts the list three ways, checking identity, focus, and a
mid-flight animation survive each move, then opens the window.
Headless-CI-safe: `App.run()` renders `max_frames=60` and returns quietly
without a display.
"""

from tre import App, Window

FRUIT = {"fig": 3, "apple": 5, "kiwi": 1, "pear": 4, "date": 2}

window = Window(width=260, height=260, title="tre -- keyed reorder")
column = window.create("box", flex_direction="vertical", gap=4, width=200)
window.root.add_child(column)

rows = {}
for name in FRUIT:
    row = window.create("box", width=200, height=36, padding_left=12, align_items="center",
                        corner_radius=8, fill=(0xF3, 0xED, 0xF7, 0xFF),
                        focusable=True, role="listitem", label=name)
    row.add_child(window.create("text", text=f"{name}  ({FRUIT[name]})", width=160, height=20))
    column.add_child(row)
    rows[name] = row


def show(order):
    """Moves rows into `order`; a row already in place isn't touched."""
    for index, key in enumerate(order):
        if column.children()[index] != rows[key]:
            column.insert_child(index, rows[key])


# -- checks ------------------------------------------------------------------
window.advance(0)
rows["kiwi"].focus()
rows["pear"].animate("fill", (0x67, 0x50, 0xA4, 0xFF), 400)
window.advance(200)
before = dict(rows)

for order in (sorted(FRUIT),                          # by name
              sorted(FRUIT, key=FRUIT.get),           # by count
              sorted(FRUIT, key=FRUIT.get, reverse=True)):
    show(order)
    assert column.children() == [rows[k] for k in order]

assert rows == before, "the same nodes, not rebuilt ones"
assert rows["kiwi"].get("focused"), "focus moved with its row"
assert rows["pear"].get_target("fill") == (0x67, 0x50, 0xA4, 0xFF)
window.advance(200)
assert rows["pear"].get("fill") == (0x67, 0x50, 0xA4, 0xFF), "its animation kept running"
print("reorder.py: checks passed")

app = App()
app.add_window(window)
app.run(max_frames=60)
print("reorder.py: exited cleanly")
