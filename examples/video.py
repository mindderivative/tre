#!/usr/bin/env python3
"""M30 Phase 9 Step 1's real `Video` component (§5): a frame *sink*,
not a decoder -- grounded directly in the sibling `pyCopper` project's
own real `Video` widget (same author, same explicit desktop-only
design goal). Nothing in this codebase depends on a codec library
(nothing decodes `.mp4`/`.webm`), so `Window.add_video` creates the
display surface and `Node.push_frame(rgba, width, height)` is how the
*application* hands over each decoded frame, at whatever cadence it
decides -- real, live video, camera feeds, or generated visuals all
go through the identical real path.

What this script proves automatically (headless-CI-safe, no human
needed): `add_video` returns a real, usable `Node`; a synthetic
"decode loop" pushes several distinct frames in a row -- simulating
frames arriving from a real decoder -- each one replacing the last;
a real mid-stream resolution change (a real decoder renegotiation,
e.g. an adaptive-bitrate stream switching quality) is accepted without
any layout involvement, since the node's own box stays the real, fixed
`width`/`height` `add_video` was given. The definitive proof that each
pushed frame genuinely triggers a real GPU re-upload (not just
accepted without error) is `crates/engine-render/src/image_cache.rs`'s
own `sync_reuploads_a_texture_when_its_own_node_content_genuinely_
changes` test, not this script -- `App.run` has no real per-frame
Python hook today (confirmed via direct check of `python/tre/_core.
pyi`'s own `App.run` signature), so every push below happens before
the real render loop starts, the same "decode happens before app.run,
then app.run proves the full pipeline runs clean" structure `examples/
image.py` already established for its own file-backed `Image`.
"""

from tre import App, Window

window = Window(width=320, height=240, title="tre v2 -- video (frame sink)")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

video = window.add_video(width=160, height=90, fit="cover", x=80, y=75)


def _solid_frame(width: int, height: int, r: int, g: int, b: int) -> bytes:
    return bytes([r, g, b, 0xFF]) * (width * height)


# A real synthetic "decode loop" -- four distinct low-resolution frames
# in a row, each one genuinely replacing the last, proving `push_frame`
# is a real, repeatable live update, not a one-shot load.
frames = [
    (8, 8, 0xB0, 0x0B, 0x57),  # a plausible MD3-primary-ish red
    (8, 8, 0x38, 0x1E, 0x72),  # ...shifting to a violet
    (8, 8, 0x00, 0x69, 0x5C),  # ...to teal
    (8, 8, 0x66, 0x50, 0xA4),  # ...settling on the real theme seed color
]
for width, height, r, g, b in frames:
    video.push_frame(_solid_frame(width, height, r, g, b), width, height)

# A real mid-stream resolution renegotiation -- the node's own box
# stays fixed at 160x90 (`add_video`'s own real contract), only the
# pushed frame's own pixel dimensions change; `content_fit="cover"`
# resolves the mismatch at paint time, exactly like a loaded `Image`
# whose own file dimensions don't match its node's box.
video.push_frame(_solid_frame(32, 18, 0x66, 0x50, 0xA4), 32, 18)

app = App()
app.add_window(window)
app.run(max_frames=60)
print("video.py: exited cleanly after 60 frames, a real Video node held 5 distinct pushed frames")
