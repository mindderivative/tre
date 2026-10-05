"""Image nodes from straight-alpha RGBA8 bytes: `window.create("image",
rgba=..., pixel_width=..., pixel_height=...)`, the buffer-length check,
positioning, `fit`, and a pixel size independent of the display box. What an
image paints is covered by `engine-render`'s `image_paint.rs`.
"""

import pytest

from tre import Node, Window
from helpers import add


def _solid_rgba(width: int, height: int, byte: int) -> bytes:
    return bytes([byte, 0x00, 0x00, 0xFF]) * (width * height)


def test_create_image_returns_a_node():
    window = Window(width=200, height=200)
    node = add(window, "image", rgba=_solid_rgba(4, 2, 0xFF), pixel_width=4, pixel_height=2, width=40, height=40)
    assert isinstance(node, Node)


def test_create_image_with_a_wrong_sized_buffer_raises_a_clear_error():
    window = Window(width=200, height=200)
    with pytest.raises(ValueError, match="a 4x2 RGBA8 frame needs 32"):
        add(window, "image", rgba=b"\x00" * 10, pixel_width=4, pixel_height=2, width=40, height=40)


def test_create_image_accepts_an_absolute_position():
    window = Window(width=200, height=200)
    node = add(window, "image", rgba=_solid_rgba(4, 2, 0xFF), pixel_width=4, pixel_height=2, width=40, height=40, position="absolute", x=10, y=20)
    assert isinstance(node, Node)


@pytest.mark.parametrize("fit", ["cover", "contain", "fill"])
def test_create_image_accepts_each_fit_value(fit):
    window = Window(width=200, height=200)
    node = add(window, "image", rgba=_solid_rgba(4, 2, 0xFF), pixel_width=4, pixel_height=2, width=40, height=40, fit=fit)
    assert isinstance(node, Node)


def test_create_image_with_an_unknown_fit_raises_a_clear_error():
    window = Window(width=200, height=200)
    with pytest.raises(ValueError, match="`fit` must be one of"):
        add(window, "image", rgba=_solid_rgba(4, 2, 0xFF), pixel_width=4, pixel_height=2, width=40, height=40, fit="stretch")


def test_create_image_pixel_dimensions_can_differ_from_the_display_box():
    """`pixel_width`/`pixel_height` describe the buffer; `width`/`height`
    are the node's box; `fit` resolves any mismatch at paint time.
    """
    window = Window(width=200, height=200)
    node = add(window, "image", rgba=_solid_rgba(4, 2, 0xFF), pixel_width=4, pixel_height=2, width=160, height=90)
    assert isinstance(node, Node)


def test_a_created_image_takes_a_new_frame_through_set():
    window = Window(width=200, height=200)
    node = add(window, "image", rgba=_solid_rgba(4, 2, 0xFF), pixel_width=4, pixel_height=2, width=40, height=40)
    node.set(rgba=_solid_rgba(4, 2, 0x80), pixel_width=4, pixel_height=2)


def test_an_images_last_column_and_row_are_not_blended_with_their_neighbours():
    """0.5.4 (#148): the renderer's `Pad` clamp blended the final texel at the
    right and bottom edges; a 4x1 red, green, blue, white image ended in a mix."""
    row = bytes([255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255])
    window = Window(width=100, height=100)
    add(window, "image", rgba=row, pixel_width=4, pixel_height=1, width=100, height=100,
        position="absolute", x=0, y=0, fit="fill")
    rgba, width, _ = window.snapshot()
    at = lambda x, y: tuple(rgba[(y * width + x) * 4 : (y * width + x) * 4 + 3])  # noqa: E731
    assert at(10, 50) == (255, 0, 0)
    assert at(90, 50) == (255, 255, 255)
    assert at(99, 50) == (255, 255, 255)

    column = bytes([255, 0, 0, 255, 0, 0, 255, 255])  # 1x2: red over blue
    window = Window(width=100, height=100)
    add(window, "image", rgba=column, pixel_width=1, pixel_height=2, width=100, height=100,
        position="absolute", x=0, y=0, fit="fill")
    rgba, width, _ = window.snapshot()
    assert at(50, 90) == (0, 0, 255)
    assert at(50, 99) == (0, 0, 255)
