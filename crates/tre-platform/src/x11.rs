//! Native X11 windowing via `x11rb`'s XCB FFI connection type
//! (`allow-unsafe-code` feature), which -- unlike x11rb's default
//! pure-Rust socket connection -- links real `libxcb` and exposes a
//! genuine `xcb_connection_t*` for Vulkan's `VK_KHR_xcb_surface` extension.
//! One shared `XCBConnection` now owns multiple windows (IMPLEMENTATION.md
//! Step 1.2), each identified by a [`WindowId`] rather than one connection
//! per window. Fallback path for X11/XWayland sessions.
//!
//! Every event this backend produces -- input and window lifecycle alike
//! -- is pushed through one shared [`tre_engine::InputEventQueue`], which
//! owns pointer-move coalescing centrally rather than each backend
//! duplicating it (mirrors `wayland.rs`'s structure).

use std::collections::HashMap;
use std::ffi::c_void;
use std::num::NonZeroU32;
use std::ptr::NonNull;

use as_raw_xcb_connection::AsRawXcbConnection;
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, RawDisplayHandle, RawWindowHandle, WindowHandle,
    XcbDisplayHandle, XcbWindowHandle,
};
use tre_engine::{ElementState, InputEvent, InputEventQueue, MouseButton, WindowId};
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{ConnectionExt as _, CreateWindowAux, EventMask, WindowClass};
use x11rb::protocol::Event;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::xcb_ffi::XCBConnection;

use crate::PlatformError;

/// One connection's worth of queued events, sized generously for a
/// per-frame drain of a handful of windows' worth of input.
const EVENT_QUEUE_CAPACITY: usize = 256;

struct WindowState {
    wm_delete_window: u32,
    last_size: (u32, u32),
}

pub struct X11Connection {
    connection: XCBConnection,
    screen_num: usize,
    next_window_id: u64,
    windows: HashMap<WindowId, WindowState>,
    window_ids: HashMap<u32, WindowId>, // raw XCB window -> our WindowId
    event_queue: InputEventQueue,
}

impl X11Connection {
    pub fn new() -> Result<Self, PlatformError> {
        let (connection, screen_num) =
            XCBConnection::connect(None).map_err(|_| PlatformError::ConnectionFailed)?;
        Ok(Self {
            connection,
            screen_num,
            next_window_id: 0,
            windows: HashMap::new(),
            window_ids: HashMap::new(),
            event_queue: InputEventQueue::with_capacity(EVENT_QUEUE_CAPACITY),
        })
    }

    pub fn create_window(
        &mut self,
        title: &str,
        width: u32,
        height: u32,
    ) -> Result<WindowId, PlatformError> {
        let screen = self.connection.setup().roots[self.screen_num].clone();
        let window = self
            .connection
            .generate_id()
            .map_err(|e| PlatformError::Other(e.to_string()))?;

        self.connection
            .create_window(
                x11rb::COPY_DEPTH_FROM_PARENT,
                window,
                screen.root,
                0,
                0,
                width as u16,
                height as u16,
                0,
                WindowClass::INPUT_OUTPUT,
                screen.root_visual,
                &CreateWindowAux::new().event_mask(
                    EventMask::STRUCTURE_NOTIFY
                        | EventMask::BUTTON_PRESS
                        | EventMask::BUTTON_RELEASE
                        | EventMask::POINTER_MOTION
                        | EventMask::KEY_PRESS
                        | EventMask::KEY_RELEASE,
                ),
            )
            .map_err(|e| PlatformError::Other(e.to_string()))?;

        self.connection
            .change_property8(
                x11rb::protocol::xproto::PropMode::REPLACE,
                window,
                x11rb::protocol::xproto::AtomEnum::WM_NAME,
                x11rb::protocol::xproto::AtomEnum::STRING,
                title.as_bytes(),
            )
            .map_err(|e| PlatformError::Other(e.to_string()))?;

        let wm_protocols = self
            .connection
            .intern_atom(false, b"WM_PROTOCOLS")
            .map_err(|e| PlatformError::Other(e.to_string()))?
            .reply()
            .map_err(|e| PlatformError::Other(e.to_string()))?
            .atom;
        let wm_delete_window = self
            .connection
            .intern_atom(false, b"WM_DELETE_WINDOW")
            .map_err(|e| PlatformError::Other(e.to_string()))?
            .reply()
            .map_err(|e| PlatformError::Other(e.to_string()))?
            .atom;
        self.connection
            .change_property32(
                x11rb::protocol::xproto::PropMode::REPLACE,
                window,
                wm_protocols,
                x11rb::protocol::xproto::AtomEnum::ATOM,
                &[wm_delete_window],
            )
            .map_err(|e| PlatformError::Other(e.to_string()))?;

        self.connection
            .map_window(window)
            .map_err(|e| PlatformError::Other(e.to_string()))?;
        self.connection
            .flush()
            .map_err(|e| PlatformError::Other(e.to_string()))?;

        let id = WindowId(self.next_window_id);
        self.next_window_id += 1;
        self.window_ids.insert(window, id);
        self.windows.insert(
            id,
            WindowState {
                wm_delete_window,
                last_size: (width, height),
            },
        );
        Ok(id)
    }

    pub fn poll_events(&mut self) -> Vec<InputEvent> {
        while let Ok(Some(event)) = self.connection.poll_for_event() {
            match event {
                Event::ClientMessage(msg) => {
                    if let Some(&id) = self.window_ids.get(&msg.window) {
                        let expected = self.windows.get(&id).map_or(0, |w| w.wm_delete_window);
                        if msg.data.as_data32()[0] == expected {
                            self.event_queue
                                .push(InputEvent::CloseRequested { window: id });
                        }
                    }
                }
                Event::ConfigureNotify(cfg) => {
                    if let Some(&id) = self.window_ids.get(&cfg.window) {
                        let size = (u32::from(cfg.width), u32::from(cfg.height));
                        let changed = self.windows.get(&id).is_some_and(|w| w.last_size != size);
                        if changed {
                            if let Some(w) = self.windows.get_mut(&id) {
                                w.last_size = size;
                            }
                            self.event_queue.push(InputEvent::Resized {
                                window: id,
                                width: size.0,
                                height: size.1,
                            });
                        }
                    }
                }
                Event::MotionNotify(motion) => {
                    if let Some(&id) = self.window_ids.get(&motion.event) {
                        self.event_queue.push(InputEvent::PointerMoved {
                            window: id,
                            x: f64::from(motion.event_x),
                            y: f64::from(motion.event_y),
                        });
                    }
                }
                Event::ButtonPress(btn) => {
                    if let Some(&id) = self.window_ids.get(&btn.event) {
                        self.event_queue.push(InputEvent::PointerButton {
                            window: id,
                            button: x11_button_code_to_mouse_button(btn.detail),
                            state: ElementState::Pressed,
                        });
                    }
                }
                Event::ButtonRelease(btn) => {
                    if let Some(&id) = self.window_ids.get(&btn.event) {
                        self.event_queue.push(InputEvent::PointerButton {
                            window: id,
                            button: x11_button_code_to_mouse_button(btn.detail),
                            state: ElementState::Released,
                        });
                    }
                }
                Event::KeyPress(key) => {
                    if let Some(&id) = self.window_ids.get(&key.event) {
                        self.event_queue.push(InputEvent::KeyboardKey {
                            window: id,
                            key_code: x11_keycode_to_evdev(key.detail),
                            state: ElementState::Pressed,
                        });
                    }
                }
                Event::KeyRelease(key) => {
                    if let Some(&id) = self.window_ids.get(&key.event) {
                        self.event_queue.push(InputEvent::KeyboardKey {
                            window: id,
                            key_code: x11_keycode_to_evdev(key.detail),
                            state: ElementState::Released,
                        });
                    }
                }
                _ => {}
            }
        }
        self.event_queue.drain()
    }

    #[must_use]
    pub fn scale_factor(&self, _window: WindowId) -> i32 {
        // X11 has no equivalent to Wayland's per-output integer scale in
        // the core protocol; Xft.dpi / RandR-based scale detection is a
        // reasonable future addition, out of this step's scope.
        1
    }
}

/// X11 button `detail` codes (core protocol, `xproto`): 1=Left, 2=Middle,
/// 3=Right; 4/5 are the scroll wheel and 6/7 horizontal scroll reported as
/// legacy "buttons" -- there is no dedicated scroll `InputEvent` yet
/// (out of Step 2's scope), so they pass through as `Other`.
fn x11_button_code_to_mouse_button(code: u8) -> MouseButton {
    match code {
        1 => MouseButton::Left,
        2 => MouseButton::Middle,
        3 => MouseButton::Right,
        other => MouseButton::Other(u16::from(other)),
    }
}

/// X11/XKB keycodes on Linux are the evdev keycode plus a fixed offset of
/// 8 -- normalize back to the same evdev numbering `InputEvent::KeyboardKey`
/// documents (and that Wayland's `wl_keyboard` uses natively) so callers
/// see one consistent scheme regardless of backend.
fn x11_keycode_to_evdev(detail: u8) -> u32 {
    u32::from(detail).saturating_sub(8)
}

impl HasDisplayHandle for X11Connection {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        // SAFETY: raw pointer from `AsRawXcbConnection`, valid for as long
        // as `self.connection` is alive, which outlives this borrow.
        let conn_ptr = self.connection.as_raw_xcb_connection() as *mut c_void;
        let ptr = NonNull::new(conn_ptr);
        let handle = XcbDisplayHandle::new(ptr, self.screen_num as i32);
        Ok(unsafe { DisplayHandle::borrow_raw(RawDisplayHandle::Xcb(handle)) })
    }
}

impl X11Connection {
    /// # Errors
    /// Returns [`HandleError::Unavailable`] if `window` is not a window
    /// created by this connection.
    pub fn window_handle(&self, window: WindowId) -> Result<WindowHandle<'_>, HandleError> {
        let raw_window = self
            .window_ids
            .iter()
            .find(|(_, &id)| id == window)
            .map(|(&raw, _)| raw)
            .ok_or(HandleError::Unavailable)?;
        let handle =
            XcbWindowHandle::new(NonZeroU32::new(raw_window).ok_or(HandleError::Unavailable)?);
        // SAFETY: `raw_window` is a valid XCB window ID for as long as
        // `self.connection` is alive, which outlives this borrow.
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Xcb(handle)) })
    }
}
