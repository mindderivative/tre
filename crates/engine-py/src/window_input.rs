//! `PyWindow`'s `resize` and clipboard methods. M100 removed the
//! synthetic-input methods that lived here (`click`, `hover`, `scroll`,
//! `press_key`, `type_text`, `copy`, `cut`, `paste`, ...): `simulate`
//! drives the same input pipeline a live window does.

use engine_core::InputEvent;
use pyo3::prelude::*;

use crate::dispatch::run_dispatch_outcome;
use crate::window::PyWindow;

#[pymethods]
impl PyWindow {
    /// M32 Phase 2 (§4, §5): a direct, programmatic "resize this
    /// window" entry point, for headless use. Updates both
    /// `self.handles.width`/`self.handles.height` -- which layout reads -- and
    /// dispatches the real
    /// `InputEvent::Resized` (`Tree::dispatch` mutates `root`'s own
    /// `layout_style.size` directly for this event, see its own doc
    /// comment).
    ///
    /// M33 Phase 2 (§4, §5, §8) closed the real, stated v1 limit this
    /// doc comment used to state here: `self.handles.width`/`height` are now a
    /// real, shared `SharedSize` (`Rc<Cell<u32>>`, `window.rs`'s own
    /// doc comment has the full real reasoning) -- `App::run`'s own
    /// `WindowRuntime` clones the identical `Rc`, so a real live
    /// winit-driven resize's own `.set()` call (`app.rs`'s `InputEvent::
    /// Resized` arm) is immediately visible here too, and vice versa.
    fn resize(&mut self, width: u32, height: u32, py: Python<'_>) {
        // 0.5.4 (#102): `width` and `height` are logical; the stored size is
        // the physical one.
        let scale = self.handles.scale.get();
        self.handles
            .width
            .set(crate::scale::to_physical(f64::from(width), scale));
        self.handles
            .height
            .set(crate::scale::to_physical(f64::from(height), scale));
        let (tree, root, handlers) = (
            self.handles.tree.clone(),
            self.handles.root,
            self.handles.handlers.clone(),
        );
        let outcome = tree.borrow_mut().dispatch(
            root,
            InputEvent::Resized {
                width: width as f32,
                height: height as f32,
            },
            crate::clock::now(&tree),
        );
        // `Resized` always dispatches to `DispatchOutcome::None`
        // (`Tree::dispatch`'s own doc comment) -- no real click/hover/
        // change outcome to build an `Event` for, so `event: None` here
        // never actually reaches a handler; kept honest rather than
        // reconstructing the `Resized` event just to thread through.
        run_dispatch_outcome(
            &handlers,
            &tree,
            &self.handles.completions,
            &outcome,
            None,
            py,
        );
    }

    /// M100: the OS clipboard's text, or `None` when it holds no text or
    /// can't be reached -- a headless environment may have no clipboard
    /// service (logged, never raised).
    fn read_clipboard(&self) -> Option<String> {
        crate::dispatch::read_clipboard()
    }

    /// M100: puts `text` on the OS clipboard. `False` when the clipboard
    /// can't be reached (logged, never raised).
    fn write_clipboard(&self, text: &str) -> bool {
        crate::dispatch::write_clipboard(text)
    }
}
