#!/usr/bin/env python3
"""Animation: three cards show what `node.animate` does.

- The first fades and rounds its corners on a linear curve.
- The second eases its `fill` on a cubic bezier and is retargeted
  halfway, so it turns back from where it is rather than jumping.
- The third chains two steps through `on_complete`, which runs once, in
  the frame the value arrives.

`window.advance(ms)` drives time headlessly, so the script checks each
of these before opening the window. Headless-CI-safe: `App.run()` renders
`max_frames=90` and returns quietly without a display or GPU. See
docs/guide/animation.md.
"""

from tre import App, Window

PURPLE = (0x67, 0x50, 0xA4, 0xFF)
TEAL = (0x03, 0xDA, 0xC6, 0xFF)
STANDARD = (0.2, 0.0, 0.0, 1.0)

window = Window(width=420, height=160, title="tre -- animation")
cards = [window.create("box", width=100, height=100, fill=PURPLE) for _ in range(3)]
for card in cards:
    window.root.add_child(card)
fade, tint, chain = cards

window.advance(0)  # pin the clock: from here on, only advance() moves it

# 1. Linear: halfway through the duration is halfway to the target.
fade.animate("opacity", 0.2, 800)
fade.animate("corner_radius", 24.0, 800)
window.advance(400)
assert abs(fade.get("opacity") - 0.6) < 1e-9
assert fade.get("corner_radius") == 12.0

# 2. Retargeting: a new animate() starts from the value on screen.
tint.animate("fill", TEAL, 400, easing=STANDARD)
window.advance(200)
midway = tint.get("fill")
assert midway not in (PURPLE, TEAL)
tint.animate("fill", PURPLE, 400, easing=STANDARD)
assert tint.get("fill") == midway, "no jump when retargeted"
assert tint.get_target("fill") == PURPLE

# 3. Completion: each step's on_complete starts the next.
steps = []


def settle():
    steps.append("grown")
    chain.animate("scale", 1.0, 150, on_complete=lambda: steps.append("settled"))


chain.animate("scale", 1.2, 150, on_complete=settle)
window.advance(150)
assert steps == ["grown"]
window.advance(150)
assert steps == ["grown", "settled"] and chain.get("scale") == 1.0
print("animation.py: checks passed")

app = App()
app.add_window(window)  # App.run() returns the window to the real clock
app.run(max_frames=90)
print("animation.py: exited cleanly")
