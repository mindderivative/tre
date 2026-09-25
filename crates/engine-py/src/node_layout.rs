//! M96: the layout properties of `node.set`/`node.get` -- size and position,
//! flex layout, and a node's place as a flex child. Each property is one
//! parser, producing an edit to the node's taffy `Style`, and one reader,
//! giving the value back the way it was set. The vocabularies are tables
//! read in both directions, shared with the legacy `set_layout`.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use taffy::prelude::{
    AlignItems, Dimension, FlexDirection, FlexWrap, JustifyContent, LengthPercentage,
    LengthPercentageAuto, Position, Rect, Style,
};
use taffy::style::{ExpandedDimension, ExpandedLengthPercentage, ExpandedLengthPercentageAuto};

/// A parsed, validated layout write, applied once every property passed.
pub(crate) type StyleEdit = Box<dyn FnOnce(&mut Style)>;

/// Every taffy-style layout property, in the order errors list them.
pub(crate) const LAYOUT_PROPS: [&str; 29] = [
    "width",
    "height",
    "min_width",
    "min_height",
    "max_width",
    "max_height",
    "aspect_ratio",
    "position",
    "x",
    "y",
    "flex_direction",
    "flex_wrap",
    "align_items",
    "justify_content",
    "gap",
    "padding",
    "padding_top",
    "padding_right",
    "padding_bottom",
    "padding_left",
    "flex_grow",
    "flex_shrink",
    "flex_basis",
    "align_self",
    "margin",
    "margin_top",
    "margin_right",
    "margin_bottom",
    "margin_left",
];

pub(crate) const FLEX_DIRECTION: [(&str, FlexDirection); 2] = [
    ("horizontal", FlexDirection::Row),
    ("vertical", FlexDirection::Column),
];

const FLEX_WRAP: [(&str, FlexWrap); 2] = [("no_wrap", FlexWrap::NoWrap), ("wrap", FlexWrap::Wrap)];

pub(crate) const ALIGN: [(&str, AlignItems); 7] = [
    ("start", AlignItems::START),
    ("end", AlignItems::END),
    ("flex_start", AlignItems::FLEX_START),
    ("flex_end", AlignItems::FLEX_END),
    ("center", AlignItems::CENTER),
    ("baseline", AlignItems::BASELINE),
    ("stretch", AlignItems::STRETCH),
];

pub(crate) const JUSTIFY: [(&str, JustifyContent); 9] = [
    ("start", JustifyContent::START),
    ("end", JustifyContent::END),
    ("flex_start", JustifyContent::FLEX_START),
    ("flex_end", JustifyContent::FLEX_END),
    ("center", JustifyContent::CENTER),
    ("stretch", JustifyContent::STRETCH),
    ("space_between", JustifyContent::SPACE_BETWEEN),
    ("space_around", JustifyContent::SPACE_AROUND),
    ("space_evenly", JustifyContent::SPACE_EVENLY),
];

const POSITION: [(&str, Position); 2] = [
    ("relative", Position::Relative),
    ("absolute", Position::Absolute),
];

fn invalid(name: &str, expected: &str) -> PyErr {
    PyValueError::new_err(format!("node property `{name}` must be {expected}"))
}

/// `value` looked up in a vocabulary table, or the list of valid names.
pub(crate) fn lookup<T: Copy>(table: &[(&str, T)], value: &str) -> Result<T, String> {
    table
        .iter()
        .find(|(name, _)| *name == value)
        .map(|(_, v)| *v)
        .ok_or_else(|| {
            let names: Vec<&str> = table.iter().map(|(n, _)| *n).collect();
            format!("one of: {}", names.join(", "))
        })
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

fn to_py<'py, T: IntoPyObject<'py>>(value: T, py: Python<'py>) -> PyResult<Py<PyAny>> {
    pyo3::IntoPyObjectExt::into_py_any(value, py)
}

/// A non-bool number, finite, and at least zero.
fn non_negative(value: &Bound<'_, PyAny>, name: &str, expected: &str) -> PyResult<f32> {
    if value.is_instance_of::<pyo3::types::PyBool>() {
        return Err(invalid(name, expected));
    }
    let number: f32 = value.extract().map_err(|_| invalid(name, expected))?;
    if number < 0.0 || !number.is_finite() {
        return Err(invalid(name, expected));
    }
    Ok(number)
}

/// A percentage string like `"50%"`, as a fraction.
fn percent(value: &Bound<'_, PyAny>) -> Option<f32> {
    let text: String = value.extract().ok()?;
    text.strip_suffix('%')?
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|p| *p >= 0.0 && p.is_finite())
        .map(|p| p / 100.0)
}

/// A size: a number of pixels, `"auto"`, or a percentage.
fn dimension(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Dimension> {
    let expected = "a number, \"auto\", or a percentage like \"50%\"";
    if let Ok(number) = non_negative(value, name, expected) {
        return Ok(Dimension::length(number));
    }
    if value.extract::<String>().is_ok_and(|t| t == "auto") {
        return Ok(Dimension::auto());
    }
    percent(value)
        .map(Dimension::percent)
        .ok_or_else(|| invalid(name, expected))
}

/// A min or max size: like `dimension`, in taffy's own type for bounds.
fn size_bound(value: &Bound<'_, PyAny>, name: &str) -> PyResult<LengthPercentageAuto> {
    Ok(match ExpandedDimension::from(dimension(value, name)?) {
        ExpandedDimension::Length(v) => LengthPercentageAuto::length(v),
        ExpandedDimension::Percent(p) => LengthPercentageAuto::percent(p),
        _ => LengthPercentageAuto::auto(),
    })
}

fn length_percentage(value: &Bound<'_, PyAny>, name: &str) -> PyResult<LengthPercentage> {
    let expected = "a non-negative number or a percentage like \"50%\"";
    if let Ok(number) = non_negative(value, name, expected) {
        return Ok(LengthPercentage::length(number));
    }
    percent(value)
        .map(LengthPercentage::percent)
        .ok_or_else(|| invalid(name, expected))
}

/// A margin or an inset: any number, a percentage, or `"auto"`/`None`.
fn length_percentage_auto(value: &Bound<'_, PyAny>, name: &str) -> PyResult<LengthPercentageAuto> {
    let expected = "a number, a percentage like \"50%\", \"auto\", or None";
    if value.is_none() || value.extract::<String>().is_ok_and(|t| t == "auto") {
        return Ok(LengthPercentageAuto::auto());
    }
    if !value.is_instance_of::<pyo3::types::PyBool>()
        && let Ok(number) = value.extract::<f32>()
        && number.is_finite()
    {
        return Ok(LengthPercentageAuto::length(number));
    }
    percent(value)
        .map(LengthPercentageAuto::percent)
        .ok_or_else(|| invalid(name, expected))
}

/// A stored fraction as the percentage it was set from -- in `f32`, the
/// precision it's stored at, so `"90%"` reads back as `"90%"`.
fn percent_text(fraction: f32) -> String {
    format!("{}%", fraction * 100.0)
}

fn dimension_to_py(dim: Dimension, py: Python<'_>) -> PyResult<Py<PyAny>> {
    match ExpandedDimension::from(dim) {
        ExpandedDimension::Length(v) => to_py(f64::from(v), py),
        ExpandedDimension::Percent(p) => to_py(percent_text(p), py),
        _ => to_py("auto", py),
    }
}

fn length_percentage_to_py(value: LengthPercentage, py: Python<'_>) -> PyResult<Py<PyAny>> {
    match ExpandedLengthPercentage::from(value) {
        ExpandedLengthPercentage::Percent(p) => to_py(percent_text(p), py),
        ExpandedLengthPercentage::Length(v) => to_py(f64::from(v), py),
        // `tre` never sets a calc() value.
        ExpandedLengthPercentage::Calc(_) => to_py(py.None(), py),
    }
}

fn length_percentage_auto_to_py(
    value: LengthPercentageAuto,
    py: Python<'_>,
) -> PyResult<Py<PyAny>> {
    match ExpandedLengthPercentageAuto::from(value) {
        ExpandedLengthPercentageAuto::Length(v) => to_py(f64::from(v), py),
        ExpandedLengthPercentageAuto::Percent(p) => to_py(percent_text(p), py),
        ExpandedLengthPercentageAuto::Auto => to_py("auto", py),
        ExpandedLengthPercentageAuto::Calc(_) => to_py(py.None(), py),
    }
}

/// A four-sided value read back as one value while the sides agree, and as
/// `(top, right, bottom, left)` otherwise.
fn sides_to_py<T: Copy + PartialEq>(
    rect: Rect<T>,
    one: fn(T, Python<'_>) -> PyResult<Py<PyAny>>,
    py: Python<'_>,
) -> PyResult<Py<PyAny>> {
    if rect.top == rect.right && rect.top == rect.bottom && rect.top == rect.left {
        return one(rect.top, py);
    }
    let sides = (
        one(rect.top, py)?,
        one(rect.right, py)?,
        one(rect.bottom, py)?,
        one(rect.left, py)?,
    );
    to_py(sides, py)
}

fn edit(f: impl FnOnce(&mut Style) + 'static) -> PyResult<StyleEdit> {
    Ok(Box::new(f))
}

/// Parses layout property `name`, or `None` if it isn't one.
pub(crate) fn parse_layout(name: &str, value: &Bound<'_, PyAny>) -> Option<PyResult<StyleEdit>> {
    if !LAYOUT_PROPS.contains(&name) {
        return None;
    }
    Some(parse_known(name, value))
}

fn parse_known(name: &str, value: &Bound<'_, PyAny>) -> PyResult<StyleEdit> {
    match name {
        "width" => {
            let v = dimension(value, name)?;
            edit(move |s| s.size.width = v)
        }
        "height" => {
            let v = dimension(value, name)?;
            edit(move |s| s.size.height = v)
        }
        "min_width" => {
            let v = size_bound(value, name)?;
            edit(move |s| s.min_size.width = v)
        }
        "min_height" => {
            let v = size_bound(value, name)?;
            edit(move |s| s.min_size.height = v)
        }
        "max_width" => {
            let v = size_bound(value, name)?;
            edit(move |s| s.max_size.width = v)
        }
        "max_height" => {
            let v = size_bound(value, name)?;
            edit(move |s| s.max_size.height = v)
        }
        "aspect_ratio" => {
            let expected = "a positive number or None";
            let v = if value.is_none() {
                None
            } else {
                Some(non_negative(value, name, expected)?)
                    .filter(|ratio| *ratio > 0.0)
                    .map(Some)
                    .ok_or_else(|| invalid(name, expected))?
            };
            edit(move |s| s.aspect_ratio = v)
        }
        "position" => {
            let v = keyword(&POSITION, value, name)?;
            edit(move |s| s.position = v)
        }
        "x" => {
            let v = length_percentage_auto(value, name)?;
            edit(move |s| s.inset.left = v)
        }
        "y" => {
            let v = length_percentage_auto(value, name)?;
            edit(move |s| s.inset.top = v)
        }
        "flex_direction" => {
            let v = keyword(&FLEX_DIRECTION, value, name)?;
            edit(move |s| s.flex_direction = v)
        }
        "flex_wrap" => {
            let v = keyword(&FLEX_WRAP, value, name)?;
            edit(move |s| s.flex_wrap = v)
        }
        "align_items" => {
            let v = keyword(&ALIGN, value, name)?;
            edit(move |s| s.align_items = Some(v))
        }
        "align_self" => {
            let v = if value.is_none() {
                None
            } else {
                Some(keyword(&ALIGN, value, name)?)
            };
            edit(move |s| s.align_self = v)
        }
        "justify_content" => {
            let v = keyword(&JUSTIFY, value, name)?;
            edit(move |s| s.justify_content = Some(v))
        }
        "gap" => {
            let v = length_percentage(value, name)?;
            edit(move |s| {
                s.gap.width = v;
                s.gap.height = v;
            })
        }
        "padding" => {
            let v = length_percentage(value, name)?;
            edit(move |s| {
                s.padding = Rect {
                    left: v,
                    right: v,
                    top: v,
                    bottom: v,
                }
            })
        }
        "padding_top" => {
            let v = length_percentage(value, name)?;
            edit(move |s| s.padding.top = v)
        }
        "padding_right" => {
            let v = length_percentage(value, name)?;
            edit(move |s| s.padding.right = v)
        }
        "padding_bottom" => {
            let v = length_percentage(value, name)?;
            edit(move |s| s.padding.bottom = v)
        }
        "padding_left" => {
            let v = length_percentage(value, name)?;
            edit(move |s| s.padding.left = v)
        }
        "margin" => {
            let v = length_percentage_auto(value, name)?;
            edit(move |s| {
                s.margin = Rect {
                    left: v,
                    right: v,
                    top: v,
                    bottom: v,
                }
            })
        }
        "margin_top" => {
            let v = length_percentage_auto(value, name)?;
            edit(move |s| s.margin.top = v)
        }
        "margin_right" => {
            let v = length_percentage_auto(value, name)?;
            edit(move |s| s.margin.right = v)
        }
        "margin_bottom" => {
            let v = length_percentage_auto(value, name)?;
            edit(move |s| s.margin.bottom = v)
        }
        "margin_left" => {
            let v = length_percentage_auto(value, name)?;
            edit(move |s| s.margin.left = v)
        }
        "flex_grow" => {
            let v = non_negative(value, name, "a non-negative number")?;
            edit(move |s| s.flex_grow = v)
        }
        "flex_shrink" => {
            let v = non_negative(value, name, "a non-negative number")?;
            edit(move |s| s.flex_shrink = v)
        }
        "flex_basis" => {
            let v = dimension(value, name)?;
            edit(move |s| s.flex_basis = v)
        }
        _ => unreachable!("parse_layout checks LAYOUT_PROPS first"),
    }
}

/// Reads layout property `name` from `style` the way it was set, or `None`
/// if it isn't one.
pub(crate) fn read_layout(
    name: &str,
    style: &Style,
    py: Python<'_>,
) -> Option<PyResult<Py<PyAny>>> {
    let value = match name {
        "width" => dimension_to_py(style.size.width, py),
        "height" => dimension_to_py(style.size.height, py),
        "min_width" => length_percentage_auto_to_py(style.min_size.width, py),
        "min_height" => length_percentage_auto_to_py(style.min_size.height, py),
        "max_width" => length_percentage_auto_to_py(style.max_size.width, py),
        "max_height" => length_percentage_auto_to_py(style.max_size.height, py),
        "aspect_ratio" => to_py(style.aspect_ratio.map(f64::from), py),
        "position" => to_py(name_of(&POSITION, &style.position), py),
        "x" => length_percentage_auto_to_py(style.inset.left, py),
        "y" => length_percentage_auto_to_py(style.inset.top, py),
        "flex_direction" => to_py(name_of(&FLEX_DIRECTION, &style.flex_direction), py),
        "flex_wrap" => to_py(name_of(&FLEX_WRAP, &style.flex_wrap), py),
        "align_items" => to_py(style.align_items.map(|v| name_of(&ALIGN, &v)), py),
        "align_self" => to_py(style.align_self.map(|v| name_of(&ALIGN, &v)), py),
        "justify_content" => to_py(style.justify_content.map(|v| name_of(&JUSTIFY, &v)), py),
        "gap" => length_percentage_to_py(style.gap.width, py),
        "padding" => sides_to_py(style.padding, length_percentage_to_py, py),
        "padding_top" => length_percentage_to_py(style.padding.top, py),
        "padding_right" => length_percentage_to_py(style.padding.right, py),
        "padding_bottom" => length_percentage_to_py(style.padding.bottom, py),
        "padding_left" => length_percentage_to_py(style.padding.left, py),
        "margin" => sides_to_py(style.margin, length_percentage_auto_to_py, py),
        "margin_top" => length_percentage_auto_to_py(style.margin.top, py),
        "margin_right" => length_percentage_auto_to_py(style.margin.right, py),
        "margin_bottom" => length_percentage_auto_to_py(style.margin.bottom, py),
        "margin_left" => length_percentage_auto_to_py(style.margin.left, py),
        "flex_grow" => to_py(f64::from(style.flex_grow), py),
        "flex_shrink" => to_py(f64::from(style.flex_shrink), py),
        "flex_basis" => dimension_to_py(style.flex_basis, py),
        _ => return None,
    };
    Some(value)
}
