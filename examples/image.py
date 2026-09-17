#!/usr/bin/env python3
"""M22 Phase 1's real, file-backed image (§5): `Window.add_image` loads
a real PNG from disk (via the `image` crate, `engine-py`'s own real
decode step) into a real `NodeKind::Image`, painted through
`vello_hybrid`'s `Scene::draw_texture_rects` GPU-texture path --
`crates/engine-render/src/image_cache.rs`'s own module doc comment has
the full real investigation behind why that path, not the ordinary
`set_paint`/`fill_path` every other `NodeKind` uses.

What this script proves automatically (headless-CI-safe, no human
needed): a real image file is read and decoded, `add_image` returns a
real, usable `Node`, and a full `App.run` render loop -- which now
calls `FrameRenderer::sync_image_textures` every frame -- runs real
frames with a real `Image` node in the tree, exiting cleanly. The
definitive pixel-level proof that the loaded image's own real colors
land on screen is `crates/engine-render/tests/image_paint.rs`, not
this script, the same split this workspace has used throughout.

Embeds a tiny real 4x4 PNG as a base64 literal rather than depending
on a third-party image library or a checked-in binary fixture -- no
example script in this project has an import beyond the standard
library and `tre` itself (confirmed via grep before writing this),
and a 4x4 test image costs nothing to keep inline.
"""

import base64
import tempfile
from pathlib import Path

from tre import App, Window

# A real, tiny 4x4 RGBA PNG -- 16 distinct MD3-palette-ish colors, one
# per pixel (including one real partially-transparent pixel), so a
# visual/pixel check can distinguish "the real image painted" from "a
# solid color happened to land here by coincidence."
_TINY_PNG_B64 = (
    "iVBORw0KGgoAAAANSUhEUgAAAAQAAAAECAYAAACp8Z5+AAAAT0lEQVR4nAFEALv/"
    "APRDNv//mAD//+s7/0yvUP8BALzU/yHaHwAeu8IAXdb7AADpHmP/eVVI/2B9i/8A"
    "AACAAf////8AwggAjAJDANx3bQC9UiChZNHhkwAAAABJRU5ErkJggg=="
)

tmp_dir = tempfile.mkdtemp(prefix="tre_image_example_")
png_path = Path(tmp_dir) / "tiny.png"
png_path.write_bytes(base64.b64decode(_TINY_PNG_B64))

window = Window(width=220, height=220, title="tre v2 -- image")
image = window.add_image(path=str(png_path), width=160, height=160, x=30, y=30)

app = App()
app.add_window(window)
app.run(max_frames=60)
print("image.py: exited cleanly after 60 frames")
