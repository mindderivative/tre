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
pub(crate) const KIND_PROPS: [&str; 26] = [
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
}

fn change(edit: impl FnOnce(&mut engine_core::Node) + 'static) -> PyResult<KindChange> {
    Ok(KindChange {
        edit: Box::new(edit),
        late: false,
        resizes_terminal: false,
        resets_rows: false,
    })
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
        matches!(kind, NodeKind::TextField(_) | NodeKind::Terminal(_))
    }
    fn image(kind: &NodeKind) -> bool {
        matches!(kind, NodeKind::Image(_))
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
        | "overflow" => ("a text", text),
        "multiline" | "show_whitespace" | "syntax_spans" | "folded_ranges" => {
            ("a text_input", text_input)
        }
        "selection" => ("a text_input or terminal", selectable),
        "rgba" | "pixel_width" | "pixel_height" | "fit" => ("an image", image),
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
