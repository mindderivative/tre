//! `PyWindow` (§8's own sketch, split back out of `App` at exactly the
//! step `App`'s own module doc comment predicted -- §14 step 14, §11.1
//! multi-window). Owns one `Tree`, its root, and its own size/title --
//! everything `App::new`/`App::add_rect` used to hold directly, now per
//! window instead of assumed singular.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use engine_core::{NodeId, NodeKind, PaintProperties, Tree};
use engine_md3::DynamicTheme;
use peniko::Color;
use pyo3::class::{PyTraverseError, PyVisit};
use pyo3::prelude::*;
use taffy::prelude::{Position, Rect as TaffyRect, Size, Style, auto, length};

use crate::dispatch::{CompletionRegistry, HandlerMap, SharedCompletions};
use crate::dock::{self, SharedDockState};
use crate::node::Node;
use crate::terminal::TerminalSession;
use crate::view::View;

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

    /// M30 Phase 1 (§5, §7): the general role resolver `Button`'s five
    /// real MD3 variants need -- `on_surface()`'s own hardcoded single-
    /// field read doesn't reach `primary`/`on_primary`/`secondary_
    /// container`/`outline`/etc, and this project's own established
    /// precedent (`checkbox_state.mark_tint = theme.on_surface()`, and
    /// every sibling call site) always resolves a role name against
    /// whichever scheme (`light`/`dark`) is actually active -- this is
    /// that same resolution, generalized to any of `ColorScheme::role`'s
    /// real token names instead of a single hardcoded field access.
    /// Returns `None` both when no theme is set yet and when `name`
    /// isn't a real MD3 role -- callers already gate on `is_set()`
    /// before reading (the same real "un-themed default survives
    /// untouched" contract `on_surface()`'s own callers already rely
    /// on), so collapsing both cases to `None` costs nothing real.
    pub(crate) fn role(&self, name: &str) -> Option<Color> {
        let theme = self.theme.as_ref()?;
        let scheme = if self.dark { &theme.dark } else { &theme.light };
        scheme.role(name)
    }

    /// M20 Phase 1 (§7.1, §7.3): whether a real theme has actually been
    /// set yet. Needed because `Checkbox`/`Slider`'s own real, pre-
    /// existing defaults (white checkmark, gray track) are genuinely
    /// *different* colors than `on_surface()`'s own no-theme-set
    /// default (real black) -- unlike `InteractionState.tint`, whose
    /// own hardcoded default already happens to equal `on_surface()`'s
    /// no-theme value, so `enable_interaction`'s own unconditional read
    /// works for it "by coincidence." Reading `on_surface()`
    /// unconditionally here would silently replace every un-themed
    /// checkbox's white mark / slider's gray track with black --
    /// checked first instead, so the real historical default survives
    /// untouched until an app genuinely calls `set_theme`.
    pub(crate) fn is_set(&self) -> bool {
        self.theme.is_some()
    }
}

/// Shared the same way `HandlerMap`/`context_menus` are -- a `View`'s
/// own construction sites get a fresh, private, never-`Window`-linked
/// instance instead (see `view.rs`), matching this phase's own stated
/// scope: only `Window`-created nodes ever see a real theme.
pub(crate) type SharedTheme = Rc<RefCell<ThemeState>>;

/// M33 Phase 2 (§4, §5, §8): a `Window`'s own real width/height,
/// shared the identical way `SharedTheme`/`HandlerMap`/`context_menus`
/// already are -- `Cell`, not `RefCell`, since `u32` is `Copy` and
/// every real access is a plain get/set, never a borrow that could
/// outlive a single statement. Closes the real, stated v1 limit M32
/// Phase 2 left open: a live, winit-driven resize used to reach only
/// `WindowRuntime`'s own separate, non-shared `u32` copy, never this
/// `PyWindow`'s own fields -- `App::run`'s own `WindowSetup`/
/// `WindowRuntime` now clone this same `Rc<Cell<u32>>` instead of
/// copying its value once at startup, so a real resize's own `.set()`
/// call is immediately visible to every interactive `add_*` factory
/// method's own `self.width`/`self.height` read, live.
pub(crate) type SharedSize = Rc<Cell<u32>>;

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
/// Shared by `window_factory.rs`'s `add_rect`/`add_text`/`add_checkbox`/
/// `add_slider`/`add_image`/`add_icon`/`add_text_field` and `window_
/// virtual_canvas.rs`'s `add_canvas` -- `pub(crate)` for exactly that
/// cross-file reason, kept here since it belongs to neither group more
/// than the other.
pub(crate) fn positioned_style(
    size: Size<taffy::style::Dimension>,
    x: Option<f32>,
    y: Option<f32>,
) -> Style {
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
    pub(crate) width: SharedSize,
    pub(crate) height: SharedSize,
    /// M27 Phase 3: wrapped in a `RefCell` (previously a plain
    /// `HashMap`) so `add_virtual_list` can be `&self` like every other
    /// `add_*` method -- the real, concrete need Phase 1's own stated
    /// residual limitation predicted: a real click handler that
    /// navigates to a new screen and builds a `Canvas`/`VirtualList`
    /// there (an entirely ordinary pattern) needs this, confirmed by
    /// hitting the exact predicted "Already borrowed" panic while
    /// building the showcase demo's own motion screen.
    pub(crate) materializers: RefCell<HashMap<NodeId, Py<PyAny>>>,
    /// M5 Phase 3 (§11.10/§11.11): the "draw callback" storage,
    /// mirroring `materializers`'s own shape exactly -- stored by
    /// `add_canvas`, invoked (exactly once per call) only by the real
    /// entry point `redraw_canvas`, never automatically every frame
    /// (see `PLAN.md`: no consumer has asked for that yet). Also
    /// `RefCell`-wrapped as of M27 Phase 3, for the identical real
    /// reason `materializers` is.
    pub(crate) canvas_draws: RefCell<HashMap<NodeId, Py<PyAny>>>,
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
    /// M30 Phase 9 Step 4 (§5, §8, §10): every real, live `Terminal`
    /// session this `Window` has spawned, keyed by its own real
    /// `NodeId` -- shared with `App.run`'s own per-frame `WindowRuntime`
    /// (`app.rs`'s own real drain loop), the identical real shape
    /// `context_menus`/`dock`/`theme` already have: plain data, no
    /// `Py<PyAny>` involved, so no `__traverse__`/`__clear__` GC
    /// obligation either.
    pub(crate) terminals: Rc<RefCell<HashMap<NodeId, TerminalSession>>>,
}

/// Real review finding: every `add_*`/`build_shell` method below used
/// to build an identical 6-field `Node` struct literal by hand (the
/// same shared state every `Node` this `Window` hands out always
/// carries) -- factored out once, so a future new shared field (the
/// exact class of thing `completions`, M9 Phase 2, once was) only
/// needs updating here, not at every one of the 11 call sites this
/// used to be duplicated across. A plain, non-`#[pymethods]` `impl`
/// block -- `pyo3` has no way to expose a method taking a raw
/// `NodeId` as a Python-callable argument, and this helper is only
/// ever called from Rust, never from Python.
impl PyWindow {
    pub(crate) fn wrap_node(&self, id: NodeId) -> Node {
        Node {
            id,
            tree: self.tree.clone(),
            handlers: self.handlers.clone(),
            context_menus: self.context_menus.clone(),
            theme: self.theme.clone(),
            completions: self.completions.clone(),
        }
    }
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
            width: Rc::new(Cell::new(width)),
            height: Rc::new(Cell::new(height)),
            materializers: RefCell::new(HashMap::new()),
            canvas_draws: RefCell::new(HashMap::new()),
            handlers: Rc::new(RefCell::new(HashMap::new())),
            context_menus: Rc::new(RefCell::new(HashMap::new())),
            dock: Rc::new(RefCell::new(dock::DockState::new())),
            theme: Rc::new(RefCell::new(ThemeState::default())),
            completions: Rc::new(RefCell::new(CompletionRegistry::new())),
            terminals: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    /// M42 Phase 1 (§4, §5, §8, §16.2, §16.4): the real, first entry
    /// point wiring `View`'s own declarative layer into a live,
    /// `winit`-driven window -- shares `view`'s own `Rc<RefCell<Tree>>`,
    /// root, `handlers`/`context_menus`, and (M42 Phase 1's own new
    /// fields) `theme`/`completions` directly into a new `Window`, the
    /// identical `Rc`-clone pattern `wrap_node` already uses for every
    /// `Node` a `Window` hands out -- not a second, parallel tree.
    ///
    /// **Real, load-bearing consequence, not obvious from the signature
    /// alone:** `view.width`/`view.height` become the *same* shared
    /// `Rc<Cell<u32>>` this new `Window`'s own `width`/`height` fields
    /// hold (mirroring `SharedSize`'s own established M33 Phase 2
    /// pattern) -- a real live resize (`App::run`'s own `on_input`
    /// closure, `app.rs`) writes through this one shared cell, so
    /// `view.click()`/`view.hover()`, called again after the window is
    /// shown, see the window's true current size immediately, not a
    /// stale value captured at `from_view` time.
    ///
    /// `dock`/`materializers`/`canvas_draws`/`terminals` default-empty,
    /// confirmed safe: `engine-spec`'s own YAML builder (`Reconciler::
    /// load`, which built `view`'s tree) has no `Terminal`/`VirtualList`/
    /// `Canvas` case, so a View-built tree can never contain a `NodeKind`
    /// that would need any of them populated.
    ///
    /// **Requires no changes to `App::run`/`WindowSetup`/`WindowRuntime`
    /// (`app.rs`):** confirmed by reading `App::run`'s own setup step --
    /// it already builds a `WindowSetup` generically from any `PyWindow`
    /// instance's `pub(crate)` fields, with no assumption a `PyWindow`
    /// was ever constructed via `PyWindow::new`. A `Window` built this
    /// way works with the existing `App.add_window()`/`App.run()` path
    /// completely unmodified.
    #[staticmethod]
    #[pyo3(signature = (view, width=480, height=200, title="tre v2"))]
    fn from_view(view: PyRef<'_, View>, width: u32, height: u32, title: &str) -> PyWindow {
        view.width.set(width);
        view.height.set(height);
        Self {
            tree: view.tree.clone(),
            root: view.reconciler.root(),
            title: title.to_string(),
            width: view.width.clone(),
            height: view.height.clone(),
            materializers: RefCell::new(HashMap::new()),
            canvas_draws: RefCell::new(HashMap::new()),
            handlers: view.handlers.clone(),
            context_menus: view.context_menus.clone(),
            dock: Rc::new(RefCell::new(dock::DockState::new())),
            theme: view.theme.clone(),
            completions: view.completions.clone(),
            terminals: Rc::new(RefCell::new(HashMap::new())),
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
        let mut tree = self.tree.borrow_mut();
        tree.set_all_interaction_tints(tint);
        // M20 Phase 1 (§7.1, §7.3): the real, deliberate scope choice
        // -- reuses this exact same already-resolved "on-surface" tint
        // rather than resolving a second, more specific MD3 role per
        // component.
        tree.set_all_component_tints(tint);
    }

    /// §11.7's own claim, matching `App::run`'s existing `PyWindow::
    /// __traverse__` reference in step 14's module doc comment: every
    /// stored `PyObject` a window keeps must be visible to CPython's
    /// cyclic GC, or a materializer closure that captures this very
    /// `Window` (a plausible, real pattern -- e.g. a bound method) forms
    /// a reference cycle the refcounting GC alone can never collect.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        for materializer in self.materializers.borrow().values() {
            visit.call(materializer)?;
        }
        // M5 Phase 3: `canvas_draws` is exactly the same class of stored
        // `PyObject` as `materializers` -- same cyclic-GC obligation.
        for draw in self.canvas_draws.borrow().values() {
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
        self.materializers.borrow_mut().clear();
        self.canvas_draws.borrow_mut().clear();
        self.handlers.borrow_mut().clear();
        self.completions.borrow_mut().callbacks.clear();
    }
}
