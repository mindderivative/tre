//! `PyWindow`'s synthetic input dispatch (review follow-through, M28
//! Phase 2, §4/§8): `click`/`hover`/`scroll`/`right_click`/`press_key`/
//! `type_text`/`copy`/`cut`/`paste`, plus container-transform
//! choreography -- every method that drives `Tree::dispatch`/the
//! clipboard from Python without a real platform input event behind
//! it. See `window_factory.rs`'s own doc comment for why this was
//! split out.

use std::rc::Rc;

use engine_core::{EventKind, InputEvent, Key, PointerButton};
use pyo3::prelude::*;
use taffy::prelude::{AvailableSpace, Size};

use crate::dispatch::{
    call_handler, interaction_config, node_center, open_context_menu, run_dispatch_outcome,
};
use crate::error::EngineError;
use crate::node::Node;
use crate::window::PyWindow;

#[pymethods]
impl PyWindow {
    /// M7 Phase 5 (§7.6): starts a real container-transform choreography
    /// between `trigger` and `destination` -- `destination` must already
    /// be attached to this same `Window`'s tree, laid out, and carry its
    /// own real target appearance (this call captures that as the
    /// animation's target before overwriting it to `trigger`'s own
    /// captured from-state; nothing visually changes until the next
    /// tick). `curve` defaults to `MotionCurve::Emphasized` -- §7.5's
    /// own text names this as container-transform's typical real curve.
    /// Computes layout first (the same reason `click`/`hover` do) so the
    /// captured bounds are fresh, not stale from before this call.
    ///
    /// M9 Phase 3 (§5): `on_complete`, when given, is called with no
    /// arguments exactly once, the real tick the whole transition
    /// genuinely finishes -- registered the same real way `Node.
    /// animate(..., on_complete=...)` already is, and wired onto the
    /// destination's own driven `transform` animation (`engine_md3::
    /// container_transform::begin`'s own doc comment: all four driven
    /// properties share one `start`/`duration`, so any one of them
    /// completing is enough). A real app can now pass a callback that
    /// calls `end_container_transform` and get automatic teardown --
    /// the exact gap `container_transform.rs`'s own doc comment named
    /// as confirmed-still-unwired before this phase.
    #[pyo3(signature = (trigger, destination, duration_ms=300, content_stagger_ms=90, on_complete=None))]
    fn begin_container_transform(
        &self,
        trigger: PyRef<'_, Node>,
        destination: PyRef<'_, Node>,
        duration_ms: u64,
        content_stagger_ms: u64,
        on_complete: Option<Py<PyAny>>,
    ) -> PyResult<()> {
        if !Rc::ptr_eq(&self.tree, &trigger.tree) || !Rc::ptr_eq(&self.tree, &destination.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        let mut tree = self.tree.borrow_mut();
        tree.compute_layout(
            self.root,
            Size {
                width: AvailableSpace::Definite(self.width as f32),
                height: AvailableSpace::Definite(self.height as f32),
            },
        );
        let config = engine_md3::ContainerTransformConfig {
            duration: std::time::Duration::from_millis(duration_ms),
            curve: engine_core::MotionCurve::Emphasized,
            content_stagger: std::time::Duration::from_millis(content_stagger_ms),
        };
        let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
        engine_md3::begin_container_transform(
            &mut tree,
            trigger.id,
            destination.id,
            &config,
            std::time::Instant::now(),
            handle,
        );
        Ok(())
    }

    /// M7 Phase 5 (§7.6, step 5): "the trigger node is hidden or
    /// removed" -- called once the caller knows the transition started
    /// by `begin_container_transform` has finished (this codebase has no
    /// real completion-queue wiring to fire it automatically, a
    /// confirmed, stated gap -- see `container_transform.rs`'s own doc
    /// comment). A plain, ordinary tree mutation: detaches `trigger`
    /// from its own parent via the already-real `Tree::detach`.
    fn end_container_transform(&self, trigger: PyRef<'_, Node>) -> PyResult<()> {
        if !Rc::ptr_eq(&self.tree, &trigger.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        engine_md3::teardown_container_transform(&mut self.tree.borrow_mut(), trigger.id);
        Ok(())
    }

    /// M4 Phase 1 step 3 (§11.10): a direct, programmatic "click this
    /// node" entry point -- the same "expose a direct method since real
    /// pointer dispatch has nowhere else to originate outside a live
    /// window" pattern every prior interaction step used (`Tree::
    /// spawn_ripple`, `Tree::open_overlay`, etc.), and this step's own
    /// real, no-window-needed way to prove `set_on_click` actually
    /// fires. Computes layout first (so `node`'s own bounds are current
    /// -- the real render loop does this every frame; nothing else does
    /// for a `Window` with no render loop attached), then dispatches a
    /// primary-button press+release pair at `node`'s own real center
    /// point -- exactly what a real mouse click there would produce.
    fn click(&self, node: PyRef<'_, Node>, py: Python<'_>) {
        let point = node_center(
            &self.tree,
            self.root,
            Size {
                width: AvailableSpace::Definite(self.width as f32),
                height: AvailableSpace::Definite(self.height as f32),
            },
            node.id,
        );

        let now = std::time::Instant::now();
        let config = interaction_config();
        // Each `dispatch` call's own `self.tree.borrow_mut()` is a
        // short-lived temporary, released before `run_dispatch_outcome` runs
        // -- a click handler that itself touches this same `Tree` (e.g.
        // animating the very node it's attached to, a real, plausible
        // pattern) would otherwise panic on a re-entrant borrow.
        let press = self.tree.borrow_mut().dispatch(
            self.root,
            InputEvent::PointerPressed {
                position: point,
                button: PointerButton::Primary,
            },
            &config,
            now,
        );
        run_dispatch_outcome(&self.handlers, press, py);

        let release = self.tree.borrow_mut().dispatch(
            self.root,
            InputEvent::PointerReleased {
                position: point,
                button: PointerButton::Primary,
            },
            &config,
            now,
        );
        run_dispatch_outcome(&self.handlers, release, py);
    }

    /// M4 Phase 6 (§7.3): `click()`'s own hover counterpart -- the same
    /// no-live-window-needed proof pattern, this time dispatching a
    /// `PointerMoved` at `node`'s own real center point, exactly what a
    /// real mouse arriving there would produce. Fires `HoverEnter`/
    /// `HoverExit` (via `Tree::dispatch`'s own `DispatchOutcome::
    /// HoverChanged`) independent of whether `node` ever called
    /// `enable_interaction()` -- §7.3's own text: the event fires
    /// regardless of whether the default MD3 visual is enabled.
    fn hover(&self, node: PyRef<'_, Node>, py: Python<'_>) {
        let point = node_center(
            &self.tree,
            self.root,
            Size {
                width: AvailableSpace::Definite(self.width as f32),
                height: AvailableSpace::Definite(self.height as f32),
            },
            node.id,
        );

        let outcome = self.tree.borrow_mut().dispatch(
            self.root,
            InputEvent::PointerMoved { position: point },
            &interaction_config(),
            std::time::Instant::now(),
        );
        run_dispatch_outcome(&self.handlers, outcome, py);
    }

    /// M8 Phase 3 (§11.7): `click()`/`hover()`'s own scroll counterpart
    /// -- the same no-live-window-needed proof pattern, dispatching a
    /// real `InputEvent::Scroll` at `node`'s own real center point,
    /// exactly what a real mouse wheel over it would produce. `delta_y`
    /// is real pixels (`ScrollDelta::Pixels`, not `Lines`) -- the
    /// clearest, most direct unit for an explicit Python call, unlike a
    /// real `winit`-driven event which may arrive as either. Fires
    /// `Tree::dispatch`'s own real scroll-bubbling (walks up from
    /// whatever's hit to the nearest `NodeKind::VirtualList` ancestor)
    /// -- `node` itself doesn't need to be the list; any of its real
    /// children work too, matching real scroll-wheel behavior.
    fn scroll(&self, node: PyRef<'_, Node>, delta_y: f64, py: Python<'_>) {
        let point = node_center(
            &self.tree,
            self.root,
            Size {
                width: AvailableSpace::Definite(self.width as f32),
                height: AvailableSpace::Definite(self.height as f32),
            },
            node.id,
        );

        let outcome = self.tree.borrow_mut().dispatch(
            self.root,
            InputEvent::Scroll {
                delta: engine_core::ScrollDelta::Pixels(0.0, delta_y),
                position: point,
            },
            &interaction_config(),
            std::time::Instant::now(),
        );
        run_dispatch_outcome(&self.handlers, outcome, py);
    }

    /// M4 Phase 7 (§11.3): `click()`'s own secondary-button (right-click)
    /// counterpart -- the same no-live-window-needed proof pattern,
    /// dispatching a secondary-button press+release pair at `node`'s own
    /// real center point. If `node` has a registered context menu
    /// (`Node.set_context_menu`), opens it via `Tree::open_overlay`,
    /// exactly what a real right-click there would produce.
    fn right_click(&self, node: PyRef<'_, Node>, py: Python<'_>) {
        let point = node_center(
            &self.tree,
            self.root,
            Size {
                width: AvailableSpace::Definite(self.width as f32),
                height: AvailableSpace::Definite(self.height as f32),
            },
            node.id,
        );

        let now = std::time::Instant::now();
        let config = interaction_config();
        self.tree.borrow_mut().dispatch(
            self.root,
            InputEvent::PointerPressed {
                position: point,
                button: PointerButton::Secondary,
            },
            &config,
            now,
        );
        let outcome = self.tree.borrow_mut().dispatch(
            self.root,
            InputEvent::PointerReleased {
                position: point,
                button: PointerButton::Secondary,
            },
            &config,
            now,
        );
        run_dispatch_outcome(&self.handlers, outcome, py);
        open_context_menu(&self.tree, &self.context_menus, self.root, outcome);
    }

    /// M4 Phase 2 (§10): `click()`'s own keyboard counterpart -- the
    /// real, no-window-needed way to test Tab/Shift-Tab focus movement
    /// and Enter/Space activation from Python, neither of which had a
    /// Python-facing entry point before this. `key` is one of `"tab"`/
    /// `"enter"`/`"space"`/`"escape"`/`"backspace"`/`"delete"`/
    /// `"left"`/`"right"`/`"home"`/`"end"` (the latter six added M15
    /// Phase 2, §8/§10, for real `TextField` editing) plus `"up"`/
    /// `"down"` (M30 Phase 9 Step 3, §8/§10, `Code Editor`'s own real
    /// multiline line-navigation) -- `engine_core::Key`'s own
    /// deliberately minimal vocabulary, not a general key-code mapping
    /// nothing here needs yet.
    #[pyo3(signature = (key, shift=false))]
    fn press_key(&self, key: &str, shift: bool, py: Python<'_>) -> PyResult<()> {
        let key = match key {
            "tab" => Key::Tab,
            "enter" => Key::Enter,
            "space" => Key::Space,
            "escape" => Key::Escape,
            "backspace" => Key::Backspace,
            "delete" => Key::Delete,
            "left" => Key::ArrowLeft,
            "right" => Key::ArrowRight,
            "up" => Key::ArrowUp,
            "down" => Key::ArrowDown,
            "home" => Key::Home,
            "end" => Key::End,
            other => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "press_key: unknown key {other:?} -- expected one of \"tab\", \"enter\", \
                     \"space\", \"escape\", \"backspace\", \"delete\", \"left\", \"right\", \
                     \"up\", \"down\", \"home\", \"end\""
                )));
            }
        };
        let outcome = self.tree.borrow_mut().dispatch(
            self.root,
            InputEvent::KeyPressed { key, shift },
            &interaction_config(),
            std::time::Instant::now(),
        );
        run_dispatch_outcome(&self.handlers, outcome, py);
        Ok(())
    }

    /// M15 Phase 2 (§8, §10): `press_key`'s own real counterpart for a
    /// produced *character* keypress -- the same no-live-window-needed
    /// synthetic-dispatch pattern, this time for `InputEvent::TextInput
    /// (String)`, mirroring exactly what a real `winit::event::KeyEvent
    /// .text` would produce for an ordinary printable-character
    /// keypress. Only meaningful when a `TextField` is the window's own
    /// currently focused node (a true no-op otherwise, `Tree::dispatch`
    /// 's own real behavior).
    fn type_text(&self, text: &str, py: Python<'_>) {
        let outcome = self.tree.borrow_mut().dispatch(
            self.root,
            InputEvent::TextInput(text.to_string()),
            &interaction_config(),
            std::time::Instant::now(),
        );
        run_dispatch_outcome(&self.handlers, outcome, py);
    }

    /// M17 Phase 1 (§8): the real, no-live-window-needed synthetic
    /// entry point for "what a Ctrl+C press would copy" -- deliberately
    /// **hermetic**, unlike the real `winit`-driven path (`engine-
    /// platform::translate_clipboard_shortcut` + `App::run`'s own
    /// `on_input` handling of `InputEvent::Copy`, both real and tested
    /// on their own terms): it never touches the actual OS clipboard,
    /// only the real, pure `Tree::text_field_selected_text` read. This
    /// is a genuine, stated scope boundary, not an oversight -- unlike
    /// `press_key`/`type_text`, which dispatch through `Tree::dispatch`
    /// the exact same way a real `winit` event would, a *real* Ctrl+C
    /// only ever originates from an actual OS-level keyboard event
    /// reaching `engine-platform` directly; there is no synthetic way
    /// to drive that path from Python without a live window, the same
    /// real category of gap this codebase's own "no live AT-SPI client"
    /// note already states honestly elsewhere.
    fn copy(&self) -> Option<String> {
        let field = self.tree.borrow().focused()?;
        self.tree.borrow().text_field_selected_text(field)
    }

    /// `copy`'s own real Cut sibling -- same real scope boundary
    /// (hermetic, no real OS clipboard touched), reusing the real,
    /// pure `Tree::cut_text_field_selection`.
    fn cut(&self, py: Python<'_>) -> Option<String> {
        let field = self.tree.borrow().focused()?;
        let text = self.tree.borrow_mut().cut_text_field_selection(field)?;
        // A real cut genuinely edits the field's own content -- fires
        // `Change` the same way `Node.set_checked`/`set_text` already
        // do for a direct, non-`Tree::dispatch` mutation (`cut_text_
        // field_selection` is called straight on `Tree`, not through
        // `dispatch`, so no `DispatchOutcome::Changed` exists here to
        // carry this automatically the way Backspace/Delete/typing get
        // it for free).
        call_handler(&self.handlers, field, EventKind::Change, py);
        Some(text)
    }

    /// `type_text`'s own real Paste-shaped sibling -- takes an explicit
    /// `text` rather than reading the real OS clipboard (the same real
    /// scope boundary `copy`/`cut` state above), so this stays
    /// deterministic and hermetic: exactly what a real Ctrl+V would do
    /// *after* the real clipboard read already happened, reusing the
    /// identical `InputEvent::TextInput` mechanism `type_text` already
    /// uses -- a real paste is genuinely nothing more than "insert this
    /// text," the same real finding `PLAN.md` already states.
    fn paste(&self, text: &str, py: Python<'_>) {
        self.type_text(text, py);
    }
}
