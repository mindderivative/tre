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
//! deliberately scoped to `Rect`/`Splitter`'s own plain (non-shape-
//! morph) fill/border paths, the single most common real paint call in
//! any app (every button/card/panel/dialog background), rather than
//! chasing the same modest win across every other curve-tessellating
//! `NodeKind` (`RadioButton`'s ring, `CircularProgress`'s arc, etc.) for
//! comparatively little additional real benefit -- a real, deliberate
//! v1 scope limit, not an oversight.

use std::collections::HashMap;

use engine_core::{NodeId, Tree};
use peniko::kurbo::{BezPath, RoundedRect, Shape};

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
}

/// Owns every real, currently-cached tessellated `BezPath` across
/// frames -- built once and threaded through every `build_tree_scene`
/// call, the identical "long-lived state, not built fresh per call"
/// shape `TextRenderer`/`Resources` already establish (see their own
/// doc comments for why). Fill and border paths live in separate maps,
/// not one shared map keyed by `NodeId` alone, since a single real
/// bordered `Rect` needs both at once.
pub struct GeometryCache {
    fill_paths: HashMap<NodeId, (RectPathParams, BezPath)>,
    border_paths: HashMap<NodeId, (RectPathParams, BezPath)>,
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

    fn get_or_build(
        cache: &mut HashMap<NodeId, (RectPathParams, BezPath)>,
        id: NodeId,
        params: RectPathParams,
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
    #[test]
    fn a_changed_radius_invalidates_the_cached_fill_path() {
        let mut tree = Tree::new();
        let id = rect_node(&mut tree);
        let mut cache = GeometryCache::new();

        let first = cache.rounded_rect_fill(id, 50.0, 50.0, 8.0).clone();
        let second = cache.rounded_rect_fill(id, 50.0, 50.0, 20.0).clone();
        assert_ne!(
            first.bounding_box(),
            second.bounding_box(),
            "a genuinely different corner radius must produce a genuinely different real path, \
             not silently reuse the stale one"
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
}
