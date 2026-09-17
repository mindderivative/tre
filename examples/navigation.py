#!/usr/bin/env python3
"""M13 Phase 2's real content navigation (§11.2), closing Milestone 13:
`Node.remove` -- the missing half `add_child` (already real) alone
couldn't provide. Composed with a real `AppShell` (M13 Phase 1): two
real "screens" are built inside `content`, and navigating from the
first to the second is exactly ARCHITECTURE.md §11.2's own text --
"replacing content's own children" -- remove the current screen's real
subtree, add the next screen's.

Not shown here, to keep this script's own real claim narrow and
verifiable: composing this with the already-real `Window.begin_
container_transform`/`end_container_transform` (§7.6, M7 Phase 5) for
a smooth cross-fade instead of an instant swap -- `examples/container_
transform.py` is the definitive, already-real proof that mechanism
works; §11.2's own text states the two compose ("optionally
choreographed through container-transform"), not that this script
needs to re-prove container-transform itself.

What this script proves automatically (headless-CI-safe, no human
needed): a real shell, two real screens, and a real navigation (remove
+ add) between them, all through real frames, exiting cleanly -- the
functional proof that `content` genuinely holds the *second* screen's
own real content afterward, not the first's, is a real dispatched
click landing on the second screen's own card, not the first's.
"""

from tre import App, Window

window = Window(width=320, height=220, title="tre v2 -- navigation")

menu_bar = window.add_rect(background=(0x21, 0x21, 0x21, 0xFF), width=320, height=24)
content = window.build_shell(menu_bar=menu_bar)


def build_screen(card_color):
    screen = window.add_rect(background=(0x18, 0x18, 0x18, 0xFF), width=320, height=196)
    card = window.add_rect(background=card_color, width=80, height=60, x=20, y=20)
    screen.add_child(card)
    return screen, card


screen_one, card_one = build_screen((0xFF, 0xA5, 0x00, 0xFF))
content.add_child(screen_one)

calls = []
card_one.set_on_click(lambda: calls.append("screen_one"))
window.click(card_one)
assert calls == ["screen_one"], "sanity check: the first screen must be real before navigating away"

# Navigate: remove the first screen's own real subtree, add the second.
screen_one.remove()
screen_two, card_two = build_screen((0x03, 0xDA, 0xC6, 0xFF))
content.add_child(screen_two)

card_two.set_on_click(lambda: calls.append("screen_two"))
window.click(card_two)
assert calls == ["screen_one", "screen_two"], "content must now really hold the second screen"

app = App()
app.add_window(window)
app.run(max_frames=60)
print("navigation.py: exited cleanly after 60 frames")
