//! §10's `AccessNodeData` -- a node's accessibility data, which
//! `Tree::build_access_update` turns into an AccessKit `TreeUpdate`.
//! M94 made every field framework-settable through `node.set(...)`
//! (`role`, `label`, `checked`, `value`, ...): the engine has no widget
//! kinds of its own to derive such states from, so the framework that
//! draws a checkbox is the one that says it is checked.

pub use accesskit::{Action, ActionData, AriaCurrent, Live, Role, Toggled};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AccessStates {
    pub disabled: bool,
}

/// M94: an accessible value -- text for a text-like control, a number for
/// a range control (a slider, a progress bar).
#[derive(Clone, Debug, PartialEq)]
pub enum AccessValue {
    Text(String),
    Number(f64),
}

/// 0.5.6 (#160): how a range value is said, and the nodes a node is described
/// by and controls.
#[derive(Clone, Debug, Default)]
pub struct AccessExtra {
    pub value_text: Option<String>,
    pub described_by: Vec<crate::NodeId>,
    pub controls: Vec<crate::NodeId>,
}

impl AccessExtra {
    fn is_empty(&self) -> bool {
        self.value_text.is_none() && self.described_by.is_empty() && self.controls.is_empty()
    }
}

/// Mirrors ARCHITECTURE.md §10's own sketch exactly, plus M94's
/// framework-settable fields -- each `None`/`false` until set through
/// `node.set(...)`, and when set, it wins over anything the engine would
/// derive from a built-in widget kind.
#[derive(Clone, Debug)]
pub struct AccessNodeData {
    pub role: Role,
    pub label: Option<String>,
    pub description: Option<String>,
    pub states: AccessStates,
    pub actions: Vec<Action>,
    pub value: Option<AccessValue>,
    pub value_min: Option<f64>,
    pub value_max: Option<f64>,
    pub value_step: Option<f64>,
    pub checked: Option<bool>,
    pub selected: Option<bool>,
    pub expanded: Option<bool>,
    /// 0.5.6 (#160): a toggle button's state; wins over `checked` when set.
    pub pressed: Option<Toggled>,
    /// 0.5.6 (#160): the value is not acceptable.
    pub invalid: bool,
    /// 0.5.6 (#160): the node is updating.
    pub busy: bool,
    /// 0.5.6 (#160): the current item of a set (a page, a step...).
    pub current: Option<AriaCurrent>,
    /// 0.5.6 (#160): the rarely set, larger accessibility data, boxed so a
    /// node that has none pays one pointer.
    pub extra: Option<Box<AccessExtra>>,
    /// A heading's level, 1 for the most important.
    pub level: Option<usize>,
    pub live: Option<Live>,
    /// Hidden from assistive technology (a decorative node).
    pub hidden: bool,
    /// `Some(true)` makes any node focusable -- by Tab (unless `tab_index`
    /// is negative), by click, and programmatically; `Some(false)` takes
    /// a node out of focus entirely. `None` keeps the legacy rule: a node
    /// is in the Tab order when it offers any action.
    pub focusable: Option<bool>,
    /// The Tab order key: positive values come first, in ascending order,
    /// then `0` in tree order; a negative value keeps the node out of the
    /// Tab order while leaving it focusable by click or `node.focus()`.
    pub tab_index: i32,
}

impl AccessNodeData {
    /// 0.5.6 (#160): changes the boxed extras; the box is dropped again when
    /// nothing is left in it.
    pub fn edit_extra(&mut self, change: impl FnOnce(&mut AccessExtra)) {
        let mut extra = self.extra.take().unwrap_or_default();
        change(&mut extra);
        if !extra.is_empty() {
            self.extra = Some(extra);
        }
    }

    pub fn new(role: Role) -> Self {
        Self {
            role,
            label: None,
            description: None,
            states: AccessStates::default(),
            actions: Vec::new(),
            value: None,
            value_min: None,
            value_max: None,
            value_step: None,
            checked: None,
            selected: None,
            expanded: None,
            pressed: None,
            invalid: false,
            busy: false,
            current: None,
            extra: None,
            level: None,
            live: None,
            hidden: false,
            focusable: None,
            tab_index: 0,
        }
    }

    /// M94: whether this node takes part in Tab navigation.
    pub fn in_tab_order(&self) -> bool {
        let focusable = self.focusable.unwrap_or(!self.actions.is_empty());
        focusable && self.tab_index >= 0
    }

    /// M94: every action offered to assistive technology -- the explicit
    /// `actions`, plus what the role and state imply, so a framework that
    /// sets `role="slider"` and a value range gets increment, decrement,
    /// and set-value without listing them.
    pub fn offered_actions(&self) -> Vec<Action> {
        let mut actions = self.actions.clone();
        let mut offer = |action: Action| {
            if !actions.contains(&action) {
                actions.push(action);
            }
        };
        if self.focusable == Some(true) {
            offer(Action::Focus);
        }
        if matches!(
            self.role,
            Role::Button
                | Role::Link
                | Role::CheckBox
                | Role::RadioButton
                | Role::Switch
                | Role::MenuItem
                | Role::Tab
                | Role::TreeItem
        ) {
            offer(Action::Click);
        }
        let ranged = self.value_min.is_some() || self.value_max.is_some();
        if matches!(self.role, Role::Slider | Role::SpinButton)
            || (ranged && self.role != Role::ProgressIndicator)
        {
            offer(Action::Increment);
            offer(Action::Decrement);
            offer(Action::SetValue);
        }
        if self.expanded.is_some() {
            offer(Action::Expand);
            offer(Action::Collapse);
        }
        actions
    }

    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    #[must_use]
    pub fn with_action(mut self, action: Action) -> Self {
        self.actions.push(action);
        self
    }
}

impl Default for AccessNodeData {
    fn default() -> Self {
        Self::new(Role::Unknown)
    }
}
