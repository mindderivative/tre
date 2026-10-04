//! M96: the properties of `node.set`/`node.get` that belong to one kind of
//! node -- text, text input, image, scroll view, and terminal. Like
//! `node_layout`, each property is one parser (checked against the node's
//! kind before anything is applied) and one reader.

use std::ops::Range;

use engine_core::{Animated, ContentFit, NodeKind, TextAlign};
use peniko::Color;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::node::validate_rgba_frame_len;
use crate::node_layout::lookup;
use crate::node_props::{color_to_py, parse_color};

/// Every kind-specific property this module handles, in error order.
pub(crate) const KIND_PROPS: [&str; 32] = [
    "text",
    "font_family",
    "font_weight",
    "font_size",
    "line_height",
    "text_align",
    "font_style",
    "letter_spacing",
    "wrap",
    "max_lines",
    "overflow",
    "spans",
    "selectable",
    "multiline",
    "selection",
    "show_whitespace",
    "syntax_spans",
    "folded_ranges",
    "rgba",
    "pixel_width",
    "pixel_height",
    "fit",
    "orientation",
    "scroll_offset",
    "cols",
    "rows",
    "item_count",
    "item_extent",
    "svg",
    "svg_size",
    "svg_color",
    "svg_images",
];

const TEXT_ALIGN: [(&str, TextAlign); 3] = [
    ("start", TextAlign::Start),
    ("center", TextAlign::Center),
    ("end", TextAlign::End),
];

pub(crate) const FIT: [(&str, ContentFit); 3] = [
    ("cover", ContentFit::Cover),
    ("contain", ContentFit::Contain),
    ("fill", ContentFit::Fill),
];

/// `true` for italic.
pub(crate) const FONT_STYLE: [(&str, bool); 2] = [("normal", false), ("italic", true)];

/// `true` wraps.
pub(crate) const WRAP: [(&str, bool); 2] = [("word", true), ("none", false)];

/// `true` for an ellipsis.
pub(crate) const OVERFLOW: [(&str, bool); 2] = [("clip", false), ("ellipsis", true)];

/// `true` for horizontal.
const ORIENTATION: [(&str, bool); 2] = [("horizontal", true), ("vertical", false)];

/// A parsed, validated write to one node.
pub(crate) struct KindChange {
    pub(crate) edit: Box<dyn FnOnce(&mut engine_core::Node)>,
    /// Applied after every other change in the same `set` -- a
    /// `selection` given with `text` selects in the new text.
    pub(crate) late: bool,
    /// Changes a terminal's grid or font, so its box follows.
    pub(crate) resizes_terminal: bool,
    /// Changes a virtual list's rows, so every built one is released and
    /// rebuilt at the next layout.
    pub(crate) resets_rows: bool,
    /// A new SVG document's width over height: the node's `aspect_ratio`
    /// follows it unless the app set its own.
    pub(crate) svg_ratio: Option<f64>,
}

fn change(edit: impl FnOnce(&mut engine_core::Node) + 'static) -> PyResult<KindChange> {
    Ok(KindChange {
        edit: Box::new(edit),
        late: false,
        resizes_terminal: false,
        resets_rows: false,
        svg_ratio: None,
    })
}

fn parse_svg_color(value: &Bound<'_, PyAny>) -> PyResult<Option<Color>> {
    if value.is_none() {
        return Ok(None);
    }
    parse_color(value, "svg_color").map(Some)
}

fn parse_svg_images(value: &Bound<'_, PyAny>) -> PyResult<engine_core::SvgImages> {
    let expected = "a dict of href -> (rgba bytes, width, height)";
    let dict = value
        .cast::<PyDict>()
        .map_err(|_| invalid("svg_images", expected))?;
    let mut images = Vec::new();
    for (href, entry) in dict.iter() {
        let href: String = href
            .extract()
            .map_err(|_| invalid("svg_images", "keyed by str hrefs"))?;
        let (rgba, width, height): (Vec<u8>, u32, u32) = entry
            .extract()
            .map_err(|_| invalid("svg_images", expected))?;
        validate_rgba_frame_len(
            &format!("node property `svg_images` entry {href:?}"),
            rgba.len(),
            width,
            height,
        )?;
        images.push((
            href,
            std::sync::Arc::new(engine_core::SvgBitmap::new(rgba, width, height)),
        ));
    }
    Ok(engine_core::SvgImages(std::sync::Arc::new(images)))
}

/// `svg`, `svg_color` and `svg_images`: whichever comes first in the call
/// parses the document with the new value of each one given, and the node's
/// current value of each one not.
fn svg_properties(name: &str, kind: &NodeKind, props: &Bound<'_, PyDict>) -> PyResult<KindChange> {
    let owner = ["svg", "svg_color", "svg_images"]
        .into_iter()
        .find(|n| props.contains(*n).unwrap_or(false));
    if owner != Some(name) {
        return change(|_| {});
    }
    let current = match kind {
        NodeKind::Svg(state) => Some(state),
        _ => None,
    };
    let (source, is_text) = match props.get_item("svg")? {
        Some(value) => match value.extract::<String>() {
            Ok(text) => (text.into_bytes(), true),
            Err(_) => (
                value
                    .extract::<Vec<u8>>()
                    .map_err(|_| invalid("svg", "an SVG document as a str or bytes"))?,
                false,
            ),
        },
        None => match current {
            Some(state) => (state.source.to_vec(), state.source_is_text),
            None => return change(|_| {}),
        },
    };
    let color = match props.get_item("svg_color")? {
        Some(v) => parse_svg_color(&v)?,
        None => current.and_then(|state| state.color),
    };
    let images = match props.get_item("svg_images")? {
        Some(v) => parse_svg_images(&v)?,
        None => current
            .map(|state| state.images.clone())
            .unwrap_or_default(),
    };
    svg_change(source, is_text, color, images)
}

/// Parses `source` (raising for a bad document) into a change that installs
/// it, and the aspect ratio the node should follow.
fn svg_change(
    source: Vec<u8>,
    is_text: bool,
    color: Option<Color>,
    images: engine_core::SvgImages,
) -> PyResult<KindChange> {
    let document = engine_core::SvgDocument::parse_full(&source, &svg_fonts(), color, &images)
        .map_err(|reason| {
            PyValueError::new_err(format!("node property `svg` isn't a valid SVG: {reason}"))
        })?;
    let ratio = document.width / document.height;
    let mut kind_change = change(move |node| {
        if let NodeKind::Svg(state) = &mut node.kind {
            *state = engine_core::SvgState {
                document,
                source: std::sync::Arc::new(source),
                source_is_text: is_text,
                color,
                images,
            };
        }
    })?;
    kind_change.svg_ratio = Some(ratio);
    Ok(kind_change)
}

/// An SVG's text is outlined when its document is parsed, so when the fonts
/// have changed since `seen` (a font registered, system fonts turned on or
/// off) every document with text is parsed again. One atomic load when
/// nothing changed.
pub(crate) fn refresh_svg_text(
    tree: &std::cell::RefCell<engine_core::Tree>,
    seen: &std::cell::Cell<u64>,
) {
    let generation = engine_render::font_generation();
    if generation == seen.get() {
        return;
    }
    seen.set(generation);
    let fonts = svg_fonts();
    tree.borrow_mut().reparse_svgs(|state| {
        engine_core::SvgDocument::parse_full(&state.source, &fonts, state.color, &state.images).ok()
    });
}

/// The fonts an SVG's text is shaped with: the engine's own, rebuilt only
/// when a font is registered.
pub(crate) fn svg_fonts() -> engine_core::SvgFonts {
    use std::sync::Mutex;
    static CACHE: Mutex<Option<(u64, engine_core::SvgFonts)>> = Mutex::new(None);
    let mut cache = CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let generation = engine_render::font_generation();
    if let Some((cached, fonts)) = cache.as_ref()
        && *cached == generation
    {
        return fonts.clone();
    }
    let (generation, files) = engine_render::all_fonts();
    let fonts = engine_core::SvgFonts::new(&files, engine_render::system_fonts());
    *cache = Some((generation, fonts.clone()));
    fonts
}

fn invalid(name: &str, expected: &str) -> PyErr {
    PyValueError::new_err(format!("node property `{name}` must be {expected}"))
}

fn not_bool(value: &Bound<'_, PyAny>, name: &str, expected: &str) -> PyResult<()> {
    if value.is_instance_of::<pyo3::types::PyBool>() {
        return Err(invalid(name, expected));
    }
    Ok(())
}

fn string(value: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    value.extract().map_err(|_| invalid(name, "a str"))
}

fn positive(value: &Bound<'_, PyAny>, name: &str) -> PyResult<f32> {
    not_bool(value, name, "a positive number")?;
    let number: f32 = value
        .extract()
        .map_err(|_| invalid(name, "a positive number"))?;
    if number <= 0.0 || !number.is_finite() {
        return Err(invalid(name, "a positive number"));
    }
    Ok(number)
}

fn boolean(value: &Bound<'_, PyAny>, name: &str) -> PyResult<bool> {
    if !value.is_instance_of::<pyo3::types::PyBool>() {
        return Err(invalid(name, "a bool"));
    }
    value.extract()
}

fn keyword<T: Copy>(table: &[(&str, T)], value: &Bound<'_, PyAny>, name: &str) -> PyResult<T> {
    let text = value.extract::<String>().unwrap_or_default();
    lookup(table, &text).map_err(|expected| invalid(name, &expected))
}

fn name_of<T: PartialEq>(table: &[(&'static str, T)], value: &T) -> &'static str {
    table
        .iter()
        .find(|(_, v)| v == value)
        .map_or("?", |(n, _)| *n)
}

/// A byte range of `text`, on character boundaries.
fn byte_range(text: &str, start: usize, end: usize, name: &str) -> PyResult<Range<usize>> {
    if start > end
        || end > text.len()
        || !text.is_char_boundary(start)
        || !text.is_char_boundary(end)
    {
        return Err(invalid(
            name,
            &format!(
                "byte offsets with start <= end <= {} on character boundaries",
                text.len()
            ),
        ));
    }
    Ok(start..end)
}

/// Which kinds `name` applies to, as words and as a test.
fn applies(name: &str) -> (&'static str, fn(&NodeKind) -> bool) {
    fn text(kind: &NodeKind) -> bool {
        matches!(kind, NodeKind::Text(_))
    }
    fn text_like(kind: &NodeKind) -> bool {
        matches!(kind, NodeKind::Text(_) | NodeKind::TextField(_))
    }
    fn text_input(kind: &NodeKind) -> bool {
        matches!(kind, NodeKind::TextField(_))
    }
    fn sized_text(kind: &NodeKind) -> bool {
        matches!(
            kind,
            NodeKind::Text(_) | NodeKind::TextField(_) | NodeKind::Terminal(_)
        )
    }
    fn selectable(kind: &NodeKind) -> bool {
        matches!(
            kind,
            NodeKind::TextField(_) | NodeKind::Terminal(_) | NodeKind::Text(_)
        )
    }
    fn image(kind: &NodeKind) -> bool {
        matches!(kind, NodeKind::Image(_))
    }
    fn svg(kind: &NodeKind) -> bool {
        matches!(kind, NodeKind::Svg(_))
    }
    fn scroll_view(kind: &NodeKind) -> bool {
        matches!(kind, NodeKind::ScrollView(_))
    }
    fn terminal(kind: &NodeKind) -> bool {
        matches!(kind, NodeKind::Terminal(_))
    }
    fn virtual_list(kind: &NodeKind) -> bool {
        matches!(kind, NodeKind::VirtualList(_))
    }
    match name {
        "text" | "font_family" | "font_weight" => ("a text or text_input", text_like),
        "font_size" => ("a text, text_input, or terminal", sized_text),
        "line_height" | "text_align" | "font_style" | "letter_spacing" | "wrap" | "max_lines"
        | "overflow" | "spans" | "selectable" => ("a text", text),
        "multiline" | "show_whitespace" | "syntax_spans" | "folded_ranges" => {
            ("a text_input", text_input)
        }
        "selection" => ("a text, text_input or terminal", selectable),
        "rgba" | "pixel_width" | "pixel_height" | "fit" => ("an image", image),
        "svg" | "svg_size" | "svg_color" | "svg_images" => ("an svg", svg),
        "orientation" | "scroll_offset" => ("a scroll_view", scroll_view),
        "item_count" | "item_extent" => ("a virtual_list", virtual_list),
        _ => ("a terminal", terminal),
    }
}

/// Parses kind property `name` for a node of `kind`, or `None` if it isn't
/// one. `props` is the whole `set` call, for properties set together.
pub(crate) fn parse_kind_prop(
    name: &str,
    value: &Bound<'_, PyAny>,
    kind: &NodeKind,
    props: &Bound<'_, PyDict>,
) -> Option<PyResult<KindChange>> {
    if !KIND_PROPS.contains(&name) {
        return None;
    }
    let (kinds, test) = applies(name);
    if !test(kind) {
        return Some(Err(PyValueError::new_err(format!(
            "node property `{name}` applies only to {kinds} node"
        ))));
    }
    Some(parse_known(name, value, kind, props))
}

fn parse_known(
    name: &str,
    value: &Bound<'_, PyAny>,
    kind: &NodeKind,
    props: &Bound<'_, PyDict>,
) -> PyResult<KindChange> {
    match name {
        "svg_size" => Err(PyValueError::new_err(
            "node property `svg_size` is read-only",
        )),
        // 0.5.4 (#141, #144, #147): the document, what `currentColor` is, and
        // the decoded images it refers to. They parse together, once, so the
        // first of them in a call does it all with the others' new values.
        "svg" | "svg_color" | "svg_images" => svg_properties(name, kind, props),
        "text" => {
            let text = string(value, name)?;
            change(move |node| match &mut node.kind {
                NodeKind::Text(state) => state.content = text,
                NodeKind::TextField(state) => {
                    state.cursor = text.len();
                    state.selection_anchor = None;
                    state.content = text;
                }
                _ => {}
            })
        }
        "font_family" => {
            let family = string(value, name)?;
            if family.is_empty() {
                return Err(invalid(name, "a non-empty str"));
            }
            change(move |node| match &mut node.kind {
                NodeKind::Text(state) => state.font_family = family,
                NodeKind::TextField(state) => state.font_family = family,
                _ => {}
            })
        }
        "font_weight" => {
            let expected = "a number from 1 to 1000";
            not_bool(value, name, expected)?;
            let weight: f32 = value.extract().map_err(|_| invalid(name, expected))?;
            if !(1.0..=1000.0).contains(&weight) {
                return Err(invalid(name, expected));
            }
            change(move |node| match &mut node.kind {
                NodeKind::Text(state) => state.font_weight = weight,
                NodeKind::TextField(state) => state.font_weight = weight,
                _ => {}
            })
        }
        "font_size" => {
            let size = positive(value, name)?;
            let mut resize = change(move |node| match &mut node.kind {
                NodeKind::Text(state) => state.font_size = size,
                NodeKind::TextField(state) => state.font_size = size,
                NodeKind::Terminal(state) => state.font_size = size,
                _ => {}
            })?;
            resize.resizes_terminal = matches!(kind, NodeKind::Terminal(_));
            Ok(resize)
        }
        "line_height" => {
            let ratio = if value.is_none() {
                None
            } else {
                Some(positive(value, name)?)
            };
            change(move |node| {
                if let NodeKind::Text(state) = &mut node.kind {
                    state.line_height = ratio;
                }
            })
        }
        "text_align" => {
            let align = keyword(&TEXT_ALIGN, value, name)?;
            change(move |node| {
                if let NodeKind::Text(state) = &mut node.kind {
                    state.align = align;
                }
            })
        }
        "font_style" | "wrap" | "overflow" => {
            let table = match name {
                "font_style" => &FONT_STYLE,
                "wrap" => &WRAP,
                _ => &OVERFLOW,
            };
            let on = keyword(table, value, name)?;
            let field = name.to_string();
            change(move |node| {
                if let NodeKind::Text(state) = &mut node.kind {
                    match field.as_str() {
                        "font_style" => state.options.italic = on,
                        "wrap" => state.options.wrap = on,
                        _ => state.options.ellipsis = on,
                    }
                }
            })
        }
        "letter_spacing" => {
            not_bool(value, name, "a number")?;
            let spacing: f32 = value
                .extract()
                .ok()
                .filter(|v: &f32| v.is_finite())
                .ok_or_else(|| invalid(name, "a number"))?;
            change(move |node| {
                if let NodeKind::Text(state) = &mut node.kind {
                    state.options.letter_spacing = spacing;
                }
            })
        }
        "max_lines" => {
            let lines = if value.is_none() {
                None
            } else {
                not_bool(value, name, "a positive int or None")?;
                Some(
                    value
                        .extract::<usize>()
                        .ok()
                        .filter(|n| *n > 0)
                        .ok_or_else(|| invalid(name, "a positive int or None"))?,
                )
            };
            change(move |node| {
                if let NodeKind::Text(state) = &mut node.kind {
                    state.options.max_lines = lines;
                }
            })
        }
        "multiline" | "show_whitespace" => {
            let on = boolean(value, name)?;
            let multiline = name == "multiline";
            change(move |node| {
                if let NodeKind::TextField(state) = &mut node.kind {
                    if multiline {
                        state.multiline = on;
                    } else {
                        state.show_whitespace = on;
                    }
                }
            })
        }
        "selection" => {
            let mut selection = if let NodeKind::Terminal(state) = kind {
                parse_terminal_selection(value, name, state.cols, state.rows)?
            } else if matches!(kind, NodeKind::Text(_)) {
                // 0.5.4 (#112): a static text's selection, or `None` for none.
                let selection = if value.is_none() {
                    None
                } else {
                    let text = match props.get_item("text")? {
                        Some(text) => string(&text, "text")?,
                        None => match kind {
                            NodeKind::Text(state) => state.content.clone(),
                            _ => String::new(),
                        },
                    };
                    let (start, end): (usize, usize) = value.extract().map_err(|_| {
                        invalid(name, "a (start, end) tuple of byte offsets, or None")
                    })?;
                    let range = byte_range(&text, start, end, name)?;
                    Some((range.start, range.end))
                };
                change(move |node| {
                    if let NodeKind::Text(state) = &mut node.kind {
                        state.options.selection = selection;
                    }
                })?
            } else {
                // Checked against the text this same call sets, if any.
                let text = match props.get_item("text")? {
                    Some(text) => string(&text, "text")?,
                    None => match kind {
                        NodeKind::TextField(state) => state.content.clone(),
                        _ => String::new(),
                    },
                };
                let expected = "a (start, end) tuple of byte offsets";
                let (start, end): (usize, usize) =
                    value.extract().map_err(|_| invalid(name, expected))?;
                let range = byte_range(&text, start, end, name)?;
                change(move |node| {
                    if let NodeKind::TextField(state) = &mut node.kind {
                        state.cursor = range.end;
                        state.selection_anchor = (range.start != range.end).then_some(range.start);
                    }
                })?
            };
            selection.late = true;
            Ok(selection)
        }
        "selectable" => {
            let on = boolean(value, name)?;
            change(move |node| {
                if let NodeKind::Text(state) = &mut node.kind {
                    state.options.selectable = on;
                    if !on {
                        state.options.selection = None;
                    }
                }
            })
        }
        "spans" => {
            let expected = "a list of (start, end, style) tuples, each style a dict with any of \
                            color, weight, italic, underline, strikethrough, font_size, \
                            font_family, link";
            let items: Vec<Bound<'_, PyAny>> =
                value.extract().map_err(|_| invalid(name, expected))?;
            let mut spans = Vec::with_capacity(items.len());
            for item in &items {
                let (start, end, style): (usize, usize, Bound<'_, pyo3::types::PyDict>) =
                    item.extract().map_err(|_| invalid(name, expected))?;
                if start > end {
                    return Err(invalid(name, expected));
                }
                let mut span = engine_core::TextSpan {
                    start,
                    end,
                    ..Default::default()
                };
                for (key, v) in style.iter() {
                    let key: String = key.extract().map_err(|_| invalid(name, expected))?;
                    match key.as_str() {
                        "color" => span.color = Some(parse_color(&v, name)?),
                        "weight" => {
                            not_bool(&v, name, "a weight from 100 to 950")?;
                            span.weight = Some(
                                v.extract::<f32>()
                                    .ok()
                                    .filter(|w| (1.0..=1000.0).contains(w))
                                    .ok_or_else(|| invalid(name, "a weight from 1 to 1000"))?,
                            );
                        }
                        "italic" => span.italic = Some(boolean(&v, name)?),
                        "underline" => span.underline = boolean(&v, name)?,
                        "strikethrough" => span.strikethrough = boolean(&v, name)?,
                        "font_size" => {
                            not_bool(&v, name, "a font size above 0")?;
                            span.font_size = Some(
                                v.extract::<f32>()
                                    .ok()
                                    .filter(|s| s.is_finite() && *s > 0.0)
                                    .ok_or_else(|| invalid(name, "a font size above 0"))?,
                            );
                        }
                        "font_family" => {
                            span.font_family = Some(
                                v.extract::<String>()
                                    .map_err(|_| invalid(name, "a font family name (a str)"))?,
                            );
                        }
                        "link" => {
                            span.link = Some(
                                v.extract::<String>()
                                    .map_err(|_| invalid(name, "a link target (a str)"))?,
                            );
                        }
                        other => {
                            return Err(invalid(
                                name,
                                &format!(
                                    "spans styled with color, weight, italic, underline, \
                                     strikethrough, font_size, font_family or link, not {other:?}"
                                ),
                            ));
                        }
                    }
                }
                spans.push(span);
            }
            change(move |node| {
                if let NodeKind::Text(state) = &mut node.kind {
                    state.options.spans = spans;
                }
            })
        }
        "syntax_spans" => {
            let expected = "a list of (start, end, color) tuples";
            let items: Vec<Bound<'_, PyAny>> =
                value.extract().map_err(|_| invalid(name, expected))?;
            let mut spans: Vec<(Range<usize>, Color)> = Vec::with_capacity(items.len());
            for item in &items {
                let (start, end, color): (usize, usize, Bound<'_, PyAny>) =
                    item.extract().map_err(|_| invalid(name, expected))?;
                if start > end {
                    return Err(invalid(name, expected));
                }
                spans.push((start..end, parse_color(&color, name)?));
            }
            change(move |node| {
                if let NodeKind::TextField(state) = &mut node.kind {
                    state.syntax_spans = spans;
                }
            })
        }
        "folded_ranges" => {
            let expected = "a list of (start, end) tuples";
            let ranges: Vec<(usize, usize)> =
                value.extract().map_err(|_| invalid(name, expected))?;
            if ranges.iter().any(|(start, end)| start > end) {
                return Err(invalid(name, expected));
            }
            change(move |node| {
                if let NodeKind::TextField(state) = &mut node.kind {
                    state.folded_ranges = ranges.into_iter().map(|(s, e)| s..e).collect();
                }
            })
        }
        "rgba" => {
            let together = "given with `pixel_width` and `pixel_height`";
            let rgba: Vec<u8> = value
                .extract()
                .map_err(|_| invalid(name, "bytes of RGBA8 pixels"))?;
            let dimension = |key: &str| -> PyResult<u32> {
                let dim = props
                    .get_item(key)?
                    .ok_or_else(|| invalid(name, together))?;
                not_bool(&dim, key, "a positive int")?;
                dim.extract::<u32>()
                    .ok()
                    .filter(|d| *d > 0)
                    .ok_or_else(|| invalid(key, "a positive int"))
            };
            let (width, height) = (dimension("pixel_width")?, dimension("pixel_height")?);
            validate_rgba_frame_len("node property `rgba`", rgba.len(), width, height)?;
            change(move |node| {
                if let NodeKind::Image(state) = &mut node.kind {
                    state.image = peniko::ImageData {
                        data: peniko::Blob::from(rgba),
                        format: peniko::ImageFormat::Rgba8,
                        alpha_type: peniko::ImageAlphaType::Alpha,
                        width,
                        height,
                    };
                }
            })
        }
        "pixel_width" | "pixel_height" => {
            // Applied with `rgba`, which it must come with.
            if props.get_item("rgba")?.is_none() {
                return Err(invalid(
                    name,
                    "given with `rgba` -- the three are set together",
                ));
            }
            change(|_| {})
        }
        "fit" => {
            let fit = keyword(&FIT, value, name)?;
            change(move |node| {
                if let NodeKind::Image(state) = &mut node.kind {
                    state.content_fit = fit;
                }
            })
        }
        "orientation" => {
            let horizontal = keyword(&ORIENTATION, value, name)?;
            change(move |node| {
                if let NodeKind::ScrollView(state) = &mut node.kind {
                    state.horizontal = horizontal;
                }
            })
        }
        "scroll_offset" => {
            not_bool(value, name, "a non-negative number")?;
            let offset: f64 = value
                .extract()
                .ok()
                .filter(|v: &f64| *v >= 0.0 && v.is_finite())
                .ok_or_else(|| invalid(name, "a non-negative number"))?;
            change(move |node| {
                if let NodeKind::ScrollView(state) = &mut node.kind {
                    state.scroll = Animated::new(offset);
                }
            })
        }
        "item_count" => {
            not_bool(value, name, "a non-negative int")?;
            let count: usize = value
                .extract()
                .map_err(|_| invalid(name, "a non-negative int"))?;
            let mut rows = change(move |node| {
                if let NodeKind::VirtualList(state) = &mut node.kind {
                    state.item_count = count;
                    state.resolved_offsets.clear();
                    state.scroll_offset = Animated::new(0.0);
                }
            })?;
            rows.resets_rows = true;
            Ok(rows)
        }
        "item_extent" => {
            let extent = positive(value, name)?;
            let mut rows = change(move |node| {
                if let NodeKind::VirtualList(state) = &mut node.kind {
                    state.item_extent = engine_core::ItemExtent::Fixed(f64::from(extent));
                    state.resolved_offsets.clear();
                }
            })?;
            rows.resets_rows = true;
            Ok(rows)
        }
        _ => {
            // "cols" | "rows"
            not_bool(value, name, "an int from 1 to 1000")?;
            let count: u16 = value
                .extract()
                .ok()
                .filter(|v| (1..=1000).contains(v))
                .ok_or_else(|| invalid(name, "an int from 1 to 1000"))?;
            let cols = name == "cols";
            let mut resize = change(move |node| {
                if let NodeKind::Terminal(state) = &mut node.kind {
                    let (c, r) = if cols {
                        (count, state.rows)
                    } else {
                        (state.cols, count)
                    };
                    state.resize_grid(c, r);
                }
            })?;
            resize.resizes_terminal = true;
            Ok(resize)
        }
    }
}

fn parse_terminal_selection(
    value: &Bound<'_, PyAny>,
    name: &str,
    cols: u16,
    rows: u16,
) -> PyResult<KindChange> {
    let expected = "None or a (start_row, start_col, end_row, end_col) tuple inside the grid";
    let selection = if value.is_none() {
        None
    } else {
        let (sr, sc, er, ec): (u16, u16, u16, u16) =
            value.extract().map_err(|_| invalid(name, expected))?;
        if sr >= rows || er >= rows || sc >= cols || ec >= cols {
            return Err(invalid(name, expected));
        }
        Some(((sr, sc), (er, ec)))
    };
    change(move |node| {
        if let NodeKind::Terminal(state) = &mut node.kind {
            state.selection_start = selection.map(|(start, _)| start);
            state.selection_end = selection.map(|(_, end)| end);
        }
    })
}

fn to_py<'py, T: IntoPyObject<'py>>(value: T, py: Python<'py>) -> PyResult<Py<PyAny>> {
    pyo3::IntoPyObjectExt::into_py_any(value, py)
}

/// Reads kind property `name`, or `None` if it isn't one (or is animatable,
/// read elsewhere). Raises when it doesn't apply to this node's kind.
pub(crate) fn read_kind_prop(
    name: &str,
    node: &engine_core::Node,
    py: Python<'_>,
) -> Option<PyResult<Py<PyAny>>> {
    if !KIND_PROPS.contains(&name) || name == "scroll_offset" {
        return None;
    }
    // M100: a terminal's text is its visible grid -- read-only, one line
    // per row, each row's trailing blanks trimmed (was `get_text()`).
    if let ("text", NodeKind::Terminal(state)) = (name, &node.kind) {
        let lines: Vec<String> = (0..state.rows)
            .map(|row| {
                let line: String = (0..state.cols).map(|col| state.cell(row, col).ch).collect();
                line.trim_end().to_string()
            })
            .collect();
        return Some(to_py(lines.join("\n"), py));
    }
    let (kinds, test) = applies(name);
    if !test(&node.kind) {
        return Some(Err(PyValueError::new_err(format!(
            "node property `{name}` applies only to {kinds} node"
        ))));
    }
    Some(read_known(name, &node.kind, py))
}

fn read_known(name: &str, kind: &NodeKind, py: Python<'_>) -> PyResult<Py<PyAny>> {
    match (name, kind) {
        ("text", NodeKind::Text(s)) => to_py(&s.content, py),
        ("text", NodeKind::TextField(s)) => to_py(&s.content, py),
        ("font_family", NodeKind::Text(s)) => to_py(&s.font_family, py),
        ("font_family", NodeKind::TextField(s)) => to_py(&s.font_family, py),
        ("font_weight", NodeKind::Text(s)) => to_py(f64::from(s.font_weight), py),
        ("font_weight", NodeKind::TextField(s)) => to_py(f64::from(s.font_weight), py),
        ("font_size", NodeKind::Text(s)) => to_py(f64::from(s.font_size), py),
        ("font_size", NodeKind::TextField(s)) => to_py(f64::from(s.font_size), py),
        ("font_size", NodeKind::Terminal(s)) => to_py(f64::from(s.font_size), py),
        ("line_height", NodeKind::Text(s)) => to_py(s.line_height.map(f64::from), py),
        ("text_align", NodeKind::Text(s)) => to_py(name_of(&TEXT_ALIGN, &s.align), py),
        ("font_style", NodeKind::Text(s)) => to_py(name_of(&FONT_STYLE, &s.options.italic), py),
        ("letter_spacing", NodeKind::Text(s)) => to_py(f64::from(s.options.letter_spacing), py),
        ("wrap", NodeKind::Text(s)) => to_py(name_of(&WRAP, &s.options.wrap), py),
        ("max_lines", NodeKind::Text(s)) => to_py(s.options.max_lines, py),
        ("selectable", NodeKind::Text(s)) => to_py(s.options.selectable, py),
        ("selection", NodeKind::Text(s)) => to_py(s.options.selection, py),
        ("spans", NodeKind::Text(s)) => {
            let mut out = Vec::with_capacity(s.options.spans.len());
            for span in &s.options.spans {
                let style = pyo3::types::PyDict::new(py);
                if let Some(color) = span.color {
                    style.set_item("color", color_to_py(color, py)?)?;
                }
                if let Some(weight) = span.weight {
                    style.set_item("weight", f64::from(weight))?;
                }
                if let Some(italic) = span.italic {
                    style.set_item("italic", italic)?;
                }
                if span.underline {
                    style.set_item("underline", true)?;
                }
                if span.strikethrough {
                    style.set_item("strikethrough", true)?;
                }
                if let Some(size) = span.font_size {
                    style.set_item("font_size", f64::from(size))?;
                }
                if let Some(family) = &span.font_family {
                    style.set_item("font_family", family)?;
                }
                if let Some(link) = &span.link {
                    style.set_item("link", link)?;
                }
                out.push((span.start, span.end, style));
            }
            to_py(out, py)
        }
        ("overflow", NodeKind::Text(s)) => to_py(name_of(&OVERFLOW, &s.options.ellipsis), py),
        ("multiline", NodeKind::TextField(s)) => to_py(s.multiline, py),
        ("show_whitespace", NodeKind::TextField(s)) => to_py(s.show_whitespace, py),
        ("selection", NodeKind::TextField(s)) => {
            to_py((s.selection_anchor.unwrap_or(s.cursor), s.cursor), py)
        }
        ("selection", NodeKind::Terminal(s)) => match (s.selection_start, s.selection_end) {
            (Some((sr, sc)), Some((er, ec))) => to_py((sr, sc, er, ec), py),
            _ => Ok(py.None()),
        },
        ("syntax_spans", NodeKind::TextField(s)) => {
            let mut out = Vec::with_capacity(s.syntax_spans.len());
            for (range, color) in &s.syntax_spans {
                out.push((range.start, range.end, color_to_py(*color, py)?));
            }
            to_py(out, py)
        }
        ("folded_ranges", NodeKind::TextField(s)) => {
            let ranges: Vec<(usize, usize)> =
                s.folded_ranges.iter().map(|r| (r.start, r.end)).collect();
            to_py(ranges, py)
        }
        ("svg_size", NodeKind::Svg(s)) => to_py((s.document.width, s.document.height), py),
        ("svg", NodeKind::Svg(s)) if s.source_is_text => {
            to_py(String::from_utf8_lossy(&s.source).into_owned(), py)
        }
        ("svg", NodeKind::Svg(s)) => to_py(pyo3::types::PyBytes::new(py, &s.source), py),
        ("svg_images", NodeKind::Svg(s)) => {
            let out = PyDict::new(py);
            for (href, bitmap) in s.images.0.iter() {
                let image = &bitmap.image;
                out.set_item(
                    href,
                    (
                        pyo3::types::PyBytes::new(py, image.data.data()),
                        image.width,
                        image.height,
                    ),
                )?;
            }
            to_py(out, py)
        }
        ("svg_color", NodeKind::Svg(s)) => match s.color {
            Some(c) => color_to_py(c, py),
            None => Ok(py.None()),
        },
        ("rgba", NodeKind::Image(s)) => {
            to_py(pyo3::types::PyBytes::new(py, s.image.data.data()), py)
        }
        ("pixel_width", NodeKind::Image(s)) => to_py(s.image.width, py),
        ("pixel_height", NodeKind::Image(s)) => to_py(s.image.height, py),
        ("fit", NodeKind::Image(s)) => to_py(name_of(&FIT, &s.content_fit), py),
        ("orientation", NodeKind::ScrollView(s)) => to_py(name_of(&ORIENTATION, &s.horizontal), py),
        ("item_count", NodeKind::VirtualList(s)) => to_py(s.item_count, py),
        ("item_extent", NodeKind::VirtualList(s)) => match s.item_extent {
            engine_core::ItemExtent::Fixed(extent) => to_py(extent, py),
            engine_core::ItemExtent::Variable => Ok(py.None()),
        },
        ("cols", NodeKind::Terminal(s)) => to_py(s.cols, py),
        ("rows", NodeKind::Terminal(s)) => to_py(s.rows, py),
        _ => unreachable!("read_kind_prop checks the kind first"),
    }
}
