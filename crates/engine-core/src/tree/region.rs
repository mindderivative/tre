//! 0.5.8 (#204): the input region of a surface -- the places where a click lands on
//! something drawn. A layer surface hands this to the compositor so a click anywhere else
//! goes to the window below.
//!
//! It is built from the same shapes hit testing uses: a node's box with its corners
//! rounded as painted, a canvas's circle, and (0.5.9, #236) a path's painted fill and
//! stroke. Each shape is flattened to contours in surface coordinates (so a node's
//! transform counts) and cut into horizontal bands. A band's covered x-spans are worked out
//! exactly, hole by hole: the band is split wherever a contour has a vertex, the edges that
//! cross each slice are paired by the non-zero winding rule, and a span runs from the
//! leftmost reach of its left edge to the rightmost reach of its right edge over the slice.
//! So a ring leaves its middle uncovered and a concave notch stays a notch. Spans become
//! rectangles, rounded outward: the region never loses a drawn pixel; at most it covers a
//! pixel or two too many along a curve.

use super::*;
use crate::HitShape;
use peniko::kurbo::{
    BezPath, Cap, Join, ParamCurveNearest, PathEl, RoundedRect, Shape, Stroke, StrokeOpts,
};

/// `(x, y, width, height)` in whole logical pixels.
pub type RegionRect = (i32, i32, i32, i32);

/// The node's hit shape, as `(top-left, top-right, bottom-right, bottom-left)` radii: what
/// is painted (`corner_radii_override`, else the uniform `corner_radius`).
fn corner_radii(paint: &PaintProperties) -> [f64; 4] {
    match &paint.corner_radii_override {
        Some(radii) => radii.current.0,
        None => [paint.corner_radius.current; 4],
    }
}

/// Whether `point` (node-local) is inside the node's box with its corners rounded as
/// painted. The ordinary hit test for a box-shaped node.
pub(super) fn shape_contains(paint: &PaintProperties, layout: &Layout, point: Point) -> bool {
    if !rect_contains(layout, point) {
        return false;
    }
    let r = corner_radii(paint);
    if r.iter().all(|v| *v <= 0.0) {
        return true;
    }
    RoundedRect::new(
        0.0,
        0.0,
        f64::from(layout.size.width),
        f64::from(layout.size.height),
        (r[0], r[1], r[2], r[3]),
    )
    .contains(point)
}

/// Whether a node paints something a person could see and aim at: a box that has a fill,
/// a gradient or a border, or any other kind of node.
fn is_drawn(node: &Node) -> bool {
    let paint = &node.paint;
    if paint.opacity.current <= 0.0 {
        return false;
    }
    match node.kind {
        NodeKind::Rect | NodeKind::Container | NodeKind::VirtualList(_) => {
            paint.background.current.components[3] > 0.0
                || paint.gradient.is_some()
                || (paint.border_width.current > 0.0
                    && paint.border_color.current.components[3] > 0.0)
        }
        _ => true,
    }
}

/// A shape: closed contours (points in surface coordinates) filled by the non-zero rule.
type Shape2 = Vec<Vec<Point>>;

/// The contours of a shape given as path elements: one per sub-path, closed implicitly.
fn contours(elements: impl Iterator<Item = PathEl>, to_surface: Affine) -> Shape2 {
    let mut out: Shape2 = Vec::new();
    let mut current: Vec<Point> = Vec::new();
    peniko::kurbo::flatten(elements, 0.25, |el| match el {
        PathEl::MoveTo(p) => {
            if current.len() >= 3 {
                out.push(std::mem::take(&mut current));
            }
            current.clear();
            current.push(to_surface * p);
        }
        PathEl::LineTo(p) => current.push(to_surface * p),
        _ => {}
    });
    if current.len() >= 3 {
        out.push(current);
    }
    out
}

/// One edge of a contour, top to bottom, with the direction it was walked in.
struct Edge {
    x_top: f64,
    y_top: f64,
    x_bottom: f64,
    y_bottom: f64,
    /// `+1` walked downward, `-1` upward: the non-zero rule adds these.
    direction: i32,
}

impl Edge {
    fn x_at(&self, y: f64) -> f64 {
        let span = self.y_bottom - self.y_top;
        self.x_top + (self.x_bottom - self.x_top) * (y - self.y_top) / span
    }
}

/// The x-spans `shape` covers anywhere in the strip `y0..=y1`, each as `(left, right)`.
/// Between two neighbouring vertex heights no edge starts, ends or crosses another, so the
/// edges that span the slice keep their order; the slice's spans are the pairs where the
/// winding is not zero, and a span's reach over the slice is from its left edge's furthest-left
/// point to its right edge's furthest-right point (the interval at each height varies
/// continuously, so their union over the slice is exactly that).
fn shape_spans(shape: &Shape2, y0: f64, y1: f64) -> Vec<(f64, f64)> {
    let mut edges: Vec<Edge> = Vec::new();
    let mut cuts: Vec<f64> = vec![y0, y1];
    for contour in shape {
        let n = contour.len();
        for i in 0..n {
            let (a, b) = (contour[i], contour[(i + 1) % n]);
            if a.y == b.y {
                continue; // a horizontal edge covers no height
            }
            let (top, bottom, direction) = if a.y < b.y { (a, b, 1) } else { (b, a, -1) };
            if bottom.y <= y0 || top.y >= y1 {
                continue;
            }
            for y in [top.y, bottom.y] {
                if y > y0 && y < y1 {
                    cuts.push(y);
                }
            }
            edges.push(Edge {
                x_top: top.x,
                y_top: top.y,
                x_bottom: bottom.x,
                y_bottom: bottom.y,
                direction,
            });
        }
    }
    if edges.is_empty() {
        return Vec::new();
    }
    cuts.sort_by(f64::total_cmp);
    cuts.dedup();
    let mut spans: Vec<(f64, f64)> = Vec::new();
    for pair in cuts.windows(2) {
        let (ya, yb) = (pair[0], pair[1]);
        if yb <= ya {
            continue;
        }
        let middle = (ya + yb) / 2.0;
        // The edges that span the whole slice, left to right.
        let mut active: Vec<&Edge> = edges
            .iter()
            .filter(|e| e.y_top <= ya && e.y_bottom >= yb)
            .collect();
        active.sort_by(|p, q| p.x_at(middle).total_cmp(&q.x_at(middle)));
        let mut winding = 0;
        let mut left: Option<&Edge> = None;
        for edge in active {
            let before = winding;
            winding += edge.direction;
            if before == 0 && winding != 0 {
                left = Some(edge);
            } else if before != 0
                && winding == 0
                && let Some(l) = left.take()
            {
                spans.push((l.x_at(ya).min(l.x_at(yb)), edge.x_at(ya).max(edge.x_at(yb))));
            }
        }
    }
    spans
}

/// A rectangle as a one-contour shape.
fn rect_shape(w: f64, h: f64, to_surface: Affine) -> Shape2 {
    contours(Rect::new(0.0, 0.0, w, h).path_elements(0.1), to_surface)
}

/// Cuts the shapes into bands of `step` pixels and returns the rectangles covering them,
/// vertically merged where consecutive bands have the same spans.
fn bands(shapes: &[Shape2], top: i32, bottom: i32, step: i32) -> Vec<RegionRect> {
    let mut out: Vec<RegionRect> = Vec::new();
    // The previous band's spans and where it started.
    let mut previous: Vec<(i32, i32)> = Vec::new();
    let mut started = top;
    let flush = |spans: &[(i32, i32)], from: i32, to: i32, out: &mut Vec<RegionRect>| {
        for &(x0, x1) in spans {
            out.push((x0, from, x1 - x0, to - from));
        }
    };
    let mut y = top;
    while y < bottom {
        let y_end = (y + step).min(bottom);
        let mut spans: Vec<(f64, f64)> = shapes
            .iter()
            .flat_map(|shape| shape_spans(shape, f64::from(y), f64::from(y_end)))
            .collect();
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Vec<(i32, i32)> = Vec::new();
        for (lo, hi) in spans {
            let (lo, hi) = (lo.floor() as i32, hi.ceil() as i32);
            match merged.last_mut() {
                Some(last) if lo <= last.1 => last.1 = last.1.max(hi),
                _ => merged.push((lo, hi)),
            }
        }
        if merged != previous {
            flush(&previous, started, y, &mut out);
            previous = merged;
            started = y;
        }
        y = y_end;
    }
    flush(&previous, started, bottom, &mut out);
    out
}

impl Tree {
    /// The rectangles, in logical pixels inside `bounds` (the surface's `(width,
    /// height)`), where a click lands on something drawn: every visible, hit-testable,
    /// drawn node's shape, unioned. At most `max_rects`; a shape with too many bands is
    /// cut coarser (still covering all of it) until it fits.
    pub fn input_region(
        &self,
        root: NodeId,
        bounds: (f64, f64),
        max_rects: usize,
    ) -> Vec<RegionRect> {
        // An open dismissible or modal layer needs the press that dismisses it: a click
        // outside a menu has to reach the engine, or it would fall through to the window
        // below and the menu would stay open. So while one is open the whole surface takes
        // input (the dismissing press is then consumed, as `dismissible` says).
        if self.has_input_grab() {
            let (w, h) = (bounds.0.ceil() as i32, bounds.1.ceil() as i32);
            return if w > 0 && h > 0 {
                vec![(0, 0, w, h)]
            } else {
                Vec::new()
            };
        }
        let mut polygons: Vec<Shape2> = Vec::new();
        self.collect_shapes(root, Affine::IDENTITY, &mut polygons);
        if polygons.is_empty() {
            return Vec::new();
        }
        // The union's vertical extent, clamped to the surface.
        let (mut top, mut bottom) = (f64::INFINITY, f64::NEG_INFINITY);
        for p in polygons.iter().flatten().flatten() {
            top = top.min(p.y);
            bottom = bottom.max(p.y);
        }
        let top = top.floor().max(0.0) as i32;
        let bottom = bottom.ceil().min(bounds.1.ceil()) as i32;
        if bottom <= top {
            return Vec::new();
        }
        let clamp = |rects: Vec<RegionRect>| -> Vec<RegionRect> {
            let (w, h) = (bounds.0.ceil() as i32, bounds.1.ceil() as i32);
            rects
                .into_iter()
                .filter_map(|(x, y, rw, rh)| {
                    let (x0, y0) = (x.max(0), y.max(0));
                    let (x1, y1) = ((x + rw).min(w), (y + rh).min(h));
                    (x1 > x0 && y1 > y0).then_some((x0, y0, x1 - x0, y1 - y0))
                })
                .collect()
        };
        let mut step = 2;
        loop {
            let rects = clamp(bands(&polygons, top, bottom, step));
            if rects.len() <= max_rects {
                return rects;
            }
            if step >= bottom - top {
                // One band: the union's bounding box, the safest cover there is.
                return clamp(bands(&polygons, top, bottom, bottom - top))
                    .into_iter()
                    .take(max_rects)
                    .collect();
            }
            step *= 2;
        }
    }

    /// Whether an open layer wants presses outside itself: a dismissible or modal one.
    pub fn has_input_grab(&self) -> bool {
        self.overlays
            .iter()
            .any(|(_, meta)| meta.dismissible || meta.modal || meta.dismiss_on_outside_click)
    }

    fn collect_shapes(&self, id: NodeId, parent_transform: Affine, out: &mut Vec<Shape2>) {
        let Some(node) = self.nodes.get(id) else {
            return;
        };
        if !node.visible {
            return;
        }
        let layout = self.layout(id);
        let (sx, sy) = self.scroll_shift(id);
        let composed = parent_transform
            * Affine::translate((
                f64::from(layout.location.x) + sx,
                f64::from(layout.location.y) + sy,
            ))
            * node
                .paint
                .local_transform(f64::from(layout.size.width), f64::from(layout.size.height));
        for &child in self.children_in_paint_order(id).iter() {
            self.collect_shapes(child, composed, out);
        }
        if !node.hit_testable || !is_drawn(node) {
            return;
        }
        let (w, h) = (f64::from(layout.size.width), f64::from(layout.size.height));
        let claims = match &node.kind {
            // A bare text label never takes a click, unless it is selectable or a link.
            NodeKind::Text(state) => {
                state.options.selectable || state.options.spans.iter().any(|s| s.link.is_some())
            }
            _ => true,
        };
        if !claims || w <= 0.0 || h <= 0.0 {
            return;
        }
        match &node.kind {
            NodeKind::Canvas(state) => match &state.hit_test {
                Some(CustomHitTest::Circle { cx, cy, radius }) => {
                    let pad = layout.padding;
                    let centre = Point::new(cx + f64::from(pad.left), cy + f64::from(pad.top));
                    // Circumscribed: a polygon around the circle, never inside it.
                    let sides = 48;
                    let r = radius / (std::f64::consts::PI / f64::from(sides)).cos();
                    let polygon: Vec<Point> = (0..sides)
                        .map(|i| {
                            let a = std::f64::consts::TAU * f64::from(i) / f64::from(sides);
                            composed * Point::new(centre.x + r * a.cos(), centre.y + r * a.sin())
                        })
                        .collect();
                    out.push(vec![polygon]);
                }
                // A path's own shape is not decomposed (yet): its box covers it.
                Some(CustomHitTest::Path { .. }) | None => out.push(rect_shape(w, h, composed)),
            },
            // 0.5.9 (#236): a path that opts in is what it paints: the fill region and the stroke,
            // each a shape of its own so that their windings never cancel. A path with neither
            // claims nothing.
            NodeKind::Path(state) if state.hit_shape == HitShape::Painted => {
                if let Some(painted) = painted_path(node, layout) {
                    let origin = composed * Affine::translate(painted.origin);
                    if let Some(fill) = &painted.fill {
                        out.push(contours(fill.elements().iter().copied(), origin));
                    }
                    if let Some((stroke, width)) = &painted.stroke {
                        let style = Stroke::new(*width)
                            .with_caps(Cap::Round)
                            .with_join(Join::Round);
                        let outline = peniko::kurbo::stroke(
                            stroke.elements().iter().copied(),
                            &style,
                            &StrokeOpts::default(),
                            0.1,
                        );
                        out.push(contours(outline.elements().iter().copied(), origin));
                    }
                }
            }
            _ => {
                let r = corner_radii(&node.paint);
                if r.iter().all(|v| *v <= 0.0) {
                    out.push(rect_shape(w, h, composed));
                } else {
                    out.push(contours(
                        RoundedRect::new(0.0, 0.0, w, h, (r[0], r[1], r[2], r[3]))
                            .path_elements(0.1),
                        composed,
                    ));
                }
            }
        }
        out.retain(|shape| !shape.is_empty());
    }
}

/// 0.5.9 (#236): what a `path` node paints, in the node's own coordinates, for hit testing and
/// the input region: the fill region (when the fill has colour or a gradient), and the stroke
/// centre line with its width (when the stroke has both). `origin` moves them from the content
/// box (inside the padding, where the painter puts the view box) to the node's corner.
pub(super) struct PaintedPath {
    pub(super) fill: Option<BezPath>,
    pub(super) stroke: Option<(BezPath, f64)>,
    pub(super) origin: peniko::kurbo::Vec2,
}

/// `None` when the node is not a path or paints nothing.
pub(super) fn painted_path(node: &Node, layout: &Layout) -> Option<PaintedPath> {
    let NodeKind::Path(state) = &node.kind else {
        return None;
    };
    let pad = layout.padding;
    let (left, top) = (f64::from(pad.left), f64::from(pad.top));
    let width = (f64::from(layout.size.width) - left - f64::from(pad.right)).max(0.0);
    let height = (f64::from(layout.size.height) - top - f64::from(pad.bottom)).max(0.0);
    let (fill, stroke) = state.geometry(width, height);
    let paint = &node.paint;
    let filled = paint.gradient.is_some() || paint.background.current.components[3] > 0.0;
    let stroke_width = paint.border_width.current;
    let stroked = stroke_width > 0.0
        && !stroke.elements().is_empty()
        && (paint.border_color.current.components[3] > 0.0 || paint.border_gradient.is_some());
    (filled || stroked).then(|| PaintedPath {
        fill: filled.then_some(fill),
        stroke: stroked.then_some((stroke, stroke_width)),
        origin: peniko::kurbo::Vec2::new(left, top),
    })
}

/// Whether `point` (node-local) is on what a `path` node paints: inside its fill, or within
/// half the stroke width of its stroke (round caps and joins, as painted).
pub(super) fn painted_contains(node: &Node, layout: &Layout, point: Point) -> bool {
    let Some(painted) = painted_path(node, layout) else {
        return false;
    };
    let local = point - painted.origin;
    if let Some(fill) = &painted.fill
        && fill.contains(local)
    {
        return true;
    }
    if let Some((stroke, width)) = &painted.stroke {
        let reach = width / 2.0;
        return stroke
            .segments()
            .any(|segment| segment.nearest(local, 0.1).distance_sq <= reach * reach);
    }
    false
}
