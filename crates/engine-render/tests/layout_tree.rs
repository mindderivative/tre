//! §14 build-order step 3: proves `engine-core`'s `Tree`/`taffy` layout
//! and `engine-render`'s painting actually compose -- not just that
//! `build_tree_scene` compiles against `Tree`, but that a node's
//! *taffy-computed position* is genuinely what determines where its
//! color lands on screen. Same headless render-to-texture-then-readback
//! discipline as `animated_rect.rs` (step 2) and `rect_scene_renders_expected_pixels`
//! (step 1).
//!
//! Two same-size children, side by side in a row, each a distinct solid
//! color: if layout were ignored (e.g. both painted at the origin), the
//! second color would simply overwrite the first at every sampled point.
//! Sampling one point inside each child's own laid-out box and asserting
//! *that* child's color is there is the actual claim this step exists to
//! prove.

use engine_core::{NodeKind, PaintProperties, Tree};
use engine_render::{FrameRenderer, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{AvailableSpace, FlexDirection, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const RED: Color = Color::from_rgba8(0xFF, 0x00, 0x00, 0xFF);
const BLUE: Color = Color::from_rgba8(0x00, 0x00, 0xFF, 0xFF);
const TRANSPARENT: Color = Color::from_rgba8(0, 0, 0, 0);

#[test]
fn two_row_children_paint_at_their_own_laid_out_positions() {
    pollster::block_on(async {
        let width: u16 = 200;
        let height: u16 = 100;

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
            PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
        );
        let left = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(100.0),
                    height: length(100.0),
                },
                ..Default::default()
            },
            PaintProperties::new(RED, 0.0, 0.0, 1.0),
        );
        let right = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(100.0),
                    height: length(100.0),
                },
                ..Default::default()
            },
            PaintProperties::new(BLUE, 0.0, 0.0, 1.0),
        );
        tree.add_child(root, left);
        tree.add_child(root, right);

        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(f32::from(width)),
                height: AvailableSpace::Definite(f32::from(height)),
            },
        );
        // Sanity-check the layout itself before trusting the render to
        // prove anything about it: left at x=0, right at x=100.
        assert_eq!(tree.layout(left).location.x, 0.0);
        assert_eq!(tree.layout(right).location.x, 100.0);

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
                label: Some("engine-render layout-tree test device"),
                required_features: wgpu::Features::empty(),
                ..Default::default()
            })
            .await
            .expect("failed to create wgpu device");

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("layout-tree test target"),
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
            &tree,
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
        let pixel_at = |x: u32, y: u32| -> [u8; 4] {
            let row_start = (y * bytes_per_row) as usize;
            let px_start = row_start + (x * 4) as usize;
            [
                data[px_start],
                data[px_start + 1],
                data[px_start + 2],
                data[px_start + 3],
            ]
        };

        // Inside the left child's own laid-out box (x in 0..100): red.
        let left_pixel = pixel_at(50, 50);
        assert_eq!(
            left_pixel,
            [0xFF, 0x00, 0x00, 0xFF],
            "pixel at (50,50), inside the left child's taffy-computed box, \
             is {left_pixel:?}, not red -- layout position isn't driving paint position"
        );

        // Inside the right child's own laid-out box (x in 100..200): blue.
        let right_pixel = pixel_at(150, 50);
        assert_eq!(
            right_pixel,
            [0x00, 0x00, 0xFF, 0xFF],
            "pixel at (150,50), inside the right child's taffy-computed box, \
             is {right_pixel:?}, not blue -- layout position isn't driving paint position"
        );
    });
}
