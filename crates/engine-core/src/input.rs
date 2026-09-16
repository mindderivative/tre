//! §4's own generic `InputEvent` enum and `AppHandler` trait -- M4 Phase
//! 1 step 1's real dispatch core. `engine-platform` translates raw
//! `winit` events into `InputEvent` and calls `Tree::dispatch` (in
//! `tree.rs`); `AppHandler` is the one remaining meaning-dependent hook
//! `Tree::dispatch` can't resolve on its own (§2 Design Principle 6) --
//! `engine-py` is the crate that actually implements it, since only it
//! can map a `NodeId` back to a registered Python callback.
//!
//! `Key` is deliberately narrow: `Tab`/`Enter`/`Space`/`Escape` only,
//! matching §10's own stated minimal keyboard focus model exactly --
//! not a general key-code/character-input mapping, which nothing here
//! needs before a real text-entry `NodeKind` exists (the same "additive
//! when its own step needs it" discipline `node.rs`'s `NodeKind` already
//! uses).

use peniko::kurbo::Point;

/// The three buttons this minimal model actually distinguishes.
/// **Correction (verified directly against `winit = "0.30.13"`'s own
/// source before this was wired up in `engine-platform`, M4 step 2):**
/// `winit::event::MouseButton` has six real variants (`Left`/`Right`/
/// `Middle`/`Back`/`Forward`/`Other(u16)`), not three -- an earlier
/// version of this doc comment claimed a 1:1 match, which was wrong,
/// not verified against the real enum. `Back`/`Forward`/`Other` have no
/// real MD3 desktop meaning yet (they're a browser-navigation
/// convention) and translate to no `InputEvent` at all -- narrowed,
/// stated, not silently dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
}

/// §10's own minimal keyboard model's exact vocabulary -- see this
/// module's own doc comment for why nothing broader is built yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Tab,
    Enter,
    Space,
    Escape,
}

/// The generic input vocabulary `engine-platform` translates real
/// `winit` events into (§4). `position` is already in the same
/// coordinate space `Tree::hit_test`/`Tree::absolute_position` use --
/// window-client pixels, top-left origin -- so `Tree::dispatch` never
/// needs to know anything about `winit`'s own event shapes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputEvent {
    PointerMoved {
        position: Point,
    },
    PointerPressed {
        position: Point,
        button: PointerButton,
    },
    PointerReleased {
        position: Point,
        button: PointerButton,
    },
    KeyPressed {
        key: Key,
        shift: bool,
    },
    KeyReleased {
        key: Key,
        shift: bool,
    },
}

/// M4 Phase 6 (§16.2): the small, real vocabulary of named events a
/// registered handler can be keyed on -- `Click` (already real since
/// M4 Phase 1) plus `HoverEnter`/`HoverExit` (§7.3's own named pair,
/// "fires... through the ordinary handler path... independent of
/// whether the default MD3 visual [i.e. hover's own opt-in animation]
/// handles it"). Deliberately not the full `Change`/`Focus` set §16.2's
/// own text eventually names -- added only when a real bound component
/// needs one, matching Design Principle 6's own calibration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EventKind {
    Click,
    HoverEnter,
    HoverExit,
}

/// The one thing `Tree::dispatch` can't resolve by itself (§2 Design
/// Principle 6: it's meaning-dependent, not mechanical) -- everything
/// mechanical (hover, focus movement, ripple-spawn-on-press) already
/// happened inside `dispatch` itself before this is ever produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DispatchOutcome {
    /// Nothing meaning-dependent happened this call.
    None,
    /// `NodeId` was activated -- a primary-button pointer click released
    /// over the same node it was pressed on, or `Enter`/`Space` while it
    /// was `Tree::focused()`. What activating a node actually *means*
    /// (call a registered `on_click`, or nothing if none is registered)
    /// is `AppHandler`'s job, not `Tree`'s.
    Activated(crate::NodeId),
    /// M4 Phase 7 (§11.3): the secondary-button (right-click) counterpart
    /// to `Activated` -- a secondary-button pointer click released over
    /// the same node it was pressed on. Separate from `Activated` since
    /// a right-click's real meaning (open a registered context menu, or
    /// nothing if none is registered) is a distinct action from a
    /// left-click's, not a variant of the same one.
    SecondaryActivated(crate::NodeId),
    /// M4 Phase 6 (§7.3): the hovered node genuinely changed this call
    /// -- `old`/`new` are whichever node was/is hovered, independent of
    /// whether either one ever opted into `InteractionState` (§7.3's
    /// own text: the event fires regardless of whether the default
    /// visual is enabled). Only produced on a real transition, matching
    /// `update_hover`'s own "repeated call, same result, is a no-op"
    /// contract -- an unchanged hover is not a new fact to report.
    HoverChanged {
        old: Option<crate::NodeId>,
        new: Option<crate::NodeId>,
    },
}

/// §4's own generic dependency-inversion trait: `engine-platform`'s
/// event loop is generic over this, `engine-py` is the crate that
/// actually implements it (it alone has GIL access and a Python
/// callback map) -- the same shape already used for `BindingResolver`
/// (§16.2) and the `on_complete` completion-queue mechanism (§5).
pub trait AppHandler {
    /// Called once per `DispatchOutcome::Activated` `Tree::dispatch`
    /// produces. Takes no `&mut Tree` -- an activation handler that
    /// wants to mutate the tree (start an animation, change a property)
    /// does so through whatever handle it already holds (`engine-py`'s
    /// own `Node`/`Rc<RefCell<Tree>>`, §9), not through this call.
    fn on_activated(&mut self, node: crate::NodeId);
}
