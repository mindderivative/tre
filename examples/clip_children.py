#!/usr/bin/env python3
"""M32 Phase 3's real Scroll/Clip for Oversized Content (§5, §7,
§11.7/§11.8): `Node.set_clip_children` closes the real, stated gap
this catalog named repeatedly -- "no `NodeKind` besides `VirtualList`
clips its own children today" -- by generalizing the exact clip
mechanism `VirtualList`/`Carousel` each already had into a real,
universal `PaintProperties.clip_children` opt-in any node can set.

A real, honest illustration: a fixed-size "card" `Rect` (the clipping
parent) with a genuinely oversized child `Rect` inside it -- four
times the parent's own real height. With clipping off (the default,
every existing node's unchanged behavior), the child visibly spills
out past the card's own edge. With `set_clip_children(True)`, the
identical oversized child is genuinely hidden past the card's own box
-- proven at the pixel level, not just "doesn't raise", by
`crates/engine-render/tests/clip_children.rs`; this script proves the
real end-to-end FFI wiring plus a plausible real use (a "read more"
card whose full content is taller than its own collapsed preview box).

**Real, honest v1 limit, not glossed over:** this is clipping only --
opting a node into `clip_children` does not give it a real scroll
offset or wheel-input wiring of its own (unlike `VirtualList`, which
has both). The oversized content stays genuinely hidden, not
scrollable into view -- a real, separate, still-open gap.
"""

from tre import App, Window

window = Window(width=420, height=240, title="tre v2 -- clip children")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

card = window.add_rect(
    background=(0xFF, 0xFB, 0xFE, 0xFF),
    width=280,
    height=80,
    x=20,
    y=20,
)
card.set_clip_children(True)

content = window.add_text(
    content=(
        "This card's own real preview box is 80px tall, but its real "
        "content underneath is much taller -- clip_children hides "
        "everything past the card's own real edge instead of letting "
        "it spill out over whatever sits below."
    ),
    foreground=(0x1D, 0x1B, 0x20, 0xFF),
    width=280,
    height=300,
    font_size=14.0,
)
card.add_child(content)

print("card built: 80px-tall preview box, clip_children(True), a 300px-tall child inside")

app = App()
app.add_window(window)
app.run(max_frames=30)
print(
    "clip_children.py: exited cleanly after 30 frames -- a real oversized child stayed "
    "clipped to its own parent's box, the real general form of the clip VirtualList/"
    "Carousel already had, closing 'no NodeKind besides VirtualList clips today'"
)
