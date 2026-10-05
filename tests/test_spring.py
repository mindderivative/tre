"""0.5.4 (#138): `easing="spring"` / `("spring", bounce)`.

A spring takes `duration_ms` as its period (about one cycle of its main motion)
and `bounce` for how far it overshoots; it lasts until it settles, so it runs
longer than `duration_ms`. Driven with `window.advance`.
"""

import pytest

from tre import Window


def setup():
    window = Window(width=300, height=100)
    box = window.create("box", width=40, height=40, fill=(200, 0, 0, 255))
    window.root.add_child(box)
    return window, box


def trace(window, node, prop, step_ms=10, total_ms=3000):
    values = []
    for _ in range(total_ms // step_ms):
        window.advance(step_ms)
        values.append(node.get(prop))
    return values


def test_a_bouncy_spring_overshoots_the_target_and_settles_on_it():
    window, box = setup()
    box.animate("translate_x", 100, 400, easing=("spring", 0.5))
    values = trace(window, box, "translate_x")
    assert max(values) > 105, f"rings past 100: {max(values)}"
    assert values[-1] == 100.0
    # It crosses the target on the way, and ends within a pixel for good.
    last_far = max(i for i, v in enumerate(values) if abs(v - 100) > 1)
    assert last_far < len(values) - 5


def test_a_spring_with_no_bounce_arrives_without_overshoot():
    window, box = setup()
    box.animate("translate_x", 100, 400, easing=("spring", 0.0))
    values = trace(window, box, "translate_x")
    assert max(values) <= 100.2
    assert values == sorted(values), "it only moves forward"
    assert values[-1] == 100.0


def test_the_plain_string_is_a_spring_with_a_little_bounce():
    window, box = setup()
    box.animate("translate_x", 100, 400, easing="spring")
    values = trace(window, box, "translate_x")
    assert 100.5 < max(values) < 130
    assert values[-1] == 100.0


def test_a_spring_lasts_longer_than_its_duration_and_then_completes_once():
    window, box = setup()
    done = []
    box.animate("translate_x", 50, 300, easing=("spring", 0.4), on_complete=lambda: done.append(1))
    window.advance(300)
    assert done == [], "the duration is its period, not its length"
    window.advance(3000)
    assert done == [1]
    assert box.get("translate_x") == 50.0


def test_a_zero_duration_spring_snaps_like_any_animation():
    window, box = setup()
    box.animate("translate_x", 70, 0, easing=("spring", 0.3))
    window.advance(1)
    assert box.get("translate_x") == 70.0


def test_a_retarget_keeps_the_motion_going():
    window, box = setup()
    box.animate("translate_x", 200, 600, easing="linear")
    window.advance(300)
    mid = box.get("translate_x")
    assert 90 < mid < 110
    # Linear: 200 over 600 ms is a third of a unit a millisecond. Retarget with
    # a spring: right after, it is still moving about that fast, not stalled.
    box.animate("translate_x", 400, 400, easing=("spring", 0.2))
    window.advance(1)
    step = box.get("translate_x") - mid
    assert 0.2 < step < 1.2, f"carried the speed: {step} a millisecond"
    # A spring started from rest moves much less in its first millisecond.
    other_window, other = setup()
    other.animate("translate_x", 400, 400, easing=("spring", 0.2))
    other_window.advance(1)
    assert other.get("translate_x") < step


def test_springs_animate_colours_and_other_properties_too():
    window, box = setup()
    box.animate("fill", (0, 0, 200, 255), 300, easing=("spring", 0.3))
    box.animate("opacity", 0.4, 300, easing=("spring", 0.3))
    window.advance(100)
    assert box.get("fill") != (200, 0, 0, 255)
    window.advance(4000)
    assert box.get("fill") == (0, 0, 200, 255)
    assert box.get("opacity") == pytest.approx(0.4)


@pytest.mark.parametrize("easing", [("spring", 1.0), ("spring", -1.0), ("spring", "x"),
                                    ("springy", 0.2), "bouncy", ("spring",), ("spring", 0.1, 3)])
def test_a_bad_spring_is_a_value_error(easing):
    _, box = setup()
    with pytest.raises(ValueError, match="easing must be"):
        box.animate("translate_x", 10, 100, easing=easing)


def test_a_more_bouncy_spring_overshoots_more():
    peaks = []
    for bounce in (0.1, 0.4, 0.7):
        window, box = setup()
        box.animate("translate_x", 100, 400, easing=("spring", bounce))
        peaks.append(max(trace(window, box, "translate_x")))
    assert peaks == sorted(peaks) and peaks[0] < peaks[-1]
