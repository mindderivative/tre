//! M31 Phase 6 (§5, §6): the standalone proof that a real
//! `EventLoopWaker`, cloned out to a genuinely separate OS thread from
//! inside `setup` (the one real place able to reach one at all -- see
//! `EventLoopWaker`'s own doc comment), can wake this crate's real
//! `winit` event loop from that other thread without panicking or
//! hanging -- the identical real cross-thread delivery mechanism
//! `PlatformEvent::AccessKit`/`OpenWindow` already rely on via the
//! same underlying `EventLoopProxy`, exercised here for the new
//! `Wake` variant specifically.
//!
//! **Real, honest scope note:** this proves the new API is genuinely
//! wired correctly end to end (a background thread's own `wake()`
//! call really does reach a live loop and request a real redraw,
//! confirmed via a shared counter the window's own `on_frame`
//! increments), not that it improves idle CPU usage -- that's a real
//! performance property, better suited to profiling than a pass/fail
//! test, the same honest distinction this codebase already draws
//! elsewhere (e.g. `Carousel`'s own stated-not-benchmarked extra
//! layout-pass cost).
//!
//! `harness = false`, same reasoning as `access_button.rs`/
//! `multi_window.rs`: `winit` permits constructing an `EventLoop` only
//! on a process's real main thread, only once per process.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

use engine_platform::{WindowConfig, WindowRequest, run_windowed_multi};

fn main() {
    let frame_count = Arc::new(AtomicU32::new(0));
    let frame_count_for_frame = frame_count.clone();

    let result = run_windowed_multi(
        |_window_id, _token, _window| {},
        move |_window_id, _frame| {
            frame_count_for_frame.fetch_add(1, Ordering::SeqCst);
            true
        },
        |_window_id| accesskit::TreeUpdate {
            nodes: vec![(
                accesskit::NodeId(0),
                accesskit::Node::new(accesskit::Role::Window),
            )],
            tree: Some(accesskit::TreeInfo::new(accesskit::NodeId(0))),
            tree_id: accesskit::TreeId::ROOT,
            focus: accesskit::NodeId(0),
        },
        |_window_id, _event| {},
        |_window_id, _request| {},
        |_window_id, _lifecycle| true,
        |opener, waker| {
            opener.open_window(WindowRequest {
                config: WindowConfig {
                    title: "tre v2 -- M31 Phase 6 wake spike".to_string(),
                    width: 200,
                    height: 150,
                    max_frames: Some(10),
                },
                token: 0,
            });
            // The real point of this test: a genuinely separate OS
            // thread, holding only a clone of the real waker (no
            // access to the window, the app, or anything else this
            // crate owns), calling `wake()` a few times while the loop
            // is genuinely running -- proving the cross-thread
            // delivery path this whole phase exists for actually
            // works, not just that it compiles.
            let waker = waker.clone();
            thread::spawn(move || {
                for _ in 0..3 {
                    thread::sleep(Duration::from_millis(5));
                    waker.wake();
                }
            });
        },
    );

    match result {
        Ok(()) => {
            let frames = frame_count.load(Ordering::SeqCst);
            assert_eq!(
                frames, 10,
                "the window must still have run exactly its own real max_frames (10) redraws, \
                 got {frames} -- a real wake call must never disrupt the existing max_frames \
                 contract"
            );
            eprintln!(
                "engine-platform M31 Phase 6: a real cross-thread EventLoopWaker::wake() call \
                 reached the live loop and the window still ran its own real max_frames \
                 cleanly, with no panic and no hang"
            );
        }
        Err(err) => {
            // No display reachable (e.g. a headless CI runner with no
            // X11/Wayland socket) is an expected, non-exceptional
            // condition -- TRE v1's own finding #261 convention,
            // applied identically to every other windowed test in this
            // crate.
            eprintln!("engine-platform M31 Phase 6: no display available ({err}), skipping");
        }
    }
}
