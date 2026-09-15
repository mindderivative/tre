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

/// The winit user-event type this crate's `EventLoop` is built with --
/// exists purely to carry `accesskit_winit::Event` across the proxy
/// boundary (see the module doc comment for why `with_event_loop_proxy`,
/// not the direct-handler API).
enum PlatformEvent {
    AccessKit(accesskit_winit::Event),
}

impl From<accesskit_winit::Event> for PlatformEvent {
    fn from(event: accesskit_winit::Event) -> Self {
        Self::AccessKit(event)
    }
}

/// Runs a minimal winit event loop, calling `on_frame` once per redraw
/// with the live window and the current 0-based frame index, and
/// `build_access_update` once per redraw (plus once immediately when the
/// platform's accessibility layer first requests a tree) to keep the
/// exposed accessibility tree in sync -- §10: "built fresh... every
/// frame, not maintained as a separate parallel structure that can
/// drift out of sync." Exits after `config.max_frames` redraws, or when
/// the window is closed.
///
/// Hands back `Arc<Window>`, not `&Window`: a caller building a
/// `wgpu::Surface` needs to create it once and keep it alive across every
/// subsequent frame, which an owned, cloneable handle supports and a
/// borrow scoped to one callback invocation cannot.
///
/// Returns `Err` rather than panicking if no display is reachable (e.g. a
/// CI runner with no X11/Wayland socket) -- this is an expected, non-
/// exceptional condition in that environment, not a bug, and every
/// `[[test]] harness = false` target built on this is expected to exit 0
/// on it rather than fail the suite (TRE v1's own convention, finding
/// #261: "gracefully exit 0, not panic, when no display/GPU is
/// reachable").
pub fn run_windowed<F, A>(
    config: WindowConfig,
    on_frame: F,
    build_access_update: A,
) -> Result<(), winit::error::EventLoopError>
where
    F: FnMut(&Arc<Window>, u32),
    A: FnMut() -> accesskit::TreeUpdate,
{
    let event_loop = EventLoop::<PlatformEvent>::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let proxy = event_loop.create_proxy();

    let mut app = App {
        config,
        window: None,
        access_adapter: None,
        proxy,
        frame: 0,
        on_frame,
        build_access_update,
    };
    event_loop.run_app(&mut app)
}

struct App<F, A> {
    config: WindowConfig,
    window: Option<Arc<Window>>,
    access_adapter: Option<accesskit_winit::Adapter>,
    proxy: EventLoopProxy<PlatformEvent>,
    frame: u32,
    on_frame: F,
    build_access_update: A,
}

impl<F, A> ApplicationHandler<PlatformEvent> for App<F, A>
where
    F: FnMut(&Arc<Window>, u32),
    A: FnMut() -> accesskit::TreeUpdate,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // accesskit_winit's own hard requirement: the adapter must be
        // created before the window is ever shown, which means creating
        // the window invisible first.
        let attrs = WindowAttributes::default()
            .with_title(self.config.title.clone())
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.config.width,
                self.config.height,
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
        self.access_adapter = Some(adapter);
        self.window = Some(Arc::new(window));
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: PlatformEvent) {
        let PlatformEvent::AccessKit(event) = event;
        // `ActionRequested`/`AccessibilityDeactivated` have no
        // interactive dispatch wired yet -- §14 step 7's own minimal
        // scope (see this module's doc comment); only the initial-tree
        // request is handled, so a screen reader gets a real tree as
        // soon as it asks, not just on the next redraw.
        if matches!(
            event.window_event,
            accesskit_winit::WindowEvent::InitialTreeRequested
        ) && let Some(adapter) = &mut self.access_adapter
        {
            adapter.update_if_active(&mut self.build_access_update);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if let (Some(window), Some(adapter)) = (&self.window, &mut self.access_adapter) {
            adapter.process_event(window, &event);
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                let Some(window) = self.window.clone() else {
                    return;
                };
                (self.on_frame)(&window, self.frame);
                if let Some(adapter) = &mut self.access_adapter {
                    adapter.update_if_active(&mut self.build_access_update);
                }
                self.frame += 1;
                if let Some(max) = self.config.max_frames
                    && self.frame >= max
                {
                    event_loop.exit();
                    return;
                }
                window.request_redraw();
            }
            _ => {}
        }
    }
}
