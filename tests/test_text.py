"""M96 Phase 3: `window.measure_text` and a text node's line layout --
`font_style`, `letter_spacing`, `wrap`, `max_lines`, and `overflow`.
Measurement shares the painter's layout code (`engine-render`'s
`build_text_layout`), whose pixels `m96_paint.rs` checks.
"""

from __future__ import annotations

from typing import Any

import pytest

import tre

SENTENCE = "the quick brown fox jumps over the lazy dog " * 4


def measure(**kwargs: Any) -> tuple[float, float]:
    return tre.Window(100, 100, "text").measure_text(**kwargs)


def test_measure_one_line() -> None:
    width, height = measure(text="Hello", font_size=16)
    assert 20 < width < 60
    assert 14 < height < 30
    wider, _ = measure(text="Hello, world", font_size=16)
    assert wider > width
    bigger, taller = measure(text="Hello", font_size=32)
    assert bigger > width * 1.8 and taller > height * 1.8


def test_measure_wraps_within_max_width() -> None:
    _, one_line = measure(text="x")
    width, height = measure(text=SENTENCE, max_width=150)
    assert width <= 150
    assert height > one_line * 5


def test_max_lines_caps_the_height() -> None:
    _, one_line = measure(text="x")
    _, height = measure(text=SENTENCE, max_width=150, max_lines=2)
    assert height == pytest.approx(one_line * 2, abs=1)


def test_an_unwrapped_ellipsis_fits_the_width() -> None:
    full, _ = measure(text=SENTENCE, wrap="none")
    cut, height = measure(text=SENTENCE, wrap="none", max_width=120, overflow="ellipsis")
    _, one_line = measure(text="x")
    assert full > 1000
    assert 100 < cut <= 120, "fills the width, never past it"
    assert height == pytest.approx(one_line, abs=1)


def test_a_wrapped_ellipsis_ends_the_last_shown_line() -> None:
    width, height = measure(text=SENTENCE, max_width=150, max_lines=2, overflow="ellipsis")
    _, one_line = measure(text="x")
    assert width <= 150
    assert height == pytest.approx(one_line * 2, abs=1)


def test_letter_spacing_adds_space_after_each_character() -> None:
    base, _ = measure(text="abcdefghij")
    spaced, _ = measure(text="abcdefghij", letter_spacing=2)
    assert spaced == pytest.approx(base + 20, abs=2)


def test_italics_are_synthesized_without_an_italic_face() -> None:
    upright = measure(text="Hello")
    italic = measure(text="Hello", font_style="italic")
    assert italic[1] == pytest.approx(upright[1], abs=1)


@pytest.mark.parametrize(
    ("kwargs", "message"),
    [
        ({"font_style": "oblique"}, "`font_style` must be one of: normal, italic"),
        ({"wrap": "char"}, "`wrap` must be one of: word, none"),
        ({"overflow": "fade"}, "`overflow` must be one of: clip, ellipsis"),
        ({"font_size": 0}, "`font_size` must be a positive number"),
    ],
)
def test_measure_rejects_bad_values(kwargs: dict[str, Any], message: str) -> None:
    with pytest.raises(ValueError, match=message):
        measure(text="x", **kwargs)


def test_text_layout_props_read_back() -> None:
    w = tre.Window(100, 100, "text")
    values: dict[str, Any] = {
        "font_style": "italic",
        "letter_spacing": 1.5,
        "wrap": "none",
        "max_lines": 2,
        "overflow": "ellipsis",
    }
    text = w.create("text", text="x", **values)
    assert {name: text.get(name) for name in values} == values
    with pytest.raises(ValueError, match="`max_lines` applies only to a text node"):
        w.create("text_input").set(max_lines=1)


def test_proof_ellipsized_list_item() -> None:
    """A list row's title, one line, cut with an ellipsis -- sized by the
    framework from `measure_text`, the way a content-sized widget is."""
    w = tre.Window(300, 200, "text")
    style: dict[str, Any] = {"font_size": 14, "max_lines": 1, "overflow": "ellipsis"}
    width, height = w.measure_text(text=SENTENCE, max_width=180, **style)
    title = w.create("text", text=SENTENCE, width=180, height=height, **style)
    row = w.create("box", padding=8, align_self="start")
    row.add_child(title)
    w.root.add_child(row)
    assert width <= 180
    assert row.get("layout_height") == pytest.approx(height + 16, abs=1)
