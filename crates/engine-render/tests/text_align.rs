//! M30 Phase 1 (§5, §7): proves `TextState.align` genuinely repositions
//! glyph ink, not just that it compiles and threads through as an inert
//! field -- the real prerequisite for `Button`'s own centered label
//! (MD3's anatomy for every button variant). Same headless render-to-
//! texture-then-readback discipline and the same `Readback`/`has_ink_in`
//! coarse-ink-presence signal `text_layout.rs` already established for
//! anti-aliased glyph edges (an exact pixel match is brittle for text;
//! "ink is concentrated on this side of the box, not that side" is the
//! real, geometric claim this test needs, mirroring `text_layout.rs`'s
//! own RTL left/right assertions).

use engine_core::{NodeKind, PaintProperties, TextAlign, TextState, Tree};
use engine_render::{FrameRenderer, GeometryCache, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const WIDTH: u16 = 300;
const HEIGHT: u16 = 60;
const TEXT_COLOR: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF);

struct Readback {
    data: Vec<u8>,
    bytes_per_row: u32,
}

impl Readback {
    fn pixel_at(&self, x: u32, y: u32) -> [u8; 4] {
        let row_start = (y * self.bytes_per_row) as usize;
        let px_start = row_start + (x * 4) as usize;
        [
            self.data[px_start],
            self.data[px_start + 1],
            self.data[px_start + 2],
            self.data[px_start + 3],
        ]
    }

    fn has_ink_in(&self, x0: u32, x1: u32, y0: u32, y1: u32) -> bool {
        (y0..y1)
            .step_by(4)
            .any(|y| (x0..x1).step_by(4).any(|x| self.pixel_at(x, y)[3] > 0))
    }
}

/// Renders `"Ok"` (a short, real button-label-shaped string) at `align`
/// in a box much wider than the text itself, and reads the result back.
async fn render(align: TextAlign) -> Readback {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Text(TextState {
            content: "Ok".to_string(),
            font_family: "Roboto".to_string(),
            font_weight: 500.0,
            font_size: 20.0,
            align,
            line_height: None,
        }),
        Style {
            size: Size {
                width: length(f32::from(WIDTH)),
                height: length(f32::from(HEIGHT)),
            },
            ..Default::default()
        },
        PaintProperties::new(TEXT_COLOR, 0.0, 0.0, 1.0),
    );
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(f32::from(WIDTH)),
            height: AvailableSpace::Definite(f32::from(HEIGHT)),
        },
    );

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
            label: Some("engine-render text-align test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("text-align test target"),
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
        &tree,
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
    let readback_buf = device.create_buffer(&wgpu::BufferDescriptor {
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
            buffer: &readback_buf,
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

    let slice = readback_buf.slice(..);
    slice.map_async(wgpu::MapMode::Read, |result| {
        result.expect("failed to map readback buffer");
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("device poll failed");
    let data = slice.get_mapped_range().to_vec();
    Readback {
        data,
        bytes_per_row,
    }
}

#[test]
fn start_aligned_text_hugs_the_left_edge() {
    pollster::block_on(async {
        let readback = render(TextAlign::Start).await;
        assert!(
            readback.has_ink_in(0, 30, 0, u32::from(HEIGHT)),
            "TextAlign::Start must paint ink flush against the box's left edge"
        );
        assert!(
            !readback.has_ink_in(
                u32::from(WIDTH) - 100,
                u32::from(WIDTH),
                0,
                u32::from(HEIGHT)
            ),
            "TextAlign::Start must not paint ink out near the box's right side, \
             for a string this short in a box this wide"
        );
    });
}

#[test]
fn center_aligned_text_moves_ink_away_from_the_left_edge_toward_the_middle() {
    pollster::block_on(async {
        let readback = render(TextAlign::Center).await;
        assert!(
            !readback.has_ink_in(0, 30, 0, u32::from(HEIGHT)),
            "TextAlign::Center must not leave ink flush against the box's left \
             edge for a short string in a box this wide -- looks like Start \
             alignment silently ran instead"
        );
        let mid = u32::from(WIDTH) / 2;
        assert!(
            readback.has_ink_in(mid - 40, mid + 40, 0, u32::from(HEIGHT)),
            "TextAlign::Center must paint ink near the box's horizontal middle"
        );
    });
}
