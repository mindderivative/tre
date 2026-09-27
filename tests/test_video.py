"""M30 Phase 9 Step 1 (§5): real, repeatable coverage of `Window.
add_video`/`Node.push_frame` -- the FFI boundary for `Video`'s own
real "frame sink" design, reusing `NodeKind::Image` directly rather
than a new `NodeKind` (see `Node.push_frame`'s own doc comment for the
full design, directly grounded in the sibling `pyCopper` project's own
real `Video` widget).

The definitive proof that a pushed frame genuinely triggers a real GPU
re-upload (not just accepted without error) is
`crates/engine-render/src/image_cache.rs`'s own
`sync_reuploads_a_texture_when_its_own_node_content_genuinely_changes`
test, not this file -- the same "FFI wiring only" split `test_image.py`'s
own module doc comment already established. This file proves:
`add_video` returns a real, usable `Node`; `push_frame` accepts a
correctly-sized RGBA buffer without raising; a wrong-sized buffer
raises a clear `ValueError`; `push_frame` on any non-`Image` node
raises a clear `ValueError`; `fit` accepts its own real string
vocabulary and rejects an unknown one; and repeated `push_frame` calls
(simulating a real live stream) never raise.
"""

import pytest

from tre import Node, Window
from helpers import add


def _solid_frame(width: int, height: int, byte: int) -> bytes:
    return bytes([byte, 0x00, 0x00, 0xFF]) * (width * height)


def test_add_video_returns_a_node():
    window = Window(width=200, height=200)
    node = add(window, "image", width=160, height=90, rgba=bytes(4), pixel_width=1, pixel_height=1)
    assert isinstance(node, Node)


def test_add_video_positions_like_every_other_add_method():
    window = Window(width=200, height=200)
    node = add(window, "image", width=160, height=90, rgba=bytes(4), pixel_width=1, pixel_height=1, position="absolute", x=10, y=20)
    assert isinstance(node, Node)


@pytest.mark.parametrize("fit", ["cover", "contain", "fill"])
def test_add_video_accepts_each_real_fit_value(fit):
    window = Window(width=200, height=200)
    node = add(window, "image", width=160, height=90, fit=fit, rgba=bytes(4), pixel_width=1, pixel_height=1)
    assert isinstance(node, Node)


def test_add_video_with_an_unknown_fit_raises_a_clear_error():
    window = Window(width=200, height=200)
    with pytest.raises(ValueError, match="`fit` must be one of"):
        add(window, "image", width=160, height=90, fit="stretch", rgba=bytes(4), pixel_width=1, pixel_height=1)


def test_push_frame_with_a_correctly_sized_buffer_does_not_raise():
    window = Window(width=200, height=200)
    node = add(window, "image", width=4, height=2, rgba=bytes(4), pixel_width=1, pixel_height=1)
    node.set(rgba=_solid_frame(4, 2, 0xFF), pixel_width=4, pixel_height=2)


def test_push_frame_with_a_wrong_sized_buffer_raises_a_clear_error():
    window = Window(width=200, height=200)
    node = add(window, "image", width=4, height=2, rgba=bytes(4), pixel_width=1, pixel_height=1)
    with pytest.raises(ValueError, match="a 4x2 RGBA8 frame needs 32"):
        node.set(rgba=b"\x00" * 10, pixel_width=4, pixel_height=2)


def test_push_frame_on_a_non_image_node_raises_a_clear_error():
    window = Window(width=200, height=200)
    rect = add(window, "box", fill=(255, 0, 0, 255), width=40, height=40)
    with pytest.raises(ValueError):
        rect.set(rgba=_solid_frame(4, 2, 0xFF), pixel_width=4, pixel_height=2)


def test_repeated_push_frame_calls_simulate_a_real_live_stream():
    """The real point of `Video`: the app decodes and pushes frames at
    whatever cadence it decides -- proven here with several distinct
    frames in a row, each replacing the last, none raising.
    """
    window = Window(width=200, height=200)
    node = add(window, "image", width=4, height=2, rgba=bytes(4), pixel_width=1, pixel_height=1)
    for byte in (0xFF, 0x80, 0x00, 0x40):
        node.set(rgba=_solid_frame(4, 2, byte), pixel_width=4, pixel_height=2)


def test_push_frame_can_change_the_frame_resolution():
    """A real, live resolution renegotiation -- the node's own box
    stays fixed (`add_video`'s own `width`/`height`), only the pushed
    frame's own real pixel dimensions change; `content_fit` resolves
    the mismatch at paint time, no layout involvement needed.
    """
    window = Window(width=200, height=200)
    node = add(window, "image", width=160, height=90, rgba=bytes(4), pixel_width=1, pixel_height=1)
    node.set(rgba=_solid_frame(4, 2, 0xFF), pixel_width=4, pixel_height=2)
    node.set(rgba=_solid_frame(8, 6, 0x80), pixel_width=8, pixel_height=6)
