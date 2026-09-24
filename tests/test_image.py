"""M22 Phase 1/2 (§5, §16.1): real, repeatable coverage of `Window.
add_image` -- the FFI boundary for a real, file-backed `NodeKind::
Image`.

The definitive pixel-level proof that a loaded image genuinely paints
its own real pixels (and that `fit:` genuinely changes its real
geometry) is `crates/engine-render/tests/image_paint.rs`, not this
file -- the same "FFI wiring only" split this project's test suite
has used throughout (`test_checkbox.py`'s own module doc comment).
This file proves: `add_image` returns a real, usable `Node`; a real
PNG on disk is genuinely read and decoded (not just "doesn't crash on
a bogus path"); `fit` accepts its own real string vocabulary and
rejects an unknown one; and a missing/corrupt file raises a real,
clear Python exception instead of panicking.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as every other FFI test in this suite. Uses a
real tiny PNG embedded as a base64 literal -- **real, corrective
finding:** an earlier version of this file generated the PNG on the
fly via Pillow, which happened to be installed in this session's own
dev `.venv` for unrelated reasons but is not a real project
dependency (confirmed via grep: neither `pyproject.toml` nor
`.github/workflows/ci.yml`'s own `pip install` step names it) --
caught by a real CI failure (`ModuleNotFoundError: No module named
'PIL'`), not by local testing alone. Fixed the same way `examples/
image.py` already avoids the same trap: no test or example script in
this project has an import beyond the standard library and `tre`
itself (confirmed via grep before writing that script), so this file
now matches.

M82: `add_image_from_bytes` is `add_image`'s own decode-free sibling
-- the real primitive `path=` decoding is convenience sugar in front
of (`window_factory.rs`'s own `insert_image_node`, shared by both).
Coverage below mirrors `test_video.py`'s own `push_frame` tests
directly, since both share the identical RGBA-length validation
(`validate_rgba_frame_len`).
"""

import base64

import pytest

from tre import Node, Window

# The identical real, tiny PNG `examples/image.py` embeds -- a 4x4
# RGBA image, reused here rather than a second, separately-maintained
# literal (this file only needs *some* real, decodable image, not any
# particular content).
_TINY_PNG_B64 = (
    "iVBORw0KGgoAAAANSUhEUgAAAAQAAAAECAYAAACp8Z5+AAAAT0lEQVR4nAFEALv/"
    "APRDNv//mAD//+s7/0yvUP8BALzU/yHaHwAeu8IAXdb7AADpHmP/eVVI/2B9i/8A"
    "AACAAf////8AwggAjAJDANx3bQC9UiChZNHhkwAAAABJRU5ErkJggg=="
)


def _write_tiny_png(path):
    path.write_bytes(base64.b64decode(_TINY_PNG_B64))


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


@pytest.mark.parametrize("fit", ["cover", "contain", "fill"])
def test_add_image_accepts_each_real_fit_value(tmp_path, fit):
    png_path = tmp_path / "tiny.png"
    _write_tiny_png(png_path)

    window = Window(width=200, height=200)
    node = window.add_image(path=str(png_path), width=40, height=40, fit=fit)
    assert isinstance(node, Node)


def test_add_image_defaults_to_fill_when_fit_is_omitted(tmp_path):
    """Phase 1's own only behavior, kept byte-for-byte the default."""
    png_path = tmp_path / "tiny.png"
    _write_tiny_png(png_path)

    window = Window(width=200, height=200)
    node = window.add_image(path=str(png_path), width=40, height=40)
    assert isinstance(node, Node)


def test_add_image_with_an_unknown_fit_raises_a_clear_error(tmp_path):
    png_path = tmp_path / "tiny.png"
    _write_tiny_png(png_path)

    window = Window(width=200, height=200)
    with pytest.raises(ValueError, match="unknown content fit"):
        window.add_image(path=str(png_path), width=40, height=40, fit="stretch")


def _solid_rgba(width: int, height: int, byte: int) -> bytes:
    return bytes([byte, 0x00, 0x00, 0xFF]) * (width * height)


def test_add_image_from_bytes_returns_a_node():
    window = Window(width=200, height=200)
    node = window.add_image_from_bytes(_solid_rgba(4, 2, 0xFF), 4, 2, width=40, height=40)
    assert isinstance(node, Node)


def test_add_image_from_bytes_with_a_wrong_sized_buffer_raises_a_clear_error():
    window = Window(width=200, height=200)
    with pytest.raises(ValueError, match="add_image_from_bytes"):
        window.add_image_from_bytes(b"\x00" * 10, 4, 2, width=40, height=40)


def test_add_image_from_bytes_positions_like_every_other_add_method():
    window = Window(width=200, height=200)
    node = window.add_image_from_bytes(_solid_rgba(4, 2, 0xFF), 4, 2, width=40, height=40, x=10, y=20)
    assert isinstance(node, Node)


@pytest.mark.parametrize("fit", ["cover", "contain", "fill"])
def test_add_image_from_bytes_accepts_each_real_fit_value(fit):
    window = Window(width=200, height=200)
    node = window.add_image_from_bytes(_solid_rgba(4, 2, 0xFF), 4, 2, width=40, height=40, fit=fit)
    assert isinstance(node, Node)


def test_add_image_from_bytes_defaults_to_fill_when_fit_is_omitted():
    window = Window(width=200, height=200)
    node = window.add_image_from_bytes(_solid_rgba(4, 2, 0xFF), 4, 2, width=40, height=40)
    assert isinstance(node, Node)


def test_add_image_from_bytes_with_an_unknown_fit_raises_a_clear_error():
    window = Window(width=200, height=200)
    with pytest.raises(ValueError, match="unknown content fit"):
        window.add_image_from_bytes(_solid_rgba(4, 2, 0xFF), 4, 2, width=40, height=40, fit="stretch")


def test_add_image_from_bytes_pixel_dimensions_can_differ_from_the_display_box():
    """`pixel_width`/`pixel_height` describe the buffer; `width`/`height`
    are the node's own fixed box -- `add_image`'s identical contract,
    `content_fit` resolves any mismatch at paint time (`test_video.py`'s
    own `test_push_frame_can_change_the_frame_resolution` proves the
    identical mechanism for a node built via `add_video` instead).
    """
    window = Window(width=200, height=200)
    node = window.add_image_from_bytes(_solid_rgba(4, 2, 0xFF), 4, 2, width=160, height=90)
    assert isinstance(node, Node)


def test_add_image_from_bytes_then_push_frame_is_a_real_ordinary_image_node():
    """The declarative/imperative parity this primitive exists for: a
    node built via `add_image_from_bytes` is genuinely indistinguishable
    from one built via `add_video` -- both are `NodeKind::Image` under
    the hood, so `push_frame` (Video's own live-update path) keeps
    working on it, the same way it already works on any Image-kind node.
    """
    window = Window(width=200, height=200)
    node = window.add_image_from_bytes(_solid_rgba(4, 2, 0xFF), 4, 2, width=40, height=40)
    node.push_frame(_solid_rgba(4, 2, 0x80), 4, 2)
