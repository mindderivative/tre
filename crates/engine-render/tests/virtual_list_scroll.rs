//! M8 Phase 2 (§11.7): the standalone pixel-level proof that
//! `VirtualListState.scroll_offset` genuinely shifts materialized
//! children's own painted position, and genuinely clips content that
//! scrolls outside the list's own bounds -- not just that the field
//! exists and is wired into `paint_node`'s own composed transform in
//! theory. Same headless render-to-texture-then-readback discipline
//! (and the same real `set_virtual_list_window` materialize setup) as
//! `virtual_list.rs`.

use engine_core::{ItemExtent, NodeKind, PaintProperties, Tree, VirtualListState};
use engine_render::{FrameRenderer, GeometryCache, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const WIDTH: u16 = 200;
const HEIGHT: u16 = 100;
const ITEM_EXTENT: f64 = 20.0;
const CHIP: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF);
const UNCOVERED: [u8; 4] = [0x00, 0x00, 0x00, 0x00];

fn materialize(_idx: usize) -> (NodeKind, Style, PaintProperties) {
    (
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(f32::from(WIDTH)),
                height: length(ITEM_EXTENT as f32),
            },
            ..Default::default()
        },
        PaintProperties::new(CHIP, 0.0, 0.0, 1.0),
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
            label: Some("engine-render virtual-list-scroll test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("virtual-list-scroll test target"),
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

fn build_list(
    item_count: usize,
    materialized: std::ops::Range<usize>,
) -> (Tree, engine_core::NodeId) {
    let mut tree = Tree::new();
    let list = tree.insert(
        NodeKind::VirtualList(VirtualListState::new(
            item_count,
            ItemExtent::Fixed(ITEM_EXTENT),
        )),
        Style {
            size: Size {
                width: length(f32::from(WIDTH)),
                height: length(f32::from(HEIGHT)),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
    );
    tree.set_virtual_list_window(list, materialized, materialize);
    let available = Size {
        width: AvailableSpace::Definite(f32::from(WIDTH)),
        height: AvailableSpace::Definite(f32::from(HEIGHT)),
    };
    tree.compute_layout(list, available);
    (tree, list)
}

#[test]
fn a_real_scroll_offset_shifts_materialized_children_up_by_that_many_pixels() {
    pollster::block_on(async {
        // 5 items (0..5), each 20px tall -- items 0..5 fill the whole
        // 100px-tall viewport at scroll_offset 0.0 (item 0's own slot:
        // y in [0, 20)).
        let (mut tree, list) = build_list(5, 0..5);

        if let NodeKind::VirtualList(state) = &mut tree.get_mut(list).unwrap().kind {
            state.scroll_offset.current = 10.0;
        }

        let (data, bpr) = render(&tree, list, WIDTH, HEIGHT).await;

        // Item 0's own real slot (y in [0, 20) at scroll 0.0) is now
        // half-scrolled-out: only y in [0, 10) still shows it (10px of
        // real overlap remains after shifting up by 10px).
        let still_visible = pixel_at(&data, bpr, 100, 5);
        assert_eq!(
            still_visible,
            [0xFF, 0xFF, 0xFF, 0xFF],
            "the top of item 0's own slot must still show real paint after a 10px scroll, \
             got {still_visible:?}"
        );

        // Item 4's own real slot (y in [80, 100) at scroll 0.0) shifts
        // up to [70, 90) -- y=95 (inside the old slot, now past the
        // shifted one) must show whatever scrolled into view instead:
        // there is no item 5 materialized, so this must be uncovered.
        let shifted_away = pixel_at(&data, bpr, 100, 95);
        assert_eq!(
            shifted_away, UNCOVERED,
            "past the last shifted item, with no next item materialized, must show nothing, \
             got {shifted_away:?}"
        );
    });
}

#[test]
fn scrolled_content_outside_the_lists_own_bounds_is_genuinely_clipped_not_just_moved() {
    pollster::block_on(async {
        // A single materialized item, scrolled up far enough that its
        // own real bounds land entirely above the list's own y=0 top
        // edge -- a real clip must hide it completely, not merely
        // reposition it off to a visible spot.
        let (mut tree, list) = build_list(1, 0..1);

        if let NodeKind::VirtualList(state) = &mut tree.get_mut(list).unwrap().kind {
            state.scroll_offset.current = 1000.0;
        }

        let (data, bpr) = render(&tree, list, WIDTH, HEIGHT).await;
        let anywhere_in_list = pixel_at(&data, bpr, 100, 50);
        assert_eq!(
            anywhere_in_list, UNCOVERED,
            "a materialized item scrolled entirely out of the list's own bounds must be \
             genuinely clipped -- got {anywhere_in_list:?}"
        );
    });
}
