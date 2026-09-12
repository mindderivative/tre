//! Started life as Phase 18 Step 18.2's real, isolated feasibility
//! test's own query-only half (proving, for the first time, that a
//! real Vulkan-linked process can publish its own AT-SPI2 tree and be
//! independently queried) -- kept on as a real, permanent piece of
//! infrastructure once that was confirmed: `demo/phase18_step18_3/
//! demo.py` (the real `tre.A11yBridge` Python-binding demo) spawns this
//! exact binary as a genuinely separate OS process to act as the real
//! AT-SPI2 client an actual assistive technology would be, while the
//! Python process keeps publishing on its own main thread. Queries the
//! live registry for `TRE_A11Y_SINGLE_PROCESS_TOOLKIT_NAME`'s own
//! toolkit_name and confirms its one tagged node's real
//! `Component.GetExtents` matches the fixed `(10, 10, 60, 40)` rect
//! both this binary's own original Rust counterpart and the Python
//! demo publish. Genuinely does not touch `tre_a11y`/`ash`/`x11rb`
//! itself -- pure `zbus`, reusing `canvas_accessibility_verify.rs`'s
//! own proven query logic verbatim.

use std::time::Duration;

use zbus::{
    blocking::{Connection, ConnectionBuilder, Proxy},
    zvariant::OwnedObjectPath,
};

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
    while std::time::Instant::now() < deadline {
        let children: Vec<(String, OwnedObjectPath)> = registry
            .call("GetChildren", &())
            .expect("Registry.GetChildren failed");
        for (bus_name, path) in children {
            let app = Proxy::new(
                bus,
                bus_name.as_str(),
                path.as_str(),
                "org.a11y.atspi.Application",
            )
            .expect("failed to build application proxy");
            let matched = app
                .get_property::<String>("ToolkitName")
                .is_ok_and(|name| name == toolkit_name);
            drop(app);
            if matched {
                return Some((bus_name, path));
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

fn main() {
    let toolkit_name = std::env::var("TRE_A11Y_SINGLE_PROCESS_TOOLKIT_NAME")
        .unwrap_or_else(|_| "tre-single-process-feasibility-check".to_string());

    let bus = a11y_bus();
    let Some((app_bus, app_root)) = find_our_app(&bus, &toolkit_name, Duration::from_secs(10))
    else {
        panic!(
            "the single-process check's own app (toolkit_name={toolkit_name:?}) never appeared \
             in the real AT-SPI2 registry within 10s -- if this is the ONLY failure, it directly \
             disproves single-process feasibility; check the other process's own stderr first"
        );
    };
    eprintln!("found our app in the real registry: bus={app_bus:?} root={app_root:?}");

    let app_accessible = Proxy::new(
        &bus,
        app_bus.as_str(),
        app_root.as_str(),
        "org.a11y.atspi.Accessible",
    )
    .unwrap();
    let synthesized_root: Vec<(String, OwnedObjectPath)> =
        app_accessible.call("GetChildren", &()).unwrap();
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
    .unwrap();
    let tagged_children: Vec<(String, OwnedObjectPath)> =
        root_accessible.call("GetChildren", &()).unwrap();
    assert_eq!(
        tagged_children.len(),
        1,
        "exactly one tagged node was published"
    );
    let (child_bus, child_path) = &tagged_children[0];

    let component = Proxy::new(
        &bus,
        child_bus.as_str(),
        child_path.as_str(),
        "org.a11y.atspi.Component",
    )
    .unwrap();
    let extents: (i32, i32, i32, i32) = component.call("GetExtents", &(0u32,)).unwrap();
    assert_eq!(
        extents,
        (10, 10, 60, 40),
        "the real, independently-queried Component.GetExtents must match the other process's \
         own real IR exactly"
    );

    eprintln!(
        "SINGLE-PROCESS FEASIBILITY CONFIRMED: a real Vulkan-linked process's own real \
         A11yBridge::publish was independently, correctly queried over live AT-SPI2 by this \
         genuinely separate verifier process -- extents={extents:?}"
    );
}
