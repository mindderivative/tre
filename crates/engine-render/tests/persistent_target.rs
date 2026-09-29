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
        let (device, queue) = support::device("persistent target test device").await;
        Self { device, queue }
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
        support::copy_out(&self.device, &self.queue, target)
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
