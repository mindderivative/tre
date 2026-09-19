//! M5 Phase 1 (§11.9): the standalone proof that `PaintProperties.
//! transform` really composes down the tree during paint -- a
//! `Container`'s animated transform (pan offset × zoom scale)
//! propagates to an untouched descendant, with zero per-descendant
//! code. Same headless render-to-texture-then-readback discipline as
//! every other pixel-level proof in this crate.
//!
//! Two claims, kept deliberately separate, matching `splitter_drag_
//! dispatch.rs`'s own before/mid-flight shape:
//!
//! 1. Before any transform is applied (`Animated::new`'s own identity
//!    default), the child rect renders at its plain taffy layout
//!    position -- proving this phase changed nothing for existing,
//!    untransformed content.
//! 2. Halfway through animating the `Container`'s own `transform`
//!    toward a combined pan+zoom target, the child -- which never set
//!    a `transform` of its own -- has visibly moved to the
//!    *interpolated* transformed position, and no longer occupies its
//!    original untransformed spot. This is the real claim: composition
//!    reaches a descendant for free, and `Interpolate for Affine`
//!    (a componentwise coefficient lerp, PLAN.md) genuinely drives it
//!    mid-flight, not just at either endpoint.

use std::time::{Duration, Instant};

use engine_core::{MotionCurve, NodeKind, PaintProperties, Tree};
use engine_render::{FrameRenderer, GeometryCache, TextRenderer, build_tree_scene};
use peniko::Color;
use peniko::kurbo::Affine;
use taffy::prelude::{AvailableSpace, Position, Rect as TaffyRect, Size, Style, auto, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const BACKGROUND: Color = Color::from_rgba8(0x11, 0x11, 0x11, 0xFF);
const CHIP: Color = Color::from_rgba8(0xFF, 0xA5, 0x00, 0xFF); // the transformed descendant

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
            label: Some("engine-render transform-composition test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("transform-composition test target"),
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
fn a_containers_animated_transform_propagates_to_an_untouched_child() {
    pollster::block_on(async {
        let width: u16 = 200;
        let height: u16 = 200;

        let mut tree = Tree::new();
        // `NodeKind::Container` deliberately paints nothing itself (it
        // exists purely to give `taffy` something to lay children out
        // against, `paint_node`'s own match arm) -- the background has
        // to be a real `Rect`, not a `Container`, for this test's own
        // "did the chip move away from its old spot" pixel check to
        // mean anything.
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

        // The "camera" node: full-canvas Container whose own `transform`
        // is what gets animated. Its own fill is transparent -- only its
        // effect on its child is under test.
        let camera = tree.insert(
            NodeKind::Container,
            absolute(0.0, 0.0, f32::from(width), f32::from(height)),
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(root, camera);

        // The child under test: a plain 40x40 chip at local (20, 20)
        // inside `camera`, with no `transform` of its own -- everything
        // that moves it has to come from composition with its parent.
        let chip = tree.insert(
            NodeKind::Rect,
            absolute(20.0, 20.0, 40.0, 40.0),
            PaintProperties::new(CHIP, 0.0, 0.0, 1.0),
        );
        tree.add_child(camera, chip);

        let available = Size {
            width: AvailableSpace::Definite(f32::from(width)),
            height: AvailableSpace::Definite(f32::from(height)),
        };
        tree.compute_layout(root, available);

        // Claim 1: before any transform, the chip renders at its plain
        // untransformed layout box, (20,20)-(60,60) -- center (40,40).
        let (data_before, bpr) = render(&tree, root, width, height).await;
        let before_center = pixel_at(&data_before, bpr, 40, 40);
        assert_eq!(
            before_center,
            [0xFF, 0xA5, 0x00, 0xFF],
            "before any transform, the chip's untransformed center (40,40) must show its \
             own color, got {before_center:?}"
        );

        // Animate the camera's own transform toward a combined pan+zoom
        // target: scale(0.5) about the origin, then translate by
        // (60, 40) -- i.e. Affine::new([0.5, 0, 0, 0.5, 60, 40]) when
        // composed, "pan offset × zoom scale" per §11.9's own text.
        let start = Instant::now();
        let target = Affine::translate((60.0, 40.0)) * Affine::scale(0.5);
        tree.get_mut(camera).unwrap().paint.transform.animate_to(
            target,
            Duration::from_secs(1),
            MotionCurve::Linear,
            start,
        );

        // Halfway through (linear, t=0.5): a componentwise lerp of
        // IDENTITY's [1,0,0,1,0,0] and the target's [0.5,0,0,0.5,60,40]
        // gives [0.75,0,0,0.75,30,20] -- maps a local point (x,y) to
        // (0.75x+30, 0.75y+20). The chip's local box (20,20)-(60,60)
        // therefore maps to canvas (45,35)-(75,65); center (60,50).
        let halfway = start + Duration::from_millis(500);
        let still_active = tree
            .get_mut(camera)
            .unwrap()
            .paint
            .transform
            .tick(halfway, &mut Vec::new());
        assert!(still_active, "transform animation ended early at t=0.5");

        let (data_mid, bpr_mid) = render(&tree, root, width, height).await;

        // Claim 2a: the chip has genuinely moved to the interpolated
        // transformed position.
        let mid_new_center = pixel_at(&data_mid, bpr_mid, 60, 50);
        assert_eq!(
            mid_new_center,
            [0xFF, 0xA5, 0x00, 0xFF],
            "mid-flight, the chip's interpolated transformed center (60,50) must show its \
             own color -- the real claim: a parent Container's animated transform \
             propagates to an untouched child with zero per-descendant code, got \
             {mid_new_center:?}"
        );

        // Claim 2b: it no longer occupies its original untransformed
        // spot -- this isn't a rect that merely got bigger and still
        // covers (40,40), it moved away from it.
        let mid_old_center = pixel_at(&data_mid, bpr_mid, 40, 40);
        assert_eq!(
            mid_old_center,
            [0x11, 0x11, 0x11, 0xFF],
            "mid-flight, the chip's original untransformed center (40,40) must now show \
             plain background -- the chip moved away, got {mid_old_center:?}"
        );
    });
}
