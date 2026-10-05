"""PNG encoding for `Window.snapshot()` (0.5.4, #108): `png_bytes` and `write_png`.

The engine returns raw RGBA8 pixels; this turns them into a PNG file with the
standard library alone (`zlib`), so reading or saving what a window drew needs
no imaging package.
"""

import struct
import zlib
from os import PathLike


def png_bytes(rgba: bytes, width: int, height: int) -> bytes:
    """A PNG file's bytes for straight-alpha RGBA8 pixels, top row first.

    Raises `ValueError` if `rgba` isn't `width * height * 4` bytes.
    """
    if width <= 0 or height <= 0:
        raise ValueError("png_bytes: width and height must be greater than 0")
    row = width * 4
    if len(rgba) != row * height:
        raise ValueError(
            f"png_bytes: expected {row * height} bytes for {width} x {height} RGBA, got {len(rgba)}"
        )
    # Each scanline is prefixed with its filter type: 0, none.
    raw = b"".join(b"\x00" + rgba[y * row : (y + 1) * row] for y in range(height))

    def chunk(kind: bytes, data: bytes) -> bytes:
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)  # 8-bit RGBA
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(raw, 6))
        + chunk(b"IEND", b"")
    )


def write_png(path: str | PathLike[str], rgba: bytes, width: int, height: int) -> None:
    """Writes straight-alpha RGBA8 pixels, as `Window.snapshot()` returns them,
    to `path` as a PNG."""
    data = png_bytes(rgba, width, height)
    with open(path, "wb") as out:
        out.write(data)
