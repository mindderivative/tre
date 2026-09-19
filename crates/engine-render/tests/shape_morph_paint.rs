//! M7 Phase 4 (§7.4): the standalone proof that `PaintProperties.shape`
//! genuinely paints a real MD3 shape morph -- a real, animatable field
//! `paint_node` never read before this phase. Same headless
//! render-to-texture-then-readback discipline as `elevation_shadow.rs`.
//!
//! Two claims, kept deliberately separate:
//!
//! 1. `shape` at its default (`ShapeKey::empty()`, `PaintProperties::
//!    new`'s own default) paints the exact same plain `RoundedRect` as
//!    before this phase -- byte-for-byte, the same "provably a no-op"
//!    standard every additive feature since M5 Phase 1 has been held to.
//! 2. A real, active shape morph paints the morphed silhouette, not the
//!    plain rect -- proven by a triangle whose own corner (outside the
//!    rect's inscribed circle but still inside the rect's own bounding
//!    box) must be background, not fill, at a point the plain rect
//!    would have painted solid.

use engine_core::{NodeKind, PaintProperties, ShapeKey, Tree};
use engine_render::{FrameRenderer, GeometryCache, TextRenderer, build_tree_scene};
use peniko::Color;
use peniko::kurbo::{BezPath, Point};
use taffy::prelude::{AvailableSpace, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const BACKGROUND: Color = Color::from_rgba8(0x11, 0x11, 0x11, 0xFF);
const CHIP: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF);
const BORDER: Color = Color::from_rgba8(0x00, 0xFF, 0x00, 0xFF);

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
            label: Some("engine-render shape-morph-paint test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shape-morph-paint test target"),
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

fn build_tree(shape: ShapeKey) -> (Tree, engine_core::NodeId) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(100.0),
                height: length(100.0),
            },
            ..Default::default()
        },
        PaintProperties::new(BACKGROUND, 0.0, 0.0, 1.0),
    );

    let chip = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(100.0),
                height: length(100.0),
            },
            ..Default::default()
        },
        PaintProperties::new(CHIP, 0.0, 0.0, 1.0),
    );
    tree.add_child(root, chip);
    tree.get_mut(chip).unwrap().paint.shape.current = shape;

    let available = Size {
        width: AvailableSpace::Definite(100.0),
        height: AvailableSpace::Definite(100.0),
    };
    tree.compute_layout(root, available);
    (tree, root)
}

#[test]
fn default_empty_shape_paints_the_plain_rect_unmodified() {
    pollster::block_on(async {
        let (tree, root) = build_tree(ShapeKey::empty());
        let (data, bpr) = render(&tree, root, 100, 100).await;

        // A corner of the 100x100 rect -- solid fill, same as every
        // pre-Phase-4 Rect test already expects.
        assert_eq!(
            pixel_at(&data, bpr, 5, 5),
            [0xFF, 0xFF, 0xFF, 0xFF],
            "an empty (default) shape must paint the exact same plain rect fill as before \
             this phase -- a true no-op"
        );
    });
}

#[test]
fn a_real_active_shape_paints_the_morphed_silhouette_not_the_plain_rect() {
    pollster::block_on(async {
        // A triangle inscribed in the 100x100 box: apex at top-center,
        // base at the bottom corners -- its own top-left area (5,5) is
        // *outside* the triangle even though it's inside the rect's own
        // bounding box, exactly the corner a plain RoundedRect fill
        // would have painted solid.
        let mut path = BezPath::new();
        path.move_to(Point::new(50.0, 0.0));
        path.line_to(Point::new(100.0, 100.0));
        path.line_to(Point::new(0.0, 100.0));
        path.close_path();
        let shape = ShapeKey::from_path(&path);

        let (tree, root) = build_tree(shape);
        let (data, bpr) = render(&tree, root, 100, 100).await;

        let corner = pixel_at(&data, bpr, 5, 5);
        assert_eq!(
            corner,
            [0x11, 0x11, 0x11, 0xFF],
            "a point inside the rect's bounding box but outside the morphed triangle must be \
             plain background, not the chip's fill -- proving paint_node drew the real \
             triangle silhouette, not the plain rect, got {corner:?}"
        );

        let inside = pixel_at(&data, bpr, 50, 90);
        assert_eq!(
            inside,
            [0xFF, 0xFF, 0xFF, 0xFF],
            "a point genuinely inside the triangle must still be the chip's own fill color, \
             got {inside:?}"
        );
    });
}

/// M39 Phase 3 (§5, §7): the real regression test for `ShapeKey::
/// inset_path` -- a border stroked while a real shape morph is active
/// must sit *inside* the fill's own edge, never bleeding past it.
///
/// A 60x60 square shape (`20..80` on both axes) centered in a 100x100
/// box, with a real 20px border (`inset = 10`). Before this phase, the
/// border stroked the raw `20..80` square edge centered, covering
/// `10..30` on the left edge -- a real, visible 10px bleed past the
/// shape's own `x=20` edge into what should be plain background.
/// After the fix, the border strokes the real *inset* `30..70` square
/// centered, covering `20..40` -- entirely inside the shape's own
/// `20..80` bounds.
#[test]
fn a_border_on_a_real_active_shape_morph_stays_inside_the_fills_own_edge() {
    pollster::block_on(async {
        let mut path = BezPath::new();
        path.move_to(Point::new(20.0, 20.0));
        path.line_to(Point::new(80.0, 20.0));
        path.line_to(Point::new(80.0, 80.0));
        path.line_to(Point::new(20.0, 80.0));
        path.close_path();
        let shape = ShapeKey::from_path(&path);

        let mut tree = Tree::new();
        let root = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(100.0),
                    height: length(100.0),
                },
                ..Default::default()
            },
            PaintProperties::new(BACKGROUND, 0.0, 0.0, 1.0),
        );
        let chip = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(100.0),
                    height: length(100.0),
                },
                ..Default::default()
            },
            PaintProperties::new(CHIP, 0.0, 0.0, 1.0),
        );
        tree.add_child(root, chip);
        {
            let node = tree.get_mut(chip).unwrap();
            node.paint.shape.current = shape;
            node.paint.border_width.current = 20.0;
            node.paint.border_color.current = BORDER;
        }
        let available = Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(100.0),
        };
        tree.compute_layout(root, available);

        let (data, bpr) = render(&tree, root, 100, 100).await;

        // x=15, mid-height: within the real pre-fix bleed zone
        // (10..30) but outside the shape's own real `x=20` edge --
        // must be plain background now, never the border color.
        let bled_zone = pixel_at(&data, bpr, 15, 50);
        assert_eq!(
            bled_zone,
            [0x11, 0x11, 0x11, 0xFF],
            "a point outside the shape's own real edge must never be the border color -- \
             the border must not bleed past the fill, got {bled_zone:?}"
        );

        // x=25, mid-height: inside the real post-fix border band
        // (20..40) -- must genuinely be the border color, proving the
        // border still paints *something* real, not just "nothing
        // ever bleeds."
        let real_border = pixel_at(&data, bpr, 25, 50);
        assert_eq!(
            real_border,
            [0x00, 0xFF, 0x00, 0xFF],
            "a point inside the real, inset border band must be the border color, \
             got {real_border:?}"
        );

        // Dead center: well inside the inset fill area, must be the
        // chip's own plain fill, not border.
        let center = pixel_at(&data, bpr, 50, 50);
        assert_eq!(
            center,
            [0xFF, 0xFF, 0xFF, 0xFF],
            "the shape's own real interior must still be the chip's fill color, \
             got {center:?}"
        );
    });
}
