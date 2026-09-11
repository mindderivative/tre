//! `tre-animation` (Phase 13 Step 13.3): real sequencing/state on top of
//! `tre-tween`'s pure interpolation math, driven by a `tre_python::
//! Clock`-style delta each frame.
//!
//! Deliberately depends on nothing PyO3-related -- `tre-python` depends
//! on this crate (not the reverse), so [`Timeline`] identifies its
//! animation targets by an opaque `u64` (mirroring `tre_engine::
//! shapes::AnimationId`'s own real, already-established pattern of "the
//! registry/caller knows what the id means, the sequencer doesn't have
//! to"). `tre-python`'s own binding maps each `u64` back to a real
//! Python object + attribute name and applies the sampled value via
//! `setattr`, keeping this crate reusable by any future caller, not
//! just `tre-python`'s specific shape classes.

use tre_tween::{Easing, Tween};

/// One scheduled animation: `target` is opaque to this crate (the
/// caller's own id for whatever it wants animated), `start_time` is the
/// [`Timeline`]'s own elapsed time when this entry was scheduled (via
/// [`Timeline::animate`]), and `tween` is the pure interpolation to
/// sample as time advances past `start_time`.
#[derive(Debug, Clone, Copy)]
struct Entry {
    target: u64,
    start_time: f32,
    tween: Tween<f32>,
}

/// A real sequencer: holds any number of concurrently-running
/// [`Tween`]s, each with its own start time relative to one shared
/// clock, advanced by real delta time each frame via [`Timeline::
/// advance`].
#[derive(Debug, Clone, Default)]
pub struct Timeline {
    entries: Vec<Entry>,
    elapsed: f32,
}

impl Timeline {
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            elapsed: 0.0,
        }
    }

    /// Schedules `tween` to start now (at this timeline's current
    /// elapsed time) against `target`. Multiple tweens can target the
    /// same `target` id (e.g. animating two different properties of the
    /// same shape) -- each is sampled and reported independently.
    pub fn animate(&mut self, target: u64, tween: Tween<f32>) {
        self.entries.push(Entry {
            target,
            start_time: self.elapsed,
            tween,
        });
    }

    /// Real seconds since this timeline was created (or last had its
    /// internal clock reset via a fresh [`Timeline::new`]).
    #[must_use]
    pub fn elapsed(&self) -> f32 {
        self.elapsed
    }

    /// Advances this timeline's own clock by `dt` real seconds and
    /// samples every scheduled entry at its own local elapsed time
    /// (this timeline's total elapsed minus the entry's own
    /// `start_time`). Returns `(target, sampled_value)` for every
    /// entry -- including ones that have already finished, which keep
    /// reporting their final `to` value (matching [`Tween::sample`]'s
    /// own clamp-past-duration contract) -- and whether at least one
    /// entry has not yet finished.
    pub fn advance(&mut self, dt: f32) -> (Vec<(u64, f32)>, bool) {
        self.elapsed += dt;
        let mut samples = Vec::with_capacity(self.entries.len());
        let mut still_running = false;
        for entry in &self.entries {
            let local_elapsed = self.elapsed - entry.start_time;
            samples.push((entry.target, entry.tween.sample(local_elapsed)));
            if local_elapsed < entry.tween.duration {
                still_running = true;
            }
        }
        (samples, still_running)
    }

    /// Drops every finished entry (`local_elapsed >= duration` as of
    /// this timeline's current elapsed time) -- a real caller calls this
    /// periodically (e.g. once a run of `advance` calls reports
    /// `still_running == false` for its own tracked targets) to keep a
    /// long-lived `Timeline` from accumulating dead entries forever.
    /// Returns the `target` id of every entry actually removed, so a
    /// caller keeping its own side-table keyed by `target` (as
    /// `tre-python`'s own `PyTimeline` does) can prune that table in
    /// lockstep instead of guessing.
    pub fn prune_finished(&mut self) -> Vec<u64> {
        let elapsed = self.elapsed;
        let mut removed = Vec::new();
        self.entries.retain(|entry| {
            let finished = elapsed - entry.start_time >= entry.tween.duration;
            if finished {
                removed.push(entry.target);
            }
            !finished
        });
        removed
    }

    #[must_use]
    pub fn active_count(&self) -> usize {
        self.entries.len()
    }
}

/// Re-exported so a caller building a `Timeline` never needs its own
/// separate `tre-tween` dependency for the common case.
pub use tre_tween::Tween as TimelineTween;

/// Convenience constructor mirroring `Tween::new`, re-exported at the
/// crate root so `tre_animation::ease(...)`/`Tween::new(...)` both work
/// without a caller reaching into `tre-tween` directly.
#[must_use]
pub fn tween(from: f32, to: f32, duration: f32, easing: Easing) -> Tween<f32> {
    Tween::new(from, to, duration, easing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_entry_samples_exactly_like_its_own_tween() {
        let mut timeline = Timeline::new();
        timeline.animate(1, tween(0.0, 10.0, 1.0, Easing::Linear));
        let (samples, still_running) = timeline.advance(0.5);
        assert_eq!(samples, vec![(1, 5.0)]);
        assert!(still_running);
    }

    #[test]
    fn advance_accumulates_across_multiple_calls() {
        let mut timeline = Timeline::new();
        timeline.animate(1, tween(0.0, 10.0, 1.0, Easing::Linear));
        timeline.advance(0.3);
        let (samples, still_running) = timeline.advance(0.3);
        // Total elapsed is now 0.6s into a 1.0s tween.
        let (_, value) = samples[0];
        assert!((value - 6.0).abs() <= 1e-4, "expected ~6.0, got {value}");
        assert!(still_running);
    }

    #[test]
    fn an_entry_reports_not_running_once_its_own_duration_elapses() {
        let mut timeline = Timeline::new();
        timeline.animate(1, tween(0.0, 10.0, 1.0, Easing::Linear));
        let (samples, still_running) = timeline.advance(2.0);
        assert_eq!(samples, vec![(1, 10.0)]);
        assert!(!still_running);
    }

    #[test]
    fn two_entries_started_at_different_times_sample_independently() {
        let mut timeline = Timeline::new();
        timeline.animate(1, tween(0.0, 10.0, 1.0, Easing::Linear));
        timeline.advance(0.5); // entry 1 is now halfway
        timeline.animate(2, tween(0.0, 100.0, 1.0, Easing::Linear)); // starts now, at elapsed=0.5
        let (samples, still_running) = timeline.advance(0.5); // elapsed=1.0 total
        let value_of = |target: u64| samples.iter().find(|(t, _)| *t == target).unwrap().1;
        // Entry 1: fully elapsed (1.0s of its own 1.0s duration) -> 10.0.
        assert!((value_of(1) - 10.0).abs() <= 1e-4);
        // Entry 2: only 0.5s of its own 1.0s duration has passed -> 50.0.
        assert!((value_of(2) - 50.0).abs() <= 1e-4);
        assert!(still_running, "entry 2 must still be running");
    }

    #[test]
    fn prune_finished_removes_only_entries_past_their_own_duration() {
        let mut timeline = Timeline::new();
        timeline.animate(1, tween(0.0, 10.0, 1.0, Easing::Linear));
        timeline.animate(2, tween(0.0, 10.0, 5.0, Easing::Linear));
        timeline.advance(2.0);
        assert_eq!(timeline.active_count(), 2);
        let removed = timeline.prune_finished();
        assert_eq!(
            removed,
            vec![1],
            "only entry 1's own target id should be reported removed"
        );
        assert_eq!(
            timeline.active_count(),
            1,
            "only the 1.0s entry should be pruned"
        );
    }
}
