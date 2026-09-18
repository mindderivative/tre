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
    /// M30 Phase 4 Step 1 (§11.3): a real, confirmed gap this
    /// milestone's own scoping text already named -- a real modal
    /// dialog must block interaction with everything behind it, which
    /// `dismiss_on_outside_click` alone can't express: `Tree::
    /// dispatch`'s own `PointerPressed` arm only ever consumes a
    /// press outside an overlay when `dismiss_overlays_outside`
    /// actually dismissed something, so a real dialog that wants
    /// "don't dismiss on scrim click, but never let the click reach
    /// the background either" (a real, common MD3 dialog behavior)
    /// had no way to express that combination before this field --
    /// confirmed by reading `dismiss_overlays_outside`'s own real
    /// filter (`meta.dismiss_on_outside_click && ...`) and its one
    /// real call site directly, not assumed from the existing
    /// dismiss-only behavior. `false` (every overlay before this
    /// step -- context menus, dropdown menus, tooltips) is a true
    /// no-op: a press outside a non-modal overlay still falls through
    /// to normal hit-testing exactly as it always has.
    pub modal: bool,
}
