//! Platform input events (TECHNICAL.md Section 8) and the frame
//! clock: `WindowId`, `MouseButton`, `ElementState`, `InputEvent`,
//! `InputEventQueue`, `FrameClock` -- split out of `lib.rs`
//! (REVIEW.md #205). Self-contained: nothing else in this crate's
//! other split-out modules (`canvas`, `rhi`) depends on it.

/// Opaque identifier for a platform window, assigned by
/// `tre-platform`'s `PlatformConnection` when a window is created
/// (IMPLEMENTATION.md Step 1.2). Stable for that window's lifetime and
/// never reused while the owning connection is alive, so it is safe to use
/// as a stable map key (e.g. per-window swapchain lookup) rather than a
/// raw pointer or index that could be invalidated by window closure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(pub u64);

/// A pointer button (TECHNICAL.md Section 8's input event model).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    /// A raw platform button code for buttons beyond the three common
    /// ones (e.g. side/forward-back buttons), passed through unchanged.
    Other(u16),
}

/// Whether a button or key was pressed or released.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementState {
    Pressed,
    Released,
}

/// A single translated, backend-agnostic event flowing from the platform
/// layer to the engine (TECHNICAL.md Section 8): the SPSC ring buffer's
/// payload type. Every variant carries the [`WindowId`] it originated
/// from so a multi-window application can route events without querying
/// per-backend state -- this is also why window lifecycle events
/// (`CloseRequested`, `Resized`) live here rather than in a separate
/// per-window enum: `PlatformConnection` now owns multiple windows behind
/// one shared connection, so every event it produces needs the same
/// window-tagging regardless of category.
///
/// `PointerMoved` events are coalesced by [`InputEventQueue`]
/// (IMPLEMENTATION.md Step 1.2): a burst of raw OS motion events for the
/// same window collapses to the single most recent position, so a slow
/// consumer never falls behind on stale mouse positions the way it could
/// on discrete clicks or key presses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputEvent {
    PointerMoved {
        window: WindowId,
        x: f64,
        y: f64,
    },
    PointerButton {
        window: WindowId,
        button: MouseButton,
        state: ElementState,
    },
    /// `key_code` is the raw platform key code (Linux evdev keycode on
    /// both Wayland and X11, per `wl_keyboard`'s and X11 `KeyCode`'s
    /// shared evdev-based numbering) -- layout-aware translation is a UI
    /// framework concern (DESIGN.md Section 2.7), out of scope here.
    KeyboardKey {
        window: WindowId,
        key_code: u32,
        state: ElementState,
    },
    CloseRequested {
        window: WindowId,
    },
    Resized {
        window: WindowId,
        width: u32,
        height: u32,
    },
}

/// Producer-side queue wrapping `tre_memory::SpscRingBuffer<InputEvent>`
/// with pointer-move coalescing (TECHNICAL.md Section 8, IMPLEMENTATION.md
/// Step 1.2): a `PointerMoved` for the same window as the currently
/// staged pending move overwrites that staged value instead of being
/// published as a new queue entry, so a burst of high-frequency raw OS
/// motion events collapses to the single most recent position by the
/// time a consumer drains the queue.
///
/// The staged value lives in this producer-exclusive struct field, never
/// in an already-published ring-buffer slot -- overwriting a *published*
/// slot in place would race a concurrent consumer that might be mid-read
/// of that exact slot (true whenever the queue holds exactly one
/// unconsumed item). Staging it here instead keeps the underlying
/// `SpscRingBuffer` itself untouched by this coalescing logic, so it
/// stays sound if a real second consumer thread is ever introduced,
/// matching that type's own "no redesign needed" design goal.
pub struct InputEventQueue {
    queue: tre_memory::SpscRingBuffer<InputEvent>,
    pending_move: Option<InputEvent>, // always `PointerMoved` when `Some`
}

impl InputEventQueue {
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            queue: tre_memory::SpscRingBuffer::with_capacity(capacity),
            pending_move: None,
        }
    }

    /// Producer-side: enqueues `event`, coalescing consecutive
    /// `PointerMoved`s for the same window per this type's doc comment.
    /// A full underlying queue silently drops the event rather than
    /// blocking or panicking (DESIGN.md Section 2.6): input events are a
    /// UI convenience, never something worth stalling a render frame for.
    pub fn push(&mut self, event: InputEvent) {
        if let InputEvent::PointerMoved { window, .. } = event {
            let coalesces = matches!(
                self.pending_move,
                Some(InputEvent::PointerMoved { window: pending_window, .. })
                    if pending_window == window
            );
            if !coalesces {
                self.flush_pending_move();
            }
            self.pending_move = Some(event);
            return;
        }
        self.flush_pending_move();
        let _ = self.queue.push(event);
    }

    /// Publishes the currently staged pending move, if any. Callers
    /// should call this once per polling cycle after translating all
    /// available raw OS events, so a move isn't left stuck in staging
    /// with nothing left to flush it this cycle.
    pub fn flush_pending_move(&mut self) {
        if let Some(event) = self.pending_move.take() {
            let _ = self.queue.push(event);
        }
    }

    /// Non-blocking drain (this step's stand-in for a real cross-thread
    /// consumer, per `PLAN.md`'s scope decision): flushes any pending
    /// move, then pops every currently queued event into a `Vec`.
    #[must_use]
    pub fn drain(&mut self) -> Vec<InputEvent> {
        self.flush_pending_move();
        std::iter::from_fn(|| self.queue.pop()).collect()
    }
}

/// A real, hardware-backed monotonic frame timer (IMPLEMENTATION.md Phase 8
/// Step 8.1 task 1; TECHNICAL.md Section 7.1's `QueryPerformanceCounter`/
/// `clock_gettime(CLOCK_MONOTONIC)` requirement -- satisfied by
/// `std::time::Instant` itself, which wraps exactly these platform APIs
/// internally, so no per-platform `#[cfg]` code is needed here). Owns real,
/// mutable frame-to-frame state (the previous tick's own timestamp), an
/// engine-lifecycle concern -- distinct from `tre-math::spring_decay`'s
/// pure, state-free formula, which a caller feeds this clock's own output
/// `dt` into.
pub struct FrameClock {
    previous_tick: Option<std::time::Instant>,
}

impl FrameClock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            previous_tick: None,
        }
    }

    /// Returns the real elapsed seconds since the previous call to `tick`,
    /// as an `f32`. The first call has no prior tick to measure a delta
    /// from, so it returns `0.0` rather than an arbitrary or undefined
    /// value.
    pub fn tick(&mut self) -> f32 {
        let now = std::time::Instant::now();
        let delta = match self.previous_tick {
            Some(previous) => now.duration_since(previous).as_secs_f32(),
            None => 0.0,
        };
        self.previous_tick = Some(now);
        delta
    }
}

impl Default for FrameClock {
    fn default() -> Self {
        Self::new()
    }
}
