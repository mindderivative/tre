//! M8 Phase 1 (§11.8): the standalone proof that `paint_node` genuinely
//! skips Vello scene-encoding for an entire off-screen subtree, not
//! just that off-screen content happens not to show up in the final
//! rendered pixels (which would be true even *without* real culling --
//! Vello only ever rasterizes into the render target's own bounds
//! regardless). Same headless render-to-texture-then-readback
//! discipline as every other pixel-level proof in this crate.
//!
//! Two claims, kept deliberately separate:
//!
//! 1. **The real, distinguishing behavior culling adds**: a parent
//!    node positioned entirely outside the viewport, whose own child
//!    has a `transform` that would otherwise bring *it* back onto
//!    visible canvas, never paints that child at all -- because the
//!    parent's own off-screen bounds skip its whole subtree's
//!    recursion before the child's own composed transform is ever
//!    computed. Without real culling, the child's own transform would
//!    still place it on-screen and it would paint normally.
//! 2. A node only *partially* overlapping the viewport (straddling its
//!    edge) still paints its visible portion -- proving this is a
//!    whole-subtree accept/reject decision based on real bounding-box
//!    overlap, not an all-or-nothing "must be fully inside" test that
//!    would wrongly drop partially-visible content.

use engine_core::{NodeKind, PaintProperties, Tree};
use engine_render::{FrameRenderer, TextRenderer, build_tree_scene};
use peniko::Color;
use peniko::kurbo::Affine;
use taffy::prelude::{AvailableSpace, Position, Rect as TaffyRect, Size, Style, auto, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const BACKGROUND: Color = Color::from_rgba8(0x11, 0x11, 0x11, 0xFF);
const CHIP: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF);

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
            label: Some("engine-render paint-culling test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("paint-culling test target"),
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
fn an_off_screen_parent_skips_its_whole_subtree_even_when_a_child_transforms_back_on_screen() {
    pollster::block_on(async {
        let (width, height) = (100u16, 100u16);
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

        // Positioned 10,000px away -- nowhere near the 100x100
        // viewport, by any reasonable definition of "off-screen."
        let offscreen_parent = tree.insert(
            NodeKind::Container,
            absolute(10_000.0, 10_000.0, 40.0, 40.0),
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(root, offscreen_parent);

        // The child's own `transform` would, on its own, translate it
        // right back onto visible canvas (roughly the viewport's own
        // center) -- if `paint_node` ever reached this node at all.
        let child = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(20.0),
                    height: length(20.0),
                },
                ..Default::default()
            },
            PaintProperties::new(CHIP, 0.0, 0.0, 1.0),
        );
        tree.add_child(offscreen_parent, child);
        tree.get_mut(child).unwrap().paint.transform.current =
            Affine::translate((-9960.0, -9960.0));

        let available = Size {
            width: AvailableSpace::Definite(f32::from(width)),
            height: AvailableSpace::Definite(f32::from(height)),
        };
        tree.compute_layout(root, available);

        let (data, bpr) = render(&tree, root, width, height).await;
        let at_childs_real_target = pixel_at(&data, bpr, 50, 50);
        assert_eq!(
            at_childs_real_target,
            [0x11, 0x11, 0x11, 0xFF],
            "the child must not paint at all -- its off-screen parent's own subtree was \
             skipped before the child's own transform was ever composed, got \
             {at_childs_real_target:?}"
        );
    });
}

#[test]
fn a_node_only_partially_overlapping_the_viewport_still_paints_its_visible_portion() {
    pollster::block_on(async {
        let (width, height) = (100u16, 100u16);
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

        // Straddles the right edge: x in [80, 120) -- half on-screen,
        // half off. A whole-subtree-skip based on bounding-box overlap
        // must still paint the visible left half.
        let straddling = tree.insert(
            NodeKind::Rect,
            absolute(80.0, 40.0, 40.0, 20.0),
            PaintProperties::new(CHIP, 0.0, 0.0, 1.0),
        );
        tree.add_child(root, straddling);

        let available = Size {
            width: AvailableSpace::Definite(f32::from(width)),
            height: AvailableSpace::Definite(f32::from(height)),
        };
        tree.compute_layout(root, available);

        let (data, bpr) = render(&tree, root, width, height).await;
        let inside_visible_half = pixel_at(&data, bpr, 90, 50);
        assert_eq!(
            inside_visible_half,
            [0xFF, 0xFF, 0xFF, 0xFF],
            "a node only partially overlapping the viewport must still paint its real, \
             visible portion -- overlap-based culling must not wrongly treat this as fully \
             off-screen, got {inside_visible_half:?}"
        );
    });
}
