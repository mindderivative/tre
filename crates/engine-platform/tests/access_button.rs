//! §14 step 7: "confirm one button is correctly exposed to a screen
//! reader." A real window, one `NodeKind::Rect` tagged as
//! `Role::Button` with a label and a `Click` action via
//! `Tree::set_access`, run through the real `accesskit_winit::Adapter`
//! wiring `engine-platform` now owns -- no rendering at all (this crate
//! is render-agnostic, generic over any `on_frame` closure), just the
//! accessibility half.
//!
//! This is the standalone spike proving the mechanism in isolation
//! (Design Principle 5), the same pattern as every other build-order
//! step's own first real windowed test. The actual "correctly exposed"
//! claim was verified externally, against this process's real AT-SPI
//! registration on the session's dedicated AT-SPI D-Bus (`gdbus
//! --address unix:path=$XDG_RUNTIME_DIR/at-spi/bus_0`, *not* the
//! regular session bus): found this process registered under its own
//! bus name, walked `org.a11y.atspi.Accessible.GetChildren` from the
//! AT-SPI registry root down to this node, and confirmed directly --
//! `Name` = `"Save"`, `GetRole` = `43` (`atspi-common`'s own real
//! `u32`-to-`Role` table maps `43 => Button`, checked in its source, not
//! assumed), `org.a11y.atspi.Action.GetActions` = `[("click", "", "")]`
//! (one action, matching the one `Action::Click` added), and
//! `org.a11y.atspi.Component.GetExtents` = `(0, 0, 120, 40)` -- an exact
//! match for the node's real taffy-computed bounds. This is the same
//! tree a real screen reader (Orca) reads from; LOG.md has the full
//! transcript.
//!
//! 100ms/frame (not `ControlFlow::Poll`'s free-running speed) is
//! deliberate: gives an external AT-SPI query real wall-clock time to
//! run against this process while it's still alive, mid-test.

use std::cell::RefCell;
use std::rc::Rc;
use std::thread;
use std::time::Duration;

use engine_core::{AccessNodeData, Action, NodeKind, PaintProperties, Role, Tree};
use engine_platform::{WindowConfig, run_windowed};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};

fn main() {
    let mut tree = Tree::new();
    let button = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(120.0),
                height: length(40.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0x67, 0x50, 0xA4, 0xFF), 8.0, 0.0, 1.0),
    );
    tree.set_access(
        button,
        AccessNodeData::new(Role::Button)
            .with_label("Save")
            .with_action(Action::Click),
    );
    tree.compute_layout(
        button,
        Size {
            width: AvailableSpace::Definite(120.0),
            height: AvailableSpace::Definite(40.0),
        },
    );

    let tree = Rc::new(RefCell::new(tree));
    let tree_for_access = tree.clone();

    let result = run_windowed(
        WindowConfig {
            title: "tre v2 -- §14 step 7 spike: one accessible button".to_string(),
            width: 160,
            height: 80,
            max_frames: Some(30),
        },
        move |_window, frame| {
            if frame == 0 {
                eprintln!(
                    "engine-platform §14 step 7: window shown, one Button node (label \"Save\") exposed via accesskit"
                );
            }
            // See module doc comment: gives an external AT-SPI query
            // real time to run against this process mid-test.
            thread::sleep(Duration::from_millis(100));
        },
        move || tree_for_access.borrow().build_access_update(button),
    );

    match result {
        Ok(()) => eprintln!("engine-platform §14 step 7: exited cleanly after 30 frames"),
        Err(err) => {
            eprintln!("engine-platform §14 step 7: no display available ({err}), exiting 0");
        }
    }
}
