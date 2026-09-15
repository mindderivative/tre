//! A real damped mass-spring-damper integrator -- distinct from
//! `tre_math::spring_decay`, which is a plain one-pole exponential
//! smoother (its own doc comment: "not a mass-spring-damper ODE...
//! never overshoots"). This is a genuine second-order ODE, integrated
//! with semi-implicit ("symplectic") Euler each step, real enough to
//! overshoot and oscillate around its target when underdamped -- the
//! real "bouncy" motion a UI spring animation is expected to have,
//! which `spring_decay` cannot produce by design.

/// A one-dimensional damped spring. Construct with real physical
/// parameters, then call [`Spring::update`] once per frame with the
/// current target and real delta time.
#[derive(Debug, Clone, Copy)]
pub struct Spring {
    /// Spring constant `k` -- higher values pull toward the target
    /// faster and, at a given damping, oscillate at a higher frequency.
    pub stiffness: f32,
    /// Damping coefficient `c` -- higher values settle faster and
    /// reduce overshoot; above a critical value (dependent on
    /// `stiffness`/`mass`) the spring stops overshooting entirely.
    pub damping: f32,
    /// Mass `m` -- higher values respond more sluggishly to the same
    /// `stiffness`/`damping`.
    pub mass: f32,
    position: f32,
    velocity: f32,
}

impl Spring {
    /// A spring starting at rest (zero velocity) at `initial_position`.
    ///
    /// # Panics
    /// In debug builds, panics if `mass <= 0.0` -- a non-positive mass
    /// makes the underlying ODE's acceleration term divide by zero or
    /// invert sign, neither of which is a meaningful spring.
    #[must_use]
    pub fn new(stiffness: f32, damping: f32, mass: f32, initial_position: f32) -> Self {
        debug_assert!(mass > 0.0, "Spring::mass must be > 0.0, got {mass}");
        Self {
            stiffness,
            damping,
            mass,
            position: initial_position,
            velocity: 0.0,
        }
    }

    #[must_use]
    pub fn position(&self) -> f32 {
        self.position
    }

    #[must_use]
    pub fn velocity(&self) -> f32 {
        self.velocity
    }

    /// Advances this spring by `dt` real seconds toward `target`,
    /// returning the new position. Semi-implicit Euler: velocity is
    /// updated from the current acceleration first, then position is
    /// updated from the *new* velocity -- unconditionally stable for a
    /// damped oscillator at reasonable frame-rate step sizes, unlike
    /// plain (explicit) Euler.
    pub fn update(&mut self, target: f32, dt: f32) -> f32 {
        let displacement = self.position - target;
        let spring_force = -self.stiffness * displacement;
        let damping_force = -self.damping * self.velocity;
        let acceleration = (spring_force + damping_force) / self.mass;
        self.velocity += acceleration * dt;
        self.position += self.velocity * dt;
        self.position
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settle(spring: &mut Spring, target: f32, steps: usize, dt: f32) -> f32 {
        let mut max_position = spring.position();
        for _ in 0..steps {
            let p = spring.update(target, dt);
            max_position = max_position.max(p);
        }
        max_position
    }

    #[test]
    fn an_underdamped_spring_overshoots_its_target() {
        // Low damping relative to stiffness/mass: a real spring should
        // swing past the target before settling -- the one property
        // `tre_math::spring_decay` can never exhibit by construction.
        let mut spring = Spring::new(200.0, 5.0, 1.0, 0.0);
        let max_position = settle(&mut spring, 1.0, 200, 1.0 / 120.0);
        assert!(
            max_position > 1.05,
            "an underdamped spring must overshoot target=1.0 by a visible margin, max was {max_position}"
        );
    }

    #[test]
    fn a_critically_overdamped_spring_never_overshoots() {
        // Very high damping relative to stiffness: should approach
        // monotonically, matching spring_decay-like behavior at the
        // other end of the parameter space.
        let mut spring = Spring::new(50.0, 200.0, 1.0, 0.0);
        let max_position = settle(&mut spring, 1.0, 500, 1.0 / 120.0);
        assert!(
            max_position <= 1.01,
            "a heavily overdamped spring should not meaningfully overshoot, max was {max_position}"
        );
    }

    #[test]
    fn a_spring_eventually_settles_near_its_target() {
        let mut spring = Spring::new(150.0, 15.0, 1.0, 0.0);
        let final_position = (0..1000).fold(0.0, |_, _| spring.update(1.0, 1.0 / 120.0));
        assert!(
            (final_position - 1.0).abs() < 0.01,
            "after many steps a damped spring must settle near its target, got {final_position}"
        );
        assert!(
            spring.velocity().abs() < 0.01,
            "a settled spring must also have near-zero velocity, got {}",
            spring.velocity()
        );
    }
}
