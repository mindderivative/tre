"""0.5.4 (#127): `window.set(glyph_cache=True)` draws text from a glyph cache."""

import pytest

from tre import Window


def scene():
    window = Window(width=160, height=60)
    window.root.set(fill=(10, 10, 14, 255), padding=4)
    label = window.create("text", text="Hello, glyph cache", font_size=18, fill=(255, 255, 255, 255))
    window.root.add_child(label)
    return window


def lit(window):
    rgba, _, _ = window.snapshot()
    return sum(1 for i in range(0, len(rgba), 4) if rgba[i] > 128)


def test_the_glyph_cache_is_off_by_default():
    assert Window(width=10, height=10).get("glyph_cache") is False


def test_it_can_be_switched_on_and_off():
    window = scene()
    window.set(glyph_cache=True)
    assert window.get("glyph_cache") is True
    window.set(glyph_cache=False)
    assert window.get("glyph_cache") is False


def test_it_must_be_a_bool():
    with pytest.raises(ValueError):
        Window(width=10, height=10).set(glyph_cache="yes")


def test_text_is_still_drawn_with_it_on_and_snapshots_repeat():
    window = scene()
    plain = lit(window)
    window.set(glyph_cache=True)
    cached = lit(window)
    assert plain > 100
    assert abs(cached - plain) < plain * 0.25
    assert window.snapshot() == window.snapshot()


def test_switching_it_back_restores_the_exact_outline_pixels():
    window = scene()
    before = window.snapshot()
    window.set(glyph_cache=True)
    window.set(glyph_cache=False)
    assert window.snapshot() == before
