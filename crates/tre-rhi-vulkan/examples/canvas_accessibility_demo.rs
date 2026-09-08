//! Phase 5 Step 5.3.3 proof: the capstone closing Step 5.3 in full. A
//! single real render produces data that flows through all three
//! systems DESIGN.md Section 5.2's "$100\%$ alignment" claim spans at
//! once -- the real GPU framebuffer, the tagged `AccessibilityNode` IR,
//! and a real, live AT-SPI2 bus -- checked against each other in one
//! process, one render, rather than three isolated tests that each
//! merely trust the others' contracts (Step 5.3.1's own IR-only unit
//! tests; Step 5.3.2's own D-Bus round trip against a synthetic,
//! never-rendered node).
//!
//! Three rects are both drawn and tagged from the exact same local
//! coordinates: a plain `Generic` rect, a plain `Button` rect, and a
//! **rotated** `Image` rect via `Affine2::from_translation_rotation_scale`
//! -- the one genuinely tricky case Step 5.3.1's own `PLAN.md` singled
//! out, here proven end to end for the first time rather than only at
//! the IR level. No clipping/overlays (Step 5.3.1's own disclosed,
//! still-open clip-stack gap would make a real, already-known limitation
//! look like a new bug) and no multi-threading (already Step 5.2.3's own
//! capstone, and already unit-tested for accessibility nodes specifically
//! in Step 5.3.1) -- this sub-step closes new ground, not already-closed
//! ground.

use std::{thread, time::Duration};

use ash::vk;
use tre_engine::{
    rgba8, AccessibilityNode, AccessibilityNodeId, AccessibilityRole, RenderingCanvas, RhiDevice,
};
use tre_math::Affine2;
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};
use zbus::{
    blocking::{Connection, ConnectionBuilder, Proxy},
    zvariant::OwnedObjectPath,
};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const CANVAS_WIDTH: u32 = 300;
const CANVAS_HEIGHT: u32 = 200;

const RECT_A_ORIGIN: (f32, f32) = (20.0, 20.0); // Generic, plain
const RECT_B_ORIGIN: (f32, f32) = (200.0, 20.0); // Button, plain
const RECT_C_TRANSLATION: [f32; 2] = [100.0, 120.0]; // Image, rotated
const RECT_SIZE: (f32, f32) = (60.0, 40.0);
const RECT_C_ROTATION_DEGREES: f32 = 30.0;

const NODE_A: AccessibilityNodeId = AccessibilityNodeId(1);
const NODE_B: AccessibilityNodeId = AccessibilityNodeId(2);
const NODE_C: AccessibilityNodeId = AccessibilityNodeId(3);

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
        thread::sleep(Duration::from_millis(50));
    }
    None
}

/// Repeatedly calls `query` (a real `GetChildren`, not a cached value)
/// until it returns exactly `expected_len` items or `timeout` elapses.
/// A single one-shot query right after the app is first found is not
/// enough: `accesskit_unix`'s per-node D-Bus interface registration
/// (`Message::RegisterInterfaces`) is its own separate, asynchronous
/// message on its background thread, so a tree with more than one
/// tagged node can be legitimately observed mid-registration -- a real
/// race neither Step 5.3.1 (no AT-SPI2 at all) nor Step 5.3.2's own
/// round-trip test (exactly one node, so this race could never surface)
/// ever exercised.
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
/// `/{node_id}` -- the real path scheme `accesskit_unix` uses
/// (`.../accessible/<adapter>/<node_id>`), so this identifies a specific
/// tagged node without relying on `GetChildren`'s return order.
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
    // exactly (`x0 as i32`, `(x1 - x0) as i32`, truncating -- not
    // rounding, and not independently truncating x0/x1 then
    // subtracting) so this check proves the real publish-and-query path
    // preserves the IR's own values exactly, rather than comparing
    // against a redundant, potentially-off-by-a-float-epsilon
    // reimplementation of Step 5.3.1's own rotation math.
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

fn main() {
    // --- Real device/pipeline, matching sdf_rounded_rect_demo's own
    // minimal single-pipeline setup exactly -- no atlas/text needed ---
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre canvas accessibility probe (never shown)", 1, 1)
        .expect("failed to open probe window");
    use raw_window_handle::HasDisplayHandle;
    let display_handle = probe_connection.display_handle().unwrap().as_raw();
    let window_handle = probe_connection
        .window_handle(probe_window)
        .unwrap()
        .as_raw();
    let (device, surface_loader, surface) =
        VulkanDevice::new(display_handle, window_handle).expect("failed to create VulkanDevice");
    unsafe {
        surface_loader.destroy_surface(surface, None);
    }
    let swapchain = HeadlessSwapchain::new(&device, CANVAS_WIDTH, CANVAS_HEIGHT)
        .expect("failed to create HeadlessSwapchain");

    let out_dir = env!("OUT_DIR");
    let vertex_spv = std::fs::read(format!("{out_dir}/sdf_rounded_rect.vert.spv"))
        .expect("failed to read compiled vertex shader");
    let fragment_spv = std::fs::read(format!("{out_dir}/sdf_rounded_rect.frag.spv"))
        .expect("failed to read compiled fragment shader");
    let pipeline = device
        .create_pipeline(&vertex_spv, &fragment_spv, tre_rhi_vulkan::HEADLESS_FORMAT)
        .expect("failed to create pipeline");

    // --- The scene: each rect is drawn and tagged from the exact same
    // local coordinates, one shared source of truth for both ---
    let white = rgba8(255, 255, 255, 255);
    let mut canvas = RenderingCanvas::new();

    canvas.draw_rounded_rect(
        RECT_A_ORIGIN.0,
        RECT_A_ORIGIN.1,
        RECT_SIZE.0,
        RECT_SIZE.1,
        0.0,
        white,
    );
    canvas.tag_accessibility_node(
        NODE_A,
        RECT_A_ORIGIN.0,
        RECT_A_ORIGIN.1,
        RECT_SIZE.0,
        RECT_SIZE.1,
        AccessibilityRole::Generic,
    );

    canvas.draw_rounded_rect(
        RECT_B_ORIGIN.0,
        RECT_B_ORIGIN.1,
        RECT_SIZE.0,
        RECT_SIZE.1,
        0.0,
        white,
    );
    canvas.tag_accessibility_node(
        NODE_B,
        RECT_B_ORIGIN.0,
        RECT_B_ORIGIN.1,
        RECT_SIZE.0,
        RECT_SIZE.1,
        AccessibilityRole::Button,
    );

    let rect_c_transform = Affine2::from_translation_rotation_scale(
        RECT_C_TRANSLATION,
        RECT_C_ROTATION_DEGREES.to_radians(),
        [1.0, 1.0],
    );
    canvas.save();
    canvas.transform(&rect_c_transform);
    canvas.draw_rounded_rect(0.0, 0.0, RECT_SIZE.0, RECT_SIZE.1, 0.0, white);
    canvas.tag_accessibility_node(
        NODE_C,
        0.0,
        0.0,
        RECT_SIZE.0,
        RECT_SIZE.1,
        AccessibilityRole::Image,
    );
    canvas.restore();

    let frame = canvas.flatten();
    assert_eq!(
        frame.accessibility_nodes.len(),
        3,
        "exactly 3 tagged nodes were recorded"
    );

    // --- Real GPU render ---
    let vertex_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&frame.vertices),
            vk::BufferUsageFlags::VERTEX_BUFFER,
        )
        .expect("failed to upload vertex buffer");
    let index_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&frame.indices),
            vk::BufferUsageFlags::INDEX_BUFFER,
        )
        .expect("failed to upload index buffer");

    let (mut cmd_buffer, image) = device.begin_frame(&swapchain).expect("begin_frame failed");
    cmd_buffer.set_pipeline(&pipeline);
    cmd_buffer.bind_vertex_buffer(&vertex_buffer, 0);
    cmd_buffer.bind_index_buffer(&index_buffer, 0);
    #[allow(
        clippy::cast_possible_truncation,
        reason = "this demo's index count is far below u32::MAX"
    )]
    cmd_buffer.draw_indexed(frame.indices.len() as u32, 0, 0);
    device
        .submit_and_present(cmd_buffer, &swapchain, image)
        .expect("submit_and_present failed");

    let bgra = swapchain
        .read_pixels_bgra8()
        .expect("failed to read back pixels");
    let pixel_at =
        |x: u32, y: u32| -> [u8; 4] { pixel_helpers::bgra_pixel_at(&bgra, CANVAS_WIDTH, x, y) };
    let background = pixel_at(0, 0);

    // --- Pixel verification: rect A/B's own plain centers, and rect
    // C's real transformed center -- computed via the exact same
    // Affine2 used to draw and tag it, not an independently-guessed
    // coordinate ---
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "this canvas's coordinates are fixed, well within [0, CANVAS_WIDTH/HEIGHT)"
    )]
    let to_pixel = |p: [f32; 2]| -> (u32, u32) { (p[0].round() as u32, p[1].round() as u32) };

    let (ax, ay) = to_pixel([
        RECT_A_ORIGIN.0 + RECT_SIZE.0 / 2.0,
        RECT_A_ORIGIN.1 + RECT_SIZE.1 / 2.0,
    ]);
    assert_eq!(
        pixel_at(ax, ay),
        [255, 255, 255, 255],
        "Rect A must render at its own center"
    );

    let (bx, by) = to_pixel([
        RECT_B_ORIGIN.0 + RECT_SIZE.0 / 2.0,
        RECT_B_ORIGIN.1 + RECT_SIZE.1 / 2.0,
    ]);
    assert_eq!(
        pixel_at(bx, by),
        [255, 255, 255, 255],
        "Rect B must render at its own center"
    );

    let rect_c_center_world =
        rect_c_transform.transform_point([RECT_SIZE.0 / 2.0, RECT_SIZE.1 / 2.0]);
    let (cx, cy) = to_pixel(rect_c_center_world);
    assert_eq!(
        pixel_at(cx, cy),
        [255, 255, 255, 255],
        "rotated Rect C must render at its own real transformed center ({cx}, {cy})"
    );

    let (gap_x, gap_y) = to_pixel([
        (RECT_A_ORIGIN.0 + RECT_SIZE.0 + RECT_B_ORIGIN.0) / 2.0,
        RECT_A_ORIGIN.1 + RECT_SIZE.1 / 2.0,
    ]);
    assert_eq!(
        pixel_at(gap_x, gap_y),
        background,
        "the gap between Rect A and Rect B must stay background"
    );
    eprintln!(
        "pixel level: all 3 rects (including rotated Rect C) rendered at their real transformed \
         positions -- OK"
    );

    // --- Real accessibility publish, real AT-SPI2 verification ---
    let toolkit_name = format!(
        "tre-canvas-accessibility-demo-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let bridge = tre_a11y::A11yBridge::connect(
        "tre-canvas-accessibility-demo",
        toolkit_name.clone(),
        "0.0.0",
    );
    let nodes = frame.accessibility_nodes.clone();
    let keep_publishing = std::sync::atomic::AtomicBool::new(true);

    // Ensures the publisher thread's loop always sees `false`, whether
    // the verification below succeeds, fails an assertion, or panics
    // for any other reason -- `thread::scope` joins every spawned thread
    // before it returns, even while unwinding, so without this an
    // assertion failure inside the scope would hang forever instead of
    // reporting the real failure.
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

        let bus = a11y_bus();
        let (app_bus, app_root) = find_our_app(&bus, &toolkit_name, Duration::from_secs(10))
            .expect("our app never appeared in the real AT-SPI2 registry within 10s");

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
            3,
            Duration::from_secs(5),
        );
        assert_eq!(
            tagged_children.len(),
            3,
            "exactly 3 tagged nodes were published"
        );

        let mut roles = Vec::new();
        for (label, node_id) in [
            ("A (Generic)", NODE_A),
            ("B (Button)", NODE_B),
            ("C (Image, rotated)", NODE_C),
        ] {
            let (child_bus, child_path) = find_child_by_node_id(&tagged_children, node_id);
            let node = frame
                .accessibility_nodes
                .iter()
                .find(|n| n.node_id == node_id)
                .expect("tagged node must be in the IR");
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
                "rect {label}'s real AT-SPI2 GetExtents must match its own real IR bounds exactly"
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
                "  rect {label}: real AT-SPI2 GetExtents = {extents:?}, GetRole = {role} -- OK"
            );
        }
        assert!(
            roles[0] != roles[1] && roles[0] != roles[2] && roles[1] != roles[2],
            "each rect's distinct AccessibilityRole must survive as a distinct real AT-SPI2 role, \
             got {roles:?}"
        );
    });
    eprintln!(
        "accessibility level: all 3 tagged nodes' real AT-SPI2 bounds match the IR exactly, and \
         each rect's role stayed distinct end to end -- OK"
    );

    let mut rgba_out = bgra.clone();
    for px in rgba_out.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_CANVAS_ACCESSIBILITY_OUTPUT")
        .unwrap_or_else(|_| "canvas_accessibility_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), CANVAS_WIDTH, CANVAS_HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba_out)
        .expect("failed to write PNG image data");

    eprintln!("wrote {CANVAS_WIDTH}x{CANVAS_HEIGHT} canvas accessibility render to {out_path}");
    eprintln!(
        "all canvas accessibility assertions passed -- Step 5.3 (5.3.1-5.3.3) closed in full"
    );
}
