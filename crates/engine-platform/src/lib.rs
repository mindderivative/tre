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

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use engine_core::{InputEvent, Key, PointerButton, ScrollDelta};
use peniko::kurbo::Point;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

/// The three buttons `engine_core::PointerButton` actually distinguishes.
/// **Real API fact, verified directly against `winit = "0.30.13"`'s own
/// `event.rs` before writing this:** `winit::event::MouseButton` has six
/// variants (`Left`/`Right`/`Middle`/`Back`/`Forward`/`Other(u16)`), not
/// three -- an earlier doc comment on `engine_core::PointerButton`
/// claimed a 1:1 three-variant match without checking, which was wrong.
/// `Back`/`Forward`/`Other` (a browser-navigation convention with no
/// real MD3 desktop meaning yet) translate to `None` -- no `InputEvent`
/// at all, a stated narrowing, not a silently-dropped case.
fn translate_pointer_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        MouseButton::Back | MouseButton::Forward | MouseButton::Other(_) => None,
    }
}

/// `engine_core::Key`'s own deliberately minimal vocabulary (§10) --
/// every other `winit` key, including every printable character,
/// produces `None` (no `InputEvent` at all). Matched against
/// `winit::keyboard::Key::Named`, verified directly against `winit`'s
/// own `keyboard.rs` (`NamedKey::{Tab, Enter, Space, Escape}` all real,
/// confirmed variants) before writing this.
fn translate_key(logical_key: &WinitKey) -> Option<Key> {
    match logical_key {
        WinitKey::Named(NamedKey::Tab) => Some(Key::Tab),
        WinitKey::Named(NamedKey::Enter) => Some(Key::Enter),
        WinitKey::Named(NamedKey::Space) => Some(Key::Space),
        WinitKey::Named(NamedKey::Escape) => Some(Key::Escape),
        _ => None,
    }
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
/// exists purely to carry `accesskit_winit::Event` and window-open
/// requests across the proxy boundary (see the module doc comment for
/// why `with_event_loop_proxy`, not the direct-handler API).
enum PlatformEvent {
    AccessKit(accesskit_winit::Event),
    OpenWindow(WindowRequest),
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

/// Runs a `winit` event loop managing any number of windows.
///
/// `setup` is called once, synchronously, before the loop starts --
/// its only job is to request the initial window(s) via the given
/// [`WindowOpener`]. For each window actually created, `on_window_created`
/// fires exactly once with its real `WindowId`, the `token` from
/// whichever `WindowRequest` produced it, and an owned `Arc<Window>` --
/// the caller's one chance to build (and stash, keyed by `WindowId`) any
/// per-window GPU/render state. `on_frame` then fires once per redraw
/// with just the `WindowId` and 0-based frame index (the caller already
/// has everything else from `on_window_created`); `build_access_update`
/// fires per window the same way, keeping each window's own exposed
/// accessibility tree in sync (§10). Exits once every window has closed
/// or reached its own `max_frames`.
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
pub fn run_windowed_multi<C, F, A, S, N, X>(
    on_window_created: C,
    on_frame: F,
    build_access_update: A,
    on_input: N,
    on_access_action: X,
    setup: S,
) -> Result<(), winit::error::EventLoopError>
where
    C: FnMut(WindowId, u64, Arc<Window>),
    F: FnMut(WindowId, u32),
    A: FnMut(WindowId) -> accesskit::TreeUpdate,
    N: FnMut(WindowId, InputEvent),
    X: FnMut(WindowId, accesskit::ActionRequest),
    S: FnOnce(&WindowOpener),
{
    let event_loop = EventLoop::<PlatformEvent>::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let proxy = event_loop.create_proxy();

    setup(&WindowOpener {
        proxy: proxy.clone(),
    });

    let mut app = MultiWindowApp {
        windows: HashMap::new(),
        proxy,
        on_window_created,
        on_frame,
        build_access_update,
        on_input,
        on_access_action,
    };
    event_loop.run_app(&mut app)
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
        },
        move |_id, frame| {
            let window = window
                .borrow()
                .clone()
                .expect("on_window_created always fires before on_frame for the same window");
            on_frame(&window, frame);
        },
        move |_id| build_access_update(),
        // Neither of this wrapper's two existing callers (`rect_window.rs`,
        // `access_button.rs`) needs input events -- a no-op keeps
        // `run_windowed`'s own signature untouched, matching this
        // function's own doc comment's "byte-for-byte source-compatible"
        // contract.
        |_id, _event| {},
        |_id, _request| {},
        |opener| {
            opener.open_window(WindowRequest { config, token: 0 });
        },
    )
}

struct PerWindow {
    window: Arc<Window>,
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
}

struct MultiWindowApp<C, F, A, N, X> {
    windows: HashMap<WindowId, PerWindow>,
    proxy: EventLoopProxy<PlatformEvent>,
    on_window_created: C,
    on_frame: F,
    build_access_update: A,
    on_input: N,
    on_access_action: X,
}

impl<C, F, A, N, X> ApplicationHandler<PlatformEvent> for MultiWindowApp<C, F, A, N, X>
where
    C: FnMut(WindowId, u64, Arc<Window>),
    F: FnMut(WindowId, u32),
    A: FnMut(WindowId) -> accesskit::TreeUpdate,
    N: FnMut(WindowId, InputEvent),
    X: FnMut(WindowId, accesskit::ActionRequest),
{
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        // Windows are created lazily, in `user_event`, as `OpenWindow`
        // requests arrive -- not eagerly here. `setup`'s own
        // `open_window` calls (sent before the loop starts) are queued
        // on the proxy and delivered as the very first `user_event`
        // calls once the loop is actually running.
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: PlatformEvent) {
        match event {
            PlatformEvent::OpenWindow(request) => {
                // accesskit_winit's own hard requirement: the adapter
                // must be created before the window is ever shown,
                // which means creating the window invisible first.
                let attrs = WindowAttributes::default()
                    .with_title(request.config.title.clone())
                    .with_inner_size(winit::dpi::LogicalSize::new(
                        request.config.width,
                        request.config.height,
                    ))
                    .with_visible(false);
                let window = event_loop
                    .create_window(attrs)
                    .expect("failed to create window");
                let adapter = accesskit_winit::Adapter::with_event_loop_proxy(
                    event_loop,
                    &window,
                    self.proxy.clone(),
                );
                window.set_visible(true);
                window.request_redraw();
                let id = window.id();
                let window = Arc::new(window);

                (self.on_window_created)(id, request.token, window.clone());
                self.windows.insert(
                    id,
                    PerWindow {
                        window,
                        access_adapter: adapter,
                        frame: 0,
                        max_frames: request.config.max_frames,
                        modifiers: ModifiersState::empty(),
                        last_cursor_position: Point::ZERO,
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
                }
                // No real per-window behavior change needed here --
                // `access_adapter.update_if_active` (used everywhere
                // this crate builds a `TreeUpdate`) already gates on
                // activation state internally, so deactivation is
                // already handled correctly by that existing check, not
                // a gap this step leaves open.
                accesskit_winit::WindowEvent::AccessibilityDeactivated => {}
            },
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Self {
            windows,
            on_frame,
            build_access_update,
            on_input,
            ..
        } = self;
        let Some(win) = windows.get_mut(&window_id) else {
            return;
        };
        win.access_adapter.process_event(&win.window, &event);

        match event {
            WindowEvent::CloseRequested => {
                windows.remove(&window_id);
                if windows.is_empty() {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                on_frame(window_id, win.frame);
                win.access_adapter
                    .update_if_active(|| build_access_update(window_id));
                win.frame += 1;
                if let Some(max) = win.max_frames
                    && win.frame >= max
                {
                    windows.remove(&window_id);
                    if windows.is_empty() {
                        event_loop.exit();
                    }
                    return;
                }
                win.window.request_redraw();
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
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                win.modifiers = modifiers.state();
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
            }
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                if let Some(key) = translate_key(&key_event.logical_key) {
                    let shift = win.modifiers.shift_key();
                    let event = match key_event.state {
                        ElementState::Pressed => InputEvent::KeyPressed { key, shift },
                        ElementState::Released => InputEvent::KeyReleased { key, shift },
                    };
                    on_input(window_id, event);
                }
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
            }
            // M7 Phase 3 (§7.1): real live OS light/dark switching --
            // verified directly against the pinned `winit = "0.30.13"`
            // source (`src/event.rs`): `WindowEvent::ThemeChanged(Theme)`
            // is real, `Theme` is `{ Light, Dark }`. Its own doc comment
            // states this is unsupported on iOS/Android/X11/Wayland/
            // Orbital -- it simply never fires there, which is a real,
            // known platform limitation of this event, not a bug in
            // this translation.
            WindowEvent::ThemeChanged(theme) => {
                on_input(
                    window_id,
                    InputEvent::ThemeChanged {
                        dark: translate_theme(theme),
                    },
                );
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
        assert_eq!(translate_pointer_button(MouseButton::Back), None);
        assert_eq!(translate_pointer_button(MouseButton::Forward), None);
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
    }

    #[test]
    fn translate_key_ignores_every_key_outside_the_minimal_vocabulary() {
        // A printable character -- real text entry needs no `NodeKind`
        // this codebase has yet (this module's own doc comment); a
        // named key this minimal model simply doesn't assign meaning to.
        assert_eq!(translate_key(&WinitKey::Character("a".into())), None);
        assert_eq!(translate_key(&WinitKey::Named(NamedKey::ArrowDown)), None);
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
