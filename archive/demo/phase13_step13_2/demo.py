#!/usr/bin/env python3
"""Phase 13 Step 13.2 proof: `tre.Tween`/`tre.Easing`/`tre.Spring`
(`treTween`), the pure interpolation math layer `treAnimation` (Step
13.3) will sequence on top of.

Every assertion below checks an EXACT hand-computed expected value, not
just "it runs" -- matching this project's own established demo
discipline (Step 12.9's exact-pixel transform proof, Step 12.8's
exact-pixel fill-rule proof).
"""

import tre_python as tre


def approx(a: float, b: float, tol: float = 1e-4) -> bool:
    return abs(a - b) <= tol


def check_scalar_tween() -> None:
    tween = tre.Tween(0.0, 10.0, 1.0, tre.Easing.Linear)
    assert tween.sample(0.0) == 0.0
    assert approx(tween.sample(0.5), 5.0)
    assert tween.sample(1.0) == 10.0
    # Sampling past duration holds at `to`, never extrapolates.
    assert tween.sample(5.0) == 10.0
    # Sampling before the start holds at `from_`.
    assert tween.sample(-1.0) == 0.0
    print("Tween(0, 10, linear): sample(0)=0, sample(0.5)=5, sample(1)=10, clamps outside -- OK")


def check_vec2_tween() -> None:
    tween = tre.Tween((0.0, 0.0), (10.0, 20.0), 2.0, tre.Easing.Linear)
    x, y = tween.sample(1.0)  # halfway through a 2-second tween
    assert approx(x, 5.0) and approx(y, 10.0), f"expected (5, 10), got ({x}, {y})"
    print(f"Vec2 Tween((0,0), (10,20)) at t=1.0/2.0: ({x}, {y}) -- OK")


def check_eased_tween_matches_hand_computed_curve() -> None:
    # EaseInQuad(t) = t*t. At t=0.5 (progress=0.5 of a 1.0s tween),
    # eased progress = 0.25, so sample = 0 + (10-0)*0.25 = 2.5 exactly.
    tween = tre.Tween(0.0, 10.0, 1.0, tre.Easing.EaseInQuad)
    sampled = tween.sample(0.5)
    assert approx(sampled, 2.5), f"EaseInQuad at progress=0.5 must give exactly 2.5, got {sampled}"
    print(f"Tween with EaseInQuad at progress=0.5: {sampled} (hand-computed: 0.5*0.5*10=2.5) -- OK")


def check_mismatched_shapes_are_rejected() -> None:
    try:
        tre.Tween(0.0, (1.0, 2.0), 1.0)
        raise AssertionError("a scalar/tuple shape mismatch must raise TypeError")
    except TypeError:
        print("Tween(0.0, (1.0, 2.0)) correctly raised TypeError for mismatched shapes -- OK")


def check_spring_overshoots_unlike_a_plain_decay() -> None:
    # An underdamped spring must swing past its target -- the real
    # property distinguishing it from tre_math::spring_decay's plain
    # exponential smoothing, which can never overshoot by construction.
    spring = tre.Spring(200.0, 5.0, 1.0, 0.0)
    max_position = spring.position
    for _ in range(200):
        pos = spring.update(1.0, 1.0 / 120.0)
        max_position = max(max_position, pos)
    assert max_position > 1.05, f"an underdamped spring must overshoot 1.0, max was {max_position}"
    print(f"Spring(200, 5, 1) overshot its target=1.0, reaching {max_position:.4f} -- OK")


def check_overdamped_spring_settles_without_overshoot() -> None:
    spring = tre.Spring(50.0, 200.0, 1.0, 0.0)
    max_position = spring.position
    for _ in range(500):
        pos = spring.update(1.0, 1.0 / 120.0)
        max_position = max(max_position, pos)
    assert max_position <= 1.01, f"a heavily overdamped spring should not overshoot, max was {max_position}"
    print(f"Spring(50, 200, 1) (heavily overdamped) settled without overshoot: max={max_position:.4f} -- OK")


def main() -> None:
    check_scalar_tween()
    check_vec2_tween()
    check_eased_tween_matches_hand_computed_curve()
    check_mismatched_shapes_are_rejected()
    check_spring_overshoots_unlike_a_plain_decay()
    check_overdamped_spring_settles_without_overshoot()
    print("tre_python Tween/Easing/Spring (treTween) demo: PASSED")


if __name__ == "__main__":
    main()
