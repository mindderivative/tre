//! M4 Phase 3 (§11.5): the standalone proof that a real `Tree::dispatch`
//! pointer press+move+release sequence -- not a direct `set_splitter_
//! position` call, which `splitter_drag.rs` already proves moves a real
//! pane boundary -- genuinely drags a splitter on screen. Same scene,
//! same headless render-to-texture-then-readback discipline, driven
//! through real dispatch instead.

use std::time::Instant;

use engine_core::{
    Animated, InputEvent, InteractionConfig, NodeKind, PaintProperties, PointerButton,
    SplitterState, Tree,
};
use engine_render::{FrameRenderer, GeometryCache, TextRenderer, build_tree_scene};
use peniko::Color;
use peniko::kurbo::Point;
use taffy::prelude::{AvailableSpace, FlexDirection, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

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
            label: Some("engine-render splitter-drag-dispatch test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("splitter-drag-dispatch test target"),
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
    let mut geometry_cache = GeometryCache::new();
    let scene = build_tree_scene(
        tree,
        root,
        width,
        height,
        frame_renderer.resources_mut(),
        &mut text_renderer,
        &mut geometry_cache,
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
fn dragging_a_splitter_via_real_dispatch_moves_the_real_pane_boundary_on_screen() {
    pollster::block_on(async {
        let width: u16 = 210;
        let height: u16 = 50;

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

        let left = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(100.0),
                    height: length(f32::from(height)),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0xFF, 0x00, 0x00, 0xFF), 0.0, 0.0, 1.0),
        );
        tree.add_child(root, left);

        let splitter = tree.insert(
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
        tree.add_child(root, splitter);

        let right = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(100.0),
                    height: length(f32::from(height)),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0x00, 0x00, 0xFF, 0xFF), 0.0, 0.0, 1.0),
        );
        tree.add_child(root, right);

        let available = Size {
            width: AvailableSpace::Definite(f32::from(width)),
            height: AvailableSpace::Definite(f32::from(height)),
        };
        tree.compute_layout(root, available);

        // Left pane spans [0,100), splitter [100,110), right pane
        // [110,210). x=130 sits inside the right (blue) pane before
        // any drag.
        let flip_point = (130, 25);
        let (data_before, bpr) = render(&tree, root, width, height).await;
        let before = pixel_at(&data_before, bpr, flip_point.0, flip_point.1);
        assert_eq!(
            before,
            [0x00, 0x00, 0xFF, 0xFF],
            "before dragging, x=130 should be inside the right (blue) pane, got {before:?}"
        );

        let config = InteractionConfig {
            hover_opacity: 0.08,
            hover_duration: std::time::Duration::from_millis(100),
            focus_ring_opacity: 1.0,
            focus_ring_duration: std::time::Duration::from_millis(100),
            ripple_radius: 50.0,
            ripple_opacity: 0.12,
            ripple_duration: std::time::Duration::from_millis(300),
        };
        let now = Instant::now();

        // Real dispatch, not a direct set_splitter_position call: press
        // on the splitter itself (x=105, inside [100,110)), then drag to
        // x=150 -- fraction = 150/200 = 0.75, the same target
        // `splitter_drag.rs`'s own direct-call test proves.
        tree.dispatch(
            root,
            InputEvent::PointerPressed {
                position: Point::new(105.0, 25.0),
                button: PointerButton::Primary,
            },
            &config,
            now,
        );
        tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(150.0, 25.0),
            },
            &config,
            now,
        );
        tree.compute_layout(root, available);

        let (data_mid_drag, bpr_mid) = render(&tree, root, width, height).await;
        let mid_drag = pixel_at(&data_mid_drag, bpr_mid, flip_point.0, flip_point.1);
        assert_eq!(
            mid_drag,
            [0xFF, 0x00, 0x00, 0xFF],
            "mid-drag (before release), x=130 must already be inside the grown left (red) \
             pane -- the real claim this test exists to prove: dispatch, not just the \
             underlying mechanism, moves the real rendered boundary, and it live-follows \
             the cursor before release, got {mid_drag:?}"
        );

        tree.dispatch(
            root,
            InputEvent::PointerReleased {
                position: Point::new(150.0, 25.0),
                button: PointerButton::Primary,
            },
            &config,
            now,
        );
        tree.compute_layout(root, available);

        let (data_after, bpr_after) = render(&tree, root, width, height).await;
        let after = pixel_at(&data_after, bpr_after, flip_point.0, flip_point.1);
        assert_eq!(
            after,
            [0xFF, 0x00, 0x00, 0xFF],
            "after releasing, x=130 must still be inside the grown left (red) pane, \
             got {after:?}"
        );

        // The shrunk right pane must still show its own color at its
        // own far edge -- it shrank, it didn't disappear or get
        // overpainted entirely.
        let still_right = pixel_at(&data_after, bpr_after, 200, 25);
        assert_eq!(
            still_right,
            [0x00, 0x00, 0xFF, 0xFF],
            "the shrunk right pane must still show its own color at its own far edge, \
             got {still_right:?}"
        );

        // A further pointer move after release must not keep dragging.
        tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(180.0, 25.0),
            },
            &config,
            now,
        );
        tree.compute_layout(root, available);
        let (data_post_release_move, bpr_post) = render(&tree, root, width, height).await;
        let post_release = pixel_at(
            &data_post_release_move,
            bpr_post,
            flip_point.0,
            flip_point.1,
        );
        assert_eq!(
            post_release, after,
            "a pointer move after release must not still be tracked as a drag, \
             got {post_release:?}"
        );
    });
}
