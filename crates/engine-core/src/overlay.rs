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
/// `dismiss_on_escape` are real data acted on by `Tree::dispatch`'s own
/// real `PointerPressed`/`KeyPressed` arms (M10 Phase 1) -- real
/// pointer/keyboard `InputEvent` dispatch and hit-testing (§11.10) have
/// existed since M4/M5, respectively; this field waited on `Tree::
/// dismiss_overlays_outside`/`dismiss_escapable_overlays` actually
/// being written, not on the dispatch mechanism itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverlayMeta {
    /// Positions the overlay's root relative to this node's computed
    /// bounds.
    pub anchor: NodeId,
    pub dismiss_on_outside_click: bool,
    pub dismiss_on_escape: bool,
}
