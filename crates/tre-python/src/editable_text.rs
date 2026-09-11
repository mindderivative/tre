//! `tre.EditableText` (Phase 13 Step 13.5: editable text) -- a real,
//! single-line text-editing model wrapping the same `Font`/`Text`
//! primitives Step 12.2/12.3 already established, built on:
//!
//! - `tre_text::caret_positions`/`hit_test` (new this step) for real
//!   caret placement, using the exact same pen-advance formula
//!   `tre_engine::text::flatten_text` itself renders with -- confirmed
//!   by reading that function's own source before writing this binding,
//!   not assumed -- so a caret computed here lands exactly where the
//!   matching glyph actually renders.
//! - The real `ImeEnabled`/`ImePreedit`/`ImeCommit`/`ImeDisabled` events
//!   already forwarded end to end since Step 12.7.
//!
//! **Real, disclosed scope limit**: single-line only, matching
//! `tre_engine::Text`'s own single-line-only scope as of Step 12.2 --
//! this covers the overwhelming majority of real GUI text input (form
//! fields, search boxes). Multi-line/word-wrap *rendering* (`Text.
//! wrap_width`) and multi-line *editing* both shipped later, in Phase
//! 15 Steps 15.1/15.2 -- this correction replaces an earlier, INACCURATE
//! claim here that multi-line label rendering already existed at this
//! step; direct reading of `tre_engine::text::flatten_text` at the time
//! Phase 15 was planned confirmed it never had (single straight pen
//! line, no `\n` handling at all). Caret/selection byte offsets are
//! also LTR-only-correct (matching `tre_text::caret_positions`'s own
//! disclosed limitation).

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use skrifa::MetadataProvider;

use crate::clipboard::PyClipboard;
use crate::error::TreError;
use crate::font::PyFont;
use crate::input::PyInputEvent;
use crate::shapes::PyText;

#[pyclass(name = "EditableText")]
pub struct PyEditableText {
    #[pyo3(get, set)]
    pub x: f32,
    #[pyo3(get, set)]
    pub y: f32,
    #[pyo3(get, set)]
    pub text: String,
    #[pyo3(get, set)]
    pub font: Py<PyFont>,
    #[pyo3(get, set)]
    pub px_size: f32,
    #[pyo3(get, set)]
    pub fill_color: u32,
    /// A real byte offset into `text`, always a valid UTF-8 char
    /// boundary (every mutating method here maintains this invariant).
    #[pyo3(get)]
    pub caret: usize,
    /// The OTHER end of a real selection range, if one is active --
    /// `None` means no selection (a plain caret). The selection itself
    /// spans `min(caret, selection_anchor)..max(caret, selection_anchor)`.
    #[pyo3(get)]
    pub selection_anchor: Option<usize>,
    /// The current IME composing (not yet committed) substring, shown
    /// at the caret position by `to_text()` but not part of `text`
    /// itself until a real `ImeCommit` event arrives.
    #[pyo3(get)]
    pub preedit: String,
    #[pyo3(get)]
    pub ime_active: bool,
}

#[pymethods]
impl PyEditableText {
    #[new]
    fn new(x: f32, y: f32, text: String, font: Py<PyFont>, px_size: f32, fill_color: u32) -> Self {
        let caret = text.len();
        Self {
            x,
            y,
            text,
            font,
            px_size,
            fill_color,
            caret,
            selection_anchor: None,
            preedit: String::new(),
            ime_active: false,
        }
    }

    /// Deletes any active selection, then inserts `text` at the caret,
    /// advancing the caret past it.
    fn insert(&mut self, text: &str) {
        self.delete_selection();
        self.text.insert_str(self.caret, text);
        self.caret += text.len();
    }

    /// Deletes the active selection, if any, moving the caret to the
    /// selection's own start. Returns whether a selection was actually
    /// active (and therefore deleted).
    fn delete_selection(&mut self) -> bool {
        if let Some(anchor) = self.selection_anchor.take() {
            let (start, end) = if anchor < self.caret {
                (anchor, self.caret)
            } else {
                (self.caret, anchor)
            };
            self.text.replace_range(start..end, "");
            self.caret = start;
            true
        } else {
            false
        }
    }

    /// Copies the active selection's own text to `clipboard`, leaving
    /// `text` unchanged -- a real no-op (not an error) when no
    /// selection is active, matching every real text field's own
    /// standard "copy with nothing selected does nothing" behavior.
    ///
    /// # Errors
    /// Raises whatever `clipboard.set_text` itself raises (a real
    /// platform clipboard failure).
    fn copy(&self, clipboard: &mut PyClipboard) -> PyResult<()> {
        if let Some(selected) = self.selected_text() {
            clipboard.set_text(selected)?;
        }
        Ok(())
    }

    /// Copies the active selection's own text to `clipboard`, then
    /// deletes it from `text` -- a real no-op (not an error) when no
    /// selection is active, matching `copy`'s own real "nothing
    /// selected, nothing happens" behavior.
    ///
    /// # Errors
    /// Raises whatever `clipboard.set_text` itself raises. If it fails,
    /// `text` is left unchanged (the delete never runs) -- a real
    /// caller never loses text to a clipboard write it can't observe
    /// succeeded.
    fn cut(&mut self, clipboard: &mut PyClipboard) -> PyResult<()> {
        if let Some(selected) = self.selected_text() {
            clipboard.set_text(selected)?;
            self.delete_selection();
        }
        Ok(())
    }

    /// Reads `clipboard`'s current real text and inserts it at the
    /// caret via `insert()` -- replacing the active selection first, if
    /// any, the same real semantics `insert()` already has everywhere
    /// else in this class.
    ///
    /// # Errors
    /// Raises whatever `clipboard.get_text` itself raises (e.g. the
    /// clipboard is empty or holds non-text content).
    fn paste(&mut self, clipboard: &mut PyClipboard) -> PyResult<()> {
        let text = clipboard.get_text()?;
        self.insert(&text);
        Ok(())
    }

    /// Deletes the active selection if any; otherwise deletes exactly
    /// one real character (not byte) before the caret -- the standard
    /// "Backspace" behavior.
    fn delete_backward(&mut self) {
        if self.delete_selection() {
            return;
        }
        if let Some(ch) = self.text[..self.caret].chars().next_back() {
            let new_caret = self.caret - ch.len_utf8();
            self.text.replace_range(new_caret..self.caret, "");
            self.caret = new_caret;
        }
    }

    /// Moves the caret to `byte_offset`, clearing any active selection.
    ///
    /// # Errors
    /// Raises `ValueError` if `byte_offset` is not a real UTF-8 char
    /// boundary in `text` (e.g. splitting a multi-byte character).
    fn set_caret(&mut self, byte_offset: usize) -> PyResult<()> {
        if !self.text.is_char_boundary(byte_offset) {
            return Err(PyValueError::new_err(format!(
                "{byte_offset} is not a valid UTF-8 char boundary in this text"
            )));
        }
        self.caret = byte_offset;
        self.selection_anchor = None;
        Ok(())
    }

    /// Sets a real selection range from `anchor` to `caret` (order
    /// doesn't matter -- `anchor`/`caret` may be given in either order).
    ///
    /// # Errors
    /// Raises `ValueError` if either offset is not a real UTF-8 char
    /// boundary in `text`.
    fn set_selection(&mut self, anchor: usize, caret: usize) -> PyResult<()> {
        if !self.text.is_char_boundary(anchor) || !self.text.is_char_boundary(caret) {
            return Err(PyValueError::new_err(
                "anchor/caret must both be valid UTF-8 char boundaries",
            ));
        }
        self.selection_anchor = Some(anchor);
        self.caret = caret;
        Ok(())
    }

    /// Finds the real caret byte offset nearest to pixel position `x`
    /// (relative to this text's own pen start, matching
    /// `tre_engine::Text`'s own rendering origin) -- reshapes `text`
    /// against `font` fresh each call (a real, cheap operation: the same
    /// shaping `tre_engine::Text` itself performs every time it
    /// re-flattens, not a new cost this method introduces).
    ///
    /// # Errors
    /// Raises `TreError` if `text` fails to shape against `font` (the
    /// same real, rare failure `tre_engine::Text`'s own rendering path
    /// can hit).
    fn hit_test(&self, py: Python<'_>, x: f32) -> PyResult<usize> {
        let font = self.font.borrow(py);
        let face_ref = skrifa::FontRef::new(&font.bytes)
            .expect("Font bytes already validated real at PyFont construction time");
        let rb_face = rustybuzz::Face::from_slice(&font.bytes, 0)
            .expect("Font bytes already validated real at PyFont construction time");
        let runs = tre_text::shape_text(&rb_face, &self.text)
            .map_err(|e| TreError::new_err(e.to_string()))?;
        let metrics = face_ref.metrics(
            skrifa::instance::Size::unscaled(),
            skrifa::instance::LocationRef::default(),
        );
        let positions =
            tre_text::caret_positions(&runs, self.text.len(), self.px_size, metrics.units_per_em);
        Ok(tre_text::hit_test(&positions, x))
    }

    /// Dispatches one real IME event (`tre.InputEvent.ImeEnabled`/
    /// `ImePreedit`/`ImeCommit`/`ImeDisabled`, forwarded end to end
    /// since Step 12.7) -- `ImePreedit` updates the composing
    /// `preedit` string (not yet part of `text`); `ImeCommit` splices
    /// its own text into `text` at the caret via `insert()` and clears
    /// `preedit`. Any other `InputEvent` variant is a real no-op (a
    /// caller is expected to forward its own full `poll_events()` list
    /// here without pre-filtering).
    fn handle_ime(&mut self, event: PyInputEvent) {
        match event {
            PyInputEvent::ImeEnabled { .. } => self.ime_active = true,
            PyInputEvent::ImePreedit { text, .. } => self.preedit = text,
            PyInputEvent::ImeCommit { text, .. } => {
                self.preedit.clear();
                self.insert(&text);
            }
            PyInputEvent::ImeDisabled { .. } => {
                self.ime_active = false;
                self.preedit.clear();
            }
            _ => {}
        }
    }

    /// Builds a real `tre.Text` reflecting this editable text's current
    /// state -- `text` with any active `preedit` spliced in at the
    /// caret for real visual composing feedback -- ready to insert into
    /// a `ShapeRegistry` and render, the same "rebuild each frame"
    /// pattern every other shape in this project already uses.
    fn to_text(&self, py: Python<'_>) -> PyResult<Py<PyText>> {
        let display_text = if self.preedit.is_empty() {
            self.text.clone()
        } else {
            let mut text = self.text.clone();
            text.insert_str(self.caret, &self.preedit);
            text
        };
        Py::new(
            py,
            PyText {
                x: self.x,
                y: self.y,
                text: display_text,
                font: self.font.clone_ref(py),
                px_size: self.px_size,
                fill_color: self.fill_color,
                opacity: 1.0,
                scale_x: 1.0,
                scale_y: 1.0,
                rotation: 0.0,
                wrap_width: None,
            },
        )
    }
}

impl PyEditableText {
    /// The active selection's own substring of `text`, if a selection
    /// is active -- `None` (not an empty string) when it isn't, so
    /// `copy`/`cut` can tell "no selection" apart from "an empty
    /// selection" (the latter can't actually occur here, since
    /// `set_selection`/click-drag never produces a zero-width range in
    /// real use, but the `Option` keeps that distinction explicit
    /// rather than relying on an empty-string convention).
    fn selected_text(&self) -> Option<&str> {
        let anchor = self.selection_anchor?;
        let (start, end) = if anchor < self.caret {
            (anchor, self.caret)
        } else {
            (self.caret, anchor)
        };
        Some(&self.text[start..end])
    }
}
