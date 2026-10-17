//! §11.3's overlay mechanism (§14 step 13): menus, tooltips, and
//! dialogs are all the same primitive -- tree-resident, not a parallel
//! structure. M96's layers (`window.show_layer`) are built on it.
//! `OverlayMeta` is the *only* new bookkeeping this needs -- everything else
//! (positioning, paint order, hit-testing, the focus model,
//! `build_access_update()`) reuses machinery that already exists,
//! per §11.3's own text, which `Tree::open_overlay`/`close_overlay`
//! (in `tree/tests.rs`) exist to prove for real, not merely assert.

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
    /// 0.5.7 (#179): the node's centre on the anchor's top-start corner, for
    /// a badge or a marker. This and the three below, and `Center`, name a
    /// *point* of the anchor and never flip or shift to fit.
    TopStart,
    TopEnd,
    BottomStart,
    BottomEnd,
    /// The node's centre on the anchor's centre.
    Center,
}

impl Placement {
    pub fn opposite(self) -> Self {
        match self {
            Self::Below => Self::Above,
            Self::Above => Self::Below,
            Self::Start => Self::End,
            Self::End => Self::Start,
            // A point has no other side.
            point => point,
        }
    }

    /// 0.5.7 (#179): whether this names a point of the anchor (a corner or
    /// its centre) rather than a side of it.
    pub fn is_point(self) -> bool {
        matches!(
            self,
            Self::TopStart | Self::TopEnd | Self::BottomStart | Self::BottomEnd | Self::Center
        )
    }

    /// 0.5.7 (#179): where a node `w` x `h` goes against `anchor`, as its
    /// top-left corner, before any offset. The four sides put the node outside
    /// the anchor, aligned to the anchor's start edge; a point puts the node's
    /// centre on that point.
    pub fn origin(self, anchor: AnchorBox, w: f64, h: f64) -> (f64, f64) {
        let AnchorBox { x, y, w: aw, h: ah } = anchor;
        let centred = |cx: f64, cy: f64| (cx - w / 2.0, cy - h / 2.0);
        match self {
            Self::Below => (x, y + ah),
            Self::Above => (x, y - h),
            Self::Start => (x - w, y),
            Self::End => (x + aw, y),
            Self::TopStart => centred(x, y),
            Self::TopEnd => centred(x + aw, y),
            Self::BottomStart => centred(x, y + ah),
            Self::BottomEnd => centred(x + aw, y + ah),
            Self::Center => centred(x + aw / 2.0, y + ah / 2.0),
        }
    }
}

/// 0.5.7 (#179): an anchor's box in window space (after its transform), the
/// rectangle a node is placed against.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AnchorBox {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// One open overlay: a legacy one (`Tree::open_overlay`), which closes
/// itself on dismissal, or an M96 layer (`Tree::show_layer`), which reports
/// dismissal instead. `Tree` keeps them in stacking order, newest on top.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
    /// 0.5.7 (#179): moves the point of the anchor the layer attaches to, in
    /// the anchor's own coordinates (a scaled anchor scales it).
    pub offset: (f64, f64),
    /// 0.5.7 (#179): `true` never flips the layer to the other side or shifts
    /// it to fit the window, so it stays where it was placed. A placement that
    /// names a point of the anchor never does either way. `false`, the default,
    /// is what layers always did.
    pub pinned: bool,
    /// M96: the node that held focus when the layer opened, for
    /// `hide_layer` to hand focus back to.
    pub restore_focus: Option<NodeId>,
}
