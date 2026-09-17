//! `PyWindow` (§8's own sketch, split back out of `App` at exactly the
//! step `App`'s own module doc comment predicted -- §14 step 14, §11.1
//! multi-window). Owns one `Tree`, its root, and its own size/title --
//! everything `App::new`/`App::add_rect` used to hold directly, now per
//! window instead of assumed singular.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use engine_core::{
    Animated, InputEvent, ItemExtent, Key, NodeId, NodeKind, PaintProperties, PointerButton,
    SplitterState, Tree, VirtualListState,
};
use engine_md3::DynamicTheme;
use peniko::Color;
use peniko::kurbo::Point;
use pyo3::class::{PyTraverseError, PyVisit};
use pyo3::prelude::*;
use taffy::prelude::{AvailableSpace, Position, Rect as TaffyRect, Size, Style, auto, length};

use crate::dispatch::{
    CompletionRegistry, HandlerMap, SharedCompletions, interaction_config, open_context_menu,
    run_dispatch_outcome,
};
use crate::dock::{self, SharedDockState};
use crate::error::EngineError;
use crate::node::Node;

const PADDING: f32 = 16.0;
const GAP: f32 = 16.0;

/// M7 Phase 3 (§7.1): a `Window`'s own theme, shared -- the same
/// `Rc<RefCell<...>>`-clone-into-every-`Node`-it-hands-out shape
/// `handlers`/`context_menus` already use. Holds the *full* `Dynamic
/// Theme` (both `light`/`dark` schemes), not just the currently active
/// color, so a real live theme switch (`InputEvent::ThemeChanged`, only
/// ever carrying a bare `dark: bool`) can re-resolve without needing the
/// original seed color again.
///
/// `theme: None` (the default, before `Window.set_theme` is ever
/// called) makes `on_surface()` return real black -- byte-for-byte
/// `InteractionState::new()`'s own hardcoded default, so a `Window`
/// that never sets a theme sees zero behavior change.
#[derive(Default)]
pub(crate) struct ThemeState {
    theme: Option<DynamicTheme>,
    dark: bool,
}

impl ThemeState {
    /// M7 Phase 3 (§7.1, Step 3): the real live-switch mutator -- called
    /// from `engine-py::App::run`'s own `on_input` closure on a real
    /// `InputEvent::ThemeChanged`, the one call site outside this module
    /// that ever needs to flip which scheme is active.
    pub(crate) fn set_dark(&mut self, dark: bool) {
        self.dark = dark;
    }

    /// MD3's real "on-surface" scheme role -- the only role this phase
    /// needs (ripple/hover's own tint, §7.3), resolved directly as a
    /// struct field rather than through `ColorScheme::role("on_surface")`
    /// -- no string lookup needed when the field name is already known
    /// at compile time.
    pub(crate) fn on_surface(&self) -> Color {
        match &self.theme {
            Some(theme) => {
                if self.dark {
                    theme.dark.on_surface
                } else {
                    theme.light.on_surface
                }
            }
            None => Color::from_rgba8(0, 0, 0, 255),
        }
    }
}

/// Shared the same way `HandlerMap`/`context_menus` are -- a `View`'s
/// own construction sites get a fresh, private, never-`Window`-linked
/// instance instead (see `view.rs`), matching this phase's own stated
/// scope: only `Window`-created nodes ever see a real theme.
pub(crate) type SharedTheme = Rc<RefCell<ThemeState>>;

/// M6 Phase 3 (§8): the real `Position::Absolute` + `taffy::Rect` inset
/// shape every Rust-level pixel test already uses internally
/// (`overlay_menu.rs`/`transform_composition.rs`/etc.'s own `absolute()`
/// helpers), factored out here since two real Python call sites
/// (`add_rect`/`add_canvas`) now need it. `x`/`y` are independently
/// optional but trigger the same positioning mode together -- if either
/// is given, the node is absolutely positioned with both insets (the
/// other defaulting to `0.0`); if neither is given, `size` alone is
/// returned unchanged (the existing implicit flex-row flow, byte-for-
/// byte backward compatible). The inset lands relative to the window's
/// own root padding-box origin (`PADDING`, `PyWindow::new`), not the
/// raw window corner -- a real, stated detail, not a silent surprise.
fn positioned_style(size: Size<taffy::style::Dimension>, x: Option<f32>, y: Option<f32>) -> Style {
    if x.is_none() && y.is_none() {
        return Style {
            size,
            ..Default::default()
        };
    }
    Style {
        position: Position::Absolute,
        inset: TaffyRect {
            left: length(x.unwrap_or(0.0)),
            top: length(y.unwrap_or(0.0)),
            right: auto(),
            bottom: auto(),
        },
        size,
        ..Default::default()
    }
}

/// `unsendable` (owns `Rc<RefCell<Tree>>`, §9) -- named `Window` to
/// Python, matching `Node`'s own "renamed to match what Python actually
/// sees" precedent (`tre.Window`, not `tre.PyWindow`); kept as the
/// `PyWindow` identifier on the Rust side since that's the name §8/§11.1
/// use throughout `ARCHITECTURE.md`.
///
/// `materializers` is §11.7's own "materialize item N" callback storage
/// and `handlers` (M4 Phase 1 step 3, §11.10; re-keyed by `(NodeId,
/// EventKind)` at M4 Phase 6, §16.2) is `Node.set_on_click`/
/// `set_on_hover_enter`/`set_on_hover_exit`'s -- both real cases of §8's
/// own review note: "storing a long-lived `PyObject` callback... is a
/// new risk class... unless the `#[pyclass]` implements `__traverse__`/
/// `__clear__`," which `node.rs`'s own module doc comment deferred
/// exactly this long, "until something actually stores one." Confirmed
/// directly against pyo3 0.29.2's own real API before implementing
/// (`tests/test_gc.rs`): no `#[pyclass(gc)]` flag exists or is needed in
/// this version -- a `#[pyclass]` simply implementing `__traverse__`/
/// `__clear__` in its `#[pymethods]` is enough to opt into cyclic GC
/// support, see below.
///
/// `handlers` is an `Rc<RefCell<...>>`, not a plain field, because
/// `Node.set_on_click`/etc (in `node.rs`) need to write into the *same*
/// map from a `Node` Python object that holds no back-reference to this
/// `PyWindow` -- shared the exact way `tree: Rc<RefCell<Tree>>` already
/// is between a `Window` and every `Node` it hands out.
#[pyclass(unsendable, name = "Window")]
pub struct PyWindow {
    pub(crate) tree: Rc<RefCell<Tree>>,
    pub(crate) root: NodeId,
    pub(crate) title: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    materializers: HashMap<NodeId, Py<PyAny>>,
    /// M5 Phase 3 (§11.10/§11.11): the "draw callback" storage,
    /// mirroring `materializers`'s own shape exactly -- stored by
    /// `add_canvas`, invoked (exactly once per call) only by the real
    /// entry point `redraw_canvas`, never automatically every frame
    /// (see `PLAN.md`: no consumer has asked for that yet).
    canvas_draws: HashMap<NodeId, Py<PyAny>>,
    pub(crate) handlers: HandlerMap,
    /// M4 Phase 7 (§11.3): `anchor NodeId -> content NodeId`, shared
    /// with every `Node` this `Window` hands out (`Node.
    /// set_context_menu` writes into it) -- see `Node`'s own doc
    /// comment for why this needs no `__traverse__`/`__clear__` entry,
    /// unlike `handlers`.
    pub(crate) context_menus: Rc<RefCell<HashMap<NodeId, NodeId>>>,
    /// M4 Phase 9 (§11.4): real docking state -- `DockLayout` plus
    /// engine-py's own zone-container/drag-handle bookkeeping `Tree`
    /// itself never stores. Plain data, no `Py<PyAny>` involved, the
    /// same reason `context_menus` needs no GC-traversal obligation.
    pub(crate) dock: SharedDockState,
    /// M7 Phase 3 (§7.1): shared with every `Node` this `Window` hands
    /// out, the same way `handlers`/`context_menus`/`dock` already are.
    pub(crate) theme: SharedTheme,
    /// M9 Phase 2 (§5): `Node.animate(..., on_complete=...)`'s own
    /// registry, shared the same way `theme` is. Holds real `Py<PyAny>`
    /// callbacks (like `handlers`, unlike `context_menus`/`dock`/
    /// `theme`) -- needs the same `__traverse__`/`__clear__` obligation
    /// below.
    pub(crate) completions: SharedCompletions,
}

#[pymethods]
impl PyWindow {
    #[new]
    #[pyo3(signature = (width=480, height=200, title="tre v2"))]
    fn new(width: u32, height: u32, title: &str) -> Self {
        let mut tree = Tree::new();
        let root = tree.insert(
            NodeKind::Container,
            Style {
                display: taffy::Display::Flex,
                flex_direction: taffy::FlexDirection::Row,
                padding: TaffyRect {
                    left: length(PADDING),
                    right: length(PADDING),
                    top: length(PADDING),
                    bottom: length(PADDING),
                },
                gap: Size {
                    width: length(GAP),
                    height: length(GAP),
                },
                size: Size {
                    width: length(width as f32),
                    height: length(height as f32),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        Self {
            tree: Rc::new(RefCell::new(tree)),
            root,
            title: title.to_string(),
            width,
            height,
            materializers: HashMap::new(),
            canvas_draws: HashMap::new(),
            handlers: Rc::new(RefCell::new(HashMap::new())),
            context_menus: Rc::new(RefCell::new(HashMap::new())),
            dock: Rc::new(RefCell::new(dock::DockState::new())),
            theme: Rc::new(RefCell::new(ThemeState::default())),
            completions: Rc::new(RefCell::new(CompletionRegistry::new())),
        }
    }

    /// M7 Phase 3 (§7.1, Step 1): builds a real MD3 `DynamicTheme` from
    /// `seed` (via the already-proven `DynamicTheme::from_seed`) and
    /// makes it this `Window`'s active theme -- `dark` picks which of
    /// the theme's own `light`/`dark` schemes is active now (the same
    /// choice a real live OS switch, §7.1 Step 3, later flips at
    /// runtime). Immediately re-resolves and pushes the real "on-
    /// surface" color into every node that already called `enable_
    /// interaction()` before this was ever set (`Tree::
    /// set_all_interaction_tints`) -- a node opting in *after* this call
    /// picks up the same color at opt-in time instead (`Node.
    /// enable_interaction`).
    #[pyo3(signature = (seed, dark=false))]
    fn set_theme(&self, seed: (u8, u8, u8, u8), dark: bool) {
        let (r, g, b, a) = seed;
        let dynamic = DynamicTheme::from_seed(Color::from_rgba8(r, g, b, a));
        let mut state = self.theme.borrow_mut();
        state.theme = Some(dynamic);
        state.dark = dark;
        let tint = state.on_surface();
        drop(state);
        self.tree.borrow_mut().set_all_interaction_tints(tint);
    }

    /// §14 step 6's own "node creation" -- one shape (a colored rect, a
    /// child of this window's implicit root row) is the real minimal
    /// slice; unchanged by the `PyWindow` split, just moved here with
    /// `App` itself.
    #[pyo3(signature = (background, width, height, x=None, y=None))]
    fn add_rect(
        &self,
        background: (u8, u8, u8, u8),
        width: f32,
        height: f32,
        x: Option<f32>,
        y: Option<f32>,
    ) -> Node {
        let (r, g, b, a) = background;
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Rect,
            positioned_style(
                Size {
                    width: length(width),
                    height: length(height),
                },
                x,
                y,
            ),
            PaintProperties::new(Color::from_rgba8(r, g, b, a), 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        Node {
            id,
            tree: self.tree.clone(),
            handlers: self.handlers.clone(),
            context_menus: self.context_menus.clone(),
            theme: self.theme.clone(),
            completions: self.completions.clone(),
        }
    }

    /// M4 Phase 3, step 2 (§11.5): the missing Python-facing half of
    /// step 1's already-real drag mechanism -- until now, nothing created a
    /// `NodeKind::Splitter` from Python at all, so `Tree::dispatch`'s
    /// real drag handling (`Tree::update_drag`/`set_splitter_position`,
    /// reachable from a real mouse the moment such a node exists) had no
    /// way to actually be exercised by a Python app.
    ///
    /// Adds a child of this window's own root row, the same append-only
    /// way `add_rect` does -- called between two `add_rect` calls (left
    /// pane, splitter, right pane, in that order), it produces exactly
    /// the resizable-pane layout §11.5's own architecture text
    /// describes, with no separate "pane container" concept needed: the
    /// window's root row already *is* the flex parent `Tree::
    /// splitter_geometry` expects, holding the splitter directly between
    /// its two real flanking siblings.
    ///
    /// `background` matches `add_rect`'s own parameter shape exactly --
    /// a real splitter typically just wants a background for its own
    /// grip/handle (§11.5's own text: `SplitterState` carries no
    /// separate appearance data). `initial_position` (0.0..=1.0 along
    /// the split axis) defaults to an even 0.5 split.
    #[pyo3(signature = (background, width, height, initial_position=0.5))]
    fn add_splitter(
        &mut self,
        background: (u8, u8, u8, u8),
        width: f32,
        height: f32,
        initial_position: f64,
    ) -> Node {
        let (r, g, b, a) = background;
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Splitter(SplitterState {
                position: Animated::new(initial_position),
            }),
            Style {
                size: Size {
                    width: length(width),
                    height: length(height),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(r, g, b, a), 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        Node {
            id,
            tree: self.tree.clone(),
            handlers: self.handlers.clone(),
            context_menus: self.context_menus.clone(),
            theme: self.theme.clone(),
            completions: self.completions.clone(),
        }
    }

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
        &mut self,
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
    fn end_container_transform(&mut self, trigger: PyRef<'_, Node>) -> PyResult<()> {
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
    fn click(&mut self, node: PyRef<'_, Node>, py: Python<'_>) {
        let point = {
            let mut tree = self.tree.borrow_mut();
            tree.compute_layout(
                self.root,
                Size {
                    width: AvailableSpace::Definite(self.width as f32),
                    height: AvailableSpace::Definite(self.height as f32),
                },
            );
            let (x, y) = tree.absolute_position(node.id);
            let layout = tree.layout(node.id);
            Point::new(
                x + f64::from(layout.size.width) / 2.0,
                y + f64::from(layout.size.height) / 2.0,
            )
        };

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
    fn hover(&mut self, node: PyRef<'_, Node>, py: Python<'_>) {
        let point = {
            let mut tree = self.tree.borrow_mut();
            tree.compute_layout(
                self.root,
                Size {
                    width: AvailableSpace::Definite(self.width as f32),
                    height: AvailableSpace::Definite(self.height as f32),
                },
            );
            let (x, y) = tree.absolute_position(node.id);
            let layout = tree.layout(node.id);
            Point::new(
                x + f64::from(layout.size.width) / 2.0,
                y + f64::from(layout.size.height) / 2.0,
            )
        };

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
    fn scroll(&mut self, node: PyRef<'_, Node>, delta_y: f64, py: Python<'_>) {
        let point = {
            let mut tree = self.tree.borrow_mut();
            tree.compute_layout(
                self.root,
                Size {
                    width: AvailableSpace::Definite(self.width as f32),
                    height: AvailableSpace::Definite(self.height as f32),
                },
            );
            let (x, y) = tree.absolute_position(node.id);
            let layout = tree.layout(node.id);
            Point::new(
                x + f64::from(layout.size.width) / 2.0,
                y + f64::from(layout.size.height) / 2.0,
            )
        };

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
    fn right_click(&mut self, node: PyRef<'_, Node>, py: Python<'_>) {
        let point = {
            let mut tree = self.tree.borrow_mut();
            tree.compute_layout(
                self.root,
                Size {
                    width: AvailableSpace::Definite(self.width as f32),
                    height: AvailableSpace::Definite(self.height as f32),
                },
            );
            let (x, y) = tree.absolute_position(node.id);
            let layout = tree.layout(node.id);
            Point::new(
                x + f64::from(layout.size.width) / 2.0,
                y + f64::from(layout.size.height) / 2.0,
            )
        };

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
    /// `"enter"`/`"space"`/`"escape"` -- `engine_core::Key`'s own
    /// deliberately minimal vocabulary (§10), not a general key-code
    /// mapping nothing here needs yet.
    #[pyo3(signature = (key, shift=false))]
    fn press_key(&mut self, key: &str, shift: bool, py: Python<'_>) -> PyResult<()> {
        let key = match key {
            "tab" => Key::Tab,
            "enter" => Key::Enter,
            "space" => Key::Space,
            "escape" => Key::Escape,
            other => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "press_key: unknown key {other:?} -- expected one of \"tab\", \"enter\", \
                     \"space\", \"escape\""
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

    /// M4 Phase 9 (§11.4): registers `container` as `side`'s real dock
    /// zone -- a plain node the app already built (e.g. via `add_rect`),
    /// exactly like `docking.rs`'s own Rust-level proof (M3 step 15
    /// Stage B) builds one by hand. `size` seeds the zone's own
    /// `Animated<f64>` extent (§11.4's own struct sketch).
    fn add_dock_zone(&mut self, side: &str, container: PyRef<'_, Node>, size: f64) -> PyResult<()> {
        let side = dock::parse_dock_side(side)?;
        dock::add_dock_zone(&self.dock, side, container.id, size);
        Ok(())
    }

    /// Real initial "put a panel in this zone" setup -- attaches
    /// `panel` as `side`'s new active tab via the existing real
    /// `Tree::apply_active_tab` (M3 step 15 Stage B), not a second
    /// resize/attach mechanism.
    fn dock_panel(&mut self, side: &str, panel: PyRef<'_, Node>) -> PyResult<()> {
        let side = dock::parse_dock_side(side)?;
        dock::dock_panel(&self.dock, &self.tree, side, panel.id)
    }

    /// Switches `side`'s own active tab by index -- the same plain
    /// index switch §11.4's own text describes, via `Tree::
    /// apply_active_tab`.
    fn set_active_tab(&mut self, side: &str, index: usize) -> PyResult<()> {
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
    fn set_dock_handle(&mut self, handle: PyRef<'_, Node>, panel: PyRef<'_, Node>) -> PyResult<()> {
        if !Rc::ptr_eq(&self.tree, &handle.tree) || !Rc::ptr_eq(&self.tree, &panel.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        dock::set_dock_handle(&self.dock, handle.id, panel.id);
        Ok(())
    }

    /// M4 Phase 9's own no-live-window-needed proof pattern (matching
    /// `.click()`/`.hover()`/`.right_click()`): starts tracking a real
    /// drag as if `handle` had just been pressed. Returns whether a
    /// drag actually started -- `handle` must already be registered via
    /// `set_dock_handle`.
    fn start_panel_drag(&mut self, handle: PyRef<'_, Node>) -> bool {
        dock::start_drag(&self.dock, handle.id)
    }

    /// The real "release" half of a drag -- hit-tests `(x, y)` against
    /// this window's own real, current layout (computed fresh here, the
    /// same "nothing else does this for a `Window` with no render loop
    /// attached" reasoning `.click()` already states) and reparents the
    /// dragged panel into whichever registered zone encloses that
    /// point, if any and if different from its current zone.
    fn drop_panel_at(&mut self, x: f64, y: f64) {
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

    /// §14 step 15 (§11.7): creates a `NodeKind::VirtualList` of
    /// `item_count` logical rows, each `item_extent` px tall (only
    /// `ItemExtent::Fixed` is exposed -- see `engine_core::ItemExtent`'s
    /// own doc comment for why the variable-height variant isn't built
    /// yet). `materialize` is stored here, keyed by the new node's own
    /// `NodeId` -- not called yet; `set_virtual_list_window` is what
    /// actually invokes it, once per newly-visible index.
    ///
    /// `height` is now a real, meaningful viewport height (M8 Phase 3,
    /// §11.7/§11.8) -- `Window.scroll`/a real dispatched mouse wheel
    /// (`Tree::dispatch`'s own `InputEvent::Scroll` arm) clamp the
    /// list's own `scroll_offset` against exactly this value. Before
    /// this phase it was left `auto()`, which taffy resolves against
    /// *content* size -- and every materialized item is `Position::
    /// Absolute` (resolved from its own `inset`, not counted toward the
    /// parent's own intrinsic size, the same real fact `open_overlay`
    /// already established), so `auto()` never gave a real viewport
    /// height at all. Defaults to this `Window`'s own real height, the
    /// same fallback shape `width` already uses.
    #[pyo3(signature = (item_count, item_extent, materialize, width=None, height=None))]
    fn add_virtual_list(
        &mut self,
        item_count: usize,
        item_extent: f64,
        materialize: Py<PyAny>,
        width: Option<f32>,
        height: Option<f32>,
    ) -> Node {
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::VirtualList(VirtualListState::new(
                item_count,
                ItemExtent::Fixed(item_extent),
            )),
            Style {
                size: Size {
                    width: length(width.unwrap_or(self.width as f32)),
                    height: length(height.unwrap_or(self.height as f32)),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        drop(tree);
        self.materializers.insert(id, materialize);
        Node {
            id,
            tree: self.tree.clone(),
            handlers: self.handlers.clone(),
            context_menus: self.context_menus.clone(),
            theme: self.theme.clone(),
            completions: self.completions.clone(),
        }
    }

    /// §14 step 15 (§11.7): the "materialize item N" FFI entry point --
    /// invokes `list`'s own stored `materialize(index: int) -> (r, g, b,
    /// a)` callback once for every index in `start..end` not already
    /// materialized, and drops (`Tree::remove`, real generational
    /// `NodeId` invalidation) whatever was materialized outside that
    /// range. `list` must be a `Node` this same `Window` created via
    /// `add_virtual_list` -- raises `ValueError` otherwise (it either
    /// isn't a `VirtualList` at all, or belongs to a different `Window`
    /// with no recorded callback for it).
    ///
    /// A materializer callback raising propagates as a real `PyErr` --
    /// caught via an error slot the closure writes into, since
    /// `Tree::set_virtual_list_window`'s own closure signature is
    /// deliberately infallible (§4: `engine-core` stays `pyo3`-agnostic,
    /// so it can't know about `PyErr`). Not transactional: an index
    /// materialized earlier in the same call before a later index raises
    /// stays in the tree -- acceptable for this step's own scope
    /// (proving the callback mechanism and measuring its real GIL
    /// overhead below), not a general rollback guarantee.
    fn set_virtual_list_window(
        &mut self,
        list: PyRef<'_, Node>,
        start: usize,
        end: usize,
        py: Python<'_>,
    ) -> PyResult<()> {
        let materialize = self
            .materializers
            .get(&list.id)
            .ok_or(EngineError::NotAVirtualList)?
            .clone_ref(py);

        let mut tree = self.tree.borrow_mut();
        let item_extent = match &tree
            .get(list.id)
            .expect("set_virtual_list_window: Node holds a NodeId missing from its own Tree")
            .kind
        {
            NodeKind::VirtualList(state) => {
                let ItemExtent::Fixed(v) = state.item_extent;
                v
            }
            _ => return Err(EngineError::NotAVirtualList.into()),
        };

        let error: RefCell<Option<PyErr>> = RefCell::new(None);
        tree.set_virtual_list_window(list.id, start..end, |idx| {
            let outcome = materialize
                .call1(py, (idx,))
                .and_then(|result| result.extract::<(u8, u8, u8, u8)>(py));
            match outcome {
                Ok((r, g, b, a)) => (
                    NodeKind::Rect,
                    Style {
                        size: Size {
                            width: auto(),
                            height: length(item_extent as f32),
                        },
                        ..Default::default()
                    },
                    PaintProperties::new(Color::from_rgba8(r, g, b, a), 0.0, 0.0, 1.0),
                ),
                Err(e) => {
                    // First failure wins; later indices in this same
                    // call still need *some* real tuple to return, so
                    // this is an inert, fully transparent placeholder,
                    // not a value anyone is meant to see on screen.
                    error.borrow_mut().get_or_insert(e);
                    (
                        NodeKind::Rect,
                        Style::default(),
                        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 0.0),
                    )
                }
            }
        });

        match error.into_inner() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// M5 Phase 3 (§11.10/§11.11): creates a `NodeKind::Canvas` of the
    /// given size. `draw` is stored here, keyed by the new node's own
    /// `NodeId` -- not called yet; `redraw_canvas` is what actually
    /// invokes it, mirroring `add_virtual_list`/`materialize`'s own
    /// "store now, invoke later" shape exactly.
    #[pyo3(signature = (width, height, draw, x=None, y=None))]
    fn add_canvas(
        &mut self,
        width: f64,
        height: f64,
        draw: Py<PyAny>,
        x: Option<f32>,
        y: Option<f32>,
    ) -> Node {
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Canvas(engine_core::CanvasState::new()),
            positioned_style(
                Size {
                    width: length(width as f32),
                    height: length(height as f32),
                },
                x,
                y,
            ),
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        drop(tree);
        self.canvas_draws.insert(id, draw);
        Node {
            id,
            tree: self.tree.clone(),
            handlers: self.handlers.clone(),
            context_menus: self.context_menus.clone(),
            theme: self.theme.clone(),
            completions: self.completions.clone(),
        }
    }

    /// The real "draw callback" invocation entry point (§11.10/§11.11):
    /// calls `canvas`'s own stored `draw(ctx: CanvasContext) -> None`
    /// callback exactly once, then replaces `canvas`'s entire real
    /// content (`Tree::set_canvas_content`) with whatever `ctx`
    /// collected. Simpler than `set_virtual_list_window`'s own
    /// per-index error-deferral -- this is exactly one call, not N, so
    /// a raised exception propagates as a real `PyErr` directly, no
    /// error slot needed.
    fn redraw_canvas(&mut self, canvas: PyRef<'_, Node>, py: Python<'_>) -> PyResult<()> {
        let draw = self
            .canvas_draws
            .get(&canvas.id)
            .ok_or(EngineError::NotACanvas)?
            .clone_ref(py);

        {
            let tree = self.tree.borrow();
            if !matches!(
                tree.get(canvas.id)
                    .expect("redraw_canvas: Node holds a NodeId missing from its own Tree")
                    .kind,
                NodeKind::Canvas(_)
            ) {
                return Err(EngineError::NotACanvas.into());
            }
        }

        let ctx = Py::new(py, crate::canvas::CanvasContext::default())?;
        draw.call1(py, (ctx.clone_ref(py),))?;

        let ctx = ctx.borrow(py);
        self.tree.borrow_mut().set_canvas_content(
            canvas.id,
            ctx.commands.clone(),
            ctx.hit_test.clone(),
        );
        Ok(())
    }

    /// §11.7's own claim, matching `App::run`'s existing `PyWindow::
    /// __traverse__` reference in step 14's module doc comment: every
    /// stored `PyObject` a window keeps must be visible to CPython's
    /// cyclic GC, or a materializer closure that captures this very
    /// `Window` (a plausible, real pattern -- e.g. a bound method) forms
    /// a reference cycle the refcounting GC alone can never collect.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        for materializer in self.materializers.values() {
            visit.call(materializer)?;
        }
        // M5 Phase 3: `canvas_draws` is exactly the same class of stored
        // `PyObject` as `materializers` -- same cyclic-GC obligation.
        for draw in self.canvas_draws.values() {
            visit.call(draw)?;
        }
        for handler in self.handlers.borrow().values() {
            visit.call(handler)?;
        }
        // M9 Phase 2: `completions` holds real `Py<PyAny>` callbacks --
        // the same cyclic-GC obligation as `handlers`.
        for callback in self.completions.borrow().callbacks.values() {
            visit.call(callback)?;
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        self.materializers.clear();
        self.canvas_draws.clear();
        self.handlers.borrow_mut().clear();
        self.completions.borrow_mut().callbacks.clear();
    }
}
