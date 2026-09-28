//! 0.4.0 M3: rendering into a `PersistentTarget` and copying it into
//! another texture -- as the window copies it into each swapchain image --
//! shows exactly what rendering straight into that texture shows, and the
//! target keeps its contents from one copy to the next.

use engine_core::{NodeId, NodeKind, PaintProperties, Shadow, Shadows, Tree};
use engine_render::{
    FrameRenderer, GeometryCache, PersistentTarget, TextRenderer, build_tree_scene,
};
use vello_gpu::{RenderSize, RenderTargetConfig};

mod support;
use support::*;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// A card with a rounded corner and a shadow over the background, and a
/// translucent box over the card -- blending, blur, and antialiasing,
/// where a copy that wasn't byte-exact would show.
fn card_scene() -> (Tree, NodeId) {
    let (mut tree, root) = scene();
    let card = tree.insert(
        NodeKind::Rect,
        placed(15.0, 15.0, 50.0, 40.0),
        PaintProperties::new(WHITE, 8.0, 1.0),
    );
    tree.add_child(root, card);
    tree.get_mut(card).unwrap().paint.shadows.current = Shadows(vec![Shadow {
        color: BLACK,
        offset_x: 0.0,
        offset_y: 6.0,
        blur: 8.0,
        spread: 0.0,
    }]);
    let tint = tree.insert(
        NodeKind::Rect,
        placed(40.0, 35.0, 45.0, 45.0),
        PaintProperties::new(peniko::Color::from_rgba8(0, 0x80, 0xFF, 0x80), 12.0, 1.0),
    );
    tree.add_child(root, tint);
    layout(&mut tree, root);
    (tree, root)
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl Gpu {
    async fn new() -> Self {
        let adapter = wgpu::Instance::default()
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                apply_limit_buckets: false,
                compatible_surface: None,
            })
            .await
            .expect("no wgpu adapter available in this environment");
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("persistent target test device"),
                ..Default::default()
            })
            .await
            .expect("failed to create wgpu device");
        Self { device, queue }
    }

    /// A texture standing in for a swapchain image: the size of the
    /// scene, copied into, and read back.
    fn swapchain_image(&self) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("stand-in swapchain image"),
            size: wgpu::Extent3d {
                width: u32::from(SIZE),
                height: u32::from(SIZE),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    /// Renders `tree` into `target`, as a window's frame does.
    fn render_into(&self, tree: &Tree, root: NodeId, target: &PersistentTarget) {
        let mut frame_renderer = FrameRenderer::new(
            &self.device,
            &RenderTargetConfig {
                format: FORMAT,
                width: SIZE,
                height: SIZE,
            },
        );
        let scene = build_tree_scene(
            tree,
            root,
            SIZE,
            SIZE,
            frame_renderer.resources_mut(),
            &mut TextRenderer::new(),
            &mut GeometryCache::new(),
        );
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        frame_renderer.render(
            &scene,
            &self.device,
            &self.queue,
            &mut encoder,
            &RenderSize {
                width: SIZE,
                height: SIZE,
            },
            target.view(),
        );
        self.queue.submit([encoder.finish()]);
    }

    /// Copies `target` into a fresh stand-in swapchain image and reads the
    /// image's pixels back, rows padded to 256 bytes as `support::render`'s are.
    fn copy_out(&self, target: &PersistentTarget) -> Vec<u8> {
        let image = self.swapchain_image();
        let bytes_per_row = (u32::from(SIZE) * 4).next_multiple_of(256);
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(bytes_per_row) * u64::from(SIZE),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        target.copy_to(&mut encoder, &image);
        encoder.copy_texture_to_buffer(
            image.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: None,
                },
            },
            image.size(),
        );
        self.queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            result.expect("failed to map readback buffer");
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll failed");
        let data = slice.get_mapped_range().expect("the readback buffer maps");
        data.to_vec()
    }
}

#[test]
fn a_copied_target_shows_exactly_what_a_direct_render_does() {
    pollster::block_on(async {
        let (tree, root) = card_scene();
        let (direct, _) = render(&tree, root).await;

        let gpu = Gpu::new().await;
        let target = PersistentTarget::new(&gpu.device, FORMAT, u32::from(SIZE), u32::from(SIZE));
        gpu.render_into(&tree, root, &target);
        let copied = gpu.copy_out(&target);

        assert_eq!(copied.len(), direct.len());
        let differing = copied.iter().zip(&direct).filter(|(a, b)| a != b).count();
        assert_eq!(
            differing, 0,
            "{differing} bytes differ from the direct render"
        );
        let frame = Frame {
            data: copied,
            bytes_per_row: (u32::from(SIZE) * 4).next_multiple_of(256),
        };
        assert_eq!(frame.at(20, 20), rgba(WHITE), "the card");
        assert_eq!(frame.at(2, 2), rgba(BACKGROUND), "the background");
    });
}

#[test]
fn the_target_keeps_its_frame_from_one_copy_to_the_next() {
    pollster::block_on(async {
        let (tree, root) = card_scene();
        let gpu = Gpu::new().await;
        let target = PersistentTarget::new(&gpu.device, FORMAT, u32::from(SIZE), u32::from(SIZE));
        gpu.render_into(&tree, root, &target);
        let first = gpu.copy_out(&target);
        // No render in between: a second swapchain image still gets the
        // whole frame -- what partial redraw builds on.
        let second = gpu.copy_out(&target);
        assert!(
            first == second,
            "the target lost its contents between copies"
        );
    });
}

#[test]
fn the_target_is_recreated_only_when_its_size_changes() {
    pollster::block_on(async {
        let gpu = Gpu::new().await;
        let mut target = PersistentTarget::new(&gpu.device, FORMAT, 100, 80);
        assert!(!target.ensure_size(&gpu.device, 100, 80), "same size: kept");
        assert!(
            target.ensure_size(&gpu.device, 120, 80),
            "new size: recreated"
        );
        assert_eq!(target.size(), (120, 80));
        assert!(
            target.ensure_size(&gpu.device, 0, 0),
            "a zero size is raised to 1x1"
        );
        assert_eq!(target.size(), (1, 1));
    });
}
