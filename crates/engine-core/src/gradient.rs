//! 0.5.4 (#109): gradient fills.
//!
//! A gradient is described relative to the node's box, so it follows the box
//! as layout moves and resizes it, and resolves to concrete points only when
//! painted (`resolve`). Angles are CSS's: 0 points up and they grow clockwise.

use peniko::Color;

use crate::Interpolate;

/// Where a gradient's colours run.
#[derive(Clone, Debug, PartialEq)]
pub enum GradientShape {
    /// Along a line through the box's centre at `angle_deg` (0 up, 90 right,
    /// 180 down), as long as the box is along that direction, as CSS's
    /// `linear-gradient` is.
    Linear { angle_deg: f64 },
    /// Outward from `center` (fractions of the box, `(0.5, 0.5)` its middle).
    /// `radius` is a fraction of the half-diagonal, so `1.0` reaches the corner
    /// farthest from the centre of a centred gradient, as CSS's default does.
    Radial { center: (f64, f64), radius: f64 },
    /// Around `center`, starting `start_deg` clockwise from up.
    Sweep { center: (f64, f64), start_deg: f64 },
}

/// A colour at a position along the gradient, `offset` in `0.0..=1.0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientStop {
    pub offset: f32,
    pub color: Color,
}

/// A gradient fill: a shape and at least two stops in increasing order.
#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    shape: GradientShape,
    stops: Vec<GradientStop>,
}

impl Gradient {
    /// A gradient of `shape` through `stops`. Errors, saying why, when there
    /// are fewer than two stops, an offset is outside `0..=1` or not a number,
    /// offsets decrease, or the shape has a non-finite number or a radius
    /// that isn't greater than 0.
    pub fn new(shape: GradientShape, stops: Vec<GradientStop>) -> Result<Self, String> {
        if stops.len() < 2 {
            return Err("a gradient needs at least two color stops".to_string());
        }
        let mut last = 0.0f32;
        for stop in &stops {
            if !(0.0..=1.0).contains(&stop.offset) {
                return Err(format!(
                    "a gradient stop's offset must be between 0 and 1, got {}",
                    stop.offset
                ));
            }
            if stop.offset < last {
                return Err("a gradient's stop offsets must not decrease".to_string());
            }
            last = stop.offset;
        }
        let finite = |values: &[f64]| values.iter().all(|v| v.is_finite());
        let ok = match &shape {
            GradientShape::Linear { angle_deg } => finite(&[*angle_deg]),
            GradientShape::Radial { center, radius } => {
                finite(&[center.0, center.1, *radius]) && *radius > 0.0
            }
            GradientShape::Sweep { center, start_deg } => finite(&[center.0, center.1, *start_deg]),
        };
        if !ok {
            return Err(
                "a gradient's angle, center and radius must be finite numbers, the radius \
                 greater than 0"
                    .to_string(),
            );
        }
        Ok(Self { shape, stops })
    }

    pub fn shape(&self) -> &GradientShape {
        &self.shape
    }

    pub fn stops(&self) -> &[GradientStop] {
        &self.stops
    }

    /// This gradient's shape and stop positions, every stop `color`: what a
    /// solid fill fades from or to.
    pub fn solid(&self, color: Color) -> Self {
        Self {
            shape: self.shape.clone(),
            stops: self
                .stops
                .iter()
                .map(|stop| GradientStop {
                    offset: stop.offset,
                    color,
                })
                .collect(),
        }
    }

    /// Whether `self` can animate smoothly to `other`: the same kind of shape
    /// and the same number of stops.
    pub fn animates_to(&self, other: &Self) -> bool {
        std::mem::discriminant(&self.shape) == std::mem::discriminant(&other.shape)
            && self.stops.len() == other.stops.len()
    }

    /// The gradient as `peniko` draws it for a box of `w` x `h`, in the box's
    /// own coordinates, and the transform to paint it under (the identity,
    /// except for a sweep, which is drawn from angle 0 and turned to its
    /// start: vello misplaces the seam of a sweep that starts elsewhere).
    pub fn resolve(&self, w: f64, h: f64) -> (peniko::Gradient, peniko::kurbo::Affine) {
        use peniko::kurbo::{Affine, Point};
        let mut paint_transform = Affine::IDENTITY;
        let gradient = match &self.shape {
            GradientShape::Linear { angle_deg } => {
                let theta = angle_deg.to_radians();
                let (dx, dy) = (theta.sin(), -theta.cos());
                // The line is as long as the box is in its direction, so the
                // first and last stops land on the corners.
                let length = (w * theta.sin()).abs() + (h * theta.cos()).abs();
                let (cx, cy) = (w / 2.0, h / 2.0);
                peniko::Gradient::new_linear(
                    Point::new(cx - dx * length / 2.0, cy - dy * length / 2.0),
                    Point::new(cx + dx * length / 2.0, cy + dy * length / 2.0),
                )
            }
            GradientShape::Radial { center, radius } => peniko::Gradient::new_radial(
                Point::new(center.0 * w, center.1 * h),
                (radius * w.hypot(h) / 2.0) as f32,
            ),
            GradientShape::Sweep { center, start_deg } => {
                // `peniko` measures from the +x axis; CSS from up.
                let at = Point::new(center.0 * w, center.1 * h);
                paint_transform = Affine::rotate_about((start_deg - 90.0).to_radians(), at);
                peniko::Gradient::new_sweep(at, 0.0, std::f32::consts::TAU)
            }
        };
        let stops: Vec<(f32, Color)> = self.stops.iter().map(|s| (s.offset, s.color)).collect();
        (gradient.with_stops(stops.as_slice()), paint_transform)
    }
}

impl Interpolate for Gradient {
    /// Shapes and stops lerp when the gradients are compatible
    /// (`animates_to`); otherwise the value holds until the end and then jumps.
    fn interpolate(&self, other: &Self, t: f64) -> Self {
        if !self.animates_to(other) {
            return if t < 1.0 { self.clone() } else { other.clone() };
        }
        let lerp = |a: f64, b: f64| a.interpolate(&b, t);
        let shape = match (&self.shape, &other.shape) {
            (GradientShape::Linear { angle_deg: a }, GradientShape::Linear { angle_deg: b }) => {
                GradientShape::Linear {
                    angle_deg: lerp(*a, *b),
                }
            }
            (
                GradientShape::Radial {
                    center: ca,
                    radius: ra,
                },
                GradientShape::Radial {
                    center: cb,
                    radius: rb,
                },
            ) => GradientShape::Radial {
                center: (lerp(ca.0, cb.0), lerp(ca.1, cb.1)),
                radius: lerp(*ra, *rb),
            },
            (
                GradientShape::Sweep {
                    center: ca,
                    start_deg: sa,
                },
                GradientShape::Sweep {
                    center: cb,
                    start_deg: sb,
                },
            ) => GradientShape::Sweep {
                center: (lerp(ca.0, cb.0), lerp(ca.1, cb.1)),
                start_deg: lerp(*sa, *sb),
            },
            _ => unreachable!("animates_to checked the shapes match"),
        };
        let stops = self
            .stops
            .iter()
            .zip(&other.stops)
            .map(|(a, b)| GradientStop {
                offset: f64::from(a.offset).interpolate(&f64::from(b.offset), t) as f32,
                color: a.color.interpolate(&b.color, t),
            })
            .collect();
        Self { shape, stops }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stop(offset: f32, r: u8) -> GradientStop {
        GradientStop {
            offset,
            color: Color::from_rgba8(r, 0, 0, 255),
        }
    }

    fn linear(angle: f64) -> Gradient {
        Gradient::new(
            GradientShape::Linear { angle_deg: angle },
            vec![stop(0.0, 0), stop(1.0, 200)],
        )
        .unwrap()
    }

    #[test]
    fn a_gradient_needs_two_ordered_stops_in_range() {
        let shape = || GradientShape::Linear { angle_deg: 0.0 };
        assert!(Gradient::new(shape(), vec![stop(0.0, 0)]).is_err());
        assert!(Gradient::new(shape(), vec![stop(0.0, 0), stop(1.5, 0)]).is_err());
        assert!(Gradient::new(shape(), vec![stop(0.0, 0), stop(f32::NAN, 0)]).is_err());
        assert!(Gradient::new(shape(), vec![stop(0.6, 0), stop(0.4, 0)]).is_err());
        assert!(Gradient::new(shape(), vec![stop(0.5, 0), stop(0.5, 9)]).is_ok());
    }

    #[test]
    fn a_shape_with_a_bad_number_is_refused() {
        let stops = || vec![stop(0.0, 0), stop(1.0, 0)];
        let radial = |radius| GradientShape::Radial {
            center: (0.5, 0.5),
            radius,
        };
        assert!(Gradient::new(radial(0.0), stops()).is_err());
        assert!(Gradient::new(radial(-1.0), stops()).is_err());
        assert!(Gradient::new(radial(f64::INFINITY), stops()).is_err());
        assert!(
            Gradient::new(
                GradientShape::Linear {
                    angle_deg: f64::NAN
                },
                stops()
            )
            .is_err()
        );
    }

    #[test]
    fn a_linear_gradient_spans_the_box_in_its_direction() {
        // 180 degrees runs top to bottom through the box's full height.
        let (g, _) = linear(180.0).resolve(100.0, 40.0);
        let peniko::GradientKind::Linear(line) = g.kind else {
            panic!("linear")
        };
        assert!((line.start.x - 50.0).abs() < 1e-6 && (line.start.y - 0.0).abs() < 1e-6);
        assert!((line.end.x - 50.0).abs() < 1e-6 && (line.end.y - 40.0).abs() < 1e-6);
        // 90 degrees runs left to right across the full width.
        let (g, _) = linear(90.0).resolve(100.0, 40.0);
        let peniko::GradientKind::Linear(line) = g.kind else {
            panic!("linear")
        };
        assert!((line.start.x - 0.0).abs() < 1e-6 && (line.end.x - 100.0).abs() < 1e-6);
    }

    #[test]
    fn compatible_gradients_lerp_and_others_jump_at_the_end() {
        let (a, b) = (linear(0.0), linear(90.0));
        assert!(a.animates_to(&b));
        let mid = a.interpolate(&b, 0.5);
        assert_eq!(mid.shape(), &GradientShape::Linear { angle_deg: 45.0 });
        let radial = Gradient::new(
            GradientShape::Radial {
                center: (0.5, 0.5),
                radius: 1.0,
            },
            vec![stop(0.0, 0), stop(1.0, 200)],
        )
        .unwrap();
        assert!(!a.animates_to(&radial));
        assert_eq!(a.interpolate(&radial, 0.99), a);
        assert_eq!(a.interpolate(&radial, 1.0), radial);
    }

    #[test]
    fn solid_keeps_the_shape_and_positions() {
        let s = linear(30.0).solid(Color::from_rgba8(9, 9, 9, 255));
        assert_eq!(s.shape(), linear(30.0).shape());
        assert!(
            s.stops()
                .iter()
                .all(|st| st.color == Color::from_rgba8(9, 9, 9, 255))
        );
        assert!(linear(30.0).animates_to(&s));
    }
}
