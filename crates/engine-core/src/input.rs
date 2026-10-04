//! §4's own generic `InputEvent` enum -- M4 Phase 1 step 1's real
//! dispatch core. `engine-platform` translates raw `winit` events into
//! `InputEvent` and calls `Tree::dispatch` (in `tree/dispatch.rs`); what a real
//! `DispatchOutcome::Activated`/`Changed` *means* (§2 Design Principle
//! 6: meaning-dependent, not mechanical) is the one remaining hook
//! `Tree::dispatch` can't resolve on its own.
//!
//! **Real correction:** this module originally sketched a generic
//! `AppHandler` trait here for that hook (M4 Phase 1 step 1's own
//! original plan) -- it was never actually implemented anywhere; the
//! real mechanism is `engine-py::dispatch.rs`'s own `HandlerMap`/
//! `run_dispatch_outcome` (a real per-`(NodeId, HandlerKey)` Python
//! callback registry, delivered by `listeners.rs`), no generic trait
//! needed at all since only `engine-py` ever calls `Tree::dispatch` in
//! practice).
//! The dead trait was removed once this was confirmed via grep -- kept
//! stated here, not silently dropped, since a stale forward-reference
//! is exactly the kind of drift this project's own doc comments are
//! supposed to catch, not cause.
//!
//! `Key` is deliberately narrow: `Tab`/`Enter`/`Space`/`Escape` only,
//! matching §10's own stated minimal keyboard focus model exactly --
//! not a general key-code/character-input mapping, which nothing here
//! needs before a real text-entry `NodeKind` exists (the same "additive
//! when its own step needs it" discipline `node.rs`'s `NodeKind` already
//! uses).

use peniko::kurbo::Point;

/// The pointer buttons this model distinguishes: primary, secondary,
/// middle, and (0.4.1, issue #21) the mouse's back and forward side
/// buttons, which apps use to navigate. `winit::event::MouseButton` also
/// has `Other(u16)`, which has no common meaning and translates to no
/// `InputEvent` at all -- a stated narrowing, not a silently dropped case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
    /// The mouse's back side button (X1).
    Back,
    /// The mouse's forward side button (X2).
    Forward,
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
    /// 0.4.2 M12 (issue #24): scroll a scroll view by its viewport.
    PageUp,
    PageDown,
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

/// M94: the modifier keys held while an event happens. Tracked by
/// `engine-platform` from `winit`'s own `ModifiersChanged` and delivered as
/// `InputEvent::ModifiersChanged`; `engine-py` keeps the latest value per
/// window and stamps it onto every pointer and key event it routes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
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
    /// 0.5.4 (#115): the OS's reduced-motion preference changed.
    ReducedMotionChanged {
        reduced: bool,
    },
    /// 0.5.4 (#115): the OS's increased-contrast preference changed.
    HighContrastChanged {
        high: bool,
    },
    /// 0.5.4 (#114): the OS is dragging a file over the window. The platform
    /// reports one per file; `position` is where the pointer last was, since
    /// `winit` gives a file drag no position of its own.
    FileHovered {
        path: std::path::PathBuf,
        position: Point,
    },
    /// 0.5.4: the drag left the window or was abandoned.
    FileHoverCancelled,
    /// 0.5.4: a file was dropped on the window (one per file).
    FileDropped {
        path: std::path::PathBuf,
        position: Point,
    },
    /// 0.5.4 (#113): one finger on a touch screen. `id` tells fingers apart for
    /// as long as they are down. `Tree::dispatch` does nothing with it: the
    /// window's gesture recognizer and event delivery do (`engine-py`).
    Touch {
        id: u64,
        phase: crate::TouchPhase,
        position: Point,
    },
    /// 0.5.4 (#113): a trackpad's pinch (macOS and iOS report one directly, as a
    /// magnification step: `1.0 + delta` is the scale to apply).
    TrackpadPinch {
        delta: f64,
        phase: crate::TouchPhase,
        position: Point,
    },
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
    /// layer"). Always lowercase (case-insensitive -- a real Ctrl+
    /// Shift+`<letter>` press is still this same shortcut, the
    /// identical real convention `Copy`/`Cut`/`PasteRequested` already
    /// established, with the one real, stated exception `Terminal
    /// CopyRequested` below carves out of `c` specifically).
    /// Deliberately carries no PTY byte of its own: `engine-core` has
    /// zero OS/platform access (§4), so turning a letter into its own
    /// real ASCII control code (and deciding whether a focused
    /// `Terminal` even exists to send it to) is `engine-py`'s own real
    /// job, the identical "pure intent signal, meaning-dependent
    /// handling downstream" split `Copy`/`Cut`/`PasteRequested`
    /// already use.
    ControlChar(char),
    /// M32 Phase 6 (§4, §5, §8): a real Ctrl+Shift+C press -- the one
    /// real, deliberate exception to `ControlChar`'s own "shift doesn't
    /// change the shortcut" rule, matching every real terminal
    /// emulator's own actual convention (pyCopper's own real `Terminal`
    /// doc comment states this directly: "Ctrl+C is always the
    /// interrupt byte here, never a copy shortcut, since there is
    /// nothing to copy without a selection" -- Ctrl+Shift+C is the
    /// real shortcut that copies instead). `engine_platform::translate_
    /// clipboard_shortcut` produces this only when both Ctrl *and*
    /// Shift are genuinely held and the key is `c`; a bare Ctrl+C stays
    /// `Copy` exactly as before (SIGINT when a real `Terminal` is
    /// focused, `engine-py`'s own existing Phase 4 handling, or a real
    /// `TextField` copy otherwise). Deliberately carries no clipboard
    /// data of its own, the identical "pure intent signal" shape
    /// `PasteRequested` already has -- `engine-core` can't read a real
    /// `Terminal`'s own selected text into a return value here either
    /// (that needs `Tree::terminal_selected_text`, called from `engine-
    /// py`'s own real handling, which also owns the actual OS clipboard
    /// write).
    TerminalCopyRequested,
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
    ///
    /// M96: `delta` keeps winit's own sign -- positive moves the content
    /// right and down, scrolling toward the start. `Tree::dispatch` turns
    /// it into offsets that grow toward the end.
    Scroll {
        delta: ScrollDelta,
        position: Point,
    },
    /// M7 Phase 3 (§7.1): the OS-level light/dark appearance changed --
    /// `winit::WindowEvent::ThemeChanged`, translated in `engine-
    /// platform`. `Tree::dispatch` is a true no-op for this event --
    /// `engine-py` only reports it to `window.on("color_scheme", ...)`
    /// listeners; what it means is the framework's call (M99).
    ThemeChanged {
        dark: bool,
    },
    /// 0.5.0 M2 (issue #28): the window gained or lost the OS's focus --
    /// `winit::WindowEvent::Focused`. Like `ThemeChanged`, `Tree::dispatch`
    /// ignores it; `engine-py` reports it as the window's `active` event, so
    /// a framework's title bar can dim while the window is inactive.
    Focused {
        focused: bool,
    },
    /// M32 Phase 2 (§4, §5): the OS-level window client area genuinely
    /// changed size -- `winit::WindowEvent::Resized`, translated in
    /// `engine-platform`, in the same window-client-pixel space every
    /// other real `InputEvent` here already uses (no DPI-scaling
    /// conversion, matching `PointerMoved`'s own established
    /// precedent). Unlike `ThemeChanged`, `Tree::dispatch` is *not* a
    /// no-op here: resizing `root`'s own `layout_style.size` is a pure
    /// taffy/layout concern `engine-core` fully owns already (no
    /// platform knowledge needed), so the real mutation happens
    /// directly in `dispatch`'s own match, not deferred to `engine-py`.
    Resized {
        width: f32,
        height: f32,
    },
    /// M94: every key press and release, named -- sent alongside (before)
    /// whichever narrow `KeyPressed`/`TextInput`/clipboard event the same
    /// key also produces, so the engine's own text editing and focus
    /// handling are unchanged. `name` is a lowercase snake_case key name
    /// (`"enter"`, `"arrow_left"`, `"f5"`) or, for a character key, the
    /// character it produces (`"a"`, `"A"` with Shift). `Tree::dispatch`
    /// ignores it; `engine-py` routes it as `key_down`/`key_up`.
    Key {
        name: String,
        pressed: bool,
        repeat: bool,
    },
    /// M94: the held modifier keys changed. `Tree::dispatch` ignores it.
    ModifiersChanged(Modifiers),
    /// M94: the window moved to a display with a different scale factor.
    /// `Tree::dispatch` ignores it.
    ScaleFactorChanged {
        scale_factor: f64,
    },
    /// M94: the pointer left the window -- clears hover, so the last
    /// hovered subtree receives its `pointer_leave`.
    PointerLeft,
}

/// M100: what Ctrl+`letter` means, shared by the live keyboard path
/// (`engine-platform`) and `window.simulate`, so both reach the same
/// handling: `c`/`x`/`v` copy, cut, and paste; Ctrl+Shift+C is a
/// terminal's copy; every other ASCII letter is a `ControlChar` (a
/// terminal's control byte, or a text input's select-all for `a`).
/// `None` for anything that isn't a single ASCII letter.
pub fn ctrl_shortcut(letter: char, shift: bool) -> Option<InputEvent> {
    if !letter.is_ascii_alphabetic() {
        return None;
    }
    Some(match letter.to_ascii_lowercase() {
        'c' if shift => InputEvent::TerminalCopyRequested,
        'c' => InputEvent::Copy,
        'x' => InputEvent::Cut,
        'v' => InputEvent::PasteRequested,
        other => InputEvent::ControlChar(other),
    })
}

/// M54 Phase 1 (§8, §16.2): the real, exact set of value shapes a
/// `Changed` outcome's own pre-mutation value can take -- found by
/// tracing every real `DispatchOutcome::Changed` producer in `tree/`
/// before writing this, not assumed: a `TextField` edit (`Backspace`/
/// `Delete`/`Space`/`Enter`/`Tab`/a real typed character) owns a
/// `String`. M99 removed the slider's `Number` and the time picker
/// dial's `Time` with their kinds; a text edit is the only producer.
/// `new_value` is deliberately *not* a sibling field anywhere this
/// type appears: unlike the old value (destroyed by the very mutation
/// that produces this outcome, so it must be captured here, mechanically,
/// or nowhere), the new value is genuinely still live in the `Tree`
/// after `dispatch()` returns -- cheaply, correctly recoverable by
/// `engine-py` reading it back (`dispatch::read_new_changed_value`).
/// Carrying it here too would just be a second copy of data the
/// caller can already read for itself.
#[derive(Clone, Debug, PartialEq)]
pub enum ChangedValue {
    Text(String),
}

/// The one thing `Tree::dispatch` can't resolve by itself (§2 Design
/// Principle 6: it's meaning-dependent, not mechanical) -- everything
/// mechanical (hover, focus movement) already happened inside
/// `dispatch` itself before this is ever produced.
///
#[derive(Clone, Debug, PartialEq)]
pub enum DispatchOutcome {
    /// Nothing meaning-dependent happened this call.
    None,
    /// `NodeId` was activated -- a primary-button pointer click released
    /// over the same node it was pressed on, or `Enter`/`Space` while it
    /// was `Tree::focused()`. What activating a node actually *means*
    /// (deliver `click` to its `node.on(...)` listeners) is
    /// `engine-py::dispatch.rs`'s job, not `Tree`'s -- this module's own doc comment has the full real
    /// reason no generic trait mediates it.
    Activated(crate::NodeId),
    /// M4 Phase 7 (§11.3): the secondary-button (right-click) counterpart
    /// to `Activated` -- a secondary-button pointer click released over
    /// the same node it was pressed on. Separate from `Activated` since
    /// a right-click's real meaning (`secondary_click`, a context menu,
    /// say) is a distinct action from a left-click's, not a variant of
    /// the same one.
    SecondaryActivated(crate::NodeId),
    /// M4 Phase 6 (§7.3): the hovered node genuinely changed this call
    /// -- `old`/`new` are whichever node was/is hovered. Only produced on
    /// a real transition, matching
    /// `update_hover`'s own "repeated call, same result, is a no-op"
    /// contract -- an unchanged hover is not a new fact to report.
    HoverChanged {
        old: Option<crate::NodeId>,
        new: Option<crate::NodeId>,
    },
    /// M14 Phase 3 (§16.7), widened M54 Phase 1 (§8, §16.2): a real
    /// mechanical edit `Tree::dispatch` itself can detect -- a real
    /// `TextField` keyboard edit. `old_value` is the value
    /// immediately *before* this outcome's own mutation, snapshotted at
    /// the one real place that already knows it's about to be
    /// overwritten (see each producer site in `tree/`) -- the only
    /// point it's still genuinely recoverable at all. Delivering it as
    /// a `change` event is `engine-py::dispatch.rs`'s job, not
    /// `Tree`'s.
    Changed {
        node: crate::NodeId,
        old_value: ChangedValue,
    },
    /// M55 (§10, §16.2): keyboard focus genuinely changed this call --
    /// `old`/`new` are whichever node was/is focused, mirroring
    /// `HoverChanged`'s own exact real shape and "only produced on a
    /// real transition" contract (`transition_focus`'s own `old == new`
    /// early return). Produced by `Tree::dispatch`'s own real
    /// click-to-focus (`PointerPressed`) and Tab-navigation
    /// (`KeyPressed`) arms -- a real AccessKit `Action::Focus` request
    /// or a real `Node.focus()` call also change `self.focused` (via
    /// `Tree::set_focus_to`) but never through `Tree::dispatch` at all,
    /// so neither ever produces this variant; `engine-py`'s own direct
    /// callers of `set_focus_to` read its now-widened return instead.
    FocusChanged {
        old: Option<crate::NodeId>,
        new: Option<crate::NodeId>,
    },
}
