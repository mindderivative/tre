#!/usr/bin/env python3
"""A press ripple built from primitives: the button clips its children
(`clip_children=True`), and each press adds a round box centered on the
press point that grows and fades, then destroys itself from its
`on_complete`. Rapid presses overlap, each ripple animating on its own.

The script checks the ripple's life with `window.simulate` and
`window.advance`, then opens the window. Headless-CI-safe: `App.run()`
renders `max_frames=60` and returns quietly without a display.
"""

from tre import App, Event, Window

ACCENT = (0x67, 0x50, 0xA4, 0xFF)
WHITE = (0xFF, 0xFF, 0xFF, 0xFF)
EASE_OUT = (0.2, 0.0, 0.0, 1.0)
W, H = 160.0, 48.0
RADIUS = (W * W + H * H) ** 0.5  # reaches every corner from anywhere inside

window = Window(width=260, height=120, title="tre -- ripple")
window.root.set(align_items="center", justify_content="center")
button = window.create("box", width=W, height=H, corner_radius=H / 2, fill=ACCENT,
                       clip_children=True, align_items="center", justify_content="center",
                       role="button", label="Press me", focusable=True, cursor="pointer")
button.add_child(window.create("text", text="Press me", fill=WHITE, width=100,
                               height=20, text_align="center"))
window.root.add_child(button)
live = []


def ripple(event: Event):
    wave = window.create("box", position="absolute", x=event.x - RADIUS, y=event.y - RADIUS,
                         width=2 * RADIUS, height=2 * RADIUS, corner_radius=RADIUS,
                         fill=(0xFF, 0xFF, 0xFF, 0x50), scale=0.0, hit_testable=False)
    button.insert_child(0, wave)  # beneath the label
    live.append(wave)

    def done():
        live.remove(wave)
        wave.destroy()

    wave.animate("scale", 1.0, 450, easing=EASE_OUT)
    wave.animate("opacity", 0.0, 450, on_complete=done)


button.on("pointer_down", ripple)

# -- checks ------------------------------------------------------------------
window.advance(0)
window.simulate("pointer_down", node=button, x=20, y=24)
window.simulate("pointer_up", node=button, x=20, y=24)
window.advance(150)
window.simulate("pointer_down", node=button, x=140, y=24)  # a second, overlapping one
assert len(live) == 2 and 0 < live[0].get("scale") < 1
window.advance(300)
assert len(live) == 1, "the first finished and destroyed itself"
window.advance(150)
assert live == [] and len(button.children()) == 1, "only the label is left"
print("ripple.py: checks passed")

app = App()
app.add_window(window)
app.run(max_frames=60)
print("ripple.py: exited cleanly")
