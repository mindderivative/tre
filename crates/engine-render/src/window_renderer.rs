//! 0.4.0 M6: one window's rendering, frame to frame.
//!
//! Everything a window keeps between frames to draw itself -- the vello
//! renderer, the text and geometry caches, the persistent target (M3) and
//! the damage tracker (M4) -- and the one frame sequence that uses them
//! (M5): upload images, drop caches for removed nodes, work out what
//! changed, render only that into the kept target (or everything, or
//! nothing), and copy the target to the surface image. The app drives it,
//! and so do the pixel tests and the benchmark, so what they check is what
//! the app does.
//!
//! A frame is two calls, so the caller can learn what changed before it
//! acquires a surface image: `prepare` (the damage), then `draw`. A frame
//! prepared but not drawn -- the surface image couldn't be acquired --
//! must be followed by `reset`, since the tracker has recorded it as
//! drawn.

use engine_core::{NodeId, Tree};
use peniko::Color;
use peniko::kurbo::{Affine, Rect, Stroke};
use vello_gpu::{RenderSize, RenderTargetConfig, Scene};

use crate::{
    Damage, DamageTracker, FrameRenderer, GeometryCache, PersistentTarget, ShaderTextures,
    TextRenderer, build_tree_scene_shaded,
};

/// The redrawn-areas overlay's colours (0.4.1 M8): a translucent fill and a
/// solid edge for partial damage, and an edge for a full redraw.
const OVERLAY_FILL: Color = Color::from_rgba8(0xFF, 0x00, 0xC8, 0x40);
const OVERLAY_EDGE: Color = Color::from_rgba8(0xFF, 0x00, 0xC8, 0xC0);
const OVERLAY_FULL: Color = Color::from_rgba8(0xFF, 0x90, 0x00, 0xC0);

/// A window's renderer and caches, and its kept frame.
pub struct WindowRenderer {
    frame_renderer: FrameRenderer,
    text: TextRenderer,
    geometry: GeometryCache,
    target: Option<PersistentTarget>,
    tracker: DamageTracker,
    /// 0.5.1 (#67): the window's clock, for shaders that read `frame.time`.
    time: f32,
}

/// A window extent as the renderer takes it (`u16`), clamped rather than
/// wrapped.
fn extent(pixels: u32) -> u16 {
    u16::try_from(pixels).unwrap_or(u16::MAX)
}

impl WindowRenderer {
    /// A renderer for a `width` x `height` surface of `format`. With
    /// `persistent`, frames render into a kept target copied to the
    /// surface image, which partial redraw needs; without it (a surface
    /// that can't be copied into), straight into the surface image, every
    /// frame in full.
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        persistent: bool,
    ) -> Self {
        Self {
            frame_renderer: FrameRenderer::new(
                device,
                &RenderTargetConfig {
                    format,
                    width: extent(width),
                    height: extent(height),
                },
            ),
            text: TextRenderer::new(),
            geometry: GeometryCache::new(),
            target: persistent.then(|| PersistentTarget::new(device, format, width, height)),
            tracker: DamageTracker::new(),
            time: 0.0,
        }
    }

    /// Sets the time, in seconds, the next frame's shaders see as
    /// `frame.time`.
    pub fn set_time(&mut self, seconds: f32) {
        self.time = seconds;
    }

    /// How many shader passes the last drawn frame ran.
    pub fn shader_pass_count(&self) -> usize {
        self.frame_renderer.shader_pass_count()
    }

    /// The window's text shaper -- the one painting uses, so hit-testing
    /// and font registration see the same layouts.
    pub fn text(&mut self) -> &mut TextRenderer {
        &mut self.text
    }

    /// Whether frames go through a kept target, so partial redraw can
    /// apply.
    pub fn has_persistent_target(&self) -> bool {
        self.target.is_some()
    }

    /// Resizes the kept target; a recreated one has lost its frame, so the
    /// next is drawn in full.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if let Some(target) = &mut self.target
            && target.ensure_size(device, width, height)
        {
            self.tracker.reset();
        }
    }

    /// Makes the next frame a full one -- after a frame prepared but not
    /// drawn, or anything that changes pixels no node records (a font
    /// registered).
    pub fn reset(&mut self) {
        self.tracker.reset();
    }

    /// A frame's first half: uploads its images, drops caches for removed
    /// nodes, and says what changed since the last frame -- always `Full`
    /// without a kept target or with `partial` off.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        tree: &Tree,
        root: NodeId,
        width: u16,
        height: u16,
        partial: bool,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Damage {
        // Every image needs its texture bound before the scene draws it.
        self.frame_renderer.sync_image_textures(tree, device, queue);
        self.text.evict_stale_layouts(tree);
        self.geometry.evict_stale(tree);
        if self.target.is_none() || !partial {
            // No walk: its answer would go unused. The reset makes the
            // first frame after partial redraw comes back on a full one.
            self.tracker.reset();
            return Damage::Full;
        }
        self.tracker
            .damage(tree, root, width, height, &mut self.text)
    }

    /// A frame's second half: renders what `damage` (this frame's
    /// `prepare`) calls for -- only its rects over the kept frame, all of
    /// it, or nothing -- and puts the frame in `surface`, whose view is
    /// `surface_view`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        tree: &Tree,
        root: NodeId,
        width: u16,
        height: u16,
        damage: &Damage,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        surface: &wgpu::Texture,
        surface_view: &wgpu::TextureView,
    ) {
        let rects = match damage {
            Damage::Rects(rects) => Some(rects.as_slice()),
            _ => None,
        };
        // Fill shaders run first, into the same encoder, so the scene can
        // draw their textures.
        let shaders = match damage {
            Damage::None => {
                self.frame_renderer.skip_shader_passes();
                ShaderTextures::none()
            }
            _ => self
                .frame_renderer
                .run_shader_passes(tree, root, width, height, self.time, device, queue, encoder),
        };
        let scene = match damage {
            // Nothing changed: the kept frame is copied as it is.
            Damage::None => None,
            _ => Some(build_tree_scene_shaded(
                tree,
                root,
                width,
                height,
                rects,
                &shaders,
                self.frame_renderer.resources_mut(),
                &mut self.text,
                &mut self.geometry,
            )),
        };
        let view = self
            .target
            .as_ref()
            .map_or(surface_view, PersistentTarget::view);
        if let Some(scene) = &scene {
            self.frame_renderer.render_into(
                scene,
                device,
                queue,
                encoder,
                &RenderSize { width, height },
                view,
                rects,
            );
        }
        if let Some(target) = &self.target {
            target.copy_to(encoder, surface);
        }
    }

    /// 0.4.1 M8: tints what `damage` redrew over the frame already in
    /// `surface_view` -- each damage rect filled translucent magenta and
    /// outlined, a full redraw outlined at the window's edge, nothing for no
    /// damage. The kept frame is never touched, so the next partial frame
    /// starts clean. Submits its own work: call it after the frame's encoder
    /// is submitted, since the renderer's buffers are written as each render
    /// is encoded.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_damage_overlay(
        &mut self,
        damage: &Damage,
        width: u16,
        height: u16,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_view: &wgpu::TextureView,
    ) {
        let mut scene = Scene::new(width, height);
        scene.set_transform(Affine::IDENTITY);
        match damage {
            Damage::None => return,
            Damage::Full => {
                let edge = Rect::new(0.0, 0.0, f64::from(width), f64::from(height)).inset(-2.0);
                scene.set_paint(OVERLAY_FULL);
                scene.set_stroke(Stroke::new(4.0));
                scene.stroke_rect(&edge);
            }
            Damage::Rects(rects) => {
                for rect in rects {
                    scene.set_paint(OVERLAY_FILL);
                    scene.fill_rect(rect);
                    scene.set_paint(OVERLAY_EDGE);
                    scene.set_stroke(Stroke::new(2.0));
                    scene.stroke_rect(&rect.inset(-1.0));
                }
            }
        }
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        self.frame_renderer.render_over(
            &scene,
            device,
            queue,
            &mut encoder,
            &RenderSize { width, height },
            surface_view,
        );
        queue.submit([encoder.finish()]);
    }
}
