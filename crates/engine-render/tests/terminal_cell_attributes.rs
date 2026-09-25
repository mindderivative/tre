//! M39 Phase 4 (§5, §7): the standalone pixel-level proof that
//! `TerminalCell`'s own four new real attributes (`dim`/`italic`/
//! `underline`/`inverse`) genuinely change what gets painted -- not
//! just that the fields exist and round-trip. Same headless
//! render-to-texture-then-readback discipline as `terminal_selection.
//! rs`, whose own harness this file mirrors verbatim.

use engine_core::{CellColor, NodeKind, PaintProperties, TerminalCell, TerminalState, Tree};
use engine_render::{FrameRenderer, GeometryCache, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const WIDTH: u16 = 200;
const HEIGHT: u16 = 100;
const WHITE: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF);
const BLUE: Color = Color::from_rgba8(0x00, 0x00, 0xFF, 0xFF);
const CONTAINER_BG: Color = Color::from_rgba8(0, 0, 0, 0xFF);

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
            label: Some("engine-render terminal-cell-attributes test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("terminal-cell-attributes test target"),
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

/// A single-row, 10-column real terminal, every cell holding `cell`
/// verbatim (so the whole visible row reflects one real, uniform
/// style) -- `terminal_selection.rs`'s own `build_scene`, parameterized
/// over the cell itself instead of always a plain `'x'`.
fn build_scene(cell: TerminalCell) -> (Tree, engine_core::NodeId) {
    let mut tree = Tree::new();
    let mut state = TerminalState::new(10, 1, "Roboto", 16.0);
    state.cells.fill(cell);
    let terminal = tree.insert(
        NodeKind::Terminal(state),
        Style {
            size: Size {
                width: length(f32::from(WIDTH)),
                height: length(f32::from(HEIGHT)),
            },
            ..Default::default()
        },
        PaintProperties::new(CONTAINER_BG, 0.0, 0.0, 1.0),
    );
    let available = Size {
        width: AvailableSpace::Definite(f32::from(WIDTH)),
        height: AvailableSpace::Definite(f32::from(HEIGHT)),
    };
    tree.compute_layout(terminal, available);
    (tree, terminal)
}

fn base_cell() -> TerminalCell {
    TerminalCell {
        ch: 'x',
        fg: CellColor::Rgb(WHITE),
        bg: CellColor::Default,
        bold: false,
        dim: false,
        italic: false,
        underline: false,
        inverse: false,
    }
}

#[test]
fn dim_paints_genuinely_different_pixels_than_full_opacity() {
    pollster::block_on(async {
        let (plain, plain_terminal) = build_scene(base_cell());
        let (dimmed, dimmed_terminal) = build_scene(TerminalCell {
            dim: true,
            ..base_cell()
        });

        let (data_plain, _bpr_plain) = render(&plain, plain_terminal).await;
        let (data_dim, _bpr_dim) = render(&dimmed, dimmed_terminal).await;

        assert_ne!(
            data_plain, data_dim,
            "a real dim cell must paint its glyph at reduced opacity, genuinely \
             different from the identical full-opacity render"
        );
    });
}

#[test]
fn italic_paints_genuinely_different_pixels_than_upright_via_the_real_synthetic_shear() {
    pollster::block_on(async {
        let (upright, upright_terminal) = build_scene(base_cell());
        let (slanted, slanted_terminal) = build_scene(TerminalCell {
            italic: true,
            ..base_cell()
        });

        let (data_upright, _bpr_upright) = render(&upright, upright_terminal).await;
        let (data_italic, _bpr_italic) = render(&slanted, slanted_terminal).await;

        assert_ne!(
            data_upright, data_italic,
            "a real italic cell must paint its glyph through a real synthetic shear \
             (glifo::GlyphRunBuilder::glyph_transform), genuinely different pixels \
             from the identical upright render"
        );
    });
}

#[test]
fn underline_paints_a_real_visible_rule_a_plain_cell_does_not() {
    pollster::block_on(async {
        let (plain, plain_terminal) = build_scene(base_cell());
        let (underlined, underlined_terminal) = build_scene(TerminalCell {
            underline: true,
            ..base_cell()
        });

        let (data_plain, bpr_plain) = render(&plain, plain_terminal).await;
        let (data_underline, bpr_underline) = render(&underlined, underlined_terminal).await;

        // Row height at 16pt monospace is comfortably over 16px; the
        // real underline rule sits at `0.85 * cell_height` -- well
        // below any real glyph stroke, so a plain cell's own pixel
        // there is still the container's own opaque black background,
        // while the underlined cell's own pixel there is real,
        // painted ink.
        let y = 17;
        let x = 4;
        let plain_pixel = pixel_at(&data_plain, bpr_plain, x, y);
        let underline_pixel = pixel_at(&data_underline, bpr_underline, x, y);
        assert_eq!(
            plain_pixel,
            [0, 0, 0, 0xFF],
            "sanity check: a plain cell's own row-bottom pixel must still be the \
             container's own opaque black background, got {plain_pixel:?}"
        );
        assert_ne!(
            plain_pixel, underline_pixel,
            "a real underlined cell must paint a real visible rule near the row's \
             own bottom, genuinely different from the plain (unpainted) background \
             there, got {underline_pixel:?}"
        );
    });
}

#[test]
fn inverse_paints_the_swapped_foreground_as_a_real_solid_background_block_when_the_cells_own_background_was_unset()
 {
    pollster::block_on(async {
        let (plain, plain_terminal) = build_scene(base_cell());
        let (inverted, inverted_terminal) = build_scene(TerminalCell {
            inverse: true,
            ..base_cell()
        });

        let (data_plain, bpr_plain) = render(&plain, plain_terminal).await;
        let (data_inverse, bpr_inverse) = render(&inverted, inverted_terminal).await;

        // A corner of the first cell, off any real glyph stroke --
        // the plain cell's own real transparent bg lets the
        // container's opaque black show through; the inverted cell's
        // own real swapped bg (the cell's own original white fg)
        // must paint a real solid block there instead.
        let (x, y) = (1, 1);
        let plain_corner = pixel_at(&data_plain, bpr_plain, x, y);
        let inverse_corner = pixel_at(&data_inverse, bpr_inverse, x, y);
        assert_eq!(
            plain_corner,
            [0, 0, 0, 0xFF],
            "sanity check: a plain cell's own corner must still be the container's \
             own real background, got {plain_corner:?}"
        );
        assert_eq!(
            inverse_corner,
            [0xFF, 0xFF, 0xFF, 0xFF],
            "a real inverse cell with no real background of its own must paint its \
             own real foreground color as a solid background block instead \
             (`terminal_cell_effective_colors`'s own real swap), got {inverse_corner:?}"
        );
    });
}

#[test]
fn inverse_swaps_a_real_explicit_background_and_foreground() {
    pollster::block_on(async {
        let cell = TerminalCell {
            bg: CellColor::Rgb(BLUE),
            ..base_cell()
        };
        let (plain, plain_terminal) = build_scene(cell);
        let (inverted, inverted_terminal) = build_scene(TerminalCell {
            inverse: true,
            ..cell
        });

        let (data_plain, bpr_plain) = render(&plain, plain_terminal).await;
        let (data_inverse, bpr_inverse) = render(&inverted, inverted_terminal).await;

        let (x, y) = (1, 1);
        let plain_corner = pixel_at(&data_plain, bpr_plain, x, y);
        let inverse_corner = pixel_at(&data_inverse, bpr_inverse, x, y);
        assert_eq!(
            plain_corner,
            [0x00, 0x00, 0xFF, 0xFF],
            "sanity check: the plain cell's own corner must be its own real, \
             explicit blue background, got {plain_corner:?}"
        );
        assert_eq!(
            inverse_corner,
            [0xFF, 0xFF, 0xFF, 0xFF],
            "a real inverse cell with an explicit background must swap to its own \
             real foreground (white) as the new background, got {inverse_corner:?}"
        );
    });
}
