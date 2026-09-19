//! §14 step 5: proves a `view.yaml` parsed by `engine-spec` actually
//! renders through the real Phase 2 pipeline (steps 1-4) this crate
//! already built -- "de-risks parsing/validation/mapping in isolation
//! ... before anything depends on it working," per the step's own text.
//! `engine-spec`'s own unit tests already prove the `WidgetSpec` ->
//! `Tree` mapping is correct in isolation (exact node kinds, exact
//! taffy positions); this test's only remaining job is proving that
//! *built* `Tree` composes with `build_tree_scene`/`FrameRenderer`
//! exactly like a hand-built one does -- same headless
//! render-to-texture-then-readback discipline as `layout_tree.rs`/
//! `text_layout.rs`.

use engine_render::{FrameRenderer, GeometryCache, TextRenderer, build_tree_scene};
use engine_spec::load_view;
use vello_hybrid::{RenderSize, RenderTargetConfig};

const VIEW_YAML: &str = include_str!("../../engine-spec/examples/view.yaml");
const WIDTH: u16 = 300;
const HEIGHT: u16 = 100;

#[test]
fn a_real_view_yaml_renders_through_the_real_pipeline() {
    pollster::block_on(async {
        let mut tree = engine_core::Tree::new();
        let root =
            load_view(&mut tree, VIEW_YAML).expect("examples/view.yaml must parse and build");
        tree.compute_layout(
            root,
            taffy::prelude::Size {
                width: taffy::prelude::AvailableSpace::Definite(f32::from(WIDTH)),
                height: taffy::prelude::AvailableSpace::Definite(f32::from(HEIGHT)),
            },
        );

        let root_node = tree.get(root).expect("root must exist");
        assert_eq!(
            root_node.children.len(),
            2,
            "view.yaml must have swatch + label children"
        );
        let swatch = root_node.children[0];
        let label = root_node.children[1];
        let swatch_layout = *tree.layout(swatch);
        let label_layout = *tree.layout(label);

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
                label: Some("engine-render spec-view test device"),
                required_features: wgpu::Features::empty(),
                ..Default::default()
            })
            .await
            .expect("failed to create wgpu device");

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("spec-view test target"),
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
        let data = slice.get_mapped_range();
        let pixel_at = |x: u32, y: u32| -> [u8; 4] {
            let row_start = (y * bytes_per_row) as usize;
            let px_start = row_start + (x * 4) as usize;
            [
                data[px_start],
                data[px_start + 1],
                data[px_start + 2],
                data[px_start + 3],
            ]
        };

        // The swatch's own center pixel is exactly view.yaml's declared
        // color -- an exact match, the same standard as every other
        // solid-rect test in this workspace.
        let swatch_center_x = (swatch_layout.location.x + swatch_layout.size.width / 2.0) as u32;
        let swatch_center_y = (swatch_layout.location.y + swatch_layout.size.height / 2.0) as u32;
        let swatch_pixel = pixel_at(swatch_center_x, swatch_center_y);
        assert_eq!(
            swatch_pixel,
            [0x67, 0x50, 0xA4, 0xFF],
            "swatch pixel {swatch_pixel:?} doesn't match view.yaml's declared #6750A4 -- \
             the parsed color didn't reach the rendered pixel"
        );

        // The label drew real ink somewhere in its own laid-out box --
        // the text-node claim, calibrated the same way text_layout.rs's
        // own test is (anti-aliased glyph edges make an exact-pixel
        // match brittle for text).
        let label_x0 = label_layout.location.x as u32;
        let label_y0 = label_layout.location.y as u32;
        let label_x1 = (label_layout.location.x + label_layout.size.width) as u32;
        let label_y1 = (label_layout.location.y + label_layout.size.height) as u32;
        let has_ink = (label_y0..label_y1).step_by(4).any(|y| {
            (label_x0..label_x1)
                .step_by(4)
                .any(|x| pixel_at(x, y)[3] > 0)
        });
        assert!(has_ink, "label node drew no ink in its own laid-out box");
    });
}
