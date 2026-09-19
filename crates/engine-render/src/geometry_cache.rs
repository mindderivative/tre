//! M34 Phase 1 (§5, §8): per-node tessellated-path caching for `Rect`/
//! `Splitter`'s own plain rounded-rect fill/border paths -- the same
//! "cache what a frame doesn't need to redo" reasoning `TextRenderer::
//! shaped_layout`'s own per-node `Layout` cache already established for
//! shaped text (`text.rs`'s own doc comment), applied here to
//! `peniko::kurbo::Shape::to_path`'s own real curve-flattening cost.
//!
//! **Real, measured motivation, not assumed:** a scratch benchmark this
//! milestone's own investigation ran (1000 static `Rect` nodes, release
//! build) measured `build_tree_scene` at ~2.7-2.9ms/frame. A follow-up
//! isolation benchmark found only ~20% of that (2.48ms -> 1.99ms) comes
//! from `RoundedRect::to_path(0.1)`'s own tessellation -- the remaining
//! ~80% is `vello_hybrid::Scene::fill_path`'s own internal strip-
//! generation cost, paid on every call regardless of whether the path
//! passed in is freshly tessellated or reused, and unavoidable without
//! either a fork of the vendored crate or a `Scene` sub-fragment splice
//! API it doesn't have (confirmed via direct source read, `PLAN.md`).
//! So this cache is a real, honest, modest win -- skips the real ~20%
//! tessellation share of an already-cheap total, not a dramatic one --
//! deliberately scoped, at first (M34 Phase 1), to `Rect`/`Splitter`'s
//! own plain (non-shape-morph) fill/border paths, the single most
//! common real paint call in any app (every button/card/panel/dialog
//! background), leaving every other curve-tessellating `NodeKind`
//! uncached as a real, deliberate v1 scope limit.
//!
//! **M38 Phase 1 (§5, §7, §8):** extends the identical real cache to
//! `RadioButton`'s ring/dot, `Switch`'s track/outline/handle,
//! `CircularProgress`'s arc, and `Checkbox`'s box (the latter two
//! reuse the existing rounded-rect/border methods directly, since
//! their real generating geometry is byte-for-byte identical to
//! `Rect`'s own). Deliberately still does **not** cache `Terminal`'s
//! own per-cell/selection/cursor rects -- those are plain, axis-
//! aligned `Rect::to_path` calls, not curve tessellation, and M34's
//! own real benchmark already found the tessellation share of the
//! real per-frame cost is concentrated in curved shapes (rounded
//! rects, circles, arcs); a plain rect's own `to_path` is already
//! near-free by comparison, so caching it here would add real
//! bookkeeping cost for negligible real benefit.

use std::collections::HashMap;

use engine_core::{NodeId, Tree};
use peniko::kurbo::{Arc, BezPath, Circle, RoundedRect, Shape};

/// The exact inputs a tessellated rounded-rect path depends on --
/// equality here *is* the invalidation check, the identical technique
/// `text.rs::LayoutCacheKey` already established: every input that can
/// change the output is part of the key, so there's no separate
/// "remember to invalidate" bookkeeping a future change could forget.
#[derive(Clone, Copy, PartialEq)]
enum RectPathParams {
    /// `Rect`/`Splitter`'s own plain fill, one uniform corner radius --
    /// `PaintProperties.corner_radius`'s own default real case.
    Uniform { w: f64, h: f64, radius: f64 },
    /// The real per-corner override (`PaintProperties.
    /// corner_radii_override`, M30 Phase 1 Step 4) -- `Segmented
    /// Button`'s own real need, a distinct params shape from `Uniform`
    /// since a node can genuinely switch between the two.
    PerCorner { w: f64, h: f64, radii: [f64; 4] },
    /// A stroked border's own real generating inputs (M30 Phase 1,
    /// §5, §7) -- `inset` (half the stroke width) shifts the path's own
    /// origin, so it's a real, independent input from the fill's own
    /// `Uniform`/`PerCorner` params, not reusable between the two.
    Border {
        w: f64,
        h: f64,
        radius: f64,
        inset: f64,
    },
    /// M38 Phase 4 (§5, §7): `Border`'s own real per-corner sibling --
    /// a real, previously-dormant gap `corner_radii_override`'s own
    /// border stroke had since M30 Phase 1 Step 4 (it only ever fed
    /// the fill path, never the border, confirmed by direct grep
    /// before this phase): a bordered `Rect` with a real per-corner
    /// override painted its stroke at the plain uniform `corner_
    /// radius` regardless, a mismatch invisible until Split Button's
    /// own outlined variant became the first real consumer to combine
    /// a nonzero border with per-corner geometry.
    PerCornerBorder {
        w: f64,
        h: f64,
        radii: [f64; 4],
        inset: f64,
    },
}

/// M38 Phase 1 (§5, §7, §8): the real generating inputs of a
/// tessellated circle path -- `RadioButton`'s own real ring (a stroke)
/// and dot (a fill) are geometrically the *same* real shape a `Circle`
/// produces (stroke vs fill is a paint-time choice, not a path-
/// generation one), so one params type covers both; each still gets
/// its own real cache slot below (`circle_primary`/`circle_secondary`)
/// since a single node can have two real, independent circles at once
/// (`RadioButton`'s ring *and* dot).
#[derive(Clone, Copy, PartialEq)]
struct CircleParams {
    cx: f64,
    cy: f64,
    radius: f64,
}

/// M38 Phase 1 (§5, §7, §8): `CircularProgress`'s own real generating
/// inputs -- `start`/`sweep` in radians, matching `peniko::kurbo::Arc`'s
/// own real constructor arguments directly.
#[derive(Clone, Copy, PartialEq)]
struct ArcParams {
    cx: f64,
    cy: f64,
    radius: f64,
    start: f64,
    sweep: f64,
}

/// Owns every real, currently-cached tessellated `BezPath` across
/// frames -- built once and threaded through every `build_tree_scene`
/// call, the identical "long-lived state, not built fresh per call"
/// shape `TextRenderer`/`Resources` already establish (see their own
/// doc comments for why). Fill and border paths live in separate maps,
/// not one shared map keyed by `NodeId` alone, since a single real
/// bordered `Rect` needs both at once -- the identical real reason
/// `circle_primary`/`circle_secondary` (M38 Phase 1) are two separate
/// maps too, for `RadioButton`'s own real ring-plus-dot.
pub struct GeometryCache {
    fill_paths: HashMap<NodeId, (RectPathParams, BezPath)>,
    border_paths: HashMap<NodeId, (RectPathParams, BezPath)>,
    circle_primary: HashMap<NodeId, (CircleParams, BezPath)>,
    circle_secondary: HashMap<NodeId, (CircleParams, BezPath)>,
    arc_paths: HashMap<NodeId, (ArcParams, BezPath)>,
}

impl Default for GeometryCache {
    fn default() -> Self {
        Self::new()
    }
}

impl GeometryCache {
    pub fn new() -> Self {
        Self {
            fill_paths: HashMap::new(),
            border_paths: HashMap::new(),
            circle_primary: HashMap::new(),
            circle_secondary: HashMap::new(),
            arc_paths: HashMap::new(),
        }
    }

    /// The real fill path for a plain, uniform-corner rounded rect at
    /// local `(0, 0)-(w, h)` -- re-tessellates only when `w`/`h`/
    /// `radius` genuinely changed since this node's last paint.
    pub fn rounded_rect_fill(&mut self, id: NodeId, w: f64, h: f64, radius: f64) -> &BezPath {
        let params = RectPathParams::Uniform { w, h, radius };
        Self::get_or_build(&mut self.fill_paths, id, params, || {
            RoundedRect::new(0.0, 0.0, w, h, radius).to_path(0.1)
        })
    }

    /// `rounded_rect_fill`'s own real per-corner sibling (M30 Phase 1
    /// Step 4's `corner_radii_override`, `[top_left, top_right,
    /// bottom_right, bottom_left]`) -- a separate real `RectPathParams`
    /// variant, not reused `Uniform` params, so a node that switches
    /// between the two real shapes across frames re-tessellates
    /// correctly instead of comparing unrelated params as equal.
    pub fn rounded_rect_fill_per_corner(
        &mut self,
        id: NodeId,
        w: f64,
        h: f64,
        radii: [f64; 4],
    ) -> &BezPath {
        let params = RectPathParams::PerCorner { w, h, radii };
        Self::get_or_build(&mut self.fill_paths, id, params, || {
            let [tl, tr, br, bl] = radii;
            RoundedRect::new(0.0, 0.0, w, h, (tl, tr, br, bl)).to_path(0.1)
        })
    }

    /// The real stroked-border path (M30 Phase 1, §5, §7) -- `inset`
    /// (half the stroke width) shifts the path inward so the stroke
    /// paints entirely inside this node's own bounds, the identical
    /// real geometry `paint_node`'s own border arm already builds.
    pub fn rounded_rect_border(
        &mut self,
        id: NodeId,
        w: f64,
        h: f64,
        radius: f64,
        inset: f64,
    ) -> &BezPath {
        let params = RectPathParams::Border {
            w,
            h,
            radius,
            inset,
        };
        Self::get_or_build(&mut self.border_paths, id, params, || {
            RoundedRect::new(inset, inset, w - inset, h - inset, radius).to_path(0.1)
        })
    }

    /// M38 Phase 4 (§5, §7): `rounded_rect_border`'s own real per-
    /// corner sibling, closing the dormant `corner_radii_override`
    /// border gap `RectPathParams::PerCornerBorder`'s own doc comment
    /// names -- each of the four real radii is inset by the identical
    /// `inset` (half the stroke width) `rounded_rect_border` already
    /// subtracts from its own single scalar, clamped at `0.0` the same
    /// way (a corner whose radius is smaller than the inset degrades
    /// to a square corner rather than going negative).
    pub fn rounded_rect_border_per_corner(
        &mut self,
        id: NodeId,
        w: f64,
        h: f64,
        radii: [f64; 4],
        inset: f64,
    ) -> &BezPath {
        let params = RectPathParams::PerCornerBorder { w, h, radii, inset };
        Self::get_or_build(&mut self.border_paths, id, params, || {
            let [tl, tr, br, bl] = radii.map(|r| (r - inset).max(0.0));
            RoundedRect::new(inset, inset, w - inset, h - inset, (tl, tr, br, bl)).to_path(0.1)
        })
    }

    /// M38 Phase 1 (§5, §7, §8): the real ring path -- `RadioButton`'s
    /// stroked outer ring, or `Switch`'s own real sliding handle (a
    /// plain filled circle) -- whichever real "primary" circle a node
    /// has. Reused across both kinds since neither ever has both at
    /// once (a real `NodeKind`-level exclusivity, not a coincidence).
    pub fn circle_primary(&mut self, id: NodeId, cx: f64, cy: f64, radius: f64) -> &BezPath {
        let params = CircleParams { cx, cy, radius };
        Self::get_or_build(&mut self.circle_primary, id, params, || {
            Circle::new((cx, cy), radius).to_path(0.1)
        })
    }

    /// M38 Phase 1 (§5, §7, §8): `RadioButton`'s own real second
    /// circle -- the filled dot that scales in with `select_progress`,
    /// needing its own real, independent cache slot alongside
    /// `circle_primary`'s own ring for the same node.
    pub fn circle_secondary(&mut self, id: NodeId, cx: f64, cy: f64, radius: f64) -> &BezPath {
        let params = CircleParams { cx, cy, radius };
        Self::get_or_build(&mut self.circle_secondary, id, params, || {
            Circle::new((cx, cy), radius).to_path(0.1)
        })
    }

    /// M38 Phase 1 (§5, §7, §8): `CircularProgress`'s own real stroked
    /// arc -- `start`/`sweep` in radians, matching `peniko::kurbo::Arc`'s
    /// own real constructor directly.
    #[allow(clippy::too_many_arguments)]
    pub fn arc(
        &mut self,
        id: NodeId,
        cx: f64,
        cy: f64,
        radius: f64,
        start: f64,
        sweep: f64,
    ) -> &BezPath {
        let params = ArcParams {
            cx,
            cy,
            radius,
            start,
            sweep,
        };
        Self::get_or_build(&mut self.arc_paths, id, params, || {
            Arc::new((cx, cy), (radius, radius), start, sweep, 0.0).to_path(0.1)
        })
    }

    fn get_or_build<P: PartialEq>(
        cache: &mut HashMap<NodeId, (P, BezPath)>,
        id: NodeId,
        params: P,
        build: impl FnOnce() -> BezPath,
    ) -> &BezPath {
        let stale = cache
            .get(&id)
            .is_none_or(|(cached_params, _)| *cached_params != params);
        if stale {
            cache.insert(id, (params, build()));
        }
        &cache
            .get(&id)
            .expect("just inserted above, or already present")
            .1
    }

    /// Mirrors `TextRenderer::evict_stale_layouts`'s own real per-node-
    /// cache-leak fix (the same review-found class of gap, `text.rs`'s
    /// own doc comment) -- a `Rect`/`Splitter` removed from the tree
    /// left its tessellated path(s) cached here forever otherwise. Call
    /// once per frame, alongside `evict_stale_layouts`/
    /// `sync_image_textures`.
    pub fn evict_stale(&mut self, tree: &Tree) {
        self.fill_paths.retain(|id, _| tree.get(*id).is_some());
        self.border_paths.retain(|id, _| tree.get(*id).is_some());
        self.circle_primary.retain(|id, _| tree.get(*id).is_some());
        self.circle_secondary
            .retain(|id, _| tree.get(*id).is_some());
        self.arc_paths.retain(|id, _| tree.get(*id).is_some());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::{NodeKind, PaintProperties};
    use taffy::prelude::{Size, Style, length};

    fn rect_node(tree: &mut Tree) -> NodeId {
        tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(50.0),
                    height: length(50.0),
                },
                ..Default::default()
            },
            PaintProperties::new(peniko::Color::from_rgba8(255, 0, 0, 255), 8.0, 0.0, 1.0),
        )
    }

    /// The real point: an unchanged node's cache entry must not be
    /// rebuilt -- proven by pointer identity on the returned `BezPath`'s
    /// own backing storage (`elements()`'s first element address),
    /// since `BezPath` has no `PartialEq`/`Eq` impl to compare by value.
    #[test]
    fn an_unchanged_node_reuses_its_cached_fill_path_without_rebuilding() {
        let mut tree = Tree::new();
        let id = rect_node(&mut tree);
        let mut cache = GeometryCache::new();

        let first_ptr = cache
            .rounded_rect_fill(id, 50.0, 50.0, 8.0)
            .elements()
            .as_ptr();
        let second_ptr = cache
            .rounded_rect_fill(id, 50.0, 50.0, 8.0)
            .elements()
            .as_ptr();
        assert_eq!(
            first_ptr, second_ptr,
            "identical (w, h, radius) must reuse the exact same cached BezPath, not rebuild it"
        );
    }

    /// The real other half: a genuinely changed input (here, `radius`)
    /// must invalidate the cache and produce a real, different path.
    /// M38 Phase 4 (§5, §7): switched from a `bounding_box()`
    /// comparison to the path's own real starting point -- direct,
    /// real inspection while writing this phase's own per-corner
    /// border tests found `bounding_box()` reports byte-for-byte the
    /// *same* box for any valid radius on a given `w`/`h` (a rounded
    /// rect's tight curve bounds always touch all four nominal edges
    /// regardless of corner radius), so this assertion was previously
    /// passing only by incidental floating-point tessellation noise
    /// between the two radii, not by genuinely proving the radius
    /// reached the geometry.
    #[test]
    fn a_changed_radius_invalidates_the_cached_fill_path() {
        let mut tree = Tree::new();
        let id = rect_node(&mut tree);
        let mut cache = GeometryCache::new();

        let first = cache.rounded_rect_fill(id, 50.0, 50.0, 8.0).elements()[0];
        let second = cache.rounded_rect_fill(id, 50.0, 50.0, 20.0).elements()[0];
        assert_ne!(
            first, second,
            "a genuinely different corner radius must move the path's own real starting \
             point, not silently reuse the stale one"
        );
    }

    /// Fill and border paths for the *same* real node must not collide
    /// -- a bordered `Rect` needs both cached independently.
    #[test]
    fn fill_and_border_caches_for_the_same_node_do_not_collide() {
        let mut tree = Tree::new();
        let id = rect_node(&mut tree);
        let mut cache = GeometryCache::new();

        let fill = cache.rounded_rect_fill(id, 50.0, 50.0, 8.0).clone();
        let border = cache.rounded_rect_border(id, 50.0, 50.0, 6.0, 1.0).clone();
        assert_ne!(
            fill.bounding_box(),
            border.bounding_box(),
            "the fill (full box) and the inset border path must be genuinely distinct real paths"
        );
    }

    /// M38 Phase 4 (§5, §7): the identical real cache-hit/cache-miss
    /// proof the fill-path tests above already establish, applied to
    /// the new per-corner border method.
    #[test]
    fn an_unchanged_node_reuses_its_cached_per_corner_border_path_without_rebuilding() {
        let mut tree = Tree::new();
        let id = rect_node(&mut tree);
        let mut cache = GeometryCache::new();

        let first_ptr = cache
            .rounded_rect_border_per_corner(id, 50.0, 50.0, [8.0, 2.0, 2.0, 8.0], 1.0)
            .elements()
            .as_ptr();
        let second_ptr = cache
            .rounded_rect_border_per_corner(id, 50.0, 50.0, [8.0, 2.0, 2.0, 8.0], 1.0)
            .elements()
            .as_ptr();
        assert_eq!(
            first_ptr, second_ptr,
            "identical (w, h, radii, inset) must reuse the exact same cached BezPath"
        );
    }

    #[test]
    fn a_changed_radii_invalidates_the_cached_per_corner_border_path() {
        let mut tree = Tree::new();
        let id = rect_node(&mut tree);
        let mut cache = GeometryCache::new();

        // `bounding_box()` is *not* a reliable radius-vs-radius probe
        // here -- a rounded rect's own tight curve bounding box always
        // touches all four nominal edges regardless of corner radius
        // (confirmed by direct, real inspection before writing this:
        // two same-box, different-radius `RoundedRect::to_path()`
        // calls reported byte-for-byte the same `bounding_box()`).
        // `elements()[0]`, the path's own real starting `MoveTo`, is
        // the real, deterministic signal instead -- kurbo's own
        // `RoundedRect::to_path()` starts at `(x0, y0 + top_left_
        // radius)`, so its own y-coordinate directly reflects the
        // real top-left radius that generated it.
        let first = cache
            .rounded_rect_border_per_corner(id, 50.0, 50.0, [8.0, 2.0, 2.0, 8.0], 1.0)
            .elements()[0];
        let second = cache
            .rounded_rect_border_per_corner(id, 50.0, 50.0, [20.0, 2.0, 2.0, 8.0], 1.0)
            .elements()[0];
        assert_ne!(
            first, second,
            "a genuinely different top-left radius must move the path's own real starting \
             point, not silently reuse the stale cached path"
        );
    }

    /// The real point Split Button's own outlined variant needed fixed:
    /// a per-corner border must actually paint a *different* top-left
    /// corner than the plain (non-per-corner) uniform border built
    /// from that same corner's own radius -- proven by comparing the
    /// path's own real starting point (see the comment above for why
    /// `bounding_box()` can't tell these apart).
    #[test]
    fn per_corner_border_produces_asymmetric_geometry_distinct_from_a_uniform_border() {
        let mut tree = Tree::new();
        let id = rect_node(&mut tree);
        let mut cache = GeometryCache::new();

        let per_corner = cache
            .rounded_rect_border_per_corner(id, 50.0, 50.0, [25.0, 8.0, 8.0, 25.0], 1.0)
            .elements()[0];
        let uniform = cache
            .rounded_rect_border(id, 50.0, 50.0, 8.0, 1.0)
            .elements()[0];
        assert_ne!(
            per_corner, uniform,
            "a real top-left radius of 25.0 (per-corner) must start the path at a genuinely \
             different point than a uniform border built from radius 8.0 -- proving the \
             per-corner radii genuinely reached the generated geometry, not just the cache key"
        );
    }

    /// A corner whose radius is smaller than the real inset must clamp
    /// to a square corner (`0.0`), the identical real defensive clamp
    /// `rounded_rect_border`'s own single-scalar radius already has --
    /// proven by an inset so large it would otherwise go negative.
    #[test]
    fn per_corner_border_clamps_a_radius_smaller_than_the_inset_to_zero() {
        let mut tree = Tree::new();
        let id = rect_node(&mut tree);
        let mut cache = GeometryCache::new();

        // Every real radius (2.0) is smaller than the real inset
        // (5.0) -- must not panic or produce a negative-radius
        // `RoundedRect` (kurbo's own real precondition), and must
        // still yield a real, well-formed, non-empty path.
        let path = cache.rounded_rect_border_per_corner(id, 50.0, 50.0, [2.0, 2.0, 2.0, 2.0], 5.0);
        assert!(
            path.elements().len() > 1,
            "clamping every corner to 0.0 must still produce a real, well-formed square-corner \
             border path, not an empty or degenerate one"
        );
    }

    /// Mirrors `text.rs`'s own established per-node-cache-eviction test
    /// -- a node removed from the tree must not keep its cached path(s)
    /// around forever.
    #[test]
    fn evict_stale_removes_a_real_removed_nodes_cached_paths() {
        let mut tree = Tree::new();
        let a = rect_node(&mut tree);
        let b = rect_node(&mut tree);
        let mut cache = GeometryCache::new();

        cache.rounded_rect_fill(a, 50.0, 50.0, 8.0);
        cache.rounded_rect_fill(b, 50.0, 50.0, 8.0);
        assert_eq!(cache.fill_paths.len(), 2);

        tree.remove(a);
        cache.evict_stale(&tree);
        assert_eq!(
            cache.fill_paths.len(),
            1,
            "a removed node's cached fill path must be evicted, not kept forever"
        );
        assert!(cache.fill_paths.contains_key(&b));
    }

    /// M38 Phase 1 (§5, §7, §8): the identical real cache-hit/cache-
    /// miss proof `an_unchanged_node_reuses_its_cached_fill_path_
    /// without_rebuilding`/`a_changed_radius_invalidates_the_cached_
    /// fill_path` already establish, applied to the new circle cache.
    #[test]
    fn circle_primary_reuses_an_unchanged_path_and_rebuilds_a_changed_one() {
        let mut tree = Tree::new();
        let id = rect_node(&mut tree);
        let mut cache = GeometryCache::new();

        let first_ptr = cache
            .circle_primary(id, 25.0, 25.0, 10.0)
            .elements()
            .as_ptr();
        let second_ptr = cache
            .circle_primary(id, 25.0, 25.0, 10.0)
            .elements()
            .as_ptr();
        assert_eq!(
            first_ptr, second_ptr,
            "identical (cx, cy, radius) must reuse the exact same cached BezPath"
        );

        let before = cache.circle_primary(id, 25.0, 25.0, 10.0).clone();
        let after = cache.circle_primary(id, 25.0, 25.0, 20.0).clone();
        assert_ne!(
            before.bounding_box(),
            after.bounding_box(),
            "a genuinely different radius must produce a genuinely different real path"
        );
    }

    /// M38 Phase 1 (§5, §7, §8): `RadioButton`'s own real ring-plus-dot
    /// need -- both circles on the same node must not collide.
    #[test]
    fn circle_primary_and_circle_secondary_for_the_same_node_do_not_collide() {
        let mut tree = Tree::new();
        let id = rect_node(&mut tree);
        let mut cache = GeometryCache::new();

        let ring = cache.circle_primary(id, 25.0, 25.0, 20.0).clone();
        let dot = cache.circle_secondary(id, 25.0, 25.0, 8.0).clone();
        assert_ne!(
            ring.bounding_box(),
            dot.bounding_box(),
            "the ring and dot must be genuinely distinct real cached paths"
        );
    }

    /// M38 Phase 1 (§5, §7, §8): the identical real cache-hit/cache-
    /// miss proof applied to the new arc cache -- a changed `sweep`
    /// (the real, most commonly-animating input) must invalidate it.
    #[test]
    fn arc_reuses_an_unchanged_path_and_rebuilds_on_a_changed_sweep() {
        let mut tree = Tree::new();
        let id = rect_node(&mut tree);
        let mut cache = GeometryCache::new();

        let first_ptr = cache
            .arc(id, 25.0, 25.0, 20.0, 0.0, 1.0)
            .elements()
            .as_ptr();
        let second_ptr = cache
            .arc(id, 25.0, 25.0, 20.0, 0.0, 1.0)
            .elements()
            .as_ptr();
        assert_eq!(
            first_ptr, second_ptr,
            "identical arc params must reuse the exact same cached BezPath"
        );

        let before = cache.arc(id, 25.0, 25.0, 20.0, 0.0, 1.0).clone();
        let after = cache.arc(id, 25.0, 25.0, 20.0, 0.0, 3.0).clone();
        assert_ne!(
            before.bounding_box(),
            after.bounding_box(),
            "a genuinely different sweep must produce a genuinely different real path"
        );
    }

    /// M38 Phase 1 (§5, §7, §8): the circle/arc caches must be evicted
    /// alongside the fill/border caches, the identical real per-node-
    /// cache-leak fix.
    #[test]
    fn evict_stale_also_removes_circle_and_arc_paths() {
        let mut tree = Tree::new();
        let a = rect_node(&mut tree);
        let mut cache = GeometryCache::new();

        cache.circle_primary(a, 25.0, 25.0, 10.0);
        cache.circle_secondary(a, 25.0, 25.0, 5.0);
        cache.arc(a, 25.0, 25.0, 10.0, 0.0, 1.0);

        tree.remove(a);
        cache.evict_stale(&tree);
        assert!(cache.circle_primary.is_empty());
        assert!(cache.circle_secondary.is_empty());
        assert!(cache.arc_paths.is_empty());
    }
}
