//! `PyWindow`'s docking delegation (review follow-through, M28 Phase 2,
//! §4/§8): thin wrappers over `dock.rs`'s own real docking mechanism --
//! every one of these methods does its real work in `dock`, not here.
//! See `window_factory.rs`'s own doc comment for why this was split
//! out.

use std::rc::Rc;

use peniko::kurbo::Point;
use pyo3::prelude::*;
use taffy::prelude::{AvailableSpace, Size};

use crate::dock;
use crate::error::EngineError;
use crate::node::Node;
use crate::window::PyWindow;

#[pymethods]
impl PyWindow {
    /// M4 Phase 9 (§11.4): registers `container` as `side`'s real dock
    /// zone -- a plain node the app already built (e.g. via `add_rect`),
    /// exactly like `docking.rs`'s own Rust-level proof (M3 step 15
    /// Stage B) builds one by hand. `size` seeds the zone's own
    /// `Animated<f64>` extent (§11.4's own struct sketch).
    fn add_dock_zone(&self, side: &str, container: PyRef<'_, Node>, size: f64) -> PyResult<()> {
        let side = dock::parse_dock_side(side)?;
        dock::add_dock_zone(&self.dock, side, container.id, size);
        Ok(())
    }

    /// Real initial "put a panel in this zone" setup -- attaches
    /// `panel` as `side`'s new active tab via the existing real
    /// `Tree::apply_active_tab` (M3 step 15 Stage B), not a second
    /// resize/attach mechanism.
    fn dock_panel(&self, side: &str, panel: PyRef<'_, Node>) -> PyResult<()> {
        let side = dock::parse_dock_side(side)?;
        dock::dock_panel(&self.dock, &self.tree, side, panel.id)
    }

    /// Switches `side`'s own active tab by index -- the same plain
    /// index switch §11.4's own text describes, via `Tree::
    /// apply_active_tab`.
    fn set_active_tab(&self, side: &str, index: usize) -> PyResult<()> {
        let side = dock::parse_dock_side(side)?;
        dock::set_active_tab(&self.dock, &self.tree, side, index)
    }

    /// Registers `handle` as `panel`'s real drag handle -- pressing
    /// `handle` (via a real mouse press or `start_panel_drag`) starts
    /// tracking a drag of `panel`, not `handle` itself, mirroring
    /// `set_context_menu`'s own "anchor names a different node" shape
    /// (M4 Phase 7).
    ///
    /// M10 Phase 2 (§8): both `handle` and `panel` must belong to this
    /// same `Window`'s own `Tree` -- the same real `Rc::ptr_eq` guard
    /// `Node.add_child`/`Node.set_context_menu`/`Window.begin_
    /// container_transform` already use, mirrored here for the same
    /// real reason (a foreign `NodeId` could alias an unrelated real
    /// node the next time it's read back).
    fn set_dock_handle(&self, handle: PyRef<'_, Node>, panel: PyRef<'_, Node>) -> PyResult<()> {
        if !Rc::ptr_eq(&self.tree, &handle.tree) || !Rc::ptr_eq(&self.tree, &panel.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        dock::set_dock_handle(&self.dock, handle.id, panel.id);
        Ok(())
    }

    /// M10 Phase 3 (§11.4): registers `content` as this `Window`'s
    /// single drop-zone highlight -- detached from root immediately (it
    /// starts hidden, the same `set_context_menu`-style contract), then
    /// shown, resized, and repositioned by `drag_panel_over` to cover
    /// whichever registered zone is currently under the pointer during
    /// a drag, and hidden again by `drop_panel_at`. The same `Rc::
    /// ptr_eq` same-tree guard M10 Phase 2 added to `set_dock_handle`,
    /// applied here for the same real reason.
    fn set_drop_zone_highlight(&self, content: PyRef<'_, Node>) -> PyResult<()> {
        if !Rc::ptr_eq(&self.tree, &content.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        dock::set_drop_zone_highlight(&self.dock, &self.tree, content.id);
        Ok(())
    }

    /// M10 Phase 3's own no-live-window-needed proof pattern (matching
    /// `start_panel_drag`/`drop_panel_at`): the real "drag in progress"
    /// step -- hit-tests `(x, y)` against this window's own real,
    /// current layout (computed fresh here, the same reasoning `.click(
    /// )`/`drop_panel_at` already state) and shows the registered
    /// highlight over whichever registered zone encloses that point, or
    /// hides it if none does. A safe no-op if no drag is in progress or
    /// no highlight is registered.
    fn drag_panel_over(&self, x: f64, y: f64) {
        {
            let mut tree = self.tree.borrow_mut();
            tree.compute_layout(
                self.root,
                Size {
                    width: AvailableSpace::Definite(self.width as f32),
                    height: AvailableSpace::Definite(self.height as f32),
                },
            );
        }
        dock::drag_over(&self.dock, &self.tree, self.root, Point::new(x, y));
    }

    /// M4 Phase 9's own no-live-window-needed proof pattern (matching
    /// `.click()`/`.hover()`/`.right_click()`): starts tracking a real
    /// drag as if `handle` had just been pressed. Returns whether a
    /// drag actually started -- `handle` must already be registered via
    /// `set_dock_handle`.
    fn start_panel_drag(&self, handle: PyRef<'_, Node>) -> bool {
        dock::start_drag(&self.dock, handle.id)
    }

    /// The real "release" half of a drag -- hit-tests `(x, y)` against
    /// this window's own real, current layout (computed fresh here, the
    /// same "nothing else does this for a `Window` with no render loop
    /// attached" reasoning `.click()` already states) and reparents the
    /// dragged panel into whichever registered zone encloses that
    /// point, if any and if different from its current zone.
    fn drop_panel_at(&self, x: f64, y: f64) {
        {
            let mut tree = self.tree.borrow_mut();
            tree.compute_layout(
                self.root,
                Size {
                    width: AvailableSpace::Definite(self.width as f32),
                    height: AvailableSpace::Definite(self.height as f32),
                },
            );
        }
        dock::end_drag_at(&self.dock, &self.tree, self.root, Point::new(x, y));
    }
}
