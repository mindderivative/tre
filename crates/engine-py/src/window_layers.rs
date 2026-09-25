//! M96: `window.show_layer` and `window.hide_layer` -- the one mechanism
//! dialogs, menus, tooltips, snackbars, sheets, and drawers are built from
//! (target API, "Layers"). A layer stacks over the window's content in show
//! order, is placed against an optional anchor at every layout, and -- when
//! modal -- blocks input beneath it and holds focus until hidden. Its
//! dismissal (an outside press or Escape) arrives as a `dismiss` event; what
//! that means is the framework's call. `tre` draws no scrim (R12).

use std::rc::Rc;

use engine_core::{FocusDirection, NodeId, OverlayMeta, Placement};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::dispatch::{fire_focus_transition, interaction_config};
use crate::error::EngineError;
use crate::node::Node;
use crate::node_handles;
use crate::window::PyWindow;

pub(crate) const PLACEMENT: [(&str, Placement); 4] = [
    ("below", Placement::Below),
    ("above", Placement::Above),
    ("start", Placement::Start),
    ("end", Placement::End),
];

type FocusTransition = Option<(Option<NodeId>, Option<NodeId>)>;

#[pymethods]
impl PyWindow {
    /// Shows `node` as a layer over the window's content, on top of any
    /// layer already open. With `anchor`, it's placed against that node on
    /// the `placement` side (`"below"`, `"above"`, `"start"`, `"end"`),
    /// flipped or shifted to fit the window -- `node.get("layer_placement")`
    /// reports the side used; without one it sits at its own `x`/`y`.
    /// `modal` blocks input beneath it and moves focus into it;
    /// `dismissible` delivers `dismiss` to it on an outside press or Escape.
    #[pyo3(signature = (node, anchor=None, placement="below", modal=false, dismissible=true))]
    fn show_layer(
        &self,
        node: PyRef<'_, Node>,
        anchor: Option<PyRef<'_, Node>>,
        placement: &str,
        modal: bool,
        dismissible: bool,
        py: Python<'_>,
    ) -> PyResult<()> {
        let (tree, root) = {
            let active = self.active.borrow();
            (active.tree.clone(), active.root)
        };
        for handle in std::iter::once(&node).chain(anchor.as_ref()) {
            if !Rc::ptr_eq(&handle.tree, &tree) {
                return Err(EngineError::ForeignNode.into());
            }
            handle.check_alive()?;
        }
        let placement = crate::node_layout::lookup(&PLACEMENT, placement).map_err(|expected| {
            PyValueError::new_err(format!("show_layer: `placement` must be {expected}"))
        })?;
        let restore_focus = tree.borrow().focused();
        let meta = OverlayMeta {
            anchor: anchor.map(|a| a.id),
            modal,
            dismissible,
            placement: Some(placement),
            restore_focus,
            ..Default::default()
        };
        if !tree.borrow_mut().show_layer(root, node.id, meta) {
            return Err(EngineError::CycleRejected.into());
        }
        if modal {
            // Focus moves into the layer: Tab's scope is now the layer.
            let config = interaction_config();
            let transition = tree.borrow_mut().move_focus(
                root,
                FocusDirection::Next,
                config.focus_ring_opacity,
                config.focus_ring_duration,
                crate::clock::now(&tree),
            );
            self.fire_focus(transition, py);
        }
        Ok(())
    }

    /// Hides the layer `node`: it's detached -- still alive while you hold
    /// it, and shown again with `show_layer` -- and focus inside it returns
    /// to the node that held it when the layer opened.
    fn hide_layer(&self, node: PyRef<'_, Node>, py: Python<'_>) -> PyResult<()> {
        let (tree, handlers) = {
            let active = self.active.borrow();
            (active.tree.clone(), active.handlers.clone())
        };
        if !Rc::ptr_eq(&node.tree, &tree) {
            return Err(EngineError::ForeignNode.into());
        }
        let focus_inside = {
            let tree = tree.borrow();
            tree.focused()
                .is_some_and(|focused| tree.ancestors(focused).any(|id| id == node.id))
        };
        let Some(meta) = tree.borrow_mut().hide_layer(node.id) else {
            return Err(PyValueError::new_err(
                "hide_layer: this node isn't a shown layer",
            ));
        };
        if focus_inside {
            let config = interaction_config();
            let now = crate::clock::now(&tree);
            let restore = meta.restore_focus.filter(|id| {
                let tree = tree.borrow();
                tree.get(*id).is_some() && !tree.ancestors(*id).any(|a| a == node.id)
            });
            let transition = match restore {
                Some(id) => tree.borrow_mut().set_focus_to(
                    id,
                    config.focus_ring_opacity,
                    config.focus_ring_duration,
                    now,
                ),
                None => tree.borrow_mut().clear_focus(
                    config.focus_ring_opacity,
                    config.focus_ring_duration,
                    now,
                ),
            };
            self.fire_focus(transition, py);
        }
        node_handles::collect(&tree, &handlers, node.id);
        Ok(())
    }
}

impl PyWindow {
    /// Fires `focus`/`blur` (and the legacy focus handlers) for a focus
    /// change a layer made.
    fn fire_focus(&self, transition: FocusTransition, py: Python<'_>) {
        let Some((old, new)) = transition else {
            return;
        };
        let (tree, handlers, context_menus) = {
            let active = self.active.borrow();
            (
                active.tree.clone(),
                active.handlers.clone(),
                active.context_menus.clone(),
            )
        };
        fire_focus_transition(
            &handlers,
            &tree,
            &context_menus,
            &self.theme,
            &self.completions,
            old,
            new,
            py,
        );
    }
}
