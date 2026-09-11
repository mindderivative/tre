#!/usr/bin/env python3
"""Phase 13 Step 13.1 proof: `tre.Clock` (`treTime`), a real Python-exposed
timer wrapping `tre_engine::FrameClock`, plus its new `elapsed()` method
(added this step -- `FrameClock` previously only had `tick()`).

Two real properties are proven, both against real `time.sleep` durations,
not just "it returns a float":

1. `tick()` returns 0.0 on the very first call, then a real per-call
   delta afterward, matching `tre_engine::FrameClock::tick`'s own
   documented and tested contract.
2. `elapsed()` accumulates real total time since the `Clock` was
   constructed, independent of how many times (or how recently) `tick()`
   has been called -- the real "t" a future `treTween`/`treAnimation`
   caller samples a tween/timeline against.
"""

import time

import tre_python as tre


def main() -> None:
    clock = tre.Clock()

    first_tick = clock.tick()
    assert first_tick == 0.0, f"first tick() must be exactly 0.0, got {first_tick}"
    print(f"first tick() == {first_tick} -- OK")

    time.sleep(0.05)
    second_tick = clock.tick()
    assert 0.03 <= second_tick < 1.0, (
        f"a real ~50ms sleep must report a delta in [0.03, 1.0)s, got {second_tick}s"
    )
    print(f"tick() after a real 50ms sleep == {second_tick:.4f}s -- OK")

    # elapsed() must reflect total time since construction, NOT time
    # since the last tick() -- proven by ticking again immediately (near
    # 0 delta) while elapsed() keeps growing from the clock's own start.
    third_tick = clock.tick()
    assert third_tick < 0.02, f"an immediate re-tick must report a near-zero delta, got {third_tick}s"

    elapsed = clock.elapsed()
    total_sleep = 0.05
    assert elapsed >= total_sleep - 0.02, (
        f"elapsed() must be at least the real ~50ms slept since construction, got {elapsed}s"
    )
    assert elapsed < 1.0, f"elapsed() must not be wildly inflated, got {elapsed}s"
    print(f"elapsed() == {elapsed:.4f}s, correctly independent of the near-zero re-tick -- OK")

    time.sleep(0.03)
    elapsed_2 = clock.elapsed()
    assert elapsed_2 > elapsed, "elapsed() must keep growing with real wall-clock time"
    print(f"elapsed() grew to {elapsed_2:.4f}s after another real 30ms sleep -- OK")

    print("tre_python Clock (treTime) demo: PASSED")


if __name__ == "__main__":
    main()
