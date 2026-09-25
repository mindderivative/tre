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
/// M96: the side of its anchor a layer prefers (`OverlayMeta.placement`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Placement {
    #[default]
    Below,
    Above,
    /// Before the anchor on the horizontal axis (left, in left-to-right).
    Start,
    /// After the anchor on the horizontal axis.
    End,
}

impl Placement {
    pub fn opposite(self) -> Self {
        match self {
            Self::Below => Self::Above,
            Self::Above => Self::Below,
            Self::Start => Self::End,
            Self::End => Self::Start,
        }
    }
}

/// One open overlay: a legacy one (`Tree::open_overlay`), which closes
/// itself on dismissal, or an M96 layer (`Tree::show_layer`), which reports
/// dismissal instead. `Tree` keeps them in stacking order, newest on top.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverlayMeta {
    /// Positions the overlay's root relative to this node's computed
    /// bounds. Always set for a legacy overlay; a layer without one sits
    /// at its own `x`/`y`.
    pub anchor: Option<NodeId>,
    pub dismiss_on_outside_click: bool,
    pub dismiss_on_escape: bool,
    /// M30 Phase 4 Step 1 (§11.3): a real modal dialog must block
    /// interaction with everything behind it -- a press outside a modal
    /// overlay never reaches the background, dismissing or not. `false`
    /// is a true no-op: an outside press falls through to normal
    /// hit-testing.
    pub modal: bool,
    /// M96: a layer that reports an outside press or Escape as a
    /// `dismiss` (`Tree::take_dismissals`) rather than closing itself --
    /// what closing means is the framework's call.
    pub dismissible: bool,
    /// M96: the side of `anchor` a layer prefers; placed at every layout,
    /// flipped or shifted to fit the window. `None` for legacy overlays,
    /// placed once when opened.
    pub placement: Option<Placement>,
    /// M96: the side the last layout actually placed the layer on.
    pub placed: Option<Placement>,
    /// M96: the node that held focus when the layer opened, for
    /// `hide_layer` to hand focus back to.
    pub restore_focus: Option<NodeId>,
}
