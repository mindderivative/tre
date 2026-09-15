//! Vello Scene building and GPU rendering.
//!
//! §14 build-order step 1: one static rounded rect through `vello_hybrid`.
//! No layout, no text, no Python -- proving the render pipeline itself
//! exists before anything is built on top of it (Design Principle 5).
//!
//! §14 build-order step 3 adds `build_tree_scene`: walks a real
//! `engine_core::Tree` (after `compute_layout` has run) and paints every
//! `NodeKind::Rect` at its taffy-computed absolute position, composing
//! layout and paint for the first time -- `build_rect_scene` above still
//! exists unchanged as step 1/2's single-static-rect proof.
//!
//! §14 build-order step 4 adds `NodeKind::Text` handling to that same
//! walk, via the `text` module's `TextRenderer` (`parley` shaping fed
//! into `vello_hybrid`'s low-level glyph API).
//!
//! Depends on `engine-core` for `Tree`/`NodeId`/`NodeKind`/
//! `PaintProperties` and, for windowing, on nothing at all -- this crate
//! never touches `winit`. Window/surface creation is the caller's job
//! (an example, or later `engine-py`); everything here is parameterized
//! over a `wgpu::Device`/`Queue`/`TextureView` the caller already has,
//! matching §4's crate-boundary rule.

mod text;

use engine_core::{NodeId, NodeKind, Tree};
use peniko::Color;
use peniko::kurbo::{Affine, Circle, Point, Rect, RoundedRect, Shape};
use vello_hybrid::{RenderSize, RenderTargetConfig, Renderer, Resources, Scene, TextureBindings};

pub use text::{TextPlacement, TextRenderer};

/// MD3 seed-adjacent purple (#6750A4) -- an arbitrary but deliberate
/// starting color, not vello_hybrid's own default, so a wrong pixel in a
/// readback test can't be confused with "the renderer drew nothing and
/// left its own default."
pub const INITIAL_COLOR: Color = Color::from_rgba8(0x67, 0x50, 0xA4, 0xFF);

/// Returns `color` with its alpha channel replaced by `opacity`
/// (0.0..=1.0), independent of whatever alpha `color` already carried --
/// this is how step 2's demo composes an `Animated<Color>` and a
/// separate `Animated<f64>` opacity into one paint value each frame,
/// rather than conflating "which color" and "how visible" into a single
/// animated type.
pub fn with_opacity(color: Color, opacity: f64) -> Color {
    Color {
        components: [
            color.components[0],
            color.components[1],
            color.components[2],
            opacity as f32,
        ],
        cs: std::marker::PhantomData,
    }
}

/// Builds the one rounded rectangle this step exists to prove -- centered
/// with a fixed margin, filled with `color` at `opacity`. Both are the
/// caller's live, already-ticked `Animated<T>::current` values (§14 step
/// 2); nothing here reads a `Node` yet -- that starts at step 3 (`taffy`
/// layout of multiple *dynamic* nodes).
pub fn build_rect_scene(width: u16, height: u16, color: Color, opacity: f64) -> Scene {
    let mut scene = Scene::new(width, height);
    let margin = 40.0;
    let rect = RoundedRect::new(
        margin,
        margin,
        f64::from(width) - margin,
        f64::from(height) - margin,
        24.0,
    );
    scene.set_transform(Affine::IDENTITY);
    scene.set_paint(with_opacity(color, opacity));
    scene.fill_path(&rect.to_path(0.1));
    scene
}

/// §14 build-order step 8: standalone spike proving `vello_hybrid`
/// 0.2.0's `Scene::fill_blurred_rounded_rect` actually produces a real
/// Gaussian-blurred shadow, not just that the call compiles -- the
/// exact risk named in §7.2/§15 ("early-stage per Vello's own release
/// notes, no API stability guarantee yet, uneven parity across the
/// `vello`/`vello_cpu`/`vello_hybrid` variants"). One rect, no `Tree`,
/// no layout, no MD3 elevation tokens -- those are deliberately later
/// steps (9, 11) that would otherwise sit on an unverified foundation.
///
/// `std_dev` is the Gaussian blur's standard deviation in pixels (not a
/// blur "radius" in the CSS `box-shadow` sense); `corner_radius` is the
/// rect's own rounded-corner radius, independent of the blur.
pub fn build_shadow_scene(
    width: u16,
    height: u16,
    color: Color,
    corner_radius: f32,
    std_dev: f32,
) -> Scene {
    let mut scene = Scene::new(width, height);
    let margin = 60.0;
    let rect = peniko::kurbo::Rect::new(
        margin,
        margin,
        f64::from(width) - margin,
        f64::from(height) - margin,
    );
    scene.set_transform(Affine::IDENTITY);
    scene.set_paint(color);
    scene.fill_blurred_rounded_rect(&rect, corner_radius, std_dev, false);
    scene
}

/// §14 build-order step 9: standalone spike proving `vello_hybrid`
/// 0.2.0's real `Scene::push_layer(clip_path, blend_mode, opacity,
/// mask, filter)` genuinely does both things §7.3's ripple model needs
/// from it -- clips a fill to an arbitrary path (here, a growing
/// circle) *and* applies an opacity multiplier to everything painted
/// inside the layer -- not just that the call compiles. One ripple over
/// one solid "button" background; no `Tree`, no `InteractionState`
/// wiring, no MD3 ripple-color token (see this step's own `LOG.md` for
/// why: real dispatch and a real color scheme don't exist yet).
pub fn build_ripple_scene(
    width: u16,
    height: u16,
    base_color: Color,
    ripple_color: Color,
    origin: Point,
    radius: f64,
    opacity: f64,
) -> Scene {
    let mut scene = Scene::new(width, height);
    let bounds = Rect::new(0.0, 0.0, f64::from(width), f64::from(height));
    scene.set_transform(Affine::IDENTITY);

    // The "button" background the ripple plays over.
    scene.set_paint(base_color);
    scene.fill_path(&bounds.to_path(0.1));

    // The ripple itself: push_layer's `clip_path` is what actually
    // confines the fill below to the circle -- the fill call itself
    // still covers the whole `bounds` rect, same as the background did.
    let circle = Circle::new(origin, radius).to_path(0.1);
    scene.push_layer(Some(&circle), None, Some(opacity as f32), None, None);
    scene.set_paint(ripple_color);
    scene.fill_path(&bounds.to_path(0.1));
    scene.pop_layer();

    scene
}

/// Walks `tree` from `root` (which must already have a computed layout --
/// call `Tree::compute_layout` first) and paints every `NodeKind::Rect`/
/// `NodeKind::Text` at its absolute on-screen position:
/// `taffy::Layout::location` is parent-relative, so this accumulates each
/// ancestor's offset on the way down rather than trusting a child's
/// location alone. `Container` nodes paint nothing themselves but still
/// recurse into their children -- they exist purely to give `taffy`
/// something to lay children out against.
///
/// `resources`/`text` are threaded through for `NodeKind::Text` nodes:
/// glyph atlasing (`resources`) and font/shaping state (`text`) both
/// need to persist across frames, so they're the caller's, not built
/// fresh per call -- see `TextRenderer`'s own doc comment for why.
pub fn build_tree_scene(
    tree: &Tree,
    root: NodeId,
    width: u16,
    height: u16,
    resources: &mut Resources,
    text: &mut TextRenderer,
) -> Scene {
    let mut scene = Scene::new(width, height);
    scene.set_transform(Affine::IDENTITY);
    paint_node(tree, root, 0.0, 0.0, &mut scene, resources, text);
    scene
}

fn paint_node(
    tree: &Tree,
    id: NodeId,
    offset_x: f64,
    offset_y: f64,
    scene: &mut Scene,
    resources: &mut Resources,
    text: &mut TextRenderer,
) {
    let node = tree
        .get(id)
        .expect("build_tree_scene: NodeId not found in this Tree");
    let layout = tree.layout(id);
    let x = offset_x + f64::from(layout.location.x);
    let y = offset_y + f64::from(layout.location.y);
    let w = f64::from(layout.size.width);
    let h = f64::from(layout.size.height);

    match &node.kind {
        NodeKind::Rect | NodeKind::Splitter(_) => {
            // A splitter's own visible grip/handle paints exactly like
            // a Rect -- `SplitterState` carries only the mechanism's
            // animatable position (§11.5), no separate appearance data,
            // since the universal `PaintProperties` every node already
            // has is all a divider's own background/corner-radius needs.
            let radius = node.paint.corner_radius.current;
            let color = with_opacity(node.paint.background.current, node.paint.opacity.current);
            let rect = RoundedRect::new(x, y, x + w, y + h, radius);
            scene.set_paint(color);
            scene.fill_path(&rect.to_path(0.1));
        }
        NodeKind::Text(state) => {
            let color = with_opacity(node.paint.background.current, node.paint.opacity.current);
            text.draw(
                scene,
                resources,
                state,
                TextPlacement {
                    x,
                    y,
                    max_width: w as f32,
                    color,
                },
            );
        }
        // A `VirtualList` container paints nothing itself, same as
        // `Container` -- it exists purely to give `taffy` something to
        // lay its (windowed) children out against; the recursive walk
        // below already only ever sees `VirtualListState::materialized`'s
        // small real subset, never `item_count`, with zero changes
        // needed here (§14 step 15, §11.7).
        NodeKind::Container | NodeKind::VirtualList(_) => {}
    }

    for &child in &node.children {
        paint_node(tree, child, x, y, scene, resources, text);
    }
}

/// Thin wrapper around `vello_hybrid::Renderer` -- it needs a mutable
/// `Resources` alongside it for every render call, which is easy to get
/// out of sync by hand; bundling them here means callers only ever see
/// one object.
pub struct FrameRenderer {
    renderer: Renderer,
    resources: Resources,
}

impl FrameRenderer {
    pub fn new(device: &wgpu::Device, config: &RenderTargetConfig) -> Self {
        let (renderer, resources) = Renderer::new(device, config);
        Self {
            renderer,
            resources,
        }
    }

    /// Renders `scene` into `target` via `encoder`. Does not submit the
    /// encoder or present anything -- that's the caller's surface/queue
    /// to manage, per the crate-boundary rule.
    pub fn render(
        &mut self,
        scene: &Scene,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        render_size: &RenderSize,
        target: &wgpu::TextureView,
    ) {
        self.renderer
            .render(
                scene,
                &mut self.resources,
                device,
                queue,
                encoder,
                render_size,
                target,
                &TextureBindings::new(),
            )
            .expect("vello_hybrid render failed");
    }

    /// The same `Resources` `render` uses internally, exposed for
    /// `build_tree_scene` to pass to `Scene::glyph_run` (§14 step 4):
    /// glyph atlasing happens during scene *construction*, before
    /// `render` is ever called, so whoever builds a text-containing
    /// `Scene` needs the identical `Resources` instance `render` will
    /// later draw with -- not a second, disconnected one. This is a
    /// narrower hole in the "callers only see one object" bundling this
    /// struct's own doc comment promises than it looks: callers still
    /// only ever hold a `FrameRenderer`, they just borrow through it for
    /// this one call.
    pub fn resources_mut(&mut self) -> &mut Resources {
        &mut self.resources
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Headless correctness check, not just "did it panic": render the
    /// one-rect scene to an offscreen texture, read the pixels back, and
    /// assert the rect's fill color actually landed where it should --
    /// the center -- and the background is untouched at a corner outside
    /// the rounded rect. This is the same render-to-texture-then-readback
    /// pattern vello_hybrid's own `render_to_file` example uses, adapted
    /// to assert instead of write a PNG. Runs without a display or a real
    /// window, so it's safe under `cargo test` on any CI runner --
    /// TRE v1's own lesson (LESSONS_LEARNED.md §3/§4) about needing a
    /// headless-safe verification path from day one, not bolted on late.
    #[test]
    fn rect_scene_renders_expected_pixels() {
        pollster::block_on(async {
            let width: u16 = 200;
            let height: u16 = 200;

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
                    label: Some("engine-render test device"),
                    required_features: wgpu::Features::empty(),
                    ..Default::default()
                })
                .await
                .expect("failed to create wgpu device");

            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("engine-render test target"),
                size: wgpu::Extent3d {
                    width: u32::from(width),
                    height: u32::from(height),
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

            let scene = build_rect_scene(width, height, INITIAL_COLOR, 1.0);
            let mut frame_renderer = FrameRenderer::new(
                &device,
                &RenderTargetConfig {
                    format: texture.format(),
                    width: u32::from(width),
                    height: u32::from(height),
                },
            );
            let render_size = RenderSize {
                width: u32::from(width),
                height: u32::from(height),
            };

            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            frame_renderer.render(&scene, &device, &queue, &mut encoder, &render_size, &view);

            let bytes_per_row = (u32::from(width) * 4).next_multiple_of(256);
            let readback = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("readback"),
                size: u64::from(bytes_per_row) * u64::from(height),
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
                    width: u32::from(width),
                    height: u32::from(height),
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

            // Center of the rect: should be the fill color.
            let center = pixel_at(u32::from(width) / 2, u32::from(height) / 2);
            assert_eq!(
                center,
                [0x67, 0x50, 0xA4, 0xFF],
                "center pixel {center:?} does not match the expected fill color -- \
                 the renderer drew something, but not the rect this test asked for"
            );

            // A corner well outside the rounded rect (margin is 40px,
            // corner radius 24px -- (5, 5) is safely in the untouched
            // background on every side).
            let corner = pixel_at(5, 5);
            assert_eq!(
                corner,
                [0, 0, 0, 0],
                "corner pixel {corner:?} is not transparent background -- \
                 the fill leaked outside the rect's bounds"
            );
        });
    }
}
