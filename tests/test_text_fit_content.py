"""0.5.6.2 (#176, #177): `width="fit-content"` and `height="fit-content"`.

A text node has no size of its own: unsized it is zero wide, so a label in a box
that centres its children wrapped one word to a line (#177), and an effect shader
over a text node with padding cropped the text to the padding's size (#176). A side
set to `"fit-content"` takes the size of its text, measured by the shaper
`Window.measure_text` uses.
"""

import math

import pytest

from tre import Shader, Window

LABEL = "Julia set, drawn by a fill shader"
STYLE = dict(font_size=22)
WHITE = (255, 255, 255, 255)
PASSTHROUGH = "fn shade(p: Pixel) -> vec4<f32> { return content(p.uv); }"


def label_in_box(align, box_width=480, **props):
    """A fit-content label in a vertical box, laid out; returns (window, label)."""
    window = Window(width=560, height=360)
    box = window.create(
        "box",
        width=box_width,
        height=300,
        flex_direction="vertical",
        align_items=align,
        justify_content="flex_end",
        padding=20,
    )
    window.root.add_child(box)
    label = window.create(
        "text", text=LABEL, fill=WHITE, width="fit-content", height="fit-content", **STYLE, **props
    )
    box.add_child(label)
    window.advance(16)
    return window, label


@pytest.mark.parametrize("align", ["center", "flex_start"])
def test_a_centred_or_start_aligned_label_is_one_line_of_its_own_width(align):
    # The table in #177: these two used to be 0 wide and wrapped.
    window, label = label_in_box(align)
    width, line = window.measure_text(LABEL, **STYLE)
    assert label.get("layout_width") == math.ceil(width)
    assert label.get("layout_height") == line


def test_a_stretched_label_fills_the_box_and_is_still_one_line():
    window, label = label_in_box("stretch")
    _, line = window.measure_text(LABEL, **STYLE)
    assert label.get("layout_width") == 440  # the box, inside its padding
    assert label.get("layout_height") == line


@pytest.mark.parametrize("align", ["center", "flex_start"])
def test_a_label_wraps_within_the_space_it_is_offered(align):
    window, label = label_in_box(align, box_width=200)  # 160 inside the padding
    assert label.get("layout_width") <= 160
    # The height is the wrapped text's, at the width the node got.
    _, wrapped = window.measure_text(LABEL, max_width=label.get("layout_width"), **STYLE)
    assert label.get("layout_height") == wrapped
    assert wrapped > window.measure_text(LABEL, **STYLE)[1]


def test_text_that_does_not_wrap_stays_one_line():
    window, label = label_in_box("center", box_width=200, wrap="none")
    width, line = window.measure_text(LABEL, wrap="none", **STYLE)
    assert label.get("layout_width") == math.ceil(width)
    assert label.get("layout_height") == line


def test_changing_the_text_lays_the_label_out_again():
    window, label = label_in_box("center")
    before = label.get("layout_width")
    label.set(text="Short")
    window.advance(16)
    assert label.get("layout_width") == math.ceil(window.measure_text("Short", **STYLE)[0])
    assert label.get("layout_width") < before


def test_changing_the_font_size_lays_the_label_out_again():
    window, label = label_in_box("center")
    before = label.get("layout_width")
    label.set(font_size=44)
    window.advance(16)
    assert label.get("layout_width") > before


def test_get_reads_fit_content_back_and_any_other_width_clears_it():
    window = Window(width=300, height=100)
    label = window.create("text", text="Hi", width="fit-content", height="fit-content", **STYLE)
    assert label.get("width") == "fit-content" and label.get("height") == "fit-content"
    label.set(width=80)
    assert label.get("width") == 80 and label.get("height") == "fit-content"
    label.set(height="auto")
    assert label.get("height") == "auto"
    label.set(width="fit-content")
    label.set(width="50%")
    assert label.get("width") == "50%"


def test_an_unsized_label_is_still_the_zero_it_always_was():
    window = Window(width=560, height=360)
    box = window.create("box", width=480, height=300, flex_direction="vertical", align_items="center")
    window.root.add_child(box)
    label = window.create("text", text=LABEL, fill=WHITE, **STYLE)
    box.add_child(label)
    window.advance(16)
    assert label.get("width") == "auto"
    assert label.get("layout_width") == 0


def test_a_box_accepts_fit_content_and_is_sized_to_its_content_as_with_auto():
    window = Window(width=300, height=200)
    boxes = []
    for size in ("fit-content", "auto"):
        box = window.create("box", width=size, height=size, padding=10)
        window.root.add_child(box)
        box.add_child(window.create("box", width=60, height=30))
        boxes.append(box)
    window.advance(16)
    fit, auto = [(b.get("layout_width"), b.get("layout_height")) for b in boxes]
    assert fit == auto


def ink(snapshot):
    """Bright pixels in a snapshot: the white text on its dark ground."""
    rgba, _, _ = snapshot
    return sum(1 for i in range(0, len(rgba), 4) if rgba[i + 3] > 0 and max(rgba[i : i + 3]) > 100)


def passthrough_ink(padding, effect):
    window = Window(width=300, height=140)
    window.root.set(fill=(0x10, 0x12, 0x1C, 0xFF))
    text = window.create(
        "text",
        text="Shine",
        font_size=40,
        fill=WHITE,
        padding=padding,
        width="fit-content",
        height="fit-content",
    )
    if effect:
        text.set(shader=Shader(PASSTHROUGH, mode="effect"))
    window.root.add_child(text)
    window.advance(16)
    return ink(window.snapshot())


@pytest.mark.parametrize("padding", [0, 12, 30])
def test_a_passthrough_effect_over_padded_text_draws_it_whole(padding):
    # #176: with the node the size of its padding the effect cropped the text
    # (1003 bright pixels fell to 126). Now it is as big as its content, so the
    # effect draws exactly what a direct draw does.
    assert passthrough_ink(padding, effect=True) == passthrough_ink(padding, effect=False) > 500
