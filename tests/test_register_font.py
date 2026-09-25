"""M86: `tre.register_font(data)` -- fonts handed to `tre` as bytes.

`tre` never reads a font file itself; the caller loads the bytes and
registers them. The render-level proof that a registered family is
actually shaped with its own face (not a silent fallback) lives in
`engine-render`'s own Rust tests, which can build a renderer that
genuinely lacks the family first -- these tests cover the Python
surface.
"""

from pathlib import Path

import pytest

import tre

FONTS_DIR = Path(__file__).resolve().parent.parent / "crates" / "engine-render" / "assets" / "fonts"


def test_register_font_returns_the_real_family_name():
    data = (FONTS_DIR / "HackNerdFontMono-Regular.ttf").read_bytes()
    assert tre.register_font(data) == ["Hack Nerd Font Mono"]


def test_the_returned_name_matches_the_exported_monospace_family():
    data = (FONTS_DIR / "HackNerdFontMono-Regular.ttf").read_bytes()
    assert tre.MONOSPACE_FONT_FAMILY in tre.register_font(data)


def test_registering_the_same_bytes_twice_is_harmless():
    data = (FONTS_DIR / "Roboto-Regular.ttf").read_bytes()
    first = tre.register_font(data)
    assert tre.register_font(data) == first == ["Roboto"]


def test_bytes_with_no_font_face_raise_value_error():
    with pytest.raises(ValueError, match="no font faces found"):
        tre.register_font(b"definitely not a font")


def test_empty_bytes_raise_value_error():
    with pytest.raises(ValueError, match="no font faces found"):
        tre.register_font(b"")


def test_a_path_string_is_rejected_the_caller_must_read_the_file():
    with pytest.raises(TypeError):
        tre.register_font(str(FONTS_DIR / "Roboto-Regular.ttf"))  # type: ignore[arg-type]
