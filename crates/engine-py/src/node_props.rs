//! M94: `node.set(**props)` / `node.get(name)` for the accessibility,
//! focus, and interaction properties of the M93 target API, and
//! `node.focus()`. `set` is atomic: every value is parsed and checked
//! before any is applied, so a bad call changes nothing. M96 extends the
//! same two methods to every property.

use std::cell::RefCell;
use std::rc::Rc;

use engine_core::{
    AccessValue, Animated, CornerRadii, Cursor, Interpolate, Live, NodeId, NodeKind, PathData,
    Role, Shadow, Shadows, TerminalPalette, Tree, WindowRegion,
};
use peniko::Color;
use peniko::kurbo::Rect;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use taffy::prelude::{AvailableSpace, Dimension};
use taffy::style::ExpandedDimension;

use crate::dispatch::{HandlerKey, fire_focus_transition};
use crate::node::Node;
use crate::node_kind_props::{KIND_PROPS, KindChange, parse_kind_prop, read_kind_prop};
use crate::node_layout::{LAYOUT_PROPS, StyleEdit, parse_layout, read_layout};

/// Every property `set` accepts besides the layout ones
/// (`node_layout::LAYOUT_PROPS`), in the order its error lists them.
const SETTABLE: [&str; 58] = [
    "visible",
    "z_index",
    "clip_children",
    "mask",
    "sticky",
    "translate_x",
    "translate_y",
    "scale",
    "rotation_deg",
    "fill",
    "stroke_color",
    "stroke_width",
    "opacity",
    "corner_radius",
    "shadows",
    "blur",
    "backdrop_blur",
    "blend_mode",
    "data",
    "view_box",
    "trim_start",
    "trim_end",
    "placeholder",
    "placeholder_fill",
    "caret_color",
    "selection_fill",
    "obscured",
    "scrollbar_fill",
    "scrollbar_width",
    "palette",
    "role",
    "label",
    "value",
    "value_min",
    "value_max",
    "value_step",
    "checked",
    "selected",
    "expanded",
    "disabled",
    "level",
    "live",
    "a11y_hidden",
    "pressed",
    "invalid",
    "busy",
    "current",
    "description",
    "describedby",
    "controls",
    "value_now",
    "value_text",
    "focusable",
    "tab_index",
    "cursor",
    "hit_testable",
    "window_region",
    "shader",
];

/// The M93 role vocabulary.
const ROLES: [(&str, Role); 24] = [
    ("button", Role::Button),
    ("checkbox", Role::CheckBox),
    ("radio", Role::RadioButton),
    ("switch", Role::Switch),
    ("slider", Role::Slider),
    ("progressbar", Role::ProgressIndicator),
    ("link", Role::Link),
    ("textbox", Role::TextInput),
    ("tab", Role::Tab),
    ("tablist", Role::TabList),
    ("tabpanel", Role::TabPanel),
    ("menu", Role::Menu),
    ("menuitem", Role::MenuItem),
    ("dialog", Role::Dialog),
    ("alert", Role::Alert),
    ("list", Role::List),
    ("listitem", Role::ListItem),
    ("tree", Role::Tree),
    ("treeitem", Role::TreeItem),
    ("heading", Role::Heading),
    ("img", Role::Image),
    ("group", Role::Group),
    ("none", Role::GenericContainer),
    // 0.5.6 (#160): AccessKit has no separator role; a splitter is the one the
    // platforms expose as a separator (AT-SPI) or separator control (UIA).
    ("separator", Role::Splitter),
];

const CURRENT: [(&str, engine_core::AriaCurrent); 5] = [
    ("page", engine_core::AriaCurrent::Page),
    ("step", engine_core::AriaCurrent::Step),
    ("location", engine_core::AriaCurrent::Location),
    ("date", engine_core::AriaCurrent::Date),
    ("time", engine_core::AriaCurrent::Time),
];

const LIVE: [(&str, Live); 3] = [
    ("off", Live::Off),
    ("polite", Live::Polite),
    ("assertive", Live::Assertive),
];

/// One parsed, validated property write.
pub(crate) enum Change {
    Role(Role),
    Label(Option<String>),
    Value(Option<AccessValue>),
    ValueMin(Option<f64>),
    ValueMax(Option<f64>),
    ValueStep(Option<f64>),
    Checked(Option<bool>),
    Selected(Option<bool>),
    Expanded(Option<bool>),
    Disabled(bool),
    Level(Option<usize>),
    Live(Option<Live>),
    A11yHidden(bool),
    /// 0.5.6 (#160): the accessibility states a screen reader hears about.
    Pressed(Option<engine_core::Toggled>),
    Invalid(bool),
    Busy(bool),
    Current(Option<engine_core::AriaCurrent>),
    Description(Option<String>),
    ValueNow(Option<f64>),
    ValueText(Option<String>),
    DescribedBy(Vec<NodeId>),
    Controls(Vec<NodeId>),
    Focusable(bool),
    TabIndex(i32),
    Cursor(Option<Cursor>),
    /// 0.5.0 M3: `window_region`.
    WindowRegion(WindowRegion),
    HitTestable(bool),
    /// 0.5.1 (#66): the node's `shader`; `None` clears it.
    Shader(Option<std::sync::Arc<engine_core::Shader>>),
    Style(StyleEdit),
    Kind(KindChange),
    /// M96: a canvas's `draw`, or a virtual list's `materialize` or
    /// `size_hint` -- stored in the handler map.
    Callback(HandlerKey, Py<PyAny>),
    Visible(bool),
    ZIndex(i32),
    ClipChildren(bool),
    /// 0.5.6 (#164): `mask`; `None` clears it.
    Mask(Option<Box<engine_core::Mask>>),
    /// 0.5.4 (#139): `sticky`, an inset from the scroller's start edge, or `None`.
    Sticky(Option<f64>),
    TranslateX(f64),
    TranslateY(f64),
    Scale(f64),
    RotationDeg(f64),
    Data(PathData),
    ViewBox(Option<Rect>),
    TrimStart(f64),
    TrimEnd(f64),
    Fill(Color),
    /// 0.5.4 (#110).
    Blur(f64),
    BackdropBlur(f64),
    BlendMode(engine_core::Blend),
    /// 0.5.4 (#109): `fill` set to a `Gradient`.
    FillGradient(engine_core::Gradient),
    StrokeColor(Color),
    StrokeGradient(engine_core::Gradient),
    StrokeWidth(f64),
    Opacity(f64),
    CornerRadius(Radius),
    Shadows(Vec<Shadow>),
    Placeholder(String),
    PlaceholderFill(Option<Color>),
    CaretColor(Option<Color>),
    SelectionFill(Option<Color>),
    Obscured(bool),
    ScrollbarFill(Option<Color>),
    ScrollbarWidth(f64),
    Palette(Box<PalettePatch>),
}

/// M95: the palette keys a `set(palette={...})` gives -- the rest keep
/// their current colors.
#[derive(Default)]
pub(crate) struct PalettePatch {
    ansi: Option<[Color; 16]>,
    foreground: Option<Color>,
    background: Option<Color>,
    cursor: Option<Color>,
    selection: Option<Color>,
}

impl PalettePatch {
    fn apply(&self, palette: &mut TerminalPalette) {
        if let Some(ansi) = self.ansi {
            palette.ansi = ansi;
        }
        let fields = [
            (self.foreground, &mut palette.foreground),
            (self.background, &mut palette.background),
            (self.cursor, &mut palette.cursor),
            (self.selection, &mut palette.selection),
        ];
        for (value, slot) in fields {
            if let Some(color) = value {
                *slot = color;
            }
        }
    }
}

fn parse_palette(value: &Bound<'_, PyAny>, name: &str) -> PyResult<PalettePatch> {
    let expected =
        "a dict with any of: ansi (16 colors), foreground, background, cursor, selection";
    let dict = value
        .cast::<PyDict>()
        .map_err(|_| invalid(name, expected))?;
    let mut patch = PalettePatch::default();
    for (key, color) in dict.iter() {
        let key: String = key.extract().map_err(|_| invalid(name, expected))?;
        match key.as_str() {
            "ansi" => {
                let colors: Vec<Bound<'_, PyAny>> = required(&color, name, expected)?;
                if colors.len() != 16 {
                    return Err(invalid(name, "a dict whose `ansi` has exactly 16 colors"));
                }
                let mut ansi = [Color::TRANSPARENT; 16];
                for (slot, color) in ansi.iter_mut().zip(&colors) {
                    *slot = parse_color(color, name)?;
                }
                patch.ansi = Some(ansi);
            }
            "foreground" => patch.foreground = Some(parse_color(&color, name)?),
            "background" => patch.background = Some(parse_color(&color, name)?),
            "cursor" => patch.cursor = Some(parse_color(&color, name)?),
            "selection" => patch.selection = Some(parse_color(&color, name)?),
            _ => return Err(invalid(name, expected)),
        }
    }
    Ok(patch)
}

fn palette_to_py(palette: &TerminalPalette, py: Python<'_>) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    let ansi = palette
        .ansi
        .iter()
        .map(|c| color_to_py(*c, py))
        .collect::<PyResult<Vec<_>>>()?;
    dict.set_item("ansi", ansi)?;
    dict.set_item("foreground", color_to_py(palette.foreground, py)?)?;
    dict.set_item("background", color_to_py(palette.background, py)?)?;
    dict.set_item("cursor", color_to_py(palette.cursor, py)?)?;
    dict.set_item("selection", color_to_py(palette.selection, py)?)?;
    Ok(dict.into_any().unbind())
}

fn optional_color(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<Color>> {
    if value.is_none() {
        return Ok(None);
    }
    parse_color(value, name).map(Some)
}

/// M95: `corner_radius` -- one radius, or `[top_left, top_right,
/// bottom_right, bottom_left]`.
#[derive(Clone, Copy)]
pub(crate) enum Radius {
    Uniform(f64),
    Corners([f64; 4]),
}

/// An `(r, g, b, a)` tuple of 0-255 ints.
pub(crate) fn parse_color(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Color> {
    let (r, g, b, a): (u8, u8, u8, u8) =
        required(value, name, "an (r, g, b, a) tuple of 0-255 ints")?;
    Ok(Color::from_rgba8(r, g, b, a))
}

/// A color as the `(r, g, b, a)` tuple it was set from.
pub(crate) fn color_to_py(color: Color, py: Python<'_>) -> PyResult<Py<PyAny>> {
    let [r, g, b, a] = color.to_rgba8().to_u8_array();
    Ok((r, g, b, a).into_pyobject(py)?.into_any().unbind())
}

/// A non-negative, finite number.
pub(crate) fn parse_non_negative(value: &Bound<'_, PyAny>, name: &str) -> PyResult<f64> {
    let number: f64 = required(value, name, "a non-negative number")?;
    if number < 0.0 || !number.is_finite() {
        return Err(invalid(name, "a non-negative number"));
    }
    Ok(number)
}

/// 0.5.6 (#164): `mask`: `None`, `"circle"`, `{"rounded": radius}` (a number or a
/// four-corner tuple) or `{"path": svg_path_data, "view_box": (x, y, w, h)}`.
fn parse_mask(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<Box<engine_core::Mask>>> {
    use engine_core::{CornerRadii, Mask};
    const EXPECTED: &str = "None, \"circle\", {\"rounded\": radius}, or {\"path\": svg_path_data, \"view_box\": (x, y, w, h)}";
    if value.is_none() {
        return Ok(None);
    }
    if let Ok(text) = value.extract::<String>() {
        return if text == "circle" {
            Ok(Some(Box::new(Mask::Circle)))
        } else {
            Err(invalid(name, EXPECTED))
        };
    }
    let dict = value
        .cast::<PyDict>()
        .map_err(|_| invalid(name, EXPECTED))?;
    let keys: Vec<String> = dict
        .keys()
        .iter()
        .map(|k| k.extract::<String>())
        .collect::<PyResult<_>>()
        .map_err(|_| invalid(name, EXPECTED))?;
    if keys == ["rounded"] {
        let radius = dict.get_item("rounded")?.expect("the key is there");
        let radii = match parse_radius(&radius, name)? {
            Radius::Uniform(r) => [r; 4],
            Radius::Corners(corners) => corners,
        };
        return Ok(Some(Box::new(Mask::Rounded(CornerRadii(radii)))));
    }
    if keys.len() == 2 && keys.iter().all(|k| k == "path" || k == "view_box") {
        let data: String = dict
            .get_item("path")?
            .expect("the key is there")
            .extract()
            .map_err(|_| invalid(name, "a mask whose path is a str of SVG path data"))?;
        let data = PathData::from_svg(&data).map_err(|err| {
            PyValueError::new_err(format!(
                "node property `{name}`: the path isn't valid SVG path data: {err}"
            ))
        })?;
        let view_box = dict.get_item("view_box")?.expect("the key is there");
        let (x, y, w, h): (f64, f64, f64, f64) = view_box.extract().map_err(|_| {
            invalid(
                name,
                "a mask whose view_box is an (x, y, width, height) tuple",
            )
        })?;
        if !(x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite())
            || w <= 0.0
            || h <= 0.0
        {
            return Err(invalid(name, "a mask whose view_box has a positive size"));
        }
        return Ok(Some(Box::new(Mask::Path {
            data,
            view_box: Rect::new(x, y, x + w, y + h),
        })));
    }
    Err(invalid(name, EXPECTED))
}

/// A mask as `parse_mask` takes it.
fn mask_to_py(mask: Option<&engine_core::Mask>, py: Python<'_>) -> PyResult<Py<PyAny>> {
    use engine_core::Mask;
    let Some(mask) = mask else {
        return Ok(py.None());
    };
    Ok(match mask {
        Mask::Circle => "circle".into_pyobject(py)?.into_any().unbind(),
        Mask::Rounded(radii) => {
            let dict = PyDict::new(py);
            let [a, b, c, d] = radii.0;
            if a == b && b == c && c == d {
                dict.set_item("rounded", a)?;
            } else {
                dict.set_item("rounded", (a, b, c, d))?;
            }
            dict.into_any().unbind()
        }
        Mask::Path { data, view_box } => {
            let dict = PyDict::new(py);
            dict.set_item("path", data.to_svg())?;
            dict.set_item(
                "view_box",
                (
                    view_box.x0,
                    view_box.y0,
                    view_box.width(),
                    view_box.height(),
                ),
            )?;
            dict.into_any().unbind()
        }
    })
}

pub(crate) fn parse_radius(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Radius> {
    let expected =
        "a non-negative number or a (top_left, top_right, bottom_right, bottom_left) tuple";
    if let Ok(number) = value.extract::<f64>() {
        if number < 0.0 || !number.is_finite() {
            return Err(invalid(name, expected));
        }
        return Ok(Radius::Uniform(number));
    }
    let corners: (f64, f64, f64, f64) = required(value, name, expected)?;
    let corners = [corners.0, corners.1, corners.2, corners.3];
    if corners.iter().any(|c| *c < 0.0 || !c.is_finite()) {
        return Err(invalid(name, expected));
    }
    Ok(Radius::Corners(corners))
}

pub(crate) fn parse_shadows(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Vec<Shadow>> {
    let expected = "a list of (color, offset_x, offset_y, blur, spread) tuples, blur non-negative";
    let items: Vec<Bound<'_, PyAny>> = required(value, name, expected)?;
    items
        .iter()
        .map(|item| {
            let (color, offset_x, offset_y, blur, spread): (Bound<'_, PyAny>, f64, f64, f64, f64) =
                required(item, name, expected)?;
            if blur < 0.0 {
                return Err(invalid(name, expected));
            }
            Ok(Shadow {
                color: parse_color(&color, name)?,
                offset_x,
                offset_y,
                blur,
                spread,
            })
        })
        .collect()
}

/// Reads an animatable property -- its current (possibly mid-animation)
/// value, or with `target` the value it's heading to. `None` for any
/// other name.
pub(crate) fn animatable_to_py(
    node: &engine_core::Node,
    name: &str,
    target: bool,
    py: Python<'_>,
) -> PyResult<Option<Py<PyAny>>> {
    fn pick<T: Interpolate + Clone>(value: &Animated<T>, target: bool) -> &T {
        if target {
            value.target()
        } else {
            &value.current
        }
    }
    let number = |v: f64| -> PyResult<Py<PyAny>> { Ok(v.into_pyobject(py)?.into_any().unbind()) };
    let value = match name {
        "fill" => match &node.paint.gradient {
            Some(gradient) => Py::new(
                py,
                crate::gradient::PyGradient {
                    inner: pick(gradient, target).clone(),
                },
            )?
            .into_any(),
            None => color_to_py(
                match &node.kind {
                    NodeKind::TextField(state) => *pick(&state.text_tint, target),
                    _ => *pick(&node.paint.background, target),
                },
                py,
            )?,
        },
        "scroll_offset" => {
            let NodeKind::ScrollView(state) = &node.kind else {
                return Err(PyValueError::new_err(
                    "node property `scroll_offset` applies only to a scroll_view node",
                ));
            };
            number(*pick(&state.scroll, target))?
        }
        "stroke_color" => match &node.paint.border_gradient {
            Some(gradient) => Py::new(
                py,
                crate::gradient::PyGradient {
                    inner: (**gradient).clone(),
                },
            )?
            .into_any(),
            None => color_to_py(*pick(&node.paint.border_color, target), py)?,
        },
        "stroke_width" => number(*pick(&node.paint.border_width, target))?,
        "translate_x" => number(*pick(&node.paint.node_transform.translate_x, target))?,
        "translate_y" => number(*pick(&node.paint.node_transform.translate_y, target))?,
        "scale" => number(*pick(&node.paint.node_transform.scale, target))?,
        "rotation_deg" => number(*pick(&node.paint.node_transform.rotation_deg, target))?,
        "opacity" => number(*pick(&node.paint.opacity, target))?,
        "blur" => number(*pick(&node.paint.blur, target))?,
        "backdrop_blur" => number(*pick(&node.paint.backdrop_blur, target))?,
        "corner_radius" => match &node.paint.corner_radii_override {
            Some(radii) => {
                let [a, b, c, d] = pick(radii, target).0;
                (a, b, c, d).into_pyobject(py)?.into_any().unbind()
            }
            None => number(*pick(&node.paint.corner_radius, target))?,
        },
        "shadows" => {
            let mut out = Vec::new();
            for shadow in &pick(&node.paint.shadows, target).0 {
                out.push((
                    color_to_py(shadow.color, py)?,
                    shadow.offset_x,
                    shadow.offset_y,
                    shadow.blur,
                    shadow.spread,
                ));
            }
            out.into_pyobject(py)?.into_any().unbind()
        }
        "data" | "trim_start" | "trim_end" => {
            let NodeKind::Path(state) = &node.kind else {
                return Err(PyValueError::new_err(format!(
                    "node property `{name}` applies only to a path node"
                )));
            };
            match name {
                "data" => pick(&state.data, target)
                    .to_svg()
                    .into_pyobject(py)?
                    .into_any()
                    .unbind(),
                "trim_start" => number(*pick(&state.trim_start, target))?,
                _ => number(*pick(&state.trim_end, target))?,
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(value))
}

/// Stops `name`'s running animation where it is. `false` for a name with
/// no animation to stop.
pub(crate) fn stop_animatable(node: &mut engine_core::Node, name: &str) -> PyResult<bool> {
    match name {
        "fill" => {
            if let Some(gradient) = &mut node.paint.gradient {
                gradient.stop();
            }
            match &mut node.kind {
                NodeKind::TextField(state) => state.text_tint.stop(),
                _ => node.paint.background.stop(),
            }
        }
        "scroll_offset" => {
            let NodeKind::ScrollView(state) = &mut node.kind else {
                return Err(PyValueError::new_err(
                    "node property `scroll_offset` applies only to a scroll_view node",
                ));
            };
            state.scroll.stop();
        }
        "stroke_color" => node.paint.border_color.stop(),
        "stroke_width" => node.paint.border_width.stop(),
        "translate_x" => node.paint.node_transform.translate_x.stop(),
        "translate_y" => node.paint.node_transform.translate_y.stop(),
        "scale" => node.paint.node_transform.scale.stop(),
        "rotation_deg" => node.paint.node_transform.rotation_deg.stop(),
        "opacity" => node.paint.opacity.stop(),
        "blur" => node.paint.blur.stop(),
        "backdrop_blur" => node.paint.backdrop_blur.stop(),
        "corner_radius" => {
            node.paint.corner_radius.stop();
            if let Some(radii) = &mut node.paint.corner_radii_override {
                radii.stop();
            }
        }
        "shadows" => node.paint.shadows.stop(),
        "data" | "trim_start" | "trim_end" => {
            let NodeKind::Path(state) = &mut node.kind else {
                return Err(PyValueError::new_err(format!(
                    "node property `{name}` applies only to a path node"
                )));
            };
            match name {
                "data" => state.data.stop(),
                "trim_start" => state.trim_start.stop(),
                _ => state.trim_end.stop(),
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// A property only one kind has: its name, that kind's name, and a test
/// for it.
type KindRequirement = (&'static str, &'static str, fn(&NodeKind) -> bool);

impl Change {
    /// The kind this change requires, if it's specific to one -- checked
    /// before anything is applied.
    fn kind_requirement(&self) -> Option<KindRequirement> {
        fn path(kind: &NodeKind) -> bool {
            matches!(kind, NodeKind::Path(_))
        }
        fn text_input(kind: &NodeKind) -> bool {
            matches!(kind, NodeKind::TextField(_))
        }
        fn scroll_view(kind: &NodeKind) -> bool {
            matches!(kind, NodeKind::ScrollView(_))
        }
        fn terminal(kind: &NodeKind) -> bool {
            matches!(kind, NodeKind::Terminal(_))
        }
        fn box_node(kind: &NodeKind) -> bool {
            matches!(
                kind,
                NodeKind::Rect | NodeKind::Container | NodeKind::Path(_) | NodeKind::Text(_)
            )
        }
        Some(match self {
            Change::Data(_) => ("data", "path", path),
            Change::ViewBox(_) => ("view_box", "path", path),
            Change::TrimStart(_) => ("trim_start", "path", path),
            Change::TrimEnd(_) => ("trim_end", "path", path),
            Change::Placeholder(_) => ("placeholder", "text_input", text_input),
            Change::PlaceholderFill(_) => ("placeholder_fill", "text_input", text_input),
            Change::CaretColor(_) => ("caret_color", "text_input", text_input),
            Change::SelectionFill(_) => ("selection_fill", "text_input", text_input),
            Change::Obscured(_) => ("obscured", "text_input", text_input),
            Change::ScrollbarFill(_) => ("scrollbar_fill", "scroll_view", scroll_view),
            Change::ScrollbarWidth(_) => ("scrollbar_width", "scroll_view", scroll_view),
            Change::Palette(_) => ("palette", "terminal", terminal),
            Change::FillGradient(_) => ("fill", "box, path or text", box_node),
            _ => return None,
        })
    }
}

/// A trim fraction, `0.0..=1.0`.
fn fraction(value: &Bound<'_, PyAny>, name: &str) -> PyResult<f64> {
    let fraction: f64 = required(value, name, "a number from 0.0 to 1.0")?;
    if !(0.0..=1.0).contains(&fraction) {
        return Err(invalid(name, "a number from 0.0 to 1.0"));
    }
    Ok(fraction)
}

fn invalid(name: &str, expected: &str) -> PyErr {
    PyValueError::new_err(format!("node property `{name}` must be {expected}"))
}

fn names<T>(table: &[(&str, T)]) -> String {
    table
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// `value`, extracted as `T` or `None`, with a clear error otherwise.
fn optional<'py, T: for<'a> FromPyObject<'a, 'py>>(
    value: &Bound<'py, PyAny>,
    name: &str,
    expected: &str,
) -> PyResult<Option<T>> {
    if value.is_none() {
        return Ok(None);
    }
    value
        .extract::<T>()
        .map(Some)
        .map_err(|_| invalid(name, expected))
}

fn required<'py, T: for<'a> FromPyObject<'a, 'py>>(
    value: &Bound<'py, PyAny>,
    name: &str,
    expected: &str,
) -> PyResult<T> {
    value.extract::<T>().map_err(|_| invalid(name, expected))
}

/// A bool that must be a real `bool`, not any truthy value.
fn boolean(value: &Bound<'_, PyAny>, name: &str) -> PyResult<bool> {
    if !value.is_instance_of::<pyo3::types::PyBool>() {
        return Err(invalid(name, "a bool"));
    }
    value.extract::<bool>()
}

fn optional_bool(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<bool>> {
    if value.is_none() {
        return Ok(None);
    }
    boolean(value, name).map(Some)
}

fn parse(name: &str, value: &Bound<'_, PyAny>) -> PyResult<Change> {
    if let Some(edit) = parse_layout(name, value) {
        return edit.map(Change::Style);
    }
    let number = |expected: &str| -> PyResult<f64> {
        let number: f64 = required(value, name, expected)?;
        if !number.is_finite() || value.is_instance_of::<pyo3::types::PyBool>() {
            return Err(invalid(name, expected));
        }
        Ok(number)
    };
    Ok(match name {
        "visible" => Change::Visible(boolean(value, name)?),
        "z_index" => {
            if value.is_instance_of::<pyo3::types::PyBool>() {
                return Err(invalid(name, "an int"));
            }
            Change::ZIndex(required(value, name, "an int")?)
        }
        "clip_children" => Change::ClipChildren(boolean(value, name)?),
        "mask" => Change::Mask(parse_mask(value, name)?),
        "sticky" => Change::Sticky(if value.is_none() {
            None
        } else if value.is_instance_of::<pyo3::types::PyBool>() {
            return Err(invalid(name, "a non-negative number of pixels, or None"));
        } else {
            Some(
                parse_non_negative(value, name)
                    .map_err(|_| invalid(name, "a non-negative number of pixels, or None"))?,
            )
        }),
        "translate_x" => Change::TranslateX(number("a number")?),
        "translate_y" => Change::TranslateY(number("a number")?),
        "scale" => Change::Scale(parse_non_negative(value, name)?),
        "rotation_deg" => Change::RotationDeg(number("a number")?),
        "role" => {
            let role: String = required(value, name, "a str")?;
            let role = ROLES
                .iter()
                .find(|(n, _)| *n == role)
                .map(|(_, r)| *r)
                .ok_or_else(|| invalid(name, &format!("one of: {}", names(&ROLES))))?;
            Change::Role(role)
        }
        "label" => Change::Label(optional(value, name, "a str or None")?),
        "value" => Change::Value(if value.is_none() {
            None
        } else if value.is_instance_of::<pyo3::types::PyBool>() {
            return Err(invalid(name, "a str, a number, or None"));
        } else if let Ok(text) = value.extract::<String>() {
            Some(AccessValue::Text(text))
        } else if let Ok(number) = value.extract::<f64>() {
            Some(AccessValue::Number(number))
        } else {
            return Err(invalid(name, "a str, a number, or None"));
        }),
        "value_min" => Change::ValueMin(optional(value, name, "a number or None")?),
        "value_max" => Change::ValueMax(optional(value, name, "a number or None")?),
        "value_step" => Change::ValueStep(optional(value, name, "a number or None")?),
        "checked" => Change::Checked(optional_bool(value, name)?),
        "selected" => Change::Selected(optional_bool(value, name)?),
        "expanded" => Change::Expanded(optional_bool(value, name)?),
        "disabled" => Change::Disabled(boolean(value, name)?),
        "level" => {
            let level: Option<usize> = optional(value, name, "a positive int or None")?;
            if level == Some(0) {
                return Err(invalid(name, "a positive int or None"));
            }
            Change::Level(level)
        }
        "live" => {
            let live: Option<String> = optional(value, name, "a str or None")?;
            Change::Live(match live {
                None => None,
                Some(live) => Some(
                    LIVE.iter()
                        .find(|(n, _)| *n == live)
                        .map(|(_, l)| *l)
                        .ok_or_else(|| invalid(name, &format!("one of: {}", names(&LIVE))))?,
                ),
            })
        }
        "a11y_hidden" => Change::A11yHidden(boolean(value, name)?),
        "pressed" => Change::Pressed(if value.is_none() {
            None
        } else if value.is_instance_of::<pyo3::types::PyBool>() {
            Some(if value.extract::<bool>()? {
                engine_core::Toggled::True
            } else {
                engine_core::Toggled::False
            })
        } else if value.extract::<String>().is_ok_and(|s| s == "mixed") {
            Some(engine_core::Toggled::Mixed)
        } else {
            return Err(invalid(name, "True, False, \"mixed\", or None"));
        }),
        "invalid" => Change::Invalid(boolean(value, name)?),
        "busy" => Change::Busy(boolean(value, name)?),
        "current" => Change::Current(if value.is_none() {
            None
        } else if value.is_instance_of::<pyo3::types::PyBool>() {
            Some(if value.extract::<bool>()? {
                engine_core::AriaCurrent::True
            } else {
                engine_core::AriaCurrent::False
            })
        } else {
            let text: String = required(value, name, "a str, a bool, or None")?;
            Some(
                CURRENT
                    .iter()
                    .find(|(n, _)| *n == text)
                    .map(|(_, c)| *c)
                    .ok_or_else(|| {
                        invalid(
                            name,
                            &format!("True, False, None, or one of: {}", names(&CURRENT)),
                        )
                    })?,
            )
        }),
        "description" => Change::Description(optional(value, name, "a str or None")?),
        "value_now" => Change::ValueNow(optional(value, name, "a number or None")?),
        "value_text" => Change::ValueText(optional(value, name, "a str or None")?),
        "focusable" => Change::Focusable(boolean(value, name)?),
        "tab_index" => Change::TabIndex(required(value, name, "an int")?),
        // 0.5.4 (#140): a `CursorImage` is a cursor too.
        "cursor"
            if value
                .extract::<crate::cursor_image::PyCursorImage>()
                .is_ok() =>
        {
            let image = value
                .extract::<crate::cursor_image::PyCursorImage>()
                .expect("checked");
            Change::Cursor(Some(Cursor::Custom(image.id)))
        }
        "cursor" => {
            let cursor: Option<String> = optional(value, name, "a str, a CursorImage, or None")?;
            Change::Cursor(match cursor {
                None => None,
                Some(cursor) => Some(Cursor::from_name(&cursor).ok_or_else(|| {
                    let valid: Vec<&str> = Cursor::ALL.iter().map(|c| c.name()).collect();
                    invalid(name, &format!("one of: {}", valid.join(", ")))
                })?),
            })
        }
        "hit_testable" => Change::HitTestable(boolean(value, name)?),
        // 0.5.0 M3 (issue #28): `"drag"` makes the node a title bar -- a
        // primary press on it, or on a non-interactive node inside, moves
        // the window -- `"none"` rules that out, `None` leaves it to the
        // nodes around it.
        "window_region" => {
            let region: Option<String> = optional(value, name, "\"drag\", \"none\", or None")?;
            Change::WindowRegion(match region.as_deref() {
                None => WindowRegion::Default,
                Some("drag") => WindowRegion::Drag,
                Some("none") => WindowRegion::NoDrag,
                Some(_) => return Err(invalid(name, "\"drag\", \"none\", or None")),
            })
        }
        "data" => {
            let data: String = required(value, name, "SVG path data (a str)")?;
            Change::Data(PathData::from_svg(&data).map_err(|err| {
                PyValueError::new_err(format!(
                    "node property `data` isn't valid SVG path data: {err}"
                ))
            })?)
        }
        "view_box" => Change::ViewBox(if value.is_none() {
            None
        } else {
            let expected = "a (min_x, min_y, width, height) tuple with a positive size, or None";
            let (x, y, w, h): (f64, f64, f64, f64) = required(value, name, expected)?;
            if w <= 0.0 || h <= 0.0 {
                return Err(invalid(name, expected));
            }
            Some(Rect::new(x, y, x + w, y + h))
        }),
        "trim_start" => Change::TrimStart(fraction(value, name)?),
        "trim_end" => Change::TrimEnd(fraction(value, name)?),
        "fill" => match value.extract::<crate::gradient::PyGradient>() {
            Ok(gradient) => Change::FillGradient(gradient.inner),
            Err(_) => Change::Fill(parse_color(value, name).map_err(|_| {
                invalid(name, "an (r, g, b, a) tuple of 0-255 ints, or a Gradient")
            })?),
        },
        "stroke_color" => match value.extract::<crate::gradient::PyGradient>() {
            Ok(gradient) => Change::StrokeGradient(gradient.inner),
            Err(_) => Change::StrokeColor(parse_color(value, name)?),
        },
        "stroke_width" => Change::StrokeWidth(parse_non_negative(value, name)?),
        "opacity" => Change::Opacity(fraction(value, name)?),
        "blur" => Change::Blur(parse_non_negative(value, name)?),
        "backdrop_blur" => Change::BackdropBlur(parse_non_negative(value, name)?),
        "blend_mode" => {
            let mode: String = required(value, name, "a blend mode name")?;
            Change::BlendMode(engine_core::Blend::from_name(&mode).ok_or_else(|| {
                let valid: Vec<&str> = engine_core::Blend::ALL.iter().map(|(n, _)| *n).collect();
                invalid(name, &format!("one of: {}", valid.join(", ")))
            })?)
        }
        "corner_radius" => Change::CornerRadius(parse_radius(value, name)?),
        "shadows" => Change::Shadows(parse_shadows(value, name)?),
        "placeholder" => Change::Placeholder(required(value, name, "a str")?),
        "placeholder_fill" => Change::PlaceholderFill(optional_color(value, name)?),
        "caret_color" => Change::CaretColor(optional_color(value, name)?),
        "selection_fill" => Change::SelectionFill(optional_color(value, name)?),
        "obscured" => Change::Obscured(boolean(value, name)?),
        "scrollbar_fill" => Change::ScrollbarFill(optional_color(value, name)?),
        "scrollbar_width" => Change::ScrollbarWidth(parse_non_negative(value, name)?),
        "palette" => Change::Palette(Box::new(parse_palette(value, name)?)),
        _ => {
            return Err(PyValueError::new_err(format!(
                "unknown node property {name:?} -- settable: {}, {}, {}, draw, materialize, \
                 size_hint",
                LAYOUT_PROPS.join(", "),
                SETTABLE.join(", "),
                KIND_PROPS.join(", ")
            )));
        }
    })
}

/// The callback properties, each stored under its handler key.
const CALLBACKS: [(&str, HandlerKey, &str); 3] = [
    ("draw", HandlerKey::Draw, "canvas"),
    ("materialize", HandlerKey::Materialize, "virtual_list"),
    ("size_hint", HandlerKey::SizeHint, "virtual_list"),
];

fn parse_callback(
    name: &str,
    value: &Bound<'_, PyAny>,
    kind: &NodeKind,
) -> Option<PyResult<Change>> {
    let (_, key, kind_name) = CALLBACKS.iter().find(|(n, _, _)| *n == name)?;
    let applies = match key {
        HandlerKey::Draw => matches!(kind, NodeKind::Canvas(_)),
        _ => matches!(kind, NodeKind::VirtualList(_)),
    };
    Some(if !applies {
        Err(PyValueError::new_err(format!(
            "node property `{name}` applies only to a {kind_name} node"
        )))
    } else if !value.is_callable() {
        Err(invalid(name, "a callable"))
    } else {
        Ok(Change::Callback(*key, value.clone().unbind()))
    })
}

/// 0.5.1 (#66): `shader` takes a `tre.Shader` or `None`. Its input nodes
/// must live in the same window as the node it is set on.
fn parse_shader(value: &Bound<'_, PyAny>, tree: &Rc<RefCell<Tree>>) -> PyResult<Change> {
    if value.is_none() {
        return Ok(Change::Shader(None));
    }
    let shader = value
        .cast::<crate::shader::Shader>()
        .map_err(|_| invalid("shader", "a tre.Shader or None"))?
        .get();
    if !shader.inputs_in(value.py(), tree) {
        return Err(PyValueError::new_err(
            "node property `shader`: the shader's input nodes belong to a different Window",
        ));
    }
    Ok(Change::Shader(Some(shader.core.clone())))
}

/// 0.5.6 (#160): `describedby` / `controls`: a node, a list of nodes, or `None`
/// -- all of this window's.
fn parse_related(
    name: &str,
    value: &Bound<'_, PyAny>,
    tree: &Rc<RefCell<Tree>>,
) -> PyResult<Vec<NodeId>> {
    const EXPECTED: &str = "a Node, a list of Nodes, or None";
    if value.is_none() {
        return Ok(Vec::new());
    }
    let items: Vec<Bound<'_, PyAny>> = if let Ok(node) = value.cast::<Node>() {
        vec![node.clone().into_any()]
    } else if value.is_instance_of::<pyo3::types::PyList>()
        || value.is_instance_of::<pyo3::types::PyTuple>()
    {
        value.try_iter()?.collect::<PyResult<Vec<_>>>()?
    } else {
        return Err(invalid(name, EXPECTED));
    };
    let mut ids = Vec::new();
    for item in items {
        let node: PyRef<'_, Node> = item.extract().map_err(|_| invalid(name, EXPECTED))?;
        if !Rc::ptr_eq(&node.tree, tree) {
            return Err(PyValueError::new_err(format!(
                "node property `{name}`: the node belongs to a different Window"
            )));
        }
        if !ids.contains(&node.id) {
            ids.push(node.id);
        }
    }
    Ok(ids)
}

/// Every property `set` accepts, parsed and checked against `kind` --
/// nothing is applied until all of them pass.
pub(crate) fn parse_all(
    props: Option<&Bound<'_, PyDict>>,
    kind: &NodeKind,
    tree: &Rc<RefCell<Tree>>,
) -> PyResult<Vec<Change>> {
    let mut changes = Vec::new();
    if let Some(props) = props {
        for (name, value) in props.iter() {
            let name: String = name.extract()?;
            if let Some(kind_change) = parse_kind_prop(&name, &value, kind, props) {
                changes.push(Change::Kind(kind_change?));
                continue;
            }
            if let Some(callback) = parse_callback(&name, &value, kind) {
                changes.push(callback?);
                continue;
            }
            if name == "shader" {
                changes.push(parse_shader(&value, tree)?);
                continue;
            }
            if name == "describedby" || name == "controls" {
                let ids = parse_related(&name, &value, tree)?;
                changes.push(if name == "controls" {
                    Change::Controls(ids)
                } else {
                    Change::DescribedBy(ids)
                });
                continue;
            }
            let change = parse(&name, &value)?;
            if let Some((prop, kind_name, applies)) = change.kind_requirement()
                && !applies(kind)
            {
                return Err(PyValueError::new_err(format!(
                    "node property `{prop}` applies only to a {kind_name} node"
                )));
            }
            changes.push(change);
        }
    }
    changes.sort_by_key(|change| matches!(change, Change::Kind(k) if k.late));
    Ok(changes)
}

#[pymethods]
impl Node {
    /// Sets any number of properties at once, atomically: every value is
    /// checked first, and a bad one raises `ValueError` without changing
    /// anything. An optional property takes `None` to clear it.
    #[pyo3(signature = (**props))]
    fn set(&self, props: Option<&Bound<'_, PyDict>>, py: Python<'_>) -> PyResult<()> {
        let changes = {
            let tree = self.tree.borrow();
            let node = tree
                .get(self.id)
                .ok_or(crate::error::EngineError::Destroyed)?;
            let changes = parse_all(props, &node.kind, &self.tree)?;
            for change in &changes {
                if let Change::Shader(Some(shader)) = change
                    && tree.shader_cycle(self.id, shader)
                {
                    return Err(PyValueError::new_err(
                        "node property `shader`: the shader would read itself -- through its \
                         inputs it depends on this node's own shader",
                    ));
                }
            }
            changes
        };
        let redraw = changes
            .iter()
            .any(|change| matches!(change, Change::Callback(HandlerKey::Draw, _)));
        self.apply(changes);
        if redraw {
            crate::node_callbacks::redraw(&self.tree, &self.handlers, self.id, py)?;
        }
        // 0.4.3 M15: a new offset is clamped to the view's range before it's
        // reported -- layout does the clamping -- so it reads back right at
        // once and fires one `scroll`, not a second when the frame lays out.
        if props.is_some_and(|p| p.contains("scroll_offset").unwrap_or(false)) {
            self.layout_box(py);
        }
        self.fire_scroll_changes(py);
        Ok(())
    }

    /// Reads one property: any `set` property, `focused`, or -- for the
    /// animatable numeric properties -- its current, possibly
    /// mid-animation value. `value` is the accessibility value.
    fn get(&self, name: &str, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = {
            let tree = self.tree.borrow();
            let node = tree.get(self.id).ok_or_else(|| {
                PyValueError::new_err("this node has been removed from its window")
            })?;
            if let Some(value) = animatable_to_py(node, name, false, py)? {
                return Ok(value);
            }
            if let Some(value) = read_layout(name, &node.layout_style, py) {
                return value;
            }
            if let Some(value) = read_kind_prop(name, node, py) {
                return value;
            }
            if let Some((_, key, _)) = CALLBACKS.iter().find(|(n, _, _)| *n == name) {
                return Ok(
                    crate::node_callbacks::callback(&self.handlers, self.id, *key, py)
                        .unwrap_or_else(|| py.None()),
                );
            }
            let access = &node.access;
            let any = |v: Bound<'_, PyAny>| v.unbind();
            match name {
                "kind" => any(crate::node::kind_id(&node.kind)
                    .into_pyobject(py)?
                    .into_any()),
                "role" => ROLES
                    .iter()
                    .find(|(_, r)| *r == access.role)
                    .map(|(n, _)| *n)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind(),
                "label" => access.label.clone().into_pyobject(py)?.into_any().unbind(),
                "value" => match &access.value {
                    Some(AccessValue::Text(text)) => {
                        any(text.clone().into_pyobject(py)?.into_any())
                    }
                    Some(AccessValue::Number(n)) => any(n.into_pyobject(py)?.into_any()),
                    None => py.None(),
                },
                "value_min" => access.value_min.into_pyobject(py)?.into_any().unbind(),
                "value_max" => access.value_max.into_pyobject(py)?.into_any().unbind(),
                "value_step" => access.value_step.into_pyobject(py)?.into_any().unbind(),
                "checked" => access.checked.into_pyobject(py)?.into_any().unbind(),
                "selected" => access.selected.into_pyobject(py)?.into_any().unbind(),
                "expanded" => access.expanded.into_pyobject(py)?.into_any().unbind(),
                "disabled" => any(access
                    .states
                    .disabled
                    .into_pyobject(py)?
                    .to_owned()
                    .into_any()),
                "level" => access.level.into_pyobject(py)?.into_any().unbind(),
                "live" => LIVE
                    .iter()
                    .find(|(_, l)| Some(*l) == access.live)
                    .map(|(n, _)| *n)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind(),
                "a11y_hidden" => any(access.hidden.into_pyobject(py)?.to_owned().into_any()),
                "pressed" => match access.pressed {
                    None => py.None(),
                    Some(engine_core::Toggled::True) => {
                        any(true.into_pyobject(py)?.to_owned().into_any())
                    }
                    Some(engine_core::Toggled::False) => {
                        any(false.into_pyobject(py)?.to_owned().into_any())
                    }
                    Some(engine_core::Toggled::Mixed) => any("mixed".into_pyobject(py)?.into_any()),
                },
                "invalid" => any(access.invalid.into_pyobject(py)?.to_owned().into_any()),
                "busy" => any(access.busy.into_pyobject(py)?.to_owned().into_any()),
                "current" => match access.current {
                    None => py.None(),
                    Some(engine_core::AriaCurrent::True) => {
                        any(true.into_pyobject(py)?.to_owned().into_any())
                    }
                    Some(engine_core::AriaCurrent::False) => {
                        any(false.into_pyobject(py)?.to_owned().into_any())
                    }
                    Some(current) => CURRENT
                        .iter()
                        .find(|(_, c)| *c == current)
                        .map(|(n, _)| *n)
                        .into_pyobject(py)?
                        .into_any()
                        .unbind(),
                },
                "description" => access
                    .description
                    .clone()
                    .into_pyobject(py)?
                    .into_any()
                    .unbind(),
                "value_now" => match &access.value {
                    Some(AccessValue::Number(n)) => any(n.into_pyobject(py)?.into_any()),
                    _ => py.None(),
                },
                "value_text" => access
                    .extra
                    .as_ref()
                    .and_then(|x| x.value_text.clone())
                    .into_pyobject(py)?
                    .into_any()
                    .unbind(),
                "describedby" | "controls" => {
                    let empty = Vec::new();
                    let ids = access.extra.as_ref().map_or(&empty, |x| {
                        if name == "controls" {
                            &x.controls
                        } else {
                            &x.described_by
                        }
                    });
                    let tree = self.tree.borrow();
                    let nodes: Vec<Node> = ids
                        .iter()
                        .filter(|id| tree.get(**id).is_some())
                        .map(|id| self.handle_to(*id))
                        .collect();
                    drop(tree);
                    any(nodes.into_pyobject(py)?.into_any())
                }
                "focusable" => {
                    let focusable = access.focusable.unwrap_or(!access.actions.is_empty());
                    any(focusable.into_pyobject(py)?.to_owned().into_any())
                }
                "tab_index" => any(access.tab_index.into_pyobject(py)?.into_any()),
                "cursor" => match node.cursor {
                    Some(Cursor::Custom(id)) => {
                        Py::new(py, crate::cursor_image::PyCursorImage::from_id(id))?.into_any()
                    }
                    other => other
                        .map(Cursor::name)
                        .into_pyobject(py)?
                        .into_any()
                        .unbind(),
                },
                "shader" => match &node.shader {
                    None => py.None(),
                    Some(core) => Py::new(
                        py,
                        crate::shader::Shader::wrap(py, core.clone(), |id| self.handle_to(id)),
                    )?
                    .into_any(),
                },
                "hit_testable" => any(node.hit_testable.into_pyobject(py)?.to_owned().into_any()),
                "window_region" => match node.window_region {
                    WindowRegion::Default => py.None(),
                    WindowRegion::Drag => any("drag".into_pyobject(py)?.into_any()),
                    WindowRegion::NoDrag => any("none".into_pyobject(py)?.into_any()),
                },
                "blend_mode" => any(node.paint.blend.name().into_pyobject(py)?.into_any()),
                "visible" => any(node.visible.into_pyobject(py)?.to_owned().into_any()),
                "z_index" => any(node.z_index.into_pyobject(py)?.into_any()),
                "mask" => mask_to_py(node.paint.mask.as_deref(), py)?,
                "clip_children" => any(node
                    .paint
                    .clip_children
                    .into_pyobject(py)?
                    .to_owned()
                    .into_any()),
                "sticky" => node.sticky.into_pyobject(py)?.into_any().into(),
                "layout_x" | "layout_y" | "layout_width" | "layout_height" => {
                    drop(tree);
                    let (x, y, w, h) = self.layout_box(py);
                    let value = match name {
                        "layout_x" => x,
                        "layout_y" => y,
                        "layout_width" => w,
                        _ => h,
                    };
                    return Ok(any(value.into_pyobject(py)?.into_any()));
                }
                "placeholder" | "placeholder_fill" | "caret_color" | "selection_fill"
                | "obscured" => {
                    let NodeKind::TextField(state) = &node.kind else {
                        return Err(PyValueError::new_err(format!(
                            "node property `{name}` applies only to a text_input node"
                        )));
                    };
                    let color = |c: Option<Color>| -> PyResult<Py<PyAny>> {
                        c.map_or_else(|| Ok(py.None()), |c| color_to_py(c, py))
                    };
                    match name {
                        "placeholder" => {
                            any(state.placeholder.clone().into_pyobject(py)?.into_any())
                        }
                        "placeholder_fill" => color(state.placeholder_fill)?,
                        "caret_color" => color(state.caret_color)?,
                        "selection_fill" => color(state.selection_fill)?,
                        _ => any(state.obscured.into_pyobject(py)?.to_owned().into_any()),
                    }
                }
                "scrollbar_fill" | "scrollbar_width" => {
                    let NodeKind::ScrollView(state) = &node.kind else {
                        return Err(PyValueError::new_err(format!(
                            "node property `{name}` applies only to a scroll_view node"
                        )));
                    };
                    if name == "scrollbar_width" {
                        any(state.scrollbar_width.into_pyobject(py)?.into_any())
                    } else {
                        match state.scrollbar_fill {
                            Some(c) => color_to_py(c, py)?,
                            None => py.None(),
                        }
                    }
                }
                "palette" => {
                    let NodeKind::Terminal(state) = &node.kind else {
                        return Err(PyValueError::new_err(
                            "node property `palette` applies only to a terminal node",
                        ));
                    };
                    palette_to_py(&state.palette, py)?
                }
                "view_box" => {
                    let NodeKind::Path(state) = &node.kind else {
                        return Err(PyValueError::new_err(
                            "node property `view_box` applies only to a path node",
                        ));
                    };
                    state
                        .view_box
                        .map(|r| (r.x0, r.y0, r.width(), r.height()))
                        .into_pyobject(py)?
                        .into_any()
                        .unbind()
                }
                "layer_placement" => {
                    // Placed at layout: run any pending layout first.
                    drop(tree);
                    self.layout_box(py);
                    let tree = self.tree.borrow();
                    return Ok(tree
                        .overlay_meta(self.id)
                        .and_then(|meta| meta.placed)
                        .and_then(|side| {
                            crate::window_layers::PLACEMENT
                                .iter()
                                .find(|(_, s)| *s == side)
                                .map(|(n, _)| *n)
                        })
                        .into_pyobject(py)?
                        .into_any()
                        .unbind());
                }
                "focused" => {
                    let focused = tree.focused() == Some(self.id);
                    any(focused.into_pyobject(py)?.to_owned().into_any())
                }
                _ => {
                    return Err(PyValueError::new_err(format!(
                        "unknown node property {name:?} -- `get` reads any property `set` \
                         takes, plus kind, focused, layer_placement, layout_x, layout_y, \
                         layout_width, and layout_height"
                    )));
                }
            }
        };
        Ok(value)
    }

    /// M95: the value `name`'s running animation is heading to -- equal to
    /// `get(name)` when nothing is animating it. Animatable properties
    /// only.
    fn get_target(&self, name: &str, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let tree = self.tree.borrow();
        let node = tree
            .get(self.id)
            .ok_or_else(|| PyValueError::new_err("this node has been removed from its window"))?;
        animatable_to_py(node, name, true, py)?.ok_or_else(|| {
            PyValueError::new_err(format!("node property {name:?} isn't animatable"))
        })
    }

    /// M95: stops `name`'s running animation where it is; its
    /// `on_complete` never fires. A no-op when nothing is animating it.
    fn stop_animation(&self, name: &str) -> PyResult<()> {
        let mut tree = self.tree.borrow_mut();
        let node = tree
            .get_mut(self.id)
            .ok_or_else(|| PyValueError::new_err("this node has been removed from its window"))?;
        if stop_animatable(node, name)? {
            Ok(())
        } else {
            Err(PyValueError::new_err(format!(
                "node property {name:?} isn't animatable"
            )))
        }
    }

    /// Moves keyboard focus to this node, firing `unfocus` and `focus` as
    /// any focus change does.
    fn focus(&self, py: Python<'_>) {
        // 0.4.2 M12: focusing scrolls the node into view, from a current
        // layout.
        self.layout_box(py);
        let transition = self.tree.borrow_mut().set_focus_to(self.id);
        if let Some((old, new)) = transition {
            fire_focus_transition(&self.handlers, &self.tree, &self.completions, old, new, py);
        }
        self.fire_scroll_changes(py);
    }

    /// 0.4.2 M12 (issue #24): scrolls every scroll view around this node
    /// just enough to show it, innermost first.
    fn scroll_into_view(&self, py: Python<'_>) {
        self.layout_box(py);
        self.tree.borrow_mut().scroll_into_view(self.id);
        self.fire_scroll_changes(py);
    }
}

impl Node {
    /// 0.4.2 M12: `scroll` events for any scroll view this call moved.
    fn fire_scroll_changes(&self, py: Python<'_>) {
        crate::listeners::fire_scroll_changes(
            &crate::event::NodeContext {
                tree: &self.tree,
                handlers: &self.handlers,
                completions: &self.completions,
            },
            py,
        );
    }

    /// M96: this node's computed box in window space -- `(x, y, width,
    /// height)` -- running any pending layout of its tree first, so it
    /// always matches the current tree. A detached subtree is laid out on
    /// its own, at its content size.
    pub(crate) fn layout_box(&self, py: Python<'_>) -> (f64, f64, f64, f64) {
        let available = |dim: Dimension| match ExpandedDimension::from(dim) {
            ExpandedDimension::Length(v) => AvailableSpace::Definite(v),
            _ => AvailableSpace::MaxContent,
        };
        let (root, size) = {
            let tree = self.tree.borrow();
            let root = tree.root_of(self.id);
            (root, tree.get(root).map(|n| n.layout_style.size))
        };
        if let Some(size) = size {
            let size = taffy::prelude::Size {
                width: available(size.width),
                height: available(size.height),
            };
            crate::node_callbacks::layout(&self.tree, root, size, &self.handlers, py);
        }
        let tree = self.tree.borrow();
        let (x, y) = tree.absolute_position(self.id);
        let layout = tree.layout(self.id);
        (
            x,
            y,
            f64::from(layout.size.width),
            f64::from(layout.size.height),
        )
    }

    /// Applies already-checked changes (`parse_all`) -- the second half of
    /// an atomic `set`, shared with `Window.create`.
    pub(crate) fn apply(&self, changes: Vec<Change>) {
        let mut tree = self.tree.borrow_mut();
        let Some(node) = tree.get_mut(self.id) else {
            return;
        };
        let mut style = None;
        let mut resize_terminal = false;
        let mut reset_rows = false;
        let mut callbacks = Vec::new();
        for change in changes {
            let access = &mut node.access;
            match change {
                Change::Role(role) => access.role = role,
                Change::Label(label) => access.label = label,
                Change::Value(value) => access.value = value,
                Change::ValueMin(min) => access.value_min = min,
                Change::ValueMax(max) => access.value_max = max,
                Change::ValueStep(step) => access.value_step = step,
                Change::Checked(checked) => access.checked = checked,
                Change::Selected(selected) => access.selected = selected,
                Change::Expanded(expanded) => access.expanded = expanded,
                Change::Disabled(disabled) => access.states.disabled = disabled,
                Change::Level(level) => access.level = level,
                Change::Live(live) => access.live = live,
                Change::A11yHidden(hidden) => access.hidden = hidden,
                Change::Pressed(pressed) => access.pressed = pressed,
                Change::Invalid(invalid) => access.invalid = invalid,
                Change::Busy(busy) => access.busy = busy,
                Change::Current(current) => access.current = current,
                Change::Description(description) => access.description = description,
                Change::ValueNow(Some(now)) => access.value = Some(AccessValue::Number(now)),
                Change::ValueNow(None) => {
                    if matches!(access.value, Some(AccessValue::Number(_))) {
                        access.value = None;
                    }
                }
                Change::ValueText(text) => access.edit_extra(|x| x.value_text = text),
                Change::DescribedBy(ids) => access.edit_extra(|x| x.described_by = ids),
                Change::Controls(ids) => access.edit_extra(|x| x.controls = ids),
                Change::Focusable(focusable) => access.focusable = Some(focusable),
                Change::TabIndex(index) => access.tab_index = index,
                Change::Cursor(cursor) => node.cursor = cursor,
                Change::WindowRegion(region) => node.window_region = region,
                Change::HitTestable(hit_testable) => node.hit_testable = hit_testable,
                Change::Shader(shader) => node.shader = shader,
                Change::Style(edit) => edit(style.get_or_insert_with(|| node.layout_style.clone())),
                Change::Kind(kind_change) => {
                    resize_terminal |= kind_change.resizes_terminal;
                    reset_rows |= kind_change.resets_rows;
                    let old_ratio = match &node.kind {
                        NodeKind::Svg(state) => Some(state.document.width / state.document.height),
                        _ => None,
                    };
                    (kind_change.edit)(node);
                    // A document's shape sizes the node's other side, unless
                    // the app chose an aspect ratio of its own.
                    if let (Some(ratio), Some(old)) = (kind_change.svg_ratio, old_ratio) {
                        let current = style
                            .as_ref()
                            .map_or(node.layout_style.aspect_ratio, |s| s.aspect_ratio);
                        if current.is_none_or(|c| (f64::from(c) - old).abs() < 1e-4) {
                            style
                                .get_or_insert_with(|| node.layout_style.clone())
                                .aspect_ratio = Some(ratio as f32);
                        }
                    }
                }
                Change::Callback(key, callback) => {
                    if key == HandlerKey::SizeHint
                        && let NodeKind::VirtualList(state) = &mut node.kind
                    {
                        state.item_extent = engine_core::ItemExtent::Variable;
                        state.resolved_offsets.clear();
                        reset_rows = true;
                    }
                    reset_rows |= key == HandlerKey::Materialize;
                    callbacks.push((key, callback));
                }
                Change::Visible(visible) => {
                    node.visible = visible;
                    style
                        .get_or_insert_with(|| node.layout_style.clone())
                        .display = if visible {
                        taffy::Display::DEFAULT
                    } else {
                        taffy::Display::None
                    };
                }
                Change::ZIndex(z) => node.z_index = z,
                Change::ClipChildren(clip) => node.paint.clip_children = clip,
                Change::Mask(mask) => node.paint.mask = mask,
                Change::Sticky(inset) => node.sticky = inset,
                Change::TranslateX(v) => node.paint.node_transform.translate_x = Animated::new(v),
                Change::TranslateY(v) => node.paint.node_transform.translate_y = Animated::new(v),
                Change::Scale(v) => node.paint.node_transform.scale = Animated::new(v),
                Change::RotationDeg(v) => {
                    node.paint.node_transform.rotation_deg = Animated::new(v);
                }
                Change::Data(data) => {
                    if let NodeKind::Path(state) = &mut node.kind {
                        state.data = Animated::new(data);
                    }
                }
                Change::ViewBox(view_box) => {
                    if let NodeKind::Path(state) = &mut node.kind {
                        state.view_box = view_box;
                    }
                }
                Change::TrimStart(start) => {
                    if let NodeKind::Path(state) = &mut node.kind {
                        state.trim_start = Animated::new(start);
                    }
                }
                Change::TrimEnd(end) => {
                    if let NodeKind::Path(state) = &mut node.kind {
                        state.trim_end = Animated::new(end);
                    }
                }
                Change::Fill(color) => {
                    match &mut node.kind {
                        NodeKind::TextField(state) => state.text_tint = Animated::new(color),
                        _ => node.paint.background = Animated::new(color),
                    }
                    // A colour replaces any gradient.
                    node.paint.gradient = None;
                }
                Change::FillGradient(gradient) => {
                    node.paint.gradient = Some(Box::new(Animated::new(gradient)));
                }
                Change::StrokeColor(color) => {
                    node.paint.border_color = Animated::new(color);
                    node.paint.border_gradient = None;
                }
                Change::StrokeGradient(gradient) => {
                    node.paint.border_gradient = Some(Box::new(gradient));
                }
                Change::StrokeWidth(width) => node.paint.border_width = Animated::new(width),
                Change::Opacity(opacity) => node.paint.opacity = Animated::new(opacity),
                Change::Blur(blur) => node.paint.blur = Animated::new(blur),
                Change::BackdropBlur(blur) => node.paint.backdrop_blur = Animated::new(blur),
                Change::BlendMode(mode) => node.paint.blend = mode,
                Change::CornerRadius(Radius::Uniform(radius)) => {
                    node.paint.corner_radius = Animated::new(radius);
                    node.paint.corner_radii_override = None;
                }
                Change::CornerRadius(Radius::Corners(corners)) => {
                    node.paint.corner_radii_override = Some(Animated::new(CornerRadii(corners)));
                }
                Change::Shadows(shadows) => {
                    node.paint.shadows = Animated::new(Shadows(shadows));
                }
                Change::Placeholder(text) => {
                    if let NodeKind::TextField(state) = &mut node.kind {
                        state.placeholder = text;
                    }
                }
                Change::PlaceholderFill(color) => {
                    if let NodeKind::TextField(state) = &mut node.kind {
                        state.placeholder_fill = color;
                    }
                }
                Change::CaretColor(color) => {
                    if let NodeKind::TextField(state) = &mut node.kind {
                        state.caret_color = color;
                    }
                }
                Change::SelectionFill(color) => {
                    if let NodeKind::TextField(state) = &mut node.kind {
                        state.selection_fill = color;
                    }
                }
                Change::Obscured(obscured) => {
                    if let NodeKind::TextField(state) = &mut node.kind {
                        state.obscured = obscured;
                    }
                }
                Change::ScrollbarFill(color) => {
                    if let NodeKind::ScrollView(state) = &mut node.kind {
                        state.scrollbar_fill = color;
                    }
                }
                Change::ScrollbarWidth(width) => {
                    if let NodeKind::ScrollView(state) = &mut node.kind {
                        state.scrollbar_width = width;
                    }
                }
                Change::Palette(patch) => {
                    if let NodeKind::Terminal(state) = &mut node.kind {
                        patch.apply(&mut state.palette);
                    }
                }
            }
        }
        // M96: a terminal's box follows its grid and font.
        if resize_terminal && let NodeKind::Terminal(state) = &node.kind {
            let (cols, rows) = (f32::from(state.cols), f32::from(state.rows));
            let (cell_width, cell_height) = crate::shaper::with(|shaper| {
                shaper.monospace_cell_size(&state.font_family, state.font_size)
            });
            let size = &mut style.get_or_insert_with(|| node.layout_style.clone()).size;
            size.width = Dimension::length(cell_width * cols);
            size.height = Dimension::length(cell_height * rows);
        }
        if let Some(style) = style {
            tree.set_layout_style(self.id, style);
        }
        let released = if reset_rows {
            tree.virtual_list_release_outside(self.id, 0..0)
        } else {
            Vec::new()
        };
        for &row in &released {
            tree.detach_collectible(row);
        }
        tree.adopt_text_selection(self.id);
        drop(tree);
        if !callbacks.is_empty() {
            let mut handlers = self.handlers.borrow_mut();
            for (key, callback) in callbacks {
                handlers.insert((self.id, key), (callback, false));
            }
        }
        for row in released {
            crate::node_handles::collect(&self.tree, &self.handlers, row);
        }
    }
}
