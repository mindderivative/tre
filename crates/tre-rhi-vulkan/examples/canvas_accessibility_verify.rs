//! The second half of Step 5.3.3's capstone (see `canvas_accessibility_demo`'s
//! own top-level doc comment for the full "why a separate binary"
//! account, REVIEW.md finding #126). This binary is **genuinely
//! Vulkan/X11-free** -- it imports only `tre_engine` (for the
//! `AccessibilityNode` types, no heavier than plain structs), `tre_a11y`,
//! and `zbus`, none of which pull in `ash`/`x11rb`/`raw-window-handle`.
//! It reads the tagged nodes `canvas_accessibility_demo` wrote out,
//! publishes them via a real `tre_a11y::A11yBridge` (the real AT-SPI2
//! registration happens in *this* process, deliberately, since
//! `accesskit_unix`'s own background thread could not complete it
//! inside a process that also links real Vulkan/X11 libraries), and
//! then acts as its own second, independent AT-SPI2 client -- exactly
//! `tre-a11y`'s own round-trip test's proven-reliable shape, extended
//! to externally-supplied, real-render-derived data instead of one
//! synthetic literal.

use std::{thread, time::Duration};

use tre_engine::{AccessibilityNode, AccessibilityNodeId, AccessibilityRole};
use zbus::{
    blocking::{Connection, ConnectionBuilder, Proxy},
    zvariant::OwnedObjectPath,
};

/// Real, upstream-confirmed root cause (REVIEW.md finding #126's final
/// account, after nine real CI pushes and two disproven hypotheses):
/// `org.a11y.Status.IsEnabled` is not a simple flag an application can
/// set -- reading `at-spi-bus-launcher.c`'s own real source
/// (`on_event_listener_registered`) shows it flips true only when
/// `at-spi2-registryd` emits a real `EventListenerRegistered` D-Bus
/// signal, which only happens when some real client calls
/// `org.a11y.atspi.Registry.RegisterEvent`. A real desktop session
/// already has some component that has done this at some point (a
/// screen reader, an accessibility-aware background service); a fresh
/// CI container has nothing that ever does, so `IsEnabled` never flips
/// and `accesskit_unix`'s own adapter -- which only activates upon
/// observing that transition -- waits forever, regardless of timeout
/// length. This is exactly what a real assistive technology does on
/// startup, so registering here is not a workaround -- it is this
/// binary honestly playing the AT role it already occupies by querying
/// the tree at all.
fn ensure_accessibility_enabled(bus: &Connection) {
    let registry = Proxy::new(
        bus,
        "org.a11y.atspi.Registry",
        "/org/a11y/atspi/registry",
        "org.a11y.atspi.Registry",
    )
    .expect("failed to build registry event proxy");
    let status = Proxy::new(bus, "org.a11y.Bus", "/org/a11y/bus", "org.a11y.Status")
        .expect("failed to build status proxy");

    // Real evidence (2026-09-08): calling RegisterEvent exactly once,
    // then passively waiting, left IsEnabled false for the full 30s in
    // real CI runs -- twice, at two different wait lengths. Reading
    // `registryd`'s own real source (`impl_RegisterEvent`) shows why a
    // single call is not reliable: `EventListenerRegistered` is a plain
    // D-Bus *signal* (`dbus_connection_send`, fire-and-forget), not a
    // stored or replayed event -- if `at-spi-bus-launcher`'s own
    // subscription to it isn't active yet at the exact moment we call,
    // the signal is simply lost forever, and no amount of passively
    // waiting afterward can recover it. Retrying the real call itself,
    // not just the property check, guarantees some attempt eventually
    // lands after that subscription is active.
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        if status.get_property::<bool>("IsEnabled").unwrap_or(false) {
            return;
        }
        let _ = registry.call::<_, _, ()>("RegisterEvent", &("object:state-changed",));
        thread::sleep(Duration::from_millis(200));
    }
    panic!(
        "org.a11y.Status.IsEnabled never became true within 30s of repeated RegisterEvent calls"
    );
}

fn a11y_bus() -> Connection {
    let session = Connection::session().expect("failed to connect to the D-Bus session bus");
    let bus_proxy = Proxy::new(&session, "org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus")
        .expect("failed to build org.a11y.Bus proxy");
    let address: String = bus_proxy
        .call("GetAddress", &())
        .expect("org.a11y.Bus.GetAddress failed -- is a real AT-SPI2 bus reachable?");
    ConnectionBuilder::address(address.as_str())
        .expect("invalid a11y bus address")
        .build()
        .expect("failed to connect to the real a11y bus")
}

/// Polls the real registry for our app, identified by `toolkit_name`,
/// for up to `timeout` -- embedding happens asynchronously on
/// `accesskit_unix`'s own background thread (Step 5.3.2), so this is a
/// real, bounded wait rather than an assumption it already happened.
fn find_our_app(
    bus: &Connection,
    toolkit_name: &str,
    timeout: Duration,
) -> Option<(String, OwnedObjectPath)> {
    let registry = Proxy::new(
        bus,
        "org.a11y.atspi.Registry",
        "/org/a11y/atspi/accessible/root",
        "org.a11y.atspi.Accessible",
    )
    .expect("failed to build registry proxy");
    let deadline = std::time::Instant::now() + timeout;
    let mut last_seen: Vec<String> = Vec::new();
    while std::time::Instant::now() < deadline {
        let children: Vec<(String, OwnedObjectPath)> = registry
            .call("GetChildren", &())
            .expect("Registry.GetChildren failed");
        last_seen.clear();
        for (bus_name, path) in children {
            let app = Proxy::new(
                bus,
                bus_name.as_str(),
                path.as_str(),
                "org.a11y.atspi.Application",
            )
            .expect("failed to build application proxy");
            let toolkit_name_seen = app.get_property::<String>("ToolkitName").ok();
            let matched = toolkit_name_seen.as_deref() == Some(toolkit_name);
            last_seen.push(format!("{bus_name} -> {toolkit_name_seen:?}"));
            drop(app);
            if matched {
                return Some((bus_name, path));
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    eprintln!(
        "find_our_app timed out looking for toolkit_name={toolkit_name:?}; last registry \
         contents ({} entries): {last_seen:#?}",
        last_seen.len()
    );
    None
}

/// Repeatedly calls `query` until it returns exactly `expected_len`
/// items or `timeout` elapses -- `accesskit_unix`'s per-node D-Bus
/// interface registration is its own separate, asynchronous message on
/// its background thread, so a tree with more than one tagged node can
/// be legitimately observed mid-registration.
fn poll_children(
    query: impl Fn() -> Vec<(String, OwnedObjectPath)>,
    expected_len: usize,
    timeout: Duration,
) -> Vec<(String, OwnedObjectPath)> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let children = query();
        if children.len() == expected_len || std::time::Instant::now() >= deadline {
            return children;
        }
        thread::sleep(Duration::from_millis(50));
    }
}

/// Finds the tagged child whose own AT-SPI2 object path ends in
/// `/{node_id}` -- the real path scheme `accesskit_unix` uses.
fn find_child_by_node_id(
    children: &[(String, OwnedObjectPath)],
    node_id: AccessibilityNodeId,
) -> &(String, OwnedObjectPath) {
    let suffix = format!("/{}", node_id.0);
    children
        .iter()
        .find(|(_, path)| path.as_str().ends_with(&suffix))
        .unwrap_or_else(|| {
            panic!(
                "no tagged child with node id {} found among {children:?}",
                node_id.0
            )
        })
}

fn expected_atspi_extents(node: &AccessibilityNode) -> (i32, i32, i32, i32) {
    // Reproduces accesskit_atspi_common::Rect's own real conversion
    // exactly (`x0 as i32`, `(x1 - x0) as i32`, truncating).
    let x0 = f64::from(node.x);
    let y0 = f64::from(node.y);
    let x1 = f64::from(node.x + node.width);
    let y1 = f64::from(node.y + node.height);
    #[allow(
        clippy::cast_possible_truncation,
        reason = "this canvas's coordinates are small and well within i32 range"
    )]
    {
        (x0 as i32, y0 as i32, (x1 - x0) as i32, (y1 - y0) as i32)
    }
}

/// Parses `canvas_accessibility_demo`'s own plain-text handoff format:
/// one line per node, `id x y width height role`.
fn parse_nodes(text: &str) -> Vec<AccessibilityNode> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(fields.len(), 6, "malformed handoff line: {line:?}");
            let role = match fields[5] {
                "Generic" => AccessibilityRole::Generic,
                "Button" => AccessibilityRole::Button,
                "TextLabel" => AccessibilityRole::TextLabel,
                "Image" => AccessibilityRole::Image,
                other => panic!("unknown role {other:?} in handoff line: {line:?}"),
            };
            AccessibilityNode {
                node_id: AccessibilityNodeId(fields[0].parse().expect("bad node id")),
                x: fields[1].parse().expect("bad x"),
                y: fields[2].parse().expect("bad y"),
                width: fields[3].parse().expect("bad width"),
                height: fields[4].parse().expect("bad height"),
                role,
            }
        })
        .collect()
}

fn main() {
    let nodes_path = std::env::var("TRE_CANVAS_ACCESSIBILITY_NODES_PATH")
        .unwrap_or_else(|_| "canvas_accessibility_nodes.txt".to_string());
    let nodes_text = std::fs::read_to_string(&nodes_path).unwrap_or_else(|error| {
        panic!("failed to read {nodes_path:?} -- run canvas_accessibility_demo first: {error}")
    });
    let nodes = parse_nodes(&nodes_text);
    assert_eq!(nodes.len(), 3, "expected exactly 3 handed-off nodes");
    eprintln!(
        "read {} tagged accessibility nodes from {nodes_path}",
        nodes.len()
    );

    let toolkit_name =
        std::env::var("TRE_CANVAS_ACCESSIBILITY_TOOLKIT_NAME").unwrap_or_else(|_| {
            format!(
                "tre-canvas-accessibility-demo-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )
        });

    // Connect our OWN verifying client to the real a11y bus before
    // A11yBridge::connect below triggers accesskit_unix's own internal
    // background thread to open its own session/a11y-bus connection --
    // matching tre-a11y's own round-trip test's exact call order.
    let bus = a11y_bus();
    // Must happen before A11yBridge::connect, and must confirm
    // IsEnabled is already true before returning: accesskit_unix's own
    // adapter only activates on *observing* IsEnabled transition to
    // true, so starting it only after this function confirms the
    // property already reads true avoids any risk of it missing a
    // transition that already happened.
    ensure_accessibility_enabled(&bus);
    let bridge = tre_a11y::A11yBridge::connect(
        "tre-canvas-accessibility-demo",
        toolkit_name.clone(),
        "0.0.0",
    );
    let keep_publishing = std::sync::atomic::AtomicBool::new(true);

    // Ensures the publisher thread's loop always sees `false`, whether
    // the verification below succeeds, fails an assertion, or panics
    // for any other reason -- `thread::scope` joins every spawned
    // thread before it returns, even while unwinding.
    struct StopOnDrop<'a>(&'a std::sync::atomic::AtomicBool);
    impl Drop for StopOnDrop<'_> {
        fn drop(&mut self) {
            self.0.store(false, std::sync::atomic::Ordering::Relaxed);
        }
    }

    thread::scope(|scope| {
        scope.spawn(|| {
            while keep_publishing.load(std::sync::atomic::Ordering::Relaxed) {
                bridge.publish(&nodes);
                thread::sleep(Duration::from_millis(20));
            }
        });
        let _stop_guard = StopOnDrop(&keep_publishing);

        let (app_bus, app_root) = find_our_app(&bus, &toolkit_name, Duration::from_secs(30))
            .expect("our app never appeared in the real AT-SPI2 registry within 30s");

        let app_accessible = Proxy::new(
            &bus,
            app_bus.as_str(),
            app_root.as_str(),
            "org.a11y.atspi.Accessible",
        )
        .expect("failed to build app-root accessible proxy");
        let synthesized_root = poll_children(
            || {
                app_accessible
                    .call("GetChildren", &())
                    .expect("GetChildren on the app root failed")
            },
            1,
            Duration::from_secs(5),
        );
        assert_eq!(
            synthesized_root.len(),
            1,
            "exactly one synthesized root child"
        );
        let (root_bus, root_path) = &synthesized_root[0];

        let root_accessible = Proxy::new(
            &bus,
            root_bus.as_str(),
            root_path.as_str(),
            "org.a11y.atspi.Accessible",
        )
        .expect("failed to build synthesized-root accessible proxy");
        let tagged_children = poll_children(
            || {
                root_accessible
                    .call("GetChildren", &())
                    .expect("GetChildren on the synthesized root failed")
            },
            nodes.len(),
            Duration::from_secs(5),
        );
        assert_eq!(
            tagged_children.len(),
            nodes.len(),
            "exactly {} tagged nodes were published",
            nodes.len()
        );

        let mut roles = Vec::new();
        for node in &nodes {
            let (child_bus, child_path) = find_child_by_node_id(&tagged_children, node.node_id);
            let expected = expected_atspi_extents(node);

            let component = Proxy::new(
                &bus,
                child_bus.as_str(),
                child_path.as_str(),
                "org.a11y.atspi.Component",
            )
            .expect("failed to build component proxy");
            let extents: (i32, i32, i32, i32) = component
                .call("GetExtents", &(0u32,))
                .expect("Component.GetExtents failed");
            assert_eq!(
                extents, expected,
                "node {}'s real AT-SPI2 GetExtents must match its own real IR bounds exactly",
                node.node_id.0
            );

            let accessible = Proxy::new(
                &bus,
                child_bus.as_str(),
                child_path.as_str(),
                "org.a11y.atspi.Accessible",
            )
            .expect("failed to build accessible proxy");
            let role: u32 = accessible
                .call("GetRole", &())
                .expect("Accessible.GetRole failed");
            roles.push(role);
            eprintln!(
                "  node {}: real AT-SPI2 GetExtents = {extents:?}, GetRole = {role} -- OK",
                node.node_id.0
            );
        }
        assert!(
            roles.iter().enumerate().all(|(i, role)| roles
                .iter()
                .enumerate()
                .all(|(j, other)| i == j || role != other)),
            "each node's distinct AccessibilityRole must survive as a distinct real AT-SPI2 role, \
             got {roles:?}"
        );
    });

    eprintln!(
        "accessibility level: all {} tagged nodes' real AT-SPI2 bounds match the IR exactly, \
         and each node's role stayed distinct end to end -- OK",
        nodes.len()
    );
    eprintln!(
        "all canvas accessibility verification assertions passed -- Step 5.3 (5.3.1-5.3.3) \
         closed in full"
    );
}
