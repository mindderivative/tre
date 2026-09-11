#!/usr/bin/env python3
"""Phase 13 Step 13.3 proof: `tre.Timeline` (`treAnimation`), the real
sequencer built on top of `treTween`'s pure math (Step 13.2) and
`treTime` (Step 13.1). Ties directly into this session's own Step 12.9
`scale_x`/`scale_y`/`rotation`/`opacity` exposure, using them as real
animation targets for the first time, and proves real integration with
the actual rendering pipeline -- not just isolated number-checking.
"""

import tre_python as tre

WIDTH, HEIGHT = 200, 200
BLACK = (0, 0, 0, 255)


def pixel_at(buf: bytes, x: int, y: int) -> tuple:
    offset = (y * WIDTH + x) * 4
    return tuple(buf[offset : offset + 4])


def check_single_property_animation() -> None:
    rect = tre.Rectangle(0.0, 50.0, 20.0, 20.0, tre.rgba8(0, 0, 0, 255))
    timeline = tre.Timeline()
    timeline.animate(rect, "x", to=100.0, duration=1.0, easing=tre.Easing.Linear)

    assert timeline.active_count == 1
    still_running = timeline.advance(0.25)
    assert still_running
    assert abs(rect.x - 25.0) < 1e-4, f"expected x=25.0 at t=0.25/1.0, got {rect.x}"
    print(f"after advance(0.25): rect.x == {rect.x} (hand-computed: 25.0) -- OK")

    still_running = timeline.advance(0.25)
    assert still_running
    assert abs(rect.x - 50.0) < 1e-4, f"expected x=50.0 at t=0.5/1.0, got {rect.x}"
    print(f"after advance(0.25) again: rect.x == {rect.x} (hand-computed: 50.0) -- OK")

    still_running = timeline.advance(1.0)  # overshoots past duration=1.0
    assert not still_running, "the animation must report finished once past its duration"
    assert rect.x == 100.0, f"a finished animation must hold at its exact 'to' value, got {rect.x}"
    print(f"after advance(1.0) (past duration): rect.x == {rect.x}, still_running=False -- OK")


def check_two_concurrent_properties_on_the_same_shape() -> None:
    # Reuses this session's own Step 12.9 scale_x/rotation exposure as
    # real animation targets -- previously these fields existed but had
    # never been driven by anything beyond a manual assignment.
    rect = tre.Rectangle(0.0, 0.0, 10.0, 10.0, tre.rgba8(255, 255, 255, 255))
    timeline = tre.Timeline()
    timeline.animate(rect, "scale_x", to=3.0, duration=2.0, easing=tre.Easing.Linear)
    timeline.animate(rect, "rotation", to=6.0, duration=2.0, easing=tre.Easing.Linear)
    assert timeline.active_count == 2

    timeline.advance(1.0)  # halfway through both 2.0s animations
    assert abs(rect.scale_x - 2.0) < 1e-4, f"expected scale_x=2.0 (1 + (3-1)*0.5), got {rect.scale_x}"
    assert abs(rect.rotation - 3.0) < 1e-4, f"expected rotation=3.0 (0 + 6*0.5), got {rect.rotation}"
    print(f"two concurrent animations on one shape: scale_x={rect.scale_x}, rotation={rect.rotation} -- OK")


def check_prune_finished_drops_completed_entries() -> None:
    rect = tre.Rectangle(0.0, 0.0, 10.0, 10.0, tre.rgba8(0, 0, 0, 255))
    timeline = tre.Timeline()
    timeline.animate(rect, "x", to=10.0, duration=0.5, easing=tre.Easing.Linear)
    timeline.animate(rect, "y", to=10.0, duration=5.0, easing=tre.Easing.Linear)
    timeline.advance(1.0)  # the x animation (0.5s) has finished; y (5.0s) has not
    assert timeline.active_count == 2
    timeline.prune_finished()
    assert timeline.active_count == 1, "only the finished x animation should be pruned"
    print("prune_finished() correctly dropped only the completed animation -- OK")


def check_real_render_integration() -> None:
    # Proves the animated shape genuinely renders through the real
    # pipeline, not just that its Python attribute changed in isolation.
    rect = tre.Rectangle(10.0, 10.0, 30.0, 30.0, tre.rgba8(0, 0, 0, 255))
    timeline = tre.Timeline()
    timeline.animate(rect, "x", to=100.0, duration=1.0, easing=tre.Easing.Linear)
    timeline.advance(0.5)  # rect.x is now 55.0

    renderer = tre.HeadlessRenderer(WIDTH, HEIGHT)
    registry = tre.ShapeRegistry()
    registry.insert_rectangle(rect)
    frame = renderer.render(registry)

    inside_new_position = pixel_at(frame, 60, 20)  # within [55, 85] x [10, 40]
    inside_old_position = pixel_at(frame, 20, 20)  # within the ORIGINAL [10, 40] footprint
    background = pixel_at(frame, 190, 190)
    assert inside_new_position == BLACK, "the animated rect must render at its NEW x position"
    assert inside_old_position == background, "the animated rect must NOT still render at its old x"
    print("animated rect.x correctly moved the real rendered pixels -- OK")


def main() -> None:
    check_single_property_animation()
    check_two_concurrent_properties_on_the_same_shape()
    check_prune_finished_drops_completed_entries()
    check_real_render_integration()
    print("tre_python Timeline (treAnimation) demo: PASSED")


if __name__ == "__main__":
    main()
