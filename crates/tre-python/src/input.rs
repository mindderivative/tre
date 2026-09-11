//! Platform input events (`tre_engine::{WindowId, MouseButton,
//! ElementState, InputEvent}`), bound directly as real PyO3 types --
//! without these, `WindowedRenderer` would be a renderer, not a GUI
//! framework's backend (the project owner's own explicit framing).
//!
//! `InputEvent` is a PyO3 "complex enum" (each variant a real, distinct
//! Python class under `tre.InputEvent`, struct-like fields readable by
//! name and usable in a Python `match`/`case`) rather than four separate
//! ad hoc Python classes -- this is what current `pyo3` (0.27) supports
//! natively for a Rust enum-with-data, so binding it this way needs no
//! hand-rolled tagged-union workaround.

use pyo3::prelude::*;
use tre_engine::{ElementState, InputEvent, MouseButton, WindowId};

/// Opaque per-window identifier (`tre_engine::WindowId`). `frozen` +
/// `eq` + `hash` since a real UI framework's own event-routing code
/// needs to use this as a dict key.
#[pyclass(name = "WindowId", frozen, eq, hash)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PyWindowId(pub WindowId);

#[pymethods]
impl PyWindowId {
    fn __repr__(&self) -> String {
        format!("WindowId({})", self.0 .0)
    }
}

impl From<WindowId> for PyWindowId {
    fn from(id: WindowId) -> Self {
        Self(id)
    }
}

/// `tre_engine::MouseButton`. `Left`/`Right`/`Middle` are empty-tuple
/// variants (`Left()`, not a bare `Left`) rather than plain unit
/// variants -- a real PyO3 0.27 constraint on "complex" (data-carrying)
/// enums: mixing true unit variants with a data variant like `Other`
/// in the same `#[pyclass]` enum is a compile error ("Unit variant is
/// not yet supported in a complex enum"), found via a real build of
/// this module.
#[pyclass(name = "MouseButton", eq)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PyMouseButton {
    Left(),
    Right(),
    Middle(),
    /// A raw platform button code for buttons beyond the three common
    /// ones (e.g. side/forward-back buttons), passed through unchanged.
    Other(u16),
}

impl From<MouseButton> for PyMouseButton {
    fn from(b: MouseButton) -> Self {
        match b {
            MouseButton::Left => Self::Left(),
            MouseButton::Right => Self::Right(),
            MouseButton::Middle => Self::Middle(),
            MouseButton::Other(code) => Self::Other(code),
        }
    }
}

/// `tre_engine::ElementState`.
#[pyclass(name = "ElementState", eq, eq_int)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PyElementState {
    Pressed,
    Released,
}

impl From<ElementState> for PyElementState {
    fn from(s: ElementState) -> Self {
        match s {
            ElementState::Pressed => Self::Pressed,
            ElementState::Released => Self::Released,
        }
    }
}

/// `tre_engine::InputEvent` -- see this module's own doc comment for why
/// this is one real PyO3 complex enum rather than four separate classes.
/// `key_code` (`KeyboardKey`) is the raw platform (Linux evdev) keycode,
/// exactly as `tre_engine::InputEvent`'s own doc comment already
/// contracts -- layout-aware translation is left to the Python UI
/// framework, matching the Rust engine's own documented scope boundary.
#[pyclass(name = "InputEvent")]
#[derive(Clone)]
pub enum PyInputEvent {
    PointerMoved {
        window: PyWindowId,
        x: f64,
        y: f64,
    },
    PointerButton {
        window: PyWindowId,
        button: PyMouseButton,
        state: PyElementState,
    },
    KeyboardKey {
        window: PyWindowId,
        key_code: u32,
        state: PyElementState,
    },
    CloseRequested {
        window: PyWindowId,
    },
    Resized {
        window: PyWindowId,
        width: u32,
        height: u32,
    },
}

impl From<InputEvent> for PyInputEvent {
    fn from(event: InputEvent) -> Self {
        match event {
            InputEvent::PointerMoved { window, x, y } => Self::PointerMoved {
                window: window.into(),
                x,
                y,
            },
            InputEvent::PointerButton {
                window,
                button,
                state,
            } => Self::PointerButton {
                window: window.into(),
                button: button.into(),
                state: state.into(),
            },
            InputEvent::KeyboardKey {
                window,
                key_code,
                state,
            } => Self::KeyboardKey {
                window: window.into(),
                key_code,
                state: state.into(),
            },
            InputEvent::CloseRequested { window } => Self::CloseRequested {
                window: window.into(),
            },
            InputEvent::Resized {
                window,
                width,
                height,
            } => Self::Resized {
                window: window.into(),
                width,
                height,
            },
        }
    }
}
