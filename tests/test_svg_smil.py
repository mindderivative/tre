"""0.5.10 (#245): SMIL animation in an `svg` node, drawn at `svg_time`."""

import pytest
from tess_re import Window

RED = (255, 0, 0, 255)


def doc(body):
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10" viewBox="0 0 20 10">'
        f"{body}</svg>"
    )


# A red square that is invisible until its animation fades it in over a second.
FADE_IN = doc(
    '<rect width="10" height="10" fill="#ff0000" opacity="0">'
    '<animate attributeName="opacity" to="1" dur="1s" fill="freeze"/></rect>'
)
# A bar that turns about the centre for ever, two seconds a turn.
SPINNER = doc(
    '<g><rect x="9" y="0" width="2" height="10" fill="#ff0000"/>'
    '<animateTransform attributeName="transform" type="rotate" to="360 10 5" '
    'dur="2s" repeatCount="indefinite"/></g>'
)
# A square that slides right by its own width over two seconds.
MORPH = doc(
    '<path fill="#ff0000" d="M0 0H10V10H0Z">'
    '<animate attributeName="d" values="M0 0H10V10H0Z;M10 0H20V10H10Z" dur="2s" fill="freeze"/></path>'
)


def shown(source):
    window = Window(width=100, height=100)
    node = window.create(
        "svg", svg=source, width=100, height=100, position="absolute", x=0, y=0
    )
    window.root.add_child(node)
    return window, node


def pixel(window, x, y):
    rgba, width, _ = window.snapshot()
    i = (y * width + x) * 4
    return tuple(rgba[i : i + 4])


def test_the_document_is_drawn_as_it_looks_at_svg_time():
    window, node = shown(FADE_IN)
    assert pixel(window, 25, 50)[3] == 0
    node.set(svg_time=1.0)
    assert pixel(window, 25, 50) == RED
    node.set(svg_time=0.5)
    assert 100 < pixel(window, 25, 50)[3] < 156
    assert node.get("svg_time") == 0.5


def test_svg_time_follows_tres_own_animation_clock():
    window, node = shown(FADE_IN)
    node.animate("svg_time", to=1.0, duration_ms=1000, easing="linear")
    window.advance(500)
    assert 100 < pixel(window, 25, 50)[3] < 156
    window.advance(600)
    assert pixel(window, 25, 50) == RED
    assert node.get("svg_time") == 1.0


def test_duration_is_none_for_a_loop_and_zero_without_animation():
    _, fade = shown(FADE_IN)
    _, spinner = shown(SPINNER)
    still_window, still = shown(doc('<rect width="10" height="10" fill="#ff0000"/>'))
    assert fade.get("svg_duration") == 1.0
    assert spinner.get("svg_duration") is None
    assert still.get("svg_duration") == 0.0
    still.set(svg_time=5.0)
    assert pixel(still_window, 25, 50) == RED


def test_a_loop_repeats_every_period_and_differs_between():
    window, node = shown(SPINNER)

    def at(t):
        node.set(svg_time=t)
        return window.snapshot()[0]

    assert at(0.0) == at(2.0)
    assert at(0.5) == at(2.5)
    assert at(0.0) != at(1.0 / 2)


def test_a_path_morphs_between_equal_command_sequences():
    window, node = shown(MORPH)
    assert pixel(window, 12, 50) == RED
    node.set(svg_time=1.0)
    assert pixel(window, 12, 50)[3] == 0
    assert pixel(window, 50, 50) == RED
    node.set(svg_time=2.0)
    assert pixel(window, 62, 50) == RED
    assert pixel(window, 12, 50)[3] == 0


def test_unsupported_smil_draws_the_first_frame_as_before():
    plain = doc('<rect width="10" height="10" fill="#ff0000"/>')
    moving = doc(
        '<rect width="10" height="10" fill="#ff0000">'
        '<animateMotion dur="1s" path="M0 0L10 0"/></rect>'
    )
    window, node = shown(moving)
    reference, _ = shown(plain)
    node.set(svg_time=0.7)
    assert window.snapshot()[0] == reference.snapshot()[0]
    assert node.get("svg_duration") == 0.0


def test_a_new_document_restarts_at_time_zero():
    window, node = shown(FADE_IN)
    node.set(svg_time=1.0)
    node.set(svg=FADE_IN)
    assert node.get("svg_time") == 0.0
    assert pixel(window, 25, 50)[3] == 0


def test_a_new_colour_keeps_the_animation_where_it_is_and_a_new_document_does_not():
    window, node = shown(FADE_IN)
    node.animate("svg_time", to=1.0, duration_ms=1000, easing="linear")
    window.advance(500)
    node.set(svg_color=(0, 0, 255, 255))
    window.advance(600)
    assert node.get("svg_time") == 1.0
    assert pixel(window, 25, 50) == RED
    node.set(svg=FADE_IN)
    assert node.get("svg_time") == 0.0
    node.set(svg_time=0.7, svg=FADE_IN)
    assert node.get("svg_time") == 0.7, "a time given with a new document is applied after it"


def test_svg_time_is_checked():
    window, node = shown(FADE_IN)
    box = window.create("box")
    with pytest.raises(ValueError, match="applies only to an svg"):
        box.set(svg_time=1.0)
    with pytest.raises(ValueError, match="read-only"):
        node.set(svg_duration=1.0)
    with pytest.raises(ValueError, match="svg_time"):
        node.animate("svg_time", to=-1.0, duration_ms=10)
    with pytest.raises(ValueError, match="svg_time"):
        node.set(svg_time=-1.0)
    with pytest.raises(ValueError, match="svg_time"):
        node.set(svg_time=True)
