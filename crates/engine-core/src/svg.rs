//! 0.5.4 (#141): an SVG document as a retained scene.
//!
//! `usvg` parses and *simplifies* the document -- it resolves styles, `use`,
//! units, `viewBox`, shapes, `transform` lists and paint servers down to
//! groups and paths in user space -- and this module flattens that tree into
//! a small owned form the renderer draws directly: groups with a transform,
//! an opacity and a clip, and paths with a fill and a stroke, each a solid
//! colour or a gradient. Parsing happens once, when the node is given its
//! document; painting never touches `usvg`.
//!
//! What is drawn: shapes, fills and strokes (caps, joins, miter limit,
//! dashes), solid colours, linear and radial gradients (with their spread
//! method), patterns, group opacity, clip paths (nested ones too), text (as
//! outlines, from the engine's own fonts), nested SVG `<image>`s, and a
//! filter that is a single Gaussian blur. What is not, and is dropped without
//! an error: raster `<image>`s (the engine decodes no image formats),
//! masks (the renderer cannot draw them yet), every other filter, and blend
//! modes.

use peniko::kurbo::{Affine, BezPath, Cap, Join, Rect, Shape, Stroke};
use peniko::{Color, Extend, Gradient};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// A parsed SVG document, ready to paint.
#[derive(Debug)]
pub struct SvgDocument {
    /// The document's own size (`width` x `height`, or its `viewBox`'s),
    /// which fits into the node's box.
    pub width: f64,
    pub height: f64,
    pub root: SvgGroup,
    /// Unique per parse: the damage tracker's stand-in for hashing the whole
    /// scene.
    pub revision: u64,
}

#[derive(Debug)]
pub struct SvgGroup {
    pub transform: Affine,
    pub opacity: f32,
    pub clip: Option<SvgClip>,
    /// A Gaussian blur's standard deviation, in the group's own units: the
    /// one filter drawn.
    pub blur: Option<f32>,
    pub children: Vec<SvgNode>,
}

#[derive(Debug)]
pub enum SvgNode {
    Group(SvgGroup),
    Path(Box<SvgPath>),
}

/// A clip path, already flattened to one outline in the clipped group's
/// coordinates.
#[derive(Debug)]
pub struct SvgClip {
    pub path: BezPath,
    pub even_odd: bool,
    /// The clip this one is itself clipped by.
    pub parent: Option<Box<SvgClip>>,
}

/// A tiled paint: `root` is drawn in tiles of `rect`'s size, the first at
/// `rect`'s origin, all under `transform`.
#[derive(Debug)]
pub struct SvgPattern {
    pub rect: Rect,
    pub transform: Affine,
    pub root: SvgGroup,
}

#[derive(Debug)]
pub struct SvgPath {
    pub path: BezPath,
    pub fill: Option<SvgFill>,
    pub stroke: Option<SvgStroke>,
    /// SVG's `paint-order: stroke`: the stroke goes under the fill.
    pub stroke_first: bool,
}

#[derive(Debug)]
pub struct SvgFill {
    pub paint: SvgPaint,
    pub even_odd: bool,
}

#[derive(Debug)]
pub struct SvgStroke {
    pub paint: SvgPaint,
    pub stroke: Stroke,
}

/// A paint, with the object's opacity already folded into its alpha.
#[derive(Debug, Clone)]
pub enum SvgPaint {
    Color(Color),
    /// A gradient and the transform it is painted under.
    Gradient(Gradient, Affine),
    Pattern(Arc<SvgPattern>),
}

fn affine(t: usvg::Transform) -> Affine {
    Affine::new([
        f64::from(t.sx),
        f64::from(t.ky),
        f64::from(t.kx),
        f64::from(t.sy),
        f64::from(t.tx),
        f64::from(t.ty),
    ])
}

fn bez_path(path: &usvg::tiny_skia_path::Path) -> BezPath {
    use usvg::tiny_skia_path::PathSegment;
    let p = |p: usvg::tiny_skia_path::Point| (f64::from(p.x), f64::from(p.y));
    let mut out = BezPath::new();
    for segment in path.segments() {
        match segment {
            PathSegment::MoveTo(a) => out.move_to(p(a)),
            PathSegment::LineTo(a) => out.line_to(p(a)),
            PathSegment::QuadTo(a, b) => out.quad_to(p(a), p(b)),
            PathSegment::CubicTo(a, b, c) => out.curve_to(p(a), p(b), p(c)),
            PathSegment::Close => out.close_path(),
        }
    }
    out
}

fn color(c: usvg::Color, opacity: f32) -> Color {
    Color::from_rgba8(
        c.red,
        c.green,
        c.blue,
        (opacity.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

fn stops(base: &[usvg::Stop], opacity: f32) -> Vec<(f32, Color)> {
    base.iter()
        .map(|s| {
            (
                s.offset().get(),
                color(s.color(), s.opacity().get() * opacity),
            )
        })
        .collect()
}

fn extend(method: usvg::SpreadMethod) -> Extend {
    match method {
        usvg::SpreadMethod::Pad => Extend::Pad,
        usvg::SpreadMethod::Reflect => Extend::Reflect,
        usvg::SpreadMethod::Repeat => Extend::Repeat,
    }
}

/// `None` for a pattern, which is not drawn.
fn paint(paint: &usvg::Paint, opacity: f32) -> Option<SvgPaint> {
    match paint {
        usvg::Paint::Color(c) => Some(SvgPaint::Color(color(*c, opacity))),
        usvg::Paint::LinearGradient(g) => {
            let gradient = Gradient::new_linear(
                (f64::from(g.x1()), f64::from(g.y1())),
                (f64::from(g.x2()), f64::from(g.y2())),
            )
            .with_extend(extend(g.spread_method()))
            .with_stops(stops(g.stops(), opacity).as_slice());
            Some(SvgPaint::Gradient(gradient, affine(g.transform())))
        }
        usvg::Paint::RadialGradient(g) => {
            let gradient = Gradient::new_two_point_radial(
                (f64::from(g.fx()), f64::from(g.fy())),
                g.fr().get(),
                (f64::from(g.cx()), f64::from(g.cy())),
                g.r().get(),
            )
            .with_extend(extend(g.spread_method()))
            .with_stops(stops(g.stops(), opacity).as_slice());
            Some(SvgPaint::Gradient(gradient, affine(g.transform())))
        }
        usvg::Paint::Pattern(p) => {
            let mut root = convert_group(p.root());
            root.opacity *= opacity;
            let r = p.rect();
            Some(SvgPaint::Pattern(Arc::new(SvgPattern {
                rect: Rect::new(
                    f64::from(r.x()),
                    f64::from(r.y()),
                    f64::from(r.x() + r.width()),
                    f64::from(r.y() + r.height()),
                ),
                transform: affine(p.transform()),
                root,
            })))
        }
    }
}

fn convert_path(path: &usvg::Path) -> Option<SvgPath> {
    if !path.is_visible() {
        return None;
    }
    let fill = path.fill().and_then(|f| {
        Some(SvgFill {
            paint: paint(f.paint(), f.opacity().get())?,
            even_odd: matches!(f.rule(), usvg::FillRule::EvenOdd),
        })
    });
    let stroke = path.stroke().and_then(|s| {
        let mut stroke = Stroke::new(f64::from(s.width().get()))
            .with_caps(match s.linecap() {
                usvg::LineCap::Butt => Cap::Butt,
                usvg::LineCap::Round => Cap::Round,
                usvg::LineCap::Square => Cap::Square,
            })
            .with_join(match s.linejoin() {
                usvg::LineJoin::Miter | usvg::LineJoin::MiterClip => Join::Miter,
                usvg::LineJoin::Round => Join::Round,
                usvg::LineJoin::Bevel => Join::Bevel,
            })
            .with_miter_limit(f64::from(s.miterlimit().get()));
        if let Some(dashes) = s.dasharray() {
            stroke = stroke.with_dashes(
                f64::from(s.dashoffset()),
                dashes.iter().map(|d| f64::from(*d)),
            );
        }
        Some(SvgStroke {
            paint: paint(s.paint(), s.opacity().get())?,
            stroke,
        })
    });
    if fill.is_none() && stroke.is_none() {
        return None;
    }
    Some(SvgPath {
        path: bez_path(path.data()),
        fill,
        stroke,
        stroke_first: matches!(path.paint_order(), usvg::PaintOrder::StrokeAndFill),
    })
}

/// Every shape of a clip path in one outline, in the clipped group's
/// coordinates. Shapes of different fill rules take non-zero.
fn convert_clip(clip: &usvg::ClipPath) -> SvgClip {
    fn collect(group: &usvg::Group, to: Affine, out: &mut BezPath, rules: &mut Vec<bool>) {
        for node in group.children() {
            match node {
                usvg::Node::Path(p) if p.is_visible() => {
                    let outline = affine_path(to, &bez_path(p.data()));
                    out.extend(outline);
                    rules.push(matches!(
                        p.fill().map(usvg::Fill::rule),
                        Some(usvg::FillRule::EvenOdd)
                    ));
                }
                usvg::Node::Group(g) => collect(g, to * affine(g.transform()), out, rules),
                _ => {}
            }
        }
    }
    let mut path = BezPath::new();
    let mut rules = Vec::new();
    collect(clip.root(), affine(clip.transform()), &mut path, &mut rules);
    let even_odd = !rules.is_empty() && rules.iter().all(|r| *r);
    SvgClip {
        path,
        even_odd,
        parent: clip.clip_path().map(|c| Box::new(convert_clip(c))),
    }
}

fn affine_path(to: Affine, path: &BezPath) -> BezPath {
    to * path.clone()
}

/// The one filter drawn: a single Gaussian blur, equal in x and y or averaged.
fn blur_of(filters: &[Arc<usvg::filter::Filter>]) -> Option<f32> {
    let [filter] = filters else { return None };
    let [primitive] = filter.primitives() else {
        return None;
    };
    match primitive.kind() {
        usvg::filter::Kind::GaussianBlur(b) => {
            let sigma = (b.std_dev_x().get() + b.std_dev_y().get()) / 2.0;
            (sigma > 0.0).then_some(sigma)
        }
        _ => None,
    }
}

/// A nested SVG `<image>`, drawn scaled into the image's own box and clipped
/// to it. A raster one is not drawn.
fn convert_image(image: &usvg::Image) -> Option<SvgGroup> {
    let usvg::ImageKind::SVG(tree) = image.kind() else {
        return None;
    };
    if !image.is_visible() || tree.size().width() <= 0.0 || tree.size().height() <= 0.0 {
        return None;
    }
    let (w, h) = (
        f64::from(image.size().width()),
        f64::from(image.size().height()),
    );
    let mut clip = BezPath::new();
    clip.extend(Rect::new(0.0, 0.0, w, h).path_elements(0.1));
    let mut inner = convert_group(tree.root());
    inner.transform = Affine::scale_non_uniform(
        w / f64::from(tree.size().width()),
        h / f64::from(tree.size().height()),
    ) * inner.transform;
    Some(SvgGroup {
        transform: Affine::IDENTITY,
        opacity: 1.0,
        clip: Some(SvgClip {
            path: clip,
            even_odd: false,
            parent: None,
        }),
        blur: None,
        children: vec![SvgNode::Group(inner)],
    })
}

fn convert_group(group: &usvg::Group) -> SvgGroup {
    let mut children = Vec::new();
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => children.push(SvgNode::Group(convert_group(g))),
            usvg::Node::Path(p) => {
                if let Some(p) = convert_path(p) {
                    children.push(SvgNode::Path(Box::new(p)));
                }
            }
            // Text is already outlines, in a group of paths.
            usvg::Node::Text(t) => children.push(SvgNode::Group(convert_group(t.flattened()))),
            usvg::Node::Image(image) => {
                if let Some(g) = convert_image(image) {
                    children.push(SvgNode::Group(g));
                }
            }
        }
    }
    SvgGroup {
        transform: affine(group.transform()),
        opacity: group.opacity().get(),
        clip: group.clip_path().map(convert_clip),
        blur: blur_of(group.filters()),
        children,
    }
}

static REVISION: AtomicU64 = AtomicU64::new(1);

impl SvgDocument {
    /// Parses an SVG (or gzip-compressed SVGZ) document, shaping its text
    /// with `fonts`. The error is `usvg`'s own message.
    pub fn parse(data: &[u8], fonts: &SvgFonts) -> Result<Arc<Self>, String> {
        let options = usvg::Options {
            fontdb: fonts.0.clone(),
            font_family: "Roboto".to_string(),
            ..usvg::Options::default()
        };
        let tree = usvg::Tree::from_data(data, &options).map_err(|e| e.to_string())?;
        let size = tree.size();
        Ok(Arc::new(Self {
            width: f64::from(size.width()),
            height: f64::from(size.height()),
            root: convert_group(tree.root()),
            revision: REVISION.fetch_add(1, Ordering::Relaxed),
        }))
    }

    /// A document that draws nothing: what a node has until it is given one.
    pub fn empty() -> Arc<Self> {
        Arc::new(Self {
            width: 1.0,
            height: 1.0,
            root: SvgGroup {
                transform: Affine::IDENTITY,
                opacity: 1.0,
                clip: None,
                blur: None,
                children: Vec::new(),
            },
            revision: REVISION.fetch_add(1, Ordering::Relaxed),
        })
    }

    /// Uniformly scales and centres the document into a `w` x `h` box
    /// (SVG's default `xMidYMid meet`).
    pub fn fit(&self, w: f64, h: f64) -> Affine {
        if self.width <= 0.0 || self.height <= 0.0 {
            return Affine::IDENTITY;
        }
        let scale = (w / self.width).min(h / self.height);
        Affine::new([
            scale,
            0.0,
            0.0,
            scale,
            (w - self.width * scale) / 2.0,
            (h - self.height * scale) / 2.0,
        ])
    }
}

/// The font files an SVG's text is shaped with, loaded once and shared by
/// every parse. Roboto is the default and sans-serif/serif family, the
/// bundled mono face is the monospace one.
#[derive(Clone)]
pub struct SvgFonts(Arc<usvg::fontdb::Database>);

impl SvgFonts {
    pub fn new(files: &[Arc<Vec<u8>>]) -> Self {
        let mut db = usvg::fontdb::Database::new();
        for file in files {
            db.load_font_data(file.to_vec());
        }
        db.set_sans_serif_family("Roboto");
        db.set_serif_family("Roboto");
        db.set_monospace_family("Hack Nerd Font Mono");
        Self(Arc::new(db))
    }

    /// No fonts: text in a document draws nothing.
    pub fn none() -> Self {
        Self(Arc::new(usvg::fontdb::Database::new()))
    }
}

/// `NodeKind::Svg`'s own state.
#[derive(Debug, Clone)]
pub struct SvgState {
    pub document: Arc<SvgDocument>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED_SQUARE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10" viewBox="0 0 20 10">
        <rect x="0" y="0" width="10" height="10" fill="#ff0000"/>
        <circle cx="15" cy="5" r="4" fill="none" stroke="#0000ff" stroke-width="2" stroke-dasharray="2 1"/>
    </svg>"##;

    #[test]
    fn parses_size_shapes_and_strokes() {
        let doc = SvgDocument::parse(RED_SQUARE.as_bytes(), &SvgFonts::none()).unwrap();
        assert_eq!((doc.width, doc.height), (20.0, 10.0));
        assert_eq!(doc.root.children.len(), 2);
        let SvgNode::Path(rect) = &doc.root.children[0] else {
            panic!("a path")
        };
        assert!(matches!(
            rect.fill.as_ref().unwrap().paint,
            SvgPaint::Color(c) if c == Color::from_rgba8(255, 0, 0, 255)
        ));
        let SvgNode::Path(circle) = &doc.root.children[1] else {
            panic!("a path")
        };
        assert!(circle.fill.is_none());
        assert_eq!(circle.stroke.as_ref().unwrap().stroke.dash_pattern.len(), 2);
    }

    #[test]
    fn gradients_and_clips_survive() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">
            <defs>
              <linearGradient id="g"><stop offset="0" stop-color="#000"/><stop offset="1" stop-color="#fff"/></linearGradient>
              <clipPath id="c"><rect width="5" height="5"/></clipPath>
            </defs>
            <g clip-path="url(#c)" opacity="0.5"><rect width="10" height="10" fill="url(#g)"/></g>
        </svg>"##;
        let doc = SvgDocument::parse(svg.as_bytes(), &SvgFonts::none()).unwrap();
        let SvgNode::Group(g) = &doc.root.children[0] else {
            panic!("a group")
        };
        assert!((g.opacity - 0.5).abs() < 1e-6);
        assert!(g.clip.is_some());
        let SvgNode::Path(p) = &g.children[0] else {
            panic!("a path")
        };
        assert!(matches!(
            p.fill.as_ref().unwrap().paint,
            SvgPaint::Gradient(..)
        ));
    }

    #[test]
    fn garbage_is_an_error_and_fit_centres() {
        assert!(SvgDocument::parse(b"not svg", &SvgFonts::none()).is_err());
        let doc = SvgDocument::parse(RED_SQUARE.as_bytes(), &SvgFonts::none()).unwrap();
        // 20x10 into 40x40: scale 2, centred vertically.
        let c = doc.fit(40.0, 40.0).as_coeffs();
        assert_eq!((c[0], c[4], c[5]), (2.0, 0.0, 10.0));
    }
}
