//! §10's `AccessNodeData` -- narrower than the full future model: no
//! states derived automatically from a `NodeKind` payload field yet
//! (`CheckboxState.checked`, etc. -- neither exists yet, §7.3's
//! interaction components land at later build-order steps), so
//! `AccessStates` carries only `disabled`, set directly. `Tree::focused`
//! exists as a plain field so `TreeUpdate.focus` always has a valid
//! value to report -- the *data* this step needs, not §10's full
//! "minimal keyboard focus model" (Tab/Shift-Tab traversal, Enter/Space
//! dispatch): that's real `InputEvent`/`AppHandler` keyboard-dispatch
//! wiring with nothing to dispatch *to* yet (no interactive component
//! exists before §7.3), so it's deferred to whichever step first needs
//! it, not built ahead of that need.
//!
//! M14 Phase 1 (§7.3): `Tree::build_access_update` derives `checked`
//! directly from `NodeKind::Checkbox`'s own real `checked: bool` --
//! deliberately *not* mirrored into a second field here, which would
//! just be two copies of the same fact that could drift out of sync;
//! `CheckboxState.checked` is the one real source of truth, exactly
//! "the app sets one property and the accessibility tree stays correct
//! for free," not "the app sets one property and something else has to
//! remember to copy it."

pub use accesskit::{Action, ActionData, Live, Role};

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
