//! M32 Phase 6 (§4, §5, §8): the standalone pixel-level proof that a
//! real `Terminal` selection genuinely paints a visible highlight --
//! not just that `TerminalState.selection_start`/`selection_end` are
//! set. Same headless render-to-texture-then-readback discipline as
//! `clip_children.rs`/`virtual_list_scroll.rs`.

use engine_core::{NodeKind, PaintProperties, TerminalCell, TerminalState, Tree};
use engine_render::{FrameRenderer, GeometryCache, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const WIDTH: u16 = 200;
const HEIGHT: u16 = 100;

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
            label: Some("engine-render terminal-selection test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("terminal-selection test target"),
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

/// A single-row, 10-column real terminal, every cell holding a real
/// visible `'x'` glyph on a transparent background -- `with_selection`
/// toggles a real `(0, 8)` selection covering the whole row.
fn build_scene(with_selection: bool) -> (Tree, engine_core::NodeId) {
    let mut tree = Tree::new();
    let mut state = TerminalState::new(10, 1, "Roboto", 16.0);
    for cell in &mut state.cells {
        *cell = TerminalCell {
            ch: 'x',
            fg: Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF),
            bg: Color::TRANSPARENT,
            bold: false,
        };
    }
    if with_selection {
        state.selection_start = Some((0, 0));
        state.selection_end = Some((0, 8));
    }
    let terminal = tree.insert(
        NodeKind::Terminal(state),
        Style {
            size: Size {
                width: length(f32::from(WIDTH)),
                height: length(f32::from(HEIGHT)),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0xFF), 0.0, 0.0, 1.0),
    );
    (tree, terminal)
}

#[test]
fn a_real_selection_paints_genuinely_different_pixels_than_no_selection() {
    pollster::block_on(async {
        let (with_sel, terminal_with) = build_scene(true);
        let (no_sel, terminal_without) = build_scene(false);

        let (data_with, bpr_with) = render(&with_sel, terminal_with).await;
        let (data_without, bpr_without) = render(&no_sel, terminal_without).await;

        // A point inside a selected cell's own background area (not on
        // a glyph stroke, which both renders would share) -- near the
        // top-left corner of cell (0, 2).
        let x = 2 * 16 + 2; // cell width ~16px @ 16pt monospace, +2px margin
        let y = 2;
        let with_pixel = pixel_at(&data_with, bpr_with, x as u32, y);
        let without_pixel = pixel_at(&data_without, bpr_without, x as u32, y);
        assert_ne!(
            with_pixel, without_pixel,
            "a real selection must genuinely tint a selected cell's own background \
             differently than the identical unselected render, got {with_pixel:?} == \
             {without_pixel:?}"
        );
    });
}

#[test]
fn a_collapsed_selection_paints_identically_to_no_selection() {
    pollster::block_on(async {
        let (mut collapsed, terminal_collapsed) = build_scene(false);
        if let NodeKind::Terminal(state) = &mut collapsed.get_mut(terminal_collapsed).unwrap().kind
        {
            state.selection_start = Some((0, 3));
            state.selection_end = Some((0, 3));
        }
        let (no_sel, terminal_without) = build_scene(false);

        let (data_collapsed, bpr_collapsed) = render(&collapsed, terminal_collapsed).await;
        let (data_without, bpr_without) = render(&no_sel, terminal_without).await;

        let x = 3 * 16 + 2;
        let y = 2;
        assert_eq!(
            pixel_at(&data_collapsed, bpr_collapsed, x as u32, y),
            pixel_at(&data_without, bpr_without, x as u32, y),
            "a collapsed (start == end) selection must be a true no-op, painting nothing"
        );
    });
}
