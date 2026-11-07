//! M95 (D4): the `path` node's data -- any vector path (arbitrary SVG
//! path data), fitted into a view box, filled, stroked, trimmed, and
//! morphed.
//!
//! **Morphing** resamples both paths by arc length rather than matching
//! their vertices, so any two paths morph -- including curves, several
//! subpaths, and open paths. Subpaths pair up in order; a closed pair is
//! aligned first by the starting point and winding that move the samples
//! least, so shapes drawn from different corners still morph cleanly. A
//! pair that can't correspond -- different subpath counts, or an open
//! path against a closed one -- switches at the halfway point instead.
//! Mid-morph frames are polygons of `MORPH_SAMPLES` points per subpath;
//! the last frame is the exact target path.

use peniko::kurbo::{
    Affine, BezPath, CubicBez, Line, ParamCurve, ParamCurveArclen, PathEl, PathSeg, Point, QuadBez,
    Rect,
};

use crate::animation::{Animated, Interpolate};

/// Points per subpath on a mid-morph frame.
const MORPH_SAMPLES: usize = 96;

/// Arc-length accuracy, in path units.
const ACCURACY: f64 = 1e-3;

/// 0.5.9 (#236): what a `path` node is hit by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HitShape {
    /// The node's box, with its corners rounded as painted: how every node was hit before, and
    /// the default.
    #[default]
    Box,
    /// What is painted: the fill region (when the fill has colour or a gradient) united with the
    /// stroke (when the stroke has width and colour), so a click in a hole, a notch or outside a
    /// curve passes to what is behind.
    Painted,
}

impl HitShape {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "box" => Some(Self::Box),
            "painted" => Some(Self::Painted),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Box => "box",
            Self::Painted => "painted",
        }
    }
}

/// #271: how a path's data morphs into other data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MorphMode {
    /// Both paths are resampled by arc length: any two paths morph, and mid-frames are polygons.
    /// The default.
    #[default]
    Resample,
    /// Two paths with the same sequence of commands morph number by number, so lines, curves and
    /// arcs stay what they are and every mid-frame is exact; any other pair resamples.
    Structure,
    /// As `Structure`, but a pair with different command sequences is an error, not a resample.
    Strict,
}

impl MorphMode {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "resample" => Some(Self::Resample),
            "structure" => Some(Self::Structure),
            "strict" => Some(Self::Strict),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Resample => "resample",
            Self::Structure => "structure",
            Self::Strict => "strict",
        }
    }
}

/// A path's data -- the value an `Animated<PathData>` interpolates. The mode is how a morph
/// *into* this data is made (the target's, as an animation's `to` carries it); it is no part of
/// the shape.
#[derive(Clone, Debug, PartialEq)]
pub struct PathData(pub BezPath, pub MorphMode);

impl PathData {
    /// Parses SVG path data -- the `d` attribute's syntax: `M`, `L`, `H`,
    /// `V`, `C`, `S`, `Q`, `T`, `A`, `Z`, absolute and relative.
    pub fn from_svg(data: &str) -> Result<Self, String> {
        BezPath::from_svg(data)
            .map(|path| Self(path, MorphMode::Resample))
            .map_err(|err| err.to_string())
    }

    /// The same data, morphed into by `mode`.
    pub fn with_mode(mut self, mode: MorphMode) -> Self {
        self.1 = mode;
        self
    }

    /// Whether the two paths have the same sequence of commands (each `M`, `L`, `Q`, `C`, `Z` in
    /// the same place; an arc is already curves), so a structure morph can pair their numbers.
    pub fn same_structure(&self, other: &Self) -> bool {
        same_structure(&self.0, &other.0)
    }

    /// The path as SVG `d` data.
    pub fn to_svg(&self) -> String {
        self.0.to_svg()
    }
}

impl Interpolate for PathData {
    fn interpolate(&self, other: &Self, t: f64) -> Self {
        if t <= 0.0 {
            return self.clone();
        }
        if t >= 1.0 {
            return other.clone();
        }
        if other.1 != MorphMode::Resample
            && let Some(path) = morph_structure(&self.0, &other.0, t)
        {
            return Self(path, other.1);
        }
        match morph(&self.0, &other.0, t) {
            Some(path) => Self(path, other.1),
            None if t < 0.5 => self.clone(),
            None => other.clone(),
        }
    }
}

fn same_structure(a: &BezPath, b: &BezPath) -> bool {
    let (a, b) = (a.elements(), b.elements());
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| std::mem::discriminant(x) == std::mem::discriminant(y))
}

/// The `t` frame of morphing `from` into `to` number by number, or `None` when their commands
/// differ. Every point moves in a straight line, so a curve stays a curve of the same kind, and a
/// quarter-circle cubic whose radius changes stays a quarter circle.
fn morph_structure(from: &BezPath, to: &BezPath, t: f64) -> Option<BezPath> {
    if !same_structure(from, to) {
        return None;
    }
    let mix = |a: Point, b: Point| a.lerp(b, t);
    let elements = from
        .elements()
        .iter()
        .zip(to.elements())
        .map(|pair| match pair {
            (PathEl::MoveTo(a), PathEl::MoveTo(b)) => PathEl::MoveTo(mix(*a, *b)),
            (PathEl::LineTo(a), PathEl::LineTo(b)) => PathEl::LineTo(mix(*a, *b)),
            (PathEl::QuadTo(a1, a2), PathEl::QuadTo(b1, b2)) => {
                PathEl::QuadTo(mix(*a1, *b1), mix(*a2, *b2))
            }
            (PathEl::CurveTo(a1, a2, a3), PathEl::CurveTo(b1, b2, b3)) => {
                PathEl::CurveTo(mix(*a1, *b1), mix(*a2, *b2), mix(*a3, *b3))
            }
            _ => PathEl::ClosePath,
        })
        .collect::<Vec<_>>();
    Some(BezPath::from_vec(elements))
}

/// One subpath: its segments and whether it closes.
struct Subpath {
    segments: Vec<PathSeg>,
    closed: bool,
}

fn subpaths(path: &BezPath) -> Vec<Subpath> {
    let mut out: Vec<Subpath> = Vec::new();
    let mut start = Point::ZERO;
    let mut last = Point::ZERO;
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                out.push(Subpath {
                    segments: Vec::new(),
                    closed: false,
                });
                start = p;
                last = p;
            }
            PathEl::LineTo(p) => {
                push_seg(&mut out, PathSeg::Line(Line::new(last, p)));
                last = p;
            }
            PathEl::QuadTo(p1, p2) => {
                push_seg(&mut out, PathSeg::Quad(QuadBez::new(last, p1, p2)));
                last = p2;
            }
            PathEl::CurveTo(p1, p2, p3) => {
                push_seg(&mut out, PathSeg::Cubic(CubicBez::new(last, p1, p2, p3)));
                last = p3;
            }
            PathEl::ClosePath => {
                if last != start {
                    push_seg(&mut out, PathSeg::Line(Line::new(last, start)));
                }
                if let Some(sub) = out.last_mut() {
                    sub.closed = true;
                }
                last = start;
            }
        }
    }
    out.retain(|sub| !sub.segments.is_empty());
    out
}

fn push_seg(out: &mut Vec<Subpath>, seg: PathSeg) {
    if out.is_empty() {
        out.push(Subpath {
            segments: Vec::new(),
            closed: false,
        });
    }
    if let Some(sub) = out.last_mut() {
        sub.segments.push(seg);
    }
}

/// `count` points spaced evenly by arc length along `sub` -- including
/// both ends of an open subpath, and without repeating the start of a
/// closed one.
fn resample(sub: &Subpath, count: usize) -> Vec<Point> {
    let lengths: Vec<f64> = sub.segments.iter().map(|s| s.arclen(ACCURACY)).collect();
    let total: f64 = lengths.iter().sum();
    let divisions = if sub.closed { count } else { count - 1 };
    let mut points = Vec::with_capacity(count);
    let mut seg_index = 0;
    let mut seg_start = 0.0;
    for i in 0..count {
        let target = total * i as f64 / divisions as f64;
        while seg_index + 1 < lengths.len() && seg_start + lengths[seg_index] < target {
            seg_start += lengths[seg_index];
            seg_index += 1;
        }
        let seg = &sub.segments[seg_index];
        let local = (target - seg_start).clamp(0.0, lengths[seg_index]);
        let param = if lengths[seg_index] > 0.0 {
            seg.inv_arclen(local, ACCURACY)
        } else {
            0.0
        };
        points.push(seg.eval(param));
    }
    points
}

/// `to` reordered -- rotated, and reversed if that fits better -- so its
/// points travel least from `from`'s. Closed subpaths only.
fn align(from: &[Point], to: &[Point]) -> Vec<Point> {
    let n = to.len();
    let reversed: Vec<Point> = to.iter().rev().copied().collect();
    let mut best = (f64::INFINITY, 0, false);
    for (candidate, is_reversed) in [(to, false), (reversed.as_slice(), true)] {
        for offset in 0..n {
            let cost: f64 = (0..n)
                .map(|i| from[i].distance_squared(candidate[(i + offset) % n]))
                .sum();
            if cost < best.0 {
                best = (cost, offset, is_reversed);
            }
        }
    }
    let (_, offset, is_reversed) = best;
    let source = if is_reversed { &reversed[..] } else { to };
    (0..n).map(|i| source[(i + offset) % n]).collect()
}

/// The `t` frame of morphing `from` into `to`, or `None` when their
/// subpaths can't correspond.
fn morph(from: &BezPath, to: &BezPath, t: f64) -> Option<BezPath> {
    let a = subpaths(from);
    let b = subpaths(to);
    if a.is_empty() || a.len() != b.len() || a.iter().zip(&b).any(|(x, y)| x.closed != y.closed) {
        return None;
    }
    let mut out = BezPath::new();
    for (x, y) in a.iter().zip(&b) {
        let pa = resample(x, MORPH_SAMPLES);
        let mut pb = resample(y, MORPH_SAMPLES);
        if x.closed {
            pb = align(&pa, &pb);
        }
        for (i, (p, q)) in pa.iter().zip(&pb).enumerate() {
            let point = p.lerp(*q, t);
            if i == 0 {
                out.move_to(point);
            } else {
                out.line_to(point);
            }
        }
        if x.closed {
            out.close_path();
        }
    }
    Some(out)
}

/// The part of `path` between the fractions `start` and `end` of its total
/// length, counted across every subpath in order -- how a stroke is
/// trimmed. `start >= end` leaves nothing.
pub fn trim(path: &BezPath, start: f64, end: f64) -> BezPath {
    let start = start.clamp(0.0, 1.0);
    let end = end.clamp(0.0, 1.0);
    if start <= 0.0 && end >= 1.0 {
        return path.clone();
    }
    let mut out = BezPath::new();
    if start >= end {
        return out;
    }
    let segments: Vec<PathSeg> = subpaths(path)
        .into_iter()
        .flat_map(|sub| sub.segments)
        .collect();
    let lengths: Vec<f64> = segments.iter().map(|s| s.arclen(ACCURACY)).collect();
    let total: f64 = lengths.iter().sum();
    let (from, to) = (start * total, end * total);
    let mut cursor = 0.0;
    let mut pen: Option<Point> = None;
    for (seg, &len) in segments.iter().zip(&lengths) {
        let (seg_from, seg_to) = (cursor, cursor + len);
        cursor = seg_to;
        if seg_to <= from || seg_from >= to || len <= 0.0 {
            continue;
        }
        let t0 = if from > seg_from {
            seg.inv_arclen(from - seg_from, ACCURACY)
        } else {
            0.0
        };
        let t1 = if to < seg_to {
            seg.inv_arclen(to - seg_from, ACCURACY)
        } else {
            1.0
        };
        let piece = seg.subsegment(t0..t1);
        let piece_start = piece.start();
        if pen.is_none_or(|p| p.distance(piece_start) > 1e-9) {
            out.move_to(piece_start);
        }
        match piece {
            PathSeg::Line(line) => out.line_to(line.p1),
            PathSeg::Quad(quad) => out.quad_to(quad.p1, quad.p2),
            PathSeg::Cubic(cubic) => out.curve_to(cubic.p1, cubic.p2, cubic.p3),
        }
        pen = Some(piece.end());
    }
    out
}

/// The transform fitting `view_box` into a `width` x `height` box --
/// uniformly scaled and centered, like SVG's default `xMidYMid meet`. No
/// view box leaves the path in the node's own coordinates.
pub fn fit_transform(view_box: Option<Rect>, width: f64, height: f64) -> Affine {
    let Some(vb) = view_box else {
        return Affine::IDENTITY;
    };
    if vb.width() <= 0.0 || vb.height() <= 0.0 {
        return Affine::IDENTITY;
    }
    let scale = (width / vb.width()).min(height / vb.height());
    let tx = (width - vb.width() * scale) / 2.0 - vb.x0 * scale;
    let ty = (height - vb.height() * scale) / 2.0 - vb.y0 * scale;
    Affine::new([scale, 0.0, 0.0, scale, tx, ty])
}

/// `NodeKind::Path`'s own state. Fill and stroke color and width live in
/// `PaintProperties` (`background`, `border_color`, `border_width`), the
/// same as every other node's.
pub struct PathState {
    pub data: Animated<PathData>,
    /// 0.5.9 (#236): which part of the node takes pointer events and counts in an
    /// `input_region="auto"`.
    pub hit_shape: HitShape,
    /// #271: how an animation of `data` morphs (`morph`).
    pub morph: MorphMode,
    /// #272: when set, `data` is this template filled in with `params`, and is evaluated again
    /// whenever a parameter moves.
    pub template: Option<crate::PathTemplate>,
    /// #272: the named numbers a template reads, each animatable.
    pub params: Vec<(String, Animated<f64>)>,
    /// The `(min_x, min_y, width, height)` region `data` is drawn in,
    /// fitted into the node's box; `None` draws `data` in node-local
    /// pixels.
    pub view_box: Option<Rect>,
    /// Trim fractions for the stroke, `0.0..=1.0`.
    pub trim_start: Animated<f64>,
    pub trim_end: Animated<f64>,
}

impl PathState {
    pub fn new(data: PathData) -> Self {
        Self {
            data: Animated::new(data),
            hit_shape: HitShape::Box,
            morph: MorphMode::Resample,
            template: None,
            params: Vec::new(),
            view_box: None,
            trim_start: Animated::new(0.0),
            trim_end: Animated::new(1.0),
        }
    }

    /// Advances every animated field; `true` while any still runs.
    pub fn tick(
        &mut self,
        now: std::time::Instant,
        completed: &mut Vec<crate::CompletionHandle>,
    ) -> bool {
        let data = self.data.tick(now, completed);
        // A parameter that moved (or took its last step) this tick changes the shape.
        let was_moving = self.params.iter().any(|(_, p)| p.active.is_some());
        let mut params_moving = false;
        for (_, param) in &mut self.params {
            params_moving |= param.tick(now, completed);
        }
        if was_moving {
            let _ = self.reevaluate();
        }
        let data = data || params_moving;
        let start = self.trim_start.tick(now, completed);
        let end = self.trim_end.tick(now, completed);
        data || start || end
    }

    /// #272: a parameter's current value.
    pub fn param(&self, name: &str) -> Option<f64> {
        self.params
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, p)| p.current)
    }

    /// #272: sets a parameter outright (ending its animation), adding it if new.
    pub fn set_param(&mut self, name: &str, value: f64) {
        match self.params.iter_mut().find(|(n, _)| n == name) {
            Some((_, p)) => p.set_now(value),
            None => self.params.push((name.to_owned(), Animated::new(value))),
        }
    }

    /// #272: fills the template in with the parameters as they are now and makes the result
    /// the node's data. `Ok` and nothing done when there is no template; an error (a missing
    /// parameter, data that does not parse) leaves the data as it was.
    pub fn reevaluate(&mut self) -> Result<(), String> {
        let Some(template) = &self.template else {
            return Ok(());
        };
        let params = &self.params;
        let data = template.evaluate(&|name| {
            params
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, p)| p.current)
        })?;
        let mode = self.data.current.1;
        self.data.set_now(data.with_mode(mode));
        Ok(())
    }

    /// The path in node-local pixels -- `data` fitted into a `width` x
    /// `height` box by the view box -- and its trimmed stroke outline.
    pub fn geometry(&self, width: f64, height: f64) -> (BezPath, BezPath) {
        let fitted = fit_transform(self.view_box, width, height) * self.data.current.0.clone();
        let stroke = trim(&fitted, self.trim_start.current, self.trim_end.current);
        (fitted, stroke)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use peniko::kurbo::Shape;

    fn square(x: f64, y: f64, size: f64) -> PathData {
        PathData::from_svg(&format!("M{x},{y} h{size} v{size} h-{size} Z")).unwrap()
    }

    #[test]
    fn svg_data_round_trips_and_rejects_garbage() {
        let path = PathData::from_svg("M0,0 L10,0 L10,10 Z").unwrap();
        assert_eq!(path.0.elements().len(), 4);
        assert!(PathData::from_svg("M0,0 X10").is_err());
        // Arcs parse too.
        assert!(PathData::from_svg("M0,12 A12,12 0 1 1 24,12").is_ok());
    }

    /// A `w` by `h` rectangle with corners of radius `r`, as SVG arcs.
    fn rounded(w: f64, h: f64, r: f64) -> PathData {
        PathData::from_svg(&format!(
            "M{r},0 H{} A{r},{r} 0 0 1 {w},{r} V{} A{r},{r} 0 0 1 {},{h} H{r} \
             A{r},{r} 0 0 1 0,{} V{r} A{r},{r} 0 0 1 {r},0 Z",
            w - r,
            h - r,
            w - r,
            h - r
        ))
        .unwrap()
    }

    #[test]
    fn a_structure_morph_keeps_every_command_and_ends_exactly() {
        let a = rounded(100.0, 60.0, 4.0);
        let b = rounded(100.0, 60.0, 20.0).with_mode(MorphMode::Structure);
        assert!(a.same_structure(&b));
        assert_eq!(a.interpolate(&b, 0.0), a);
        assert_eq!(a.interpolate(&b, 1.0), b);
        let mid = a.interpolate(&b, 0.5);
        let kinds = |p: &PathData| {
            p.0.elements()
                .iter()
                .map(std::mem::discriminant)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            kinds(&mid),
            kinds(&a),
            "the arcs are still four cubics, not a polygon"
        );
        // Midway the corner is the radius 12 arc: its cubic's far end is at (12, 0) from the corner.
        let want = rounded(100.0, 60.0, 12.0);
        for (x, y) in mid.0.elements().iter().zip(want.0.elements()) {
            let close = |p: Point, q: Point| p.distance(q) < 1e-9;
            match (x, y) {
                (PathEl::MoveTo(p), PathEl::MoveTo(q)) | (PathEl::LineTo(p), PathEl::LineTo(q)) => {
                    assert!(close(*p, *q))
                }
                (PathEl::CurveTo(a1, a2, a3), PathEl::CurveTo(b1, b2, b3)) => {
                    assert!(close(*a1, *b1) && close(*a2, *b2) && close(*a3, *b3))
                }
                (PathEl::ClosePath, PathEl::ClosePath) => {}
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn a_structure_morph_of_different_commands_resamples_and_the_default_always_does() {
        let a = rounded(100.0, 60.0, 4.0);
        let line = PathData::from_svg("M0,0 L100,0 L100,60 L0,60 Z").unwrap();
        assert!(!a.same_structure(&line));
        let structured = line.clone().with_mode(MorphMode::Structure);
        let mid = a.interpolate(&structured, 0.5);
        assert_eq!(mid.0.elements().len(), MORPH_SAMPLES + 1, "resampled");
        // The default never pairs numbers, even for equal commands.
        let plain = rounded(100.0, 60.0, 20.0);
        assert_eq!(
            a.interpolate(&plain, 0.5).0.elements().len(),
            MORPH_SAMPLES + 1
        );
    }

    #[test]
    fn morph_endpoints_are_exact_and_midpoints_resample() {
        let a = square(0.0, 0.0, 10.0);
        let b = square(10.0, 10.0, 10.0);
        assert_eq!(a.interpolate(&b, 0.0), a);
        assert_eq!(a.interpolate(&b, 1.0), b);
        let mid = a.interpolate(&b, 0.5);
        let bounds = mid.0.bounding_box();
        assert!((bounds.x0 - 5.0).abs() < 1e-6 && (bounds.x1 - 15.0).abs() < 1e-6);
        // One closed polygon of MORPH_SAMPLES points: move, lines, close.
        assert_eq!(mid.0.elements().len(), MORPH_SAMPLES + 1);
    }

    #[test]
    fn closed_morph_aligns_a_differently_started_contour() {
        // The same square, drawn from the opposite corner the other way
        // round: once aligned, a mid-morph frame is still that square.
        let a = square(0.0, 0.0, 10.0);
        let b = PathData::from_svg("M10,10 h-10 v-10 h10 Z").unwrap();
        let mid = a.interpolate(&b, 0.5);
        let bounds = mid.0.bounding_box();
        assert!((bounds.width() - 10.0).abs() < 1e-6);
        assert!((bounds.height() - 10.0).abs() < 1e-6);
    }

    #[test]
    fn open_paths_morph_and_open_against_closed_switches_halfway() {
        let line = PathData::from_svg("M0,0 L10,0").unwrap();
        let other = PathData::from_svg("M0,10 L10,10").unwrap();
        let mid = line.interpolate(&other, 0.5);
        let bounds = mid.0.bounding_box();
        assert!((bounds.y0 - 5.0).abs() < 1e-6 && (bounds.y1 - 5.0).abs() < 1e-6);

        let closed = square(0.0, 0.0, 10.0);
        assert_eq!(line.interpolate(&closed, 0.4), line);
        assert_eq!(line.interpolate(&closed, 0.6), closed);
    }

    #[test]
    fn trim_keeps_the_requested_fraction_of_the_length() {
        let line = BezPath::from_svg("M0,0 L100,0").unwrap();
        let trimmed = trim(&line, 0.25, 0.75);
        let bounds = trimmed.bounding_box();
        assert!((bounds.x0 - 25.0).abs() < 1e-6 && (bounds.x1 - 75.0).abs() < 1e-6);
        assert!(trim(&line, 0.6, 0.4).elements().is_empty());
        assert_eq!(trim(&line, 0.0, 1.0), line);

        // Across two subpaths of equal length: the first half is exactly
        // the first subpath.
        let two = BezPath::from_svg("M0,0 L10,0 M0,10 L10,10").unwrap();
        let first = trim(&two, 0.0, 0.5).bounding_box();
        assert!(first.y1.abs() < 1e-6 && (first.x1 - 10.0).abs() < 1e-6);
    }

    #[test]
    fn view_box_fits_uniformly_and_centers() {
        let vb = Rect::new(0.0, 0.0, 24.0, 24.0);
        let fit = fit_transform(Some(vb), 48.0, 96.0);
        assert_eq!(fit * Point::new(0.0, 0.0), Point::new(0.0, 24.0));
        assert_eq!(fit * Point::new(24.0, 24.0), Point::new(48.0, 72.0));
        assert_eq!(fit_transform(None, 48.0, 96.0), Affine::IDENTITY);
    }
}
