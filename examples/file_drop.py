#!/usr/bin/env python3
"""File drop: drag files from the OS onto a window.

A drop zone outlines itself while files hover over it and lists what is
dropped. The OS reports file drags on Windows, macOS and X11 (not Wayland), so
this example also drives itself with `window.simulate` and runs anywhere;
pass `--watch` to keep the window open and try it with real files.

See docs/guide/events-and-input.md.
"""

import sys

from tre import App, Window

ACCENT = (0x67, 0x50, 0xA4, 0xFF)

window = Window(width=420, height=260, title="tre -- file drop")
zone = window.create("box", width=360, height=200, x=30, y=30, position="absolute",
                     corner_radius=16, fill=(0xF3, 0xED, 0xF7, 0xFF))
label = window.create("text", text="Drop files here", font_size=18, width=320, height=160,
                      x=20, y=20, position="absolute", fill=(0x1C, 0x1B, 0x1F, 0xFF))
zone.add_child(label)
window.root.add_child(zone)

zone.on("file_hover", lambda e: zone.set(stroke_color=ACCENT, stroke_width=3))
zone.on("file_hover_cancel", lambda: zone.set(stroke_width=0))


def dropped(e) -> None:
    zone.set(stroke_width=0)
    label.set(text="Dropped:\n" + "\n".join(e.paths))
    print("file_drop.py: dropped", e.paths)


zone.on("file_drop", dropped)

if "--watch" in sys.argv:
    app = App()
    app.add_window(window)
    app.run()
else:
    window.simulate("file_hover", paths=["/tmp/notes.txt", "/tmp/photo.png"], x=200, y=130)
    assert zone.get("stroke_width") == 3
    window.simulate("file_hover_cancel")
    assert zone.get("stroke_width") == 0
    window.simulate("file_drop", paths=["/tmp/notes.txt", "/tmp/photo.png"], x=200, y=130)
    assert "photo.png" in label.get("text")
    print("file_drop.py: exited cleanly")
