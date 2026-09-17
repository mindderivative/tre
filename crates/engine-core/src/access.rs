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

pub use accesskit::{Action, Role};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AccessStates {
    pub disabled: bool,
}

/// Mirrors ARCHITECTURE.md §10's own sketch exactly.
#[derive(Clone, Debug)]
pub struct AccessNodeData {
    pub role: Role,
    pub label: Option<String>,
    pub description: Option<String>,
    pub states: AccessStates,
    pub actions: Vec<Action>,
}

impl AccessNodeData {
    pub fn new(role: Role) -> Self {
        Self {
            role,
            label: None,
            description: None,
            states: AccessStates::default(),
            actions: Vec::new(),
        }
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
