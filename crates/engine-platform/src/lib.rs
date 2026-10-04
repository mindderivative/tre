//! `winit` `EventLoop`/`ApplicationHandler` and the `accesskit_winit`
//! adapter.
//!
//! §14 build-order step 1: just enough to open a real OS window and pump
//! its event loop for `engine-render`'s own example. No `AppHandler`/
//! `InputEvent` dispatch yet (§4, §9) -- that lands once `engine-core`
//! defines those generic types. Surface/device/queue creation and
//! rendering are the caller's job; this crate only owns the window and
//! its event pump.
//!
//! §14 step 7 adds the real `accesskit_winit::Adapter` wiring (§10: "the
//! only crate depending on `winit`" is also the one that owns
//! `accesskit_winit`, `engine-core` owns the plain `accesskit` data
//! crate and `Tree::build_access_update`). Every window `run_windowed`
//! opens is accessible from the start -- `build_access_update` is a
//! required second closure, not optional, matching §10's own "keyboard
//! operability ships from day one" stance; a caller with no interesting
//! `AccessNodeData` set yet still gets a valid (if minimal, all
//! `Role::Unknown`) tree rather than no tree at all.
//!
//! Uses [`accesskit_winit::Adapter::with_event_loop_proxy`], not
//! `with_direct_handlers`: the direct-handler traits require `Send`
//! (each "may be called on any thread, depending on the underlying
//! platform adapter"), and this crate's own `Tree` is deliberately
//! `Rc<RefCell<Tree>>` (`engine-py`'s own real finding, §9 -- `!Send` by
//! design). The proxy approach keeps only a thin, genuinely `Send`
//! `PlatformEvent` crossing threads; the actual `Tree`-touching
//! `build_access_update` call always happens back on the main thread,
//! inside `user_event`/`window_event`, same as everything else here.
//!
//! §14 step 14 (§11.1) adds real multi-window support:
//! [`run_windowed_multi`] manages any number of simultaneously open
//! windows, each with its own `accesskit_winit::Adapter` and frame
//! counter, keyed by `winit`'s own `WindowId` -- `accesskit_winit::
//! Event` already carries a real `window_id` (confirmed directly in its
//! source), so routing *that* to the right window is a genuine `HashMap`
//! lookup, not new dispatch machinery, matching §11.1's own claim for
//! exactly the two kinds of event this crate has ever dispatched
//! (window-level `winit` events, `accesskit` events) -- real pointer/
//! keyboard `InputEvent` dispatch still doesn't exist anywhere in this
//! codebase, so that part of §11.1's text ("unchanged from the single-
//! window model already designed") stays aspirational until a later
//! step builds it. [`run_windowed`] (the original single-window
//! signature) is now a thin wrapper over [`run_windowed_multi`], kept
//! byte-for-byte source-compatible for its two existing callers
//! (`rect_window.rs`, `access_button.rs`) rather than churning them for
//! a capability neither test needs.
//!
//! Windows are only ever opened up front, via the `setup` closure,
//! before the blocking event loop starts -- not dynamically mid-session
//! from a live external call. Nothing needs that yet (Python's own
//! single call stack can't interleave with the blocking loop without a
//! callback hook this step doesn't build), and `WindowOpener` staying
//! usable only inside `setup` is a real, stated scope limit, not an
//! oversight.
//!
//! **M4 Phase 1 step 2** adds the real translation this module's own
//! doc comment above named as still missing: `WindowEvent::CursorMoved`/
//! `MouseInput`/`KeyboardInput` become `engine_core::InputEvent`, handed
//! to a new `on_input` closure -- the same "just another closure crossing
//! the boundary" shape `on_frame`/`build_access_update` already use, not
//! a new pattern. `engine-platform` never touches a `Tree` itself here;
//! it doesn't have one (generic over whatever the caller does with the
//! translated event, matching `on_frame`'s own existing inversion for
//! rendering). See `translate_pointer_button`/`translate_key`'s own doc
//! comments for the real `winit = "0.30.13"` API facts this translation
//! is built on, verified directly in its source, not assumed.
//!
//! **M4 Phase 2** closes the other real gap step 7's own accesskit
//! wiring left open: `accesskit_winit::WindowEvent::ActionRequested`
//! (a real screen reader naming a node to activate or focus, §10) now
//! reaches a new `on_access_action` closure, handed up as the raw
//! `accesskit::ActionRequest` -- unlike `on_input`, not translated into
//! an `engine_core` type here, since converting `request.target_node`
//! back into a real `NodeId` needs `engine_core::from_access_id`, which
//! only a caller that already depends on `engine-core` can call.
//! `AccessibilityDeactivated` needs no new handling: every `TreeUpdate`
//! build in this module already goes through `access_adapter::
//! update_if_active`, which already gates on activation state.

pub mod appearance;
pub mod titlebar;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use engine_core::{InputEvent, Key, Modifiers, PointerButton, ScrollDelta};
use peniko::kurbo::Point;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

/// `winit`'s mouse button as `engine_core::PointerButton`: left, right, and
/// middle as primary, secondary, and middle, and (0.4.1, issue #21) the
/// back and forward side buttons as themselves. `Other(n)` -- no common
/// meaning -- translates to `None`, no `InputEvent` at all.
fn translate_pointer_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        // 0.4.1 (issue #21): the side buttons, for back/forward navigation.
        MouseButton::Back => Some(PointerButton::Back),
        MouseButton::Forward => Some(PointerButton::Forward),
        MouseButton::Other(_) => None,
    }
}

/// `engine_core::Key`'s own deliberately minimal vocabulary (§10) --
/// every other `winit` key, including every printable character,
/// produces `None` (no `KeyPressed`/`KeyReleased` `InputEvent` at all
/// -- a printable character instead reaches `InputEvent::TextInput`
/// below, M15 Phase 2). Matched against `winit::keyboard::Key::Named`,
/// verified directly against `winit`'s own `keyboard.rs` (`NamedKey::
/// {Tab, Enter, Space, Escape, Backspace, Delete, ArrowLeft,
/// ArrowRight, ArrowUp, ArrowDown, Home, End}` all real, confirmed
/// variants) before writing this. `ArrowUp`/`ArrowDown` (M30 Phase 9
/// Step 3, §10): `Code Editor`'s own real multiline line-navigation
/// need.
fn translate_key(logical_key: &WinitKey) -> Option<Key> {
    match logical_key {
        WinitKey::Named(NamedKey::Tab) => Some(Key::Tab),
        WinitKey::Named(NamedKey::Enter) => Some(Key::Enter),
        WinitKey::Named(NamedKey::Space) => Some(Key::Space),
        WinitKey::Named(NamedKey::Escape) => Some(Key::Escape),
        WinitKey::Named(NamedKey::Backspace) => Some(Key::Backspace),
        WinitKey::Named(NamedKey::Delete) => Some(Key::Delete),
        WinitKey::Named(NamedKey::ArrowLeft) => Some(Key::ArrowLeft),
        WinitKey::Named(NamedKey::ArrowRight) => Some(Key::ArrowRight),
        WinitKey::Named(NamedKey::ArrowUp) => Some(Key::ArrowUp),
        WinitKey::Named(NamedKey::ArrowDown) => Some(Key::ArrowDown),
        WinitKey::Named(NamedKey::Home) => Some(Key::Home),
        WinitKey::Named(NamedKey::End) => Some(Key::End),
        WinitKey::Named(NamedKey::PageUp) => Some(Key::PageUp),
        WinitKey::Named(NamedKey::PageDown) => Some(Key::PageDown),
        _ => None,
    }
}

/// M17 Phase 1 (§8), widened M32 Phase 4 (§4, §8) and M32 Phase 6 (§4,
/// §5, §8): the real Ctrl+`<letter>` vocabulary -- checked only when
/// the real `ModifiersState::control_key()` is held (the caller's own
/// job), since `logical_key` alone is Ctrl-blind (confirmed via direct
/// source read of `winit`'s own `event.rs`: "This value is affected by
/// all modifiers except Ctrl"). Case-insensitive for the letter itself
/// (`Character("C")`/`Character("c")` are the identical real shortcut),
/// but `shift` is now taken as a real, explicit `bool` (the caller's
/// own already-computed `ModifiersState::shift_key()`), not inferred
/// from the character's own case -- a real, deliberate correctness fix
/// this phase made: relying on `Character` case to detect Shift would
/// conflate a real Shift press with Caps Lock, a genuinely different
/// real modifier `winit`'s own `logical_key` does not distinguish for
/// a letter key. `c`/`x`/`v` keep their own real `Copy`/`Cut`/
/// `PasteRequested` meaning when `shift` is false, unchanged since M17
/// Phase 1; `c` with `shift` true produces the new `TerminalCopyRequested`
/// instead (M32 Phase 6) -- the one real, stated exception to "shift
/// doesn't change the shortcut." Every other single ASCII letter
/// produces `InputEvent::ControlChar` (M32 Phase 4) instead of `None`
/// -- `engine-py`'s own `on_input` decides what a real Ctrl+`<letter>`
/// means downstream. A multi-character `Character` payload (a real, if
/// rare, possibility for some IME/dead-key sequences) or any non-
/// alphabetic character still produces `None`, the same deliberately
/// minimal-vocabulary contract this function always had.
fn translate_clipboard_shortcut(logical_key: &WinitKey, shift: bool) -> Option<InputEvent> {
    let WinitKey::Character(c) = logical_key else {
        return None;
    };
    let mut chars = c.chars();
    let ch = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    // M100: the letter's meaning is shared with `window.simulate`.
    engine_core::ctrl_shortcut(ch, shift)
}

/// M4 Phase 8 (§11.7/§11.8 groundwork): `winit::event::MouseScrollDelta`
/// has exactly two real variants, verified directly in `winit =
/// "0.30.13"`'s vendored `event.rs` before writing this -- `LineDelta`
/// (a touchpad/wheel notch count) and `PixelDelta` (raw pixels, when
/// the platform/device supports it) are genuinely different units, so
/// `engine_core::ScrollDelta` mirrors the real split rather than
/// collapsing it. `PixelDelta`'s own `PhysicalPosition<f64>` -> plain
/// `(f64, f64)` is a field copy, the same "no unit conversion needed"
/// shape `CursorMoved`'s own translation already uses.
/// 0.5.4 (#113): `winit`'s touch phase as the engine's.
fn translate_touch_phase(phase: winit::event::TouchPhase) -> engine_core::TouchPhase {
    match phase {
        winit::event::TouchPhase::Started => engine_core::TouchPhase::Started,
        winit::event::TouchPhase::Moved => engine_core::TouchPhase::Moved,
        winit::event::TouchPhase::Ended => engine_core::TouchPhase::Ended,
        winit::event::TouchPhase::Cancelled => engine_core::TouchPhase::Cancelled,
    }
}

fn translate_scroll_delta(delta: MouseScrollDelta) -> ScrollDelta {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => ScrollDelta::Lines(f64::from(x), f64::from(y)),
        MouseScrollDelta::PixelDelta(position) => ScrollDelta::Pixels(position.x, position.y),
    }
}

/// M7 Phase 3 (§7.1): `winit::window::Theme` collapsed to the single
/// `bool` `InputEvent::ThemeChanged` carries -- factored out as its own
/// free function (matching `translate_pointer_button`/`translate_key`'s
/// own shape) so it's directly unit-testable with no live `EventLoop`.
fn translate_theme(theme: winit::window::Theme) -> bool {
    theme == winit::window::Theme::Dark
}

/// M94: the name `InputEvent::Key` carries for every key -- a named key's
/// `winit` variant in lowercase snake_case (`ArrowLeft` -> `"arrow_left"`,
/// `F5` -> `"f5"`), or a character key's produced character (`"a"`, `"A"`
/// with Shift). `None` for dead and unidentified keys, which produce no
/// `key_down`/`key_up`.
fn key_name(logical_key: &WinitKey) -> Option<String> {
    match logical_key {
        WinitKey::Named(named) => {
            let camel = format!("{named:?}");
            let mut name = String::with_capacity(camel.len() + 4);
            let mut previous: Option<char> = None;
            for ch in camel.chars() {
                if ch.is_ascii_uppercase()
                    && previous.is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit())
                {
                    name.push('_');
                }
                name.push(ch.to_ascii_lowercase());
                previous = Some(ch);
            }
            Some(name)
        }
        WinitKey::Character(text) => Some(text.to_string()),
        WinitKey::Dead(_) | WinitKey::Unidentified(_) => None,
    }
}

/// M94: `winit`'s modifier state as the engine's own `Modifiers`.
fn translate_modifiers(state: ModifiersState) -> Modifiers {
    Modifiers {
        shift: state.shift_key(),
        ctrl: state.control_key(),
        alt: state.alt_key(),
        meta: state.super_key(),
    }
}

/// M94: a window's lifecycle moments `run_windowed_multi`'s
/// `on_lifecycle` callback is told about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowLifecycle {
    /// The user asked to close the window. The callback returns `true`
    /// to close it, `false` to keep it open.
    CloseRequested,
    /// The window is closing now -- closed by the user, or by reaching
    /// its own `max_frames`. The callback's return value is ignored.
    Closed,
}

pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    /// Auto-exit after this many redraws, so a demo built on this is
    /// self-asserting and headless-CI-safe instead of requiring a human
    /// to close the window -- TRE v1's own convention
    /// (LESSONS_LEARNED.md, the "gracefully exit 0" lesson from finding
    /// #261). `None` runs until the user closes the window.
    pub max_frames: Option<u32>,
    /// 0.5.0 M2 (issue #28): how the window is made beyond its title and
    /// size.
    pub options: WindowOptions,
}

/// 0.5.0 M2 (issue #28): a window's creation settings beyond its title and
/// size. `Default` is an ordinary decorated window.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowOptions {
    /// Whether the OS draws the window's title bar and borders. `false`
    /// leaves all of it to the app, which draws its own; on Windows the
    /// window keeps its shadow either way.
    pub decorations: bool,
    /// Whether the window opens maximized.
    pub maximized: bool,
    /// Whether the window opens fullscreen, borderless on its monitor.
    pub fullscreen: bool,
    /// The smallest inner size, `(width, height)` in logical pixels, the
    /// user can resize the window to.
    pub min_size: Option<(f64, f64)>,
    /// The window's icon, straight-alpha RGBA8 with its width and height --
    /// used on Windows and X11; Wayland and macOS take the app's icon from
    /// its desktop file or bundle.
    pub icon: Option<(Vec<u8>, u32, u32)>,
    /// 0.5.4 (#137): whether the window can be see-through: asks the OS for a
    /// window with an alpha channel. Fixed when the window opens (X11 can only
    /// choose it then).
    pub transparent: bool,
    /// 0.5.4 (#137): asks the compositor to blur what is behind the window,
    /// where it can (Wayland with KDE's blur protocol, macOS).
    pub blur: bool,
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            decorations: true,
            maximized: false,
            fullscreen: false,
            min_size: None,
            icon: None,
            transparent: false,
            blur: false,
        }
    }
}

/// 0.5.0 M3 (issue #28): how close together two presses must be to make a
/// double-click -- the user's own setting on Windows and macOS, 500 ms
/// elsewhere, where there's no one system setting to read.
#[cfg(target_os = "windows")]
pub fn double_click_time() -> std::time::Duration {
    // SAFETY: a plain Win32 call with no arguments or preconditions.
    let ms = unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime() };
    std::time::Duration::from_millis(u64::from(ms))
}

/// 0.5.0 M3: see the Windows version.
#[cfg(target_os = "macos")]
pub fn double_click_time() -> std::time::Duration {
    // SAFETY: a class property read with no preconditions.
    let seconds = unsafe { objc2_app_kit::NSEvent::doubleClickInterval() };
    std::time::Duration::from_secs_f64(seconds)
}

/// 0.5.0 M3: see the Windows version.
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn double_click_time() -> std::time::Duration {
    std::time::Duration::from_millis(500)
}

/// 0.5.0 M2 (issue #28): which windowing system `window` runs on --
/// `"wayland"`, `"x11"`, `"windows"`, or `"macos"` -- for a framework whose
/// title bar behaves differently on each. On Linux the open window's own
/// handle answers; before one opens, it's the backend `winit` would pick
/// (`WINIT_UNIX_BACKEND` if set, else Wayland when `WAYLAND_DISPLAY` is).
pub fn platform_name(window: Option<&Window>) -> &'static str {
    if cfg!(target_os = "windows") {
        return "windows";
    }
    if cfg!(target_os = "macos") {
        return "macos";
    }
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if let Some(handle) = window.and_then(|w| w.window_handle().ok()) {
        return match handle.as_raw() {
            RawWindowHandle::Wayland(_) => "wayland",
            _ => "x11",
        };
    }
    match std::env::var("WINIT_UNIX_BACKEND").as_deref() {
        Ok("x11") => "x11",
        Ok("wayland") => "wayland",
        _ if std::env::var_os("WAYLAND_DISPLAY").is_some() => "wayland",
        _ => "x11",
    }
}

/// One window-open request, tagged with a caller-assigned `token` so
/// [`run_windowed_multi`]'s `on_window_created` callback can correlate
/// the real `WindowId` it's handed back with whatever the caller
/// originally asked for (e.g. `engine-py`'s `App` uses this to know
/// which registered `PyWindow` a newly created OS window belongs to).
pub struct WindowRequest {
    pub config: WindowConfig,
    pub token: u64,
}

/// The winit user-event type this crate's `EventLoop` is built with --
/// exists purely to carry `accesskit_winit::Event`, window-open
/// requests, and real cross-thread wake requests across the proxy
/// boundary (see the module doc comment for why `with_event_loop_
/// proxy`, not the direct-handler API).
enum PlatformEvent {
    AccessKit(accesskit_winit::Event),
    OpenWindow(WindowRequest),
    /// M31 Phase 6 (§5, §6): a real, generic "something changed
    /// outside the event loop's own thread, redraw" signal -- closes
    /// the real, stated v1 cost M30 Phase 9 Step 4 (Terminal) found
    /// and left open (a background PTY reader thread producing new
    /// output had no other way to wake an otherwise-idle
    /// `ControlFlow::Wait` loop, so that step widened `any_active` to
    /// keep continuously polling instead). Deliberately untargeted
    /// (no `WindowId` payload) -- redraws every real open window, the
    /// same real "whole-loop signal, not per-window" shape `any_active`
    /// itself already has, confirmed as the simpler, still-correct v1
    /// answer this phase's own scoping note left as an open question.
    Wake,
    /// M106 (issue #18): the XDG settings portal announced a new light/
    /// dark appearance -- Linux's stand-in for `winit`'s own
    /// `WindowEvent::ThemeChanged`, which never fires there. Every open
    /// window is told, as the OS tells each window on macOS and Windows.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    ThemeChanged(bool),
    /// 0.5.4 (#115): the portal announced a new reduced-motion or contrast
    /// preference; every open window is told.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    ReducedMotionChanged(bool),
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    HighContrastChanged(bool),
    /// 0.5.0 M2 (issue #28): the app asked to close a window
    /// (`EventLoopWaker::close_window`) -- handled exactly as the user
    /// closing it, `CloseRequested` first, which the app may refuse.
    CloseWindow(WindowId),
    /// 0.5.0 M2: report window `id`'s current size as a resize --
    /// `EventLoopWaker::report_size`.
    ReportSize(WindowId),
}

impl From<accesskit_winit::Event> for PlatformEvent {
    fn from(event: accesskit_winit::Event) -> Self {
        Self::AccessKit(event)
    }
}

/// A handle for requesting new windows, valid only inside the `setup`
/// closure `run_windowed_multi` calls once, before the event loop
/// starts running -- see the module doc comment for why this doesn't
/// (yet) support opening a window later, mid-session.
pub struct WindowOpener {
    proxy: EventLoopProxy<PlatformEvent>,
}

impl WindowOpener {
    pub fn open_window(&self, request: WindowRequest) {
        let _ = self.proxy.send_event(PlatformEvent::OpenWindow(request));
    }
}

/// M31 Phase 6 (§5, §6): a real, `Send` + `Clone` handle any background
/// producer can hold onto (unlike `WindowOpener`, whose own doc comment
/// states it's valid only inside `setup`) -- `run_windowed_multi`'s
/// `setup` closure is the one real place to hand a clone of this to
/// whatever already-constructed background thread needs it (e.g. a
/// real `TerminalSession`'s own PTY reader thread), since nothing
/// dynamically created later during the loop's own run has a way to
/// reach back into `setup` again (the identical real "no mid-session
/// window-opening yet" limitation `WindowOpener`'s own doc comment
/// already states, for the same underlying reason: `setup` runs
/// exactly once). `EventLoopProxy` is real, confirmed `Send` (why
/// `accesskit_winit`'s own cross-thread wiring already relies on it)
/// and `Clone`, so this wrapper costs nothing beyond what the proxy
/// itself already provides.
#[derive(Clone)]
pub struct EventLoopWaker {
    proxy: EventLoopProxy<PlatformEvent>,
}

impl EventLoopWaker {
    /// Requests a real redraw of every currently open window --
    /// silently, safely no-ops if the loop has already exited, the
    /// identical real error-handling convention `WindowOpener::
    /// open_window`'s own body already establishes (`let _ = ...`).
    /// Safe to call from any thread, any number of times, before or
    /// after the loop itself has started or stopped.
    pub fn wake(&self) {
        let _ = self.proxy.send_event(PlatformEvent::Wake);
    }

    /// 0.5.0 M2 (issue #28): asks the loop to close window `id` as if the
    /// user had -- `on_lifecycle` hears `CloseRequested` first and may keep
    /// it open. Handled on the loop's next turn, so a caller already inside
    /// one of the loop's callbacks isn't re-entered. A no-op for a window
    /// already closed, or once the loop has exited.
    pub fn close_window(&self, id: WindowId) {
        let _ = self.proxy.send_event(PlatformEvent::CloseWindow(id));
    }

    /// 0.5.0 M2: has the loop report window `id`'s current size to
    /// `on_input` as an `InputEvent::Resized`, on its next turn -- for a size
    /// the app changed itself, which `winit` may apply at once without a
    /// `WindowEvent::Resized` (Wayland does).
    pub fn report_size(&self, id: WindowId) {
        let _ = self.proxy.send_event(PlatformEvent::ReportSize(id));
    }
}

/// Runs a `winit` event loop managing any number of windows.
///
/// `setup` is called once, synchronously, before the loop starts --
/// its only job is to request the initial window(s) via the given
/// [`WindowOpener`]. For each window actually created, `on_window_created`
/// fires exactly once with its real `WindowId`, the `token` from
/// whichever `WindowRequest` produced it, and an owned `Arc<Window>` --
/// the caller's one chance to build (and stash, keyed by `WindowId`) any
/// per-window GPU/render state. It returns whether that worked: `false`
/// (0.4.0: no GPU adapter, say) closes the window and ends the loop at
/// once, for the caller to report. `on_frame` then fires once per redraw
/// with just the `WindowId`, the 0-based frame index (the caller already
/// has everything else from `on_window_created`), and (0.4.0 M6) whether
/// the OS asked for this redraw -- an expose -- rather than this loop
/// (animation, input, a wake), where the two didn't coincide; `build_access_update`
/// fires per window the same way, keeping each window's own exposed
/// accessibility tree in sync (§10). Exits once every window has closed
/// or reached its own `max_frames`.
///
/// M29 Phase 2 (§5, §6): `on_frame`'s own `bool` return -- `true` means
/// "this window is still genuinely animating, call me again next tick
/// with no other trigger needed"; `false` means "nothing left to
/// animate, don't bother scheduling another redraw on my account." This
/// function keeps `ControlFlow::Poll` for as long as *any* open window
/// last reported `true`, and drops to `ControlFlow::Wait` the moment
/// every open window has settled -- real input still reaches a waiting
/// loop exactly as before (`winit` delivers `WindowEvent`s regardless
/// of `ControlFlow`), each real input-handling arm below just has to
/// ask for its own next redraw explicitly now, since nothing else will.
/// A caller with no real animation and no real input (this crate's own
/// `access_button.rs`/`rect_window.rs`/`multi_window.rs` test harnesses,
/// via [`run_windowed`]'s own wrapper) can simply always return `true`
/// to keep its pre-M29 always-polling behavior byte-for-byte.
///
/// **Real finding, caught by actually running every example in the
/// workspace against this change, not assumed:** a window opened with
/// `max_frames: Some(_)` keeps polling toward its own frame count
/// regardless of what `on_frame` itself reports, even if it reports
/// `false` every single frame -- otherwise a static "runs N frames then
/// exits" window (this codebase's own dominant example/test pattern)
/// would never reach `max_frames` once idle, and hang forever under
/// `ControlFlow::Wait` waiting for input that never arrives. Only a
/// genuinely unbounded window (`max_frames: None`) actually needs
/// `on_frame` to report accurately for Phase 2's idle-CPU benefit to
/// apply to it at all.
///
/// Returns `Err` rather than panicking if no display is reachable --
/// see [`run_windowed`]'s own doc comment for why that's expected, not
/// exceptional, on some CI runners.
///
/// `on_input` (M4 Phase 1 step 2) fires once per real pointer/keyboard
/// event this module knows how to translate (`translate_pointer_button`/
/// `translate_key`'s own doc comments name exactly which ones) -- a
/// genuinely new `InputEvent` per call, in the same window-client-pixel
/// coordinate space `Tree::hit_test`/`absolute_position` already use.
/// This function never calls `Tree::dispatch` itself -- it doesn't have
/// a `Tree` (generic over whatever the caller does with the event,
/// matching `on_frame`'s own existing inversion for rendering).
///
/// `on_access_action` (M4 Phase 2, §10) fires once per real
/// `accesskit::ActionRequest` a platform accessibility client sends --
/// "a screen reader focusing a node directly" dispatches `Action::
/// Focus`, activating a control dispatches `Action::Click`. Handed up
/// raw, unlike `on_input`'s already-translated `InputEvent`: this
/// module doesn't know `engine_core::NodeId` exists, and converting
/// `request.target_node` back to one needs `engine_core::
/// from_access_id`, which only a caller that already depends on
/// `engine-core` for everything else can call.
///
/// M31 Phase 6 (§5, §6): `setup` now also receives a real
/// [`EventLoopWaker`] handle -- unlike the `WindowOpener` it already
/// received (valid only inside this one, single call), a caller may
/// clone the waker out to any already-constructed background producer
/// that needs to wake a genuinely idle `ControlFlow::Wait` loop later
/// (a real, live PTY reader thread, for one) -- `setup` is the one
/// real place able to reach both a fresh proxy and any real,
/// already-built per-window state to wire it into.
#[allow(clippy::too_many_arguments)]
pub fn run_windowed_multi<C, F, A, S, N, X, L>(
    on_window_created: C,
    on_frame: F,
    build_access_update: A,
    on_input: N,
    on_access_action: X,
    on_lifecycle: L,
    setup: S,
) -> Result<(), winit::error::EventLoopError>
where
    C: FnMut(WindowId, u64, Arc<Window>) -> bool,
    F: FnMut(WindowId, u32, bool) -> bool,
    A: FnMut(WindowId) -> accesskit::TreeUpdate,
    N: FnMut(WindowId, InputEvent),
    X: FnMut(WindowId, accesskit::ActionRequest),
    L: FnMut(WindowId, WindowLifecycle) -> bool,
    S: FnOnce(&WindowOpener, &EventLoopWaker),
{
    run_windowed_multi_with(
        on_window_created,
        on_frame,
        build_access_update,
        on_input,
        on_access_action,
        on_lifecycle,
        setup,
        None,
    )
}

/// 0.5.1 (#92): what a host that holds a lock the loop's *waiting* shouldn't
/// (Python's GIL) gives the loop: `release` is called right before the loop
/// goes to wait for the next event, and `reacquire` as soon as it wakes --
/// before any of the callbacks above run. Nothing the callbacks do needs the
/// lock while the loop is between the two.
pub struct IdleHooks {
    pub release: Box<dyn FnMut()>,
    pub reacquire: Box<dyn FnMut()>,
}

/// [`run_windowed_multi`] with [`IdleHooks`] around the loop's wait.
#[allow(clippy::too_many_arguments)]
pub fn run_windowed_multi_with<C, F, A, S, N, X, L>(
    on_window_created: C,
    on_frame: F,
    build_access_update: A,
    on_input: N,
    on_access_action: X,
    on_lifecycle: L,
    setup: S,
    idle: Option<IdleHooks>,
) -> Result<(), winit::error::EventLoopError>
where
    C: FnMut(WindowId, u64, Arc<Window>) -> bool,
    F: FnMut(WindowId, u32, bool) -> bool,
    A: FnMut(WindowId) -> accesskit::TreeUpdate,
    N: FnMut(WindowId, InputEvent),
    X: FnMut(WindowId, accesskit::ActionRequest),
    L: FnMut(WindowId, WindowLifecycle) -> bool,
    S: FnOnce(&WindowOpener, &EventLoopWaker),
{
    let event_loop = EventLoop::<PlatformEvent>::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let proxy = event_loop.create_proxy();

    #[cfg(target_os = "linux")]
    {
        let proxy = proxy.clone();
        appearance::portal::watch(move |change| {
            use appearance::portal::Change;
            let event = match change {
                Change::Dark(dark) => PlatformEvent::ThemeChanged(dark),
                Change::ReducedMotion(reduced) => PlatformEvent::ReducedMotionChanged(reduced),
                Change::HighContrast(high) => PlatformEvent::HighContrastChanged(high),
            };
            proxy.send_event(event).is_ok()
        });
    }

    setup(
        &WindowOpener {
            proxy: proxy.clone(),
        },
        &EventLoopWaker {
            proxy: proxy.clone(),
        },
    );

    let mut app = MultiWindowApp {
        windows: HashMap::new(),
        proxy,
        on_window_created,
        on_frame,
        build_access_update,
        on_input,
        on_access_action,
        on_lifecycle,
        idle,
        released: false,
    };
    let result = event_loop.run_app(&mut app);
    // The loop may end right after a wait began: take the lock back.
    app.attach();
    result
}

/// The original single-window entry point, kept source-compatible for
/// its two existing callers -- a thin wrapper over
/// [`run_windowed_multi`] that stashes the one `Arc<Window>` it gets
/// from `on_window_created` and hands it back to `on_frame` every
/// redraw, matching this function's own pre-step-14 signature exactly.
pub fn run_windowed<F, A>(
    config: WindowConfig,
    mut on_frame: F,
    mut build_access_update: A,
) -> Result<(), winit::error::EventLoopError>
where
    F: FnMut(&Arc<Window>, u32),
    A: FnMut() -> accesskit::TreeUpdate,
{
    let window: Rc<RefCell<Option<Arc<Window>>>> = Rc::new(RefCell::new(None));
    let window_for_created = window.clone();

    run_windowed_multi(
        move |_id, _token, created| {
            *window_for_created.borrow_mut() = Some(created);
            true
        },
        move |_id, frame, _os_requested| {
            let window = window
                .borrow()
                .clone()
                .expect("on_window_created always fires before on_frame for the same window");
            on_frame(&window, frame);
            // M29 Phase 2: this wrapper's own two existing callers
            // (`rect_window.rs`/`access_button.rs`) render unconditionally
            // every frame already, with no animation-aware concept of
            // their own -- always reporting "still animating" keeps
            // them polling exactly as before this phase, byte-for-byte.
            true
        },
        move |_id| build_access_update(),
        // Neither of this wrapper's two existing callers (`rect_window.rs`,
        // `access_button.rs`) needs input events -- a no-op keeps
        // `run_windowed`'s own signature untouched, matching this
        // function's own doc comment's "byte-for-byte source-compatible"
        // contract.
        |_id, _event| {},
        |_id, _request| {},
        |_id, _lifecycle| true,
        |opener, _waker| {
            opener.open_window(WindowRequest { config, token: 0 });
        },
    )
}

struct PerWindow {
    window: Arc<Window>,
    /// 0.4.0 M6: whether the pending redraw is one this loop asked for
    /// (animation, input, a wake) rather than the OS (an expose) -- read
    /// and cleared at `RedrawRequested`, and handed to `on_frame`.
    app_requested: bool,
    access_adapter: accesskit_winit::Adapter,
    frame: u32,
    max_frames: Option<u32>,
    /// M4 Phase 1 step 2: `KeyEvent` doesn't carry modifier state itself
    /// -- `WindowEvent::ModifiersChanged` is a separate event -- so this
    /// is updated there and read (via `.shift_key()`) when a
    /// `KeyboardInput` needs to know whether it's really Tab or
    /// Shift-Tab.
    modifiers: ModifiersState,
    /// M4 Phase 1 step 2: `winit`'s own `MouseInput` carries no position
    /// -- it always corresponds to wherever the most recent `CursorMoved`
    /// put the pointer -- so this is tracked here and read when
    /// translating a press/release into an `InputEvent`.
    last_cursor_position: Point,
    /// 0.5.4 (#115): the OS preferences as last seen, to report a change when
    /// the window regains focus (Windows and macOS announce none).
    reduced_motion: Option<bool>,
    high_contrast: Option<bool>,
    /// M29 Phase 2 (§5, §6): this window's own last-reported `on_frame`
    /// return -- `true` until the first real `RedrawRequested` settles
    /// it, so a freshly created window (which already gets one explicit
    /// `request_redraw()` call, below) doesn't accidentally drop the
    /// whole loop to `ControlFlow::Wait` before it's ever painted once.
    animating: bool,
}

impl PerWindow {
    /// Asks for a redraw, noting that this loop asked for it.
    fn request_redraw(&mut self) {
        self.app_requested = true;
        self.window.request_redraw();
    }
}

struct MultiWindowApp<C, F, A, N, X, L> {
    windows: HashMap<WindowId, PerWindow>,
    proxy: EventLoopProxy<PlatformEvent>,
    on_window_created: C,
    on_frame: F,
    build_access_update: A,
    on_input: N,
    on_access_action: X,
    on_lifecycle: L,
    /// 0.5.1 (#92): see [`IdleHooks`].
    idle: Option<IdleHooks>,
    /// Whether `idle.release` has been called with no `reacquire` since.
    released: bool,
}

impl<C, F, A, N, X, L> MultiWindowApp<C, F, A, N, X, L> {
    /// Takes back what `park` gave up, if it did. Every handler starts with
    /// this, so a callback never runs while the host's lock is released,
    /// whatever order the platform delivers the loop's events in.
    fn attach(&mut self) {
        if self.released {
            self.released = false;
            if let Some(idle) = &mut self.idle {
                (idle.reacquire)();
            }
        }
    }

    /// Gives the host's lock up for the wait that follows.
    fn park(&mut self) {
        if !self.released
            && let Some(idle) = &mut self.idle
        {
            self.released = true;
            (idle.release)();
        }
    }
}

impl<C, F, A, N, X, L> ApplicationHandler<PlatformEvent> for MultiWindowApp<C, F, A, N, X, L>
where
    C: FnMut(WindowId, u64, Arc<Window>) -> bool,
    F: FnMut(WindowId, u32, bool) -> bool,
    A: FnMut(WindowId) -> accesskit::TreeUpdate,
    N: FnMut(WindowId, InputEvent),
    X: FnMut(WindowId, accesskit::ActionRequest),
    L: FnMut(WindowId, WindowLifecycle) -> bool,
{
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: winit::event::StartCause) {
        self.attach();
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.park();
    }

    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        self.attach();
        // Windows are created lazily, in `user_event`, as `OpenWindow`
        // requests arrive -- not eagerly here. `setup`'s own
        // `open_window` calls (sent before the loop starts) are queued
        // on the proxy and delivered as the very first `user_event`
        // calls once the loop is actually running.
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: PlatformEvent) {
        self.attach();
        match event {
            PlatformEvent::OpenWindow(request) => {
                // accesskit_winit's own hard requirement: the adapter
                // must be created before the window is ever shown,
                // which means creating the window invisible first.
                let options = &request.config.options;
                let mut attrs =
                    WindowAttributes::default()
                        .with_title(request.config.title.clone())
                        .with_inner_size(winit::dpi::LogicalSize::new(
                            request.config.width,
                            request.config.height,
                        ))
                        .with_decorations(options.decorations)
                        .with_maximized(options.maximized)
                        .with_fullscreen(
                            options
                                .fullscreen
                                .then_some(winit::window::Fullscreen::Borderless(None)),
                        )
                        .with_window_icon(options.icon.clone().and_then(|(rgba, w, h)| {
                            winit::window::Icon::from_rgba(rgba, w, h).ok()
                        }))
                        .with_transparent(options.transparent)
                        .with_blur(options.blur)
                        .with_visible(false);
                if let Some((w, h)) = options.min_size {
                    attrs = attrs.with_min_inner_size(winit::dpi::LogicalSize::new(w, h));
                }
                // 0.5.0 M2: Windows drops an undecorated window's shadow
                // unless asked to keep it; a decorated one has it anyway.
                #[cfg(target_os = "windows")]
                let attrs = {
                    use winit::platform::windows::WindowAttributesExtWindows;
                    attrs.with_undecorated_shadow(true)
                };
                // 0.5.0 M4: macOS can't resize an undecorated window, so it
                // stays decorated, with its title bar a transparent overlay
                // on full-size content (see `titlebar`).
                #[cfg(target_os = "macos")]
                let attrs = if options.decorations {
                    attrs
                } else {
                    use winit::platform::macos::WindowAttributesExtMacOS;
                    attrs
                        .with_decorations(true)
                        .with_titlebar_transparent(true)
                        .with_fullsize_content_view(true)
                        .with_title_hidden(true)
                };
                let window = event_loop
                    .create_window(attrs)
                    .expect("failed to create window");
                let adapter = accesskit_winit::Adapter::with_event_loop_proxy(
                    event_loop,
                    &window,
                    self.proxy.clone(),
                );
                // M17 Phase 2 (§8): real, load-bearing -- `Window::
                // set_ime_allowed`'s own doc comment states plainly
                // "IME is not allowed by default" (confirmed via direct
                // source read); without this call, `WindowEvent::Ime`
                // never fires at all, silently dead-ending the whole
                // feature. "During the preedit phase the window will
                // NOT get `KeyboardInput` events" (also real, same doc
                // comment) -- composing and plain typing are already
                // mutually exclusive at the `winit` level, nothing this
                // codebase needs to coordinate itself.
                window.set_ime_allowed(true);
                window.set_visible(true);
                window.request_redraw();
                let id = window.id();
                let window = Arc::new(window);

                if !(self.on_window_created)(id, request.token, window.clone()) {
                    event_loop.exit();
                    return;
                }
                self.windows.insert(
                    id,
                    PerWindow {
                        window,
                        // The first frame is this loop's own request.
                        app_requested: true,
                        access_adapter: adapter,
                        frame: 0,
                        max_frames: request.config.max_frames,
                        modifiers: ModifiersState::empty(),
                        last_cursor_position: Point::ZERO,
                        reduced_motion: None,
                        high_contrast: None,
                        animating: true,
                    },
                );
            }
            PlatformEvent::AccessKit(event) => match event.window_event {
                accesskit_winit::WindowEvent::InitialTreeRequested => {
                    let Self {
                        windows,
                        build_access_update,
                        ..
                    } = self;
                    if let Some(win) = windows.get_mut(&event.window_id) {
                        win.access_adapter
                            .update_if_active(|| build_access_update(event.window_id));
                    }
                }
                // M4 Phase 2 (§10): a real platform accessibility client
                // (a screen reader) naming a node to activate or focus
                // directly -- handed up raw, converted and dispatched by
                // the caller (this module's own doc comment explains
                // why: no `engine_core::NodeId` knowledge here).
                accesskit_winit::WindowEvent::ActionRequested(request) => {
                    (self.on_access_action)(event.window_id, request);
                    // M29 Phase 2 (§5, §6): a screen-reader-driven
                    // `Action::Focus`/`Action::Click` is a real, `Tree`-
                    // mutating input path with no `WindowEvent` behind it
                    // at all -- the one real gap Phase 2's own scoping
                    // named as needing a direct check. Without this, a
                    // window sitting in `ControlFlow::Wait` would never
                    // paint the result of an assistive-technology action
                    // until some *other*, unrelated event happened to
                    // wake it.
                    if let Some(win) = self.windows.get_mut(&event.window_id) {
                        win.request_redraw();
                    }
                }
                // No real per-window behavior change needed here --
                // `access_adapter.update_if_active` (used everywhere
                // this crate builds a `TreeUpdate`) already gates on
                // activation state internally, so deactivation is
                // already handled correctly by that existing check, not
                // a gap this step leaves open.
                accesskit_winit::WindowEvent::AccessibilityDeactivated => {}
            },
            // M31 Phase 6 (§5, §6): a real cross-thread wake -- just
            // sending this event already woke a genuinely idle
            // `ControlFlow::Wait` loop far enough to run this handler
            // at all; requesting a real redraw of every open window is
            // what actually gets `on_frame` (and, through it, a real
            // background producer's own new output) painted. Real,
            // deliberately whole-loop, not scoped to whichever window
            // the real change happened to originate in -- the same
            // real "whole-loop signal" shape `any_active` itself
            // already has.
            PlatformEvent::Wake => {
                for win in self.windows.values_mut() {
                    win.request_redraw();
                }
            }
            PlatformEvent::ThemeChanged(dark) => {
                for (&id, win) in &self.windows {
                    (self.on_input)(id, InputEvent::ThemeChanged { dark });
                    win.window.request_redraw();
                }
            }
            PlatformEvent::ReducedMotionChanged(reduced) => {
                for (&id, win) in &mut self.windows {
                    win.reduced_motion = Some(reduced);
                    (self.on_input)(id, InputEvent::ReducedMotionChanged { reduced });
                    win.window.request_redraw();
                }
            }
            PlatformEvent::HighContrastChanged(high) => {
                for (&id, win) in &mut self.windows {
                    win.high_contrast = Some(high);
                    (self.on_input)(id, InputEvent::HighContrastChanged { high });
                    win.window.request_redraw();
                }
            }
            PlatformEvent::ReportSize(id) => {
                if let Some(win) = self.windows.get(&id) {
                    let size = win.window.inner_size();
                    (self.on_input)(
                        id,
                        InputEvent::Resized {
                            width: size.width as f32,
                            height: size.height as f32,
                        },
                    );
                    win.window.request_redraw();
                }
            }
            // 0.5.0 M2: the app's own close -- the same path as
            // `WindowEvent::CloseRequested` below.
            PlatformEvent::CloseWindow(id) => {
                if self.windows.contains_key(&id)
                    && (self.on_lifecycle)(id, WindowLifecycle::CloseRequested)
                {
                    (self.on_lifecycle)(id, WindowLifecycle::Closed);
                    self.windows.remove(&id);
                    if self.windows.is_empty() {
                        event_loop.exit();
                    }
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        self.attach();
        let Self {
            windows,
            on_frame,
            build_access_update,
            on_input,
            on_lifecycle,
            ..
        } = self;
        let Some(win) = windows.get_mut(&window_id) else {
            return;
        };
        win.access_adapter.process_event(&win.window, &event);

        match event {
            // M94: the app decides -- `on_lifecycle` returning `false`
            // keeps the window open (a cancelled `close_requested`).
            WindowEvent::CloseRequested => {
                if on_lifecycle(window_id, WindowLifecycle::CloseRequested) {
                    on_lifecycle(window_id, WindowLifecycle::Closed);
                    windows.remove(&window_id);
                    if windows.is_empty() {
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let os_requested = !std::mem::take(&mut win.app_requested);
                let real_still_animating = on_frame(window_id, win.frame, os_requested);
                // M29 Phase 2: real finding, caught by actually running
                // this against every example/test in the workspace, not
                // assumed -- a bounded run (`max_frames: Some(_)`) must
                // keep polling toward its own frame count regardless of
                // `on_frame`'s own animation/dirty report, or it would
                // never reach that count once idle and hang forever
                // under `ControlFlow::Wait` with no real input arriving
                // to wake it. Every existing example/test in this
                // workspace relies on exactly this "runs N frames then
                // exits" pattern to finish in bounded wall-clock time.
                // Only a genuinely unbounded window (`max_frames: None`,
                // a real interactive app) gets Phase 2's idle-CPU benefit.
                let still_animating = real_still_animating || win.max_frames.is_some();
                win.animating = still_animating;
                win.access_adapter
                    .update_if_active(|| build_access_update(window_id));
                win.frame += 1;
                if let Some(max) = win.max_frames
                    && win.frame >= max
                {
                    on_lifecycle(window_id, WindowLifecycle::Closed);
                    windows.remove(&window_id);
                    if windows.is_empty() {
                        event_loop.exit();
                    }
                    return;
                }
                // M29 Phase 2 (§5, §6): the polling half of the same
                // dirty-tracking signal Phase 1 taught `Tree` to compute
                // -- a window `on_frame` reports still-animating keeps
                // scheduling its own next redraw exactly like every
                // window unconditionally did before this phase; one that
                // reports settled does not, and instead waits for a real
                // input event (or another window's own animation) to
                // wake it via an explicit `request_redraw()` call
                // elsewhere in this file.
                if still_animating {
                    win.request_redraw();
                }
                // `ControlFlow` is a single, event-loop-wide setting, not
                // per-window -- with more than one window open, this
                // must stay `Poll` as long as *any* of them is still
                // animating, and only drop to `Wait` once every open
                // window has independently settled.
                let any_window_animating = windows.values().any(|w| w.animating);
                event_loop.set_control_flow(if any_window_animating {
                    ControlFlow::Poll
                } else {
                    ControlFlow::Wait
                });
            }
            // M4 Phase 1 step 2: the real translation this module's own
            // doc comment named as still missing. `PhysicalPosition<f64>`
            // -> `kurbo::Point` is a plain field copy -- both are
            // window-client pixels, top-left origin, no unit conversion
            // needed.
            WindowEvent::CursorMoved { position, .. } => {
                let position = Point::new(position.x, position.y);
                win.last_cursor_position = position;
                on_input(window_id, InputEvent::PointerMoved { position });
                // M29 Phase 2 (§5, §6): a waiting loop only wakes for a
                // real event like this one -- nothing else will ask for
                // the next redraw on its own anymore, so every real
                // input-handling arm below does, unconditionally (cheap,
                // and simpler/safer than tracking whether this specific
                // event actually changed anything worth a repaint --
                // `Tree`'s own dirty flag, Phase 1, already makes an
                // extra request here free if it turns out nothing did).
                win.request_redraw();
            }
            // 0.5.4 (#113): a finger on the screen. Positions are the
            // window's pixels, as the pointer's are.
            WindowEvent::Touch(touch) => {
                let position = Point::new(touch.location.x, touch.location.y);
                on_input(
                    window_id,
                    InputEvent::Touch {
                        id: touch.id,
                        phase: translate_touch_phase(touch.phase),
                        position,
                    },
                );
                win.request_redraw();
            }
            // 0.5.4 (#113): a trackpad pinch (macOS, iOS), at the cursor.
            WindowEvent::PinchGesture { delta, phase, .. } => {
                on_input(
                    window_id,
                    InputEvent::TrackpadPinch {
                        delta,
                        phase: translate_touch_phase(phase),
                        position: win.last_cursor_position,
                    },
                );
                win.request_redraw();
            }
            // 0.5.4 (#114): files dragged over and dropped on the window.
            // `winit` gives them no position; the pointer's last is the best
            // there is.
            WindowEvent::HoveredFile(path) => {
                let position = win.last_cursor_position;
                on_input(window_id, InputEvent::FileHovered { path, position });
                win.request_redraw();
            }
            WindowEvent::HoveredFileCancelled => {
                on_input(window_id, InputEvent::FileHoverCancelled);
                win.request_redraw();
            }
            WindowEvent::DroppedFile(path) => {
                let position = win.last_cursor_position;
                on_input(window_id, InputEvent::FileDropped { path, position });
                win.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                win.modifiers = modifiers.state();
                on_input(
                    window_id,
                    InputEvent::ModifiersChanged(translate_modifiers(win.modifiers)),
                );
            }
            // M94: nothing is hovered once the pointer leaves the window.
            WindowEvent::CursorLeft { .. } => {
                on_input(window_id, InputEvent::PointerLeft);
                win.request_redraw();
            }
            // M94: delivered to Python as the window's `scale_factor`
            // event.
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                on_input(window_id, InputEvent::ScaleFactorChanged { scale_factor });
                win.request_redraw();
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(button) = translate_pointer_button(button) {
                    // `winit`'s own `MouseInput` carries no position --
                    // it always corresponds to the cursor's last known
                    // `CursorMoved` position (which always arrives first
                    // in practice, the OS reports pointer position
                    // continuously), tracked in `last_cursor_position`
                    // above for exactly this.
                    let position = win.last_cursor_position;
                    let event = match state {
                        ElementState::Pressed => InputEvent::PointerPressed { position, button },
                        ElementState::Released => InputEvent::PointerReleased { position, button },
                    };
                    on_input(window_id, event);
                }
                win.request_redraw();
            }
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                // M94: every named or character key reaches Python as
                // `key_down`/`key_up`, before (and independently of) the
                // engine's own narrow key, clipboard, and text handling
                // below.
                if let Some(name) = key_name(&key_event.logical_key) {
                    on_input(
                        window_id,
                        InputEvent::Key {
                            name,
                            pressed: key_event.state == ElementState::Pressed,
                            repeat: key_event.repeat,
                        },
                    );
                }
                if let Some(key) = translate_key(&key_event.logical_key) {
                    let shift = win.modifiers.shift_key();
                    let event = match key_event.state {
                        ElementState::Pressed => InputEvent::KeyPressed { key, shift },
                        ElementState::Released => InputEvent::KeyReleased { key, shift },
                    };
                    on_input(window_id, event);
                } else if key_event.state == ElementState::Pressed
                    && win.modifiers.control_key()
                    && let Some(clipboard_event) = translate_clipboard_shortcut(
                        &key_event.logical_key,
                        win.modifiers.shift_key(),
                    )
                {
                    // M17 Phase 1 (§8), checked *before* the `TextInput`
                    // fallback below -- real, load-bearing ordering, not
                    // arbitrary. `winit::event::KeyEvent.logical_key` is
                    // documented as "affected by all modifiers except
                    // Ctrl" (confirmed via direct source read), so a
                    // real Ctrl+C press produces `Character("c")`,
                    // identical to a bare `c` press -- without this
                    // check running first, it would fall through and
                    // insert a literal "c" instead, a real latent bug
                    // this phase's own design surfaced and fixes as a
                    // side effect, not a separate patch.
                    on_input(window_id, clipboard_event);
                } else if key_event.state == ElementState::Pressed
                    && let Some(text) = &key_event.text
                {
                    // M15 Phase 2 (§8, §10): a real, produced character
                    // keypress `translate_key` doesn't already claim as
                    // a named/control key -- `KeyEvent.text: Option<
                    // SmolStr>` is `winit`'s own real per-keypress
                    // produced text (confirmed via direct source read
                    // of the pinned `winit = "0.30.13"`), fired only on
                    // press (not release, which has no real "text
                    // input" meaning).
                    on_input(window_id, InputEvent::TextInput(text.to_string()));
                }
                win.request_redraw();
            }
            // M4 Phase 8 (§11.7/§11.8 groundwork): `winit`'s own
            // `MouseWheel` carries no position either, the same real
            // fact `MouseInput` already works around -- reuses
            // `last_cursor_position` identically.
            WindowEvent::MouseWheel { delta, .. } => {
                on_input(
                    window_id,
                    InputEvent::Scroll {
                        delta: translate_scroll_delta(delta),
                        position: win.last_cursor_position,
                    },
                );
                win.request_redraw();
            }
            // M7 Phase 3 (§7.1): real live OS light/dark switching --
            // verified directly against the pinned `winit = "0.30.13"`
            // source (`src/event.rs`): `WindowEvent::ThemeChanged(Theme)`
            // is real, `Theme` is `{ Light, Dark }`. Its own doc comment
            // states this is unsupported on iOS/Android/X11/Wayland/
            // Orbital -- it simply never fires there. M106: on Linux the
            // XDG settings portal stands in for it
            // (`PlatformEvent::ThemeChanged`, from `appearance::portal`).
            WindowEvent::ThemeChanged(theme) => {
                on_input(
                    window_id,
                    InputEvent::ThemeChanged {
                        dark: translate_theme(theme),
                    },
                );
                win.request_redraw();
            }
            // 0.5.0 M2 (issue #28): the window gained or lost focus.
            WindowEvent::Focused(focused) => {
                if focused {
                    // 0.5.4 (#115): coming back from the system settings is when
                    // a changed preference is noticed where the OS says nothing.
                    let motion = appearance::current_reduced_motion(Some(&win.window));
                    if let Some(reduced) = motion
                        && win.reduced_motion != Some(reduced)
                    {
                        let changed = win.reduced_motion.is_some();
                        win.reduced_motion = Some(reduced);
                        if changed {
                            on_input(window_id, InputEvent::ReducedMotionChanged { reduced });
                        }
                    }
                    let contrast = appearance::current_high_contrast(Some(&win.window));
                    if let Some(high) = contrast
                        && win.high_contrast != Some(high)
                    {
                        let changed = win.high_contrast.is_some();
                        win.high_contrast = Some(high);
                        if changed {
                            on_input(window_id, InputEvent::HighContrastChanged { high });
                        }
                    }
                }
                on_input(window_id, InputEvent::Focused { focused });
                win.request_redraw();
            }
            // M32 Phase 2 (§4, §5): the real gap this phase closes --
            // "nothing resizes any node's box when its window resizes."
            // `winit`'s own `PhysicalSize<u32>` fields are handed
            // through as plain `f32`s, the identical "no DPI-scaling
            // conversion" convention `CursorMoved`'s own translation
            // above already established for this codebase. The real
            // GPU-surface reconfiguration this also requires (`engine-
            // platform` has no GPU knowledge at all, §4) is `engine-
            // py`'s own `on_input` closure's job, the same "translate
            // the raw event, let the real handler decide what it means"
            // split `ThemeChanged` above already follows.
            WindowEvent::Resized(size) => {
                on_input(
                    window_id,
                    InputEvent::Resized {
                        width: size.width as f32,
                        height: size.height as f32,
                    },
                );
                win.request_redraw();
            }
            // M17 Phase 2 (§8): real IME composition, reachable only
            // because `resumed`'s own window creation now calls
            // `Window::set_ime_allowed(true)` -- see that call site's
            // own doc comment for why this event otherwise never fires
            // at all. `Enabled`/`Disabled` are true no-ops for now, the
            // same "not manufactured ahead of a real need" scope every
            // other minimal-vocabulary translation in this module
            // already keeps -- `Commit` reaches the exact same real
            // `TextInput` mechanism a plain keypress already uses (M15
            // Phase 2), no new variant needed for it at all.
            WindowEvent::Ime(ime) => {
                match ime {
                    Ime::Preedit(text, _cursor_range) => {
                        on_input(window_id, InputEvent::ImePreedit(text));
                    }
                    Ime::Commit(text) => {
                        on_input(window_id, InputEvent::TextInput(text));
                    }
                    Ime::Enabled | Ime::Disabled => {}
                }
                win.request_redraw();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Design Principle 5: validated standalone against real `winit`
    // enum values, no live `EventLoop` needed -- `translate_pointer_button`/
    // `translate_key` are plain data-in, data-out functions.

    #[test]
    fn translate_pointer_button_maps_the_three_real_buttons_this_model_distinguishes() {
        assert_eq!(
            translate_pointer_button(MouseButton::Left),
            Some(PointerButton::Primary)
        );
        assert_eq!(
            translate_pointer_button(MouseButton::Right),
            Some(PointerButton::Secondary)
        );
        assert_eq!(
            translate_pointer_button(MouseButton::Middle),
            Some(PointerButton::Middle)
        );
    }

    #[test]
    fn translate_pointer_button_ignores_buttons_with_no_md3_desktop_meaning_yet() {
        assert_eq!(
            translate_pointer_button(MouseButton::Back),
            Some(PointerButton::Back)
        );
        assert_eq!(
            translate_pointer_button(MouseButton::Forward),
            Some(PointerButton::Forward)
        );
        assert_eq!(translate_pointer_button(MouseButton::Other(7)), None);
    }

    #[test]
    fn translate_key_maps_exactly_the_minimal_keyboard_vocabulary() {
        assert_eq!(
            translate_key(&WinitKey::Named(NamedKey::Tab)),
            Some(Key::Tab)
        );
        assert_eq!(
            translate_key(&WinitKey::Named(NamedKey::Enter)),
            Some(Key::Enter)
        );
        assert_eq!(
            translate_key(&WinitKey::Named(NamedKey::Space)),
            Some(Key::Space)
        );
        assert_eq!(
            translate_key(&WinitKey::Named(NamedKey::Escape)),
            Some(Key::Escape)
        );
        // M15 Phase 2 (§8, §10): the real named/control keys `TextField`
        // editing added to the minimal vocabulary.
        assert_eq!(
            translate_key(&WinitKey::Named(NamedKey::Backspace)),
            Some(Key::Backspace)
        );
        assert_eq!(
            translate_key(&WinitKey::Named(NamedKey::Delete)),
            Some(Key::Delete)
        );
        assert_eq!(
            translate_key(&WinitKey::Named(NamedKey::ArrowLeft)),
            Some(Key::ArrowLeft)
        );
        assert_eq!(
            translate_key(&WinitKey::Named(NamedKey::ArrowRight)),
            Some(Key::ArrowRight)
        );
        // M30 Phase 9 Step 3 (§10): the real named keys `Code Editor`'s
        // own multiline line-navigation added to the vocabulary.
        assert_eq!(
            translate_key(&WinitKey::Named(NamedKey::ArrowUp)),
            Some(Key::ArrowUp)
        );
        assert_eq!(
            translate_key(&WinitKey::Named(NamedKey::ArrowDown)),
            Some(Key::ArrowDown)
        );
        assert_eq!(
            translate_key(&WinitKey::Named(NamedKey::Home)),
            Some(Key::Home)
        );
        assert_eq!(
            translate_key(&WinitKey::Named(NamedKey::End)),
            Some(Key::End)
        );
        assert_eq!(
            translate_key(&WinitKey::Named(NamedKey::PageUp)),
            Some(Key::PageUp)
        );
        assert_eq!(
            translate_key(&WinitKey::Named(NamedKey::PageDown)),
            Some(Key::PageDown)
        );
    }

    #[test]
    fn key_name_snake_cases_named_keys_and_passes_characters_through() {
        use winit::keyboard::SmolStr;
        assert_eq!(
            key_name(&WinitKey::Named(NamedKey::ArrowLeft)).as_deref(),
            Some("arrow_left")
        );
        assert_eq!(
            key_name(&WinitKey::Named(NamedKey::F5)).as_deref(),
            Some("f5")
        );
        assert_eq!(
            key_name(&WinitKey::Named(NamedKey::F12)).as_deref(),
            Some("f12")
        );
        assert_eq!(
            key_name(&WinitKey::Named(NamedKey::Enter)).as_deref(),
            Some("enter")
        );
        assert_eq!(
            key_name(&WinitKey::Named(NamedKey::Space)).as_deref(),
            Some("space")
        );
        assert_eq!(
            key_name(&WinitKey::Named(NamedKey::PageUp)).as_deref(),
            Some("page_up")
        );
        assert_eq!(
            key_name(&WinitKey::Character(SmolStr::new("a"))).as_deref(),
            Some("a")
        );
        assert_eq!(
            key_name(&WinitKey::Character(SmolStr::new("A"))).as_deref(),
            Some("A")
        );
        assert_eq!(key_name(&WinitKey::Dead(None)), None);
    }

    #[test]
    fn translate_modifiers_maps_all_four_keys() {
        let state = ModifiersState::SHIFT | ModifiersState::SUPER;
        assert_eq!(
            translate_modifiers(state),
            Modifiers {
                shift: true,
                ctrl: false,
                alt: false,
                meta: true,
            }
        );
        assert_eq!(
            translate_modifiers(ModifiersState::empty()),
            Modifiers::default()
        );
    }

    #[test]
    fn translate_key_ignores_every_key_outside_the_minimal_vocabulary() {
        // A printable character -- real text entry needs no `NodeKind`
        // this codebase has yet (this module's own doc comment); a
        // named key this minimal model simply doesn't assign meaning to.
        assert_eq!(translate_key(&WinitKey::Character("a".into())), None);
        // (Page Down was the example here until 0.4.2 gave it a meaning.)
        assert_eq!(translate_key(&WinitKey::Named(NamedKey::Insert)), None);
    }

    #[test]
    fn translate_clipboard_shortcut_maps_the_real_copy_cut_paste_vocabulary() {
        assert_eq!(
            translate_clipboard_shortcut(&WinitKey::Character("c".into()), false),
            Some(InputEvent::Copy)
        );
        assert_eq!(
            translate_clipboard_shortcut(&WinitKey::Character("x".into()), false),
            Some(InputEvent::Cut)
        );
        assert_eq!(
            translate_clipboard_shortcut(&WinitKey::Character("v".into()), false),
            Some(InputEvent::PasteRequested)
        );
    }

    #[test]
    fn translate_clipboard_shortcut_is_case_insensitive_for_the_letter_itself() {
        // Real, deliberate: an uppercase `Character` (Caps Lock, say,
        // with no real Shift held) must still read as a plain Ctrl+C,
        // not the real Ctrl+Shift+C-only `TerminalCopyRequested` --
        // `shift` is a separate, explicit real `bool` now, never
        // inferred from the character's own case.
        assert_eq!(
            translate_clipboard_shortcut(&WinitKey::Character("C".into()), false),
            Some(InputEvent::Copy)
        );
    }

    #[test]
    fn translate_clipboard_shortcut_ignores_a_named_key_and_a_non_alphabetic_character() {
        assert_eq!(
            translate_clipboard_shortcut(&WinitKey::Named(NamedKey::Enter), false),
            None
        );
        assert_eq!(
            translate_clipboard_shortcut(&WinitKey::Character("1".into()), false),
            None
        );
        // A real, if rare, multi-character `Character` payload (some
        // IME/dead-key sequences) is not a single real Ctrl+<letter>
        // shortcut -- must not panic or silently pick the first char.
        assert_eq!(
            translate_clipboard_shortcut(&WinitKey::Character("ab".into()), false),
            None
        );
    }

    /// M32 Phase 4 (§4, §8): every letter besides `c`/`x`/`v` now
    /// produces the new `ControlChar`, not `None` -- the real fix this
    /// phase exists for.
    #[test]
    fn translate_clipboard_shortcut_maps_every_other_letter_to_control_char() {
        assert_eq!(
            translate_clipboard_shortcut(&WinitKey::Character("a".into()), false),
            Some(InputEvent::ControlChar('a'))
        );
        assert_eq!(
            translate_clipboard_shortcut(&WinitKey::Character("Z".into()), false),
            Some(InputEvent::ControlChar('z')),
            "case-insensitive, the same real convention c/x/v already established"
        );
    }

    /// M32 Phase 6 (§4, §5, §8): the one real exception to "shift
    /// doesn't change the shortcut" -- a genuine Ctrl+Shift+C produces
    /// `TerminalCopyRequested`, not `Copy`; every other letter (including
    /// `x`/`v`) stays completely unaffected by `shift`.
    #[test]
    fn translate_clipboard_shortcut_shift_c_is_terminal_copy_requested() {
        assert_eq!(
            translate_clipboard_shortcut(&WinitKey::Character("c".into()), true),
            Some(InputEvent::TerminalCopyRequested)
        );
        assert_eq!(
            translate_clipboard_shortcut(&WinitKey::Character("x".into()), true),
            Some(InputEvent::Cut),
            "shift must not change any other real shortcut's own meaning"
        );
        assert_eq!(
            translate_clipboard_shortcut(&WinitKey::Character("v".into()), true),
            Some(InputEvent::PasteRequested)
        );
        assert_eq!(
            translate_clipboard_shortcut(&WinitKey::Character("a".into()), true),
            Some(InputEvent::ControlChar('a'))
        );
    }

    #[test]
    fn translate_theme_maps_winits_two_real_variants_to_the_matching_bool() {
        assert!(translate_theme(winit::window::Theme::Dark));
        assert!(!translate_theme(winit::window::Theme::Light));
    }

    #[test]
    fn translate_scroll_delta_preserves_the_real_line_pixel_distinction() {
        assert_eq!(
            translate_scroll_delta(MouseScrollDelta::LineDelta(0.0, 3.0)),
            ScrollDelta::Lines(0.0, 3.0)
        );
        assert_eq!(
            translate_scroll_delta(MouseScrollDelta::LineDelta(-1.5, 0.0)),
            ScrollDelta::Lines(-1.5, 0.0)
        );
        assert_eq!(
            translate_scroll_delta(MouseScrollDelta::PixelDelta(
                winit::dpi::PhysicalPosition::new(0.0, -40.0)
            )),
            ScrollDelta::Pixels(0.0, -40.0)
        );
    }
}
