//! Standalone verification that native windowing works before wiring it
//! into Vulkan: opens a window (Wayland or X11 depending on
//! TRE_FORCE_BACKEND / the session type), prints every event, and exits
//! on close or after a frame budget.

use tre_platform::{PlatformWindow, WindowEvent};

fn main() {
    let backend = std::env::var("TRE_FORCE_BACKEND").unwrap_or_default();
    let mut window = match backend.as_str() {
        "wayland" => PlatformWindow::new_wayland("tre platform smoke test", 480, 320),
        "x11" => PlatformWindow::new_x11("tre platform smoke test", 480, 320),
        _ => PlatformWindow::new("tre platform smoke test", 480, 320),
    }
    .expect("failed to open window");

    eprintln!("window opened, scale factor = {}", window.scale_factor());

    let max_iters: u32 = std::env::var("TRE_SMOKE_TEST_ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(600);

    for i in 0..max_iters {
        let events = window.poll_events();
        for event in &events {
            eprintln!("[{i}] event: {event:?}");
        }
        if events.contains(&WindowEvent::CloseRequested) {
            eprintln!("close requested, exiting");
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    eprintln!("smoke test finished ({max_iters} iterations) without a close request");
}
