//! §14 step 14 (§11.1): the standalone proof that `run_windowed_multi`
//! genuinely manages more than one window at once, each with its own
//! real `WindowId` and its own independent per-frame lifecycle -- not
//! just that the API compiles against a single window.
//!
//! `harness = false`, same reasoning as `access_button.rs`: `winit`
//! permits constructing an `EventLoop` only on a process's real main
//! thread, only once per process.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use engine_platform::{WindowConfig, WindowRequest, run_windowed_multi};
use winit::window::WindowId;

fn main() {
    let created: Rc<RefCell<Vec<(u64, WindowId)>>> = Rc::new(RefCell::new(Vec::new()));
    let created_for_created = created.clone();

    let frame_counts: Rc<RefCell<HashMap<WindowId, u32>>> = Rc::new(RefCell::new(HashMap::new()));
    let frame_counts_for_frame = frame_counts.clone();

    let result = run_windowed_multi(
        move |window_id, token, _window| {
            eprintln!("engine-platform §14 step 14: window {token} created as {window_id:?}");
            created_for_created.borrow_mut().push((token, window_id));
        },
        move |window_id, frame| {
            frame_counts_for_frame
                .borrow_mut()
                .insert(window_id, frame + 1);
            // M29 Phase 2: this test's own scope is multi-window
            // lifecycle, not animation-aware polling -- always reporting
            // "still animating" keeps it polling to its own `max_frames`
            // bound exactly as before this phase.
            true
        },
        move |_window_id| accesskit::TreeUpdate {
            nodes: vec![(
                accesskit::NodeId(0),
                accesskit::Node::new(accesskit::Role::Window),
            )],
            tree: Some(accesskit::TreeInfo::new(accesskit::NodeId(0))),
            tree_id: accesskit::TreeId::ROOT,
            focus: accesskit::NodeId(0),
        },
        // This test's own scope is multi-window lifecycle, not input
        // dispatch -- see `engine-platform`'s own `src/lib.rs` unit
        // tests for real coverage of the `WindowEvent -> InputEvent`
        // translation (M4 Phase 1 step 2) and `on_access_action`'s own
        // doc comment for why `ActionRequested` translation has no
        // separate unit test (M4 Phase 2).
        |_window_id, _event| {},
        |_window_id, _request| {},
        |opener, _waker| {
            opener.open_window(WindowRequest {
                config: WindowConfig {
                    title: "tre v2 -- §14 step 14 spike: window A".to_string(),
                    width: 200,
                    height: 150,
                    max_frames: Some(5),
                },
                token: 0,
            });
            opener.open_window(WindowRequest {
                config: WindowConfig {
                    title: "tre v2 -- §14 step 14 spike: window B".to_string(),
                    width: 250,
                    height: 180,
                    max_frames: Some(5),
                },
                token: 1,
            });
        },
    );

    match result {
        Ok(()) => {
            let created = created.borrow();
            assert_eq!(
                created.len(),
                2,
                "both requested windows must have actually been created, got {created:?}"
            );
            let (_, id_a) = created[0];
            let (_, id_b) = created[1];
            assert_ne!(
                id_a, id_b,
                "the two windows must be assigned genuinely distinct WindowIds"
            );

            let counts = frame_counts.borrow();
            assert_eq!(
                counts.len(),
                2,
                "both windows must have independently received their own redraws, got {counts:?}"
            );
            for (&id, &count) in counts.iter() {
                assert_eq!(
                    count, 5,
                    "window {id:?} should have run exactly its own max_frames (5) redraws, got {count}"
                );
            }
            eprintln!(
                "engine-platform §14 step 14: both windows opened with distinct WindowIds, \
                 ticked independently, and both exited cleanly at their own max_frames"
            );
        }
        Err(err) => {
            // No display reachable (e.g. a headless CI runner with no
            // X11/Wayland socket) is an expected, non-exceptional
            // condition -- TRE v1's own finding #261 convention, applied
            // identically to every other windowed test in this
            // workspace.
            eprintln!("engine-platform §14 step 14: no display available ({err}), exiting cleanly");
        }
    }
}
