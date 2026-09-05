//! Standalone verification that native windowing and input work before
//! wiring them into Vulkan: opens a window (Wayland or X11 depending on
//! TRE_FORCE_BACKEND / the session type), prints every event (window
//! lifecycle and pointer/keyboard input), and exits on close or after a
//! frame budget.

use tre_platform::{InputEvent, PlatformConnection};

fn main() {
    let backend = std::env::var("TRE_FORCE_BACKEND").unwrap_or_default();
    let mut connection = match backend.as_str() {
        "wayland" => PlatformConnection::new_wayland(),
        "x11" => PlatformConnection::new_x11(),
        _ => PlatformConnection::new(),
    }
    .expect("failed to connect to display server");

    let window = connection
        .create_window("tre platform smoke test", 480, 320)
        .expect("failed to open window");

    eprintln!(
        "window opened, scale factor = {}",
        connection.scale_factor(window)
    );

    let max_iters: u32 = std::env::var("TRE_SMOKE_TEST_ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(600);

    for i in 0..max_iters {
        let events = connection.poll_events();
        for event in &events {
            eprintln!("[{i}] event: {event:?}");
        }
        if events
            .iter()
            .any(|e| matches!(e, InputEvent::CloseRequested { .. }))
        {
            eprintln!("close requested, exiting");
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    eprintln!("smoke test finished ({max_iters} iterations) without a close request");
}
