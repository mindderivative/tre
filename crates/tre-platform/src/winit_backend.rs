//! Native windowing backed by `winit` (Phase 11 Step 11.1), replacing the
//! previous hand-rolled Wayland (`wayland-client`)/X11 (`x11rb`) backends
//! with one implementation -- winit unifies both behind its own
//! `ApplicationHandler` callback model, so there is no longer a need for
//! two separate protocol integrations. See `lib.rs`'s module doc and
//! IMPLEMENTATION.md Step 11.1 for the full migration rationale.
//!
//! `ActiveEventLoop` (needed to create a `Window`) is only reachable inside
//! an `ApplicationHandler` callback, never from an arbitrary imperative
//! call, so [`WinitConnection::create_window`] stages a request and
//! immediately pumps the event loop once with `Some(Duration::ZERO)` --
//! verified against winit's own source to always run `new_events` (not
//! just `resumed`, which fires exactly once) on every single pump, even
//! with no real OS events pending. That pump drains the staged request
//! before `create_window` returns, so the method is fully synchronous
//! from the caller's perspective, matching the previous backends exactly.

use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};
use tre_engine::{ElementState, InputEvent, InputEventQueue, MouseButton, WindowId};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopBuilder};
use winit::platform::pump_events::EventLoopExtPumpEvents;
use winit::platform::scancode::PhysicalKeyExtScancode;
use winit::platform::wayland::EventLoopBuilderExtWayland;
use winit::platform::x11::EventLoopBuilderExtX11;
use winit::window::{Icon, Window, WindowAttributes};

use crate::{PlatformError, WindowIcon};

/// One connection's worth of queued events, sized generously for a
/// per-frame drain of a handful of windows' worth of input (matches the
/// previous backends' own `EVENT_QUEUE_CAPACITY`).
const EVENT_QUEUE_CAPACITY: usize = 256;

/// A window creation request staged by [`WinitConnection::create_window`]
/// and drained inside [`Handler::new_events`], the only place an
/// `ActiveEventLoop` (required to actually create the `Window`) is
/// reachable.
struct PendingCreate {
    id: u64,
    title: String,
    width: u32,
    height: u32,
}

struct Handler {
    /// Our id -> the `Window` it owns. Also the source of truth for
    /// `scale_factor`/`window_handle` lookups.
    windows: HashMap<u64, Window>,
    /// winit's own id -> our id, populated at creation time and consulted
    /// in `window_event` to translate incoming events back to our
    /// caller-facing `WindowId`.
    winit_to_ours: HashMap<winit::window::WindowId, u64>,
    next_id: u64,
    pending_creates: VecDeque<PendingCreate>,
    create_errors: HashMap<u64, PlatformError>,
    events: InputEventQueue,
}

impl Handler {
    fn new() -> Self {
        Self {
            windows: HashMap::new(),
            winit_to_ours: HashMap::new(),
            next_id: 0,
            pending_creates: VecDeque::new(),
            create_errors: HashMap::new(),
            events: InputEventQueue::with_capacity(EVENT_QUEUE_CAPACITY),
        }
    }
}

impl ApplicationHandler for Handler {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        // `Resumed` fires exactly once, on the very first `pump_app_events`
        // call -- pending window creation is drained in `new_events`
        // instead (below), which runs on every pump, so windows #2, #3,
        // ... are not stranded waiting for a `Resumed` that never repeats.
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, _cause: StartCause) {
        while let Some(req) = self.pending_creates.pop_front() {
            let attrs = WindowAttributes::default()
                .with_title(req.title)
                .with_inner_size(PhysicalSize::new(req.width, req.height));
            match event_loop.create_window(attrs) {
                Ok(window) => {
                    self.winit_to_ours.insert(window.id(), req.id);
                    self.windows.insert(req.id, window);
                }
                Err(e) => {
                    self.create_errors
                        .insert(req.id, PlatformError::Other(e.to_string()));
                }
            }
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(&ours) = self.winit_to_ours.get(&window_id) else {
            // Not one of our windows (or already torn down) -- drop it.
            return;
        };
        let window = WindowId(ours);
        match event {
            WindowEvent::CloseRequested => {
                self.events.push(InputEvent::CloseRequested { window });
            }
            WindowEvent::Resized(size) => {
                self.events.push(InputEvent::Resized {
                    window,
                    width: size.width,
                    height: size.height,
                });
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.events.push(InputEvent::PointerMoved {
                    window,
                    x: position.x,
                    y: position.y,
                });
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.events.push(InputEvent::PointerButton {
                    window,
                    button: map_mouse_button(button),
                    state: map_element_state(state),
                });
            }
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                // `to_scancode()` returns the Linux evdev keycode on both
                // Wayland and X11 (winit's own doc comment on
                // `PhysicalKeyExtScancode`) -- the exact numbering
                // `InputEvent::KeyboardKey::key_code` already contracts, so
                // this is a like-for-like replacement, not a new semantic.
                // `None` only for a truly unidentified key, dropped here
                // just as the previous backends implicitly dropped it.
                if let Some(key_code) = key_event.physical_key.to_scancode() {
                    self.events.push(InputEvent::KeyboardKey {
                        window,
                        key_code,
                        state: map_element_state(key_event.state),
                    });
                }
            }
            _ => {}
        }
    }
}

fn map_element_state(state: winit::event::ElementState) -> ElementState {
    match state {
        winit::event::ElementState::Pressed => ElementState::Pressed,
        winit::event::ElementState::Released => ElementState::Released,
    }
}

/// Winit's `Back`/`Forward` variants are already decoded, semantic values
/// (unlike `Other(u16)`, which does carry a raw platform code) -- mapped
/// here to the conventional Linux evdev `BTN_BACK`/`BTN_FORWARD` codes
/// (`linux/input-event-codes.h`: 0x116/0x115) as the closest match to
/// `MouseButton::Other`'s "raw platform button code" contract, since no
/// current caller distinguishes these two buttons specially.
fn map_mouse_button(button: winit::event::MouseButton) -> MouseButton {
    match button {
        winit::event::MouseButton::Left => MouseButton::Left,
        winit::event::MouseButton::Right => MouseButton::Right,
        winit::event::MouseButton::Middle => MouseButton::Middle,
        winit::event::MouseButton::Back => MouseButton::Other(0x116),
        winit::event::MouseButton::Forward => MouseButton::Other(0x115),
        winit::event::MouseButton::Other(code) => MouseButton::Other(code),
    }
}

/// One `winit::event_loop::EventLoop`-backed connection, shared by every
/// window it creates -- both `PlatformConnection::Wayland`/`X11` variants
/// wrap this same type, built with a forced backend
/// (`EventLoopBuilderExtWayland::with_wayland`/
/// `EventLoopBuilderExtX11::with_x11`) or left to winit's own
/// auto-detection.
///
/// Winit permits constructing an `EventLoop` only once per process, ever
/// (a permanent, process-global restriction, not reset between calls) --
/// harmless for every current caller (every demo/example is a separate
/// `fn main()` process, each building exactly one `PlatformConnection`),
/// but worth knowing before any future in-process test tries to build two.
pub struct WinitConnection {
    event_loop: EventLoop<()>,
    handler: Handler,
}

impl WinitConnection {
    pub fn new() -> Result<Self, PlatformError> {
        Self::build(EventLoop::builder())
    }

    pub fn new_wayland() -> Result<Self, PlatformError> {
        let mut builder = EventLoop::builder();
        builder.with_wayland();
        Self::build(builder)
    }

    pub fn new_x11() -> Result<Self, PlatformError> {
        let mut builder = EventLoop::builder();
        builder.with_x11();
        Self::build(builder)
    }

    fn build(mut builder: EventLoopBuilder<()>) -> Result<Self, PlatformError> {
        let event_loop = builder.build().map_err(|e| match e {
            winit::error::EventLoopError::RecreationAttempt => PlatformError::ConnectionFailed,
            other => PlatformError::Other(other.to_string()),
        })?;
        Ok(Self {
            event_loop,
            handler: Handler::new(),
        })
    }

    pub fn create_window(
        &mut self,
        title: &str,
        width: u32,
        height: u32,
    ) -> Result<WindowId, PlatformError> {
        let id = self.handler.next_id;
        self.handler.next_id += 1;
        self.handler.pending_creates.push_back(PendingCreate {
            id,
            title: title.to_string(),
            width,
            height,
        });
        // Immediately pump once so the request above is drained inside
        // `Handler::new_events` before this method returns -- see this
        // module's doc comment for why that is guaranteed to happen.
        let _ = self
            .event_loop
            .pump_app_events(Some(Duration::ZERO), &mut self.handler);
        match self.handler.create_errors.remove(&id) {
            Some(e) => Err(e),
            None => Ok(WindowId(id)),
        }
    }

    pub fn poll_events(&mut self) -> Vec<InputEvent> {
        let _ = self
            .event_loop
            .pump_app_events(Some(Duration::ZERO), &mut self.handler);
        self.handler.events.drain()
    }

    /// Real per-window value from winit (Wayland `wp-fractional-scale`
    /// falling back to integer scale; X11 `Xft.dpi`/RandR), returned at
    /// its own real `f64` precision (REVIEW.md finding #178: the previous
    /// `i32` signature rounded this away deliberately, as a bounded-scope
    /// trade-off; widened here since it's now the whole point of the
    /// call). An unknown/already-closed `window` returns `1.0`, matching
    /// the previous X11 backend's own unconditional integer default.
    #[must_use]
    pub fn scale_factor(&self, window: WindowId) -> f64 {
        self.handler
            .windows
            .get(&window.0)
            .map_or(1.0, Window::scale_factor)
    }

    /// # Errors
    /// Returns [`HandleError::Unavailable`] if `window` is not a window
    /// created by this connection (e.g. it was already closed and removed).
    pub fn window_handle(&self, window: WindowId) -> Result<WindowHandle<'_>, HandleError> {
        self.handler
            .windows
            .get(&window.0)
            .ok_or(HandleError::Unavailable)?
            .window_handle()
    }

    fn window(&self, window: WindowId) -> Result<&Window, PlatformError> {
        self.handler
            .windows
            .get(&window.0)
            .ok_or(PlatformError::UnknownWindow)
    }

    pub fn set_title(&self, window: WindowId, title: &str) -> Result<(), PlatformError> {
        self.window(window)?.set_title(title);
        Ok(())
    }

    pub fn set_minimized(&self, window: WindowId, minimized: bool) -> Result<(), PlatformError> {
        self.window(window)?.set_minimized(minimized);
        Ok(())
    }

    #[must_use]
    pub fn is_minimized(&self, window: WindowId) -> Option<bool> {
        self.handler.windows.get(&window.0)?.is_minimized()
    }

    pub fn set_maximized(&self, window: WindowId, maximized: bool) -> Result<(), PlatformError> {
        self.window(window)?.set_maximized(maximized);
        Ok(())
    }

    #[must_use]
    pub fn is_maximized(&self, window: WindowId) -> bool {
        self.handler
            .windows
            .get(&window.0)
            .is_some_and(Window::is_maximized)
    }

    pub fn set_icon(
        &self,
        window: WindowId,
        icon: Option<WindowIcon>,
    ) -> Result<(), PlatformError> {
        let win = self.window(window)?;
        let icon = icon
            .map(|i| Icon::from_rgba(i.rgba, i.width, i.height))
            .transpose()
            .map_err(|e| PlatformError::Other(e.to_string()))?;
        win.set_window_icon(icon);
        Ok(())
    }
}

impl HasDisplayHandle for WinitConnection {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.event_loop.display_handle()
    }
}
