//! M5 Phase 4 (§11.11): the standalone proof that transform composition
//! (Phase 1), transform-aware hit-testing including the custom override
//! (Phases 2-3), `NodeKind::Canvas` (Phase 3), and `NodeKind::VirtualList`
//! (M3 step 15 Stage C) genuinely compose in one real tree -- not just
//! separately, in isolation, the way every existing test up to this
//! phase proves each piece alone. §11.11's own text: "no new framework
//! mechanism ... both compose entirely from what's already specified."
//!
//! One shared "camera" `Container` with a real (non-identity) animated
//! `transform`, holding three children of three different kinds -- a
//! `Rect` graph "node," a `Canvas` graph "edge," and a `VirtualList`
//! with one materialized item (standing in for "many graph nodes,
//! windowed") -- and asserts all three paint AND hit-test correctly at
//! their transformed positions, plus that the edge's custom hit-test
//! still genuinely excludes an off-path point inside its own bounding
//! box, re-confirming Phase 3's "override, not narrowing" claim holds
//! inside a combined scene.

use engine_core::{
    CanvasState, CustomHitTest, DrawCommand, ItemExtent, NodeKind, PaintProperties, Tree,
    VirtualListState,
};
use engine_render::{FrameRenderer, TextRenderer, build_tree_scene};
use peniko::Color;
use peniko::kurbo::{Affine, BezPath, Point};
use taffy::prelude::{AvailableSpace, Position, Rect as TaffyRect, Size, Style, auto, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const BACKGROUND: Color = Color::from_rgba8(0x11, 0x11, 0x11, 0xFF);
const NODE_COLOR: Color = Color::from_rgba8(0xFF, 0xA5, 0x00, 0xFF);
const EDGE_COLOR: Color = Color::from_rgba8(0x00, 0xFF, 0x00, 0xFF);
const VLIST_COLOR: Color = Color::from_rgba8(0x00, 0x99, 0xFF, 0xFF);

fn absolute(left: f32, top: f32, width: f32, height: f32) -> Style {
    Style {
        position: Position::Absolute,
        inset: TaffyRect {
            left: length(left),
            top: length(top),
            right: auto(),
            bottom: auto(),
        },
        size: Size {
            width: length(width),
            height: length(height),
        },
        ..Default::default()
    }
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
            label: Some("engine-render graph-composition test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("graph-composition test target"),
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

fn virtual_list_item(_idx: usize) -> (NodeKind, Style, PaintProperties) {
    (
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(20.0),
                height: length(10.0),
            },
            ..Default::default()
        },
        PaintProperties::new(VLIST_COLOR, 0.0, 0.0, 1.0),
    )
}

#[test]
fn canvas_transform_custom_hit_test_and_virtual_list_all_compose_under_one_shared_transform() {
    pollster::block_on(async {
        let width: u16 = 300;
        let height: u16 = 300;

        let mut tree = Tree::new();
        let root = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(f32::from(width)),
                    height: length(f32::from(height)),
                },
                ..Default::default()
            },
            PaintProperties::new(BACKGROUND, 0.0, 0.0, 1.0),
        );

        let camera = tree.insert(
            NodeKind::Container,
            absolute(0.0, 0.0, f32::from(width), f32::from(height)),
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(root, camera);

        // A real graph "node": an ordinary Rect, the already-real,
        // existing mechanism for a simple clickable circle/node --
        // Canvas/custom hit-testing is for edges, not this.
        let node = tree.insert(
            NodeKind::Rect,
            absolute(20.0, 20.0, 20.0, 20.0),
            PaintProperties::new(NODE_COLOR, 0.0, 0.0, 1.0),
        );
        tree.add_child(camera, node);

        // A real graph "edge": a Canvas with one StrokePath and its own
        // Path custom hit-test, both in the same node-local coordinate
        // space (0,0 at the edge node's own top-left).
        let mut edge_state = CanvasState::new();
        let mut line = BezPath::new();
        line.move_to((100.0, 100.0));
        line.line_to((140.0, 100.0));
        edge_state.commands.push(DrawCommand::StrokePath {
            path: line.clone(),
            color: EDGE_COLOR,
            width: 6.0,
        });
        edge_state.hit_test = Some(CustomHitTest::Path {
            path: line,
            tolerance: 5.0,
        });
        let edge = tree.insert(
            NodeKind::Canvas(edge_state),
            absolute(0.0, 0.0, f32::from(width), f32::from(height)),
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(camera, edge);

        // A `VirtualList` standing in for "many graph nodes, windowed" --
        // only its one materialized item needs to exist as a real Node.
        let list = tree.insert(
            NodeKind::VirtualList(VirtualListState::new(1, ItemExtent::Fixed(10.0))),
            absolute(200.0, 200.0, 40.0, 10.0),
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(camera, list);

        let available = Size {
            width: AvailableSpace::Definite(f32::from(width)),
            height: AvailableSpace::Definite(f32::from(height)),
        };
        tree.compute_layout(root, available);
        tree.set_virtual_list_window(list, 0..1, virtual_list_item);
        tree.compute_layout(root, available);

        // Capture the materialized item's pure local position -- valid
        // regardless of `camera`'s own transform (M5 Phase 1/2's own
        // established boundary: `absolute_position` stays pure
        // translation) -- before setting a real transform below.
        let materialized = match &tree.get(list).unwrap().kind {
            NodeKind::VirtualList(state) => *state.materialized.get(&0).unwrap(),
            _ => unreachable!(),
        };
        let (item_x, item_y) = tree.absolute_position(materialized);

        // A real translate-only transform on `camera` -- every child's
        // painted/hit-tested position must shift by exactly this offset,
        // regardless of its own `NodeKind`.
        let offset = (40.0, 40.0);
        tree.get_mut(camera).unwrap().paint.transform.current = Affine::translate(offset);
        tree.compute_layout(root, available);

        let (data, bpr) = render(&tree, root, width, height).await;

        // The Rect node: local box (20,20)-(40,40) -> canvas
        // (60,60)-(80,80); center (70,70).
        let node_pixel = pixel_at(&data, bpr, 70, 70);
        assert_eq!(
            node_pixel,
            [0xFF, 0xA5, 0x00, 0xFF],
            "the Rect node must paint at its transformed position, got {node_pixel:?}"
        );
        assert_eq!(
            tree.hit_test(
                root,
                Point::new(70.0 + 0.0, 70.0 + 0.0) // already canvas-space
            ),
            Some(node),
            "hit-testing must find the Rect node at its transformed position"
        );

        // The Canvas edge: local line midpoint (120,100) -> canvas
        // (160,140).
        let edge_pixel = pixel_at(&data, bpr, 160, 140);
        assert_eq!(
            edge_pixel,
            [0x00, 0xFF, 0x00, 0xFF],
            "the Canvas edge must paint at its transformed position, got {edge_pixel:?}"
        );
        assert_eq!(
            tree.hit_test(root, Point::new(160.0, 140.0)),
            Some(edge),
            "hit-testing must find the Canvas edge (via its custom Path hit-test) at its \
             transformed position"
        );
        // A point still inside the edge's own (huge, full-canvas)
        // bounding box, but far from the actual stroked line, must NOT
        // hit the edge -- re-confirms Phase 3's "override, not
        // narrowing" claim holds inside a combined scene.
        assert_ne!(
            tree.hit_test(root, Point::new(160.0, 290.0)),
            Some(edge),
            "a point inside the edge's bounding box but far from its actual path must miss \
             the edge's custom hit-test, even in a combined scene"
        );

        // The VirtualList's materialized item: its own captured local
        // position, shifted by the same real transform offset.
        let item_canvas_x = (item_x + offset.0) as u32;
        let item_canvas_y = (item_y + offset.1) as u32;
        let vlist_pixel = pixel_at(&data, bpr, item_canvas_x + 2, item_canvas_y + 2);
        assert_eq!(
            vlist_pixel,
            [0x00, 0x99, 0xFF, 0xFF],
            "the VirtualList's materialized item must paint at its transformed position, \
             got {vlist_pixel:?}"
        );
        assert_eq!(
            tree.hit_test(
                root,
                Point::new(
                    f64::from(item_canvas_x) + 2.0,
                    f64::from(item_canvas_y) + 2.0
                )
            ),
            Some(materialized),
            "hit-testing must find the VirtualList's materialized item at its transformed \
             position"
        );
    });
}
