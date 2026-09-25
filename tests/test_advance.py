"""M96 Phase 1: `window.advance(ms)` -- deterministic headless time, so
animated widgets are testable where `App.run()` renders no frames.
"""

from __future__ import annotations

import pytest

import tre


def pinned_window() -> tre.Window:
    w = tre.Window(200, 100, "advance")
    w.advance(0)  # pin the clock, so an animation starts at a known instant
    return w


def test_advance_moves_an_animation_by_exactly_that_much() -> None:
    w = pinned_window()
    box = w.create("box", opacity=0.0)
    box.animate("opacity", 1.0, 200)
    w.advance(50)
    assert box.get("opacity") == pytest.approx(0.25)
    w.advance(100)
    assert box.get("opacity") == pytest.approx(0.75)
    w.advance(1000)
    assert box.get("opacity") == 1.0


def test_advance_runs_completions() -> None:
    w = pinned_window()
    box = w.create("box")
    done: list[str] = []
    box.animate("opacity", 0.5, 100, on_complete=lambda: done.append("done"))
    w.advance(99)
    assert done == []
    w.advance(1)
    assert done == ["done"]


def test_each_window_keeps_its_own_time() -> None:
    first = pinned_window()
    second = pinned_window()
    a = first.create("box", opacity=0.0)
    b = second.create("box", opacity=0.0)
    a.animate("opacity", 1.0, 100)
    b.animate("opacity", 1.0, 100)
    first.advance(100)
    second.advance(50)
    assert a.get("opacity") == 1.0
    assert b.get("opacity") == pytest.approx(0.5)


@pytest.mark.parametrize("ms", [-1.0, float("nan"), float("inf")])
def test_advance_rejects_a_bad_duration(ms: float) -> None:
    with pytest.raises(ValueError, match="non-negative number"):
        tre.Window(10, 10, "advance").advance(ms)
