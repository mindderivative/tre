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
use crate::listeners::WindowListenerMap;
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
    /// M50 Phase 1: shape/elevation overrides for the imperative MD3
    /// catalog, populated from `Window.set_theme`'s `custom_theme`
    /// (`ThemeSpec.components`, `engine-spec/src/theme.rs`). Empty (the
    /// default) is a true no-op -- `shape`/`elevation` below both
    /// return `None` for every key, so every existing `add_*` factory's
    /// own hardcoded fallback survives untouched, the identical
    /// "un-themed default survives" contract `is_set()`'s own callers
    /// already rely on for color. M61 (§16.3): stores `Resolved
    /// ComponentOverride`, not the parse-time `engine_spec::
    /// ComponentOverride` directly -- `resolve_components` (below)
    /// converts each entry's own `corner_radius`/`elevation` (a literal
    /// or a named token) into a plain `f64` exactly once, at real
    /// `set_theme` time, so `shape`/`elevation` below stay the simple,
    /// infallible `Option<f64>` they always were -- zero ripple into
    /// `window_factory.rs`'s dozens of existing `theme.shape(...)`
    /// call sites.
    components: HashMap<String, ResolvedComponentOverride>,
    /// M63 (§7.1, §16.3): per-role overrides for the imperative MD3
    /// catalog's own typography, populated from `Window.set_theme`'s
    /// `custom_theme` (`ThemeSpec.typography`, `engine-spec/src/
    /// theme.rs`) -- `components` above's own real sibling, one real
    /// tier simpler: `TypographyOverride`'s 4 fields are already plain
    /// literals (`Option<String>`/`Option<f32>`), no token-reference
    /// machinery like `ComponentOverride`'s `ShapeOrElevationSpec` at
    /// all, so nothing needs eager resolution at `set_theme` time --
    /// the parse-time struct is stored directly. Empty (the default) is
    /// a true no-op, the identical contract `components`'s own doc
    /// comment already states: `typography()` below still returns the
    /// real, shipped MD3 default for every role regardless.
    typography: HashMap<String, engine_spec::TypographyOverride>,
}

/// M63 (§7.1, §16.3): `engine_md3::TypeStyle`'s own real theme-resolved
/// form -- `font_family` widened from `&'static str` to an owned
/// `String` since a theme's own override is real caller-supplied data,
/// not a compile-time constant, the identical real reason `Resolved
/// ComponentOverride` exists as its own distinct type rather than
/// reusing the parse-time struct directly.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResolvedTypeStyle {
    pub(crate) font_family: String,
    pub(crate) font_weight: f32,
    pub(crate) font_size: f32,
    pub(crate) line_height: f32,
}

/// M61 (§16.3): `engine_spec::ComponentOverride`'s own real resolved
/// form -- both fields already-concrete `f64`s, a token name (if any)
/// already looked up. See `ThemeState.components`'s own doc comment for
/// why this exists as a distinct type rather than storing the parse-
/// time struct directly.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct ResolvedComponentOverride {
    corner_radius: Option<f64>,
    elevation: Option<f64>,
}

/// M61 (§16.3): resolves every `engine_spec::ComponentOverride` in
/// `raw` into a `ResolvedComponentOverride`, failing loudly (a real
/// Python `ValueError`, naming the offending component key and token)
/// on the first unrecognized token name -- the identical "fail loudly
/// at the boundary" contract `engine-spec::build.rs`'s own `Spec
/// Error::UnknownShapeToken` already established for the declarative
/// `StyleSpec` path, mirrored here for the imperative one.
fn resolve_components(
    raw: HashMap<String, engine_spec::ComponentOverride>,
) -> PyResult<HashMap<String, ResolvedComponentOverride>> {
    raw.into_iter()
        .map(|(key, override_)| {
            let resolve = |field: &str,
                           value: &Option<engine_spec::ShapeOrElevationSpec>,
                           is_elevation: bool| {
                value
                    .as_ref()
                    .map(|v| {
                        v.resolve(is_elevation).ok_or_else(|| {
                            pyo3::exceptions::PyValueError::new_err(format!(
                                "components: {key:?} has an unknown {field} token: {v:?}"
                            ))
                        })
                    })
                    .transpose()
            };
            Ok((
                key.clone(),
                ResolvedComponentOverride {
                    corner_radius: resolve("corner_radius", &override_.corner_radius, false)?,
                    elevation: resolve("elevation", &override_.elevation, true)?,
                },
            ))
        })
        .collect()
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

    /// M63 (§7.1, §16.3): resolves one real MD3 typography role
    /// (`"label_large"`, etc.) to its final, real values -- `engine_md3
    /// ::type_style_named`'s own shipped default first, then any of
    /// `self.typography[role]`'s own 4 fields, independently, on top
    /// (the identical per-field-override cascade `engine-spec::build.rs
    /// ::resolve_text_style` already establishes for the declarative
    /// surface). Unlike `shape`/`elevation` above, this never needs an
    /// external `.unwrap_or(SHIPPED_CONST)` at each call site: `engine_
    /// md3::type_style_named` already *is* the real, correct un-themed
    /// default, whether or not a theme has been set at all -- `None` is
    /// reserved for a genuinely unrecognized role name, which every
    /// real internal call site (a fixed string literal this catalog
    /// itself chose) should never actually produce.
    pub(crate) fn typography(&self, role: &str) -> Option<ResolvedTypeStyle> {
        let base = engine_md3::type_style_named(role)?;
        let override_ = self.typography.get(role);
        Some(ResolvedTypeStyle {
            font_family: override_
                .and_then(|o| o.font_family.clone())
                .unwrap_or_else(|| base.font_family.to_string()),
            font_weight: override_
                .and_then(|o| o.font_weight)
                .unwrap_or(base.font_weight),
            font_size: override_
                .and_then(|o| o.font_size)
                .unwrap_or(base.font_size),
            line_height: override_
                .and_then(|o| o.line_height)
                .unwrap_or(base.line_height),
        })
    }

    /// M50 Phase 1: the real 2-tier lookup every `add_*` factory's own
    /// corner-radius consults, from Phase 2 onward -- `"<component>.
    /// <variant>"` first (when `variant` is given), then the bare
    /// `"<component>"` key, `None` if neither has this specific field
    /// set. Callers keep their own existing hardcoded constant/formula
    /// as the fallback (`theme.shape("card", None).unwrap_or(CARD_
    /// CORNER_RADIUS)`) -- this method never invents a default of its
    /// own. **Real, deliberate per-field fallthrough, not per-entry:**
    /// a variant-specific entry that sets only `elevation` must not
    /// block the bare key's own `corner_radius` from being found --
    /// each field is looked up independently, not "does a variant
    /// entry exist at all."
    pub(crate) fn shape(&self, component: &str, variant: Option<&str>) -> Option<f64> {
        self.lookup(component, variant, |o| o.corner_radius)
    }

    /// `shape`'s own sibling for elevation -- identical 2-tier,
    /// per-field lookup.
    pub(crate) fn elevation(&self, component: &str, variant: Option<&str>) -> Option<f64> {
        self.lookup(component, variant, |o| o.elevation)
    }

    /// Test-only constructor (`ThemeState`'s own fields are private to
    /// this module) -- lets a `RetitheHook` builder's own unit tests,
    /// living beside the factory they belong to in `window_factory.rs`,
    /// exercise a real, themed `ThemeState` without needing `PyWindow`/
    /// `Window.set_theme`/the GIL. `#[cfg(test)]`-gated: compiled into
    /// test binaries only, zero production API surface.
    #[cfg(test)]
    pub(crate) fn for_test(seed: Color) -> Self {
        Self {
            theme: Some(DynamicTheme::from_seed(seed)),
            dark: false,
            components: HashMap::new(),
            typography: HashMap::new(),
        }
    }

    /// `for_test`'s own sibling with a real `components:` override --
    /// needed by any `RetitheHook` test that also proves a corner_
    /// radius/elevation override re-resolves live, not just color.
    /// Takes the real parse-time `engine_spec::ComponentOverride` (what
    /// a test naturally constructs, matching real theme YAML shape) and
    /// resolves it through the identical real `resolve_components` path
    /// `Window.set_theme` itself uses -- `expect`s success, since a
    /// test passing a deliberately-unresolvable token would be testing
    /// the wrong thing here (that path has its own dedicated test).
    #[cfg(test)]
    pub(crate) fn for_test_with_components(
        seed: Color,
        components: HashMap<String, engine_spec::ComponentOverride>,
    ) -> Self {
        Self {
            theme: Some(DynamicTheme::from_seed(seed)),
            dark: false,
            components: resolve_components(components)
                .expect("test-supplied components must all resolve"),
            typography: HashMap::new(),
        }
    }

    /// `for_test`'s own sibling with a real `typography:` override --
    /// no `resolve_*` call needed (unlike `for_test_with_components`),
    /// since `TypographyOverride`'s own fields are already plain
    /// literals with nothing to fail on.
    #[cfg(test)]
    pub(crate) fn for_test_with_typography(
        seed: Color,
        typography: HashMap<String, engine_spec::TypographyOverride>,
    ) -> Self {
        Self {
            theme: Some(DynamicTheme::from_seed(seed)),
            dark: false,
            components: HashMap::new(),
            typography,
        }
    }

    fn lookup(
        &self,
        component: &str,
        variant: Option<&str>,
        field: impl Fn(&ResolvedComponentOverride) -> Option<f64>,
    ) -> Option<f64> {
        if let Some(variant) = variant
            && let Some(value) = self
                .components
                .get(&format!("{component}.{variant}"))
                .and_then(&field)
        {
            return Some(value);
        }
        self.components.get(component).and_then(field)
    }
}

/// Shared the same way `HandlerMap`/`context_menus` are -- a `View`'s
/// own construction sites get a fresh, private, never-`Window`-linked
/// instance instead (see `view.rs`), matching this phase's own stated
/// scope: only `Window`-created nodes ever see a real theme.
pub(crate) type SharedTheme = Rc<RefCell<ThemeState>>;

/// M71 (§7.1, §8): a real, read-only Python-facing view onto a
/// `Window`'s own `ThemeState` -- until now, every one of `role`/
/// `is_set`/`shape`/`elevation`/`typography` was `pub(crate)`, reachable
/// only from `window_factory.rs`'s own ~35 composition-only factories,
/// with zero Python-facing equivalent anywhere (confirmed via grep
/// before writing this). That's the one real, confirmed blocker to
/// building those same compositions in Python instead (the sibling
/// `Tesserae` project's own real next milestone) -- an app author
/// reimplementing `add_button`'s own construction logic in Python needs
/// the exact same theme lookups that Rust version already makes, or the
/// result silently drifts from real MD3 parity the moment a theme is
/// set. A cheap `Rc` clone, not a fresh resolve -- mirrors `Node`'s own
/// `theme: SharedTheme` field exactly (`build_node`/every `add_*`
/// factory's own wrapping pattern), so reading `window.theme` twice
/// sees the identical live state a real `set_theme()` call in between
/// would change.
#[pyclass(unsendable, name = "Theme")]
pub struct Theme {
    state: SharedTheme,
}

#[pymethods]
impl Theme {
    /// Mirrors `ThemeState::role` exactly, converted to the same
    /// `(r, g, b, a)` u8 tuple every other real Python-facing color
    /// already uses (`view.rs::parse_background_color`'s own real
    /// `Color::to_rgba8().to_u8_array()` conversion, not a fresh one).
    /// `None` both when no theme is set yet and when `name` isn't a
    /// real MD3 role -- the identical real "un-themed default survives"
    /// contract every `ThemeState::role` caller already relies on, so
    /// collapsing both cases costs nothing real here either.
    fn role(&self, name: &str) -> Option<(u8, u8, u8, u8)> {
        let [r, g, b, a] = self.state.borrow().role(name)?.to_rgba8().to_u8_array();
        Some((r, g, b, a))
    }

    /// Mirrors `ThemeState::is_set` exactly.
    fn is_set(&self) -> bool {
        self.state.borrow().is_set()
    }

    /// Mirrors `ThemeState::shape` exactly -- same real 2-tier,
    /// per-field lookup (`"<component>.<variant>"` first, then the bare
    /// `"<component>"` key), same `None`-means-"no override, use your
    /// own formula default" contract every `add_*` factory's own
    /// `.unwrap_or(...)` call site already relies on.
    #[pyo3(signature = (component, variant=None))]
    fn shape(&self, component: &str, variant: Option<&str>) -> Option<f64> {
        self.state.borrow().shape(component, variant)
    }

    /// `shape`'s own sibling for elevation -- identical contract.
    #[pyo3(signature = (component, variant=None))]
    fn elevation(&self, component: &str, variant: Option<&str>) -> Option<f64> {
        self.state.borrow().elevation(component, variant)
    }

    /// Mirrors `ThemeState::typography` exactly, flattened from
    /// `ResolvedTypeStyle`'s own 4 named fields into a plain
    /// `(family, weight, size, line_height)` tuple -- a real, shipped
    /// MD3 default for every recognized role regardless of whether a
    /// theme is set, `None` reserved for a genuinely unrecognized role
    /// name (the same real contract `ThemeState::typography`'s own doc
    /// comment already states).
    fn typography(&self, role: &str) -> Option<(String, f32, f32, f32)> {
        let resolved = self.state.borrow().typography(role)?;
        Some((
            resolved.font_family,
            resolved.font_weight,
            resolved.font_size,
            resolved.line_height,
        ))
    }
}

/// M52 Phase 1 (§7.1, §7.3): live re-theme for `Window`'s own imperative
/// MD3 catalog -- the "live token-linkage" mechanism named and
/// deliberately deferred since M49. Each themed `add_*` factory
/// registers one of these (via a small, named, independently-testable
/// builder function like `button_retheme_hook` in `window_factory.rs`,
/// not an inline closure) right before returning its node(s); `Window.
/// set_theme` replays every registered hook after installing the new
/// `ThemeState`, so an already-built button's real container/label
/// color and corner_radius/elevation are recomputed and overwritten in
/// place -- the exact same "recompute from the same inputs the
/// component's own `resolve_*_colors`/`theme.shape`/`theme.elevation`
/// call originally used, then snap the result in" convention `patch_
/// node` (M51, `engine-spec::build`) already established for the
/// declarative surface. `Box<dyn Fn(&ThemeState, &mut Tree)>`, not
/// `Py<PyAny>`: every hook this milestone registers closes only over
/// plain Rust values (`NodeId`s, owned `String`/`f32` params) and calls
/// only pure-Rust resolvers (`resolve_button_colors` and siblings all
/// take `&ThemeState` plus plain args, confirmed via direct read, no
/// `Python<'_>`/GIL type anywhere) -- so no `__traverse__`/`__clear__`
/// GC obligation applies, unlike `handlers`/`completions`.
pub(crate) type RetitheHook = Box<dyn Fn(&ThemeState, &mut Tree)>;

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

/// M42 Phase 2 (§4, §5, §8, §16.2, §16.4): the real, atomically
/// swappable "what a live `Window` currently dispatches against and
/// paints" bundle -- `Window.show_view` writes a whole new one of these
/// in a single `RefCell` replace, and `WindowRuntime`'s own per-frame/
/// per-input closures (`app.rs`) re-sync their existing plain `tree`/
/// `root`/`handlers`/`context_menus` fields from it at the top of every
/// real invocation, instead of every one of this crate's dozens of
/// pre-existing `runtime.tree`/`.root`/`.handlers`/`.context_menus`
/// call sites needing to be rewritten through an extra layer of
/// indirection.
///
/// **Real, load-bearing correctness finding, not covered by this
/// milestone's own original plan text (which named only a `(Tree,
/// NodeId)` pair):** `engine_core::NodeId` is a `slotmap` generational
/// key (`crates/engine-core/src/node.rs`), unique only *within* the
/// `Tree` that allocated it -- two independent `View`s' own root nodes
/// can (and, confirmed by how `slotmap` allocates keys, routinely do)
/// collide on the identical raw value. `HandlerMap`/`context_menus` are
/// keyed by `(NodeId, EventKind)`/`NodeId` alone, with no per-`Tree`
/// namespacing -- sharing one persistent map across a `show_view`
/// switch would silently cross-wire a different `View`'s old callback
/// onto a colliding `NodeId` in the new one. `tree`/`root`/`handlers`/
/// `context_menus` are therefore swapped together, atomically, as one
/// unit. `theme`/`completions` deliberately stay outside it -- see
/// `Window.show_view`'s own doc comment for why.
pub(crate) struct ActiveTree {
    pub(crate) tree: Rc<RefCell<Tree>>,
    pub(crate) root: NodeId,
    pub(crate) handlers: HandlerMap,
    pub(crate) context_menus: Rc<RefCell<HashMap<NodeId, NodeId>>>,
}

pub(crate) type SharedActiveTree = Rc<RefCell<ActiveTree>>;

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
    /// M52 Phase 1 (§7.1, §7.3): every registered `RetitheHook`, in
    /// registration order (order never matters for correctness -- each
    /// hook only ever writes its own captured `NodeId`s). Plain
    /// `RefCell<Vec<...>>`, not `Rc`-shared like `theme`/`handlers` --
    /// only this `Window`'s own `add_*` methods (push) and `set_theme`
    /// (replay) ever touch it, the same "not shared with `Node`" shape
    /// `materializers`/`canvas_draws` already have. **Real, deliberately
    /// accepted limitation, named not hidden:** never pruned when a
    /// hook's own node(s) are later removed (`Node.remove()`) -- a
    /// stale hook becomes a silent no-op on the next `set_theme` call
    /// (`Tree::get_mut` returns `None`), the identical accepted
    /// tradeoff `handlers`/`materializers`/`context_menus` already have
    /// today (confirmed via direct read: none of them are pruned on
    /// node removal either).
    pub(crate) retheme_hooks: RefCell<Vec<RetitheHook>>,
    /// M42 Phase 2 (§4, §5, §8, §16.2, §16.4): the real, swappable
    /// "currently shown" bundle -- initialized to mirror this `Window`'s
    /// own `tree`/`root`/`handlers`/`context_menus` at construction
    /// time (`new`/`from_view`), and never read directly by any `add_*`
    /// factory below (those keep using the plain fields above
    /// unchanged) -- only `App::run`'s own `WindowSetup`/`WindowRuntime`
    /// and `show_view` (below) ever touch it.
    pub(crate) active: SharedActiveTree,
    /// M94: `window.on(...)` listeners -- the window's own, independent of
    /// which tree it shows.
    pub(crate) window_listeners: WindowListenerMap,
    /// M94: the OS window while `App.run()` has it open -- `None` before
    /// and after. `window.set(title=...)` and `window.get("scale_factor")`
    /// reach it here; `App.run()` fills and clears it.
    pub(crate) os_window: SharedOsWindow,
}

/// M94: see `PyWindow::os_window`.
pub(crate) type SharedOsWindow = Rc<RefCell<Option<std::sync::Arc<winit::window::Window>>>>;

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
        let tree = Rc::new(RefCell::new(tree));
        let handlers: HandlerMap = Rc::new(RefCell::new(HashMap::new()));
        let context_menus = Rc::new(RefCell::new(HashMap::new()));
        let active = Rc::new(RefCell::new(ActiveTree {
            tree: tree.clone(),
            root,
            handlers: handlers.clone(),
            context_menus: context_menus.clone(),
        }));
        Self {
            tree,
            root,
            title: title.to_string(),
            width: Rc::new(Cell::new(width)),
            height: Rc::new(Cell::new(height)),
            materializers: RefCell::new(HashMap::new()),
            canvas_draws: RefCell::new(HashMap::new()),
            handlers,
            context_menus,
            dock: Rc::new(RefCell::new(dock::DockState::new())),
            theme: Rc::new(RefCell::new(ThemeState::default())),
            completions: Rc::new(RefCell::new(CompletionRegistry::new())),
            terminals: Rc::new(RefCell::new(HashMap::new())),
            retheme_hooks: RefCell::new(Vec::new()),
            active,
            window_listeners: Rc::new(RefCell::new(HashMap::new())),
            os_window: Rc::new(RefCell::new(None)),
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
        let root = view.reconciler.root();
        let active = Rc::new(RefCell::new(ActiveTree {
            tree: view.tree.clone(),
            root,
            handlers: view.handlers.clone(),
            context_menus: view.context_menus.clone(),
        }));
        Self {
            tree: view.tree.clone(),
            root,
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
            // M52 Phase 1: fresh, empty, not shared with `view` -- a
            // `View`-built tree never calls an `add_*` factory (it goes
            // through `engine_spec::build`/`Reconciler`, a completely
            // separate path that already gets its own live re-theme via
            // `View.set_theme`/`Reconciler::retheme`, M51), so there is
            // nothing to inherit here. A caller who then calls `add_*`
            // directly on this `Window` still gets a real, correctly-
            // registered hook for that node going forward.
            retheme_hooks: RefCell::new(Vec::new()),
            active,
            window_listeners: Rc::new(RefCell::new(HashMap::new())),
            os_window: Rc::new(RefCell::new(None)),
        }
    }

    /// M42 Phase 2 (§4, §5, §8, §16.2, §16.4): switches which `View` a
    /// *live* `Window` shows, without closing/reopening it -- the real
    /// capability the user's own explicit plan-review feedback asked
    /// for: "this allows for switching of current views without needing
    /// to bootstrap each view/viewModel." Each named `View` a real
    /// Tesserae-style app keeps around stays fully alive (its own
    /// `Reconciler`/bindings/`Signal` subscriptions intact, untouched by
    /// this call) -- only the shared `ActiveTree` bundle this `Window`'s
    /// live render loop reads from is atomically replaced, one `RefCell`
    /// write, picked up on the very next real frame.
    ///
    /// Mirrors `from_view`'s own real "sync the window's current size
    /// into the view" step, so `view`'s own `click()`/`hover()` lay out
    /// at this window's true current size immediately after switching --
    /// **real, stated limit, not silently glossed over:** unlike
    /// `from_view`'s own `width`/`height`-sharing (the *same*
    /// `Rc<Cell<u32>>`), this only copies the *current* size once, at
    /// switch time -- a later live resize while a *different* `View` is
    /// showing won't keep this one's own `width`/`height` in sync until
    /// `show_view` is called on it again. Real, separate follow-up if a
    /// live resize ever needs to reach every registered View at once,
    /// not just the currently-active one -- not needed for this
    /// milestone's own real scope (only the active View is ever visible
    /// or interactive at a time).
    fn show_view(&self, view: PyRef<'_, View>) {
        view.width.set(self.width.get());
        view.height.set(self.height.get());
        *self.active.borrow_mut() = ActiveTree {
            tree: view.tree.clone(),
            root: view.reconciler.root(),
            handlers: view.handlers.clone(),
            context_menus: view.context_menus.clone(),
        };
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
    /// M49 Phase 1: `custom_theme` (a path to a real theme YAML file,
    /// `ThemeSpec` -- `engine-spec/src/theme.rs`) applies its `colors:`
    /// role overrides to *both* `light`/`dark` schemes before either is
    /// stored -- every existing `resolve_*_colors` call site across
    /// `window_factory.rs`'s real MD3 catalog needs zero changes, since
    /// they already resolve colors through this same `ThemeState::role`
    /// -> `ColorScheme::role` chain. If `custom_theme` names its own
    /// `seed:`, it overrides the `seed` argument -- unlike `View::new`'s
    /// own optional `theme_seed` (where an explicit argument can signal
    /// deliberate intent by being present at all), `seed` here is
    /// *required*, so there's no way to omit it to mean "defer to
    /// whatever the theme file says" -- the theme file winning when it
    /// has an opinion is the only way that deference is expressible.
    /// `custom_theme`'s own `styles:`/`dark` are not used here at all --
    /// `styles:` only matters to the declarative `StyleSpec` cascade
    /// (`View`). M50 Phase 1: `custom_theme`'s own `components:` *is*
    /// now used here -- stored into `ThemeState.components`, consulted
    /// by `ThemeState::shape`/`elevation` as each `add_*` factory
    /// consults it (see `window_factory.rs`; `M49`'s own doc comment
    /// above, "corner-radius/elevation never consult any theme
    /// regardless," was the real state of the world *before* M50, not
    /// a permanent limit). M50 Phase 5: `default_theme` (omitted ->
    /// the engine's own shipped default, matching `View::new`'s
    /// identical convention) supplies the baseline `components:`
    /// underneath `custom_theme`'s own overrides (custom wins on any
    /// overlapping key) -- deliberately scoped to `components:` only:
    /// `default_theme`'s own `colors:`/`seed:` are never consulted
    /// here, since `Window`'s color/seed story is already fully served
    /// by the required `seed` argument plus `custom_theme`'s own
    /// override, and letting a second theme file quietly compete with
    /// a required argument would be a real, confusing ambiguity this
    /// milestone deliberately doesn't introduce.
    ///
    /// M86: `default_theme_spec`/`custom_theme_spec` are the dict forms
    /// of `default_theme`/`custom_theme` (the same schema the YAML file
    /// would hold), each mutually exclusive with its path twin -- so a
    /// framework that loads its own theme files never hands `tre` a
    /// path.
    #[pyo3(signature = (seed, dark=false, default_theme=None, custom_theme=None, default_theme_spec=None, custom_theme_spec=None))]
    #[allow(clippy::too_many_arguments)]
    fn set_theme(
        &self,
        py: Python<'_>,
        seed: (u8, u8, u8, u8),
        dark: bool,
        default_theme: Option<String>,
        custom_theme: Option<String>,
        default_theme_spec: Option<Py<PyAny>>,
        custom_theme_spec: Option<Py<PyAny>>,
    ) -> PyResult<()> {
        let default_theme_spec = crate::view::resolve_theme_input(
            py,
            "Window.set_theme",
            ("default_theme=", default_theme.as_deref()),
            ("default_theme_spec=", default_theme_spec.as_ref()),
        )?
        .unwrap_or_else(crate::view::shipped_default_theme_spec);
        let custom_theme_spec = crate::view::resolve_theme_input(
            py,
            "Window.set_theme",
            ("custom_theme=", custom_theme.as_deref()),
            ("custom_theme_spec=", custom_theme_spec.as_ref()),
        )?;

        // M61 (§16.3): resolved *before* any `state` mutation begins
        // below (matching `apply_overrides`'s own identical "fail
        // before touching state" discipline just below this) -- a
        // theme with an unresolvable `components:` token must leave
        // `ThemeState` completely untouched, the same real "a failed
        // `set_theme` call is a true no-op" contract every other real
        // failure path in this method already gives.
        let mut components = default_theme_spec.components.clone();
        if let Some(custom) = &custom_theme_spec {
            components.extend(custom.components.clone());
        }
        let resolved_components = resolve_components(components)?;

        // M63 (§7.1, §16.3): `typography:` -- the identical real merge
        // `components:` above already establishes (default theme first,
        // custom theme's own entries layered on top, custom wins on any
        // overlapping key). No fallible resolution needed here (unlike
        // `components:`'s own `resolve_components` call) -- `Typography
        // Override`'s 4 fields are already plain literals.
        let mut typography = default_theme_spec.typography.clone();
        if let Some(custom) = &custom_theme_spec {
            typography.extend(custom.typography.clone());
        }

        let seed = match custom_theme_spec
            .as_ref()
            .map(crate::view::theme_spec_seed)
            .transpose()?
            .flatten()
        {
            Some(theme_seed) => theme_seed,
            None => seed,
        };

        let (r, g, b, a) = seed;
        let mut dynamic = DynamicTheme::from_seed(Color::from_rgba8(r, g, b, a));
        if let Some(custom) = &custom_theme_spec
            && !custom.colors.is_empty()
        {
            dynamic
                .light
                .apply_overrides(&custom.colors)
                .map_err(pyo3::exceptions::PyValueError::new_err)?;
            dynamic
                .dark
                .apply_overrides(&custom.colors)
                .map_err(pyo3::exceptions::PyValueError::new_err)?;
        }
        let mut state = self.theme.borrow_mut();
        state.theme = Some(dynamic);
        state.dark = dark;
        // M50: `components:` -- default theme first, then custom
        // theme's own overrides layered on top (custom wins on any
        // overlapping key, a plain `HashMap::extend`). Replaces (not
        // merges with) whatever a *previous* `set_theme` call may have
        // set, matching `state.theme`/`state.dark` right above -- each
        // `set_theme` call is a complete, fresh theme selection, not
        // an incremental patch onto the last one. (M61: built and
        // resolved earlier, before any `state` mutation began.)
        state.components = resolved_components;
        state.typography = typography;
        let tint = state.on_surface();
        let mut tree = self.tree.borrow_mut();
        tree.set_all_interaction_tints(tint);
        // M20 Phase 1 (§7.1, §7.3): the real, deliberate scope choice
        // -- reuses this exact same already-resolved "on-surface" tint
        // rather than resolving a second, more specific MD3 role per
        // component.
        tree.set_all_component_tints(tint);
        // M52 Phase 1 (§7.1, §7.3): replay every registered `Retithe
        // Hook` now that `state` reflects the newly-installed theme --
        // each hook recomputes its own component's real container/
        // label color and corner_radius/elevation from scratch (the
        // same `resolve_*_colors`/`theme.shape`/`theme.elevation` calls
        // its own `add_*` factory made at construction time) and
        // overwrites in place, snapping any in-flight animation --
        // purely additive alongside the two tint pushes above, which
        // stay exactly as they were (unchanged, zero regression risk;
        // they're still the only mechanism for a plain node that opted
        // into `InteractionState` via `Node.enable_interaction()`
        // directly, never built by a themed factory).
        for hook in self.retheme_hooks.borrow().iter() {
            hook(&state, &mut tree);
        }
        drop(state);
        Ok(())
    }

    /// M71 (§7.1, §8): read-only access to this window's own live theme
    /// resolution -- `role`/`is_set`/`shape`/`elevation`/`typography`,
    /// the exact same lookups `window_factory.rs`'s own composition-only
    /// factories already make internally, now real and reachable from
    /// Python. A fresh `Theme` wrapper each access (a cheap `Rc` clone,
    /// see `Theme`'s own doc comment) -- reads always see this window's
    /// current live state, including after a real `set_theme()` call.
    #[getter]
    fn theme(&self) -> Theme {
        Theme {
            state: self.theme.clone(),
        }
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
        for (handler, _wants_event) in self.handlers.borrow().values() {
            visit.call(handler)?;
        }
        // M9 Phase 2: `completions` holds real `Py<PyAny>` callbacks --
        // the same cyclic-GC obligation as `handlers`.
        for callback in self.completions.borrow().callbacks.values() {
            visit.call(callback)?;
        }
        // M94: window listeners are stored callbacks too.
        for (handler, _wants_event) in self.window_listeners.borrow().values() {
            visit.call(handler)?;
        }
        // M42 Phase 2: after a real `show_view` switch, `self.active`'s
        // own `handlers` can be a *different* `HandlerMap` than
        // `self.handlers` above (the newly-shown `View`'s own) -- its
        // callbacks are already reachable via that `View`'s own
        // `__traverse__` too, but only for as long as that `View`'s own
        // Python wrapper object stays alive. If it doesn't (a real app
        // dropped its own reference after switching away), this Rust-
        // level `Rc` clone is still the only thing keeping those
        // callbacks alive -- traversing it directly here closes that
        // real, if narrow, gap rather than leaving a cycle CPython's own
        // collector could never find.
        //
        // **Real, load-bearing bug caught by this crate's own existing
        // `test_window_participates_in_cyclic_gc_when_a_click_handler_
        // captures_it_back` regression test, not found by inspection
        // alone:** for an ordinary `Window` that never called `show_
        // view` (the common case, `active.handlers` still the *same*
        // `Rc` as `self.handlers` above), an unconditional second loop
        // here calls `visit.call` on the identical `Py<PyAny>` object a
        // second time within this same `tp_traverse` invocation.
        // CPython's cyclic collector counts each `visit.call` as one
        // real outgoing reference when subtracting internal refs from
        // an object's total refcount -- reporting the same real,
        // single reference twice makes a genuine cycle look like it
        // still has an external referent, so it survives collection
        // (confirmed: this exact regression test started failing before
        // this `Rc::ptr_eq` guard was added). Skipped entirely unless
        // `active`'s handlers are genuinely a *different* map.
        let active_handlers = self.active.borrow().handlers.clone();
        if !Rc::ptr_eq(&self.handlers, &active_handlers) {
            for (handler, _wants_event) in active_handlers.borrow().values() {
                visit.call(handler)?;
            }
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        self.materializers.borrow_mut().clear();
        self.canvas_draws.borrow_mut().clear();
        self.handlers.borrow_mut().clear();
        self.completions.borrow_mut().callbacks.clear();
        self.active.borrow().handlers.borrow_mut().clear();
        self.window_listeners.borrow_mut().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with(components: &[(&str, ResolvedComponentOverride)]) -> ThemeState {
        ThemeState {
            components: components
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn shape_with_no_override_at_all_returns_none() {
        let state = ThemeState::default();
        assert_eq!(state.shape("card", None), None);
        assert_eq!(state.shape("fab", Some("small")), None);
    }

    #[test]
    fn shape_reads_the_bare_component_key_when_no_variant_is_given() {
        let state = state_with(&[(
            "card",
            ResolvedComponentOverride {
                corner_radius: Some(16.0),
                elevation: None,
            },
        )]);
        assert_eq!(state.shape("card", None), Some(16.0));
    }

    #[test]
    fn shape_prefers_the_variant_specific_key_over_the_bare_one() {
        let state = state_with(&[
            (
                "fab",
                ResolvedComponentOverride {
                    corner_radius: Some(16.0),
                    elevation: None,
                },
            ),
            (
                "fab.small",
                ResolvedComponentOverride {
                    corner_radius: Some(12.0),
                    elevation: None,
                },
            ),
        ]);
        assert_eq!(state.shape("fab", Some("small")), Some(12.0));
        // A variant not named by any override falls back to the bare key.
        assert_eq!(state.shape("fab", Some("large")), Some(16.0));
    }

    // --- M63 (§7.1, §16.3): typography ---

    /// Unlike `shape`/`elevation` above, `typography` never needs an
    /// external `.unwrap_or(SHIPPED_CONST)` -- `engine_md3::type_style_
    /// named` itself already *is* the real, correct un-themed default,
    /// with or without a real theme ever being set at all.
    #[test]
    fn typography_with_no_override_returns_the_real_shipped_default() {
        let state = ThemeState::default();
        let resolved = state.typography("label_large").unwrap();
        let expected = engine_md3::type_style_named("label_large").unwrap();
        assert_eq!(resolved.font_family, expected.font_family);
        assert_eq!(resolved.font_weight, expected.font_weight);
        assert_eq!(resolved.font_size, expected.font_size);
        assert_eq!(resolved.line_height, expected.line_height);
    }

    #[test]
    fn typography_with_an_unrecognized_role_returns_none() {
        let state = ThemeState::default();
        assert_eq!(state.typography("subtitle_huge"), None);
    }

    /// The actual `ThemeSpec.typography` end-to-end claim: a real
    /// per-field override must win over the role's own shipped default,
    /// and every field the override leaves unset must still come from
    /// the shipped default -- the identical per-field cascade `engine-
    /// spec::build.rs::resolve_text_style` already establishes for the
    /// declarative surface, proven here for the imperative one.
    #[test]
    fn typography_applies_a_real_per_field_override_on_top_of_the_shipped_default() {
        let mut overrides = HashMap::new();
        overrides.insert(
            "label_large".to_string(),
            engine_spec::TypographyOverride {
                font_family: Some("Inter".to_string()),
                font_weight: None,
                font_size: Some(20.0),
                line_height: None,
            },
        );
        let state = ThemeState::for_test_with_typography(
            Color::from_rgba8(0x67, 0x50, 0xA4, 0xFF),
            overrides,
        );
        let resolved = state.typography("label_large").unwrap();
        let expected = engine_md3::type_style_named("label_large").unwrap();
        assert_eq!(resolved.font_family, "Inter", "the override must win");
        assert_eq!(
            resolved.font_weight, expected.font_weight,
            "a field the override leaves unset must still come from the shipped default"
        );
        assert_eq!(resolved.font_size, 20.0, "the override must win");
        assert_eq!(resolved.line_height, expected.line_height);
    }

    /// The real bug caught before this shipped: a variant-specific
    /// entry that sets only `elevation` must not block the *bare* key's
    /// own `corner_radius` -- each field is looked up independently,
    /// not "does a variant entry exist at all."
    #[test]
    fn a_variant_entry_setting_only_elevation_does_not_shadow_the_bare_keys_corner_radius() {
        let state = state_with(&[
            (
                "card",
                ResolvedComponentOverride {
                    corner_radius: Some(16.0),
                    elevation: None,
                },
            ),
            (
                "card.elevated",
                ResolvedComponentOverride {
                    corner_radius: None,
                    elevation: Some(2.0),
                },
            ),
        ]);
        assert_eq!(state.shape("card", Some("elevated")), Some(16.0));
        assert_eq!(state.elevation("card", Some("elevated")), Some(2.0));
    }

    #[test]
    fn elevation_lookup_mirrors_shapes_own_precedence() {
        let state = state_with(&[
            (
                "button",
                ResolvedComponentOverride {
                    corner_radius: None,
                    elevation: Some(0.0),
                },
            ),
            (
                "button.elevated",
                ResolvedComponentOverride {
                    corner_radius: None,
                    elevation: Some(1.0),
                },
            ),
        ]);
        assert_eq!(state.elevation("button", Some("elevated")), Some(1.0));
        assert_eq!(state.elevation("button", Some("filled")), Some(0.0));
        assert_eq!(state.elevation("button", None), Some(0.0));
    }

    /// M52 Phase 1: the `RetitheHook` mechanism itself -- GIL-free,
    /// no `PyWindow`/pyo3 involved, since `Box<dyn Fn(&ThemeState, &mut
    /// Tree)>` closures over plain Rust values need none. Confirms two
    /// registered hooks both fire, in registration order, and each
    /// writes only its own captured `NodeId`.
    #[test]
    fn every_registered_hook_fires_and_writes_only_its_own_node() {
        let mut tree = Tree::new();
        let a = tree.insert(
            NodeKind::Rect,
            Style::default(),
            PaintProperties::new(Color::TRANSPARENT, 0.0, 0.0, 1.0),
        );
        let b = tree.insert(
            NodeKind::Rect,
            Style::default(),
            PaintProperties::new(Color::TRANSPARENT, 0.0, 0.0, 1.0),
        );
        let hooks: Vec<RetitheHook> = vec![
            Box::new(move |_theme, tree| {
                if let Some(node) = tree.get_mut(a) {
                    node.paint.corner_radius = engine_core::Animated::new(4.0);
                }
            }),
            Box::new(move |_theme, tree| {
                if let Some(node) = tree.get_mut(b) {
                    node.paint.corner_radius = engine_core::Animated::new(8.0);
                }
            }),
        ];
        let theme = ThemeState::default();
        for hook in &hooks {
            hook(&theme, &mut tree);
        }
        assert_eq!(tree.get(a).unwrap().paint.corner_radius.current, 4.0);
        assert_eq!(tree.get(b).unwrap().paint.corner_radius.current, 8.0);
    }

    /// A hook that captured a `NodeId` whose node was since removed
    /// (`Node.remove()`, real, ordinary usage) must be a safe, silent
    /// no-op on replay -- the same "stale side-table entry is harmless"
    /// contract `handlers`/`materializers`/`context_menus` already have,
    /// extended to this new side table, not a new regression class.
    #[test]
    fn a_hook_whose_node_was_since_removed_is_a_safe_no_op() {
        let mut tree = Tree::new();
        let removed = tree.insert(
            NodeKind::Rect,
            Style::default(),
            PaintProperties::new(Color::TRANSPARENT, 0.0, 0.0, 1.0),
        );
        tree.remove(removed);
        let hook: RetitheHook = Box::new(move |_theme, tree| {
            if let Some(node) = tree.get_mut(removed) {
                node.paint.corner_radius = engine_core::Animated::new(99.0);
            }
        });
        let theme = ThemeState::default();
        hook(&theme, &mut tree);
        assert!(
            tree.get(removed).is_none(),
            "the removed node must still be gone -- the hook must not have panicked or resurrected it"
        );
    }
}
