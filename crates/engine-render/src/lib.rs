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

use engine_core::{DrawCommand, NodeId, NodeKind, Tree};
use peniko::Color;
use peniko::kurbo::{Affine, Circle, Point, Rect, RoundedRect, Shape, Stroke};
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
    // M8 Phase 1 (§11.8): the real, canvas-space "currently visible"
    // rect -- the whole viewport at the top of the walk. Threaded
    // through `paint_node`'s own recursion so a later `NodeKind` (a
    // real scrollable `VirtualList`, M8 Phase 2) can narrow it on the
    // way into its own clipped children, not just check it once here.
    let visible = Rect::new(0.0, 0.0, f64::from(width), f64::from(height));
    paint_node(
        tree,
        root,
        Affine::IDENTITY,
        visible,
        &mut scene,
        resources,
        text,
    );
    scene
}

/// §11.9 (M5 Phase 1): `parent_transform` is the caller's own composed
/// (canvas-space) transform for this node's *parent*; this call folds in
/// this node's taffy-computed layout position and its own
/// `PaintProperties.transform` in one product --
/// `parent * translate(layout.location) * own_transform` -- exactly
/// like nested `<g transform>` in SVG, "just ordinary matrix
/// multiplication during the paint walk" (§11.9's own text). Every
/// node's own content (and any recursive call for its children) is then
/// painted/positioned in **local, node-relative coordinates**
/// (`(0, 0)` to `(w, h)`), with `scene.set_transform(composed)` doing
/// the mapping into canvas space -- both `Scene::fill_path` and
/// `Scene::glyph_run` genuinely respect the scene's current transform
/// (confirmed directly in `vello_hybrid = "0.2.0"`'s vendored source),
/// so this needs no per-`NodeKind` special-casing. When every node's own
/// `transform` is the default `Affine::IDENTITY` (true for every node
/// before this phase), `composed` reduces to exactly the same
/// accumulated pure translation this function used to compute by hand
/// via `offset_x`/`offset_y` -- purely additive, no behavior change for
/// any existing content.
/// M7 Phase 2 (§7.2): the real MD3 "key" shadow layer's own `(offset_y,
/// blur)`, in px, at a real (possibly fractional -- `elevation` is a
/// real `Animated<f64>`, not a discrete 0-5 enum) elevation level.
/// Transcribed directly from Material Web's own `elevation/internal/
/// _elevation.scss` (the real, current, production CSS Google ships --
/// verified via its own documented per-integer-level comments, not
/// recalled or guessed, `PLAN.md`), preserving its exact piecewise-
/// linear formula so a fractional `elevation` (a card animating its own
/// lift) interpolates smoothly rather than snapping between integer
/// levels. Each term below reproduces one documented level's own real
/// value -- hand-checked against all 6 before trusting it.
fn key_shadow_geometry(level: f64) -> (f32, f32) {
    // level1: 0,1,2,0 -- level2: 0,1,2,0 -- level3: 0,1,3,0
    // level4: 0,2,3,0 -- level5: 0,4,4,0
    let level1_y = level.clamp(0.0, 1.0);
    let level4_y = (level - 3.0).clamp(0.0, 1.0);
    let level5_y = 2.0 * (level - 4.0).clamp(0.0, 1.0);
    let y = level1_y + level4_y + level5_y;

    let level1_blur = 2.0 * level.clamp(0.0, 1.0);
    let level3_blur = (level - 2.0).clamp(0.0, 1.0);
    let level5_blur = (level - 4.0).clamp(0.0, 1.0);
    let blur = level1_blur + level3_blur + level5_blur;

    (y as f32, blur as f32)
}

/// The real MD3 "ambient" shadow layer's own `(offset_y, blur,
/// spread)`, in px -- same real source and same "matches every
/// documented integer level" verification as `key_shadow_geometry`.
/// level1: 0,1,3,1 -- level2: 0,2,6,2 -- level3: 0,4,8,3
/// level4: 0,6,10,4 -- level5: 0,8,12,6
fn ambient_shadow_geometry(level: f64) -> (f32, f32, f32) {
    let level1_y = level.clamp(0.0, 1.0);
    let level2_y = (level - 1.0).clamp(0.0, 1.0);
    let level3to5_y = 2.0 * (level - 2.0).clamp(0.0, 3.0);
    let y = level1_y + level2_y + level3to5_y;

    let level1to2_blur = 3.0 * level.clamp(0.0, 2.0);
    let level3to5_blur = 2.0 * (level - 2.0).clamp(0.0, 3.0);
    let blur = level1to2_blur + level3to5_blur;

    let level1to4_spread = level.clamp(0.0, 4.0);
    let level5_spread = 2.0 * (level - 4.0).clamp(0.0, 1.0);
    let spread = level1to4_spread + level5_spread;

    (y as f32, blur as f32, spread as f32)
}

/// CSS Backgrounds and Borders Module Level 3's own real conversion,
/// verified directly (`PLAN.md`): "a Gaussian blur with a standard
/// deviation equal to half the blur radius."
fn blur_to_std_dev(blur_px: f32) -> f32 {
    blur_px / 2.0
}

/// MD3's real `shadow` color role -- the neutral palette's own tone-0
/// (blackest) position, constant regardless of the active theme's seed
/// color (verified via search, `PLAN.md`), so this doesn't need to wait
/// for M7 Phase 3's dynamic-color wiring the way ripple/hover's own
/// tint does; `opacity` is each layer's own real, documented alpha
/// (key: 0.3, ambient: 0.15).
fn shadow_color(opacity: f32) -> Color {
    with_opacity(Color::from_rgba8(0, 0, 0, 255), f64::from(opacity))
}

fn paint_node(
    tree: &Tree,
    id: NodeId,
    parent_transform: Affine,
    visible: Rect,
    scene: &mut Scene,
    resources: &mut Resources,
    text: &mut TextRenderer,
) {
    let node = tree
        .get(id)
        .expect("build_tree_scene: NodeId not found in this Tree");
    let layout = tree.layout(id);
    let w = f64::from(layout.size.width);
    let h = f64::from(layout.size.height);
    let composed = parent_transform
        * Affine::translate((f64::from(layout.location.x), f64::from(layout.location.y)))
        * node.paint.transform.current;

    // M8 Phase 1 (§11.8): a whole-subtree skip, not a per-pixel clip --
    // this node's own real, composed, absolute bounding box (all four
    // local corners transformed, not just two opposite ones, so this
    // stays correct even under a future rotation-capable `Affine`, not
    // just today's shear/rotation-free subspace) checked against the
    // "currently visible" rect threaded down from `build_tree_scene`.
    // No Vello scene-encoding happens at all for a node -- or anything
    // in its subtree -- that doesn't overlap it; `tree` is an immutable
    // reference throughout this whole walk, so there's no side effect
    // to lose by skipping.
    let corners = [
        composed * Point::new(0.0, 0.0),
        composed * Point::new(w, 0.0),
        composed * Point::new(0.0, h),
        composed * Point::new(w, h),
    ];
    let min_x = corners.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let max_x = corners
        .iter()
        .map(|p| p.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = corners.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    let max_y = corners
        .iter()
        .map(|p| p.y)
        .fold(f64::NEG_INFINITY, f64::max);
    let bounds = Rect::new(min_x, min_y, max_x, max_y);
    if !bounds.overlaps(visible) {
        return;
    }

    scene.set_transform(composed);

    // M7 Phase 2 (§7.2): a real shadow, for any NodeKind, painted
    // behind everything else -- elevation is a universal PaintProperties
    // field, and a shadow only ever needs this node's own bounds/corner
    // radius, independent of what it actually draws on top (the same
    // "zero special-casing" shape M5 Phase 1's transform composition
    // and M4 Phase 5's ripple/hover already established). Skipped
    // entirely at elevation <= 0.0 -- matching level 0's own real
    // 0px-everywhere values, not a degenerate zero-blur draw call.
    let elevation = node.paint.elevation.current;
    if elevation > 0.0 {
        let radius = node.paint.corner_radius.current as f32;

        // Ambient first, key second -- real box-shadow stacking order
        // (a later shadow paints on top of an earlier one).
        let (ambient_y, ambient_blur, ambient_spread) = ambient_shadow_geometry(elevation);
        let ambient_rect = Rect::new(
            -f64::from(ambient_spread),
            f64::from(ambient_y) - f64::from(ambient_spread),
            w + f64::from(ambient_spread),
            h + f64::from(ambient_y) + f64::from(ambient_spread),
        );
        scene.set_paint(shadow_color(0.15));
        scene.fill_blurred_rounded_rect(
            &ambient_rect,
            radius,
            blur_to_std_dev(ambient_blur),
            false,
        );

        let (key_y, key_blur) = key_shadow_geometry(elevation);
        let key_rect = Rect::new(0.0, f64::from(key_y), w, h + f64::from(key_y));
        scene.set_paint(shadow_color(0.3));
        scene.fill_blurred_rounded_rect(&key_rect, radius, blur_to_std_dev(key_blur), false);
    }

    match &node.kind {
        NodeKind::Rect | NodeKind::Splitter(_) => {
            // A splitter's own visible grip/handle paints exactly like
            // a Rect -- `SplitterState` carries only the mechanism's
            // animatable position (§11.5), no separate appearance data,
            // since the universal `PaintProperties` every node already
            // has is all a divider's own background/corner-radius needs.
            let color = with_opacity(node.paint.background.current, node.paint.opacity.current);
            scene.set_paint(color);
            // M7 Phase 4 (§7.4): a real, active shape morph (`node.
            // paint.shape.current` non-empty) paints the current
            // interpolated silhouette instead of the plain rounded
            // rect -- `shape` defaults to `ShapeKey::empty()`, so a
            // node that never touches it renders byte-for-byte the
            // same `RoundedRect` fill as before this phase.
            if node.paint.shape.current.is_empty() {
                let radius = node.paint.corner_radius.current;
                let rect = RoundedRect::new(0.0, 0.0, w, h, radius);
                scene.fill_path(&rect.to_path(0.1));
            } else {
                scene.fill_path(&node.paint.shape.current.to_path());
            }
        }
        NodeKind::Text(state) => {
            let color = with_opacity(node.paint.background.current, node.paint.opacity.current);
            text.draw(
                scene,
                resources,
                state,
                TextPlacement {
                    x: 0.0,
                    y: 0.0,
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
        // M5 Phase 3 (§11.10, §11.11): replays `state.commands`, already
        // resolved ahead of time by `engine-py::Window.redraw_canvas`
        // (`canvas.rs`'s own module doc comment) -- every coordinate is
        // node-local, drawn under the same `composed` transform as
        // every other `NodeKind`, with zero special-casing beyond this
        // one match arm.
        NodeKind::Canvas(state) => {
            for command in &state.commands {
                match command {
                    DrawCommand::FillRect {
                        x,
                        y,
                        width,
                        height,
                        color,
                    } => {
                        scene.set_paint(*color);
                        scene.fill_path(&Rect::new(*x, *y, x + width, y + height).to_path(0.1));
                    }
                    DrawCommand::FillCircle {
                        cx,
                        cy,
                        radius,
                        color,
                    } => {
                        scene.set_paint(*color);
                        scene.fill_path(&Circle::new((*cx, *cy), *radius).to_path(0.1));
                    }
                    DrawCommand::StrokePath { path, color, width } => {
                        scene.set_paint(*color);
                        scene.set_stroke(Stroke::new(*width));
                        scene.stroke_path(path);
                    }
                }
            }
        }
    }

    // M4 Phase 5 (§7.3): the real ripple/hover state-layer paint --
    // `interaction_mut`/`Tree::dispatch`'s ripple-spawn/`update_hover`
    // were already real and correctly animating `InteractionState`
    // since M3 Phase 5 step 9 and M4 Phase 1 respectively, but nothing
    // in this real per-node walk ever painted it -- only the standalone
    // `build_ripple_scene` spike (above) ever drew a ripple, over a
    // synthetic single-button scene with no real `Tree` at all. Painted
    // after the node's own fill and before its children, matching real
    // MD3 (a state layer sits under a component's own content, e.g. an
    // icon/label). `interaction.tint` (M7 Phase 3, §7.1) is already an
    // MD3 "on-surface"-resolved plain `Color` by the time it reaches
    // here -- `engine-py::Window.set_theme`/`Node.enable_interaction`
    // resolve it, `engine-render` never touches `engine_md3` (§4).
    // `hover_opacity`/each ripple's own `opacity` are already the real,
    // live, animated 0.0..~0.12 values `engine-core` computed -- filling
    // with `with_opacity` at that exact value is a true no-op when it's
    // `0.0`, not a special-cased skip.
    if let Some(interaction) = &node.interaction {
        let radius = node.paint.corner_radius.current;
        let bounds = RoundedRect::new(0.0, 0.0, w, h, radius).to_path(0.1);

        scene.set_paint(with_opacity(
            interaction.tint,
            interaction.hover_opacity.current,
        ));
        scene.fill_path(&bounds);

        for ripple in &interaction.ripples {
            // `ripple.origin` is a real pointer coordinate captured by
            // `Tree::dispatch` in absolute canvas space (§11.9's own
            // stated scope: hit-testing/dispatch aren't transform-aware
            // until Phase 2) -- but this node's own paths are now drawn
            // in *local* space under `scene.set_transform(composed)`
            // (M5 Phase 1), so `origin` has to be mapped back into that
            // same local space via `composed`'s inverse before use.
            // `push_layer`'s own `clip_path` intersected with the fill
            // path below is exactly "this ripple, bounded to this
            // node's own shape" -- no second, nested `push_layer` call
            // needed to achieve that intersection.
            let local_origin = composed.inverse() * ripple.origin;
            let circle = Circle::new(local_origin, ripple.radius.current).to_path(0.1);
            scene.push_layer(
                Some(&circle),
                None,
                Some(ripple.opacity.current as f32),
                None,
                None,
            );
            scene.set_paint(interaction.tint);
            scene.fill_path(&bounds);
            scene.pop_layer();
        }
    }

    // M8 Phase 2 (§11.7): a `VirtualList`'s own materialized children
    // scroll and clip for real -- every other `NodeKind` recurses
    // exactly as before this phase (no other kind introduces a real
    // visual clip today, confirmed via direct read before this change,
    // so narrowing `visible` for any of them would wrongly cull
    // legitimately-overflowing content nothing here actually hides).
    if let NodeKind::VirtualList(state) = &node.kind {
        // The real clip: a local `(0, 0)-(w, h)` path, pushed under
        // this node's own `composed` transform (already active via
        // `scene.set_transform(composed)` above) -- `Scene::push_layer`
        // bakes its own `clip_path` into absolute strips at the moment
        // it's called (confirmed by reading `vello_hybrid`'s own
        // source), so it stays correctly anchored even though each
        // child below goes on to set its own transform.
        let clip_radius = node.paint.corner_radius.current;
        let clip = RoundedRect::new(0.0, 0.0, w, h, clip_radius).to_path(0.1);
        scene.push_layer(Some(&clip), None, None, None, None);

        // The real scroll offset: composed into the transform children
        // recurse with, not `layout_style` -- their own taffy layout
        // never changes, only where they're painted does. Vertical
        // only, a real, stated v1 scope limit (`PLAN.md`).
        let scrolled = composed * Affine::translate((0.0, -state.scroll_offset.current));

        // Reuses `bounds` (this node's own real composed absolute box,
        // already computed above for its own Phase 1 culling check) to
        // narrow `visible` for its children -- a materialized child
        // sitting outside the clip is now genuinely engine-culled too,
        // not just visually hidden behind the clip pushed above.
        let narrowed = visible.intersect(bounds);
        for &child in &node.children {
            paint_node(tree, child, scrolled, narrowed, scene, resources, text);
        }

        scene.pop_layer();
    } else {
        for &child in &node.children {
            paint_node(tree, child, composed, visible, scene, resources, text);
        }
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
