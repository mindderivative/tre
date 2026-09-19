//! M5 Phase 3 (§11.10, §11.11): the standalone proof that
//! `NodeKind::Canvas`'s `DrawCommand`s paint real pixels, in the same
//! node-local coordinate space every other `NodeKind` draws into --
//! composed with an ancestor transform (M5 Phase 1) exactly like a
//! `Rect`, with zero special-casing. Same headless
//! render-to-texture-then-readback discipline as every other
//! pixel-level proof in this crate.

use engine_core::{CanvasState, CustomHitTest, DrawCommand, NodeKind, PaintProperties, Tree};
use engine_render::{FrameRenderer, GeometryCache, TextRenderer, build_tree_scene};
use peniko::Color;
use peniko::kurbo::{Affine, BezPath};
use taffy::prelude::{AvailableSpace, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const BACKGROUND: Color = Color::from_rgba8(0x11, 0x11, 0x11, 0xFF);
const CIRCLE: Color = Color::from_rgba8(0x00, 0xFF, 0x00, 0xFF);
const LINE: Color = Color::from_rgba8(0xFF, 0x00, 0x00, 0xFF);

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
            label: Some("engine-render canvas-paint test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("canvas-paint test target"),
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
fn canvas_draw_commands_paint_real_pixels_composed_with_an_ancestor_transform() {
    pollster::block_on(async {
        let width: u16 = 200;
        let height: u16 = 200;

        let mut tree = Tree::new();
        // Real Rect background (Container paints nothing -- same lesson
        // M5 Phase 1's own transform_composition.rs test learned the
        // hard way).
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

        let mut state = CanvasState::new();
        state.commands.push(DrawCommand::FillCircle {
            cx: 20.0,
            cy: 20.0,
            radius: 10.0,
            color: CIRCLE,
        });
        let mut line = BezPath::new();
        line.move_to((0.0, 0.0));
        line.line_to((40.0, 0.0));
        state.commands.push(DrawCommand::StrokePath {
            path: line,
            color: LINE,
            width: 4.0,
        });
        state.hit_test = Some(CustomHitTest::Circle {
            cx: 20.0,
            cy: 20.0,
            radius: 10.0,
        });

        let canvas = tree.insert(
            NodeKind::Canvas(state),
            Style {
                size: Size {
                    width: length(40.0),
                    height: length(40.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(root, canvas);

        // Animate the canvas's own transform (M5 Phase 1's own
        // composition mechanism) so this test also proves Canvas
        // content isn't special-cased out of it.
        tree.get_mut(canvas).unwrap().paint.transform.current = Affine::translate((60.0, 60.0));

        let available = Size {
            width: AvailableSpace::Definite(f32::from(width)),
            height: AvailableSpace::Definite(f32::from(height)),
        };
        tree.compute_layout(root, available);

        let (data, bpr) = render(&tree, root, width, height).await;

        // The circle's local center (20,20) maps to canvas (80,80)
        // under the (60,60) translate.
        let circle_center = pixel_at(&data, bpr, 80, 80);
        assert_eq!(
            circle_center,
            [0x00, 0xFF, 0x00, 0xFF],
            "the FillCircle command must paint at its transformed on-screen position, \
             got {circle_center:?}"
        );

        // The stroked line's local midpoint (20,0) maps to canvas
        // (80,60).
        let line_point = pixel_at(&data, bpr, 80, 60);
        assert_eq!(
            line_point,
            [0xFF, 0x00, 0x00, 0xFF],
            "the StrokePath command must paint at its transformed on-screen position, \
             got {line_point:?}"
        );

        // A point clearly outside both commands' painted area, still
        // within the canvas's own 40x40 box, must show plain background.
        let empty_spot = pixel_at(&data, bpr, 95, 95);
        assert_eq!(
            empty_spot,
            [0x11, 0x11, 0x11, 0xFF],
            "an empty part of the canvas's own box must show background through it, \
             got {empty_spot:?}"
        );
    });
}

/// M25 Phase 2 (§5, §6): a real, previously-missing compounding -- a
/// `Canvas` node's own real `PaintProperties.opacity` must fade every
/// one of its `DrawCommand`s, not just leave them at full alpha
/// regardless. The real range-check pattern `animated_rect.rs`'s own
/// mid-flight test already established (strictly between the fully-
/// opaque and fully-transparent extremes), since predicting an exact
/// blended byte value depends on `vello_hybrid`'s own real compositing
/// internals this test doesn't need to know.
#[test]
fn canvas_draw_commands_compound_with_the_nodes_own_real_opacity() {
    pollster::block_on(async {
        let width: u16 = 100;
        let height: u16 = 100;

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

        let mut state = CanvasState::new();
        state.commands.push(DrawCommand::FillRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            color: CIRCLE,
        });

        let canvas = tree.insert(
            NodeKind::Canvas(state),
            Style {
                size: Size {
                    width: length(f32::from(width)),
                    height: length(f32::from(height)),
                },
                ..Default::default()
            },
            // opacity = 0.5, the fourth positional field.
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 0.5),
        );
        tree.add_child(root, canvas);

        let available = Size {
            width: AvailableSpace::Definite(f32::from(width)),
            height: AvailableSpace::Definite(f32::from(height)),
        };
        tree.compute_layout(root, available);

        let (data, bpr) = render(&tree, root, width, height).await;
        let center = pixel_at(&data, bpr, 50, 50);

        // Green channel: fully-opaque would be 0xFF, fully-transparent
        // (pure background) would be 0x11 -- real partial opacity must
        // land strictly between the two.
        let observed_g = f32::from(center[1]);
        assert!(
            observed_g > f32::from(0x11_u8) && observed_g < f32::from(0xFF_u8),
            "green channel {observed_g} is not strictly between background (0x11) and the \
             command's own full-alpha color (0xFF) -- the FillRect isn't compounding with the \
             node's own real opacity, got {center:?}"
        );
    });
}
