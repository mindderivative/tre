//! M32 Phase 3 (§5, §7, §11.7/§11.8): the standalone pixel-level proof
//! that `PaintProperties.clip_children` genuinely clips an oversized
//! child to its own parent's real box -- the real, general form of the
//! clip `VirtualList`/`Carousel` each already have, proven the
//! identical headless render-to-texture-then-readback way
//! `virtual_list_scroll.rs` already proves theirs. Also proves the
//! real, necessary negative: `clip_children: false` (every existing
//! node, unchanged) must still let an oversized child paint past its
//! own parent's box exactly as it always did -- a true no-op, not a
//! silent behavior change for every node that never opts in.

use engine_core::{NodeKind, PaintProperties, Tree};
use engine_render::{FrameRenderer, GeometryCache, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const WIDTH: u16 = 100;
const HEIGHT: u16 = 100;
const CHIP: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF);
const UNCOVERED: [u8; 4] = [0x00, 0x00, 0x00, 0x00];

async fn render(tree: &Tree, root: engine_core::NodeId) -> (Vec<u8>, u32) {
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
            label: Some("engine-render clip-children test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("clip-children test target"),
        size: wgpu::Extent3d {
            width: u32::from(WIDTH),
            height: u32::from(HEIGHT),
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
            width: u32::from(WIDTH),
            height: u32::from(HEIGHT),
        },
    );
    let mut text_renderer = TextRenderer::new();
    let mut geometry_cache = GeometryCache::new();
    let scene = build_tree_scene(
        tree,
        root,
        WIDTH,
        HEIGHT,
        frame_renderer.resources_mut(),
        &mut text_renderer,
        &mut geometry_cache,
    );
    let render_size = RenderSize {
        width: u32::from(WIDTH),
        height: u32::from(HEIGHT),
    };
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    frame_renderer.render(&scene, &device, &queue, &mut encoder, &render_size, &view);

    let bytes_per_row = (u32::from(WIDTH) * 4).next_multiple_of(256);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(bytes_per_row) * u64::from(HEIGHT),
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
            width: u32::from(WIDTH),
            height: u32::from(HEIGHT),
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

/// A 50x50 parent at the origin with a single 50x200 child (four times
/// its own parent's real height) -- `clip_container` toggles the new
/// `PaintProperties.clip_children` on the parent.
fn build_scene(clip_container: bool) -> (Tree, engine_core::NodeId) {
    let mut tree = Tree::new();
    let mut paint = PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0);
    paint.clip_children = clip_container;
    let parent = tree.insert(
        NodeKind::Container,
        Style {
            size: Size {
                width: length(50.0),
                height: length(50.0),
            },
            ..Default::default()
        },
        paint,
    );
    let child = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(50.0),
                height: length(200.0),
            },
            ..Default::default()
        },
        PaintProperties::new(CHIP, 0.0, 0.0, 1.0),
    );
    tree.add_child(parent, child);
    tree.compute_layout(
        parent,
        Size {
            width: AvailableSpace::Definite(f32::from(WIDTH)),
            height: AvailableSpace::Definite(f32::from(HEIGHT)),
        },
    );
    (tree, parent)
}

#[test]
fn clip_children_true_genuinely_hides_an_oversized_child_past_the_parents_own_box() {
    pollster::block_on(async {
        let (tree, parent) = build_scene(true);
        let (data, bpr) = render(&tree, parent).await;

        // Inside the parent's own 50x50 box: real paint.
        let inside = pixel_at(&data, bpr, 25, 25);
        assert_eq!(
            inside,
            [0xFF, 0xFF, 0xFF, 0xFF],
            "the child's own real paint must still show inside the parent's own box, got \
             {inside:?}"
        );

        // Past the parent's own 50px-tall box (y=90, well inside the
        // child's own real 200px height): must be genuinely clipped.
        let past_parent = pixel_at(&data, bpr, 25, 90);
        assert_eq!(
            past_parent, UNCOVERED,
            "clip_children: true must genuinely hide a child past its own parent's real box, \
             got {past_parent:?}"
        );
    });
}

#[test]
fn clip_children_false_is_a_true_no_op_the_oversized_child_still_paints_past_the_parent() {
    pollster::block_on(async {
        let (tree, parent) = build_scene(false);
        let (data, bpr) = render(&tree, parent).await;

        // The identical real point (y=90, past the parent's own 50px-
        // tall box) must still show real paint -- every existing node
        // in this codebase defaults to `clip_children: false`, and this
        // phase must not silently change any of their real behavior.
        let past_parent = pixel_at(&data, bpr, 25, 90);
        assert_eq!(
            past_parent,
            [0xFF, 0xFF, 0xFF, 0xFF],
            "clip_children: false (the default) must be a true no-op -- an oversized child must \
             still paint past its own parent's box exactly as before this phase, got \
             {past_parent:?}"
        );
    });
}
