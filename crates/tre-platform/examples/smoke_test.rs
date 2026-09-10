//! Standalone verification that native windowing and input work before
//! wiring them into Vulkan: opens a window (Wayland or X11 depending on
//! TRE_FORCE_BACKEND / the session type), exercises the window-chrome API
//! (title/minimize/maximize/icon, Phase 11 Step 11.2), prints every event
//! (window lifecycle and pointer/keyboard input), and exits on close or
//! after a frame budget.

use tre_platform::{InputEvent, PlatformConnection, WindowIcon};

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

    connection
        .set_title(window, "tre platform smoke test (retitled)")
        .expect("set_title on a real window should succeed");
    eprintln!("title changed after creation");

    // set_maximized/set_minimized only send a request -- the compositor's
    // own confirmation arrives asynchronously, via a real round trip
    // poll_events() later processes (found by real testing: an
    // is_maximized() called immediately after set_maximized(true), with
    // no intervening poll_events(), still reported false). A few polls
    // with a short real sleep between them gives the compositor time to
    // actually respond before we check.
    fn poll_until_settled(connection: &mut PlatformConnection) {
        for _ in 0..5 {
            let _ = connection.poll_events();
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    }

    connection
        .set_maximized(window, true)
        .expect("set_maximized on a real window should succeed");
    poll_until_settled(&mut connection);
    eprintln!(
        "maximized requested, settled: is_maximized = {}",
        connection.is_maximized(window)
    );
    connection
        .set_maximized(window, false)
        .expect("set_maximized on a real window should succeed");
    poll_until_settled(&mut connection);
    eprintln!(
        "restore requested, settled: is_maximized = {}",
        connection.is_maximized(window)
    );

    connection
        .set_minimized(window, true)
        .expect("set_minimized on a real window should succeed");
    poll_until_settled(&mut connection);
    eprintln!(
        "minimized requested, settled: is_minimized = {:?}",
        connection.is_minimized(window)
    );
    connection
        .set_minimized(window, false)
        .expect("set_minimized on a real window should succeed");
    eprintln!("un-minimize requested (a no-op on Wayland, by protocol design)");

    // A tiny 2x2 red/green/blue/white RGBA icon -- just enough real pixel
    // data to exercise Icon::from_rgba's validation, not a real asset.
    let icon = WindowIcon {
        rgba: vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ],
        width: 2,
        height: 2,
    };
    connection
        .set_icon(window, Some(icon))
        .expect("a validly-sized icon should be accepted (Wayland accepts and no-ops it)");
    eprintln!("icon set (X11: visible in the WM's own chrome; Wayland: accepted, no-op)");

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
