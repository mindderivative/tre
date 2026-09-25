//! M94: `node.set(**props)` / `node.get(name)` for the accessibility,
//! focus, and interaction properties of the M93 target API, and
//! `node.focus()`. `set` is atomic: every value is parsed and checked
//! before any is applied, so a bad call changes nothing. M96 extends the
//! same two methods to every property.

use engine_core::{AccessValue, Animated, Cursor, Live, NodeKind, PathData, Role};
use peniko::kurbo::Rect;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use taffy::prelude::Dimension;
use taffy::style::ExpandedDimension;

use crate::dispatch::{fire_focus_transition, interaction_config};
use crate::node::Node;

/// Every property `set` accepts today, in the order its error lists them.
const SETTABLE: [&str; 23] = [
    "width",
    "height",
    "data",
    "view_box",
    "trim_start",
    "trim_end",
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
    "focusable",
    "tab_index",
    "cursor",
    "hit_testable",
];

/// The M93 role vocabulary.
const ROLES: [(&str, Role); 23] = [
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
    Focusable(bool),
    TabIndex(i32),
    Cursor(Option<Cursor>),
    HitTestable(bool),
    Width(Dimension),
    Height(Dimension),
    Data(PathData),
    ViewBox(Option<Rect>),
    TrimStart(f64),
    TrimEnd(f64),
}

impl Change {
    /// The property this change writes, for kind errors.
    fn path_only(&self) -> Option<&'static str> {
        match self {
            Change::Data(_) => Some("data"),
            Change::ViewBox(_) => Some("view_box"),
            Change::TrimStart(_) => Some("trim_start"),
            Change::TrimEnd(_) => Some("trim_end"),
            _ => None,
        }
    }
}

/// A size: a number of logical pixels, `"auto"`, or a percentage string.
fn dimension(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Dimension> {
    let expected = "a number, \"auto\", or a percentage like \"50%\"";
    if value.is_instance_of::<pyo3::types::PyBool>() {
        return Err(invalid(name, expected));
    }
    if let Ok(number) = value.extract::<f64>() {
        if number < 0.0 || !number.is_finite() {
            return Err(invalid(name, expected));
        }
        return Ok(Dimension::length(number as f32));
    }
    let text: String = value.extract().map_err(|_| invalid(name, expected))?;
    if text == "auto" {
        return Ok(Dimension::auto());
    }
    text.strip_suffix('%')
        .and_then(|number| number.trim().parse::<f32>().ok())
        .filter(|percent| *percent >= 0.0 && percent.is_finite())
        .map(|percent| Dimension::percent(percent / 100.0))
        .ok_or_else(|| invalid(name, expected))
}

/// A size read back the way it was set.
fn dimension_to_py(dim: Dimension, py: Python<'_>) -> PyResult<Py<PyAny>> {
    Ok(match ExpandedDimension::from(dim) {
        ExpandedDimension::Length(v) => f64::from(v).into_pyobject(py)?.into_any().unbind(),
        ExpandedDimension::Percent(p) => format!("{}%", f64::from(p) * 100.0)
            .into_pyobject(py)?
            .into_any()
            .unbind(),
        _ => "auto".into_pyobject(py)?.into_any().unbind(),
    })
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
    Ok(match name {
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
        "focusable" => Change::Focusable(boolean(value, name)?),
        "tab_index" => Change::TabIndex(required(value, name, "an int")?),
        "cursor" => {
            let cursor: Option<String> = optional(value, name, "a str or None")?;
            Change::Cursor(match cursor {
                None => None,
                Some(cursor) => Some(Cursor::from_name(&cursor).ok_or_else(|| {
                    let valid: Vec<&str> = Cursor::ALL.iter().map(|c| c.name()).collect();
                    invalid(name, &format!("one of: {}", valid.join(", ")))
                })?),
            })
        }
        "hit_testable" => Change::HitTestable(boolean(value, name)?),
        "width" => Change::Width(dimension(value, name)?),
        "height" => Change::Height(dimension(value, name)?),
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
        _ => {
            return Err(PyValueError::new_err(format!(
                "unknown node property {name:?} -- settable: {}",
                SETTABLE.join(", ")
            )));
        }
    })
}

/// Every property `set` accepts, parsed and checked against `kind` --
/// nothing is applied until all of them pass.
pub(crate) fn parse_all(
    props: Option<&Bound<'_, PyDict>>,
    kind: &NodeKind,
) -> PyResult<Vec<Change>> {
    let mut changes = Vec::new();
    if let Some(props) = props {
        for (name, value) in props.iter() {
            let name: String = name.extract()?;
            let change = parse(&name, &value)?;
            if let Some(prop) = change.path_only()
                && !matches!(kind, NodeKind::Path(_))
            {
                return Err(PyValueError::new_err(format!(
                    "node property `{prop}` applies only to a path node"
                )));
            }
            changes.push(change);
        }
    }
    Ok(changes)
}

/// The built-in widget kinds whose legacy numeric `value` `get` keeps
/// reading (the slider's position, a progress indicator's fraction).
fn has_legacy_value(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Slider(_) | NodeKind::LinearProgress(_) | NodeKind::CircularProgress(_)
    )
}

#[pymethods]
impl Node {
    /// Sets any number of properties at once, atomically: every value is
    /// checked first, and a bad one raises `ValueError` without changing
    /// anything. An optional property takes `None` to clear it.
    #[pyo3(signature = (**props))]
    fn set(&self, props: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        let changes = {
            let tree = self.tree.borrow();
            let node = tree.get(self.id).ok_or_else(|| {
                PyValueError::new_err("this node has been removed from its window")
            })?;
            parse_all(props, &node.kind)?
        };
        self.apply(changes);
        Ok(())
    }

    /// Reads one property: any `set` property, `focused`, or -- for the
    /// animatable numeric properties -- its current, possibly
    /// mid-animation value. On a built-in slider or progress indicator,
    /// `value` stays that widget's numeric value.
    fn get(&self, name: &str, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = {
            let tree = self.tree.borrow();
            let node = tree.get(self.id).ok_or_else(|| {
                PyValueError::new_err("this node has been removed from its window")
            })?;
            let access = &node.access;
            let any = |v: Bound<'_, PyAny>| v.unbind();
            match name {
                "role" => ROLES
                    .iter()
                    .find(|(_, r)| *r == access.role)
                    .map(|(n, _)| *n)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind(),
                "label" => access.label.clone().into_pyobject(py)?.into_any().unbind(),
                "value" if !has_legacy_value(&node.kind) => match &access.value {
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
                "focusable" => {
                    let focusable = access.focusable.unwrap_or(!access.actions.is_empty());
                    any(focusable.into_pyobject(py)?.to_owned().into_any())
                }
                "tab_index" => any(access.tab_index.into_pyobject(py)?.into_any()),
                "cursor" => node
                    .cursor
                    .map(Cursor::name)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind(),
                "hit_testable" => any(node.hit_testable.into_pyobject(py)?.to_owned().into_any()),
                "width" => dimension_to_py(node.layout_style.size.width, py)?,
                "height" => dimension_to_py(node.layout_style.size.height, py)?,
                "data" | "view_box" | "trim_start" | "trim_end" => {
                    let NodeKind::Path(state) = &node.kind else {
                        return Err(PyValueError::new_err(format!(
                            "node property `{name}` applies only to a path node"
                        )));
                    };
                    match name {
                        "data" => any(state.data.current.to_svg().into_pyobject(py)?.into_any()),
                        "view_box" => state
                            .view_box
                            .map(|r| (r.x0, r.y0, r.width(), r.height()))
                            .into_pyobject(py)?
                            .into_any()
                            .unbind(),
                        "trim_start" => any(state.trim_start.current.into_pyobject(py)?.into_any()),
                        _ => any(state.trim_end.current.into_pyobject(py)?.into_any()),
                    }
                }
                "focused" => {
                    let focused = tree.focused() == Some(self.id);
                    any(focused.into_pyobject(py)?.to_owned().into_any())
                }
                _ => {
                    drop(tree);
                    return Ok(self
                        .get_number(name)?
                        .into_pyobject(py)?
                        .into_any()
                        .unbind());
                }
            }
        };
        Ok(value)
    }

    /// Moves keyboard focus to this node, firing `blur` and `focus` (and
    /// the legacy focus handlers) as any focus change does.
    fn focus(&self, py: Python<'_>) {
        let config = interaction_config();
        let transition = self.tree.borrow_mut().set_focus_to(
            self.id,
            config.focus_ring_opacity,
            config.focus_ring_duration,
            std::time::Instant::now(),
        );
        if let Some((old, new)) = transition {
            fire_focus_transition(
                &self.handlers,
                &self.tree,
                &self.context_menus,
                &self.theme,
                &self.completions,
                old,
                new,
                py,
            );
        }
    }
}

impl Node {
    /// Applies already-checked changes (`parse_all`) -- the second half of
    /// an atomic `set`, shared with `Window.create`.
    pub(crate) fn apply(&self, changes: Vec<Change>) {
        let mut tree = self.tree.borrow_mut();
        let Some(node) = tree.get_mut(self.id) else {
            return;
        };
        let mut style = None;
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
                Change::Focusable(focusable) => access.focusable = Some(focusable),
                Change::TabIndex(index) => access.tab_index = index,
                Change::Cursor(cursor) => node.cursor = cursor,
                Change::HitTestable(hit_testable) => node.hit_testable = hit_testable,
                Change::Width(width) => {
                    style
                        .get_or_insert_with(|| node.layout_style.clone())
                        .size
                        .width = width;
                }
                Change::Height(height) => {
                    style
                        .get_or_insert_with(|| node.layout_style.clone())
                        .size
                        .height = height;
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
            }
        }
        if let Some(style) = style {
            tree.set_layout_style(self.id, style);
        }
    }
}
