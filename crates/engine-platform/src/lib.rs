//! `winit` `EventLoop`/`ApplicationHandler` and the `accesskit_winit`
//! adapter.
//!
//! §14 build-order step 1: just enough to open a real OS window and pump
//! its event loop for `engine-render`'s own example. No `AppHandler`/
//! `InputEvent` dispatch yet (§4, §9) -- that lands once `engine-core`
//! defines those generic types. Surface/device/queue creation and
//! rendering are the caller's job; this crate only owns the window and
//! its event pump.
//!
//! §14 step 7 adds the real `accesskit_winit::Adapter` wiring (§10: "the
//! only crate depending on `winit`" is also the one that owns
//! `accesskit_winit`, `engine-core` owns the plain `accesskit` data
//! crate and `Tree::build_access_update`). Every window `run_windowed`
//! opens is accessible from the start -- `build_access_update` is a
//! required second closure, not optional, matching §10's own "keyboard
//! operability ships from day one" stance; a caller with no interesting
//! `AccessNodeData` set yet still gets a valid (if minimal, all
//! `Role::Unknown`) tree rather than no tree at all.
//!
//! Uses [`accesskit_winit::Adapter::with_event_loop_proxy`], not
//! `with_direct_handlers`: the direct-handler traits require `Send`
//! (each "may be called on any thread, depending on the underlying
//! platform adapter"), and this crate's own `Tree` is deliberately
//! `Rc<RefCell<Tree>>` (`engine-py`'s own real finding, §9 -- `!Send` by
//! design). The proxy approach keeps only a thin, genuinely `Send`
//! `PlatformEvent` crossing threads; the actual `Tree`-touching
//! `build_access_update` call always happens back on the main thread,
//! inside `user_event`/`window_event`, same as everything else here.
//!
//! §14 step 14 (§11.1) adds real multi-window support:
//! [`run_windowed_multi`] manages any number of simultaneously open
//! windows, each with its own `accesskit_winit::Adapter` and frame
//! counter, keyed by `winit`'s own `WindowId` -- `accesskit_winit::
//! Event` already carries a real `window_id` (confirmed directly in its
//! source), so routing *that* to the right window is a genuine `HashMap`
//! lookup, not new dispatch machinery, matching §11.1's own claim for
//! exactly the two kinds of event this crate has ever dispatched
//! (window-level `winit` events, `accesskit` events) -- real pointer/
//! keyboard `InputEvent` dispatch still doesn't exist anywhere in this
//! codebase, so that part of §11.1's text ("unchanged from the single-
//! window model already designed") stays aspirational until a later
//! step builds it. [`run_windowed`] (the original single-window
//! signature) is now a thin wrapper over [`run_windowed_multi`], kept
//! byte-for-byte source-compatible for its two existing callers
//! (`rect_window.rs`, `access_button.rs`) rather than churning them for
//! a capability neither test needs.
//!
//! Windows are only ever opened up front, via the `setup` closure,
//! before the blocking event loop starts -- not dynamically mid-session
//! from a live external call. Nothing needs that yet (Python's own
//! single call stack can't interleave with the blocking loop without a
//! callback hook this step doesn't build), and `WindowOpener` staying
//! usable only inside `setup` is a real, stated scope limit, not an
//! oversight.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowAttributes, WindowId};

pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    /// Auto-exit after this many redraws, so a demo built on this is
    /// self-asserting and headless-CI-safe instead of requiring a human
    /// to close the window -- TRE v1's own convention
    /// (LESSONS_LEARNED.md, the "gracefully exit 0" lesson from finding
    /// #261). `None` runs until the user closes the window.
    pub max_frames: Option<u32>,
}

/// One window-open request, tagged with a caller-assigned `token` so
/// [`run_windowed_multi`]'s `on_window_created` callback can correlate
/// the real `WindowId` it's handed back with whatever the caller
/// originally asked for (e.g. `engine-py`'s `App` uses this to know
/// which registered `PyWindow` a newly created OS window belongs to).
pub struct WindowRequest {
    pub config: WindowConfig,
    pub token: u64,
}

/// The winit user-event type this crate's `EventLoop` is built with --
/// exists purely to carry `accesskit_winit::Event` and window-open
/// requests across the proxy boundary (see the module doc comment for
/// why `with_event_loop_proxy`, not the direct-handler API).
enum PlatformEvent {
    AccessKit(accesskit_winit::Event),
    OpenWindow(WindowRequest),
}

impl From<accesskit_winit::Event> for PlatformEvent {
    fn from(event: accesskit_winit::Event) -> Self {
        Self::AccessKit(event)
    }
}

/// A handle for requesting new windows, valid only inside the `setup`
/// closure `run_windowed_multi` calls once, before the event loop
/// starts running -- see the module doc comment for why this doesn't
/// (yet) support opening a window later, mid-session.
pub struct WindowOpener {
    proxy: EventLoopProxy<PlatformEvent>,
}

impl WindowOpener {
    pub fn open_window(&self, request: WindowRequest) {
        let _ = self.proxy.send_event(PlatformEvent::OpenWindow(request));
    }
}

/// Runs a `winit` event loop managing any number of windows.
///
/// `setup` is called once, synchronously, before the loop starts --
/// its only job is to request the initial window(s) via the given
/// [`WindowOpener`]. For each window actually created, `on_window_created`
/// fires exactly once with its real `WindowId`, the `token` from
/// whichever `WindowRequest` produced it, and an owned `Arc<Window>` --
/// the caller's one chance to build (and stash, keyed by `WindowId`) any
/// per-window GPU/render state. `on_frame` then fires once per redraw
/// with just the `WindowId` and 0-based frame index (the caller already
/// has everything else from `on_window_created`); `build_access_update`
/// fires per window the same way, keeping each window's own exposed
/// accessibility tree in sync (§10). Exits once every window has closed
/// or reached its own `max_frames`.
///
/// Returns `Err` rather than panicking if no display is reachable --
/// see [`run_windowed`]'s own doc comment for why that's expected, not
/// exceptional, on some CI runners.
pub fn run_windowed_multi<C, F, A, S>(
    on_window_created: C,
    on_frame: F,
    build_access_update: A,
    setup: S,
) -> Result<(), winit::error::EventLoopError>
where
    C: FnMut(WindowId, u64, Arc<Window>),
    F: FnMut(WindowId, u32),
    A: FnMut(WindowId) -> accesskit::TreeUpdate,
    S: FnOnce(&WindowOpener),
{
    let event_loop = EventLoop::<PlatformEvent>::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let proxy = event_loop.create_proxy();

    setup(&WindowOpener {
        proxy: proxy.clone(),
    });

    let mut app = MultiWindowApp {
        windows: HashMap::new(),
        proxy,
        on_window_created,
        on_frame,
        build_access_update,
    };
    event_loop.run_app(&mut app)
}

/// The original single-window entry point, kept source-compatible for
/// its two existing callers -- a thin wrapper over
/// [`run_windowed_multi`] that stashes the one `Arc<Window>` it gets
/// from `on_window_created` and hands it back to `on_frame` every
/// redraw, matching this function's own pre-step-14 signature exactly.
pub fn run_windowed<F, A>(
    config: WindowConfig,
    mut on_frame: F,
    mut build_access_update: A,
) -> Result<(), winit::error::EventLoopError>
where
    F: FnMut(&Arc<Window>, u32),
    A: FnMut() -> accesskit::TreeUpdate,
{
    let window: Rc<RefCell<Option<Arc<Window>>>> = Rc::new(RefCell::new(None));
    let window_for_created = window.clone();

    run_windowed_multi(
        move |_id, _token, created| {
            *window_for_created.borrow_mut() = Some(created);
        },
        move |_id, frame| {
            let window = window
                .borrow()
                .clone()
                .expect("on_window_created always fires before on_frame for the same window");
            on_frame(&window, frame);
        },
        move |_id| build_access_update(),
        |opener| {
            opener.open_window(WindowRequest { config, token: 0 });
        },
    )
}

struct PerWindow {
    window: Arc<Window>,
    access_adapter: accesskit_winit::Adapter,
    frame: u32,
    max_frames: Option<u32>,
}

struct MultiWindowApp<C, F, A> {
    windows: HashMap<WindowId, PerWindow>,
    proxy: EventLoopProxy<PlatformEvent>,
    on_window_created: C,
    on_frame: F,
    build_access_update: A,
}

impl<C, F, A> ApplicationHandler<PlatformEvent> for MultiWindowApp<C, F, A>
where
    C: FnMut(WindowId, u64, Arc<Window>),
    F: FnMut(WindowId, u32),
    A: FnMut(WindowId) -> accesskit::TreeUpdate,
{
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        // Windows are created lazily, in `user_event`, as `OpenWindow`
        // requests arrive -- not eagerly here. `setup`'s own
        // `open_window` calls (sent before the loop starts) are queued
        // on the proxy and delivered as the very first `user_event`
        // calls once the loop is actually running.
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: PlatformEvent) {
        match event {
            PlatformEvent::OpenWindow(request) => {
                // accesskit_winit's own hard requirement: the adapter
                // must be created before the window is ever shown,
                // which means creating the window invisible first.
                let attrs = WindowAttributes::default()
                    .with_title(request.config.title.clone())
                    .with_inner_size(winit::dpi::LogicalSize::new(
                        request.config.width,
                        request.config.height,
                    ))
                    .with_visible(false);
                let window = event_loop
                    .create_window(attrs)
                    .expect("failed to create window");
                let adapter = accesskit_winit::Adapter::with_event_loop_proxy(
                    event_loop,
                    &window,
                    self.proxy.clone(),
                );
                window.set_visible(true);
                window.request_redraw();
                let id = window.id();
                let window = Arc::new(window);

                (self.on_window_created)(id, request.token, window.clone());
                self.windows.insert(
                    id,
                    PerWindow {
                        window,
                        access_adapter: adapter,
                        frame: 0,
                        max_frames: request.config.max_frames,
                    },
                );
            }
            PlatformEvent::AccessKit(event) => {
                // `ActionRequested`/`AccessibilityDeactivated` have no
                // interactive dispatch wired yet -- §14 step 7's own
                // minimal scope (see this module's doc comment); only
                // the initial-tree request is handled.
                if matches!(
                    event.window_event,
                    accesskit_winit::WindowEvent::InitialTreeRequested
                ) {
                    let Self {
                        windows,
                        build_access_update,
                        ..
                    } = self;
                    if let Some(win) = windows.get_mut(&event.window_id) {
                        win.access_adapter
                            .update_if_active(|| build_access_update(event.window_id));
                    }
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Self {
            windows,
            on_frame,
            build_access_update,
            ..
        } = self;
        let Some(win) = windows.get_mut(&window_id) else {
            return;
        };
        win.access_adapter.process_event(&win.window, &event);

        match event {
            WindowEvent::CloseRequested => {
                windows.remove(&window_id);
                if windows.is_empty() {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                on_frame(window_id, win.frame);
                win.access_adapter
                    .update_if_active(|| build_access_update(window_id));
                win.frame += 1;
                if let Some(max) = win.max_frames
                    && win.frame >= max
                {
                    windows.remove(&window_id);
                    if windows.is_empty() {
                        event_loop.exit();
                    }
                    return;
                }
                win.window.request_redraw();
            }
            _ => {}
        }
    }
}
