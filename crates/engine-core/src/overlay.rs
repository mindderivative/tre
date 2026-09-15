//! §11.3's overlay mechanism (§14 step 13): "menu bars, dropdown menus,
//! context menus, tooltips, and MD3 dialogs are all the same missing
//! primitive... tree-resident, not a parallel structure." `OverlayMeta`
//! is the *only* new bookkeeping this needs -- everything else
//! (positioning, paint order, hit-testing, the focus model,
//! `build_access_update()`) reuses machinery that already exists,
//! per §11.3's own text, which `Tree::open_overlay`/`close_overlay`
//! (in `tree.rs`) exist to prove for real, not merely assert.

use crate::node::NodeId;

/// Matches §11.3's own struct sketch exactly. `dismiss_on_outside_click`/
/// `dismiss_on_escape` are stored as real data here but not yet acted on
/// by any dispatch mechanism -- real pointer/keyboard `InputEvent`/
/// `AppHandler` dispatch and hit-testing (§11.10) don't exist anywhere
/// in this codebase yet (checked directly), so there's nothing to wire
/// dismissal *to* yet. Additive once that dispatch exists, the same
/// "the field is real, the behavior lands with its own step" shape as
/// `Tree::focused` since step 7.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverlayMeta {
    /// Positions the overlay's root relative to this node's computed
    /// bounds.
    pub anchor: NodeId,
    pub dismiss_on_outside_click: bool,
    pub dismiss_on_escape: bool,
}
