//! 0.4.0 M5: a partial redraw -- only the damage rects cleared and
//! repainted, over the kept last frame -- gives exactly the pixels a full
//! redraw of the changed tree does. Each test renders a scene in full into
//! a `PersistentTarget`, changes one thing, redraws only the damage into
//! the same target, and compares it byte for byte with a fresh full render.

use engine_core::{NodeId, NodeKind, PaintProperties, Shadow, Shadows, TextState, Tree};
use engine_render::{
    Damage, DamageTracker, FrameRenderer, GeometryCache, PersistentTarget, TextRenderer,
    build_tree_scene, build_tree_scene_in,
};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Position, Rect as TaffyRect, Size, Style, auto, length};
use vello_gpu::{RenderSize, RenderTargetConfig};

const W: u16 = 240;
const H: u16 = 160;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl Gpu {
    fn new() -> Self {
        pollster::block_on(async {
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
                    label: Some("partial redraw test device"),
                    ..Default::default()
                })
                .await
                .expect("failed to create wgpu device");
            Self { device, queue }
        })
    }
}

/// A window's rendering state: a target that keeps its frame, a renderer,
/// and a damage tracker -- what `engine-py` keeps per window.
struct Window {
    target: PersistentTarget,
    renderer: FrameRenderer,
    text: TextRenderer,
    geometry: GeometryCache,
    tracker: DamageTracker,
}

impl Window {
    fn new(gpu: &Gpu) -> Self {
        Self {
            target: PersistentTarget::new(&gpu.device, FORMAT, u32::from(W), u32::from(H)),
            renderer: FrameRenderer::new(
                &gpu.device,
                &RenderTargetConfig {
                    format: FORMAT,
                    width: W,
                    height: H,
                },
            ),
            text: TextRenderer::new(),
            geometry: GeometryCache::new(),
            tracker: DamageTracker::new(),
        }
    }

    /// One frame: full, partial, or nothing, as the damage says. Returns it.
    fn frame(&mut self, gpu: &Gpu, tree: &Tree, root: NodeId, partial: bool) -> Damage {
        let damage = self.tracker.damage(tree, root, W, H, &mut self.text);
        let damage = if partial { damage } else { Damage::Full };
        let rects = match &damage {
            Damage::None => return damage,
            Damage::Full => None,
            Damage::Rects(rects) => Some(rects.as_slice()),
        };
        let scene = match rects {
            Some(rects) => build_tree_scene_in(
                tree,
                root,
                W,
                H,
                rects,
                self.renderer.resources_mut(),
                &mut self.text,
                &mut self.geometry,
            ),
            None => build_tree_scene(
                tree,
                root,
                W,
                H,
                self.renderer.resources_mut(),
                &mut self.text,
                &mut self.geometry,
            ),
        };
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        self.renderer.render_into(
            &scene,
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            &RenderSize {
                width: W,
                height: H,
            },
            self.target.view(),
            rects,
        );
        gpu.queue.submit([encoder.finish()]);
        damage
    }

    /// The target's pixels, read back through a copy -- as the window's
    /// swapchain image receives them.
    fn pixels(&self, gpu: &Gpu) -> Vec<u8> {
        let image = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("stand-in swapchain image"),
            size: wgpu::Extent3d {
                width: u32::from(W),
                height: u32::from(H),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let bytes_per_row = (u32::from(W) * 4).next_multiple_of(256);
        let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(bytes_per_row) * u64::from(H),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        self.target.copy_to(&mut encoder, &image);
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
        gpu.queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            result.expect("failed to map readback buffer");
        });
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll failed");
        let data = slice.get_mapped_range().expect("the readback buffer maps");
        data.to_vec()
    }
}

struct Scene {
    tree: Tree,
    root: NodeId,
    card: NodeId,
    tint: NodeId,
    label: NodeId,
    group: NodeId,
}

fn placed(x: f32, y: f32, w: f32, h: f32) -> Style {
    Style {
        position: Position::Absolute,
        inset: TaffyRect {
            left: length(x),
            top: length(y),
            right: auto(),
            bottom: auto(),
        },
        size: Size {
            width: length(w),
            height: length(h),
        },
        ..Default::default()
    }
}

fn add(tree: &mut Tree, parent: NodeId, kind: NodeKind, style: Style, color: Color) -> NodeId {
    let id = tree.insert(kind, style, PaintProperties::new(color, 6.0, 1.0));
    tree.add_child(parent, id);
    id
}

/// A background, a shadowed rounded card, a translucent box overlapping
/// it, a text label, and a group holding a child.
fn scene() -> Scene {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(f32::from(W)),
                height: length(f32::from(H)),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0x22, 0x22, 0x28, 0xFF), 0.0, 1.0),
    );
    let card = add(
        &mut tree,
        root,
        NodeKind::Rect,
        placed(20.0, 20.0, 80.0, 50.0),
        Color::from_rgba8(0xF0, 0xF0, 0xF0, 0xFF),
    );
    tree.get_mut(card).unwrap().paint.shadows.current = Shadows(vec![Shadow {
        color: Color::from_rgba8(0, 0, 0, 0xA0),
        offset_x: 0.0,
        offset_y: 6.0,
        blur: 8.0,
        spread: 1.0,
    }]);
    let tint = add(
        &mut tree,
        root,
        NodeKind::Rect,
        placed(70.0, 45.0, 60.0, 40.0),
        Color::from_rgba8(0x30, 0x90, 0xFF, 0x90),
    );
    let label = add(
        &mut tree,
        root,
        NodeKind::Text(TextState {
            content: "Partial".into(),
            font_family: "Roboto".into(),
            font_weight: 400.0,
            font_size: 16.0,
            align: Default::default(),
            line_height: None,
            options: Default::default(),
        }),
        placed(150.0, 20.0, 80.0, 20.0),
        Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF),
    );
    let group = add(
        &mut tree,
        root,
        NodeKind::Rect,
        placed(140.0, 90.0, 80.0, 50.0),
        Color::from_rgba8(0x60, 0x40, 0x90, 0xFF),
    );
    add(
        &mut tree,
        group,
        NodeKind::Rect,
        placed(10.0, 10.0, 30.0, 30.0),
        Color::from_rgba8(0xFF, 0xC0, 0x40, 0xFF),
    );
    layout(&mut tree, root);
    Scene {
        tree,
        root,
        card,
        tint,
        label,
        group,
    }
}

fn layout(tree: &mut Tree, root: NodeId) {
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(f32::from(W)),
            height: AvailableSpace::Definite(f32::from(H)),
        },
    );
}

/// Renders the scene in full, applies `change`, redraws partially, and
/// checks the result against a fresh full render of the changed tree.
fn partial_matches_full(change: impl FnOnce(&mut Scene)) {
    let gpu = Gpu::new();
    let mut s = scene();
    let mut window = Window::new(&gpu);
    assert_eq!(window.frame(&gpu, &s.tree, s.root, true), Damage::Full);

    change(&mut s);
    layout(&mut s.tree, s.root);
    let damage = window.frame(&gpu, &s.tree, s.root, true);
    assert!(
        matches!(damage, Damage::Rects(_)),
        "a small change redraws partially, got {damage:?}"
    );
    let partial = window.pixels(&gpu);

    let mut reference = Window::new(&gpu);
    reference.frame(&gpu, &s.tree, s.root, false);
    let full = reference.pixels(&gpu);

    let differing = partial.iter().zip(&full).filter(|(a, b)| a != b).count();
    assert_eq!(
        differing, 0,
        "{differing} bytes differ between partial and full redraw ({damage:?})"
    );
}

#[test]
fn a_color_change_under_an_overlapping_translucent_box() {
    partial_matches_full(|s| {
        s.tree.get_mut(s.card).unwrap().paint.background.current =
            Color::from_rgba8(0xE0, 0x50, 0x50, 0xFF);
    });
}

#[test]
fn a_move_that_uncovers_what_was_underneath() {
    partial_matches_full(|s| {
        let mut style = s.tree.get(s.tint).unwrap().layout_style.clone();
        style.inset.left = length(100.0);
        s.tree.set_layout_style(s.tint, style);
    });
}

#[test]
fn a_shadow_change() {
    partial_matches_full(|s| {
        s.tree.get_mut(s.card).unwrap().paint.shadows.current.0[0].offset_y = 14.0;
    });
}

#[test]
fn a_text_change() {
    partial_matches_full(|s| {
        if let NodeKind::Text(state) = &mut s.tree.get_mut(s.label).unwrap().kind {
            state.content = "Redrawn".into();
        }
    });
}

#[test]
fn a_group_opacity_change() {
    partial_matches_full(|s| {
        s.tree.get_mut(s.group).unwrap().paint.opacity.current = 0.4;
    });
}

#[test]
fn a_node_removed() {
    partial_matches_full(|s| {
        s.tree.detach(s.root, s.tint);
    });
}

#[test]
fn nothing_changed_renders_nothing_and_keeps_the_frame() {
    let gpu = Gpu::new();
    let s = scene();
    let mut window = Window::new(&gpu);
    window.frame(&gpu, &s.tree, s.root, true);
    let before = window.pixels(&gpu);
    assert_eq!(window.frame(&gpu, &s.tree, s.root, true), Damage::None);
    assert!(before == window.pixels(&gpu));
}
