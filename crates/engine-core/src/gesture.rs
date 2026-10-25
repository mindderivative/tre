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
    /// 0.5.8 (#132): two fingers turning about their midpoint, once past the
    /// rotation slop. Comes with the pinch the same fingers make.
    Rotate,
    /// 0.5.8 (#132): a pan that lifted fast and far enough to be a flick in one
    /// direction. Comes just after the pan's own `Ended`.
    Swipe { direction: SwipeDirection },
}

/// The way a swipe went: the pan's dominant axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwipeDirection {
    Left,
    Right,
    Up,
    Down,
}

impl SwipeDirection {
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Up => "up",
            Self::Down => "down",
        }
    }
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
    /// A rotation's angle so far, in radians from where the fingers started,
    /// positive clockwise on the screen (y down); zero for any other gesture.
    pub rotation: f64,
    /// A rotation's step since the last event, in radians.
    pub rotation_delta: f64,
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
            rotation: 0.0,
            rotation_delta: 0.0,
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
    /// 0.5.8 (#132): how far two fingers have to turn, in radians, before it is
    /// a rotation (about 7 degrees): less is the wobble of a pinch.
    pub rotate_slop: f64,
    /// 0.5.8 (#132): a pan is a swipe if it lifts at least this fast, in pixels
    /// a second, ...
    pub swipe_velocity: f64,
    /// ... and has travelled at least this far, in pixels.
    pub swipe_distance: f64,
}

impl Default for GestureConfig {
    fn default() -> Self {
        Self {
            slop: 10.0,
            long_press: Duration::from_millis(500),
            tap_max: Duration::from_millis(300),
            double_tap_gap: Duration::from_millis(300),
            double_tap_distance: 30.0,
            rotate_slop: 0.12,
            swipe_velocity: 600.0,
            swipe_distance: 40.0,
        }
    }
}

/// The angle of the line from `a` to `b`, in radians (y down: clockwise is positive).
fn angle(a: Point, b: Point) -> f64 {
    (b.y - a.y).atan2(b.x - a.x)
}

/// `to - from` brought into (-pi, pi]: a turn across the branch cut is a small step.
fn angle_step(from: f64, to: f64) -> f64 {
    let mut step = to - from;
    while step > std::f64::consts::PI {
        step -= std::f64::consts::TAU;
    }
    while step <= -std::f64::consts::PI {
        step += std::f64::consts::TAU;
    }
    step
}

/// Which way a flick went: the larger of the two axes.
fn swipe_direction(velocity: Vec2) -> SwipeDirection {
    if velocity.x.abs() >= velocity.y.abs() {
        if velocity.x < 0.0 {
            SwipeDirection::Left
        } else {
            SwipeDirection::Right
        }
    } else if velocity.y < 0.0 {
        SwipeDirection::Up
    } else {
        SwipeDirection::Down
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
        /// 0.5.8 (#132): the line between the fingers last time, the turn so
        /// far, and whether it has passed the slop (a rotation has begun).
        last_angle: f64,
        rotation: f64,
        rotating: bool,
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

    /// 0.5.8 (#132): the distance, in layout pixels, a touch may wander and still
    /// be a tap or a long press. Takes effect with the next touch.
    pub fn set_slop(&mut self, slop: f64) {
        self.config.slop = slop.max(0.0);
    }

    pub fn slop(&self) -> f64 {
        self.config.slop
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
                    let mut out = vec![gesture];
                    // 0.5.8 (#132): a fast, long enough lift is also a swipe.
                    if ended
                        && gesture.velocity.hypot() >= self.config.swipe_velocity
                        && (position - origin).hypot() >= self.config.swipe_distance
                    {
                        let mut swipe = Gesture::new(
                            GestureKind::Swipe {
                                direction: swipe_direction(gesture.velocity),
                            },
                            GesturePhase::Ended,
                            position,
                            origin,
                        );
                        swipe.total = gesture.total;
                        swipe.velocity = gesture.velocity;
                        out.push(swipe);
                    }
                    out
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
                    last_angle,
                    rotation,
                    rotating,
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
                        last_angle,
                        rotation,
                        rotating,
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
                        // 0.5.8 (#132): the turn of the line between the fingers.
                        let now_angle = angle(a.position, b.position);
                        let step = angle_step(last_angle, now_angle);
                        let rotation = rotation + step;
                        let was_rotating = rotating;
                        let rotating = rotating || rotation.abs() >= self.config.rotate_slop;
                        let mut out = vec![gesture];
                        if rotating {
                            // The first event carries the whole turn so far, so an
                            // app that applies each step ends up at the right angle.
                            let mut turn = Gesture::new(
                                GestureKind::Rotate,
                                if was_rotating {
                                    GesturePhase::Changed
                                } else {
                                    GesturePhase::Began
                                },
                                mid,
                                origin_midpoint,
                            );
                            turn.delta = mid - last_midpoint;
                            turn.rotation = rotation;
                            turn.rotation_delta = if was_rotating { step } else { rotation };
                            out.push(turn);
                        }
                        self.state = State::Pinching {
                            a,
                            b,
                            origin_midpoint,
                            last_midpoint: mid,
                            start_distance,
                            last_scale: scale,
                            began: true,
                            last_angle: now_angle,
                            rotation,
                            rotating,
                        };
                        out
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
                        let mut out = vec![gesture];
                        if rotating {
                            let mut turn = Gesture::new(
                                GestureKind::Rotate,
                                if phase == TouchPhase::Ended {
                                    GesturePhase::Ended
                                } else {
                                    GesturePhase::Cancelled
                                },
                                mid,
                                origin_midpoint,
                            );
                            turn.rotation = rotation;
                            out.push(turn);
                        }
                        out
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
            last_angle: angle(a.position, b.position),
            rotation: 0.0,
            rotating: false,
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

    // --- 0.5.8 (#132): rotation and swipe

    /// Two fingers `r` apart about (100, 100) at `radians` (0 = the second to the right).
    fn pair(radians: f64, r: f64) -> (Point, Point) {
        let (dx, dy) = (radians.cos() * r / 2.0, radians.sin() * r / 2.0);
        (p(100.0 - dx, 100.0 - dy), p(100.0 + dx, 100.0 + dy))
    }

    fn rotates(gestures: &[Gesture]) -> Vec<&Gesture> {
        gestures
            .iter()
            .filter(|g| g.kind == GestureKind::Rotate)
            .collect()
    }

    /// Moves both fingers to `pair(radians, r)`, finger 1 then finger 2 (each its own
    /// touch event, as a screen sends them); returns everything they said.
    fn turn_to(r: &mut GestureRecognizer, c: &Clock, ms: u64, radians: f64) -> Vec<Gesture> {
        let (a, b) = pair(radians, 100.0);
        let mut out = r.touch(1, TouchPhase::Moved, a, c.at(ms));
        out.extend(r.touch(2, TouchPhase::Moved, b, c.at(ms + 1)));
        out
    }

    #[test]
    fn two_fingers_turning_past_the_slop_are_a_rotation_that_begins_with_the_whole_turn() {
        let (mut r, c) = (GestureRecognizer::default(), clock());
        let (a0, b0) = pair(0.0, 100.0);
        r.touch(1, TouchPhase::Started, a0, c.at(0));
        r.touch(2, TouchPhase::Started, b0, c.at(10));
        // A small wobble (0.05 rad, under the 0.12 slop): a pinch, no rotation.
        let g = turn_to(&mut r, &c, 20, 0.05);
        assert!(rotates(&g).is_empty(), "{g:?}");
        assert!(g.iter().any(|g| g.kind == GestureKind::Pinch));
        // Past the slop: it begins once, carrying the whole turn so far, and the
        // steps add up to the turn.
        let g = turn_to(&mut r, &c, 30, 0.3);
        let turns = rotates(&g);
        assert_eq!(turns[0].phase, GesturePhase::Began);
        assert!(turns[1..].iter().all(|t| t.phase == GesturePhase::Changed));
        assert!((turns[0].rotation_delta - turns[0].rotation).abs() < 1e-9);
        assert!((turns.last().unwrap().rotation - 0.3).abs() < 1e-9);
        let steps: f64 = turns.iter().map(|t| t.rotation_delta).sum();
        assert!((steps - 0.3).abs() < 1e-9, "{steps}");
        // Then only steps.
        let g = turn_to(&mut r, &c, 40, 0.5);
        let turns = rotates(&g);
        assert!(turns.iter().all(|t| t.phase == GesturePhase::Changed));
        assert!((turns.last().unwrap().rotation - 0.5).abs() < 1e-9);
        let steps: f64 = turns.iter().map(|t| t.rotation_delta).sum();
        assert!((steps - 0.2).abs() < 1e-9, "{steps}");
        // And it ends with the pinch, still carrying the total.
        let g = r.touch(2, TouchPhase::Ended, pair(0.5, 100.0).1, c.at(50));
        let turns = rotates(&g);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].phase, GesturePhase::Ended);
        assert!((turns[0].rotation - 0.5).abs() < 1e-9);
    }

    #[test]
    fn a_turn_the_other_way_is_negative_and_a_turn_across_the_branch_cut_is_a_small_step() {
        let (mut r, c) = (GestureRecognizer::default(), clock());
        let start = std::f64::consts::PI - 0.1; // just under the cut
        let (a0, b0) = pair(start, 100.0);
        r.touch(1, TouchPhase::Started, a0, c.at(0));
        r.touch(2, TouchPhase::Started, b0, c.at(10));
        // Clockwise (positive) across pi: the angle wraps to -pi + 0.2.
        let g = turn_to(&mut r, &c, 20, start + 0.3);
        assert!((rotates(&g).last().unwrap().rotation - 0.3).abs() < 1e-9);
        // Back, and past the start the other way.
        let g = turn_to(&mut r, &c, 30, start - 0.4);
        assert!((rotates(&g).last().unwrap().rotation - -0.4).abs() < 1e-9);
    }

    #[test]
    fn two_fingers_that_only_pinch_never_rotate() {
        let (mut r, c) = (GestureRecognizer::default(), clock());
        let (a0, b0) = pair(0.0, 100.0);
        r.touch(1, TouchPhase::Started, a0, c.at(0));
        r.touch(2, TouchPhase::Started, b0, c.at(10));
        for (i, width) in [120.0, 160.0, 90.0].into_iter().enumerate() {
            let (a, b) = pair(0.0, width);
            r.touch(1, TouchPhase::Moved, a, c.at(20 + i as u64 * 10));
            let g = r.touch(2, TouchPhase::Moved, b, c.at(25 + i as u64 * 10));
            assert!(rotates(&g).is_empty());
        }
        let g = r.touch(1, TouchPhase::Ended, pair(0.0, 90.0).0, c.at(100));
        assert!(rotates(&g).is_empty());
    }

    fn flick(
        r: &mut GestureRecognizer,
        c: &Clock,
        from: Point,
        to: Point,
        ms: u64,
    ) -> Vec<Gesture> {
        r.touch(1, TouchPhase::Started, from, c.at(0));
        let steps = 5;
        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            let at = Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
            r.touch(1, TouchPhase::Moved, at, c.at(ms * i / steps));
        }
        r.touch(1, TouchPhase::Ended, to, c.at(ms))
    }

    #[test]
    fn a_fast_long_pan_is_a_swipe_in_its_dominant_direction_after_the_pan_ends() {
        for (to, direction) in [
            (p(300.0, 105.0), SwipeDirection::Right),
            (p(-100.0, 95.0), SwipeDirection::Left),
            (p(105.0, 300.0), SwipeDirection::Down),
            (p(95.0, -100.0), SwipeDirection::Up),
        ] {
            let (mut r, c) = (GestureRecognizer::default(), clock());
            let g = flick(&mut r, &c, p(100.0, 100.0), to, 150);
            assert_eq!(g.len(), 2, "{g:?}");
            assert_eq!(g[0].kind, GestureKind::Pan);
            assert_eq!(g[0].phase, GesturePhase::Ended);
            assert_eq!(g[1].kind, GestureKind::Swipe { direction });
            assert!(g[1].velocity.hypot() >= 600.0);
            assert!(g[1].total.hypot() >= 40.0);
        }
    }

    #[test]
    fn a_slow_or_short_pan_is_not_a_swipe() {
        let (mut r, c) = (GestureRecognizer::default(), clock());
        // Far but slow (200 px in 2 s = 100 px/s).
        let g = flick(&mut r, &c, p(0.0, 0.0), p(200.0, 0.0), 2000);
        assert_eq!(g.len(), 1);
        // Fast but short (25 px in 20 ms = 1250 px/s, under the 40 px distance).
        let (mut r, c) = (GestureRecognizer::default(), clock());
        let g = flick(&mut r, &c, p(0.0, 0.0), p(25.0, 0.0), 20);
        assert!(
            g.iter()
                .all(|g| !matches!(g.kind, GestureKind::Swipe { .. })),
            "{g:?}"
        );
    }

    #[test]
    fn the_slop_can_be_set_and_the_pan_begins_past_it() {
        let (mut r, c) = (GestureRecognizer::default(), clock());
        r.set_slop(30.0);
        assert_eq!(r.slop(), 30.0);
        r.touch(1, TouchPhase::Started, p(0.0, 0.0), c.at(0));
        assert!(
            r.touch(1, TouchPhase::Moved, p(25.0, 0.0), c.at(10))
                .is_empty()
        );
        let g = r.touch(1, TouchPhase::Moved, p(35.0, 0.0), c.at(20));
        assert_eq!(g[0].kind, GestureKind::Pan);
    }
}
