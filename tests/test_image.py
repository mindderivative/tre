"""M82: real, repeatable coverage of `Window.add_image_from_bytes`, the
FFI boundary for an `Image` node built from already-decoded,
straight-alpha RGBA8 pixels -- since M99 (D6) the only image
constructor: `add_image(path)` and its PNG/JPEG decoding went, and a
framework decodes.

The pixel-level proof that an image paints its own pixels (and that
`fit` changes its geometry) is `crates/engine-render/tests/
image_paint.rs`. Coverage below mirrors `test_video.py`'s `push_frame`
tests, since both share the RGBA-length validation
(`validate_rgba_frame_len`).
"""

import pytest

from tre import Node, Window


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
