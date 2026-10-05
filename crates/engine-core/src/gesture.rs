//! 0.5.4 (#113): touch gestures -- tap, long press, pan and pinch -- from raw
//! touches.
//!
//! A pure state machine: it is fed touches and the time they happened (and
//! polled for the time passing, which a long press needs), and says what
//! gesture they make. It knows nothing of nodes or the window, so everything
//! about it is tested with synthetic touches and a synthetic clock; no touch
//! hardware is needed to know it recognizes what it should.
//!
//! Positions and distances are in the one unit the caller uses (the engine's
//! layout pixels).

use std::time::{Duration, Instant};

use peniko::kurbo::{Point, Vec2};

/// Where a touch is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    /// The system took the touch away (a palm, a gesture of its own).
    Cancelled,
}

/// What kind of gesture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GestureKind {
    /// A quick touch and release in place. `count` is 2 for a second tap close
    /// in time and place to the first.
    Tap { count: u8 },
    /// A touch held in place past the long-press time.
    LongPress,
    /// One finger dragging.
    Pan,
    /// Two fingers moving together or apart (and, with them, across).
    Pinch,
}

/// Where a pan or a pinch is in its life. A tap and a long press are one-shot
/// and use `Ended`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GesturePhase {
    Began,
    Changed,
    Ended,
    Cancelled,
}

/// A recognized gesture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gesture {
    pub kind: GestureKind,
    pub phase: GesturePhase,
    /// The touch (or, for a pinch, the midpoint of the two) now.
    pub position: Point,
    /// Where it started: the touch's first position, or the pinch's first midpoint.
    pub origin: Point,
    /// How far `position` moved since the last event of this gesture.
    pub delta: Vec2,
    /// How far it has moved in all, from `origin`.
    pub total: Vec2,
    /// A pinch's distance between fingers now over what it was at the start
    /// (`1.0` for any other gesture).
    pub scale: f64,
    /// A pinch's `scale` over the last event's: the step to apply (`1.0` otherwise).
    pub scale_delta: f64,
    /// A pan's speed when its fingers lift, in pixels per second; zero otherwise.
    pub velocity: Vec2,
}

impl Gesture {
    fn new(kind: GestureKind, phase: GesturePhase, position: Point, origin: Point) -> Self {
        Self {
            kind,
            phase,
            position,
            origin,
            delta: Vec2::ZERO,
            total: position - origin,
            scale: 1.0,
            scale_delta: 1.0,
            velocity: Vec2::ZERO,
        }
    }
}

/// How far, and how long, a touch has to go before it is one gesture and not
/// another.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GestureConfig {
    /// A touch that stays within this distance of where it started can still be a
    /// tap or a long press; beyond it, it is a pan.
    pub slop: f64,
    /// How long a touch held in place becomes a long press.
    pub long_press: Duration,
    /// A touch released within this is a tap; held longer (and not a long press
    /// because it moved a little) it is nothing.
    pub tap_max: Duration,
    /// A second tap within this time and `double_tap_distance` of the first is a
    /// double tap.
    pub double_tap_gap: Duration,
    pub double_tap_distance: f64,
}

impl Default for GestureConfig {
    fn default() -> Self {
        Self {
            slop: 10.0,
            long_press: Duration::from_millis(500),
            tap_max: Duration::from_millis(300),
            double_tap_gap: Duration::from_millis(300),
            double_tap_distance: 30.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Finger {
    id: u64,
    position: Point,
}

/// One finger down and not yet anything.
#[derive(Clone, Copy, Debug)]
struct Candidate {
    id: u64,
    origin: Point,
    since: Instant,
}

#[derive(Debug)]
enum State {
    Idle,
    /// A finger is down; it may become a tap, a long press or a pan.
    Pending(Candidate),
    /// The long press fired; the finger is still down, and nothing more comes of it.
    LongPressed(u64),
    Panning {
        id: u64,
        origin: Point,
        last: Point,
        total_from: Point,
        samples: Vec<(Instant, Point)>,
    },
    /// Two fingers down.
    Pinching {
        a: Finger,
        b: Finger,
        origin_midpoint: Point,
        last_midpoint: Point,
        start_distance: f64,
        last_scale: f64,
        began: bool,
    },
    /// A pinch ended with a finger still down: nothing until all lift.
    Spent(Vec<u64>),
}

/// Recognizes tap, long press, pan and pinch from touches.
pub struct GestureRecognizer {
    config: GestureConfig,
    state: State,
    last_tap: Option<(Instant, Point)>,
}

fn distance(a: Point, b: Point) -> f64 {
    (a - b).hypot()
}

fn midpoint(a: Point, b: Point) -> Point {
    Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
}

/// How far back a pan's release velocity looks.
const VELOCITY_WINDOW: Duration = Duration::from_millis(100);

impl Default for GestureRecognizer {
    fn default() -> Self {
        Self::new(GestureConfig::default())
    }
}

impl GestureRecognizer {
    pub fn new(config: GestureConfig) -> Self {
        Self {
            config,
            state: State::Idle,
            last_tap: None,
        }
    }

    /// Whether any finger is down: the caller should keep polling while so.
    pub fn is_active(&self) -> bool {
        !matches!(self.state, State::Idle)
    }

    /// When `poll` next has something to say, if a touch is waiting to become a
    /// long press.
    pub fn deadline(&self) -> Option<Instant> {
        match &self.state {
            State::Pending(c) => Some(c.since + self.config.long_press),
            _ => None,
        }
    }

    /// Feeds one touch at `now`; returns the gestures it completes or advances.
    pub fn touch(
        &mut self,
        id: u64,
        phase: TouchPhase,
        position: Point,
        now: Instant,
    ) -> Vec<Gesture> {
        let state = std::mem::replace(&mut self.state, State::Idle);
        match (state, phase) {
            // --- a first finger
            (State::Idle, TouchPhase::Started) => {
                self.state = State::Pending(Candidate {
                    id,
                    origin: position,
                    since: now,
                });
                Vec::new()
            }
            (State::Idle, _) => Vec::new(),

            (State::Pending(c), TouchPhase::Moved) if c.id == id => {
                if distance(position, c.origin) <= self.config.slop {
                    self.state = State::Pending(c);
                    return Vec::new();
                }
                let mut gesture =
                    Gesture::new(GestureKind::Pan, GesturePhase::Began, position, c.origin);
                gesture.delta = position - c.origin;
                self.state = State::Panning {
                    id,
                    origin: c.origin,
                    last: position,
                    total_from: c.origin,
                    samples: vec![(c.since, c.origin), (now, position)],
                };
                vec![gesture]
            }
            (State::Pending(c), TouchPhase::Ended) if c.id == id => {
                let held = now.saturating_duration_since(c.since);
                if held > self.config.tap_max {
                    self.last_tap = None;
                    return Vec::new();
                }
                let count = match self.last_tap {
                    Some((when, at))
                        if now.saturating_duration_since(when) <= self.config.double_tap_gap
                            && distance(at, position) <= self.config.double_tap_distance =>
                    {
                        2
                    }
                    _ => 1,
                };
                // A double tap is not the start of a triple.
                self.last_tap = (count == 1).then_some((now, position));
                vec![Gesture::new(
                    GestureKind::Tap { count },
                    GesturePhase::Ended,
                    position,
                    c.origin,
                )]
            }
            (State::Pending(c), TouchPhase::Cancelled) if c.id == id => Vec::new(),
            // A second finger: a pinch begins (when the fingers move).
            (State::Pending(c), TouchPhase::Started) if c.id != id => {
                self.last_tap = None;
                self.start_pinch(
                    Finger {
                        id: c.id,
                        position: c.origin,
                    },
                    Finger { id, position },
                );
                Vec::new()
            }
            (state @ State::Pending(_), _) => {
                self.state = state;
                Vec::new()
            }

            // --- a long press that already fired
            (State::LongPressed(held), TouchPhase::Ended | TouchPhase::Cancelled) if held == id => {
                Vec::new()
            }
            (State::LongPressed(held), _) => {
                self.state = State::LongPressed(held);
                Vec::new()
            }

            // --- a pan
            (
                State::Panning {
                    id: pan_id,
                    origin,
                    last,
                    total_from,
                    mut samples,
                },
                phase,
            ) if pan_id == id => match phase {
                TouchPhase::Moved => {
                    samples.push((now, position));
                    samples
                        .retain(|(t, _)| now.saturating_duration_since(*t) <= VELOCITY_WINDOW * 3);
                    let mut gesture =
                        Gesture::new(GestureKind::Pan, GesturePhase::Changed, position, origin);
                    gesture.delta = position - last;
                    gesture.total = position - total_from;
                    self.state = State::Panning {
                        id,
                        origin,
                        last: position,
                        total_from,
                        samples,
                    };
                    vec![gesture]
                }
                TouchPhase::Ended | TouchPhase::Cancelled => {
                    let ended = phase == TouchPhase::Ended;
                    let mut gesture = Gesture::new(
                        GestureKind::Pan,
                        if ended {
                            GesturePhase::Ended
                        } else {
                            GesturePhase::Cancelled
                        },
                        position,
                        origin,
                    );
                    gesture.delta = position - last;
                    gesture.total = position - total_from;
                    if ended {
                        samples.push((now, position));
                        gesture.velocity = release_velocity(&samples, now);
                    }
                    vec![gesture]
                }
                TouchPhase::Started => {
                    self.state = State::Panning {
                        id: pan_id,
                        origin,
                        last,
                        total_from,
                        samples,
                    };
                    Vec::new()
                }
            },
            // A second finger lands during a pan: the pan ends, a pinch begins.
            (
                State::Panning {
                    id: pan_id,
                    origin,
                    last,
                    total_from,
                    ..
                },
                TouchPhase::Started,
            ) => {
                let mut ended = Gesture::new(GestureKind::Pan, GesturePhase::Ended, last, origin);
                ended.total = last - total_from;
                self.start_pinch(
                    Finger {
                        id: pan_id,
                        position: last,
                    },
                    Finger { id, position },
                );
                vec![ended]
            }
            (state @ State::Panning { .. }, _) => {
                self.state = state;
                Vec::new()
            }

            // --- a pinch
            (
                State::Pinching {
                    mut a,
                    mut b,
                    origin_midpoint,
                    last_midpoint,
                    start_distance,
                    last_scale,
                    began,
                },
                phase,
            ) => {
                let moved = if a.id == id {
                    Some(&mut a)
                } else if b.id == id {
                    Some(&mut b)
                } else {
                    None
                };
                let Some(finger) = moved else {
                    // A third finger is ignored.
                    self.state = State::Pinching {
                        a,
                        b,
                        origin_midpoint,
                        last_midpoint,
                        start_distance,
                        last_scale,
                        began,
                    };
                    return Vec::new();
                };
                match phase {
                    TouchPhase::Moved | TouchPhase::Started => {
                        finger.position = position;
                        let mid = midpoint(a.position, b.position);
                        let scale = if start_distance > 0.0 {
                            distance(a.position, b.position) / start_distance
                        } else {
                            1.0
                        };
                        let mut gesture = Gesture::new(
                            GestureKind::Pinch,
                            if began {
                                GesturePhase::Changed
                            } else {
                                GesturePhase::Began
                            },
                            mid,
                            origin_midpoint,
                        );
                        gesture.delta = mid - last_midpoint;
                        gesture.scale = scale;
                        gesture.scale_delta = if last_scale > 0.0 {
                            scale / last_scale
                        } else {
                            1.0
                        };
                        self.state = State::Pinching {
                            a,
                            b,
                            origin_midpoint,
                            last_midpoint: mid,
                            start_distance,
                            last_scale: scale,
                            began: true,
                        };
                        vec![gesture]
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        finger.position = position;
                        let other = if a.id == id { b.id } else { a.id };
                        let mid = midpoint(a.position, b.position);
                        self.state = State::Spent(vec![other]);
                        if !began {
                            return Vec::new();
                        }
                        let mut gesture = Gesture::new(
                            GestureKind::Pinch,
                            if phase == TouchPhase::Ended {
                                GesturePhase::Ended
                            } else {
                                GesturePhase::Cancelled
                            },
                            mid,
                            origin_midpoint,
                        );
                        gesture.scale = last_scale;
                        vec![gesture]
                    }
                }
            }

            // --- fingers left down after a pinch
            (State::Spent(mut down), phase) => {
                match phase {
                    TouchPhase::Started => down.push(id),
                    TouchPhase::Ended | TouchPhase::Cancelled => down.retain(|d| *d != id),
                    TouchPhase::Moved => {}
                }
                if !down.is_empty() {
                    self.state = State::Spent(down);
                }
                Vec::new()
            }
        }
    }

    fn start_pinch(&mut self, a: Finger, b: Finger) {
        let start_distance = distance(a.position, b.position);
        let mid = midpoint(a.position, b.position);
        self.state = State::Pinching {
            a,
            b,
            origin_midpoint: mid,
            last_midpoint: mid,
            start_distance,
            last_scale: 1.0,
            began: false,
        };
    }

    /// The time passing: a touch held in place for the long-press time becomes a
    /// long press. Call it as time moves; returns what it fires.
    pub fn poll(&mut self, now: Instant) -> Vec<Gesture> {
        let State::Pending(c) = &self.state else {
            return Vec::new();
        };
        if now.saturating_duration_since(c.since) < self.config.long_press {
            return Vec::new();
        }
        let gesture = Gesture::new(
            GestureKind::LongPress,
            GesturePhase::Ended,
            c.origin,
            c.origin,
        );
        let id = c.id;
        self.state = State::LongPressed(id);
        self.last_tap = None;
        vec![gesture]
    }
}

/// The speed over the last `VELOCITY_WINDOW` of samples, in pixels a second.
fn release_velocity(samples: &[(Instant, Point)], now: Instant) -> Vec2 {
    let recent: Vec<&(Instant, Point)> = samples
        .iter()
        .filter(|(t, _)| now.saturating_duration_since(*t) <= VELOCITY_WINDOW)
        .collect();
    let (Some(first), Some(last)) = (recent.first(), recent.last()) else {
        return Vec2::ZERO;
    };
    let seconds = last.0.saturating_duration_since(first.0).as_secs_f64();
    if seconds <= 0.0 {
        return Vec2::ZERO;
    }
    (last.1 - first.1) / seconds
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Clock(Instant);
    impl Clock {
        fn at(&self, ms: u64) -> Instant {
            self.0 + Duration::from_millis(ms)
        }
    }

    fn clock() -> Clock {
        Clock(Instant::now())
    }

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn a_quick_touch_in_place_is_a_tap() {
        let (mut r, c) = (GestureRecognizer::default(), clock());
        assert!(
            r.touch(1, TouchPhase::Started, p(50.0, 50.0), c.at(0))
                .is_empty()
        );
        assert!(
            r.touch(1, TouchPhase::Moved, p(53.0, 52.0), c.at(40))
                .is_empty()
        );
        let g = r.touch(1, TouchPhase::Ended, p(53.0, 52.0), c.at(90));
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].kind, GestureKind::Tap { count: 1 });
        assert_eq!(g[0].position, p(53.0, 52.0));
        assert!(!r.is_active());
    }

    #[test]
    fn two_close_taps_are_a_double_tap_and_a_third_starts_over() {
        let (mut r, c) = (GestureRecognizer::default(), clock());
        let tap = |r: &mut GestureRecognizer, at: u64, x: f64| {
            r.touch(1, TouchPhase::Started, p(x, 10.0), c.at(at));
            r.touch(1, TouchPhase::Ended, p(x, 10.0), c.at(at + 50))
        };
        assert_eq!(tap(&mut r, 0, 20.0)[0].kind, GestureKind::Tap { count: 1 });
        assert_eq!(
            tap(&mut r, 200, 25.0)[0].kind,
            GestureKind::Tap { count: 2 }
        );
        assert_eq!(
            tap(&mut r, 400, 25.0)[0].kind,
            GestureKind::Tap { count: 1 }
        );
        // Too slow, or too far, is not a double.
        assert_eq!(
            tap(&mut r, 1500, 25.0)[0].kind,
            GestureKind::Tap { count: 1 }
        );
        assert_eq!(
            tap(&mut r, 1700, 200.0)[0].kind,
            GestureKind::Tap { count: 1 }
        );
    }

    #[test]
    fn a_touch_held_in_place_is_a_long_press_once_and_not_a_tap() {
        let (mut r, c) = (GestureRecognizer::default(), clock());
        r.touch(1, TouchPhase::Started, p(10.0, 10.0), c.at(0));
        assert_eq!(r.deadline(), Some(c.at(500)));
        assert!(r.poll(c.at(499)).is_empty());
        let g = r.poll(c.at(500));
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].kind, GestureKind::LongPress);
        assert_eq!(g[0].position, p(10.0, 10.0));
        assert!(r.poll(c.at(900)).is_empty(), "once");
        assert!(r.deadline().is_none());
        assert!(
            r.touch(1, TouchPhase::Ended, p(10.0, 10.0), c.at(1000))
                .is_empty()
        );
        assert!(!r.is_active());
    }

    #[test]
    fn a_slow_release_in_place_is_neither_tap_nor_long_press_if_polled_late() {
        let (mut r, c) = (GestureRecognizer::default(), clock());
        r.touch(1, TouchPhase::Started, p(10.0, 10.0), c.at(0));
        // Never polled, released after the tap time: nothing.
        assert!(
            r.touch(1, TouchPhase::Ended, p(10.0, 10.0), c.at(400))
                .is_empty()
        );
    }

    #[test]
    fn moving_past_the_slop_is_a_pan_that_reports_steps_and_totals() {
        let (mut r, c) = (GestureRecognizer::default(), clock());
        r.touch(1, TouchPhase::Started, p(100.0, 100.0), c.at(0));
        assert!(
            r.touch(1, TouchPhase::Moved, p(105.0, 100.0), c.at(10))
                .is_empty(),
            "inside the slop"
        );
        let began = r.touch(1, TouchPhase::Moved, p(120.0, 100.0), c.at(20));
        assert_eq!(began.len(), 1);
        assert_eq!(began[0].kind, GestureKind::Pan);
        assert_eq!(began[0].phase, GesturePhase::Began);
        assert_eq!(
            began[0].delta,
            Vec2::new(20.0, 0.0),
            "from where it started"
        );
        let changed = r.touch(1, TouchPhase::Moved, p(130.0, 110.0), c.at(30));
        assert_eq!(changed[0].phase, GesturePhase::Changed);
        assert_eq!(changed[0].delta, Vec2::new(10.0, 10.0));
        assert_eq!(changed[0].total, Vec2::new(30.0, 10.0));
        let ended = r.touch(1, TouchPhase::Ended, p(130.0, 110.0), c.at(40));
        assert_eq!(ended[0].phase, GesturePhase::Ended);
        assert_eq!(ended[0].total, Vec2::new(30.0, 10.0));
        assert!(!r.is_active());
    }

    #[test]
    fn a_pan_ends_with_the_speed_it_was_lifted_at() {
        let (mut r, c) = (GestureRecognizer::default(), clock());
        r.touch(1, TouchPhase::Started, p(0.0, 0.0), c.at(0));
        // 1 px a millisecond: 1000 px/s.
        for i in 1..=10u64 {
            r.touch(1, TouchPhase::Moved, p(i as f64 * 10.0, 0.0), c.at(i * 10));
        }
        let ended = r.touch(1, TouchPhase::Ended, p(100.0, 0.0), c.at(100));
        assert!(
            (ended[0].velocity.x - 1000.0).abs() < 50.0,
            "{:?}",
            ended[0].velocity
        );
        assert!(ended[0].velocity.y.abs() < 1.0);
    }

    #[test]
    fn a_pan_does_not_become_a_long_press() {
        let (mut r, c) = (GestureRecognizer::default(), clock());
        r.touch(1, TouchPhase::Started, p(0.0, 0.0), c.at(0));
        r.touch(1, TouchPhase::Moved, p(50.0, 0.0), c.at(100));
        assert!(r.poll(c.at(2000)).is_empty());
        assert!(r.deadline().is_none());
    }

    #[test]
    fn two_fingers_pinch_with_a_scale_and_a_midpoint() {
        let (mut r, c) = (GestureRecognizer::default(), clock());
        r.touch(1, TouchPhase::Started, p(100.0, 100.0), c.at(0));
        assert!(
            r.touch(2, TouchPhase::Started, p(200.0, 100.0), c.at(5))
                .is_empty()
        );
        // Spread: finger 2 moves out to 300, distance 100 -> 200.
        let began = r.touch(2, TouchPhase::Moved, p(300.0, 100.0), c.at(20));
        assert_eq!(began.len(), 1);
        assert_eq!(began[0].kind, GestureKind::Pinch);
        assert_eq!(began[0].phase, GesturePhase::Began);
        assert!((began[0].scale - 2.0).abs() < 1e-9);
        assert_eq!(began[0].position, p(200.0, 100.0));
        assert_eq!(began[0].origin, p(150.0, 100.0));
        // Pinch in to a distance of 100 again: scale 1, a step of 0.5.
        let changed = r.touch(2, TouchPhase::Moved, p(200.0, 100.0), c.at(40));
        assert_eq!(changed[0].phase, GesturePhase::Changed);
        assert!((changed[0].scale - 1.0).abs() < 1e-9);
        assert!((changed[0].scale_delta - 0.5).abs() < 1e-9);
        let ended = r.touch(2, TouchPhase::Ended, p(200.0, 100.0), c.at(60));
        assert_eq!(ended[0].phase, GesturePhase::Ended);
        // The finger still down does nothing, and lifting it ends all of it.
        assert!(r.is_active());
        assert!(
            r.touch(1, TouchPhase::Moved, p(0.0, 0.0), c.at(70))
                .is_empty()
        );
        assert!(
            r.touch(1, TouchPhase::Ended, p(0.0, 0.0), c.at(80))
                .is_empty()
        );
        assert!(!r.is_active());
    }

    #[test]
    fn two_fingers_that_never_move_make_no_gesture() {
        let (mut r, c) = (GestureRecognizer::default(), clock());
        r.touch(1, TouchPhase::Started, p(0.0, 0.0), c.at(0));
        r.touch(2, TouchPhase::Started, p(50.0, 0.0), c.at(5));
        assert!(
            r.touch(2, TouchPhase::Ended, p(50.0, 0.0), c.at(50))
                .is_empty()
        );
        assert!(
            r.touch(1, TouchPhase::Ended, p(0.0, 0.0), c.at(60))
                .is_empty()
        );
        assert!(!r.is_active());
    }

    #[test]
    fn a_second_finger_during_a_pan_ends_it_and_starts_a_pinch() {
        let (mut r, c) = (GestureRecognizer::default(), clock());
        r.touch(1, TouchPhase::Started, p(0.0, 0.0), c.at(0));
        r.touch(1, TouchPhase::Moved, p(40.0, 0.0), c.at(10));
        let g = r.touch(2, TouchPhase::Started, p(140.0, 0.0), c.at(20));
        assert_eq!(g.len(), 1);
        assert_eq!(
            (g[0].kind, g[0].phase),
            (GestureKind::Pan, GesturePhase::Ended)
        );
        let began = r.touch(2, TouchPhase::Moved, p(240.0, 0.0), c.at(30));
        assert_eq!(began[0].kind, GestureKind::Pinch);
        assert!((began[0].scale - 2.0).abs() < 1e-9);
    }

    #[test]
    fn a_cancelled_touch_cancels_its_gesture() {
        let (mut r, c) = (GestureRecognizer::default(), clock());
        r.touch(1, TouchPhase::Started, p(0.0, 0.0), c.at(0));
        r.touch(1, TouchPhase::Moved, p(40.0, 0.0), c.at(10));
        let g = r.touch(1, TouchPhase::Cancelled, p(40.0, 0.0), c.at(20));
        assert_eq!(g[0].phase, GesturePhase::Cancelled);
        assert!(!r.is_active());
        // And a cancelled tap makes no tap.
        r.touch(2, TouchPhase::Started, p(5.0, 5.0), c.at(100));
        assert!(
            r.touch(2, TouchPhase::Cancelled, p(5.0, 5.0), c.at(110))
                .is_empty()
        );
    }

    #[test]
    fn stray_touches_are_ignored() {
        let (mut r, c) = (GestureRecognizer::default(), clock());
        assert!(
            r.touch(9, TouchPhase::Moved, p(1.0, 1.0), c.at(0))
                .is_empty()
        );
        assert!(
            r.touch(9, TouchPhase::Ended, p(1.0, 1.0), c.at(1))
                .is_empty()
        );
        assert!(!r.is_active());
        // A different finger's move or end during a pending touch changes nothing.
        r.touch(1, TouchPhase::Started, p(0.0, 0.0), c.at(10));
        assert!(
            r.touch(5, TouchPhase::Moved, p(300.0, 0.0), c.at(20))
                .is_empty()
        );
        assert!(r.is_active());
    }
}
