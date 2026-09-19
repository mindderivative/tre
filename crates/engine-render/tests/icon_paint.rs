//! M23 Phase 1 (§1, §3): the standalone proof that `NodeKind::Icon`
//! genuinely paints a real, correctly-transformed vector fill -- not
//! just that the match arm compiles. Same headless render-to-texture-
//! then-readback discipline as `checkbox_paint.rs`/`image_paint.rs`.
//!
//! Uses a synthesized, hand-built `BezPath` in the icon's own real SVG
//! source space (`viewBox="0 -960 960 960"`, `x: 0..960, y: -960..0`)
//! covering only the *left half* of that space -- not a fetched real
//! icon -- deliberately asymmetric so a real, decisive claim about the
//! `paint_node` transform (scale, and the real negative-`y` ->
//! `0..960` translate) can be tested: a correct transform paints only
//! the node's own left half; a wrong translate (e.g. the shape ending
//! up entirely outside the node's visible box) would paint neither
//! half, not "the wrong half" -- so this is a genuine geometry proof,
//! not merely a color-landed-somewhere check.

use engine_core::{ICON_VIEWBOX_SIZE, IconState, NodeKind, PaintProperties, Tree};
use engine_render::{FrameRenderer, GeometryCache, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const BACKGROUND: Color = Color::from_rgba8(0x11, 0x11, 0x11, 0xFF);
const TINT: Color = Color::from_rgba8(0x1C, 0x1B, 0x1F, 0xFF);

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
            label: Some("engine-render icon-paint test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("icon-paint test target"),
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

/// The left half of the real `viewBox="0 -960 960 960"` source space
/// (`x: 0..480, y: -960..0`) -- a real, hand-built `BezPath`, not a
/// fetched icon.
fn left_half_square() -> peniko::kurbo::BezPath {
    let half = ICON_VIEWBOX_SIZE / 2.0;
    peniko::kurbo::BezPath::from_svg(&format!(
        "M0,{neg} L{half},{neg} L{half},0 L0,0 Z",
        neg = -ICON_VIEWBOX_SIZE
    ))
    .expect("a real, hand-built SVG path must parse")
}

fn build_tree() -> (Tree, engine_core::NodeId) {
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

    let icon = tree.insert(
        NodeKind::Icon(IconState::new(left_half_square(), TINT)),
        Style {
            size: Size {
                width: length(100.0),
                height: length(100.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
    );
    tree.add_child(root, icon);

    let available = Size {
        width: AvailableSpace::Definite(100.0),
        height: AvailableSpace::Definite(100.0),
    };
    tree.compute_layout(root, available);
    (tree, root)
}

#[test]
fn an_icon_node_paints_only_its_own_real_left_half() {
    pollster::block_on(async {
        let (tree, root) = build_tree();
        let (data, bpr) = render(&tree, root, 100, 100).await;

        // Well inside the left half (x < 50), at the vertical center --
        // far from any edge-sampling softness.
        let left = pixel_at(&data, bpr, 20, 50);
        assert_eq!(
            left,
            [0x1C, 0x1B, 0x1F, 0xFF],
            "the icon's own real left-half path must paint its real tint on the node's \
             left side, got {left:?} -- a wrong transform would leave this untouched too"
        );

        // Well inside the right half (x > 50) -- must show only plain
        // background, proving the transform didn't paint the whole box
        // (which would make the left-half claim above meaningless).
        let right = pixel_at(&data, bpr, 80, 50);
        assert_eq!(
            right,
            [0x11, 0x11, 0x11, 0xFF],
            "the node's own right half must show only plain background, got {right:?}"
        );
    });
}

/// M35 Phase 2 (§5, §8): the real, decisive pixel-level proof
/// `IconState.rotation` actually rotates the painted output, not just
/// that the field compiles -- reuses the identical asymmetric left-
/// half icon above, so a real 180° rotation must paint the *right*
/// half instead of the left, the same "a wrong transform paints
/// neither half, not the wrong half" decisive-proof discipline the
/// un-rotated test above already established.
#[test]
fn a_rotated_icon_node_paints_its_own_real_right_half_instead() {
    pollster::block_on(async {
        let (mut tree, root) = build_tree();
        let icon = tree
            .get(root)
            .expect("root must exist")
            .children
            .first()
            .copied()
            .expect("root must have the real icon child");
        let node = tree.get_mut(icon).expect("icon node must exist");
        let NodeKind::Icon(state) = &mut node.kind else {
            panic!("expected NodeKind::Icon");
        };
        state.rotation.current = 180.0;

        let (data, bpr) = render(&tree, root, 100, 100).await;

        let left = pixel_at(&data, bpr, 20, 50);
        assert_eq!(
            left,
            [0x11, 0x11, 0x11, 0xFF],
            "after a real 180° rotation, the node's own left half must show only plain \
             background, got {left:?}"
        );

        let right = pixel_at(&data, bpr, 80, 50);
        assert_eq!(
            right,
            [0x1C, 0x1B, 0x1F, 0xFF],
            "after a real 180° rotation, the icon's own tint must land on the node's own \
             right half instead, got {right:?}"
        );
    });
}
