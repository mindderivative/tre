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


def test_a_gradient_applies_only_to_a_box_path_or_text():
    window = Window(width=100, height=100)
    field = window.create("text_input", width=50, height=20)
    with pytest.raises(ValueError, match="box"):
        field.set(fill=Gradient.linear([BLACK, RED]))
    assert isinstance(field.get("fill"), tuple)


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


# 0.5.4 (#129): gradients on more than a box's fill.

GREEN = (0, 255, 0, 255)


def ramp():
    return Gradient.linear([BLACK, RED], angle=90)


def test_a_canvas_painter_takes_a_gradient_over_the_shape():
    window = Window(width=100, height=100)
    window.root.set(padding=0)

    def draw(ctx):
        ctx.fill_rect(20, 20, 60, 60, ramp())
        ctx.fill_circle(50, 90, 8, GREEN)

    window.root.add_child(window.create("canvas", width=100, height=100, draw=draw))
    left, right = shot_pixel(window, 24, 50), shot_pixel(window, 76, 50)
    assert left[0] < 40 and right[0] > 215
    assert shot_pixel(window, 50, 90)[:3] == (0, 255, 0)


def test_a_painter_refuses_a_bad_paint():
    window = Window(width=50, height=50)

    def draw(ctx):
        ctx.fill_rect(0, 0, 10, 10, "red")

    with pytest.raises(ValueError, match="Gradient"):
        window.root.add_child(window.create("canvas", width=50, height=50, draw=draw))


def test_a_gradient_stroke_color_paints_the_border_and_reads_back():
    window = Window(width=100, height=100)
    window.root.set(padding=0)
    box = window.create("box", width=80, height=80, stroke_width=10)
    window.root.add_child(box)
    box.set(stroke_color=ramp())
    assert isinstance(box.get("stroke_color"), Gradient)
    # (12, 5) and (74, 5) lie along the top border, near its two ends.
    top_left, top_right = shot_pixel(window, 12, 5), shot_pixel(window, 74, 5)
    assert top_left[0] < top_right[0] and top_right[0] > 200
    box.set(stroke_color=GREEN)
    assert box.get("stroke_color") == GREEN


def test_a_gradient_stroke_does_not_animate():
    window = Window(width=100, height=100)
    box = window.create("box", width=80, height=80, stroke_width=4)
    window.root.add_child(box)
    with pytest.raises(ValueError, match="stroke"):
        box.animate("stroke_color", ramp(), 100)
    box.set(stroke_color=ramp())
    with pytest.raises(ValueError, match="stroke"):
        box.animate("stroke_color", GREEN, 100)


def test_a_text_and_a_path_take_a_gradient_fill():
    window = Window(width=120, height=60)
    window.root.set(padding=0)
    label = window.create("text", text="MMMMMM", font_size=36, font_weight=700)
    window.root.add_child(label)
    label.set(fill=ramp())
    assert isinstance(label.get("fill"), Gradient)
    rgba, width, _ = window.snapshot()
    red = lambda x0, x1: max(rgba[(y * width + x) * 4] for x in range(x0, x1) for y in range(60))
    assert red(0, 25) < red(60, 120)
    path = window.create("path", data="M0,0 H10 V10 H0 Z", width=20, height=20)
    window.root.add_child(path)
    path.set(fill=ramp())
    assert isinstance(path.get("fill"), Gradient)


def test_a_text_input_still_refuses_a_gradient_fill():
    window = Window(width=100, height=60)
    field = window.create("text_input", width=80, height=30)
    window.root.add_child(field)
    with pytest.raises(ValueError, match="box"):
        field.set(fill=ramp())
