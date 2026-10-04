#!/usr/bin/env python3
"""Honour the OS's reduced-motion and increased-contrast preferences.

`window.get("reduced_motion")` and `get("high_contrast")` read what the user
asked the system for, and the window's `reduced_motion` and `high_contrast`
events report a change. The engine changes nothing itself: here a card eases to
a new colour over 600 ms normally and snaps when motion is reduced, and swaps to
a high-contrast palette when contrast is raised.

Headless-CI-safe: renders `max_frames=60` and returns quietly without a display.
Pass `--watch` to keep the window open and flip the setting in your system
settings. See docs/reference/events.md.
"""

import sys

from tre import App, Window

NORMAL = {"fill": (0xF3, 0xED, 0xF7, 0xFF), "ink": (0x1C, 0x1B, 0x1F, 0xFF), "accent": (0x67, 0x50, 0xA4, 0xFF)}
CONTRAST = {"fill": (0xFF, 0xFF, 0xFF, 0xFF), "ink": (0x00, 0x00, 0x00, 0xFF), "accent": (0x00, 0x00, 0xC8, 0xFF)}

window = Window(width=420, height=200, title="tre -- preferences")
card = window.create("box", width=360, height=120, x=30, y=40, position="absolute", corner_radius=16)
label = window.create("text", text="", font_size=18, width=320, height=80, x=20, y=20, position="absolute")
card.add_child(label)
window.root.add_child(card)

state = {"reduced": bool(window.get("reduced_motion")), "contrast": bool(window.get("high_contrast"))}


def palette() -> dict:
    return CONTRAST if state["contrast"] else NORMAL


def paint() -> None:
    colors = palette()
    card.set(fill=colors["fill"])
    label.set(
        fill=colors["ink"],
        text=f"Reduced motion: {state['reduced']}\nIncreased contrast: {state['contrast']}",
    )


def pulse() -> None:
    """Ease the card's border colour -- or just set it, with motion reduced."""
    target = palette()["accent"]
    if state["reduced"]:
        card.set(stroke_color=target, stroke_width=3)
    else:
        card.set(stroke_width=3)
        card.animate("stroke_color", target, 600)


def on_motion(e) -> None:
    state["reduced"] = e.reduced_motion
    paint()
    pulse()


def on_contrast(e) -> None:
    state["contrast"] = e.high_contrast
    paint()
    pulse()


window.on("reduced_motion", on_motion)
window.on("high_contrast", on_contrast)
paint()
pulse()

if "--watch" in sys.argv:
    app = App()
    app.add_window(window)
    app.run()
else:
    window.simulate("reduced_motion", value=True)
    assert card.get("stroke_color") == NORMAL["accent"], "snapped, not eased"
    window.simulate("high_contrast", value=True)
    assert card.get("fill") == CONTRAST["fill"]
    print("accessibility_preferences.py: exited cleanly")
