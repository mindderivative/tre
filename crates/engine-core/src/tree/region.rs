//! 0.5.7 (#204): the input region of a surface -- the places where a click lands on
//! something drawn. A layer surface hands this to the compositor so a click anywhere else
//! goes to the window below.
//!
//! It is built from the same shapes hit testing uses: a node's box with its corners
//! rounded as painted, a canvas's circle. Each shape is flattened to a polygon in surface
//! coordinates (so a node's transform counts), the polygons are cut into horizontal bands,
//! and each band's covered x-spans become rectangles. Bands are measured over their whole
//! height and rounded outward, so the region never loses a drawn pixel; at most it covers a
//! pixel or two too many along a curve.

use super::*;
use peniko::kurbo::{PathEl, RoundedRect, Shape};

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

/// A closed polygon (points in surface coordinates) for a shape given as path elements.
fn flatten(elements: impl Iterator<Item = PathEl>, to_surface: Affine) -> Vec<Point> {
    let mut points = Vec::new();
    peniko::kurbo::flatten(elements, 0.25, |el| match el {
        PathEl::MoveTo(p) | PathEl::LineTo(p) => points.push(to_surface * p),
        _ => {}
    });
    points
}

/// The x-extent of `polygon` within the strip `y0..=y1`, or `None` if it misses it.
fn strip_extent(polygon: &[Point], y0: f64, y1: f64) -> Option<(f64, f64)> {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    let mut take = |x: f64| {
        lo = lo.min(x);
        hi = hi.max(x);
    };
    let n = polygon.len();
    for i in 0..n {
        let (a, b) = (polygon[i], polygon[(i + 1) % n]);
        let (top, bottom) = if a.y <= b.y { (a, b) } else { (b, a) };
        if bottom.y < y0 || top.y > y1 {
            continue;
        }
        let at = |y: f64| {
            if (bottom.y - top.y).abs() < f64::EPSILON {
                None
            } else {
                Some(top.x + (bottom.x - top.x) * (y - top.y) / (bottom.y - top.y))
            }
        };
        // Clip the edge to the strip; a horizontal edge inside it counts whole.
        let from = if top.y >= y0 {
            top.x
        } else {
            at(y0).unwrap_or(top.x)
        };
        let to = if bottom.y <= y1 {
            bottom.x
        } else {
            at(y1).unwrap_or(bottom.x)
        };
        take(from);
        take(to);
    }
    (lo <= hi).then_some((lo, hi))
}

/// Cuts the polygons into bands of `step` pixels and returns the rectangles covering them,
/// vertically merged where consecutive bands have the same spans.
fn bands(polygons: &[Vec<Point>], top: i32, bottom: i32, step: i32) -> Vec<RegionRect> {
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
        let mut spans: Vec<(f64, f64)> = polygons
            .iter()
            .filter_map(|p| strip_extent(p, f64::from(y), f64::from(y_end)))
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
        let mut polygons = Vec::new();
        self.collect_shapes(root, Affine::IDENTITY, &mut polygons);
        if polygons.is_empty() {
            return Vec::new();
        }
        // The union's vertical extent, clamped to the surface.
        let (mut top, mut bottom) = (f64::INFINITY, f64::NEG_INFINITY);
        for p in polygons.iter().flatten() {
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

    fn collect_shapes(&self, id: NodeId, parent_transform: Affine, out: &mut Vec<Vec<Point>>) {
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
        let polygon = match &node.kind {
            NodeKind::Canvas(state) => match &state.hit_test {
                Some(CustomHitTest::Circle { cx, cy, radius }) => {
                    let pad = layout.padding;
                    let centre = Point::new(cx + f64::from(pad.left), cy + f64::from(pad.top));
                    // Circumscribed: a polygon around the circle, never inside it.
                    let sides = 48;
                    let r = radius / (std::f64::consts::PI / f64::from(sides)).cos();
                    (0..sides)
                        .map(|i| {
                            let a = std::f64::consts::TAU * f64::from(i) / f64::from(sides);
                            composed * Point::new(centre.x + r * a.cos(), centre.y + r * a.sin())
                        })
                        .collect()
                }
                // A path's own shape is not decomposed (yet): its box covers it.
                Some(CustomHitTest::Path { .. }) | None => {
                    flatten(Rect::new(0.0, 0.0, w, h).path_elements(0.1), composed)
                }
            },
            _ => {
                let r = corner_radii(&node.paint);
                if r.iter().all(|v| *v <= 0.0) {
                    flatten(Rect::new(0.0, 0.0, w, h).path_elements(0.1), composed)
                } else {
                    flatten(
                        RoundedRect::new(0.0, 0.0, w, h, (r[0], r[1], r[2], r[3]))
                            .path_elements(0.1),
                        composed,
                    )
                }
            }
        };
        if polygon.len() >= 3 {
            out.push(polygon);
        }
    }
}
