"""0.5.4 (#108): `Window.snapshot()` and the PNG helpers."""

import struct
import zlib

import pytest

import tre
from tre import Window


def pixel(shot, x, y):
    rgba, width, _ = shot
    i = (y * width + x) * 4
    return tuple(rgba[i : i + 4])


def boxed_window():
    window = Window(width=80, height=50)
    window.root.set(fill=(0x10, 0x20, 0x30, 255))
    box = window.create("box", width=20, height=10, x=10, y=5, position="absolute",
                        fill=(255, 0, 0, 255))
    window.root.add_child(box)
    return window


def test_a_window_that_was_never_run_can_be_snapshot():
    shot = boxed_window().snapshot()
    rgba, width, height = shot
    assert (width, height) == (80, 50)
    assert len(rgba) == 80 * 50 * 4
    assert pixel(shot, 0, 0) == (0x10, 0x20, 0x30, 255)
    assert pixel(shot, 15, 8) == (255, 0, 0, 255)
    assert pixel(shot, 9, 8) == (0x10, 0x20, 0x30, 255)


def test_scale_multiplies_the_pixels_not_the_layout():
    window = boxed_window()
    shot = window.snapshot(scale=2.0)
    assert (shot[1], shot[2]) == (160, 100)
    assert pixel(shot, 30, 16) == (255, 0, 0, 255)
    assert pixel(shot, 19, 16) == (0x10, 0x20, 0x30, 255)


def test_a_snapshot_at_another_size_leaves_the_window_as_it_was():
    window = boxed_window()
    shot = window.snapshot(width=40, height=25)
    assert (shot[1], shot[2]) == (40, 25)
    assert (window.get("width"), window.get("height")) == (80, 50)
    again = window.snapshot()
    assert (again[1], again[2]) == (80, 50)
    assert pixel(again, 15, 8) == (255, 0, 0, 255)


def test_a_snapshot_is_repeatable():
    window = boxed_window()
    assert window.snapshot() == window.snapshot()


@pytest.mark.parametrize("kwargs", [{"width": 0}, {"height": -1}, {"scale": 0}, {"scale": float("nan")}])
def test_a_bad_size_or_scale_is_a_value_error(kwargs):
    with pytest.raises(ValueError, match="greater than 0"):
        boxed_window().snapshot(**kwargs)


def test_a_size_the_gpu_cannot_render_is_a_runtime_error():
    with pytest.raises(RuntimeError, match="outside what this GPU can render"):
        boxed_window().snapshot(width=100000)


def decode_png(data):
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    pos, chunks = 8, {}
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos : pos + 4])
        kind = data[pos + 4 : pos + 8]
        body = data[pos + 8 : pos + 8 + length]
        (crc,) = struct.unpack(">I", data[pos + 8 + length : pos + 12 + length])
        assert crc == zlib.crc32(kind + body)
        chunks.setdefault(kind, b"")
        chunks[kind] += body
        pos += 12 + length
    width, height, depth, color = struct.unpack(">IIBB", chunks[b"IHDR"][:10])
    assert (depth, color) == (8, 6)
    raw = zlib.decompress(chunks[b"IDAT"])
    row = width * 4
    pixels = b"".join(raw[y * (row + 1) + 1 : (y + 1) * (row + 1)] for y in range(height))
    return width, height, pixels


def test_png_round_trips_the_pixels(tmp_path):
    rgba, width, height = boxed_window().snapshot()
    path = tmp_path / "shot.png"
    tre.write_png(path, rgba, width, height)
    assert decode_png(path.read_bytes()) == (width, height, rgba)


def test_png_bytes_checks_the_length():
    with pytest.raises(ValueError, match="expected 16 bytes"):
        tre.png_bytes(b"\x00" * 15, 2, 2)
    with pytest.raises(ValueError, match="greater than 0"):
        tre.png_bytes(b"", 0, 1)
