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
                width: AvailableSpace::Definite(self.width.get() as f32),
                height: AvailableSpace::Definite(self.height.get() as f32),
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
            crate::clock::now(&self.tree),
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
        // M42 Phase 2 (§4, §5, §8, §16.2, §16.4): reads through `self.
        // active` rather than `self.tree`/`self.root`/`self.handlers`
        // directly -- after a real `show_view` switch, those plain
        // fields still name this `Window`'s *original* tree/root/
        // handlers (from construction), not whatever it currently
        // shows. `active` is what `App::run`'s own live render loop
        // actually dispatches against (`app.rs`'s per-frame/per-input
        // re-sync), so this synthetic entry point must read the same
        // source to stay correct/testable after a switch.
        //
        // **Real bug caught before shipping, not found later:** an
        // earlier draft of this method held `self.active.borrow()`
        // (a live `Ref`) across the `run_dispatch_outcome` calls below
        // -- a real click handler that itself calls `Window.show_view`
        // (a genuine, expected pattern: a nav button switching screens)
        // would then hit `self.active.borrow_mut()` while this same
        // `Ref` was still alive, panicking with "already borrowed."
        // Cloning the three `Rc`s out and dropping the borrow
        // immediately, before any dispatch/callback runs, is what
        // actually avoids that -- the identical pattern `app.rs`'s own
        // `frame`/`input` closures already use for the same real reason.
        let (tree, root, handlers, context_menus) = {
            let active = self.active.borrow();
            (
                active.tree.clone(),
                active.root,
                active.handlers.clone(),
                active.context_menus.clone(),
            )
        };
        let point = node_center(
            &tree,
            root,
            Size {
                width: AvailableSpace::Definite(self.width.get() as f32),
                height: AvailableSpace::Definite(self.height.get() as f32),
            },
            node.id,
        );

        let now = crate::clock::now(&tree);
        let config = interaction_config();
        // Each `dispatch` call's own `tree.borrow_mut()` is a short-
        // lived temporary, released before `run_dispatch_outcome` runs
        // -- a click handler that itself touches this same `Tree` (e.g.
        // animating the very node it's attached to, a real, plausible
        // pattern) would otherwise panic on a re-entrant borrow.
        let press_event = InputEvent::PointerPressed {
            position: point,
            button: PointerButton::Primary,
        };
        let press = tree
            .borrow_mut()
            .dispatch(root, press_event.clone(), &config, now);
        run_dispatch_outcome(
            &handlers,
            &tree,
            &context_menus,
            &self.theme,
            &self.completions,
            &press,
            Some(&press_event),
            py,
        );

        let release_event = InputEvent::PointerReleased {
            position: point,
            button: PointerButton::Primary,
        };
        let release = tree
            .borrow_mut()
            .dispatch(root, release_event.clone(), &config, now);
        run_dispatch_outcome(
            &handlers,
            &tree,
            &context_menus,
            &self.theme,
            &self.completions,
            &release,
            Some(&release_event),
            py,
        );
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
        // M42 Phase 2: see `click()`'s own identical real reasoning --
        // clone-then-drop, never a live borrow held across a dispatch
        // call that can call back into a real Python handler.
        let (tree, root, handlers, context_menus) = {
            let active = self.active.borrow();
            (
                active.tree.clone(),
                active.root,
                active.handlers.clone(),
                active.context_menus.clone(),
            )
        };
        let point = node_center(
            &tree,
            root,
            Size {
                width: AvailableSpace::Definite(self.width.get() as f32),
                height: AvailableSpace::Definite(self.height.get() as f32),
            },
            node.id,
        );

        let event = InputEvent::PointerMoved { position: point };
        let outcome = tree.borrow_mut().dispatch(
            root,
            event.clone(),
            &interaction_config(),
            crate::clock::now(&tree),
        );
        run_dispatch_outcome(
            &handlers,
            &tree,
            &context_menus,
            &self.theme,
            &self.completions,
            &outcome,
            Some(&event),
            py,
        );
    }

    /// M55 (§10, §16.2): `click`/`hover`'s own real `Focus` sibling --
    /// the same no-live-window-needed proof pattern, and the real way
    /// to test `FocusEnter`/`FocusExit` without relying on Tab-order or
    /// click-to-focus side effects. Unlike `click`/`hover`, there's no
    /// real `InputEvent` for "focus this specific node" (the same
    /// reason AccessKit's own `Action::Focus` handling calls `Tree::
    /// set_focus_to` directly too, `app.rs`) -- calls it directly here
    /// and fires the transition via the shared `dispatch::fire_focus_
    /// transition`, the identical real mechanism `run_dispatch_outcome`
    /// 's own `FocusChanged` arm uses.
    fn focus(&self, node: PyRef<'_, Node>, py: Python<'_>) {
        let (tree, handlers, context_menus) = {
            let active = self.active.borrow();
            (
                active.tree.clone(),
                active.handlers.clone(),
                active.context_menus.clone(),
            )
        };
        let config = interaction_config();
        let transition = tree.borrow_mut().set_focus_to(
            node.id,
            config.focus_ring_opacity,
            config.focus_ring_duration,
            crate::clock::now(&tree),
        );
        if let Some((old, new)) = transition {
            crate::dispatch::fire_focus_transition(
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

    /// M32 Phase 2 (§4, §5): a direct, programmatic "resize this
    /// window" entry point -- the same no-live-window-needed proof
    /// pattern `click`/`hover` already establish (a real resize has
    /// nowhere else to originate outside a live window either).
    /// Updates both `self.width`/`self.height` -- which every other
    /// synthetic method here (`click`/`hover`/`scroll`, plus every
    /// interactive `add_*` factory method in `window_factory.rs`) reads
    /// for its own `compute_layout` call -- and dispatches the real
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
        let (tree, root, handlers, context_menus) = {
            let active = self.active.borrow();
            (
                active.tree.clone(),
                active.root,
                active.handlers.clone(),
                active.context_menus.clone(),
            )
        };
        let outcome = tree.borrow_mut().dispatch(
            root,
            InputEvent::Resized {
                width: width as f32,
                height: height as f32,
            },
            &interaction_config(),
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
            &context_menus,
            &self.theme,
            &self.completions,
            &outcome,
            None,
            py,
        );
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
    ///
    /// M32 Phase 5 (§4, §8): if `node` is itself a real `Terminal`,
    /// this moves its own real viewport into scrollback instead --
    /// mirrors `app.rs`'s own real winit `MouseWheel` handling, but
    /// simpler: `node` names the target explicitly, no hit-test needed.
    ///
    /// M36 Phase 1 (§5, §7, §11.7): `delta_x`, real pixels, real,
    /// additive widening for a real horizontal `ScrollView` -- ignored
    /// by the `VirtualList`/`Carousel`/`Terminal` branches above,
    /// which stay vertical-only (a real, stated, pre-existing scope,
    /// not touched by this phase).
    #[pyo3(signature = (node, delta_y, delta_x=0.0))]
    fn scroll(&self, node: PyRef<'_, Node>, delta_y: f64, delta_x: f64, py: Python<'_>) {
        // M42 Phase 2: see `click()`'s own identical real reasoning --
        // clone-then-drop, never a live borrow held across a dispatch
        // call that can call back into a real Python handler.
        let (tree, root, handlers, context_menus) = {
            let active = self.active.borrow();
            (
                active.tree.clone(),
                active.root,
                active.handlers.clone(),
                active.context_menus.clone(),
            )
        };
        let is_terminal = matches!(
            tree.borrow().get(node.id).map(|n| &n.kind),
            Some(engine_core::NodeKind::Terminal(_))
        );
        if is_terminal {
            if let Some(session) = self.terminals.borrow_mut().get_mut(&node.id) {
                session.scroll_by(&mut tree.borrow_mut(), node.id, (delta_y / 20.0) as i64);
            }
            return;
        }

        let point = node_center(
            &tree,
            root,
            Size {
                width: AvailableSpace::Definite(self.width.get() as f32),
                height: AvailableSpace::Definite(self.height.get() as f32),
            },
            node.id,
        );

        let outcome = tree.borrow_mut().dispatch(
            root,
            InputEvent::Scroll {
                delta: engine_core::ScrollDelta::Pixels(delta_x, delta_y),
                position: point,
            },
            &interaction_config(),
            crate::clock::now(&tree),
        );
        // `Scroll` is a true no-op for `Tree::dispatch`'s own outcome
        // today (`InputEvent::Scroll`'s own doc comment) -- same
        // reasoning as `resize`'s own `Resized` handling just above.
        run_dispatch_outcome(
            &handlers,
            &tree,
            &context_menus,
            &self.theme,
            &self.completions,
            &outcome,
            None,
            py,
        );
    }

    /// M4 Phase 7 (§11.3): `click()`'s own secondary-button (right-click)
    /// counterpart -- the same no-live-window-needed proof pattern,
    /// dispatching a secondary-button press+release pair at `node`'s own
    /// real center point. If `node` has a registered context menu
    /// (`Node.set_context_menu`), opens it via `Tree::open_overlay`,
    /// exactly what a real right-click there would produce.
    fn right_click(&self, node: PyRef<'_, Node>, py: Python<'_>) {
        // M42 Phase 2: see `click()`'s own identical real reasoning --
        // clone-then-drop, never a live borrow held across a dispatch
        // call that can call back into a real Python handler.
        let (tree, root, handlers, context_menus) = {
            let active = self.active.borrow();
            (
                active.tree.clone(),
                active.root,
                active.handlers.clone(),
                active.context_menus.clone(),
            )
        };
        let point = node_center(
            &tree,
            root,
            Size {
                width: AvailableSpace::Definite(self.width.get() as f32),
                height: AvailableSpace::Definite(self.height.get() as f32),
            },
            node.id,
        );

        let now = crate::clock::now(&tree);
        let config = interaction_config();
        // M55 (§10, §16.2): a real gap found while scoping `Focus`
        // events -- this press's own outcome used to be discarded
        // with no variable at all, so a real right-click-to-focus
        // (M53) on `TextField`/`Terminal` was structurally
        // unobservable from this entry point. Captured and forwarded
        // now, the same way every other real dispatch call site
        // already does.
        let press_event = InputEvent::PointerPressed {
            position: point,
            button: PointerButton::Secondary,
        };
        let press = tree
            .borrow_mut()
            .dispatch(root, press_event.clone(), &config, now);
        run_dispatch_outcome(
            &handlers,
            &tree,
            &context_menus,
            &self.theme,
            &self.completions,
            &press,
            Some(&press_event),
            py,
        );
        let release_event = InputEvent::PointerReleased {
            position: point,
            button: PointerButton::Secondary,
        };
        let outcome = tree
            .borrow_mut()
            .dispatch(root, release_event.clone(), &config, now);
        run_dispatch_outcome(
            &handlers,
            &tree,
            &context_menus,
            &self.theme,
            &self.completions,
            &outcome,
            Some(&release_event),
            py,
        );
        open_context_menu(&tree, &context_menus, root, &outcome);
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
        let event = InputEvent::KeyPressed { key, shift };
        // M30 Phase 9 Step 4 (§5, §8, §10): the identical real terminal-
        // keyboard-routing check `app.rs`'s own real `on_input` closure
        // already applies to a genuine `winit` keypress -- see its own
        // doc comment (`input_bytes_for`, `terminal.rs`) for the full
        // real reasoning. `press_key`/`type_text` exist specifically to
        // mirror what a real platform event would do without a live
        // window, so a focused `Terminal` must behave identically here
        // too, not just when a real window is open.
        if route_to_terminal(self, &event) {
            return Ok(());
        }
        // M57 (§8): reads through `self.active`, not `self.tree`/
        // `self.root`/`self.handlers`/`self.context_menus` directly --
        // the identical real staleness fix `click`/`hover`/`scroll`/
        // `focus` already established, so a real key press reaches
        // whatever View is currently shown after a `show_view` switch.
        let (tree, root, handlers, context_menus) = {
            let active = self.active.borrow();
            (
                active.tree.clone(),
                active.root,
                active.handlers.clone(),
                active.context_menus.clone(),
            )
        };
        let outcome = tree.borrow_mut().dispatch(
            root,
            event.clone(),
            &interaction_config(),
            crate::clock::now(&tree),
        );
        run_dispatch_outcome(
            &handlers,
            &tree,
            &context_menus,
            &self.theme,
            &self.completions,
            &outcome,
            Some(&event),
            py,
        );
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
        let event = InputEvent::TextInput(text.to_string());
        // M30 Phase 9 Step 4 (§5, §8, §10): see `press_key`'s own
        // identical real terminal-routing comment just above.
        if route_to_terminal(self, &event) {
            return;
        }
        // M57 (§8): see `press_key`'s own identical `active`-routing
        // fix just above -- `paste()` forwards to this method, so it
        // gets the same fix for free, with no separate change needed.
        let (tree, root, handlers, context_menus) = {
            let active = self.active.borrow();
            (
                active.tree.clone(),
                active.root,
                active.handlers.clone(),
                active.context_menus.clone(),
            )
        };
        let outcome = tree.borrow_mut().dispatch(
            root,
            event.clone(),
            &interaction_config(),
            crate::clock::now(&tree),
        );
        run_dispatch_outcome(
            &handlers,
            &tree,
            &context_menus,
            &self.theme,
            &self.completions,
            &outcome,
            Some(&event),
            py,
        );
    }

    /// M32 Phase 4 (§4, §8): the real, no-live-window-needed synthetic
    /// entry point for a real Ctrl+`<letter>` press reaching a focused
    /// `Terminal` -- SIGINT (`"c"`) included, the headline real
    /// capability this phase exists for. Mirrors `press_key`/`type_
    /// text`'s own real terminal-routing precedent above, but scoped
    /// deliberately narrower: unlike those two (which fall through to
    /// ordinary `Tree::dispatch` when nothing terminal-shaped is
    /// focused), this method only ever does terminal routing -- when no
    /// `Terminal` is focused, it returns `False` and touches nothing
    /// else (never a `TextField`'s own clipboard state; `copy`/`cut`/
    /// `paste` below already own that real, separate, hermetic surface).
    /// `letter` must be exactly one ASCII letter (`a`-`z`, case-
    /// insensitive, the same real convention `Copy`/`Cut`/
    /// `PasteRequested` already established) -- raises `ValueError`
    /// otherwise, matching `press_key`'s own "unknown key" contract.
    fn press_ctrl(&self, letter: &str) -> PyResult<bool> {
        let mut chars = letter.chars();
        let (Some(ch), None) = (chars.next(), chars.next()) else {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "press_ctrl: {letter:?} must be exactly one ASCII letter"
            )));
        };
        if !ch.is_ascii_alphabetic() {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "press_ctrl: {letter:?} must be exactly one ASCII letter"
            )));
        }
        let event = match ch.to_ascii_lowercase() {
            'c' => InputEvent::Copy,
            'x' => InputEvent::Cut,
            'v' => InputEvent::PasteRequested,
            other => InputEvent::ControlChar(other),
        };
        Ok(route_control_char_to_terminal(self, &event))
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
        // M57 (§8): reads through `self.active`, not `self.tree`
        // directly -- the identical real staleness fix `click`/`hover`/
        // etc. already established, so a real `TextField` selection
        // survives a `show_view` switch, matching every other real
        // "act on the currently focused node" method.
        let tree = self.active.borrow().tree.clone();
        let field = tree.borrow().focused()?;
        tree.borrow().text_field_selected_text(field)
    }

    /// M32 Phase 6 (§4, §5, §8): `copy`'s own real `Terminal` sibling
    /// -- the identical real hermetic scope boundary (never touches
    /// the actual OS clipboard, only the real, pure `Tree::terminal_
    /// selected_text` read); a real Ctrl+Shift+C only ever originates
    /// from an actual OS-level keyboard event, the same real "no
    /// synthetic way to drive that specific path" gap `copy`'s own doc
    /// comment already states for Ctrl+C. Reads whichever `Terminal`
    /// is currently focused -- use `Node.set_terminal_selection` first
    /// to seed a real selection without a live mouse drag.
    fn copy_terminal_selection(&self) -> Option<String> {
        // M57 (§8): reads through `self.active`, the identical real
        // staleness fix `copy`'s own sibling above just got.
        let tree = self.active.borrow().tree.clone();
        let terminal = tree.borrow().focused()?;
        tree.borrow().terminal_selected_text(terminal)
    }

    /// `copy`'s own real Cut sibling -- same real scope boundary
    /// (hermetic, no real OS clipboard touched), reusing the real,
    /// pure `Tree::cut_text_field_selection`.
    fn cut(&self, py: Python<'_>) -> Option<String> {
        // M57 (§8): reads through `self.active`, not `self.tree`/
        // `self.handlers`/`self.context_menus` directly -- the
        // identical real staleness fix every other "act on the
        // currently focused node" method already got, so a real cut
        // (and the `Change` handler it fires) targets whatever View is
        // currently shown after a `show_view` switch. `theme`/
        // `completions` stay plain `self.*` reads -- deliberately
        // outside the swapped `active` bundle, per `ActiveTree`'s own
        // doc comment.
        let (tree, handlers, context_menus) = {
            let active = self.active.borrow();
            (
                active.tree.clone(),
                active.handlers.clone(),
                active.context_menus.clone(),
            )
        };
        let field = tree.borrow().focused()?;
        // M54 Phase 2: the field's own real pre-cut content, read
        // before `cut_text_field_selection` mutates it -- the one real
        // place it's still whole, the same "snapshot before mutate"
        // discipline `engine-core`'s own real `Changed` producers use.
        let old = crate::dispatch::read_new_changed_value(&tree.borrow(), field, py);
        let text = tree.borrow_mut().cut_text_field_selection(field)?;
        // A real cut genuinely edits the field's own content -- fires
        // `Change` the same way `Node.set_checked`/`set_text` already
        // do for a direct, non-`Tree::dispatch` mutation (`cut_text_
        // field_selection` is called straight on `Tree`, not through
        // `dispatch`, so no `DispatchOutcome::Changed` exists here to
        // carry this automatically the way Backspace/Delete/typing get
        // it for free).
        let ctx = crate::event::NodeContext {
            tree: &tree,
            handlers: &handlers,
            context_menus: &context_menus,
            theme: &self.theme,
            completions: &self.completions,
        };
        call_handler(&handlers, field, EventKind::Change, py, |py| {
            let old = old?;
            let new = crate::dispatch::read_new_changed_value(&tree.borrow(), field, py)?;
            crate::event::Event::change(py, field, &ctx, old, new)
        });
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

    /// M53 Phase 2 (§8, §10, §11.3): `copy`'s own **real**, non-hermetic
    /// sibling -- unlike `copy` above (which never touches the actual
    /// OS clipboard, by design, since M17 Phase 1), this genuinely
    /// writes the currently-focused `TextField`'s own real selection to
    /// the real system clipboard, via the exact same shared logic
    /// `App::run`'s own winit-driven Ctrl+C path uses (`crate::dispatch
    /// ::copy_focused_selection_to_clipboard`, factored out of `app.rs`
    /// this same milestone). This is the real gap this milestone was
    /// scoped to close: a context-menu "Copy" item's own `on_click`
    /// callback previously had nothing real to call. Returns `true`
    /// only on a genuine, complete write -- `False` both when nothing
    /// is focused/selected and when the real OS clipboard is
    /// unreachable (logged via `tracing::warn!`, never a panic, the
    /// same "real, expected, gracefully-handled" policy this crate
    /// already established for no-GPU/no-display).
    fn copy_to_system_clipboard(&self) -> bool {
        let tree = self.active.borrow().tree.clone();
        crate::dispatch::copy_focused_selection_to_clipboard(&tree)
    }

    /// `copy_to_system_clipboard`'s own real Cut sibling -- genuinely
    /// removes the currently-focused field's own selection and fires a
    /// real `Change` handler, but only once the real clipboard write
    /// actually succeeds (a failed write must never silently destroy
    /// the user's own selected text with no way to recover it). Shares
    /// `crate::dispatch::cut_focused_selection_to_clipboard` with
    /// `App::run`'s own real winit-driven Ctrl+X path.
    fn cut_to_system_clipboard(&self, py: Python<'_>) -> bool {
        let (tree, handlers, context_menus) = {
            let active = self.active.borrow();
            (
                active.tree.clone(),
                active.handlers.clone(),
                active.context_menus.clone(),
            )
        };
        crate::dispatch::cut_focused_selection_to_clipboard(
            &tree,
            &handlers,
            &context_menus,
            &self.theme,
            &self.completions,
            py,
        )
    }

    /// `copy_to_system_clipboard`'s own real Paste sibling -- reads the
    /// real system clipboard and inserts it into whichever field is
    /// currently focused, exactly like a real Ctrl+V (`Tree::dispatch`
    /// already resolves the focused target internally for `TextInput`,
    /// so this never needs its own focused-field check). Shares `crate
    /// ::dispatch::paste_clipboard_into_focused` with `App::run`'s own
    /// real winit-driven path. Returns whether the real clipboard
    /// *read* succeeded, not whether a field happened to be focused to
    /// receive it -- a real OS read can genuinely fail on its own,
    /// independent of anything this `Window`'s own tree state.
    fn paste_from_system_clipboard(&self, py: Python<'_>) -> bool {
        let (tree, root, handlers, context_menus) = {
            let active = self.active.borrow();
            (
                active.tree.clone(),
                active.root,
                active.handlers.clone(),
                active.context_menus.clone(),
            )
        };
        crate::dispatch::paste_clipboard_into_focused(
            &tree,
            root,
            &handlers,
            &context_menus,
            &self.theme,
            &self.completions,
            py,
        )
    }

    /// M53 Phase 2 (§8, §10, §11.3): a real "Select All" -- selects the
    /// currently-focused `TextField`'s own entire content, matching
    /// every real desktop text field's own Ctrl+A convention (cursor
    /// lands at the end, not the start). Thin wrapper over `Tree::
    /// select_all_text_field` (Phase 1), the same "acts on whatever's
    /// currently focused" convention `copy`/`cut`/`paste`/`copy_to_
    /// system_clipboard`/etc. all already share. Returns whether a real
    /// `TextField` was actually focused -- a true no-op otherwise (no
    /// field focused, or the focused node isn't a `TextField`).
    fn select_all(&self) -> bool {
        // M57 (§8): reads through `self.active`, the identical real
        // staleness fix `copy`/`copy_terminal_selection` above just got.
        let tree = self.active.borrow().tree.clone();
        let Some(field) = tree.borrow().focused() else {
            return false;
        };
        tree.borrow_mut().select_all_text_field(field)
    }
}

/// M30 Phase 9 Step 4 (§5, §8, §10): `press_key`/`type_text`'s own
/// shared real terminal-keyboard check -- if `window`'s own currently
/// focused node is a real `Terminal`, `event` is translated
/// (`terminal::input_bytes_for`) and written straight to its own PTY,
/// returning `true` (the caller's own "already handled, don't also
/// call `Tree::dispatch`" signal). Mirrors `app.rs`'s own real
/// `on_input` closure exactly, so a synthetic keystroke behaves
/// identically to a genuine platform one.
fn route_to_terminal(window: &PyWindow, event: &InputEvent) -> bool {
    let Some(bytes) = crate::terminal::input_bytes_for(event) else {
        return false;
    };
    // M57 (§8): reads through `window.active`, not `window.tree`
    // directly -- a focused `Terminal` in a `show_view`-switched View
    // must be reachable from here too, the identical real staleness
    // fix every other "act on the currently focused node" method
    // already got. `window.terminals` stays a plain window-level read
    // (terminal PTY sessions are owned by the `Window` itself, never
    // swapped by `show_view`, the same real reason `theme`/
    // `completions` also stay outside the `active` bundle).
    let focused_terminal = {
        let active = window.active.borrow();
        let tree = active.tree.borrow();
        tree.focused().filter(|&id| {
            matches!(
                tree.get(id).map(|node| &node.kind),
                Some(engine_core::NodeKind::Terminal(_))
            )
        })
    };
    let Some(terminal_id) = focused_terminal else {
        return false;
    };
    if let Some(session) = window.terminals.borrow_mut().get_mut(&terminal_id) {
        session.write_input(&bytes);
    }
    true
}

/// M32 Phase 4 (§4, §8): `route_to_terminal`'s own real Ctrl+`<letter>`
/// sibling -- the identical real "translate, then write straight to a
/// focused terminal's own PTY" shape, using `terminal::control_byte_for`
/// instead of `input_bytes_for`. Mirrors `app.rs`'s own real `on_input`
/// closure's new `control_byte_for` handling exactly, so `Window.
/// press_ctrl` behaves identically to a genuine Ctrl+`<letter>` keypress.
fn route_control_char_to_terminal(window: &PyWindow, event: &InputEvent) -> bool {
    let Some(byte) = crate::terminal::control_byte_for(event) else {
        return false;
    };
    // M57 (§8): the identical real `active`-routing fix `route_to_
    // terminal`'s own sibling just above got.
    let focused_terminal = {
        let active = window.active.borrow();
        let tree = active.tree.borrow();
        tree.focused().filter(|&id| {
            matches!(
                tree.get(id).map(|node| &node.kind),
                Some(engine_core::NodeKind::Terminal(_))
            )
        })
    };
    let Some(terminal_id) = focused_terminal else {
        return false;
    };
    if let Some(session) = window.terminals.borrow_mut().get_mut(&terminal_id) {
        session.write_input(&[byte]);
    }
    true
}
