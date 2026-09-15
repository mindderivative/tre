//! Standard easing curves (the Robert Penner set every real animation
//! library implements), each a pure `f32 -> f32` function on `t in
//! [0.0, 1.0]` returning the eased progress -- callers are expected to
//! clamp `t` themselves if they need out-of-range behavior defined
//! (these functions extrapolate rather than clamp, which is occasionally
//! useful for a deliberate overshoot and never silently wrong).

/// No easing: eased progress equals `t` exactly.
#[must_use]
pub fn linear(t: f32) -> f32 {
    t
}

#[must_use]
pub fn ease_in_quad(t: f32) -> f32 {
    t * t
}

#[must_use]
pub fn ease_out_quad(t: f32) -> f32 {
    1.0 - (1.0 - t) * (1.0 - t)
}

#[must_use]
pub fn ease_in_out_quad(t: f32) -> f32 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
    }
}

#[must_use]
pub fn ease_in_cubic(t: f32) -> f32 {
    t * t * t
}

#[must_use]
pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

#[must_use]
pub fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

#[must_use]
pub fn ease_in_quart(t: f32) -> f32 {
    t.powi(4)
}

#[must_use]
pub fn ease_out_quart(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(4)
}

#[must_use]
pub fn ease_in_out_quart(t: f32) -> f32 {
    if t < 0.5 {
        8.0 * t.powi(4)
    } else {
        1.0 - (-2.0 * t + 2.0).powi(4) / 2.0
    }
}

#[must_use]
pub fn ease_in_quint(t: f32) -> f32 {
    t.powi(5)
}

#[must_use]
pub fn ease_out_quint(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(5)
}

#[must_use]
pub fn ease_in_out_quint(t: f32) -> f32 {
    if t < 0.5 {
        16.0 * t.powi(5)
    } else {
        1.0 - (-2.0 * t + 2.0).powi(5) / 2.0
    }
}

/// A named easing curve, dispatching to one of the free functions above
/// -- lets a caller (especially `tre-python`, which cannot pass a raw
/// Rust function pointer across the PyO3 boundary as a first-class
/// value) select a curve by a plain, `Copy` value instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Easing {
    #[default]
    Linear,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
    EaseInQuart,
    EaseOutQuart,
    EaseInOutQuart,
    EaseInQuint,
    EaseOutQuint,
    EaseInOutQuint,
}

impl Easing {
    /// Applies this curve to `t`.
    #[must_use]
    pub fn apply(self, t: f32) -> f32 {
        match self {
            Self::Linear => linear(t),
            Self::EaseInQuad => ease_in_quad(t),
            Self::EaseOutQuad => ease_out_quad(t),
            Self::EaseInOutQuad => ease_in_out_quad(t),
            Self::EaseInCubic => ease_in_cubic(t),
            Self::EaseOutCubic => ease_out_cubic(t),
            Self::EaseInOutCubic => ease_in_out_cubic(t),
            Self::EaseInQuart => ease_in_quart(t),
            Self::EaseOutQuart => ease_out_quart(t),
            Self::EaseInOutQuart => ease_in_out_quart(t),
            Self::EaseInQuint => ease_in_quint(t),
            Self::EaseOutQuint => ease_out_quint(t),
            Self::EaseInOutQuint => ease_in_out_quint(t),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_easing_curve_maps_zero_to_zero_and_one_to_one() {
        let curves = [
            Easing::Linear,
            Easing::EaseInQuad,
            Easing::EaseOutQuad,
            Easing::EaseInOutQuad,
            Easing::EaseInCubic,
            Easing::EaseOutCubic,
            Easing::EaseInOutCubic,
            Easing::EaseInQuart,
            Easing::EaseOutQuart,
            Easing::EaseInOutQuart,
            Easing::EaseInQuint,
            Easing::EaseOutQuint,
            Easing::EaseInOutQuint,
        ];
        for curve in curves {
            assert!(
                (curve.apply(0.0)).abs() <= f32::EPSILON,
                "{curve:?}(0.0) must be 0.0"
            );
            assert!(
                (curve.apply(1.0) - 1.0).abs() <= 1e-5,
                "{curve:?}(1.0) must be 1.0, got {}",
                curve.apply(1.0)
            );
        }
    }

    #[test]
    fn ease_in_quad_starts_slower_than_linear() {
        assert!(ease_in_quad(0.25) < linear(0.25));
    }

    #[test]
    fn ease_out_quad_starts_faster_than_linear() {
        assert!(ease_out_quad(0.25) > linear(0.25));
    }

    #[test]
    fn ease_in_out_quad_is_symmetric_about_the_midpoint() {
        let t = 0.3;
        let a = ease_in_out_quad(t);
        let b = 1.0 - ease_in_out_quad(1.0 - t);
        assert!((a - b).abs() <= 1e-5, "expected symmetry, got {a} vs {b}");
    }
}
