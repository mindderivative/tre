"""0.5.4 (#109): gradient fills -- `tre.Gradient` as a box's `fill`."""

import pytest

import tre
from tre import Gradient, Window

BLACK = (0, 0, 0, 255)
RED = (255, 0, 0, 255)
BLUE = (0, 0, 255, 255)


def shot_pixel(window, x, y):
    rgba, width, _ = window.snapshot()
    i = (y * width + x) * 4
    return tuple(rgba[i : i + 4])


def boxed(fill):
    window = Window(width=100, height=100)
    box = window.create("box", width=60, height=60, fill=fill)
    window.root.add_child(box)
    return window, box


def test_a_gradient_is_described_by_its_parts():
    g = Gradient.linear([BLACK, RED], angle=90)
    assert (g.kind, g.angle, g.center, g.radius, g.start) == ("linear", 90.0, None, None, None)
    assert g.stops == [(0.0, BLACK), (1.0, RED)]
    r = Gradient.radial([RED, BLUE], center=(0.25, 0.75), radius=0.5)
    assert (r.kind, r.center, r.radius, r.angle) == ("radial", (0.25, 0.75), 0.5, None)
    s = Gradient.sweep([RED, BLUE, RED], start=90)
    assert (s.kind, s.start, s.center) == ("sweep", 90.0, (0.5, 0.5))
    assert [o for o, _ in s.stops] == [0.0, 0.5, 1.0]
    assert "linear" in repr(g)


def test_stops_can_be_given_with_offsets():
    g = Gradient.linear([(0.0, BLACK), (0.25, RED), (1.0, BLUE)])
    assert [o for o, _ in g.stops] == [0.0, 0.25, 1.0]


def test_equal_gradients_are_equal():
    assert Gradient.linear([BLACK, RED]) == Gradient.linear([BLACK, RED])
    assert Gradient.linear([BLACK, RED]) != Gradient.linear([BLACK, RED], angle=90)


@pytest.mark.parametrize(
    "build, message",
    [
        (lambda: Gradient.linear([BLACK]), "at least two"),
        (lambda: Gradient.linear([]), "at least two"),
        (lambda: Gradient.linear([(0.0, BLACK), (1.5, RED)]), "between 0 and 1"),
        (lambda: Gradient.linear([(0.8, BLACK), (0.2, RED)]), "must not decrease"),
        (lambda: Gradient.linear([BLACK, (1, 2, 3)]), "stops"),
        (lambda: Gradient.linear(5), "sequence"),
        (lambda: Gradient.radial([BLACK, RED], radius=0), "greater than 0"),
        (lambda: Gradient.linear([BLACK, RED], angle=float("nan")), "finite"),
    ],
)
def test_a_bad_gradient_is_a_value_error(build, message):
    with pytest.raises(ValueError, match=message):
        build()


def test_a_gradient_fill_draws_a_ramp():
    window, _ = boxed(Gradient.linear([BLACK, RED], angle=90))
    left, mid, right = (shot_pixel(window, x, 40) for x in (17, 46, 75))
    assert left[0] < 20 and right[0] > 235
    assert abs(mid[0] - 128) < 14


def test_get_fill_returns_the_gradient_and_a_colour_replaces_it():
    g = Gradient.linear([BLACK, RED])
    window, box = boxed(g)
    assert box.get("fill") == g
    box.set(fill=BLUE)
    assert box.get("fill") == BLUE
    assert shot_pixel(window, 46, 40) == BLUE


def test_set_to_a_gradient_after_a_colour():
    window, box = boxed(BLUE)
    box.set(fill=Gradient.linear([BLACK, RED], angle=90))
    assert shot_pixel(window, 75, 40)[0] > 235


def test_a_gradient_fill_works_on_the_root():
    window = Window(width=100, height=100)
    window.root.set(fill=Gradient.linear([BLACK, RED], angle=90))
    assert shot_pixel(window, 97, 50)[0] > 235


def test_a_bad_fill_names_what_is_accepted():
    _, box = boxed(BLUE)
    with pytest.raises(ValueError, match="Gradient"):
        box.set(fill="red")


def test_a_gradient_applies_only_to_a_box():
    window = Window(width=100, height=100)
    text = window.create("text", text="hi", width=50, height=20)
    with pytest.raises(ValueError, match="box"):
        text.set(fill=Gradient.linear([BLACK, RED]))
    assert isinstance(text.get("fill"), tuple)


def test_animating_between_gradients_interpolates():
    window, box = boxed(Gradient.linear([BLACK, BLACK], angle=90))
    box.animate("fill", Gradient.linear([RED, RED], angle=90), 1000)
    window.advance(500)
    mid = box.get("fill")
    assert all(abs(o[0] - 128) <= 2 for _, o in mid.stops)
    assert box.get_target("fill") == Gradient.linear([RED, RED], angle=90)
    window.advance(600)
    assert box.get("fill") == Gradient.linear([RED, RED], angle=90)


def test_animating_from_a_colour_fades_a_gradient_in():
    window, box = boxed(RED)
    box.animate("fill", Gradient.linear([BLACK, BLUE], angle=90), 1000)
    window.advance(0)
    start = box.get("fill")
    assert all(c == RED for _, c in start.stops), start.stops
    window.advance(1100)
    assert box.get("fill") == Gradient.linear([BLACK, BLUE], angle=90)


def test_an_animation_between_unlike_gradients_is_refused():
    _, box = boxed(Gradient.linear([BLACK, RED]))
    with pytest.raises(ValueError, match="same kind"):
        box.animate("fill", Gradient.radial([BLACK, RED]), 100)
    with pytest.raises(ValueError, match="same kind"):
        box.animate("fill", Gradient.linear([BLACK, RED, BLUE]), 100)
    with pytest.raises(ValueError, match="to a color"):
        box.animate("fill", BLUE, 100)


def test_the_gradient_is_exported_and_the_pixels_can_be_saved(tmp_path):
    assert tre.Gradient is Gradient
    window, _ = boxed(Gradient.sweep([RED, BLUE, RED]))
    tre.write_png(tmp_path / "sweep.png", *window.snapshot())
    assert (tmp_path / "sweep.png").stat().st_size > 100
