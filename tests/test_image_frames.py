"""Streaming frames into an image node: `node.set(rgba=..., pixel_width=...,
pixel_height=...)` replaces its pixels, checks the buffer length, may change
the resolution, and applies only to an image. The app decodes; `tre` shows.
"""

import pytest

from tre import Node, Window
from helpers import add


def _solid_frame(width: int, height: int, byte: int) -> bytes:
    return bytes([byte, 0x00, 0x00, 0xFF]) * (width * height)


def test_create_image_with_a_one_pixel_frame_returns_a_node():
    window = Window(width=200, height=200)
    node = add(window, "image", width=160, height=90, rgba=bytes(4), pixel_width=1, pixel_height=1)
    assert isinstance(node, Node)


def test_a_frame_image_accepts_an_absolute_position():
    window = Window(width=200, height=200)
    node = add(window, "image", width=160, height=90, rgba=bytes(4), pixel_width=1, pixel_height=1, position="absolute", x=10, y=20)
    assert isinstance(node, Node)


@pytest.mark.parametrize("fit", ["cover", "contain", "fill"])
def test_a_frame_image_accepts_each_fit_value(fit):
    window = Window(width=200, height=200)
    node = add(window, "image", width=160, height=90, fit=fit, rgba=bytes(4), pixel_width=1, pixel_height=1)
    assert isinstance(node, Node)


def test_a_frame_image_rejects_an_unknown_fit():
    window = Window(width=200, height=200)
    with pytest.raises(ValueError, match="`fit` must be one of"):
        add(window, "image", width=160, height=90, fit="stretch", rgba=bytes(4), pixel_width=1, pixel_height=1)


def test_setting_a_correctly_sized_frame_does_not_raise():
    window = Window(width=200, height=200)
    node = add(window, "image", width=4, height=2, rgba=bytes(4), pixel_width=1, pixel_height=1)
    node.set(rgba=_solid_frame(4, 2, 0xFF), pixel_width=4, pixel_height=2)


def test_setting_a_wrong_sized_frame_raises_a_clear_error():
    window = Window(width=200, height=200)
    node = add(window, "image", width=4, height=2, rgba=bytes(4), pixel_width=1, pixel_height=1)
    with pytest.raises(ValueError, match="a 4x2 RGBA8 frame needs 32"):
        node.set(rgba=b"\x00" * 10, pixel_width=4, pixel_height=2)


def test_setting_a_frame_on_a_non_image_node_raises():
    window = Window(width=200, height=200)
    rect = add(window, "box", fill=(255, 0, 0, 255), width=40, height=40)
    with pytest.raises(ValueError):
        rect.set(rgba=_solid_frame(4, 2, 0xFF), pixel_width=4, pixel_height=2)


def test_repeated_frames_stream_into_one_image():
    """The app pushes frames at whatever cadence it decides; each
    replaces the last."""
    window = Window(width=200, height=200)
    node = add(window, "image", width=4, height=2, rgba=bytes(4), pixel_width=1, pixel_height=1)
    for byte in (0xFF, 0x80, 0x00, 0x40):
        node.set(rgba=_solid_frame(4, 2, byte), pixel_width=4, pixel_height=2)


def test_a_new_frame_can_change_the_pixel_resolution():
    """The node's box stays fixed while the frame's pixel size changes;
    `fit` resolves the mismatch at paint time."""
    window = Window(width=200, height=200)
    node = add(window, "image", width=160, height=90, rgba=bytes(4), pixel_width=1, pixel_height=1)
    node.set(rgba=_solid_frame(4, 2, 0xFF), pixel_width=4, pixel_height=2)
    node.set(rgba=_solid_frame(8, 6, 0x80), pixel_width=8, pixel_height=6)


def test_a_frame_larger_than_a_gpu_texture_raises_a_clear_error():
    window = Window(width=200, height=200)
    node = add(window, "image", width=160, height=90, rgba=bytes(4), pixel_width=1, pixel_height=1)
    with pytest.raises(ValueError, match="larger than the 8192x8192"):
        node.set(rgba=bytes(9000 * 4), pixel_width=9000, pixel_height=1)
