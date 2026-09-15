//! §14 build-order step 15 (§11.7): the standalone pixel-level proof that
//! `Tree::set_virtual_list_window` genuinely paints only its materialized
//! window, at each item's own correct logical position, and genuinely
//! stops painting an item the moment it's recycled out of the window --
//! not just that `VirtualListState.materialized`'s `NodeId` bookkeeping
//! is correct, which `engine-core`'s own unit tests already cover. Same
//! headless render-to-texture-then-readback discipline as every other
//! pixel-level proof in this crate.

use engine_core::{ItemExtent, NodeKind, PaintProperties, Tree, VirtualListState};
use engine_render::{FrameRenderer, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const WIDTH: u16 = 200;
const HEIGHT: u16 = 200;
const ITEM_EXTENT: f64 = 20.0;

fn color_for(idx: usize) -> Color {
    match idx {
        0 => Color::from_rgba8(0xFF, 0x00, 0x00, 0xFF), // red
        1 => Color::from_rgba8(0x00, 0xFF, 0x00, 0xFF), // green
        2 => Color::from_rgba8(0x00, 0x00, 0xFF, 0xFF), // blue
        3 => Color::from_rgba8(0xFF, 0xFF, 0x00, 0xFF), // yellow
        4 => Color::from_rgba8(0xFF, 0x00, 0xFF, 0xFF), // magenta
        5 => Color::from_rgba8(0x00, 0xFF, 0xFF, 0xFF), // cyan
        6 => Color::from_rgba8(0xFF, 0x80, 0x00, 0xFF), // orange
        7 => Color::from_rgba8(0x80, 0x00, 0xFF, 0xFF), // purple
        other => panic!("color_for: no test color defined for index {other}"),
    }
}

fn materialize(idx: usize) -> (NodeKind, Style, PaintProperties) {
    (
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(f32::from(WIDTH)),
                height: length(ITEM_EXTENT as f32),
            },
            ..Default::default()
        },
        PaintProperties::new(color_for(idx), 0.0, 0.0, 1.0),
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
            label: Some("engine-render virtual-list test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("virtual-list test target"),
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

fn as_rgba(color: Color) -> [u8; 4] {
    let [r, g, b, a] = color.to_rgba8().to_u8_array();
    [r, g, b, a]
}

// A region no `fill_path` call ever touches keeps the render target's own
// zero-initialized clear state -- fully transparent, confirmed directly
// against this exact test setup (a `VirtualList` root, which -- like
// `Container` -- paints nothing itself, so a slot outside the
// materialized window is genuinely never painted by anything at all).
// This is a *different* case from step 15 Stage B's own finding
// (`docking.rs`): there, the "uncovered" area was actually covered by a
// real `NodeKind::Rect` fill using a zero-alpha color, which is not the
// same as no fill_path call happening at all -- don't conflate the two
// without re-checking, which is exactly what this comment is doing.
const UNCOVERED: [u8; 4] = [0x00, 0x00, 0x00, 0x00];

#[test]
fn virtual_list_paints_only_its_materialized_window_at_each_items_real_position() {
    pollster::block_on(async {
        let mut tree = Tree::new();
        // item_count is 100,000 -- the whole point of §11.7 is that this
        // never becomes 100,000 real Nodes; only whatever
        // `set_virtual_list_window` is told is visible ever does.
        let list = tree.insert(
            NodeKind::VirtualList(VirtualListState::new(
                100_000,
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

        let available = Size {
            width: AvailableSpace::Definite(f32::from(WIDTH)),
            height: AvailableSpace::Definite(f32::from(HEIGHT)),
        };

        tree.set_virtual_list_window(list, 0..5, materialize);
        tree.compute_layout(list, available);

        let (data, bpr) = render(&tree, list, WIDTH, HEIGHT).await;
        for idx in 0..5 {
            let y = (idx as f64 * ITEM_EXTENT + ITEM_EXTENT / 2.0) as u32;
            let pixel = pixel_at(&data, bpr, 100, y);
            assert_eq!(
                pixel,
                as_rgba(color_for(idx)),
                "materialized item {idx} must paint its own color at its own logical position, y={y}"
            );
        }
        // Index 5 was never asked for -- not a real Node at all, so its
        // slot on screen must show nothing (this pipeline's own real
        // "uncovered" state, established at step 15 Stage B: opaque
        // black, not transparent).
        let unmaterialized = pixel_at(&data, bpr, 100, 110);
        assert_eq!(
            unmaterialized, UNCOVERED,
            "an index outside the requested window must never have been painted at all"
        );

        // Scroll: 0..5 -> 3..8. Indices 0,1,2 are recycled out (real
        // Tree::remove, per engine-core's own NodeId test); 5,6,7 newly
        // enter.
        tree.set_virtual_list_window(list, 3..8, materialize);
        tree.compute_layout(list, available);

        let (data, bpr) = render(&tree, list, WIDTH, HEIGHT).await;

        // The real claim this test exists to prove: item 0's on-screen
        // slot, which showed real red paint a moment ago, must now show
        // nothing -- recycling genuinely stops painting it, it isn't
        // merely bookkeeping that no longer references it.
        let recycled_away = pixel_at(&data, bpr, 100, 10);
        assert_eq!(
            recycled_away, UNCOVERED,
            "item 0's slot must show nothing after it's recycled out of the window"
        );

        // Item 3 stayed in the window across the scroll -- must still
        // show its own color, undisturbed.
        let stayed = pixel_at(&data, bpr, 100, 70);
        assert_eq!(
            stayed,
            as_rgba(color_for(3)),
            "item 3 must be unaffected by scrolling since it never left the window"
        );

        // Items 5, 6, 7 are newly materialized -- must now show their
        // own real paint, where a moment ago there was nothing.
        for idx in 5..8 {
            let y = (idx as f64 * ITEM_EXTENT + ITEM_EXTENT / 2.0) as u32;
            let pixel = pixel_at(&data, bpr, 100, y);
            assert_eq!(
                pixel,
                as_rgba(color_for(idx)),
                "newly materialized item {idx} must paint its own color at y={y}"
            );
        }
    });
}
