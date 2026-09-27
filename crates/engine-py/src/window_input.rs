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
    /// `self.width`/`self.height` -- which layout reads -- and
    /// dispatches the real
    /// `InputEvent::Resized` (`Tree::dispatch` mutates `root`'s own
    /// `layout_style.size` directly for this event, see its own doc
    /// comment).
    ///
    /// M33 Phase 2 (§4, §5, §8) closed the real, stated v1 limit this
    /// doc comment used to state here: `self.width`/`height` are now a
    /// real, shared `SharedSize` (`Rc<Cell<u32>>`, `window.rs`'s own
    /// doc comment has the full real reasoning) -- `App::run`'s own
    /// `WindowRuntime` clones the identical `Rc`, so a real live
    /// winit-driven resize's own `.set()` call (`app.rs`'s `InputEvent::
    /// Resized` arm) is immediately visible here too, and vice versa.
    fn resize(&mut self, width: u32, height: u32, py: Python<'_>) {
        self.width.set(width);
        self.height.set(height);
        // M57 (§8): the `Tree::dispatch` call routes through `self.
        // active`, not `self.tree`/`self.root`/`self.handlers`/`self.
        // context_menus` directly -- a real resize must mutate whichever
        // View is currently shown, the identical real staleness fix
        // every other "act on the currently active view" method already
        // got. `self.width`/`height.set()` above stay window-level,
        // correctly unaffected -- a real, shared `SharedSize` regardless
        // of which View is currently active.
        let (tree, root, handlers) = (self.tree.clone(), self.root, self.handlers.clone());
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
        run_dispatch_outcome(&handlers, &tree, &self.completions, &outcome, None, py);
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
