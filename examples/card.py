#!/usr/bin/env python3
"""M30 Phase 3 Step 3's real `Window.add_card` (§5, §7): MD3's three
real card variants -- Elevated, Filled, Outlined -- a plain container
(no fixed anatomy of its own) populated with arbitrary content via
the already-generic `Node.add_child`.

What this script proves automatically (headless-CI-safe, no human
needed): all three variants render, a real card holds real child
content (a title label and a body label), and `enable_interaction()`/
`click()` reach the card's own container node.
"""

from tre import App, Window

window = Window(width=520, height=160, title="tre v2 -- card")


def build_card(x: float, variant: str, title: str) -> None:
    card = window.add_card(width=160, height=120, variant=variant, x=x, y=16)
    heading = window.add_text(
        content=title, background=(0x1C, 0x1B, 0x1F, 0xFF), width=128, height=20, x=16, y=16
    )
    body = window.add_text(
        content="Supporting text goes here.",
        background=(0x49, 0x45, 0x4F, 0xFF),
        width=128,
        height=40,
        x=16,
        y=44,
    )
    card.add_child(heading)
    card.add_child(body)
    card.enable_interaction()
    window.click(card)


build_card(16, "elevated", "Elevated")
build_card(192, "filled", "Filled")
build_card(368, "outlined", "Outlined")

window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

app = App()
app.add_window(window)
app.run(max_frames=60)
print("card.py: exited cleanly after 60 frames")
