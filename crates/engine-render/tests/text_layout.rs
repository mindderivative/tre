//! §14 build-order step 4: the typography spike. Proves `parley`'s
//! shaping actually reaches the screen through `engine-core::Tree` +
//! `engine-render::build_tree_scene`, for real MD3-ish type roles
//! (distinct Roboto weights/sizes) and a non-Latin, right-to-left string
//! (Arabic, exercising BiDi and font-fallback-free real shaping since
//! the font is registered directly, not discovered).
//!
//! Same headless render-to-texture-then-readback discipline as
//! `layout_tree.rs`/`animated_rect.rs`, calibrated differently: a solid
//! rect's fill is an exact flat color over an exact geometric region, so
//! those tests assert exact pixel values. Anti-aliased glyph edges make
//! that approach brittle for text, so here the signal is coarser but
//! still real and still geometric: "ink exists somewhere in this node's
//! laid-out box" (proves shaping+drawing actually ran, not a silent
//! no-op) and, for the RTL string specifically, "ink is on the right
//! side of its box, not the left" (proves BiDi positioning actually
//! took effect, not just that *a* glyph got drawn somewhere).

use engine_core::{NodeKind, PaintProperties, TextAlign, TextState, Tree};
use engine_render::{FrameRenderer, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{AvailableSpace, FlexDirection, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const WIDTH: u16 = 400;
const HEIGHT: u16 = 200;
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

    /// True if any sampled point in `[x0, x1) x [y0, y1)` (a 4px stride
    /// grid -- fine enough to catch real glyph ink, coarse enough to
    /// stay fast) is non-transparent.
    fn has_ink_in(&self, x0: u32, x1: u32, y0: u32, y1: u32) -> bool {
        (y0..y1)
            .step_by(4)
            .any(|y| (x0..x1).step_by(4).any(|x| self.pixel_at(x, y)[3] > 0))
    }
}

#[test]
fn type_roles_and_rtl_string_render_real_ink() {
    pollster::block_on(async {
        let mut tree = Tree::new();
        let root = tree.insert(
            NodeKind::Container,
            Style {
                display: taffy::Display::Flex,
                flex_direction: FlexDirection::Column,
                size: Size {
                    width: length(f32::from(WIDTH)),
                    height: length(f32::from(HEIGHT)),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );

        let body = tree.insert(
            NodeKind::Text(TextState {
                content: "Body text at a real Roboto Regular size.".to_string(),
                font_family: "Roboto".to_string(),
                font_weight: 400.0,
                font_size: 16.0,
                align: TextAlign::Start,
            }),
            Style {
                size: Size {
                    width: length(f32::from(WIDTH)),
                    height: length(30.0),
                },
                ..Default::default()
            },
            PaintProperties::new(TEXT_COLOR, 0.0, 0.0, 1.0),
        );
        let headline = tree.insert(
            NodeKind::Text(TextState {
                content: "Headline".to_string(),
                font_family: "Roboto".to_string(),
                font_weight: 500.0,
                font_size: 32.0,
                align: TextAlign::Start,
            }),
            Style {
                size: Size {
                    width: length(f32::from(WIDTH)),
                    height: length(50.0),
                },
                ..Default::default()
            },
            PaintProperties::new(TEXT_COLOR, 0.0, 0.0, 1.0),
        );
        let arabic = tree.insert(
            NodeKind::Text(TextState {
                content: "مرحبا بالعالم".to_string(),
                font_family: "Noto Sans Arabic".to_string(),
                font_weight: 400.0,
                font_size: 24.0,
                align: TextAlign::Start,
            }),
            Style {
                size: Size {
                    width: length(f32::from(WIDTH)),
                    height: length(40.0),
                },
                ..Default::default()
            },
            PaintProperties::new(TEXT_COLOR, 0.0, 0.0, 1.0),
        );
        tree.add_child(root, body);
        tree.add_child(root, headline);
        tree.add_child(root, arabic);

        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(f32::from(WIDTH)),
                height: AvailableSpace::Definite(f32::from(HEIGHT)),
            },
        );
        let body_box = *tree.layout(body);
        let headline_box = *tree.layout(headline);
        let arabic_box = *tree.layout(arabic);

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
                label: Some("engine-render text-layout test device"),
                required_features: wgpu::Features::empty(),
                ..Default::default()
            })
            .await
            .expect("failed to create wgpu device");

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("text-layout test target"),
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
        let scene = build_tree_scene(
            &tree,
            root,
            WIDTH,
            HEIGHT,
            frame_renderer.resources_mut(),
            &mut text_renderer,
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
        let readback = Readback {
            data,
            bytes_per_row,
        };

        // Each type role actually drew something inside its own
        // taffy-computed box -- proves parley shaping + glifo glyph
        // conversion + vello_hybrid atlas rendering are wired together
        // end to end, not silently no-op-ing.
        assert!(
            readback.has_ink_in(
                0,
                u32::from(WIDTH),
                body_box.location.y as u32,
                (body_box.location.y + body_box.size.height) as u32,
            ),
            "Body role (Roboto Regular, 16px) drew no ink in its own box"
        );
        assert!(
            readback.has_ink_in(
                0,
                u32::from(WIDTH),
                headline_box.location.y as u32,
                (headline_box.location.y + headline_box.size.height) as u32,
            ),
            "Headline role (Roboto Medium, 32px) drew no ink in its own box"
        );

        // The RTL string: ink must be concentrated on the right side of
        // its box (where an RTL paragraph starts), not the left -- the
        // actual claim this spike exists to prove about parley's BiDi
        // handling, not just "some glyph rendered somewhere."
        let arabic_y0 = arabic_box.location.y as u32;
        let arabic_y1 = (arabic_box.location.y + arabic_box.size.height) as u32;
        let box_right = u32::from(WIDTH);
        assert!(
            readback.has_ink_in(box_right - 40, box_right, arabic_y0, arabic_y1),
            "Arabic (RTL) text has no ink near the right edge of its box -- \
             expected an RTL paragraph to start from the right"
        );
        assert!(
            !readback.has_ink_in(0, 40, arabic_y0, arabic_y1),
            "Arabic (RTL) text has ink flush against the left edge of its box -- \
             looks like it was placed as if it were LTR, not shaped RTL"
        );
    });
}
