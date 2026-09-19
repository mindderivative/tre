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
/// M15 Phase 2 (§8, §10) widens this with the real named/control keys
/// `TextField` editing needs (`Backspace`/`Delete`/`ArrowLeft`/
/// `ArrowRight`/`Home`/`End`) -- still deliberately minimal, still no
/// general key-code mapping: every printable character reaches `Tree::
/// dispatch` through the sibling `InputEvent::TextInput(String)`
/// variant instead (mirroring `winit::event::KeyEvent`'s own real
/// split between `logical_key`/`text`), not through this enum.
/// `ArrowUp`/`ArrowDown` (M30 Phase 9 Step 3, §10): `Code Editor`'s own
/// real, load-bearing need -- multiline text genuinely can't be
/// navigated by line without them, unlike a single-line `TextField`,
/// which had no real use for either until now (confirmed via grep:
/// no consumer anywhere referenced them before this).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Tab,
    Enter,
    Space,
    Escape,
    Backspace,
    Delete,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
}

/// M4 Phase 8 (§11.7/§11.8 groundwork): mirrors `winit::event::
/// MouseScrollDelta`'s own real two-variant split, verified directly in
/// `winit = "0.30.13"`'s vendored source before writing this --
/// `LineDelta` (a touchpad/wheel notch count) and `PixelDelta` (raw
/// pixels, when the platform/device supports it) are genuinely
/// different units, not two names for the same thing, so collapsing
/// them into one plain `(f64, f64)` would misrepresent real magnitude
/// differences for no real reason -- nothing consumes the value's
/// magnitude yet at all (this phase is input plumbing only), so the
/// honest choice is to preserve the real distinction rather than
/// assume a simplification, the same lesson `PointerButton`'s own past
/// correction already taught this codebase once.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollDelta {
    Lines(f64, f64),
    Pixels(f64, f64),
}

/// The generic input vocabulary `engine-platform` translates real
/// `winit` events into (§4). `position` is already in the same
/// coordinate space `Tree::hit_test`/`Tree::absolute_position` use --
/// window-client pixels, top-left origin -- so `Tree::dispatch` never
/// needs to know anything about `winit`'s own event shapes.
///
/// M15 Phase 2 (§8, §10): no longer `Copy` -- the new `TextInput
/// (String)` variant owns a real, non-`Copy` `String` (mirroring
/// `winit::event::KeyEvent.text: Option<SmolStr>`'s own real produced-
/// text payload). Every real caller already takes `InputEvent` by
/// value, confirmed via grep before this change, so dropping `Copy`
/// (keeping `Clone`) needed no call-site rewrites.
#[derive(Clone, Debug, PartialEq)]
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
    /// M15 Phase 2 (§8, §10): a real, produced *character* keypress --
    /// mirrors `winit::event::KeyEvent.text: Option<SmolStr>` exactly
    /// (confirmed via direct source read of the pinned `winit =
    /// "0.30.13"`), fired for a printable-character keypress that
    /// `translate_key` doesn't already claim as a named/control key.
    /// Only meaningful when a `NodeKind::TextField` is the `Tree`'s own
    /// real focused node -- a true no-op otherwise, the same "mechanism
    /// only, engine-core never knows meaning" shape every other real
    /// dispatch already follows (Design Principle 6).
    TextInput(String),
    /// M17 Phase 1 (§8): a real Ctrl+C press -- fired by `engine-
    /// platform` (checked *before* the `TextInput` fallback above,
    /// fixing a real latent bug that predates this phase: `winit`'s own
    /// `logical_key` is documented as "affected by all modifiers except
    /// Ctrl," so a bare Ctrl+C press produces `Character("c")`, exactly
    /// like an unmodified `c` -- without this check, it would have
    /// silently inserted a literal "c" instead). Deliberately carries
    /// no clipboard data: `engine-core` has zero OS/platform access
    /// (§4's crate-boundary rule) and can't touch a real clipboard --
    /// this is a pure intent signal `Tree::dispatch` never resolves
    /// itself (see `DispatchOutcome`'s own doc comment), left for
    /// `engine-py`'s own real handling, the same "meaning-dependent
    /// outcome" split `Activated` already established.
    Copy,
    /// M17 Phase 1 (§8): `Copy`'s own real Ctrl+X sibling -- same real
    /// reasoning, same fix for the same latent bug.
    Cut,
    /// M17 Phase 1 (§8): a real Ctrl+V press. Named `PasteRequested`,
    /// not `Paste`, since -- unlike `Copy`/`Cut`, which only ever *read*
    /// real `Tree` state `engine-core` already owns -- a real paste
    /// needs content from the actual OS clipboard, which `engine-core`
    /// can never reach; this only ever signals "the user asked to
    /// paste," and `engine-py`'s own real handling supplies the actual
    /// text afterward via the already-real `TextInput` mechanism above.
    PasteRequested,
    /// M32 Phase 4 (§4, §8): a real Ctrl+`<letter>` press for any
    /// letter besides `c`/`x`/`v` (already `Copy`/`Cut`/`PasteRequested`
    /// above, unchanged) -- `engine_platform::translate_clipboard_
    /// shortcut`'s own real detection widened to the full alphabet,
    /// the identical real fix `crates/engine-py/src/terminal.rs`'s own
    /// `input_bytes_for` doc comment already named as this exact,
    /// stated gap ("`engine_core::InputEvent` carries no real
    /// modifier-key state... reaching a focused terminal today
    /// requires a real, separate change to that earlier translation
    /// layer"). Always lowercase (case-insensitive, the identical real
    /// "a real Ctrl+Shift+`<letter>` press is the same shortcut"
    /// convention `Copy`/`Cut`/`PasteRequested` already established).
    /// Deliberately carries no PTY byte of its own: `engine-core` has
    /// zero OS/platform access (§4), so turning a letter into its own
    /// real ASCII control code (and deciding whether a focused
    /// `Terminal` even exists to send it to) is `engine-py`'s own real
    /// job, the identical "pure intent signal, meaning-dependent
    /// handling downstream" split `Copy`/`Cut`/`PasteRequested`
    /// already use.
    ControlChar(char),
    /// M17 Phase 2 (§8): a real IME composition preview update --
    /// mirrors `winit::event::Ime::Preedit`'s own text (dropping its
    /// real sub-cursor-range detail, a stated simplification -- see
    /// `TextFieldState.preedit`'s own doc comment). An empty string
    /// means "the preview was cleared," `winit`'s own real convention
    /// for this event, reused verbatim rather than a separate variant.
    /// `Tree::dispatch` only ever sets/clears the focused `TextField`'s
    /// own `preedit` -- a real mutation, but never a `Change` (nothing
    /// has actually been typed yet). A real IME `Commit` needs no
    /// sibling variant here at all: it reaches the exact same
    /// `TextInput` above, the identical mechanism a plain keypress
    /// already uses (M15 Phase 2).
    ImePreedit(String),
    /// M4 Phase 8: `position` is the cursor's last known position (the
    /// same `last_cursor_position` tracking `MouseInput` already
    /// reuses in `engine-platform`, since `winit`'s own `MouseWheel`
    /// carries no position either) -- a future scroll-to-node wiring
    /// will need to know which node the cursor is over. `Tree::
    /// dispatch` is deliberately a true no-op for this event today
    /// (plumbing only, §11.7/§11.8's own still-open scrollable-viewport
    /// gap isn't built yet).
    Scroll {
        delta: ScrollDelta,
        position: Point,
    },
    /// M7 Phase 3 (§7.1): the OS-level light/dark appearance changed --
    /// `winit::WindowEvent::ThemeChanged`, translated in `engine-
    /// platform`. `Tree::dispatch` is a true no-op for this event, the
    /// same "plumbing only" shape `Scroll` above already established --
    /// resolving a new theme color and pushing it into every node's
    /// `InteractionState::tint` is `engine-py`'s own job (`Tree::
    /// set_all_interaction_tints`), not something `Tree::dispatch`
    /// itself has the MD3 context to do.
    ThemeChanged {
        dark: bool,
    },
    /// M32 Phase 2 (§4, §5): the OS-level window client area genuinely
    /// changed size -- `winit::WindowEvent::Resized`, translated in
    /// `engine-platform`, in the same window-client-pixel space every
    /// other real `InputEvent` here already uses (no DPI-scaling
    /// conversion, matching `PointerMoved`'s own established
    /// precedent). Unlike `ThemeChanged`, `Tree::dispatch` is *not* a
    /// no-op here: resizing `root`'s own `layout_style.size` is a pure
    /// taffy/layout concern `engine-core` fully owns already (no MD3
    /// or platform knowledge needed), so the real mutation happens
    /// directly in `dispatch`'s own match, not deferred to `engine-py`.
    Resized {
        width: f32,
        height: f32,
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
    /// M14 Phase 3 (§16.7): a real, genuine edit -- a `Slider` drag
    /// ending (mechanical, detected by `Tree::dispatch` itself, the
    /// same way `HoverChanged` is), or `Node.set_checked` being called
    /// on a `Checkbox` (not mechanical in the same sense -- `engine-
    /// core` never touches `checked` itself, Design Principle 6 -- so
    /// that firing happens directly in `engine-py`, not through `Tree::
    /// dispatch` at all; see `Node.set_checked`'s own doc comment).
    /// §16.7's own real "two-way binding" sugar is built on this.
    Change,
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
    /// M14 Phase 3 (§16.7): a real `Slider` drag genuinely ended (a
    /// primary-button `PointerReleased` while `Tree`'s own internal
    /// drag-tracking held this `NodeId`) -- the mechanical half of a
    /// real edit `Tree::dispatch` itself can detect, the same way
    /// `HoverChanged` already is; what a real `Change` means (call a
    /// registered handler, or nothing) is still `AppHandler`'s job.
    Changed(crate::NodeId),
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
