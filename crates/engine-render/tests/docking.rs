//! §14 build-order step 15 (§11.4): the standalone "real docked screen"
//! proof -- a Left/Center/Right layout (the same mechanism a full
//! 5-zone layout would use), each zone showing only its own active tab,
//! resized via Stage A's own real splitter mechanism (not a second
//! resize path -- §11.4's own "docking is a *consumer* of splitters"
//! text, proven by literal code reuse here, not just architectural
//! framing). Same headless render-to-texture-then-readback discipline
//! as every other pixel-level proof in this crate.

use std::time::Instant;

use engine_core::{
    Animated, DockLayout, DockSide, DockZone, NodeKind, PaintProperties, SplitterState, Tree,
};
use engine_render::{FrameRenderer, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{AvailableSpace, FlexDirection, Size, Style, auto, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const LEFT_TAB_A: Color = Color::from_rgba8(0xFF, 0x00, 0x00, 0xFF); // red
const LEFT_TAB_B: Color = Color::from_rgba8(0x00, 0xFF, 0x00, 0xFF); // green
const CENTER: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF); // white
const RIGHT: Color = Color::from_rgba8(0x00, 0x00, 0xFF, 0xFF); // blue

fn rect(tree: &mut Tree, color: Color, width: f32, height: f32) -> engine_core::NodeId {
    tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(width),
                height: length(height),
            },
            ..Default::default()
        },
        PaintProperties::new(color, 0.0, 0.0, 1.0),
    )
}

// A dock panel's real content fills whatever width its zone container
// currently has (the zone container is what the shared splitter
// mechanism resizes, per `Tree::set_splitter_position` -- see
// `crates/engine-core/src/tree.rs`), not a size fixed at construction
// time. `flex_grow: 1.0` + `width: auto()` is taffy's own mechanism for
// "stretch to fill the flex parent's available space" (confirmed at
// `taffy-0.14.0/src/style/mod.rs:718`), matching how a real tabbed
// panel's content behaves.
fn flex_fill_rect(tree: &mut Tree, color: Color, height: f32) -> engine_core::NodeId {
    tree.insert(
        NodeKind::Rect,
        Style {
            flex_grow: 1.0,
            size: Size {
                width: auto(),
                height: length(height),
            },
            ..Default::default()
        },
        PaintProperties::new(color, 0.0, 0.0, 1.0),
    )
}

async fn render(tree: &Tree, root: engine_core::NodeId, width: u16, height: u16) -> (Vec<u8>, u32) {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .expect("no wgpu adapter available in this environment");
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("engine-render docking test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("docking test target"),
        size: wgpu::Extent3d {
            width: u32::from(width),
            height: u32::from(height),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let mut frame_renderer = FrameRenderer::new(
        &device,
        &RenderTargetConfig {
            format: texture.format(),
            width: u32::from(width),
            height: u32::from(height),
        },
    );
    let mut text_renderer = TextRenderer::new();
    let scene = build_tree_scene(
        tree,
        root,
        width,
        height,
        frame_renderer.resources_mut(),
        &mut text_renderer,
    );
    let render_size = RenderSize {
        width: u32::from(width),
        height: u32::from(height),
    };
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    frame_renderer.render(&scene, &device, &queue, &mut encoder, &render_size, &view);

    let bytes_per_row = (u32::from(width) * 4).next_multiple_of(256);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(bytes_per_row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: None,
            },
        },
        wgpu::Extent3d {
            width: u32::from(width),
            height: u32::from(height),
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);

    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |result| {
        result.expect("failed to map readback buffer");
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("device poll failed");

    let data = slice.get_mapped_range();
    let mut out = vec![0u8; data.len()];
    out.copy_from_slice(&data);
    (out, bytes_per_row)
}

fn pixel_at(data: &[u8], bytes_per_row: u32, x: u32, y: u32) -> [u8; 4] {
    let row_start = (y * bytes_per_row) as usize;
    let start = row_start + (x * 4) as usize;
    [
        data[start],
        data[start + 1],
        data[start + 2],
        data[start + 3],
    ]
}

#[test]
fn docked_layout_shows_active_tabs_and_resizes_via_the_shared_splitter_mechanism() {
    pollster::block_on(async {
        // Left(100) | splitter(10) | Center(100) | splitter(10) | Right(80) = 300
        let width: u16 = 300;
        let height: u16 = 60;

        let mut tree = Tree::new();
        let root = tree.insert(
            NodeKind::Container,
            Style {
                display: taffy::Display::Flex,
                flex_direction: FlexDirection::Row,
                size: Size {
                    width: length(f32::from(width)),
                    height: length(f32::from(height)),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );

        // The Left zone's own container, holding two tabs (only one
        // ever attached at a time).
        let left_container = rect(
            &mut tree,
            Color::from_rgba8(0, 0, 0, 0),
            100.0,
            f32::from(height),
        );
        let left_tab_a = flex_fill_rect(&mut tree, LEFT_TAB_A, f32::from(height));
        let left_tab_b = flex_fill_rect(&mut tree, LEFT_TAB_B, f32::from(height));
        tree.add_child(root, left_container);

        let left_right_splitter = tree.insert(
            NodeKind::Splitter(SplitterState {
                position: Animated::new(0.5),
            }),
            Style {
                size: Size {
                    width: length(10.0),
                    height: length(f32::from(height)),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0x80, 0x80, 0x80, 0xFF), 0.0, 0.0, 1.0),
        );
        tree.add_child(root, left_right_splitter);

        let center_container = rect(&mut tree, CENTER, 100.0, f32::from(height));
        tree.add_child(root, center_container);

        let center_right_splitter = tree.insert(
            NodeKind::Splitter(SplitterState {
                position: Animated::new(0.5),
            }),
            Style {
                size: Size {
                    width: length(10.0),
                    height: length(f32::from(height)),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0x80, 0x80, 0x80, 0xFF), 0.0, 0.0, 1.0),
        );
        tree.add_child(root, center_right_splitter);

        let right_container = rect(&mut tree, RIGHT, 80.0, f32::from(height));
        tree.add_child(root, right_container);
        let _ = center_right_splitter; // keeps the layout geometrically real; not resized in this test

        let mut layout = DockLayout::new();
        let mut left_zone = DockZone::new(100.0);
        left_zone.panels = smallvec::smallvec![left_tab_a, left_tab_b];
        left_zone.active_tab = 0;
        layout.set_zone(DockSide::Left, left_zone);

        tree.apply_active_tab(left_container, layout.zone(DockSide::Left).unwrap());

        let available = Size {
            width: AvailableSpace::Definite(f32::from(width)),
            height: AvailableSpace::Definite(f32::from(height)),
        };
        tree.compute_layout(root, available);

        // Left zone (tab A, red) spans [0,100); center (white) spans
        // [110,210); right (blue) spans [220,300).
        let (data, bpr) = render(&tree, root, width, height).await;
        assert_eq!(
            pixel_at(&data, bpr, 50, 30),
            [0xFF, 0x00, 0x00, 0xFF],
            "Left zone should show tab A (red)"
        );
        assert_eq!(
            pixel_at(&data, bpr, 160, 30),
            [0xFF, 0xFF, 0xFF, 0xFF],
            "Center zone should show its own color"
        );
        assert_eq!(
            pixel_at(&data, bpr, 260, 30),
            [0x00, 0x00, 0xFF, 0xFF],
            "Right zone should show its own color"
        );

        // Switch the Left zone's active tab -- must not disturb Center/Right at all.
        layout.zone_mut(DockSide::Left).unwrap().active_tab = 1;
        tree.apply_active_tab(left_container, layout.zone(DockSide::Left).unwrap());
        tree.compute_layout(root, available);

        let (data, bpr) = render(&tree, root, width, height).await;
        assert_eq!(
            pixel_at(&data, bpr, 50, 30),
            [0x00, 0xFF, 0x00, 0xFF],
            "Left zone must now show tab B (green) after switching"
        );
        assert_eq!(
            pixel_at(&data, bpr, 160, 30),
            [0xFF, 0xFF, 0xFF, 0xFF],
            "Center zone must be completely unaffected by the Left zone's tab switch"
        );
        assert_eq!(
            pixel_at(&data, bpr, 260, 30),
            [0x00, 0x00, 0xFF, 0xFF],
            "Right zone must be completely unaffected by the Left zone's tab switch"
        );

        // Resize the Left|Center boundary via the *same* splitter
        // mechanism Stage A already proved -- no docking-specific
        // resize code exists anywhere in this test or in engine-core.
        let now = Instant::now();
        tree.set_splitter_position(left_right_splitter, 0.8, now);
        tree.compute_layout(root, available);

        // Left+Center share 200px; at 0.8 the Left zone grows to 160px,
        // so x=130 (inside the old Center zone, now inside the grown
        // Left zone) must show the Left zone's current tab (green).
        let (data, bpr) = render(&tree, root, width, height).await;
        let moved_boundary_point = pixel_at(&data, bpr, 130, 30);
        assert_eq!(
            moved_boundary_point,
            [0x00, 0xFF, 0x00, 0xFF],
            "after resizing via the shared splitter mechanism, the Left zone's real \
             boundary must have moved -- got {moved_boundary_point:?}"
        );
        // The Right zone, untouched by this splitter, must be exactly where it always was.
        assert_eq!(
            pixel_at(&data, bpr, 260, 30),
            [0x00, 0x00, 0xFF, 0xFF],
            "Right zone must be unaffected by the Left|Center resize"
        );
    });
}
