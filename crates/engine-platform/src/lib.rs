//! `winit` `EventLoop`/`ApplicationHandler` and, later, the
//! `accesskit_winit` adapter.
//!
//! §14 build-order step 1: just enough to open a real OS window and pump
//! its event loop for `engine-render`'s own example. No `AppHandler`/
//! `InputEvent` dispatch yet (§4, §9) -- that lands once `engine-core`
//! defines those generic types. Surface/device/queue creation and
//! rendering are the caller's job; this crate only owns the window and
//! its event pump.

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
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

/// Runs a minimal winit event loop, calling `on_frame` once per redraw
/// with the live window and the current 0-based frame index. Exits after
/// `config.max_frames` redraws, or when the window is closed.
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
pub fn run_windowed<F>(
    config: WindowConfig,
    on_frame: F,
) -> Result<(), winit::error::EventLoopError>
where
    F: FnMut(&Arc<Window>, u32),
{
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App {
        config,
        window: None,
        frame: 0,
        on_frame,
    };
    event_loop.run_app(&mut app)
}

struct App<F> {
    config: WindowConfig,
    window: Option<Arc<Window>>,
    frame: u32,
    on_frame: F,
}

impl<F: FnMut(&Arc<Window>, u32)> ApplicationHandler for App<F> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = WindowAttributes::default()
            .with_title(self.config.title.clone())
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.config.width,
                self.config.height,
            ));
        let window = event_loop
            .create_window(attrs)
            .expect("failed to create window");
        window.request_redraw();
        self.window = Some(Arc::new(window));
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                let Some(window) = self.window.clone() else {
                    return;
                };
                (self.on_frame)(&window, self.frame);
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
