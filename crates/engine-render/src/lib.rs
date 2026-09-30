//! Vello Scene building and GPU rendering.
//!
//! §14 build-order step 1: one static rounded rect through the renderer
//! (`vello_hybrid` then; 0.4.0 M2: upstream `vello_gpu` at a pinned commit).
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
//! into the renderer's low-level glyph API).
//!
//! Depends on `engine-core` for `Tree`/`NodeId`/`NodeKind`/
//! `PaintProperties` and, for windowing, on nothing at all -- this crate
//! never touches `winit`. Window/surface creation is the caller's job
//! (an example, or later `engine-py`); everything here is parameterized
//! over a `wgpu::Device`/`Queue`/`TextureView` the caller already has,
//! matching §4's crate-boundary rule.

mod damage;
mod fonts;
mod geometry_cache;
mod image_cache;
mod persistent_target;
mod text;
mod walk;
mod window_renderer;

use engine_core::{
    ContentFit, DrawCommand, NodeId, NodeKind, SCROLLBAR_MARGIN, SCROLLBAR_THICKNESS,
    ScrollViewState, Tree, VirtualListState,
};
use peniko::Color;
use peniko::kurbo::{Affine, BezPath, Circle, Rect, RoundedRect, Shape, Stroke};
use vello_gpu::{RenderSize, RenderTargetConfig, Renderer, Resources, Scene};

pub use damage::{Damage, DamageTracker, MAX_RECTS};
pub use fonts::{NoFontFacesFound, register_font};
pub use geometry_cache::GeometryCache;
pub use image_cache::MAX_IMAGE_DIMENSION;
pub use persistent_target::PersistentTarget;
pub use text::{FontSpec, MONOSPACE_FONT_FAMILY, TextPlacement, TextRenderer};
pub use window_renderer::WindowRenderer;

/// MD3 seed-adjacent purple (#6750A4) -- an arbitrary but deliberate
/// starting color, not the renderer's own default, so a wrong pixel in a
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

/// §14 build-order step 8: standalone spike proving the renderer's
/// `Scene::fill_blurred_rounded_rect` (then `vello_hybrid` 0.2.0; 0.4.0:
/// `vello_gpu`, and `tests/shadow_spike.rs` still checks it) produces a real
/// Gaussian-blurred shadow, not just that the call compiles -- the
/// exact risk named in §7.2/§15 ("early-stage per Vello's own release
/// notes, no API stability guarantee yet, uneven parity across the
/// `vello`/`vello_cpu`/`vello_hybrid` variants"). One rect, no `Tree`,
/// no layout.
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

/// Walks `tree` from `root` (which must already have a computed layout --
/// call `Tree::compute_layout` first) and paints every `NodeKind::Rect`/
/// `NodeKind::Text` at its absolute on-screen position:
/// `taffy::Layout::location` is parent-relative, so this accumulates each
/// ancestor's offset on the way down rather than trusting a child's
/// location alone. A `Container` (a window's root) paints like a `Rect`
/// (0.5.0 M5); a `VirtualList` or `ScrollView` paints nothing itself but
/// still recurses into its children.
///
/// `resources`/`text` are threaded through for `NodeKind::Text` nodes:
/// glyph atlasing (`resources`) and font/shaping state (`text`) both
/// need to persist across frames, so they're the caller's, not built
/// fresh per call -- see `TextRenderer`'s own doc comment for why.
/// `geometry` (M34 Phase 1, §5, §8) is the identical kind of caller-
/// owned, cross-frame cache, for a `Rect`'s own tessellated
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
    build_scene(tree, root, width, height, None, resources, text, geometry)
}

/// 0.4.0 M5: `build_tree_scene` for a partial redraw -- only what paints
/// inside `rects` (window pixels, non-overlapping, from `Damage::Rects`):
/// the scene is clipped to them, and a node is drawn only where what it
/// paints -- shadows and overflow included -- reaches one of them. Rendered with `TargetInit::Clear(ClearSettings::Rects)` over
/// the kept last frame, it matches a full redraw inside the rects.
#[allow(clippy::too_many_arguments)]
pub fn build_tree_scene_in(
    tree: &Tree,
    root: NodeId,
    width: u16,
    height: u16,
    rects: &[Rect],
    resources: &mut Resources,
    text: &mut TextRenderer,
    geometry: &mut GeometryCache,
) -> Scene {
    build_scene(
        tree,
        root,
        width,
        height,
        Some(rects),
        resources,
        text,
        geometry,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_scene(
    tree: &Tree,
    root: NodeId,
    width: u16,
    height: u16,
    rects: Option<&[Rect]>,
    resources: &mut Resources,
    text: &mut TextRenderer,
    geometry: &mut GeometryCache,
) -> Scene {
    let mut scene = Scene::new(width, height);
    scene.set_transform(Affine::IDENTITY);
    // M8 Phase 1 (§11.8): the real, canvas-space "currently visible"
    // rect -- the whole viewport at the top of the walk -- which `walk`
    // narrows under a clipping node on the way into its children. A partial redraw keeps the same
    // `visible`, so it culls exactly as a full redraw does, and tests each
    // node against the damage `rects` besides.
    let visible = Rect::new(0.0, 0.0, f64::from(width), f64::from(height));
    if let Some(rects) = rects {
        let mut clip = BezPath::new();
        for rect in rects {
            // Rounded outward to whole pixels, as `render_into` clears.
            clip.extend(rect.expand().path_elements(0.1));
        }
        scene.push_clip_layer(&clip);
    }
    let mut painter = Painter {
        tree,
        rects,
        scene: &mut scene,
        resources,
        text,
        geometry,
        open: Vec::new(),
    };
    walk::walk(tree, root, visible, &mut painter);
    if rects.is_some() {
        scene.pop_layer();
    }
    scene
}

/// CSS Backgrounds and Borders Module Level 3's own real conversion,
/// verified directly (`PLAN.md`): "a Gaussian blur with a standard
/// deviation equal to half the blur radius."
fn blur_to_std_dev(blur_px: f32) -> f32 {
    blur_px / 2.0
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
/// The transform a node paints under: its parent's, then its layout
/// position, then its own transform parts. 0.4.0 M4: shared by the paint
/// walk and `DamageTracker`, so the two can't disagree.
pub(crate) fn composed_transform(
    parent: Affine,
    position: (f64, f64),
    node: &engine_core::Node,
    w: f64,
    h: f64,
) -> Affine {
    parent * Affine::translate(position) * node.paint.local_transform(w, h)
}

/// The window-space bounding box of `rect` (node-local) under
/// `composed` -- all four corners, so it stays right under rotation.
pub(crate) fn transformed_bounds(composed: Affine, rect: Rect) -> Rect {
    composed.transform_rect_bbox(rect)
}

/// Whether `node` clips its children to its rounded box: scroll views
/// and virtual lists always, anything with `clip_children` set.
pub(crate) fn clips_children(node: &engine_core::Node) -> bool {
    matches!(
        node.kind,
        NodeKind::VirtualList(_) | NodeKind::ScrollView(_)
    ) || node.paint.clip_children
}

/// The paint walk (`walk::Visitor`): each node's own paint, its group
/// opacity layer, and its children's clip layer, which `leave` closes.
struct Painter<'a> {
    tree: &'a Tree,
    /// A partial redraw's damage rects; `None` paints everything.
    rects: Option<&'a [Rect]>,
    scene: &'a mut Scene,
    resources: &'a mut Resources,
    text: &'a mut TextRenderer,
    geometry: &'a mut GeometryCache,
    /// Per node entered and not yet left: whether it opened an opacity
    /// layer, whether it opened a clip layer, and whether it drew itself.
    open: Vec<(bool, bool, bool)>,
}

impl<'t> walk::Visitor<'t> for Painter<'_> {
    fn enter(&mut self, v: &walk::Visit<'t>) -> bool {
        let node = v.node;
        // 0.4.0 M5: in a partial redraw a node draws only if what it paints
        // (`damage::painted_rect`, the extent the damage walk records --
        // shadows and overflow included, not just its box) reaches a damage
        // rect. A node that clips its children confines its whole subtree to
        // that extent, so missing skips the subtree; any other node's children
        // may overflow it, and still decide for themselves.
        let draw_self = match self.rects {
            None => true,
            Some(rects) => {
                let painted = damage::painted_rect(
                    self.text, v.id, node, v.composed, v.w, v.h, v.bounds, v.visible,
                );
                let hit = rects.iter().any(|r| painted.overlaps(*r));
                if !hit && clips_children(node) {
                    return false;
                }
                hit
            }
        };

        // M95: group opacity -- the node and its whole subtree composite as
        // one layer at `opacity`, so a fading container fades its children
        // too. The node's own paints are then fully opaque (`own_alpha` in
        // `draw_own`), their colors' own alpha applying through
        // `with_opacity`.
        let opacity = node.paint.opacity.current;
        let layered = opacity < 1.0;
        if layered {
            self.scene
                .push_layer(None, None, Some(opacity as f32), None, None);
        }
        if draw_self {
            draw_own(
                self.tree,
                v.id,
                node,
                v.w,
                v.h,
                v.composed,
                self.scene,
                self.resources,
                self.text,
                self.geometry,
            );
        }

        // M8 Phase 2 (§11.7): a node that clips its children paints them
        // inside a clip layer of its rounded box (the walk narrows their
        // visible rect to match).
        // M32 Phase 3 (§5, §7, §11.7/§11.8): any `NodeKind` takes this
        // branch when `PaintProperties.clip_children` is genuinely set.
        // No scroll-offset translation for the general case, the
        // identical real v1 limit `clip_children`'s own doc comment
        // states: clipping only, not a new scroll mechanism.
        //
        // M36 Phase 1 (§5, §7, §11.7): `ScrollView` always takes this
        // branch (its own real anatomy always clips, not an opt-in) --
        // and needs no scroll-offset translation here either: `Tree::sync_scroll_
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
        // correct for it too, exactly like `ScrollView`.
        let clipped = clips_children(node);
        if clipped {
            let clip_radius = node.paint.corner_radius.current;
            let clip = self.geometry.rounded_rect_fill(v.id, v.w, v.h, clip_radius);
            self.scene.push_layer(Some(clip), None, None, None, None);
        }
        self.open.push((layered, clipped, draw_self));
        true
    }

    fn leave(&mut self, v: &walk::Visit<'t>) {
        let (layered, clipped, drew) = self.open.pop().expect("every node left was entered");
        if clipped {
            self.scene.pop_layer();
            // M38 Phase 6 (§5, §7, §11.7): a real `ScrollView`'s own real
            // scrollbar thumb -- painted here, *after* every real child
            // (mirrors pyCopper's own real `paint_foreground`, which runs
            // after children for exactly this reason: the thumb sits over
            // the scrolled content, not under it). `scene`'s own ambient
            // transform is whatever the last painted child left it at, not
            // necessarily `composed` any more -- reset it explicitly first,
            // the same real discipline every other paint call in this
            // function already follows.
            if let NodeKind::ScrollView(state) = &v.node.kind
                && drew
            {
                paint_scroll_view_thumb(self.tree, v.id, state, v.w, v.h, v.composed, self.scene);
            }
            // M47 (§5, §7, §11.7): the identical real "paint after every
            // child" scrollbar thumb, for `VirtualList` -- closes the one
            // real gap M38 Phase 6 left open (named in M37's own trailer
            // note): `VirtualList` has always scrolled correctly via wheel
            // input, it just never had a visual thumb.
            if let NodeKind::VirtualList(state) = &v.node.kind
                && drew
            {
                paint_virtual_list_thumb(state, v.w, v.h, v.composed, self.scene);
            }
        }
        if layered {
            self.scene.pop_layer();
        }
    }
}

/// A node's own paint -- shadows, then what its kind draws -- in its own
/// coordinates under `composed`.
#[allow(clippy::too_many_arguments)]
fn draw_own(
    tree: &Tree,
    id: NodeId,
    node: &engine_core::Node,
    w: f64,
    h: f64,
    composed: Affine,
    scene: &mut Scene,
    resources: &mut Resources,
    text: &mut TextRenderer,
    geometry: &mut GeometryCache,
) {
    let own_alpha = 1.0;
    scene.set_transform(composed);

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
        // 0.5.0 M5: a `Container` -- a window's root -- takes a box's paint
        // properties too; it painted nothing before, so a root's fill never
        // showed.
        NodeKind::Rect | NodeKind::Container => {
            let color = with_opacity(node.paint.background.current, own_alpha);
            scene.set_paint(color);
            // M30 Phase 1 Step 4 (§5, §7): `corner_radii_override`
            // (`[top_left, top_right, bottom_right, bottom_left]`)
            // wins when set (a segmented group's first/last segment,
            // rounded only on its outer edge). `None` falls
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
            // M30 Phase 1 (§5, §7): a real stroked border (the Python
            // API's `stroke_color`/`stroke_width`), universal
            // `PaintProperties` like `background`/`corner_radius`. Inset by half the stroke
            // width so the border paints entirely *inside* this node's
            // own bounds (kurbo strokes are centered on the path by
            // default) -- a border never grows past the node's own
            // taffy-computed box the way a naive un-inset stroke would.
            // Skipped entirely at `border_width <= 0.0`.
            let border_width = node.paint.border_width.current;
            if border_width > 0.0 {
                let inset = border_width / 2.0;
                // M38 Phase 4 (§5, §7): the border path now matches
                // whichever real fill geometry this node actually used
                // just above, closing a real, previously-dormant gap --
                // `RectPathParams::PerCornerBorder`'s own doc comment
                // has the full story (`geometry_cache.rs`).
                let border_path: std::borrow::Cow<'_, BezPath> = if let Some(radii) = node
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
        NodeKind::Text(state) => {
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
        // `draw_field` paints its content/caret/selection on top in
        // `state.text_tint` (below).
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
            // same real cache `Rect`/`Terminal` already use -- also
            // reused directly below for this same node's
            // own real clip layer, since both need the identical path.
            scene.fill_path(geometry.rounded_rect_fill(id, w, h, radius));

            // M20 Phase 2 (§7.1, §7.3): the real resolved color comes
            // from `state.text_tint` -- plain dark by default
            // (byte-for-byte the old hardcoded literal), or whatever
            // the text input's `fill` sets.
            let text_color = with_opacity(state.text_tint.current, own_alpha);
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
        // fixed color (`engine-render` has no design system to resolve
        // one from, §4).
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
        // A `VirtualList` container paints nothing itself -- it exists
        // purely to give `taffy` something to lay its (windowed) children
        // out against; the recursive walk
        // below already only ever sees `VirtualListState::materialized`'s
        // small real subset, never `item_count`, with zero changes
        // needed here (§14 step 15, §11.7).
        // M36 Phase 1 (§5, §7, §11.7): `ScrollView` paints nothing of
        // its own, like `VirtualList` -- its one real
        // child's own absolute position is already baked into `layout_
        // style` by `Tree::sync_scroll_view_layouts`, so the ordinary
        // recursive walk below (composed transform only, no extra
        // paint-time offset) already paints it in the right place; the
        // unconditional clip below is this kind's only other real
        // paint-time behavior.
        NodeKind::VirtualList(_) | NodeKind::ScrollView(_) => {}
        // M5 Phase 3 (§11.10, §11.11): replays `state.commands`, already
        // resolved ahead of time by `engine-py`'s `draw` callback
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
        // M22 Phase 1 (§5): an image is an externally owned GPU texture
        // (`image_cache` has why -- the renderer doesn't take CPU pixel
        // data), bound under `id`'s `TextureId` in the `TextureBindings`
        // `FrameRenderer::render`/`render_into` pass on;
        // `sync_image_textures`, run before rendering each frame,
        // guarantees the binding. 0.4.0 M2:
        // `vello_gpu` draws it as an `ImageSource::ExternalTexture` paint
        // over a filled rect -- `source_region` is the part of the image
        // shown (all of it, or `cover`'s crop), and `transform` maps its
        // texels onto the node box. The paint has no opacity of its own,
        // so an opacity layer wraps it, skipped when fully transparent.
        NodeKind::Image(state) => {
            let img_width = state.image.width;
            let img_height = state.image.height;
            let node_opacity = own_alpha;
            // An image too large to upload has no texture to draw.
            let fits = img_width <= image_cache::MAX_IMAGE_DIMENSION
                && img_height <= image_cache::MAX_IMAGE_DIMENSION;
            if img_width > 0 && img_height > 0 && fits && node_opacity > 0.0 {
                let (source_region, transform) =
                    image_sample_rect(w, h, img_width, img_height, state.content_fit);
                // The texture is an image paint: `transform` maps the
                // source region's texels to the node box, as the paint
                // transform, and the fill covers the region's image there.
                let region = peniko::kurbo::Rect::new(
                    0.0,
                    0.0,
                    f64::from(source_region.x1 - source_region.x0),
                    f64::from(source_region.y1 - source_region.y0),
                );
                scene.push_layer(None, None, Some(node_opacity as f32), None, None);
                scene.set_paint(vello_common::paint::Image {
                    image: vello_common::paint::ImageSource::external_texture(
                        image_cache::texture_id_for(id),
                        source_region,
                        true,
                    ),
                    sampler: peniko::ImageSampler {
                        quality: peniko::ImageQuality::Medium,
                        ..Default::default()
                    },
                });
                scene.set_paint_transform(transform);
                scene.fill_rect(&transform.transform_rect_bbox(region));
                scene.reset_paint_transform();
                scene.pop_layer();
            }
        }
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
    }
}

/// M38 Phase 6 (§5, §7, §11.7): a real `ScrollView`'s own real
/// scrollbar thumb -- paints only when there is genuinely something to
/// scroll (`content_extent > viewport_extent`, the identical real
/// `self.scrollable` gate pyCopper's own `paint_foreground` already
/// uses), reading `ScrollViewState::thumb_geometry`'s own real,
/// shared geometry (the identical values `Tree::grabs_scroll_view_
/// thumb`/`update_scroll_view_thumb_drag` already compute, so paint
/// and hit-testing/dragging can never drift). Painted in the view's
/// `scrollbar_fill`, or `DEFAULT_SCROLLBAR_FILL` when it sets none.
/// Uncached (no
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

/// Thin wrapper around `vello_gpu::Renderer` -- it needs a mutable
/// `Resources` alongside it for every render call, which is easy to get
/// out of sync by hand; bundling them here means callers only ever see
/// one object.
pub struct FrameRenderer {
    renderer: Renderer,
    resources: Resources,
    // M22 Phase 1 (§5): every real `Image` node's own GPU texture,
    // plus the live `TextureBindings` `render` hands to `vello_gpu`
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
        self.render_into(scene, device, queue, encoder, render_size, target, None);
    }

    /// 0.4.0 M5: `render`, clearing only `rects` (window pixels) and
    /// keeping the rest of `target` -- for a scene from
    /// `build_tree_scene_in` with the same rects. `None` clears it all, as
    /// `render` does.
    #[allow(clippy::too_many_arguments)]
    pub fn render_into(
        &mut self,
        scene: &Scene,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        render_size: &RenderSize,
        target: &wgpu::TextureView,
        rects: Option<&[Rect]>,
    ) {
        let clear_rects: Vec<vello_common::geometry::RectU16> = rects
            .unwrap_or_default()
            .iter()
            .map(|r| {
                // Whole pixels, rounded outward as the scene's clip is, so
                // a fractional edge's partly covered pixel is cleared too.
                let r = r.expand();
                let at = |v: f64| v.clamp(0.0, f64::from(u16::MAX)) as u16;
                vello_common::geometry::RectU16::new(at(r.x0), at(r.y0), at(r.x1), at(r.y1))
            })
            .collect();
        let clear = match rects {
            // Transparent, as the full clear is: the scene's own background
            // paints over it.
            Some(_) => vello_gpu::ClearSettings::Rects {
                color: peniko::color::AlphaColor::TRANSPARENT,
                rects: &clear_rects,
            },
            None => vello_gpu::ClearSettings::default(),
        };
        self.renderer
            .render(
                scene,
                &mut self.resources,
                device,
                queue,
                encoder,
                render_size,
                target,
                None,
                self.images.bindings(),
                vello_gpu::TargetInit::Clear(clear),
            )
            .expect("vello_gpu render failed");
    }

    /// 0.4.1 M8: renders `scene` over `target`'s existing pixels
    /// (`TargetInit::SrcOver`), clearing nothing -- for drawing on top of a
    /// finished frame, as the redrawn-areas overlay does.
    #[allow(clippy::too_many_arguments)]
    pub fn render_over(
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
                None,
                self.images.bindings(),
                vello_gpu::TargetInit::SrcOver,
            )
            .expect("vello_gpu render failed");
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
    /// pattern the renderer's own `render_to_file` example uses, adapted
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
                    apply_limit_buckets: false,
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
                    width,
                    height,
                },
            );
            let render_size = RenderSize { width, height };

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

            let data = slice.get_mapped_range().expect("the readback buffer maps");
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
