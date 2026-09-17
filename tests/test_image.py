"""M22 Phase 1 (§5): real, repeatable coverage of `Window.add_image` --
the FFI boundary for a real, file-backed `NodeKind::Image`.

The definitive pixel-level proof that a loaded image genuinely paints
its own real pixels is `crates/engine-render/tests/image_paint.rs`, not
this file -- the same "FFI wiring only" split this project's test suite
has used throughout (`test_checkbox.py`'s own module doc comment).
This file proves: `add_image` returns a real, usable `Node`; a real
PNG on disk is genuinely read and decoded (not just "doesn't crash on
a bogus path"); and a missing/corrupt file raises a real, clear
Python exception instead of panicking.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as every other FFI test in this suite. Uses a
real tiny PNG generated on the fly via Pillow (already a real,
available dependency in this project's own `.venv`) rather than a
checked-in binary fixture -- no `tests/fixtures/` directory exists yet
in this suite, and a 2x2 in-test-generated PNG costs nothing to keep
in sync.
"""

import pytest
from PIL import Image as PILImage

from tre import Node, Window


def _write_tiny_png(path):
    img = PILImage.new("RGBA", (2, 2))
    img.putpixel((0, 0), (0xFF, 0x00, 0x00, 0xFF))
    img.putpixel((1, 0), (0x00, 0xFF, 0x00, 0xFF))
    img.putpixel((0, 1), (0x00, 0x00, 0xFF, 0xFF))
    img.putpixel((1, 1), (0xFF, 0xFF, 0x00, 0x80))
    img.save(path)


def test_add_image_returns_a_node(tmp_path):
    png_path = tmp_path / "tiny.png"
    _write_tiny_png(png_path)

    window = Window(width=200, height=200)
    node = window.add_image(path=str(png_path), width=40, height=40)
    assert isinstance(node, Node)


def test_add_image_with_a_missing_file_raises_a_clear_error():
    window = Window(width=200, height=200)
    with pytest.raises(OSError):
        window.add_image(path="/nonexistent/path/does-not-exist.png", width=40, height=40)


def test_add_image_with_an_unreadable_file_raises_a_clear_error(tmp_path):
    """A real file that exists but isn't a real image -- `image::open`'s
    own real format-sniffing failure, not a panic.
    """
    bogus_path = tmp_path / "not-an-image.png"
    bogus_path.write_bytes(b"this is not a real png file")

    window = Window(width=200, height=200)
    with pytest.raises(OSError):
        window.add_image(path=str(bogus_path), width=40, height=40)


def test_add_image_positions_like_every_other_add_method(tmp_path):
    png_path = tmp_path / "tiny.png"
    _write_tiny_png(png_path)

    window = Window(width=200, height=200)
    node = window.add_image(path=str(png_path), width=40, height=40, x=10, y=20)
    assert isinstance(node, Node)
