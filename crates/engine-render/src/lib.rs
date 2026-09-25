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

mod fonts;
mod geometry_cache;
mod image_cache;
mod text;

use engine_core::{
    ContentFit, DrawCommand, ICON_VIEWBOX_SIZE, Interpolate, NodeId, NodeKind, SCROLLBAR_MARGIN,
    SCROLLBAR_THICKNESS, ScrollViewState, TimePickerDialMode, Tree, VirtualListState,
};
use peniko::Color;
use peniko::kurbo::{Affine, BezPath, Circle, Line, Point, Rect, RoundedRect, Shape, Stroke, Vec2};
use vello_hybrid::{RenderSize, RenderTargetConfig, Renderer, Resources, Scene};

pub use fonts::{NoFontFacesFound, register_font};
pub use geometry_cache::GeometryCache;
pub use text::{MONOSPACE_FONT_FAMILY, TextPlacement, TextRenderer};

/// MD3 seed-adjacent purple (#6750A4) -- an arbitrary but deliberate
/// starting color, not vello_hybrid's own default, so a wrong pixel in a
/// readback test can't be confused with "the renderer drew nothing and
/// left its own default."
pub const INITIAL_COLOR: Color = Color::from_rgba8(0x67, 0x50, 0xA4, 0xFF);

/// Returns `color` with its alpha multiplied by `opacity` (0.0..=1.0).
/// M95: multiplied, where it used to *replace* the alpha -- so a color's
/// own alpha (a translucent fill, a transparent box) finally renders, as
/// the M93 target API's `(r, g, b, a)` colors require.
pub fn with_opacity(color: Color, opacity: f64) -> Color {
    Color {
        components: [
            color.components[0],
            color.components[1],
            color.components[2],
            color.components[3] * opacity as f32,
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
/// `geometry` (M34 Phase 1, §5, §8) is the identical kind of caller-
/// owned, cross-frame cache, for `Rect`/`Splitter`'s own tessellated
/// fill/border paths -- see `GeometryCache`'s own doc comment.
pub fn build_tree_scene(
    tree: &Tree,
    root: NodeId,
    width: u16,
    height: u16,
    resources: &mut Resources,
    text: &mut TextRenderer,
    geometry: &mut GeometryCache,
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
        geometry,
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

/// M22 Phase 2 (§16.1): the real `(source_region, transform)` pair
/// `Scene::draw_texture_rects` needs for a given `ContentFit` --
/// `Scene::draw_texture_rects`'s own doc comment states the real
/// contract this relies on: the destination drawn is always `transform`
/// applied to a rect whose size is `source_region`'s own real pixel
/// width/height, not the node's `(w, h)` box directly. `Fill` reuses
/// the identical, unchanged math Phase 1 already shipped (the full
/// image, non-uniformly scaled to `(w, h)` exactly). `Contain` scales
/// the full image *uniformly* by the smaller of the two axis ratios
/// (so it never overflows either axis) and centers the result inside
/// `(w, h)` via a real translate composed on top of the scale --
/// `source_region` stays the full image, since nothing is cropped.
/// `Cover` instead crops `source_region` to the same aspect ratio as
/// `(w, h)` (centered within the full image), so a uniform scale of
/// *that* cropped region already lands exactly on `(w, h)` with zero
/// overflow -- deliberately not "scale the full image up and rely on
/// an implicit clip," since `NodeKind::Image` has none today and
/// adding one is real, unneeded complexity no real use case here asks
/// for.
fn image_sample_rect(
    w: f64,
    h: f64,
    img_width: u32,
    img_height: u32,
    fit: ContentFit,
) -> (vello_common::geometry::RectU16, Affine) {
    let iw = f64::from(img_width);
    let ih = f64::from(img_height);
    let full = vello_common::geometry::RectU16 {
        x0: 0,
        y0: 0,
        x1: img_width as u16,
        y1: img_height as u16,
    };
    match fit {
        ContentFit::Fill => (full, Affine::scale_non_uniform(w / iw, h / ih)),
        ContentFit::Contain => {
            let scale = (w / iw).min(h / ih);
            let offset = ((w - iw * scale) / 2.0, (h - ih * scale) / 2.0);
            (full, Affine::translate(offset) * Affine::scale(scale))
        }
        ContentFit::Cover => {
            let box_aspect = w / h;
            let img_aspect = iw / ih;
            let (sx, sy, sw, sh) = if img_aspect > box_aspect {
                let sw = ih * box_aspect;
                (((iw - sw) / 2.0).max(0.0), 0.0, sw, ih)
            } else {
                let sh = iw / box_aspect;
                (0.0, ((ih - sh) / 2.0).max(0.0), iw, sh)
            };
            let x0 = sx.round() as u16;
            let y0 = sy.round() as u16;
            let x1 = ((sx + sw).round() as i64)
                .clamp(i64::from(x0) + 1, i64::from(img_width))
                .max(0) as u16;
            let y1 = ((sy + sh).round() as i64)
                .clamp(i64::from(y0) + 1, i64::from(img_height))
                .max(0) as u16;
            let region = vello_common::geometry::RectU16 { x0, y0, x1, y1 };
            let transform =
                Affine::scale_non_uniform(w / f64::from(x1 - x0), h / f64::from(y1 - y0));
            (region, transform)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_node(
    tree: &Tree,
    id: NodeId,
    parent_transform: Affine,
    visible: Rect,
    scene: &mut Scene,
    resources: &mut Resources,
    text: &mut TextRenderer,
    geometry: &mut GeometryCache,
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

    // M95: group opacity -- the node and its whole subtree composite as
    // one layer at `opacity`, so a fading container fades its children
    // too. The node's own paints below are then fully opaque
    // (`own_alpha`), their colors' own alpha applying through
    // `with_opacity`. A fully transparent node skips its subtree.
    let opacity = node.paint.opacity.current;
    if opacity <= 0.0 {
        return;
    }
    let layered = opacity < 1.0;
    if layered {
        scene.push_layer(None, None, Some(opacity as f32), None, None);
    }
    let own_alpha = 1.0;

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
    // M25 Phase 2 (§5, §6): a real, previously-missing compounding --
    // a fading node's own real shadow must fade with it (the same real
    // expectation any CSS `opacity` compositing already has: a shadow
    // is part of what "how visible is this node" governs, not a
    // separate, always-opaque layer underneath it). `<= 0.0` also
    // skips the shadow now, matching `elevation <= 0.0`'s own existing
    // "don't draw an invisible thing" precedent.
    let node_opacity = own_alpha;
    if elevation > 0.0 && node_opacity > 0.0 {
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
        scene.set_paint(shadow_color(0.15 * node_opacity as f32));
        scene.fill_blurred_rounded_rect(
            &ambient_rect,
            radius,
            blur_to_std_dev(ambient_blur),
            false,
        );

        let (key_y, key_blur) = key_shadow_geometry(elevation);
        let key_rect = Rect::new(0.0, f64::from(key_y), w, h + f64::from(key_y));
        scene.set_paint(shadow_color(0.3 * node_opacity as f32));
        scene.fill_blurred_rounded_rect(&key_rect, radius, blur_to_std_dev(key_blur), false);
    }

    // M95: the `shadows` list, CSS `box-shadow`'s model -- each is the
    // node's own rounded box, offset, grown by `spread` (its corners too),
    // and blurred; the first listed paints on top, so the list paints in
    // reverse. A node with differing corner radii shadows with their mean,
    // the blurred rounded rect taking one radius.
    let shadows = &node.paint.shadows.current.0;
    if !shadows.is_empty() {
        let radius = match &node.paint.corner_radii_override {
            Some(radii) => radii.current.0.iter().sum::<f64>() / 4.0,
            None => node.paint.corner_radius.current,
        };
        for shadow in shadows.iter().rev() {
            if shadow.color.components[3] <= 0.0 {
                continue;
            }
            let rect = Rect::new(
                shadow.offset_x - shadow.spread,
                shadow.offset_y - shadow.spread,
                w + shadow.offset_x + shadow.spread,
                h + shadow.offset_y + shadow.spread,
            );
            let shadow_radius = (radius + shadow.spread).max(0.0);
            scene.set_paint(with_opacity(shadow.color, own_alpha));
            if shadow.blur > 0.0 {
                scene.fill_blurred_rounded_rect(
                    &rect,
                    shadow_radius as f32,
                    blur_to_std_dev(shadow.blur as f32),
                    false,
                );
            } else {
                scene.fill_path(&RoundedRect::from_rect(rect, shadow_radius).to_path(0.1));
            }
        }
    }

    match &node.kind {
        NodeKind::Rect | NodeKind::Splitter(_) | NodeKind::LoadingIndicator(_) => {
            // A splitter's own visible grip/handle paints exactly like
            // a Rect -- `SplitterState` carries only the mechanism's
            // animatable position (§11.5), no separate appearance data,
            // since the universal `PaintProperties` every node already
            // has is all a divider's own background/corner-radius needs.
            // M39 Phase 2 (§5, §7): a real `LoadingIndicator` joins this
            // same arm too -- its own real appearance is entirely
            // `PaintProperties.shape` (`Tree::tick_all`'s own new
            // per-`LoadingIndicator` case keeps it perpetually non-
            // empty from the very first tick onward), so the identical
            // real "active shape morph paints the current silhouette"
            // branch just below already paints it correctly with zero
            // new paint code.
            let color = with_opacity(node.paint.background.current, own_alpha);
            scene.set_paint(color);
            // M7 Phase 4 (§7.4): a real, active shape morph (`node.
            // paint.shape.current` non-empty) paints the current
            // interpolated silhouette instead of the plain rounded
            // rect -- `shape` defaults to `ShapeKey::empty()`, so a
            // node that never touches it renders byte-for-byte the
            // same `RoundedRect` fill as before this phase.
            if node.paint.shape.current.is_empty() {
                // M30 Phase 1 Step 4 (§5, §7): `corner_radii_override`
                // (`[top_left, top_right, bottom_right, bottom_left]`)
                // wins when set -- `Segmented Button`'s own real need
                // (a first/last segment rounded only on its outer
                // edge). `None` (every node before this step) falls
                // through to the identical uniform-scalar `RoundedRect`
                // this arm always painted.
                // M34 Phase 1 (§5, §8): the real path itself comes from
                // `geometry` now -- re-tessellated only when this
                // node's own `w`/`h`/radius genuinely changed since its
                // last paint, not rebuilt from scratch every frame
                // (`GeometryCache`'s own doc comment has the real,
                // measured motivation).
                let path = match node
                    .paint
                    .corner_radii_override
                    .as_ref()
                    .map(|r| r.current.0)
                {
                    Some(radii) => geometry.rounded_rect_fill_per_corner(id, w, h, radii),
                    None => geometry.rounded_rect_fill(id, w, h, node.paint.corner_radius.current),
                };
                scene.fill_path(path);
            } else {
                scene.fill_path(&node.paint.shape.current.to_path());
            }
            // M30 Phase 1 (§5, §7): a real stroked border -- MD3's
            // Outlined button variant is the real consumer, but this is
            // universal `PaintProperties`, not `Button`-specific, the
            // same "any Rect/Splitter can use it" reach `background`/
            // `corner_radius` already have. Inset by half the stroke
            // width so the border paints entirely *inside* this node's
            // own bounds (kurbo strokes are centered on the path by
            // default) -- a border never grows past the node's own
            // taffy-computed box the way a naive un-inset stroke would.
            // Skipped entirely at `border_width <= 0.0`, the same
            // "off unless a caller opts in" contract `elevation`
            // already established.
            let border_width = node.paint.border_width.current;
            if border_width > 0.0 {
                let inset = border_width / 2.0;
                // M38 Phase 4 (§5, §7): the border path now matches
                // whichever real fill geometry this node actually used
                // just above, closing a real, previously-dormant gap --
                // `RectPathParams::PerCornerBorder`'s own doc comment
                // has the full story (`geometry_cache.rs`).
                let border_path: std::borrow::Cow<'_, BezPath> =
                    if !node.paint.shape.current.is_empty() {
                        // M39 Phase 3 (§5, §7): a real, active shape
                        // morph now strokes a real *inset* polygon
                        // (`ShapeKey::inset_path`'s own doc comment has
                        // the full real algorithm and its stated scope
                        // limit) instead of the raw silhouette centered
                        // -- closes M38 Phase 4's own real, previously-
                        // stated v1 gap where a shape-morphed border
                        // could sit up to half its own width outside
                        // the fill's own edge.
                        std::borrow::Cow::Owned(node.paint.shape.current.inset_path(inset))
                    } else if let Some(radii) = node
                        .paint
                        .corner_radii_override
                        .as_ref()
                        .map(|r| r.current.0)
                    {
                        std::borrow::Cow::Borrowed(
                            geometry.rounded_rect_border_per_corner(id, w, h, radii, inset),
                        )
                    } else {
                        let radius = (node.paint.corner_radius.current - inset).max(0.0);
                        std::borrow::Cow::Borrowed(
                            geometry.rounded_rect_border(id, w, h, radius, inset),
                        )
                    };
                let border_color = with_opacity(node.paint.border_color.current, own_alpha);
                scene.set_paint(border_color);
                scene.set_stroke(Stroke::new(border_width));
                scene.stroke_path(&border_path);
            }
        }
        NodeKind::Text(state) | NodeKind::Link(state) => {
            let color = with_opacity(node.paint.background.current, own_alpha);
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
                id,
            );
        }
        // M15 Phase 1 (§5, §16.7): unlike `NodeKind::Text` (a plain
        // label with no visible box, `background` repurposed as the
        // glyph color), a real `TextField` is a genuinely boxed input
        // -- `background` paints its own real fill first (the same
        // `RoundedRect` fill every other boxed `NodeKind` uses), and
        // `draw_field` paints its content/caret/selection on top in a
        // fixed, real, not-yet-theme-aware color (the same "real but
        // not yet theme-aware" scope `Checkbox`'s own hardcoded white
        // checkmark, M14 Phase 1, already established -- `engine-render`
        // has no `engine-md3` dependency, §4, to resolve a real
        // on-surface token from here).
        NodeKind::TextField(state) => {
            let radius = node.paint.corner_radius.current;
            let bg = with_opacity(node.paint.background.current, own_alpha);
            scene.set_paint(bg);
            // M38 Phase 7 (§5, §8): a real, previously-uncached fill --
            // direct grep before this phase found this arm still built
            // a fresh `RoundedRect::to_path` every frame, despite M38
            // Phase 1's own completion note claiming otherwise (a real,
            // honest correction: that claim was wrong, caught here by
            // checking the actual current source rather than trusting
            // the prior write-up). Routed through `geometry` now, the
            // same real cache `Checkbox`/`Switch`/`Terminal` already
            // use -- also reused directly below for this same node's
            // own real clip layer, since both need the identical path.
            scene.fill_path(geometry.rounded_rect_fill(id, w, h, radius));

            // M20 Phase 2 (§7.1, §7.3): the real resolved color now
            // comes from `state.text_tint` -- plain dark by default
            // (byte-for-byte the old hardcoded literal), a real
            // resolved MD3 "on-surface" color once `Window.set_theme`
            // has pushed one in.
            let text_color = with_opacity(state.text_tint, own_alpha);
            // M38 Phase 7 (§5, §8): a real, genuinely overflowing
            // `multiline` field now clips its own painted content to
            // its own box and shifts it up by `scroll_offset` -- a
            // real, previously-existing gap (confirmed by direct read
            // before this phase: no clip layer existed here at all,
            // so overflowing text simply painted past the node's own
            // bounds). Single-line fields never set `scroll_offset`
            // (`Tree::scroll_text_field_caret_into_view`'s own real
            // multiline-only guard), so `y` stays `0.0` for them,
            // byte-for-byte unchanged. Clipping only when `multiline`
            // (not universally): a single-line field's own real
            // horizontal overflow behavior is unaffected, a real,
            // deliberate v1 scope match to this phase's own "Code
            // Editor" title, not a general text-overflow feature.
            // M39 Phase 1 (§5, §8): `horizontal_scroll_offset`'s own
            // real paint-time shift, the identical real mechanism
            // `scroll_offset`'s own `y` shift just above already
            // establishes -- `0.0` for every field that never sets it
            // (every single-line field, and every multiline field
            // whose own longest real line still fits the box).
            let text_at = TextPlacement {
                x: -state.horizontal_scroll_offset.current,
                y: -state.scroll_offset.current,
                max_width: w as f32,
                color: text_color,
            };
            let show_caret = tree.focused() == Some(id);
            if state.multiline {
                let clip = geometry.rounded_rect_fill(id, w, h, radius);
                scene.push_layer(Some(clip), None, None, None, None);
                text.draw_field(scene, resources, state, text_at, show_caret, id);
                scene.pop_layer();
            } else {
                text.draw_field(scene, resources, state, text_at, show_caret, id);
            }
        }
        // M30 Phase 9 Step 4 (§5, §8, §10): a real terminal's own cell
        // grid -- `background` paints the real box fill first (the
        // same `RoundedRect` fill `TextField`'s own arm just above
        // already establishes), then `draw_terminal` paints every real
        // cell's own background/glyph on top, plus the caret, in a
        // fixed, not-yet-theme-aware color (the same real scope
        // `TextField`'s own arm already accepts -- `engine-render` has
        // no `engine-md3` dependency to resolve a real theme token
        // from here, §4).
        NodeKind::Terminal(state) => {
            let radius = node.paint.corner_radius.current;
            let bg = with_opacity(node.paint.background.current, own_alpha);
            scene.set_paint(bg);
            scene.fill_path(geometry.rounded_rect_fill(id, w, h, radius));

            let cursor_color =
                with_opacity(peniko::Color::from_rgba8(0x1C, 0x1B, 0x1F, 0xFF), own_alpha);
            text.draw_terminal(
                scene,
                resources,
                state,
                TextPlacement {
                    x: 0.0,
                    y: 0.0,
                    max_width: w as f32,
                    color: cursor_color,
                },
                tree.focused() == Some(id),
                id,
            );
        }
        // A `VirtualList` container paints nothing itself, same as
        // `Container` -- it exists purely to give `taffy` something to
        // lay its (windowed) children out against; the recursive walk
        // below already only ever sees `VirtualListState::materialized`'s
        // small real subset, never `item_count`, with zero changes
        // needed here (§14 step 15, §11.7).
        // M30 Phase 9 Step 5 (§5, §7, §11.7): a `Carousel` paints
        // nothing of its own beyond the generic background/corner-
        // radius box every `NodeKind` already gets above -- the
        // identical real "exists purely to give `taffy` something to
        // lay its children out against" shape `Container`/`VirtualList`
        // already have. Real item geometry (position/width) is already
        // fully baked into each child's own `layout_style` by `Tree::
        // sync_carousel_layouts`, so nothing kind-specific is needed
        // here at all; the clip below is this kind's only other real
        // paint-time behavior.
        // M36 Phase 1 (§5, §7, §11.7): `ScrollView` paints nothing of
        // its own either, the identical real shape -- its one real
        // child's own absolute position is already baked into `layout_
        // style` by `Tree::sync_scroll_view_layouts`, so the ordinary
        // recursive walk below (composed transform only, no extra
        // paint-time offset) already paints it in the right place; the
        // unconditional clip below is this kind's only other real
        // paint-time behavior, mirroring `Carousel`'s own.
        NodeKind::Container
        | NodeKind::VirtualList(_)
        | NodeKind::Carousel(_)
        | NodeKind::ScrollView(_) => {}
        // M5 Phase 3 (§11.10, §11.11): replays `state.commands`, already
        // resolved ahead of time by `engine-py::Window.redraw_canvas`
        // (`canvas.rs`'s own module doc comment) -- every coordinate is
        // node-local, drawn under the same `composed` transform as
        // every other `NodeKind`, with zero special-casing beyond this
        // one match arm.
        // M25 Phase 2 (§5, §6): every real `DrawCommand`'s own color
        // now compounds with `own_alpha` -- a real,
        // previously-missing gap (confirmed via direct read: this arm
        // painted every command's own raw color, the only real
        // `NodeKind` arm in this whole match that never touched the
        // node's own universal opacity at all).
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
                        scene.set_paint(with_opacity(*color, own_alpha));
                        scene.fill_path(&Rect::new(*x, *y, x + width, y + height).to_path(0.1));
                    }
                    DrawCommand::FillCircle {
                        cx,
                        cy,
                        radius,
                        color,
                    } => {
                        scene.set_paint(with_opacity(*color, own_alpha));
                        scene.fill_path(&Circle::new((*cx, *cy), *radius).to_path(0.1));
                    }
                    DrawCommand::StrokePath { path, color, width } => {
                        scene.set_paint(with_opacity(*color, own_alpha));
                        scene.set_stroke(Stroke::new(*width));
                        scene.stroke_path(path);
                    }
                }
            }
        }
        // M14 Phase 1 (§5, §7.3): the box itself paints exactly like a
        // Rect (same rounded-rect fill), then a real checkmark tick
        // path strokes on top, its own opacity driven directly by
        // `check_progress` -- 0.0 (unchecked) paints no visible mark at
        // all, 1.0 (checked) paints it fully opaque, and any value
        // between (mid-animation) fades it in/out smoothly. M20 Phase 1
        // (§7.1, §7.3): the mark's own real color now comes from
        // `state.mark_tint` -- plain white by default (byte-for-byte
        // the old hardcoded literal), a real resolved MD3 "on-surface"
        // color once `Window.set_theme` has pushed one in.
        NodeKind::Checkbox(state) => {
            let color = with_opacity(node.paint.background.current, own_alpha);
            scene.set_paint(color);
            let radius = node.paint.corner_radius.current;
            // M38 Phase 1 (§5, §7, §8): the real box path is byte-for-
            // byte the same geometry `Rect`'s own fill already caches
            // -- reuses `GeometryCache::rounded_rect_fill` directly
            // rather than a second, parallel Checkbox-only cache slot.
            scene.fill_path(geometry.rounded_rect_fill(id, w, h, radius));

            if state.check_progress.current > 0.0 {
                let mut mark = BezPath::new();
                mark.move_to((w * 0.2, h * 0.55));
                mark.line_to((w * 0.42, h * 0.75));
                mark.line_to((w * 0.8, h * 0.25));
                // M25 Phase 2 (§5, §6): a real, previously-missing
                // compounding -- the checkmark's own real alpha
                // multiplied only `check_progress` before this, never
                // `own_alpha` too, so a checked
                // checkbox mid-fade-out would show its own checkmark
                // at full alpha while its box correctly faded. Two
                // independent real "how visible" factors, multiplied
                // together the same way any compositing pipeline
                // compounds independent alpha sources.
                scene.set_paint(with_opacity(
                    state.mark_tint,
                    state.check_progress.current * own_alpha,
                ));
                scene.set_stroke(Stroke::new((w.min(h) * 0.12).max(1.0)));
                scene.stroke_path(&mark);
            }
        }
        // M30 Phase 2 Step 1 (§5, §7.3): `Checkbox`'s own real anatomy,
        // mirrored -- a stroked ring (not a filled box, MD3's real
        // radio-button shape) plus a real filled dot that scales in
        // with `select_progress`. The ring's own color rides the same
        // `select_progress` timeline via `Interpolate for peniko::
        // Color` (real since §5's own animation core, `lerp_rect`
        // under the hood) rather than snapping instantly between
        // `unselected_tint`/`selected_tint` -- a real, smooth color
        // transition, not two disconnected static states.
        NodeKind::RadioButton(state) => {
            let ring_color = state
                .unselected_tint
                .interpolate(&state.selected_tint, state.select_progress.current);
            let stroke_width = (w.min(h) * 0.1).max(2.0);
            let ring_radius = (w.min(h) / 2.0) - stroke_width / 2.0;
            scene.set_paint(with_opacity(ring_color, own_alpha));
            scene.set_stroke(Stroke::new(stroke_width));
            // M38 Phase 1 (§5, §7, §8): the real ring/dot paths, now
            // cached -- see `GeometryCache::circle_primary`/
            // `circle_secondary`'s own doc comments for why a single
            // node needs two independent real circle cache slots.
            scene.stroke_path(geometry.circle_primary(id, w / 2.0, h / 2.0, ring_radius.max(0.0)));

            if state.select_progress.current > 0.0 {
                let dot_radius = (w.min(h) / 2.0) * 0.5 * state.select_progress.current;
                scene.set_paint(with_opacity(
                    state.selected_tint,
                    state.select_progress.current * own_alpha,
                ));
                scene.fill_path(geometry.circle_secondary(id, w / 2.0, h / 2.0, dot_radius));
            }
        }
        // M30 Phase 2 Step 2 (§5, §7.3): a real MD3 switch -- track
        // fill (color-interpolated between `track_off_tint`/`track_
        // on_tint`, `RadioButton`'s own real technique reused), a real
        // fading outline stroke (only `track_outline_tint`, MD3's own
        // real separate unselected-only role -- opacity scaled by
        // `1.0 - toggle_progress`, so it's gone by the time the switch
        // is fully on), and a handle that both slides *and* grows.
        // `handle_radius`/`cx`'s own real formula: verified real MD3
        // ratios (16dp/32dp track height unselected, 24dp/32dp
        // selected, `(32-16)/2=8` unselected padding, `(32-24)/2=4`
        // selected padding) collapse to one clean symmetric travel
        // range, `h * 0.5` from each edge, at both ends -- not a
        // coincidence: `handle_radius + padding` is `8+8=16=h*0.5`
        // unselected and `12+4=16=h*0.5` selected, the same real
        // total inset either way.
        NodeKind::Switch(state) => {
            let t = state.toggle_progress.current;
            let track_color = state.track_off_tint.interpolate(&state.track_on_tint, t);
            let track_radius = h / 2.0;
            scene.set_paint(with_opacity(track_color, own_alpha));
            // M38 Phase 1 (§5, §7, §8): the real track/outline/handle
            // paths, now cached -- the track and outline are byte-for-
            // byte the same real geometry `Rect`'s own fill/border
            // already cache (reused directly, not a parallel Switch-
            // only cache slot); the handle shares `circle_primary`
            // with `RadioButton`'s own ring, since neither kind ever
            // has both a `circle_primary` and a `RadioButton`-style
            // ring/dot pair at once.
            scene.fill_path(geometry.rounded_rect_fill(id, w, h, track_radius));

            if t < 1.0 {
                let stroke_width = (h * 0.06).max(1.5);
                let inset = stroke_width / 2.0;
                let outline_radius = (track_radius - inset).max(0.0);
                scene.set_paint(with_opacity(
                    state.track_outline_tint,
                    (1.0 - t) * own_alpha,
                ));
                scene.set_stroke(Stroke::new(stroke_width));
                scene.stroke_path(geometry.rounded_rect_border(id, w, h, outline_radius, inset));
            }

            let handle_radius = h * (0.25 + 0.125 * t);
            let handle_color = state.handle_off_tint.interpolate(&state.handle_on_tint, t);
            let cx = h * 0.5 + t * (w - h);
            scene.set_paint(with_opacity(handle_color, own_alpha));
            scene.fill_path(geometry.circle_primary(id, cx, h / 2.0, handle_radius));
        }
        // M30 Phase 3 Step 2 (§5, §7): a real MD3 linear progress
        // indicator -- the track (`track_tint`, spanning the node's
        // own full width) painted first, then the indicator on top,
        // its own real width `value.current * w` -- the identical
        // real "value is a fraction of the node's own box" technique
        // `NodeKind::Slider`'s own `thumb_position * w` already uses,
        // just filling a growing bar instead of moving a fixed-size
        // thumb. Real MD3 anatomy: `corner-none` on both (a flat
        // rectangle, not rounded), confirmed from Material Web's own
        // token source, not assumed rounded like most of this
        // catalog's other shapes.
        NodeKind::LinearProgress(state) => {
            scene.set_paint(with_opacity(state.track_tint, own_alpha));
            scene.fill_path(&Rect::new(0.0, 0.0, w, h).to_path(0.1));

            let indicator_width = state.value.current.clamp(0.0, 1.0) * w;
            if indicator_width > 0.0 {
                scene.set_paint(with_opacity(state.indicator_tint, own_alpha));
                scene.fill_path(&Rect::new(0.0, 0.0, indicator_width, h).to_path(0.1));
            }
        }
        // M30 Phase 3 Step 2 (§5, §7): `LinearProgress`'s own real
        // circular sibling -- a stroked arc from real MD3's own
        // 12-o'clock start (`-PI/2`), sweeping clockwise by `value *
        // 2*PI`. No separate background track ring painted here --
        // real MD3 anatomy genuinely has none for this indicator
        // (`CircularProgressState`'s own doc comment has the real,
        // confirmed finding). Stroke width is the real MD3 4dp/48dp
        // ratio, scaled to whatever real size this node's own box is,
        // the same proportional-to-own-box technique `RadioButton`'s
        // ring/`Switch`'s track outline already use rather than a
        // fixed literal px value.
        NodeKind::CircularProgress(state) => {
            let stroke_width = (w.min(h) * (4.0 / 48.0)).max(1.0);
            let radius = (w.min(h) / 2.0) - stroke_width / 2.0;
            let sweep = state.value.current.clamp(0.0, 1.0) * std::f64::consts::TAU;
            if sweep > 0.0 {
                // M38 Phase 1 (§5, §7, §8): the real arc path, now
                // cached -- see `GeometryCache::arc`'s own doc comment.
                let arc_path = geometry.arc(
                    id,
                    w / 2.0,
                    h / 2.0,
                    radius.max(0.0),
                    -std::f64::consts::FRAC_PI_2,
                    sweep,
                );
                scene.set_paint(with_opacity(state.indicator_tint, own_alpha));
                scene.set_stroke(Stroke::new(stroke_width));
                scene.stroke_path(arc_path);
            }
        }
        // M14 Phase 2 (§5, §7.3): a real track (a thin bar spanning the
        // node's own full width, vertically centered) plus a real
        // thumb (a filled circle at `thumb_position * w`, the node's
        // own real `background` color -- the same universal field
        // every other `NodeKind`'s primary fill already uses). M20
        // Phase 1 (§7.1, §7.3): the track's own real color now comes
        // from `state.track_tint` -- plain gray by default (byte-for-
        // byte the old hardcoded literal), a real resolved MD3
        // "on-surface" color once `Window.set_theme` has pushed one in.
        NodeKind::Slider(state) => {
            let track_height = (h * 0.15).max(2.0);
            let track_rect = Rect::new(0.0, (h - track_height) / 2.0, w, (h + track_height) / 2.0);
            // M25 Phase 2 (§5, §6): a real, previously-missing
            // compounding -- only the thumb (below) multiplied by
            // `own_alpha`; the track painted its own
            // real `track_tint` raw, a real internal inconsistency
            // within this one `NodeKind`.
            scene.set_paint(with_opacity(state.track_tint, own_alpha));
            scene.fill_path(&track_rect.to_path(0.1));

            let thumb_radius = (h * 0.4).max(4.0);
            let thumb_x = state.thumb_position.current * w;
            let thumb_color = with_opacity(node.paint.background.current, own_alpha);
            scene.set_paint(thumb_color);
            scene.fill_path(&Circle::new((thumb_x, h / 2.0), thumb_radius).to_path(0.1));
        }
        // M22 Phase 1 (§5): **real finding, confirmed by a failing
        // test, not assumed:** `vello_hybrid`'s ordinary `set_paint`+
        // `fill_path` path panics on CPU-side pixel data
        // (`ImageSource::Pixmap`) -- "pixmap image sources are not
        // supported by Vello Hybrid" -- only a pre-registered,
        // externally-owned GPU texture (`ImageSource::OpaqueId`) is
        // ever accepted by its own wgpu renderer. `Scene::
        // draw_texture_rects` is the one real, currently-supported
        // path (`image_cache`'s own module doc comment has the full
        // investigation): `id`'s own deterministic `TextureId`
        // (`image_cache::texture_id_for`) must already be bound in
        // the `TextureBindings` `FrameRenderer::render` hands to
        // `vello_hybrid` -- `FrameRenderer::sync_image_textures`,
        // called once per frame before `render`, is what guarantees
        // that. `source_region` is the image's own full real pixel
        // extent; `transform` scales that local rect up to the node's
        // own `(w, h)` box -- Phase 1's own stated "stretched to fill"
        // scope, composed on top of `scene.set_transform(composed)`
        // (already active above). Real content-fit modes (cover/
        // contain) are Phase 2's, §16.1.
        // M25 Phase 2 (§5, §6): a real, previously-missing compounding
        // -- `Scene::draw_texture_rects` has no opacity parameter of
        // its own at all (confirmed via direct source read), unlike
        // every `set_paint`-based fill in this match. `push_layer`'s
        // own real `opacity` parameter (the identical mechanism the
        // ripple/hover overlay above already uses for the same real
        // "an opacity-only layer, no clip") wraps the draw instead --
        // skipped entirely at `opacity <= 0.0`, the same "don't draw
        // an invisible thing" precedent the elevation section above
        // already established.
        NodeKind::Image(state) => {
            let img_width = state.image.width;
            let img_height = state.image.height;
            let node_opacity = own_alpha;
            if img_width > 0 && img_height > 0 && node_opacity > 0.0 {
                let (source_region, transform) =
                    image_sample_rect(w, h, img_width, img_height, state.content_fit);
                scene.push_layer(None, None, Some(node_opacity as f32), None, None);
                scene.draw_texture_rects(
                    image_cache::texture_id_for(id),
                    peniko::ImageQuality::Medium,
                    [vello_hybrid::SampleRect {
                        source_region,
                        transform,
                    }],
                );
                scene.pop_layer();
            }
        }
        // M23 Phase 1 (§1, §3): `state.path` is real, already-parsed
        // `BezPath` data in the icon's own fixed `0..ICON_VIEWBOX_SIZE`
        // SVG-source coordinate space (§1's own real "MD3's own icon
        // set embedded as `kurbo::BezPath` data" design) -- every
        // curated icon shares the identical real `viewBox="0 -960 960
        // 960"`, confirmed via a real fetch of eight distinct icons
        // directly from Google's own CDN, so a single fixed transform
        // (translate the real negative-y range up into `0..960`, then
        // scale into the node's own local box) applies uniformly.
        // `Scene::fill_path` always draws in whatever transform is
        // currently active (unlike `Image`'s own `draw_texture_rects`,
        // whose `SampleRect.transform` is a real, separate per-call
        // argument needing no such restore) -- `composed` is put back
        // immediately after, since the post-match ripple/hover overlay
        // below relies on it still being active.
        // M95 (D4): any vector path, in node-local pixels once fitted
        // into the view box. The fill is the whole path; the stroke is
        // the trimmed outline, centered on the path as in SVG, with round
        // caps and joins, and its width stays in pixels however the view
        // box scales the path.
        NodeKind::Path(state) => {
            let (fill, stroke) = state.geometry(w, h);
            let fill_color = node.paint.background.current;
            if fill_color.components[3] > 0.0 {
                scene.set_paint(with_opacity(fill_color, own_alpha));
                scene.fill_path(&fill);
            }
            let stroke_width = node.paint.border_width.current;
            if stroke_width > 0.0 && !stroke.elements().is_empty() {
                scene.set_paint(with_opacity(node.paint.border_color.current, own_alpha));
                scene.set_stroke(
                    Stroke::new(stroke_width)
                        .with_caps(peniko::kurbo::Cap::Round)
                        .with_join(peniko::kurbo::Join::Round),
                );
                scene.stroke_path(&stroke);
            }
        }
        NodeKind::Icon(state) => {
            let icon_scale = 1.0 / ICON_VIEWBOX_SIZE;
            let icon_transform = Affine::scale_non_uniform(w * icon_scale, h * icon_scale)
                * Affine::translate((0.0, ICON_VIEWBOX_SIZE));
            // M35 Phase 2 (§5, §8): `Split Button`'s own real trailing-
            // icon rotation -- a real, fresh `Affine::rotate` built
            // straight from `state.rotation.current` (degrees) every
            // frame, composed in *local* node space (around this
            // node's own real center, `(w/2, h/2)`) before the fixed
            // viewBox-to-local `icon_transform` above, so the icon
            // visually spins in place regardless of its own internal
            // viewBox geometry. `IconState`'s own doc comment has the
            // full real reason this is a dedicated scalar field, not
            // routed through `PaintProperties.transform`.
            let rotation = if state.rotation.current != 0.0 {
                Affine::translate((w / 2.0, h / 2.0))
                    * Affine::rotate(state.rotation.current.to_radians())
                    * Affine::translate((-w / 2.0, -h / 2.0))
            } else {
                Affine::IDENTITY
            };
            scene.set_transform(composed * rotation * icon_transform);
            scene.set_paint(with_opacity(state.tint.current, own_alpha));
            scene.fill_path(&state.path);
            scene.set_transform(composed);
        }
        // M39 Phase 2 Step 2 (§5, §7): a real MD3 Time Picker dial --
        // see `TimePickerDialState`'s own doc comment for the full
        // real design and its stated v1 scope limits (no digit
        // labels; plain tick dots stand in for them here). Angle
        // convention is byte-for-byte `CircularProgress`'s own arm
        // above: `-PI/2` (12 o'clock) is the real zero point,
        // sweeping clockwise -- `Tree::update_time_picker_dial_drag`
        // (`engine-core`) already established this same convention
        // for the reverse (pointer -> angle) direction, so paint and
        // drag agree on where every real hour/minute position sits.
        NodeKind::TimePickerDial(state) => {
            let face_radius = w.min(h) / 2.0;
            let (cx, cy) = (w / 2.0, h / 2.0);
            let center = Point::new(cx, cy);

            scene.set_paint(with_opacity(state.face_tint, own_alpha));
            scene.fill_path(&Circle::new(center, face_radius).to_path(0.1));

            // 12 real tick-dot positions -- the honest v1 stand-in for
            // real MD3's own painted digit labels (see the struct doc
            // comment for why no text is shaped here).
            let tick_tint = with_opacity(state.hand_tint, 0.4 * own_alpha);
            let tick_radius = (face_radius * 0.04).max(1.0);
            let tick_orbit = face_radius * 0.84;
            scene.set_paint(tick_tint);
            for i in 0..12 {
                let angle =
                    -std::f64::consts::FRAC_PI_2 + (f64::from(i) / 12.0) * std::f64::consts::TAU;
                let tick_center = center + Vec2::new(angle.cos(), angle.sin()) * tick_orbit;
                scene.fill_path(&Circle::new(tick_center, tick_radius).to_path(0.1));
            }

            let hand_width = (face_radius * 0.05).max(1.5);
            let hand_paint = with_opacity(state.hand_tint, own_alpha);
            scene.set_stroke(Stroke::new(hand_width));
            scene.set_paint(hand_paint);

            let hour_angle = -std::f64::consts::FRAC_PI_2
                + (f64::from(state.hour % 12) / 12.0) * std::f64::consts::TAU;
            let hour_tip =
                center + Vec2::new(hour_angle.cos(), hour_angle.sin()) * (face_radius * 0.5);
            scene.stroke_path(&Line::new(center, hour_tip).to_path(0.1));

            let minute_angle = -std::f64::consts::FRAC_PI_2
                + (f64::from(state.minute) / 60.0) * std::f64::consts::TAU;
            let minute_tip =
                center + Vec2::new(minute_angle.cos(), minute_angle.sin()) * (face_radius * 0.78);
            scene.stroke_path(&Line::new(center, minute_tip).to_path(0.1));

            // The real selector dot -- MD3's own real "which hand is
            // currently draggable" indicator, at the active hand's own
            // tip.
            let selector_tip = match state.mode {
                TimePickerDialMode::Hour => hour_tip,
                TimePickerDialMode::Minute => minute_tip,
            };
            scene.set_paint(hand_paint);
            scene.fill_path(&Circle::new(selector_tip, face_radius * 0.14).to_path(0.1));

            // A small real center hub, the same real anatomy a
            // physical analog clock face has.
            scene.fill_path(&Circle::new(center, face_radius * 0.03).to_path(0.1));
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
        let bounds = geometry.rounded_rect_fill(id, w, h, radius);

        // M25 Phase 2 (§5, §6): a real, previously-missing compounding
        // -- both the hover overlay and each ripple (below) multiplied
        // only by their own real, independent opacity (`hover_
        // opacity`/`ripple.opacity`) before this, never by `node.
        // paint.opacity.current` too, so a fading node's own ripple/
        // hover state layer would stay fully visible while everything
        // else around it faded.
        scene.set_paint(with_opacity(
            interaction.tint,
            interaction.hover_opacity.current * own_alpha,
        ));
        scene.fill_path(bounds);

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
                Some((ripple.opacity.current * own_alpha) as f32),
                None,
                None,
            );
            scene.set_paint(interaction.tint);
            scene.fill_path(bounds);
            scene.pop_layer();
        }
    }

    // M8 Phase 2 (§11.7): a `VirtualList`'s own materialized children
    // scroll and clip for real -- every other `NodeKind` recurses
    // exactly as before this phase (no other kind introduces a real
    // visual clip today, confirmed via direct read before this change,
    // so narrowing `visible` for any of them would wrongly cull
    // legitimately-overflowing content nothing here actually hides).
    if matches!(
        node.kind,
        NodeKind::VirtualList(_) | NodeKind::Carousel(_) | NodeKind::ScrollView(_)
    ) || node.paint.clip_children
    {
        // M30 Phase 9 Step 5 (§5, §7, §11.7): the real MD3 "clip items
        // to the strip, so one scrolled off does not spill out" anatomy
        // (pyCopper's own real `CLIPS_CHILDREN = True`) -- the identical
        // real clip mechanism `VirtualList` above already uses. No
        // scroll-offset translation needed here, unlike `VirtualList`:
        // `Tree::sync_carousel_layouts` already bakes every real item's
        // shifted position straight into its own `layout_style`, so
        // `composed` alone (each child's own real `Layout::location`)
        // is already correct -- the same real design choice that keeps
        // hit-testing and paint from ever disagreeing (`sync_carousel_
        // layouts`'s own doc comment).
        //
        // M32 Phase 3 (§5, §7, §11.7/§11.8): `Carousel` always takes
        // this branch (its own real MD3 anatomy, not an opt-in); any
        // other `NodeKind` takes it only when `PaintProperties.clip_
        // children` is genuinely set -- the real, general form of this
        // same clip, closing "no `NodeKind` besides `VirtualList` clips
        // today." No scroll-offset translation for the general case
        // either, the identical real v1 limit `clip_children`'s own doc
        // comment states: clipping only, not a new scroll mechanism.
        //
        // M36 Phase 1 (§5, §7, §11.7): `ScrollView` also always takes
        // this branch (its own real anatomy always clips, matching
        // `Carousel`, not an opt-in) -- and, like `Carousel`, needs no
        // scroll-offset translation here either: `Tree::sync_scroll_
        // view_layouts` already bakes its one real child's own current
        // scroll-shifted position into `layout_style` every frame, the
        // identical bug-avoiding "paint and hit-test read the same real
        // position, by construction" design this phase's own
        // investigation found `VirtualList` did *not* actually have.
        //
        // M37 (§5, §7, §11.7): `VirtualList` now joins this same
        // branch too, closing that real gap directly -- `Tree::sync_
        // virtual_list_layouts` bakes every real materialized item's
        // own current scroll-adjusted position into `layout_style`
        // every frame, the identical fix, so the separate paint-time-
        // only `Affine::translate` this branch used to need for
        // `VirtualList` alone is gone: `composed` alone is now already
        // correct for it too, exactly like `Carousel`/`ScrollView`.
        let clip_radius = node.paint.corner_radius.current;
        let clip = geometry.rounded_rect_fill(id, w, h, clip_radius);
        scene.push_layer(Some(clip), None, None, None, None);

        let narrowed = visible.intersect(bounds);
        for &child in &node.children {
            paint_node(
                tree, child, composed, narrowed, scene, resources, text, geometry,
            );
        }

        scene.pop_layer();

        // M38 Phase 6 (§5, §7, §11.7): a real `ScrollView`'s own real
        // scrollbar thumb -- painted here, *after* every real child
        // (mirrors pyCopper's own real `paint_foreground`, which runs
        // after children for exactly this reason: the thumb sits over
        // the scrolled content, not under it). `scene`'s own ambient
        // transform is whatever the last painted child left it at, not
        // necessarily `composed` any more -- reset it explicitly first,
        // the same real discipline every other paint call in this
        // function already follows.
        if let NodeKind::ScrollView(state) = &node.kind {
            paint_scroll_view_thumb(tree, id, state, w, h, composed, scene);
        }
        // M47 (§5, §7, §11.7): the identical real "paint after every
        // child" scrollbar thumb, for `VirtualList` -- closes the one
        // real gap M38 Phase 6 left open (named in M37's own trailer
        // note): `VirtualList` has always scrolled correctly via wheel
        // input, it just never had a visual thumb.
        if let NodeKind::VirtualList(state) = &node.kind {
            paint_virtual_list_thumb(state, w, h, composed, scene);
        }
    } else {
        for &child in &node.children {
            paint_node(
                tree, child, composed, visible, scene, resources, text, geometry,
            );
        }
    }
    if layered {
        scene.pop_layer();
    }
}

/// M38 Phase 6 (§5, §7, §11.7): a real `ScrollView`'s own real
/// scrollbar thumb -- paints only when there is genuinely something to
/// scroll (`content_extent > viewport_extent`, the identical real
/// `self.scrollable` gate pyCopper's own `paint_foreground` already
/// uses), reading `ScrollViewState::thumb_geometry`'s own real,
/// shared geometry (the identical values `Tree::grabs_scroll_view_
/// thumb`/`update_scroll_view_thumb_drag` already compute, so paint
/// and hit-testing/dragging can never drift). **Real, honest v1 scope
/// choice, stated directly:** a fixed, literal color (real MD3
/// baseline `outline_variant`, `0xCAC4D0`) at pyCopper's own real
/// `BAR_OPACITY` (0.55) -- `engine-render` has no `engine-md3`
/// dependency to resolve a real live theme token from (§4), the
/// identical "real but not yet theme-aware" scope `TextField`'s own
/// hardcoded caret color already established. Uncached (no
/// `GeometryCache` entry): the thumb's own position changes on every
/// real scroll tick, so a per-frame cache would rarely hit anyway,
/// and it's a genuinely small shape (`SCROLLBAR_THICKNESS` = 4px
/// wide) -- not worth the bookkeeping this catalog's own established
/// "cache only where it measurably helps" discipline (M34 Phase 1's
/// own real benchmark) already requires justifying.
fn paint_scroll_view_thumb(
    tree: &Tree,
    view: NodeId,
    state: &ScrollViewState,
    viewport_w: f64,
    viewport_h: f64,
    composed: Affine,
    scene: &mut Scene,
) {
    let Some(node) = tree.get(view) else {
        return;
    };
    let Some(&child) = node.children.first() else {
        return;
    };
    let child_layout = tree.layout(child);
    let (viewport_extent, content_extent) = if state.horizontal {
        (viewport_w, f64::from(child_layout.size.width))
    } else {
        (viewport_h, f64::from(child_layout.size.height))
    };
    if content_extent <= viewport_extent {
        return;
    }
    let (track, thumb, along) = state.thumb_geometry(viewport_extent, content_extent);
    if track <= 0.0 {
        return;
    }

    let thickness = state.scrollbar_width;
    let (x, y, w, h) = if state.horizontal {
        (
            along,
            viewport_h - thickness - SCROLLBAR_MARGIN,
            thumb,
            thickness,
        )
    } else {
        (
            viewport_w - thickness - SCROLLBAR_MARGIN,
            along,
            thickness,
            thumb,
        )
    };
    let color = state.scrollbar_fill.unwrap_or(DEFAULT_SCROLLBAR_FILL);
    fill_scrollbar_thumb(x, y, w, h, color, composed, scene);
}

/// M47 (§5, §7, §11.7): a real `VirtualList`'s own real scrollbar
/// thumb -- the identical real gate/geometry/paint technique `paint_
/// scroll_view_thumb` (M38 Phase 6) already established, narrowed to
/// `VirtualList`'s own vertical-only axis. Unlike `ScrollView`,
/// there's no single real child to measure for content extent --
/// `VirtualListState::total_extent()` (already the same real primitive
/// `Tree::scroll_virtual_list_by`'s own clamping and `Tree::grabs_
/// virtual_list_thumb`'s own hit-test use) is the real source of truth
/// here too, so paint and hit-testing/dragging can never disagree about
/// it.
fn paint_virtual_list_thumb(
    state: &VirtualListState,
    viewport_w: f64,
    viewport_h: f64,
    composed: Affine,
    scene: &mut Scene,
) {
    if state.total_extent() <= viewport_h {
        return;
    }
    let (track, thumb, along) = state.thumb_geometry(viewport_h);
    if track <= 0.0 {
        return;
    }
    let (x, y, w, h) = (
        viewport_w - SCROLLBAR_THICKNESS - SCROLLBAR_MARGIN,
        along,
        SCROLLBAR_THICKNESS,
        thumb,
    );
    fill_scrollbar_thumb(x, y, w, h, DEFAULT_SCROLLBAR_FILL, composed, scene);
}

/// M47 (§5, §7, §11.7): the real geometry-to-pixels fill both `paint_
/// scroll_view_thumb`/`paint_virtual_list_thumb` need -- factored out
/// once a second real caller needed the identical `RoundedRect` fill
/// at the identical color/opacity/radius, the same "two real call
/// sites justify factoring out" precedent this codebase already uses
/// throughout.
/// M95: the thumb color when a scroll view sets no `scrollbar_fill` --
/// MD3's `outline_variant` at 55%.
const DEFAULT_SCROLLBAR_FILL: Color = Color::from_rgba8(0xCA, 0xC4, 0xD0, 0x8C);

fn fill_scrollbar_thumb(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    color: Color,
    composed: Affine,
    scene: &mut Scene,
) {
    scene.set_transform(composed);
    scene.set_paint(color);
    scene.fill_path(&RoundedRect::new(x, y, x + w, y + h, SCROLLBAR_THUMB_RADIUS).to_path(0.1));
}

/// M38 Phase 6 (§5, §7, §11.7): pure-paint scrollbar tokens -- ported
/// directly from pyCopper's own real `BAR_RADIUS` (`widgets/scroll.py`;
/// its `BAR_OPACITY` of 0.55 now lives in `DEFAULT_SCROLLBAR_FILL`'s
/// alpha, M95). Live here, not `engine-core`, since neither
/// `Tree::grabs_scroll_view_thumb` nor `update_scroll_view_thumb_drag`
/// needs a fill radius or an opacity to do real hit-testing/dragging --
/// the identical real "geometry constants both crates need live in
/// `engine-core`, pure-paint ones stay in `engine-render`" split
/// `SCROLLBAR_THICKNESS`/`SCROLLBAR_MARGIN`'s own doc comment already
/// states for the reverse case.
const SCROLLBAR_THUMB_RADIUS: f64 = 2.0;

/// Thin wrapper around `vello_hybrid::Renderer` -- it needs a mutable
/// `Resources` alongside it for every render call, which is easy to get
/// out of sync by hand; bundling them here means callers only ever see
/// one object.
pub struct FrameRenderer {
    renderer: Renderer,
    resources: Resources,
    // M22 Phase 1 (§5): every real `Image` node's own GPU texture,
    // plus the live `TextureBindings` `render` hands to `vello_hybrid`
    // -- empty (byte-for-byte this struct's pre-M22 behavior) unless a
    // caller's own tree has real `Image` nodes and calls
    // `sync_image_textures`.
    images: image_cache::ImageTextureCache,
}

impl FrameRenderer {
    pub fn new(device: &wgpu::Device, config: &RenderTargetConfig) -> Self {
        let (renderer, resources) = Renderer::new(device, config);
        Self {
            renderer,
            resources,
            images: image_cache::ImageTextureCache::new(),
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
                self.images.bindings(),
            )
            .expect("vello_hybrid render failed");
    }

    /// M22 Phase 1 (§5): ensures every real `Image` node in `tree` has
    /// a real, uploaded GPU texture bound before the next `render`
    /// call -- see `ImageTextureCache::sync`'s own doc comment for the
    /// full "why". A caller whose tree has no `Image` nodes never
    /// needs to call this at all; `render`'s own `TextureBindings`
    /// then stays empty, exactly this crate's pre-M22 behavior.
    pub fn sync_image_textures(&mut self, tree: &Tree, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.images.sync(tree, device, queue);
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
