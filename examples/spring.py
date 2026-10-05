#!/usr/bin/env python3
"""Springs: animation with overshoot and a retarget that keeps its speed.

`easing=("spring", bounce)` animates with a damped spring: `duration_ms` is its
period, `bounce` how far it overshoots (0 is none, towards 1 rings more). It lasts
until it settles. Three cards move to the same place with different bounce, and
the last is retargeted halfway to show the motion carrying on instead of stalling.

Headless-CI-safe: runs by `window.advance`, no display needed, and prints each
card's peak. Pass `--watch` to see it live and click to retarget.
See docs/guide/animation.md.
"""

import sys

from tre import App, Window

window = Window(width=520, height=240, title="tre -- springs")
window.root.set(padding=0)
cards = []
for i, bounce in enumerate((0.0, 0.35, 0.7)):
    card = window.create("box", width=60, height=60, x=20, y=20 + i * 70, position="absolute",
                         corner_radius=12, fill=(0x67, 0x50, 0xA4, 0xFF))
    window.root.add_child(card)
    cards.append((bounce, card))


def launch(target: float = 400) -> None:
    for bounce, card in cards:
        card.animate("translate_x", target, 500, easing=("spring", bounce))


launch()

if "--watch" in sys.argv:
    window.root.on("click", lambda e: launch(40 if cards[0][1].get_target("translate_x") > 200 else 400))
    app = App()
    app.add_window(window)
    app.run()
else:
    peaks = [0.0] * len(cards)
    for step in range(300):
        window.advance(10)
        if step == 25:  # retarget the last card mid-flight, back toward the start
            cards[-1][1].animate("translate_x", 40, 500, easing=("spring", 0.35))
        for i, (_, card) in enumerate(cards[:2]):
            peaks[i] = max(peaks[i], card.get("translate_x"))
    for (bounce, card), peak in zip(cards[:2], peaks):
        print(f"spring.py: bounce {bounce}: peak {peak:.1f}, ends at {card.get('translate_x'):.1f}")
    assert peaks[0] <= 400.5 and peaks[1] > 405
    print("spring.py: exited cleanly")
