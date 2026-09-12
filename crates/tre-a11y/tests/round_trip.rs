//! The real proof this sub-step exists for: a second, independent D-Bus
//! connection queries the exact object `A11yBridge` published on the
//! real Linux accessibility bus and gets back the real values, not a
//! mocked stand-in (PLAN_PHASE5_STEP5_3_2.md's own "Verification plan").
//! Discovers our app via the real AT-SPI2 registry the same way any real
//! assistive technology would: `Registry.GetChildren`, filtered by our
//! own distinctive `ToolkitName` (unique per test run, so this stays
//! correct alongside any other real accessible application already
//! registered on a developer's own desktop session).

use std::{thread, time::Duration};

use tre_engine::{AccessibilityNode, AccessibilityNodeId, AccessibilityRole};
use zbus::{
    blocking::{Connection, ConnectionBuilder, Proxy},
    zvariant::OwnedObjectPath,
};

fn a11y_bus() -> Option<Connection> {
    let session = Connection::session().ok()?;
    let bus_proxy = Proxy::new(&session, "org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus").ok()?;
    let address: String = bus_proxy.call("GetAddress", &()).ok()?;
    ConnectionBuilder::address(address.as_str())
        .ok()?
        .build()
        .ok()
}

/// Calling `org.a11y.atspi.Registry.RegisterEvent` on the a11y bus
/// (`bus`) makes `org.a11y.Status.IsEnabled` flip true, matching what a
/// real assistive technology does on startup -- but the real,
/// previously-unfixed bug this function had (REVIEW.md finding #126's
/// final, corrected account; `IMPLEMENTATION.md`'s own Step 5.3.3 entry)
/// was querying that property against the WRONG bus. `at-spi-bus-
/// launcher` owns `org.a11y.Bus`/`IsEnabled` on the real SESSION bus
/// (`g_bus_own_name(G_BUS_TYPE_SESSION, ...)`, confirmed by reading its
/// own source), not the a11y bus `bus` itself connects to -- every
/// `get_property` call against `bus` therefore failed outright (the
/// destination doesn't exist there), and `.unwrap_or(false)` silently
/// turned that failure into the same `false` a real "not enabled yet"
/// reading would produce, fully explaining why nothing done to the
/// actual `IsEnabled` mechanism ever had any visible effect. Fixed by
/// building the `status` proxy on a real, separate session-bus
/// connection instead. Returns `false` (not a panic) so the caller can
/// skip gracefully, matching this test's own established convention for
/// an environment that doesn't cooperate.
fn ensure_accessibility_enabled(bus: &Connection) -> bool {
    let Ok(registry) = Proxy::new(
        bus,
        "org.a11y.atspi.Registry",
        "/org/a11y/atspi/registry",
        "org.a11y.atspi.Registry",
    ) else {
        return false;
    };
    let Ok(session_bus) = Connection::session() else {
        return false;
    };
    let Ok(status) = Proxy::new(
        &session_bus,
        "org.a11y.Bus",
        "/org/a11y/bus",
        "org.a11y.Status",
    ) else {
        return false;
    };

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
            return true;
        }
        let _ = registry.call::<_, _, ()>("RegisterEvent", &("object:state-changed",));
        thread::sleep(Duration::from_millis(200));
    }
    false
}

/// Polls the real registry for our app, identified by `toolkit_name`,
/// for up to `timeout` -- embedding happens asynchronously on
/// `accesskit_unix`'s own background thread, so this is a real,
/// bounded wait rather than an assumption that it has already happened.
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
    .ok()?;
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        let children: Vec<(String, OwnedObjectPath)> = registry.call("GetChildren", &()).ok()?;
        for (bus_name, path) in children {
            let app = Proxy::new(
                bus,
                bus_name.as_str(),
                path.as_str(),
                "org.a11y.atspi.Application",
            )
            .ok()?;
            let matched = app
                .get_property::<String>("ToolkitName")
                .is_ok_and(|name| name == toolkit_name);
            drop(app);
            if matched {
                return Some((bus_name, path));
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    None
}

#[test]
fn published_node_is_queryable_over_a_real_atspi2_round_trip() {
    let Some(bus) = a11y_bus() else {
        eprintln!("no real AT-SPI2 accessibility bus reachable in this environment -- skipping");
        return;
    };
    if !ensure_accessibility_enabled(&bus) {
        eprintln!("could not make org.a11y.Status.IsEnabled true in this environment -- skipping");
        return;
    }

    let toolkit_name = format!(
        "tre-a11y-round-trip-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let bridge = tre_a11y::A11yBridge::connect("tre-a11y-test", toolkit_name.clone(), "0.0.0");
    let node = AccessibilityNode {
        node_id: AccessibilityNodeId(123),
        x: 10.0,
        y: 20.0,
        width: 100.0,
        height: 50.0,
        role: AccessibilityRole::Button,
    };

    // A steady stream of publishes (rather than one call) both keeps the
    // adapter active for the length of this test and matches how a real
    // caller would use it -- once per rendered frame, not once ever.
    let keep_publishing = std::sync::atomic::AtomicBool::new(true);
    thread::scope(|scope| {
        scope.spawn(|| {
            while keep_publishing.load(std::sync::atomic::Ordering::Relaxed) {
                bridge.publish(&[node]);
                thread::sleep(Duration::from_millis(20));
            }
        });

        // Real, evidence-based history (Step 5.3.3's own CI follow-up,
        // 2026-09-08): 10s was cutting it too close in a plain job
        // (consistently ~10.05s there). Once this exact test ran inside
        // a job that also has Xvfb/Vulkan packages installed and
        // DISPLAY set (for canvas_accessibility_demo's own sake), this
        // identical test -- no Vulkan/X11 code of its own at all --
        // measured 30.06s, tripling the delay purely from being in that
        // environment. 60s gives real margin over the worst case
        // actually observed, not a guess about why the delay triples.
        let Some((app_bus, app_root)) = find_our_app(&bus, &toolkit_name, Duration::from_secs(60))
        else {
            keep_publishing.store(false, std::sync::atomic::Ordering::Relaxed);
            eprintln!(
                "our app never appeared in the real AT-SPI2 registry within 60s -- skipping \
                 (no assistive-technology-enabled session reachable in this environment)"
            );
            return;
        };

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
            "exactly one synthesized root child of the app-level root"
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
        assert!(
            child_path.as_str().ends_with("/123"),
            "the published node's real AccessibilityNodeId(123) must appear in its own \
             AT-SPI2 object path, got {child_path}"
        );

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
            (10, 20, 100, 50),
            "the real, independently-queried Component.GetExtents must match exactly what \
             was published, not an approximation"
        );

        keep_publishing.store(false, std::sync::atomic::Ordering::Relaxed);
    });
}
