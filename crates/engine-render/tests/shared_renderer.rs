//! 0.5.9 (#235): windows on one device draw with one `vello_gpu` renderer.
//!
//! The renderer (pipelines, glyph atlas, gradient cache) is made once and shared; each window
//! keeps its own text shaper, caches, images, kept frame and damage. What must hold: two windows
//! of different sizes and content, drawn in turn through one shared renderer, show byte for byte
//! what each shows with a renderer of its own, including after the other has drawn in between.

mod support;

use engine_core::{NodeId, NodeKind, PaintProperties, Shadow, Shadows, TextState, Tree};
use engine_render::{SharedRenderer, WindowRenderer};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// A window's tree: a card with a shadow, a translucent tint and a line of text.
fn tree(w: u16, h: u16, text: &str, card: Color) -> (Tree, NodeId) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        Style {
            size: Size {
                width: length(f32::from(w)),
                height: length(f32::from(h)),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0x11, 0x11, 0x11, 0xFF), 0.0, 1.0),
    );
    let card = tree.insert(
        NodeKind::Rect,
        support::placed(10.0, 10.0, f32::from(w) / 2.0, f32::from(h) / 2.0),
        PaintProperties::new(card, 8.0, 1.0),
    );
    tree.add_child(root, card);
    tree.get_mut(card).unwrap().paint.shadows.current = Shadows(vec![Shadow {
        color: support::BLACK,
        offset_x: 0.0,
        offset_y: 5.0,
        blur: 7.0,
        spread: 0.0,
    }]);
    let tint = tree.insert(
        NodeKind::Rect,
        support::placed(f32::from(w) / 3.0, f32::from(h) / 3.0, 40.0, 40.0),
        PaintProperties::new(Color::from_rgba8(0, 0x80, 0xFF, 0x80), 12.0, 1.0),
    );
    tree.add_child(root, tint);
    let label = tree.insert(
        NodeKind::Text(TextState {
            content: text.into(),
            font_family: "Roboto".into(),
            font_weight: 400.0,
            font_size: 14.0,
            align: Default::default(),
            line_height: None,
            options: Default::default(),
        }),
        support::placed(12.0, f32::from(h) - 30.0, f32::from(w) - 24.0, 20.0),
        PaintProperties::new(support::WHITE, 0.0, 1.0),
    );
    tree.add_child(root, label);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(f32::from(w)),
            height: AvailableSpace::Definite(f32::from(h)),
        },
    );
    (tree, root)
}

/// One window: a renderer, a stand-in swapchain image and what to draw.
struct Win {
    renderer: WindowRenderer,
    surface: wgpu::Texture,
    view: wgpu::TextureView,
    tree: Tree,
    root: NodeId,
    w: u16,
    h: u16,
}

impl Win {
    fn new(
        device: &wgpu::Device,
        shared: Option<&SharedRenderer>,
        w: u16,
        h: u16,
        text: &str,
        card: Color,
    ) -> Self {
        let surface = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("stand-in swapchain image"),
            size: wgpu::Extent3d {
                width: u32::from(w),
                height: u32::from(h),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = surface.create_view(&wgpu::TextureViewDescriptor::default());
        let renderer = match shared {
            Some(shared) => {
                WindowRenderer::sharing(device, FORMAT, u32::from(w), u32::from(h), true, shared)
            }
            None => WindowRenderer::new(device, FORMAT, u32::from(w), u32::from(h), true),
        };
        let (tree, root) = tree(w, h, text, card);
        Self {
            renderer,
            surface,
            view,
            tree,
            root,
            w,
            h,
        }
    }

    /// One frame as the app runs it, then the image read back.
    fn frame(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, partial: bool) -> Vec<u8> {
        let damage = self.renderer.prepare(
            &self.tree, self.root, self.w, self.h, partial, device, queue,
        );
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        self.renderer.draw(
            &self.tree,
            self.root,
            self.w,
            self.h,
            &damage,
            device,
            queue,
            &mut encoder,
            &self.surface,
            &self.view,
        );
        queue.submit([encoder.finish()]);
        support::read_texture(device, queue, &self.surface)
    }
}

const A: (u16, u16, &str) = (160, 110, "Shared A: fgjpqy");
const B: (u16, u16, &str) = (240, 150, "Window B, a longer label WXYZ");

fn orange() -> Color {
    Color::from_rgba8(0xFF, 0x90, 0x20, 0xFF)
}

fn teal() -> Color {
    Color::from_rgba8(0x20, 0xC0, 0xB0, 0xFF)
}

/// Frames for the two windows in turn, three rounds (full, kept-frame copy, full again), with the
/// renderer shared or each window's own. The images each window showed, in order.
fn run(device: &wgpu::Device, queue: &wgpu::Queue, share: bool) -> Vec<(char, Vec<u8>)> {
    let shared = share.then(|| {
        SharedRenderer::new(
            device,
            &vello_gpu::RenderTargetConfig {
                format: FORMAT,
                width: A.0,
                height: A.1,
            },
        )
    });
    let mut a = Win::new(device, shared.as_ref(), A.0, A.1, A.2, orange());
    let mut b = Win::new(device, shared.as_ref(), B.0, B.1, B.2, teal());
    let mut shown = Vec::new();
    for partial in [true, true, false] {
        shown.push(('a', a.frame(device, queue, partial)));
        shown.push(('b', b.frame(device, queue, partial)));
    }
    shown
}

#[test]
fn two_windows_of_different_sizes_look_the_same_sharing_a_renderer_as_with_their_own() {
    pollster::block_on(async {
        let (device, queue) = support::device("shared renderer").await;
        let own = run(&device, &queue, false);
        let shared = run(&device, &queue, true);
        assert_eq!(own.len(), shared.len());
        for (i, ((name, mine), (_, ours))) in own.iter().zip(&shared).enumerate() {
            assert_eq!(
                mine, ours,
                "frame {i} of window {name} differs when the renderer is shared"
            );
        }
        // The windows really differ, so equality is not two blank images.
        assert_ne!(own[0].1.len(), own[1].1.len());
        // The label's glyphs were drawn: white pixels in the bottom rows of window A's image
        // (rows are padded to 256 bytes).
        let stride = (usize::from(A.0) * 4).next_multiple_of(256);
        let label_rows =
            &own[0].1[stride * (usize::from(A.1) - 30)..stride * (usize::from(A.1) - 8)];
        let white = label_rows
            .chunks(4)
            .filter(|px| px[0] > 0xC0 && px[1] > 0xC0 && px[2] > 0xC0)
            .count();
        assert!(white > 20, "{white} white pixels: the text drew");
    });
}

#[test]
fn a_shared_renderer_is_held_by_every_window_that_uses_it_and_freed_with_the_last() {
    pollster::block_on(async {
        let (device, _queue) = support::device("shared renderer holders").await;
        let shared = SharedRenderer::new(
            &device,
            &vello_gpu::RenderTargetConfig {
                format: FORMAT,
                width: 64,
                height: 64,
            },
        );
        assert_eq!(shared.holders(), 1);
        let a = WindowRenderer::sharing(&device, FORMAT, 64, 64, true, &shared);
        let b = WindowRenderer::sharing(&device, FORMAT, 64, 64, true, &shared);
        assert_eq!(shared.holders(), 3, "the handle and two windows");
        drop(a);
        assert_eq!(shared.holders(), 2);
        drop(b);
        assert_eq!(shared.holders(), 1);
    });
}

/// A window that makes its own renderer shares nothing with another that does.
#[test]
fn a_window_made_with_new_has_a_renderer_of_its_own() {
    pollster::block_on(async {
        let (device, _queue) = support::device("own renderer").await;
        let shared = SharedRenderer::new(
            &device,
            &vello_gpu::RenderTargetConfig {
                format: FORMAT,
                width: 64,
                height: 64,
            },
        );
        let _own = WindowRenderer::new(&device, FORMAT, 64, 64, true);
        assert_eq!(shared.holders(), 1, "`new` did not take the shared one");
    });
}
