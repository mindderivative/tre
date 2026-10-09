"""0.5.6 (#161): the `animation_end` node event."""

from tre import Window


def scene():
    window = Window(width=200, height=120)
    box = window.create("box", width=60, height=40, fill=(255, 0, 0, 255))
    window.root.add_child(box)
    return window, box


def test_it_fires_when_an_animation_runs_to_its_end():
    window, box = scene()
    seen = []
    box.on("animation_end", lambda e: seen.append((e.property, e.finished, e.target == box)))
    box.animate("opacity", 0.2, 100)
    window.advance(50)
    assert seen == []
    window.advance(60)
    assert seen == [("opacity", True, True)]
    window.advance(500)
    assert len(seen) == 1


def test_it_fires_for_each_property_and_does_not_bubble():
    window, box = scene()
    outer = []
    window.root.on("animation_end", lambda e: outer.append(e))
    inner = []
    box.on("animation_end", lambda e: inner.append(e.property))
    box.animate("opacity", 0.5, 50)
    box.animate("scale", 0.5, 80)
    box.animate("corner_radius", 8, 20)
    window.advance(200)
    assert sorted(inner) == ["corner_radius", "opacity", "scale"]
    assert outer == []


def test_a_replaced_animation_ends_as_not_finished_and_its_callback_is_released():
    window, box = scene()
    seen, called = [], []
    box.on("animation_end", lambda e: seen.append(e.finished))
    box.animate("opacity", 0.2, 1000, on_complete=lambda: called.append("first"))
    window.advance(100)
    box.animate("opacity", 0.9, 100)          # replaces it, from where it is
    window.advance(10)
    assert seen == [False]
    window.advance(200)
    assert seen == [False, True]
    assert called == []                         # the replaced one's callback never runs


def test_setting_the_property_cancels_its_animation():
    window, box = scene()
    seen = []
    box.on("animation_end", lambda e: seen.append((e.property, e.finished)))
    box.animate("opacity", 0.2, 1000)
    window.advance(100)
    box.set(opacity=0.7)
    window.advance(10)
    assert seen == [("opacity", False)]
    assert box.get("opacity") == 0.7


def test_on_complete_still_runs_and_comes_before_the_event():
    window, box = scene()
    order = []
    box.on("animation_end", lambda e: order.append("event"))
    box.animate("opacity", 0.2, 50, on_complete=lambda: order.append("callback"))
    window.advance(100)
    assert order == ["callback", "event"]


def test_the_event_names_include_animation_end():
    import pytest

    _, box = scene()
    with pytest.raises(ValueError, match="animation_end"):
        box.on("nonsense", lambda e: None)
