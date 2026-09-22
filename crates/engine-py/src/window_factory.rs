//! `PyWindow`'s node-factory methods (review follow-through, M28 Phase
//! 2, §4/§8): every `add_*`/`build_shell` method that creates a new
//! node and hands back a real `Node` wrapping it. Split out of
//! `window.rs` itself, which used to hold this together with synthetic
//! input dispatch, docking delegation, and virtual-list/canvas
//! plumbing -- four largely independent responsibilities the review's
//! own architecture lens flagged as sharing one ~1700-line file only
//! because that's where each was added at the time, not by design.
//! `PyWindow`'s real `#[pymethods]` now spans this file plus `window.rs`
//! (construction/theming/GC), `window_input.rs`, `window_docking.rs`,
//! and `window_virtual_canvas.rs` -- enabled by pyo3's own
//! `multiple-pymethods` feature (`Cargo.toml`), no behavior change.

use std::rc::Rc;

use engine_core::{
    AccessNodeData, Action, Animated, CheckboxState, CircularProgressState, ContentFit, IconState,
    ImageState, LinearProgressState, LoadingIndicatorState, NodeId, NodeKind, OverlayMeta,
    PaintProperties, RadioButtonState, Role, ShapeKey, SliderState, SplitterState, SwitchState,
    TextAlign, TextFieldState, TextState, TimePickerDialState, Tree,
};
use engine_render::{MONOSPACE_FONT_FAMILY, TextRenderer};
use peniko::Color;
use peniko::kurbo::{Affine, RoundedRect, Shape};
use pyo3::prelude::*;
use taffy::prelude::{
    AlignItems, JustifyContent, Position, Rect as TaffyRect, Size, Style, auto, length, zero,
};

use crate::error::EngineError;
use crate::node::Node;
use crate::terminal::TerminalSession;
use crate::window::{PyWindow, positioned_style};

/// M30 Phase 1 Step 3 (§5, §7): `add_icon`'s own real curated-icon-
/// name-to-`BezPath` lookup (`engine_md3::icons::path_for`), factored
/// out once it gained a second real caller (`add_icon_button`) and a
/// third (`add_fab`/`add_extended_fab`, this step) -- the same real
/// "duplicated at 2+ call sites, worth a shared helper" threshold
/// `positioned_style`/`wrap_node` already established in this file and
/// `window.rs` respectively, not a new convention invented here.
fn resolve_icon_path(name: &str) -> PyResult<peniko::kurbo::BezPath> {
    let d = engine_md3::icons::path_for(name).ok_or_else(|| {
        let known: Vec<&str> = engine_md3::icons::names().collect();
        pyo3::exceptions::PyValueError::new_err(format!(
            "unknown icon {name:?} -- expected one of {known:?}"
        ))
    })?;
    Ok(peniko::kurbo::BezPath::from_svg(d).unwrap_or_else(|e| {
        panic!("engine_md3::icons's own curated path data for {name:?} must parse: {e}")
    }))
}

/// M22 Phase 2 (§16.1): `Window.add_image`'s own real `fit:` string
/// vocabulary -- `parse_dock_side`'s own established pattern
/// (`dock.rs`), applied to `ContentFit`'s three real variants.
fn parse_content_fit(fit: &str) -> PyResult<ContentFit> {
    match fit {
        "cover" => Ok(ContentFit::Cover),
        "contain" => Ok(ContentFit::Contain),
        "fill" => Ok(ContentFit::Fill),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown content fit {other:?} -- expected one of \"cover\", \"contain\", \"fill\""
        ))),
    }
}

/// M30 Phase 9 Step 5 (§5, §7, §11.7): `parse_content_fit`'s own real
/// shape, mirrored for MD3's three real carousel layouts.
fn parse_carousel_layout(layout: &str) -> PyResult<engine_core::CarouselLayout> {
    match layout {
        "uncontained" => Ok(engine_core::CarouselLayout::Uncontained),
        "hero" => Ok(engine_core::CarouselLayout::Hero),
        "multi_browse" => Ok(engine_core::CarouselLayout::MultiBrowse),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown carousel layout {other:?} -- expected one of \"uncontained\", \"hero\", \"multi_browse\""
        ))),
    }
}

/// M30 Phase 1 (§5, §7): a fully transparent fill -- `Rect`'s own real
/// "paint nothing" value (`border_paint.rs`'s own proof that `alpha:
/// 0` genuinely paints no pixels applies identically to `background`),
/// used by `Outlined`/`Text`'s real MD3 anatomy: neither variant has a
/// filled container at all, only `Outlined`'s real 1dp stroke or (for
/// `Text`) nothing but the label itself.
const TRANSPARENT: Color = Color::from_rgba8(0, 0, 0, 0);

/// M30 Phase 1 (§5, §7): real Material 3 baseline-scheme hex values
/// (the same published baseline seed-color tokens Compose Material3's
/// own default theme ships), used as every `Button`/`FAB` variant's
/// real un-themed fallback -- the identical "real historical default,
/// not black" contract `CheckboxState`'s white mark / `SliderState`'s
/// gray track already establish for a `Window` that never calls
/// `set_theme`. Resolved role by role, not just "the whole light
/// `ColorScheme`", since a themed `Window` resolves through
/// `ThemeState::role` instead the moment `is_set()` is true. Named
/// `Md3Baseline`, not `ButtonBaseline` -- widened at Step 3 (`FAB`)
/// beyond `Button`'s own original subset, so the name no longer
/// pointed at just one consumer.
struct Md3Baseline;
impl Md3Baseline {
    const PRIMARY: Color = Color::from_rgba8(0x67, 0x50, 0xA4, 0xFF);
    const ON_PRIMARY: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF);
    const PRIMARY_CONTAINER: Color = Color::from_rgba8(0xEA, 0xDD, 0xFF, 0xFF);
    const ON_PRIMARY_CONTAINER: Color = Color::from_rgba8(0x21, 0x00, 0x5D, 0xFF);
    const SECONDARY_CONTAINER: Color = Color::from_rgba8(0xE8, 0xDE, 0xF8, 0xFF);
    const ON_SECONDARY_CONTAINER: Color = Color::from_rgba8(0x1D, 0x19, 0x2B, 0xFF);
    const TERTIARY_CONTAINER: Color = Color::from_rgba8(0xFF, 0xD8, 0xE4, 0xFF);
    const ON_TERTIARY_CONTAINER: Color = Color::from_rgba8(0x31, 0x11, 0x1D, 0xFF);
    const SURFACE_CONTAINER: Color = Color::from_rgba8(0xF3, 0xED, 0xF7, 0xFF);
    const SURFACE_CONTAINER_LOW: Color = Color::from_rgba8(0xF7, 0xF2, 0xFA, 0xFF);
    const SURFACE_CONTAINER_HIGH: Color = Color::from_rgba8(0xEC, 0xE6, 0xF0, 0xFF);
    const SURFACE_CONTAINER_HIGHEST: Color = Color::from_rgba8(0xE6, 0xE0, 0xE9, 0xFF);
    const OUTLINE: Color = Color::from_rgba8(0x79, 0x74, 0x7E, 0xFF);
    const OUTLINE_VARIANT: Color = Color::from_rgba8(0xCA, 0xC4, 0xD0, 0xFF);
    const INVERSE_SURFACE: Color = Color::from_rgba8(0x31, 0x30, 0x33, 0xFF);
    const INVERSE_ON_SURFACE: Color = Color::from_rgba8(0xF4, 0xEF, 0xF4, 0xFF);
    /// M30 Phase 4 Step 2 (§7): real, confirmed via Material Web's own
    /// token chain, not guessed -- `_md-sys-color.scss`'s own
    /// `values-light()` maps `inverse-primary` to `md-ref-palette`'s
    /// `primary80` tone, and `_md-ref-palette.scss` gives `primary80`
    /// as `#D0BCFF` for the real M3 baseline seed (`primary40` =
    /// `#6750A4`, matching this file's own `PRIMARY` above).
    const INVERSE_PRIMARY: Color = Color::from_rgba8(0xD0, 0xBC, 0xFF, 0xFF);
    const SCRIM: Color = Color::from_rgba8(0x00, 0x00, 0x00, 0xFF);
    const SURFACE: Color = Color::from_rgba8(0xFF, 0xFB, 0xFE, 0xFF);
    const ON_SURFACE_VARIANT: Color = Color::from_rgba8(0x49, 0x45, 0x4F, 0xFF);
    const ERROR: Color = Color::from_rgba8(0xB3, 0x26, 0x1E, 0xFF);
    const ON_ERROR: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF);
}

/// M30 Phase 1 (§5, §7): the real per-variant paint this button's own
/// container/label/(optional) border resolve to -- `elevation` reuses
/// `PaintProperties.elevation`'s own already-real drop-shadow (verified
/// against Material Web's own `_elevation.scss` formula, §14 step 3),
/// not a fake flat highlight; `Elevated`'s real MD3 rest-state level is
/// 1 (1dp), every other variant's rest state is level 0.
struct ButtonColors {
    container: Color,
    label: Color,
    border_color: Color,
    border_width: f64,
    elevation: f64,
}

/// M30 Phase 1 (§5, §7): resolves `Window.add_button`'s real `variant:`
/// string against MD3's five real button variants (Elevated/Filled/
/// FilledTonal/Outlined/Text -- this catalog's own §5 scope), reading
/// through `theme` exactly the way `add_checkbox`/`add_slider` already
/// gate on `theme.is_set()` before ever reading a role, so an un-themed
/// `Window` keeps painting `Md3Baseline`'s own real historical
/// default rather than silently going black.
/// M50 Phase 2: `component` names the real theme-override key this
/// call site's own elevation should be looked up under (`"button"` for
/// `add_button` itself, `"icon_button"` for `add_icon_button`, since
/// each real factory gets its own key even where several reuse this
/// one resolver -- `crates/engine-spec/src/theme.rs`'s own `components:`
/// doc comment). `theme.elevation(component, Some(variant))` is tried
/// first; the existing hardcoded per-variant literal survives as the
/// fallback, the identical "un-themed default survives untouched"
/// contract colors already established.
fn resolve_button_colors(
    theme: &crate::window::ThemeState,
    variant: &str,
    component: &str,
) -> PyResult<ButtonColors> {
    let role = |name: &str, fallback: Color| -> Color {
        if theme.is_set() {
            theme.role(name).unwrap_or(fallback)
        } else {
            fallback
        }
    };
    let elevation =
        |default: f64| -> f64 { theme.elevation(component, Some(variant)).unwrap_or(default) };
    match variant {
        "elevated" => Ok(ButtonColors {
            container: role("surface_container_low", Md3Baseline::SURFACE_CONTAINER_LOW),
            label: role("primary", Md3Baseline::PRIMARY),
            border_color: TRANSPARENT,
            border_width: 0.0,
            elevation: elevation(1.0),
        }),
        "filled" => Ok(ButtonColors {
            container: role("primary", Md3Baseline::PRIMARY),
            label: role("on_primary", Md3Baseline::ON_PRIMARY),
            border_color: TRANSPARENT,
            border_width: 0.0,
            elevation: elevation(0.0),
        }),
        "filled_tonal" => Ok(ButtonColors {
            container: role("secondary_container", Md3Baseline::SECONDARY_CONTAINER),
            label: role(
                "on_secondary_container",
                Md3Baseline::ON_SECONDARY_CONTAINER,
            ),
            border_color: TRANSPARENT,
            border_width: 0.0,
            elevation: elevation(0.0),
        }),
        "outlined" => Ok(ButtonColors {
            container: TRANSPARENT,
            label: role("primary", Md3Baseline::PRIMARY),
            border_color: role("outline", Md3Baseline::OUTLINE),
            border_width: 1.0,
            elevation: elevation(0.0),
        }),
        "text" => Ok(ButtonColors {
            container: TRANSPARENT,
            label: role("primary", Md3Baseline::PRIMARY),
            border_color: TRANSPARENT,
            border_width: 0.0,
            elevation: elevation(0.0),
        }),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown button variant {other:?} -- expected one of \"elevated\", \"filled\", \
             \"filled_tonal\", \"outlined\", \"text\""
        ))),
    }
}

/// M52 Phase 1 (§7.1, §7.3): `add_button`'s own real `RetitheHook`
/// builder -- the milestone's own proof-of-concept, chosen for being
/// the simplest real themed factory (fixed 2-node topology, a single
/// shared `resolve_button_colors` call, no custom `NodeKind` payload
/// field). Recomputes exactly what `add_button` itself computed at
/// construction time, from the same captured inputs, and snaps the
/// result into the container/label nodes in place -- `Ok`/`Err` from
/// `resolve_button_colors` is deliberately swallowed (a hook can't
/// return a `PyResult` to any caller, and an unknown `variant` string
/// was already validated the moment `add_button` first accepted it, so
/// this can only fail if something else already went wrong; a silent
/// no-op is the same safe fallback `tree.get_mut(id) -> None` already
/// gives for a stale `NodeId`). A named, standalone function (not an
/// inline closure written at the `add_button` call site) so it can be
/// unit-tested directly, GIL-free, without constructing a `PyWindow`.
fn button_retheme_hook(
    container: NodeId,
    label: NodeId,
    variant: String,
    height: f32,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let Ok(colors) = resolve_button_colors(theme, &variant, "button") else {
            return;
        };
        let corner_radius = theme
            .shape("button", Some(&variant))
            .unwrap_or_else(|| f64::from(height) / 2.0);
        if let Some(node) = tree.get_mut(container) {
            node.paint.background = Animated::new(colors.container);
            node.paint.corner_radius = Animated::new(corner_radius);
            node.paint.elevation = Animated::new(colors.elevation);
            node.paint.border_color = Animated::new(colors.border_color);
            node.paint.border_width = Animated::new(colors.border_width);
        }
        if let Some(node) = tree.get_mut(label) {
            node.paint.background = Animated::new(colors.label);
        }
    })
}

/// M30 Phase 1 Step 3 (§5, §7): `FAB`/`Extended FAB` share one real
/// color-variant system, distinct from `Button`'s own -- Surface (the
/// real MD3 default)/Primary/Secondary/Tertiary, not Elevated/Filled/
/// Filled Tonal/Outlined/Text. Verified against Material Web's own
/// real component token source (`tokens/versions/v0_192/_md-comp-fab-
/// surface.scss` and `_md-comp-fab-primary.scss`, the same reference
/// implementation this crate's `_elevation.scss` verification already
/// trusted) rather than assumed from memory: Surface resolves
/// `container` from `surface-container-high`/`icon` from `primary`;
/// Primary resolves `container` from `primary-container`/`icon` from
/// `on-primary-container`; Secondary/Tertiary follow the identical
/// `<name>-container`/`on-<name>-container` pattern MD3 uses
/// everywhere else in the spec (`Button`'s own Filled Tonal variant
/// included). `FAB` has no border/outline variant in real MD3 at all
/// -- `resolve_fab_colors` returns none, unlike `ButtonColors`.
struct FabColors {
    container: Color,
    icon: Color,
}

fn resolve_fab_colors(theme: &crate::window::ThemeState, variant: &str) -> PyResult<FabColors> {
    let role = |name: &str, fallback: Color| -> Color {
        if theme.is_set() {
            theme.role(name).unwrap_or(fallback)
        } else {
            fallback
        }
    };
    match variant {
        "surface" => Ok(FabColors {
            container: role(
                "surface_container_high",
                Md3Baseline::SURFACE_CONTAINER_HIGH,
            ),
            icon: role("primary", Md3Baseline::PRIMARY),
        }),
        "primary" => Ok(FabColors {
            container: role("primary_container", Md3Baseline::PRIMARY_CONTAINER),
            icon: role("on_primary_container", Md3Baseline::ON_PRIMARY_CONTAINER),
        }),
        "secondary" => Ok(FabColors {
            container: role("secondary_container", Md3Baseline::SECONDARY_CONTAINER),
            icon: role(
                "on_secondary_container",
                Md3Baseline::ON_SECONDARY_CONTAINER,
            ),
        }),
        "tertiary" => Ok(FabColors {
            container: role("tertiary_container", Md3Baseline::TERTIARY_CONTAINER),
            icon: role("on_tertiary_container", Md3Baseline::ON_TERTIARY_CONTAINER),
        }),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown FAB variant {other:?} -- expected one of \"surface\", \"primary\", \
             \"secondary\", \"tertiary\""
        ))),
    }
}

/// M30 Phase 1 Step 3 (§5, §7): `FAB`'s three real MD3 sizes -- each
/// pairs its own real container size with its own real, independently
/// specified shape-corner token (Small: 40dp container/`corner-
/// medium` 12dp; Default: 56dp/`corner-large` 16dp; Large: 96dp/
/// `corner-extra-large` 28dp) -- verified against Material Web's own
/// token source, not a single proportional formula guessed from one
/// data point (the three real ratios -- 12/40, 16/56, 28/96 -- are
/// close but not identical, so a formula would have been a fabricated
/// approximation, not real fidelity).
fn fab_shape(size: &str) -> PyResult<(f32, f32)> {
    match size {
        "small" => Ok((40.0, 12.0)),
        "default" => Ok((56.0, 16.0)),
        "large" => Ok((96.0, 28.0)),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown FAB size {other:?} -- expected one of \"small\", \"default\", \"large\""
        ))),
    }
}

/// MD3's own real FAB anatomy constants, verified against Material
/// Web's own token source: a real MD3 FAB is elevated at rest (level
/// 3 -- `PaintProperties.elevation`'s own real level-index scale, not
/// literal dp, matching `Button`'s own `Elevated` variant's real
/// rest-state level 1), and its icon is a fixed real 24dp token
/// regardless of which of the three real sizes is used.
const FAB_ICON_SIZE: f32 = 24.0;
const FAB_REST_ELEVATION_LEVEL: f64 = 3.0;

/// M30 Phase 2 Step 3 (§5, §7): `Chip`'s real per-variant paint,
/// verified against Material Web's own token source (`tokens/
/// versions/v0_192/_md-comp-assist-chip.scss`/`_md-comp-filter-chip.
/// scss`) rather than assumed. Real, deliberate design: unlike
/// `Button`/`FAB`, `Chip` is built as a plain composition (`Rect` +
/// optional leading `Icon` + `Text` + optional trailing `Icon`), not
/// a new first-class `NodeKind` -- the same real architectural line
/// `Segmented Button`'s own selection already draws: a component
/// whose "selected" meaning is fundamentally group/app state (Filter
/// Chip's own real toggle) stays a static composition the app re-
/// paints on demand, while a standalone single control (`Checkbox`/
/// `RadioButton`/`Switch`) gets engine-owned animated state. No new
/// `Tree::hit_test_at` fix needed here -- the earlier `NodeKind::
/// Text`/`NodeKind::Icon` arms already cover a chip's own children.
struct ChipColors {
    container: Color,
    border_color: Color,
    border_width: f64,
    label: Color,
    icon: Color,
}

/// Real, verified MD3 finding: Assist Chip's own label role is
/// `on_surface`, genuinely different from Filter/Input/Suggestion's
/// shared `on_surface_variant` -- not a typo, confirmed from the real
/// token file before writing this. Filter Chip's real *selected*
/// state is the only one with a filled container at all
/// (`secondary_container`/`on_secondary_container`, `Button`'s own
/// Filled Tonal pattern reused) -- every other variant (and Filter
/// itself when unselected) is transparent with a real 1dp `outline`
/// stroke.
fn resolve_chip_colors(
    theme: &crate::window::ThemeState,
    variant: &str,
    selected: bool,
) -> PyResult<ChipColors> {
    let role = |name: &str, fallback: Color| -> Color {
        if theme.is_set() {
            theme.role(name).unwrap_or(fallback)
        } else {
            fallback
        }
    };
    match variant {
        "assist" => Ok(ChipColors {
            container: TRANSPARENT,
            border_color: role("outline", Md3Baseline::OUTLINE),
            border_width: 1.0,
            label: theme.on_surface(),
            icon: role("primary", Md3Baseline::PRIMARY),
        }),
        "filter" if selected => Ok(ChipColors {
            container: role("secondary_container", Md3Baseline::SECONDARY_CONTAINER),
            border_color: TRANSPARENT,
            border_width: 0.0,
            label: role(
                "on_secondary_container",
                Md3Baseline::ON_SECONDARY_CONTAINER,
            ),
            icon: role(
                "on_secondary_container",
                Md3Baseline::ON_SECONDARY_CONTAINER,
            ),
        }),
        "filter" | "input" | "suggestion" => Ok(ChipColors {
            container: TRANSPARENT,
            border_color: role("outline", Md3Baseline::OUTLINE),
            border_width: 1.0,
            label: role("on_surface_variant", Md3Baseline::ON_SURFACE_VARIANT),
            icon: role("on_surface_variant", Md3Baseline::ON_SURFACE_VARIANT),
        }),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown chip variant {other:?} -- expected one of \"assist\", \"filter\", \
             \"input\", \"suggestion\""
        ))),
    }
}

/// MD3's own real Chip anatomy constants (M3 spec, Chips component
/// page): 32dp height, `corner-small` (8dp) -- genuinely not the
/// "Full" shape `Button`/`FAB`/`Icon Button` all use, confirmed from
/// the real token file, not assumed consistent. Icon (leading,
/// trailing, or the real selected-state checkmark) is a real 18dp
/// token, smaller than every other component's 24dp. **One real
/// number not found in the fetched token file, so not claimed as
/// independently re-verified:** the chip's own horizontal padding and
/// icon-label gap -- both real, reasonable MD3 values, stated
/// honestly rather than presented as verified against the same
/// primary source the others were.
const CHIP_HEIGHT: f32 = 32.0;
const CHIP_CORNER_RADIUS: f64 = 8.0;
const CHIP_ICON_SIZE: f32 = 18.0;
const CHIP_LEADING_PADDING_WITH_ICON: f32 = 8.0;
const CHIP_LEADING_PADDING_NO_ICON: f32 = 16.0;
const CHIP_TRAILING_PADDING_WITH_ICON: f32 = 8.0;
const CHIP_TRAILING_PADDING_NO_ICON: f32 = 16.0;
const CHIP_ICON_GAP: f32 = 8.0;

/// M30 Phase 2 Step 4 (§5, §7, §11.3): `Menu`'s real anatomy,
/// verified against Material Web's own token source. **A real,
/// confirmed finding, not assumed:** Material Web has no dedicated
/// `_md-comp-menu-item.scss` token file at all (a direct fetch 404s)
/// -- a real MD3 menu genuinely reuses the plain List Item's own
/// tokens for its rows (`_md-comp-list.scss`: 56dp height, 24dp
/// leading icon, 16dp leading space, `on-surface` label, `on-surface-
/// variant` icon), not a separate menu-specific row shape. The panel
/// itself has its own real tokens (`_md-comp-menu.scss`):
/// `surface-container` fill, `corner-extra-small` (4dp), a real
/// rest-state elevation (level 2). **One real number not found in
/// either fetched token file, so not claimed as independently
/// verified:** the icon-to-label gap within one row -- a real,
/// reasonable MD3 value, stated honestly.
const MENU_ITEM_HEIGHT: f32 = 56.0;
const MENU_ITEM_ICON_SIZE: f32 = 24.0;
const MENU_ITEM_LEADING_SPACE: f32 = 16.0;
const MENU_ITEM_ICON_GAP: f32 = 12.0;
const MENU_PANEL_CORNER_RADIUS: f64 = 4.0;
const MENU_PANEL_ELEVATION: f64 = 2.0;

/// M30 Phase 3 Step 1 (§5, §7): `Badge`'s real two real sizes,
/// verified against Material Web's own token source (`_md-comp-
/// badge.scss`): a real 6dp dot (no label at all) and a real 16dp
/// labeled pill, both `error`-filled, `corner-full`. The labeled
/// variant's own real type role is Label Small (11sp/500 weight,
/// MD3's own smallest label size) -- genuinely smaller than every
/// other component's Label Large (14sp) this catalog has used so
/// far, confirmed from MD3's own real type scale, not assumed the
/// same size fits.
const BADGE_DOT_SIZE: f32 = 6.0;
const BADGE_LABELED_HEIGHT: f32 = 16.0;
const BADGE_LABEL_FONT_SIZE: f32 = 11.0;
const BADGE_LABEL_FONT_WEIGHT: f32 = 500.0;

/// M30 Phase 3 Step 3 (§5, §7): `Card`'s real three MD3 variants,
/// verified against Material Web's own token source (`_md-comp-
/// elevated-card.scss`/`_md-comp-filled-card.scss`/`_md-comp-
/// outlined-card.scss`) before writing any code -- all three share
/// the identical real `corner-medium` shape (12dp), but differ in
/// container color/elevation/border exactly the way `Button`'s own
/// Elevated/Filled/Outlined variants do, the same real MD3 pattern
/// reused a second time at the container level. Outlined Card's real
/// border role is `outline_variant`, genuinely distinct from
/// `outline` (`Button`'s Outlined variant's own role) -- confirmed
/// from the real token file, not assumed the same role reused. `Card`
/// is a plain container, not a fixed anatomy -- the app populates it
/// with arbitrary children via the already-generic `Node.add_child`,
/// the same real "engine gives primitives, app composes content"
/// shape every other plain-container node in this codebase already
/// has (`add_rect` included).
struct CardColors {
    container: Color,
    border_color: Color,
    border_width: f64,
    elevation: f64,
}

/// M50 Phase 3: `elevation` consults `theme.elevation("card", Some(
/// variant))` first, the same real pattern `resolve_button_colors`
/// already established (`window_factory.rs`).
fn resolve_card_colors(theme: &crate::window::ThemeState, variant: &str) -> PyResult<CardColors> {
    let role = |name: &str, fallback: Color| -> Color {
        if theme.is_set() {
            theme.role(name).unwrap_or(fallback)
        } else {
            fallback
        }
    };
    let elevation =
        |default: f64| -> f64 { theme.elevation("card", Some(variant)).unwrap_or(default) };
    match variant {
        "elevated" => Ok(CardColors {
            container: role("surface_container_low", Md3Baseline::SURFACE_CONTAINER_LOW),
            border_color: TRANSPARENT,
            border_width: 0.0,
            elevation: elevation(1.0),
        }),
        "filled" => Ok(CardColors {
            container: role(
                "surface_container_highest",
                Md3Baseline::SURFACE_CONTAINER_HIGHEST,
            ),
            border_color: TRANSPARENT,
            border_width: 0.0,
            elevation: elevation(0.0),
        }),
        "outlined" => Ok(CardColors {
            container: role("surface", Md3Baseline::SURFACE),
            border_color: role("outline_variant", Md3Baseline::OUTLINE_VARIANT),
            border_width: 1.0,
            elevation: elevation(0.0),
        }),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown card variant {other:?} -- expected one of \"elevated\", \"filled\", \
             \"outlined\""
        ))),
    }
}

const CARD_CORNER_RADIUS: f64 = 12.0;

/// M30 Phase 3 Step 4 (§5, §7): `Divider`'s real anatomy, verified
/// against Material Web's own token source (`_md-comp-divider.scss`):
/// a real 1dp line, `outline_variant` -- the identical real role
/// `Card`'s own Outlined variant already resolves (`Md3Baseline::
/// OUTLINE_VARIANT`), reused here rather than a second lookup.
const DIVIDER_THICKNESS: f32 = 1.0;

/// M30 Phase 9 Step 2 (§5): `Node Graph`'s real anatomy -- no official
/// MD3 page exists, so not independently token-verified; designed
/// from the sibling `pyCopper` project's own real `NodeGraph` widget
/// (`NodeElement.TITLE_HEIGHT`/`PAD_X`), reused directly rather than
/// invented fresh.
const NODE_GRAPH_TITLE_HEIGHT: f32 = 32.0;
const NODE_GRAPH_TITLE_PADDING: f32 = 12.0;

/// M30 Phase 3 Step 5 (§5, §7, §11.3): `Tooltip`'s real (Plain
/// variant) anatomy, verified against Material Web's own token
/// source (`_md-comp-plain-tooltip.scss`) before writing any code:
/// `inverse_surface` fill, `inverse_on_surface` text, `corner-extra-
/// small` (4dp), Body Small's own real type role (12sp/400 weight --
/// genuinely a *body* role, not a *label* role like every interactive
/// component in this catalog has used so far, confirmed from MD3's
/// own real type scale). **One real number not found in the fetched
/// token file, so not claimed as independently verified:** the real
/// 24dp panel height and 8dp horizontal padding -- both real,
/// reasonable MD3 values, stated honestly rather than presented as
/// verified against the same primary source the others were.
const TOOLTIP_HEIGHT: f32 = 24.0;
const TOOLTIP_CORNER_RADIUS: f64 = 4.0;
const TOOLTIP_HORIZONTAL_PADDING: f32 = 8.0;
const TOOLTIP_FONT_SIZE: f32 = 12.0;
const TOOLTIP_FONT_WEIGHT: f32 = 400.0;

/// M30 Phase 4 Step 1 (§5, §7, §11.3): `Dialog`'s real anatomy,
/// verified against Material Web's own token source (`_md-comp-
/// dialog.scss`) before writing any code: `surface_container_high`
/// fill, `corner-extra-large` (28dp -- genuinely larger than every
/// other component's own corner radius in this catalog so far,
/// confirmed rather than assumed), a real rest-state elevation
/// (level 3, the highest real elevation any component in this
/// catalog has used). Headline uses Headline Small (24sp/400 weight)
/// on `on_surface`; supporting text uses Body Medium (14sp/400
/// weight) on `on_surface_variant` -- both real *body*/*headline*
/// type roles, not the *label* role every interactive component in
/// this catalog has used. **Two real numbers not found in the
/// fetched token file, so not claimed as independently verified:**
/// the real 24dp panel padding and 16dp headline-to-body gap -- both
/// real, reasonable MD3 values, stated honestly. The real 32% scrim
/// opacity is a well-established MD3 convention, applied through the
/// already-real `PaintProperties.opacity` field -- no new paint
/// capability needed for it.
const DIALOG_CORNER_RADIUS: f64 = 28.0;
const DIALOG_ELEVATION: f64 = 3.0;
const DIALOG_PADDING: f32 = 24.0;
const DIALOG_HEADLINE_GAP: f32 = 16.0;
const DIALOG_HEADLINE_FONT_SIZE: f32 = 24.0;
const DIALOG_HEADLINE_FONT_WEIGHT: f32 = 400.0;
const DIALOG_BODY_FONT_SIZE: f32 = 14.0;
const DIALOG_BODY_FONT_WEIGHT: f32 = 400.0;
const DIALOG_SCRIM_OPACITY: f64 = 0.32;

/// MD3's own real Snackbar anatomy (M30 Phase 4 Step 2), verified
/// against Material Web's own token source (`_md-comp-snackbar.scss`)
/// before writing any code: `with-single-line-container-height` (48dp,
/// no multi-line variant scoped by this step), `container-shape`
/// (`corner-extra-small`, 4dp), `container-elevation` (`level3`).
/// Horizontal padding/gap/action-width/desktop-anchor-margins have no
/// discrete token in that same file (confirmed by the same fetch) --
/// reasonable, MD3-consistent values, stated honestly, the identical
/// caveat `Dialog`'s own padding constants above already carry.
const SNACKBAR_HEIGHT: f32 = 48.0;
const SNACKBAR_CORNER_RADIUS: f64 = 4.0;
const SNACKBAR_ELEVATION: f64 = 3.0;
const SNACKBAR_HORIZONTAL_PADDING: f32 = 16.0;
const SNACKBAR_GAP: f32 = 8.0;
const SNACKBAR_ICON_SIZE: f32 = 24.0;
const SNACKBAR_ACTION_WIDTH: f32 = 64.0;
const SNACKBAR_BOTTOM_MARGIN: f32 = 24.0;
const SNACKBAR_LEFT_MARGIN: f32 = 24.0;

/// MD3's own real Side Sheet anatomy (M30 Phase 4 Step 3). **Real,
/// confirmed finding, not assumed:** Material Web has no dedicated
/// side-sheet token file at all (a direct fetch 404s, the identical
/// real finding `Menu`'s own token investigation already hit) --
/// these reuse `Navigation Drawer`'s own real tokens instead
/// (`_md-comp-navigation-drawer.scss`), the structurally closest real
/// MD3 component (an edge-docked elevated panel): real `Standard`
/// container color `surface`/elevation `level0` (flat, embedded in
/// the layout, never floating) vs. real `Modal` container color
/// `surface_container_low`/elevation `level1` (a real shadow, since
/// it floats over content); `container-shape` `corner-large-end`
/// (16dp, matching `FAB`'s own already-confirmed real `corner-large`
/// value, rounded only on the corners *away* from the docked edge);
/// real `container-width` (360px) and `container-height` (100%).
const SIDE_SHEET_WIDTH: f32 = 360.0;
const SIDE_SHEET_CORNER_RADIUS: f64 = 16.0;
const SIDE_SHEET_STANDARD_ELEVATION: f64 = 0.0;
const SIDE_SHEET_MODAL_ELEVATION: f64 = 1.0;

/// MD3's own real Navigation Rail anatomy (M30 Phase 5 Step 1),
/// verified against Material Web's own token source before writing
/// any code (`_md-comp-navigation-rail.scss`): `container-color`
/// `surface` (a real, confirmed re-fetch correction -- a first pass
/// misattributed the active-indicator's own `secondary_container`
/// color to the rail's container instead, caught by asking the
/// second fetch for the container-color line verbatim, not assumed
/// from the first summary), `container-width` 80px,
/// `container-elevation` `level0` (flat, no shadow -- a rail is
/// always docked flush to the screen edge, never floating),
/// `container-shape` `corner-none` (genuinely square, the first
/// component in this whole catalog with zero rounding anywhere).
/// Active indicator: `secondary_container` fill, 56×32dp,
/// `corner-full` (a real pill, `height / 2.0`). Icon: 24dp,
/// `on_secondary_container` active / `on_surface_variant` inactive.
/// Label: Label Medium (12sp, traced through `_md-sys-typescale.scss`
/// to `_md-ref-typeface.scss` for the real numeric weights --
/// `weight-medium` = 500 inactive, `weight-bold` = 700 for the
/// active/"prominent" variant, a real, genuine *weight* difference
/// between active/inactive, not just a color change like every prior
/// component in this catalog), `on_surface` active / `on_surface_
/// variant` inactive.
const NAV_RAIL_WIDTH: f32 = 80.0;
const NAV_RAIL_ICON_SIZE: f32 = 24.0;
const NAV_RAIL_INDICATOR_WIDTH: f32 = 56.0;
const NAV_RAIL_INDICATOR_HEIGHT: f32 = 32.0;
const NAV_RAIL_INDICATOR_CORNER_RADIUS: f64 = NAV_RAIL_INDICATOR_HEIGHT as f64 / 2.0;
const NAV_RAIL_LABEL_FONT_SIZE: f32 = 12.0;
const NAV_RAIL_LABEL_WEIGHT_INACTIVE: f32 = 500.0;
const NAV_RAIL_LABEL_WEIGHT_ACTIVE: f32 = 700.0;
/// Not a discrete token in the rail's own token file (confirmed by
/// the same fetch) -- reasonable, MD3-consistent values, the
/// identical honest caveat `Dialog`'s own padding constants carry.
const NAV_RAIL_ITEM_GAP: f32 = 4.0;
const NAV_RAIL_ITEM_SPACING: f32 = 12.0;
const NAV_RAIL_TOP_PADDING: f32 = 44.0;

/// MD3's own real Navigation Drawer item/destination anatomy (M30
/// Phase 5 Step 2), verified against Material Web's own token source
/// (`_md-comp-navigation-drawer.scss`) before writing any code --
/// the container tokens themselves are identical to `Side Sheet`'s
/// own (`SIDE_SHEET_*`, reused directly below, both real values come
/// from this same source file, confirmed by direct re-fetch, not
/// assumed to still match from memory) since Material Web has no
/// separate side-sheet token file at all (Phase 4 Step 3's own real
/// finding). Real per-item anatomy: active indicator 336×56dp,
/// `corner-full` (28dp), `secondary_container` fill; active icon/
/// label `on_secondary_container`, inactive `on_surface_variant`;
/// label Label Large (14sp) with the identical real active/inactive
/// weight pair `Navigation Rail` already found (500/`weight-medium`
/// inactive, 700/`weight-bold` active -- `NAV_RAIL_LABEL_WEIGHT_*`
/// reused directly, not re-declared). Leading space/icon gap reuse
/// `Menu`'s own already-verified List Item tokens (`MENU_ITEM_*`) --
/// Navigation Drawer's real item height (56dp) matches List Item's
/// own real height exactly, a real, confirmed coincidence worth
/// reusing rather than re-declaring duplicate constants for the
/// identical real MD3 value.
const NAV_DRAWER_INDICATOR_WIDTH: f32 = 336.0;
const NAV_DRAWER_INDICATOR_CORNER_RADIUS: f64 = MENU_ITEM_HEIGHT as f64 / 2.0;
/// The indicator's own real 336dp width centers itself inside the
/// panel's real 360dp width via plain flex `align_items: CENTER` --
/// `(360 - 336) / 2.0 = 12.0dp` on each side, a real, derived value,
/// not a discrete token, needing no explicit margin constant of its
/// own to express.
/// Not discrete tokens in the drawer's own token file (confirmed by
/// the same fetch) -- reasonable, MD3-consistent values, the
/// identical honest caveat `Dialog`'s own padding constants carry.
const NAV_DRAWER_ITEM_SPACING: f32 = 4.0;
const NAV_DRAWER_TOP_PADDING: f32 = 12.0;

/// MD3's own real Top App Bar anatomy (M30 Phase 5 Step 3), the
/// *Small* variant -- verified against Material Web's own token
/// source before writing any code. **Real, confirmed finding: this
/// component's real token file isn't named the way every prior
/// component's was** -- `_md-comp-top-app-bar.scss` 404s; MD3's four
/// real variants (Small/Medium/Large/Small-Centered) each get their
/// own separate file (`_md-comp-top-app-bar-small.scss`, confirmed
/// via a real GitHub directory listing before guessing a filename a
/// second time), the same real per-variant-file shape `Chip`'s own
/// Assist/Filter split already established. Real values: `surface`
/// fill, `level0` elevation (flat, matching every other docked-chrome
/// component this catalog has found), 64dp height. Headline: Title
/// Large (22sp/400 weight -- traced through `_md-sys-typescale.scss`
/// into `_md-ref-typeface.scss`'s real `weight-regular` = 400,
/// `title-large-size` = `1.375rem` = 22px), `on_surface`. Leading
/// icon 24dp `on_surface`; trailing icon(s) 24dp `on_surface_variant`
/// -- a real, confirmed asymmetry (leading uses the plain `on_surface`
/// role, trailing the variant), not assumed identical.
const TOP_APP_BAR_HEIGHT: f32 = 64.0;
const TOP_APP_BAR_ICON_SIZE: f32 = 24.0;
const TOP_APP_BAR_HEADLINE_FONT_SIZE: f32 = 22.0;
const TOP_APP_BAR_HEADLINE_FONT_WEIGHT: f32 = 400.0;
/// Not discrete tokens in the small-variant's own token file
/// (confirmed by the same fetch) -- reasonable, MD3-consistent
/// values, the identical honest caveat `Dialog`'s own padding
/// constants carry.
const TOP_APP_BAR_ICON_BUTTON_SIZE: f32 = 40.0;
const TOP_APP_BAR_HORIZONTAL_PADDING: f32 = 4.0;
const TOP_APP_BAR_HEADLINE_START_PADDING: f32 = 16.0;
const TOP_APP_BAR_TRAILING_ICON_GAP: f32 = 8.0;

/// M35 Phase 1 (§5, §7): real MD3 Toolbar anatomy (`add_toolbar`'s own
/// doc comment has the full real verification/scope note) --
/// `COMPONENT_TOOLBARS.md`'s own "By default all toolbars are 64dp
/// high" applies to both the docked and floating variants alike.
/// `TOOLBAR_PADDING` is the spec's own real "minimum outside padding
/// of 16dp"; `TOOLBAR_ITEM_GAP` its own real "equal padding between
/// items" default (32dp) -- both apply identically to either
/// orientation's own main axis (this constant flips onto whichever
/// `taffy::FlexDirection` `add_toolbar` picks).
const TOOLBAR_HEIGHT: f32 = 64.0;
const TOOLBAR_PADDING: f32 = 16.0;
const TOOLBAR_ITEM_GAP: f32 = 32.0;

/// M35 Phase 2 (§5, §7): real MD3 Split Button tokens, the `xsmall`
/// size class (`COMPONENT_SPLIT_BUTTONS.md`'s own scraped token
/// table: `md.comp.split-button.xsmall.between-space` = 2dp,
/// `...xsmall.trailing-button.icon.size` = 22dp) -- used as flat
/// constants regardless of the caller's own chosen `height`, the
/// identical real precedent `TOP_APP_BAR_ICON_SIZE` already
/// established for a fixed real MD3 token value that doesn't scale
/// with a caller-chosen container size.
const SPLIT_BUTTON_GAP: f32 = 2.0;
const SPLIT_BUTTON_TRAILING_ICON_SIZE: f32 = 22.0;
/// M38 Phase 4 (§5, §7): the real MD3 shape-scale "small" step -- the
/// exact real value confirmed via `material-components-android`'s own
/// `docs/components/ButtonGroup.md` ("8dp inner corners") for a
/// connected group's own inner corners while interacting, the closest
/// real documented anatomy to Split Button's own "the inner corners
/// change shape for hovered, focused, and pressed states"
/// (`COMPONENT_SPLIT_BUTTONS.md`) -- the M3 spec's own JS-rendered
/// split-button spec page carries no fetchable static value.
const SPLIT_BUTTON_INNER_CORNER_RADIUS: f64 = 8.0;

/// M35 Phase 3 (§5, §7): real MD3 Standard Button Group tokens
/// (`COMPONENT_BUTTON_GROUPS.md`'s own scraped "Standard button group
/// inner padding" measurements: 18dp XS, 12dp S, 8dp M/L/XL) --
/// `BUTTON_GROUP_GAP` uses the real M/L/XL shared value, the most
/// common real default. **Real, honestly-stated gap:** no discrete
/// numeric token for how much a pressed child *grows* by exists
/// anywhere in the scraped spec (only the qualitative "briefly changes
/// the width of itself and adjacent buttons") -- `BUTTON_GROUP_GROW`
/// is a real, reasonable, MD3-plausible value, the identical honest
/// caveat `TOP_APP_BAR_ICON_BUTTON_SIZE`'s own doc comment already
/// carries for an undocumented real constant.
const BUTTON_GROUP_GAP: f32 = 8.0;
const BUTTON_GROUP_GROW: f32 = 12.0;

/// M38 Phase 5 (§5, §7): real MD3 Expressive "buttons reshape as you
/// press them" -- a per-child press morph is genuinely distinct from
/// Split Button's own hover-driven *inner-corner* tightening (M38
/// Phase 4): every one of a pressed button's own four corners
/// uniformly tighten from the full pill down to a real, per-size
/// "square" radius, not just the two corners facing a neighbor.
/// **Real, cited value, the same honest-sourcing discipline
/// `BUTTON_GROUP_GROW`'s own doc comment already established for an
/// undocumented constant:** the M3 site's own buttons spec page is
/// JS-rendered (no fetchable static content, confirmed the same way
/// M38 Phase 4's own research already found for Split Button); the
/// real per-size pressed-corner-radius table instead comes from a
/// real, cited open-source MD3 Expressive button implementation
/// (`callstack/react-native-paper` PR #5097): 8dp for XS/S, 12dp for
/// M, 16dp for L/XL. This codebase has no discrete size-class
/// parameter (`add_button`'s own established convention: a literal
/// `height`, never a size enum, the identical real choice `add_
/// split_button`'s own doc comment already states) -- `button_group_
/// pressed_corner_radius` maps a literal `height` onto that same real
/// three-tier table instead, using MD3's own published button-size
/// scale (XS 32dp/S 36dp/M 40dp/L 48dp/XL 56dp) to place the real
/// tier boundaries at the honest midpoints between adjacent tiers.
fn button_group_pressed_corner_radius(height: f64) -> f64 {
    if height <= 38.0 {
        8.0
    } else if height <= 44.0 {
        12.0
    } else {
        16.0
    }
}

/// MD3's own real Tabs anatomy (M30 Phase 5 Step 4), the *Primary
/// Navigation Tab* variant -- verified against Material Web's own
/// token source before writing any code. **Real, confirmed finding:**
/// MD3 has exactly two real Tab variants, each with its own separate
/// token file (`_md-comp-primary-navigation-tab.scss`/`_md-comp-
/// secondary-navigation-tab.scss`, confirmed via a real directory
/// listing, the same discovery technique `Top App Bar` (Step 3) just
/// used) -- Primary is this step's real scope, Secondary deliberately
/// out of scope, matching this catalog's own "real per-variant
/// investigation, not one guessed formula" discipline. Real container:
/// `surface` fill, `corner-none`, `level0` elevation (flat, matching
/// every other docked-chrome component found so far), 48dp height.
/// Real active indicator: `primary` fill, 3dp height, real shape `(3px
/// 3px 0px 0px)` -- rounded only on its own top corners, confirmed
/// (not assumed symmetric like `corner-full`). Active label/icon:
/// `primary`; inactive: `on_surface_variant`. Label type role is Title
/// Small, **a real, confirmed numeric coincidence with Label Large
/// worth stating, not conflating:** `title-small-size` (0.875rem =
/// 14px) and `weight-medium` (500) are numerically identical to
/// `BUTTON_LABEL_FONT_SIZE`/`_WEIGHT`, but a genuinely distinct real
/// MD3 type role -- declared as its own constants below rather than
/// silently reusing a different role's, even though today's numbers
/// match. Icon (optional, "with-icon" token set): 24dp, same real
/// active/inactive color pair as the label.
const TAB_HEIGHT: f32 = 48.0;
const TAB_LABEL_FONT_SIZE: f32 = 14.0;
const TAB_LABEL_FONT_WEIGHT: f32 = 500.0;
const TAB_ICON_SIZE: f32 = 24.0;
const TAB_INDICATOR_HEIGHT: f32 = 3.0;
const TAB_INDICATOR_CORNER_RADIUS: f64 = 3.0;
/// Not a discrete token in the tab's own token file (confirmed by the
/// same fetch) -- a reasonable, MD3-consistent value, the identical
/// honest caveat `Dialog`'s own padding constants carry.
const TAB_ICON_LABEL_GAP: f32 = 2.0;

/// MD3's own real Search anatomy (M30 Phase 5 Step 5), closing
/// Phase 5's own component list -- verified against Material Web's
/// own token source before writing any code, confirmed via a real
/// GitHub directory listing that both real files exist exactly where
/// this step's own name already implies (`_md-comp-search-bar.scss`/
/// `_md-comp-search-view.scss`, the "bar and view" pairing this
/// step's own tracker text already names). Real Search Bar anatomy:
/// `surface_container_high` fill, real `corner-full` shape (56dp
/// height, matching `Snackbar`'s own already-real single-line height
/// pattern of "shape derived from height/2"), a real rest-state
/// elevation (level 3, matching `Menu`'s own panel and `Dialog`'s own
/// panel). Leading icon `on_surface`; trailing icon(s) `on_surface_
/// variant` -- a real, confirmed asymmetry, the same shape `Top App
/// Bar` already found. Input text is Body Large (16sp/400 weight,
/// traced through `_md-sys-typescale.scss` into `_md-ref-typeface.
/// scss`'s real `weight-regular` = 400), `on_surface`; the same real
/// role/type pair the placeholder/"supporting text" uses too, just
/// `on_surface_variant` instead. Real Search View anatomy (the real
/// *docked* variant -- MD3's own real *full-screen* variant is a
/// mobile pattern, excluded per this milestone's own desktop-
/// adaptation rule): `surface_container_high` (the identical real
/// role the bar itself uses), elevation level 3, real `corner-extra-
/// large` shape -- the identical real 28dp value `Dialog`'s own
/// `DIALOG_CORNER_RADIUS` already confirmed for the same real token,
/// reused directly rather than re-declared.
///
/// **Real, deliberate design reusing `TextField`'s own existing real
/// `NodeKind`, not a bare styled box:** the search bar's own input is
/// a genuine `NodeKind::TextField` (`add_text_field`'s own real
/// construction pattern mirrored inline, not called through --
/// established precedent throughout this file: every `add_*` method
/// builds its own nodes directly, none cross-call another factory
/// method, avoiding any re-entrant `self.tree.borrow_mut()` risk) --
/// the app gets every one of `TextField`'s already-real capabilities
/// (typing, focus, selection, IME) for free, not a re-implementation.
///
/// **Real, deliberate design reusing `Window.open_menu`/`close_menu`
/// directly, not new dedicated methods:** `add_search_view`'s own
/// returned panel is shown/hidden the identical real way `Tooltip`'s
/// own panel already is (Phase 3 Step 5's own real precedent,
/// deliberately not given its own `open_tooltip`/`close_tooltip`
/// pair) -- a real dropdown-below-anchor overlay is exactly what
/// `open_overlay`'s own original, simplest form already does, with
/// the search bar container itself as a real, natural anchor; no
/// synthetic anchor node needed this time, unlike every modal variant
/// this milestone built (`Dialog`/`Side Sheet`/`Navigation Drawer`).
const SEARCH_BAR_HEIGHT: f32 = 56.0;
const SEARCH_BAR_CORNER_RADIUS: f64 = SEARCH_BAR_HEIGHT as f64 / 2.0;
const SEARCH_BAR_ELEVATION: f64 = 3.0;
const SEARCH_INPUT_FONT_SIZE: f32 = 16.0;
const SEARCH_INPUT_FONT_WEIGHT: f32 = 400.0;
const SEARCH_VIEW_ELEVATION: f64 = 3.0;
/// Icon size (24dp) is the universal real MD3 icon token this whole
/// catalog already reuses everywhere -- the search bar's own token
/// file doesn't declare a separate discrete one (confirmed by the
/// same fetch). Padding/icon-button-size are likewise not discrete
/// tokens there -- reasonable, MD3-consistent values, the identical
/// honest caveat `Dialog`'s own padding constants carry.
const SEARCH_ICON_SIZE: f32 = 24.0;
const SEARCH_ICON_BUTTON_SIZE: f32 = 40.0;
const SEARCH_BAR_HORIZONTAL_PADDING: f32 = 4.0;
const SEARCH_TRAILING_ICON_GAP: f32 = 8.0;

/// MD3's own real List/ListItem anatomy (M30 Phase 6 Step 1). Real,
/// confirmed relationship to `VirtualList` (already real since M4/M8):
/// this is a plain, non-virtualized list for small real collections
/// -- `VirtualList` stays the real choice for large ones, not two
/// unrelated mechanisms, this step's own real scope note already
/// named this explicitly. Real per-item anatomy is List Item's own
/// token file, already investigated once (Phase 2 Step 4, `Menu`'s
/// own real finding that Material Web has no dedicated menu-item
/// token file and reuses List Item's directly) -- `MENU_ITEM_*`
/// reused verbatim here for the identical real reason, plus a real,
/// confirmed new finding this step made: the *two-line* variant (72dp,
/// vs. the already-known one-line 56dp) for a headline + real
/// supporting-text second line, Body Medium (`DIALOG_BODY_FONT_SIZE`/
/// `_WEIGHT` reused directly, the identical real role `Dialog`'s own
/// body text already uses), `on_surface_variant` -- the same real
/// role trailing supporting text (metadata) also uses, confirmed from
/// the same fetch. No divider token exists in the list's own file
/// (confirmed by the same fetch) -- an app composes one itself via
/// the already-real `add_divider` (`OUTLINE_VARIANT`) if wanted, the
/// same "engine gives primitives, app composes" contract every other
/// bare-container component in this catalog already has.
const LIST_ITEM_TWO_LINE_HEIGHT: f32 = 72.0;

/// `Accordion` (M30 Phase 6 Step 2). **Real, confirmed finding,
/// already established rather than re-derived here:** MD3 has no
/// official Accordion component page at all -- `BUILD_TRACKER.md`'s
/// own scope text already names this, confirmed by pyCopper's own
/// real prior research against the full M3 reference library;
/// grounded instead in the Lists guideline's own "expand and collapse
/// in a folder-like manner" text, the identical real grounding
/// pyCopper used. Real, deliberate scope: only the *header* (title +
/// expand/collapse chevron) is this step's own real new anatomy --
/// the collapsible content region has no distinctive MD3 styling of
/// its own, so the app composes it from any already-real container
/// (`add_rect`/`add_card`) and shows/hides it via the already-generic
/// `Node.add_child`/`Node.remove()`, the identical "engine gives
/// primitives, app composes" contract `Card` itself already has --
/// no dedicated `add_accordion_content` method invented for something
/// with zero real anatomy of its own. Header reuses List Item's own
/// real anatomy (`MENU_ITEM_*`) exactly -- the same real "no dedicated
/// token file, reuse List Item's" pattern `Menu`/`List` already
/// established.
///
/// **Real, confirmed engine limitation found and honestly worked
/// around, not silently assumed solved:** `Node.animate("transform",
/// ...)` only ever composes translate+scale (`extract_translate_
/// scale`'s own real, stated shape, M6 Phase 2) -- there is no real
/// rotation capability exposed to Python at all. A literal spinning
/// chevron is not buildable through the existing API. The real,
/// working substitute: `Affine::scale(-1.0)` (a uniform negative
/// scale, flipping both axes) is mathematically identical to a 180°
/// rotation for any point-symmetric glyph, and the curated `expand_
/// more` chevron (a plain V-shape, symmetric about its own center) is
/// exactly that -- so `Node.animate("transform", (0.0, 0.0, -1.0))`
/// on the real, independently-returned chevron `Node` genuinely does
/// flip it into a convincing "expanded" orientation, reusing scale
/// (already real) rather than needing a new rotation primitive.
const ACCORDION_CHEVRON_ICON: &str = "expand_more";

/// `Tree View` (M30 Phase 6 Step 3). **Real, confirmed grounding,
/// already established rather than re-derived here:** the identical
/// real "no official M3 component page, grounded in the Lists
/// guideline's own expand-and-collapse text" finding `Accordion`
/// already made, applied recursively -- `BUILD_TRACKER.md`'s own
/// scope text already names this exactly. Real, deliberate design:
/// a tree node's own row is `Accordion`'s own header anatomy again
/// (reusing the identical `MENU_ITEM_*`/`ACCORDION_CHEVRON_ICON`
/// constants, not re-declared), with one real, new addition a flat
/// accordion header never needed -- a per-node `depth: usize` real
/// left-indent, the one real, load-bearing difference "recursively"
/// actually means here. Real, honest design choice: `leaf: bool`
/// omits the chevron entirely for a childless node -- a leaf has
/// nothing to expand, matching real desktop file-browser convention,
/// not the app's own responsibility to fake with an invisible one.
const TREE_NODE_INDENT_WIDTH: f32 = 24.0;

/// `Date Picker`'s own real day-cell anatomy (M30 Phase 7 Step 1),
/// verified against Material Web's own token source before writing
/// any code (`_md-comp-date-picker-docked.scss` -- the real *docked*
/// variant, this milestone's own desktop-adaptation choice over the
/// mobile-oriented *modal* full dialog variant, the identical real
/// "docked over full-screen" precedent `Search View` already made).
/// **Real, deliberate scope, not a partial build:** only the day
/// *cell* is this step's own real new anatomy -- a real calendar
/// grid needs real date arithmetic (month lengths, weekday-of-month,
/// leap years), which is genuinely application logic with zero real
/// MD3-specific content, already trivially available via Python's own
/// `datetime`/`calendar` modules; no engine-owned calendar primitive
/// is invented for something that isn't actually a rendering/
/// interaction concern, the identical real "engine gives primitives,
/// app composes" contract every bare-container component in this
/// catalog already has (`Card`, `Accordion`'s own content region).
/// Real per-cell anatomy: 48×48dp, `corner-full` (24dp radius, a real
/// circle). Selected: `primary` fill, `on_primary` label. Today (not
/// selected): a real 1dp `primary` outline (`PaintProperties.
/// border_color`/`border_width`, already-real since Phase 1 Step 1),
/// `primary` label, no fill. Neither: no fill/border, `on_surface`
/// label -- `on_surface_variant` instead for a real day belonging to
/// an adjacent month (a real, confirmed distinct role, not assumed
/// identical to the plain unselected case). Label reuses Body Large's
/// own already-declared constants (`SEARCH_INPUT_FONT_SIZE`/
/// `_WEIGHT`, the identical real MD3 type role `Search Bar`'s own
/// input text already uses), not re-declared.
const DATE_CELL_SIZE: f32 = 48.0;
const DATE_CELL_CORNER_RADIUS: f64 = DATE_CELL_SIZE as f64 / 2.0;
const DATE_TODAY_OUTLINE_WIDTH: f64 = 1.0;

/// `Time Picker` (M30 Phase 7 Step 2), the real *Time Input* variant
/// -- verified against Material Web's own token source
/// (`_md-comp-time-input.scss`) before writing any code. **Real,
/// deliberate scope, not the full component:** MD3's other real Time
/// variant, the analog clock-face dial (`_md-comp-time-picker.scss`,
/// confirmed to exist via the same real directory listing `Date
/// Picker`'s own investigation already found), needs a genuinely new
/// engine-core capability this catalog doesn't have anywhere --
/// drag-to-angle gesture handling and converting a circular hit point
/// into a time value -- a real, separate, much larger undertaking
/// than any per-component investigation this milestone has done so
/// far; Time Input is the real, desktop-realistic scope this step
/// actually builds. Real field anatomy: 96×72dp, `surface_container_
/// highest` fill, real `corner-small` shape -- the identical real
/// 8dp value `Chip`'s own `CHIP_CORNER_RADIUS` already confirmed for
/// the same real token, reused directly. Label is Display Medium
/// (2.8125rem = 45px, `weight-regular` = 400, traced through
/// `_md-sys-typescale.scss`/`_md-ref-typeface.scss`), `on_surface` --
/// a real, genuinely large numeral display, distinct from every other
/// type role this catalog has used so far. **Real, deliberate reuse
/// of `TextField`'s own existing real `NodeKind`, the identical real
/// design `Search Bar` already established:** the hour/minute field
/// is a genuine `NodeKind::TextField`, not a bare styled box -- every
/// one of its already-real capabilities (typing, focus, selection)
/// work for free. Period selector (AM/PM): 52×72dp overall, split
/// into two real 52×36dp options stacked vertically, real `corner-
/// small` shape; selected: `tertiary_container` fill (a real, newly-
/// used-for-time role, `Md3Baseline::TERTIARY_CONTAINER`/`ON_
/// TERTIARY_CONTAINER` both already declared from an earlier
/// component); unselected: transparent fill, `on_surface` label, the
/// same real convention `Segmented Button`/`Chip` already established
/// for their own unselected states. **Real, honest caveat, not
/// independently token-verified:** the period-selector's own label
/// type role and any shared-outline-frame detail beyond its overall
/// container dimensions weren't present in the fetched token set --
/// Label Large (the near-universal real "control button label" role
/// every other component in this catalog already uses) and a plain
/// two-independent-buttons anatomy (no shared border) are reasonable,
/// MD3-consistent choices, stated honestly rather than asserted as
/// independently confirmed.
const TIME_FIELD_WIDTH: f32 = 96.0;
const TIME_FIELD_HEIGHT: f32 = 72.0;
const TIME_DISPLAY_FONT_SIZE: f32 = 45.0;
const TIME_DISPLAY_FONT_WEIGHT: f32 = 400.0;
const PERIOD_SELECTOR_WIDTH: f32 = 52.0;
const PERIOD_SELECTOR_HEIGHT: f32 = 72.0;
const PERIOD_OPTION_HEIGHT: f32 = PERIOD_SELECTOR_HEIGHT / 2.0;

/// `Popover` (M30 Phase 8 Step 1), grounded in MD3's own real *Rich
/// Tooltip* anatomy -- `BUILD_TRACKER.md`'s own scope text already
/// names this real grounding (pyCopper's own prior research, reused
/// rather than re-derived); the exact real token values below were
/// still directly re-verified against Material Web's own token
/// source (`_md-comp-rich-tooltip.scss`), not assumed to still match
/// from pyCopper's own different codebase. Real, confirmed anatomy:
/// `surface_container` fill, real `corner-medium` shape -- the
/// identical real 12dp value `Card`'s own `CARD_CORNER_RADIUS`
/// already confirmed for the same real token, reused directly -- a
/// real rest-state elevation (level 2, the identical real value
/// `Menu`'s own panel, `MENU_PANEL_ELEVATION`, already uses). Subhead
/// is Title Small (`on_surface_variant`) -- the identical real
/// numeric coincidence `Tabs`'s own `TAB_LABEL_FONT_SIZE`/`_WEIGHT`
/// already found and declared distinct constants for, reused here a
/// second time. Supporting text is Body Medium (`on_surface_variant`,
/// `DIALOG_BODY_FONT_SIZE`/`_WEIGHT` reused directly, the identical
/// real role `Dialog`'s own body text already uses).
///
/// **Real, deliberate reuse of `Window.open_menu`/`close_menu`
/// directly, the identical real design `Tooltip`/`Search View`
/// already established, not new dedicated methods:** a real Popover
/// is genuinely *persistent* -- unlike the already-real Plain
/// `Tooltip` (Phase 3 Step 5), which dismisses automatically on
/// hover-exit, a Rich Tooltip stays open until the user interacts
/// elsewhere or explicitly dismisses it, exactly the real behavior
/// `open_menu`'s own `dismiss_on_outside_click: true` already gives
/// for free -- no new persistence mechanism needed.
const POPOVER_CORNER_RADIUS: f64 = CARD_CORNER_RADIUS;
const POPOVER_ELEVATION: f64 = MENU_PANEL_ELEVATION;
const POPOVER_SUBHEAD_FONT_SIZE: f32 = TAB_LABEL_FONT_SIZE;
const POPOVER_SUBHEAD_FONT_WEIGHT: f32 = TAB_LABEL_FONT_WEIGHT;
/// Not a discrete token in the rich-tooltip's own token file
/// (confirmed by the same fetch) -- a reasonable, MD3-consistent
/// value, the identical honest caveat `Dialog`'s own padding
/// constants carry.
const POPOVER_PADDING: f32 = 16.0;
const POPOVER_SUBHEAD_GAP: f32 = 8.0;

/// `Link` (M30 Phase 8 Step 2) -- MD3 has no official Link component
/// page (confirmed by the same real per-directory-listing technique
/// this whole milestone already uses; no `_md-comp-link*` file
/// exists). Real, honest choice, not independently token-verified:
/// `primary` is the well-established, near-universal real MD3 link-
/// color convention (the same role every other "tap this to act"
/// text/label affordance in this catalog already resolves through),
/// reused directly rather than inventing a new role for something
/// with no dedicated token source. Label reuses Body Large's own
/// already-declared constants (`SEARCH_INPUT_FONT_SIZE`/`_WEIGHT`,
/// the identical real MD3 type role `Search Bar`'s own input text
/// already uses) -- a real link is ordinary running-text-sized
/// content, not a control-button label like `Label Large`.
///
/// **Real engine-core capability this step fulfills, not invented
/// fresh:** `NodeKind::Link` (a new, genuine `NodeKind`, `engine-
/// core/src/node.rs`) -- Phase 1's own `Tree::hit_test_at` fix
/// already stated the real commitment this makes good on: a bare
/// `Text` node deliberately never independently claims a hit (it
/// always defers to its real interactive container), so a real
/// standalone clickable label needs its own dedicated `NodeKind`,
/// the same "each interactive component is its own real `NodeKind`"
/// precedent `Checkbox`/`Slider`/`TextField` already established.
/// `NodeKind::Link` reuses `TextState` verbatim as its own payload
/// (identical real content/font shape to `Text`, only the variant
/// tag differs) and needs zero new hit-test logic at all -- by simply
/// not matching `NodeKind::Text(_) => false`, it falls through to
/// `hit_test_at`'s own existing `_ => rect_contains(...)` catch-all,
/// independent hit-testing "for free." Proven directly in `engine-
/// core`'s own test suite (`link_independently_claims_a_hit_where_
/// text_would_defer`): the identical real geometry built twice, once
/// with a bare `Text` child (defers, the parent claims the hit) and
/// once with a `Link` child (claims it directly) -- a real, concrete
/// contrast, not assumed from the enum shape alone.
const LINK_FONT_SIZE: f32 = SEARCH_INPUT_FONT_SIZE;
const LINK_FONT_WEIGHT: f32 = SEARCH_INPUT_FONT_WEIGHT;

/// `SpinBox`, a real numeric increment control (M30 Phase 8 Step 3).
/// **Real, deliberate naming, not the obvious guess:** pyCopper's own
/// real prior naming-risk finding, reused directly per `BUILD_TRACKER
/// .md`'s own scope text -- MD3's own vocabulary already uses
/// "Stepper" for a completely different real component (a multi-step
/// flow indicator), so this is named `SpinBox` from the start,
/// avoiding the exact collision pyCopper caught and had to rename
/// around mid-build. MD3 has no official page for either real name
/// (confirmed via the same directory-listing technique this whole
/// milestone already uses). Real, honest anatomy, not independently
/// token-verified: the numeric field reuses the identical real
/// `surface_container_highest`/`corner-small` convention `Time Input`
/// already established for a small boxed numeric display (a
/// genuinely smaller real footprint than Time Input's own 96×72dp
/// display, since a spin-box value is typically a short quantity, not
/// a two-digit clock field) -- Body Large text (`SEARCH_INPUT_FONT_
/// SIZE`/`_WEIGHT` reused directly), `on_surface`. Increment/
/// decrement reuse `Icon Button`'s own exact real anatomy (Phase 1
/// Step 2) -- `SEARCH_ICON_BUTTON_SIZE` reused directly (the
/// identical real 40dp value `Search Bar`/`Top App Bar` already
/// settled on for a compact icon-button footprint) -- with the newly
/// curated `add`/`remove` glyphs (`remove` fetched fresh this step,
/// the tenth curated icon, the identical real "additive... when a
/// real need asks for more" growth this module's own doc comment
/// already promises, `expand_more`'s own real precedent from Phase 6
/// Step 2 a second time).
const SPIN_BOX_FIELD_WIDTH: f32 = 64.0;
const SPIN_BOX_FIELD_HEIGHT: f32 = 40.0;
const SPIN_BOX_BUTTON_SIZE: f32 = SEARCH_ICON_BUTTON_SIZE;
const SPIN_BOX_GAP: f32 = 4.0;

/// `Pagination` (M30 Phase 8 Step 4) -- MD3 has no official page
/// (confirmed via the same real per-directory-listing technique this
/// whole milestone already uses; no `_md-comp-pagination*` file
/// exists). Real, honest anatomy, not independently token-verified:
/// each page indicator reuses the identical real 40dp circular
/// footprint `Search Bar`/`Top App Bar`/`SpinBox` already settled on
/// (`SEARCH_ICON_BUTTON_SIZE`, `corner_radius = size / 2.0`).
/// Selected: `primary` fill, `on_primary` label -- the identical real
/// selected-state pair `Date Picker`'s own day cell already uses (a
/// real, closely related "which one of these is active" affordance).
/// Unselected: transparent fill, `on_surface_variant` label -- the
/// same real convention `Segmented Button`/`Chip`/`Navigation Rail`
/// already established for their own unselected states. Prev/next
/// reuse `Icon Button`'s own exact real anatomy a fourth time this
/// catalog already has, with the newly curated `arrow_forward` glyph
/// pairing the already-curated `arrow_back` -- the eleventh curated
/// icon, the identical real "additive... when a real need asks for
/// more" growth `expand_more`/`remove` already established.
const PAGE_ITEM_SIZE: f32 = SEARCH_ICON_BUTTON_SIZE;
const PAGE_ITEM_CORNER_RADIUS: f64 = PAGE_ITEM_SIZE as f64 / 2.0;
const PAGE_ITEM_GAP: f32 = 4.0;

/// `Status Bar` (M30 Phase 8 Step 5) -- MD3 has no official page
/// (confirmed via the same real per-directory-listing technique this
/// whole milestone already uses). Real, honest anatomy, not
/// independently token-verified: a real, deliberate design reusing
/// `AppShell`'s own already-real `status_bar` region -- `build_shell`
/// (§14 step 13) already accepts any pre-built `Node` for it (a thin
/// bottom row in its own real flex-column layout), confirmed by
/// direct re-read before writing this step's own code; no new shell-
/// level wiring needed, only the real, styled bar *content* this step
/// actually builds. Thin (24dp, a genuine desktop convention, shorter
/// than every other bar in this catalog), `surface_container` fill --
/// the same real subtle-chrome role `Menu`'s own panel already uses.
/// Status text reuses Label Small's own already-declared constants
/// (`BADGE_LABEL_FONT_SIZE`/`_WEIGHT`, the identical real MD3 type
/// role `Badge`'s own labeled variant already found, Phase 3 Step 1),
/// `on_surface_variant`.
const STATUS_BAR_HEIGHT: f32 = 24.0;
const STATUS_BAR_PADDING: f32 = 8.0;

/// MD3's own real Button anatomy constants (M3 spec, Buttons component
/// page): 24dp horizontal padding for a label-only button (no leading/
/// trailing icon -- that's `Icon Button`'s own separate anatomy, Phase
/// 1 Step 2, not this one), Label Large's real type role (14sp/500
/// weight) for the button's own label.
const BUTTON_HORIZONTAL_PADDING: f32 = 24.0;
const BUTTON_LABEL_FONT_SIZE: f32 = 14.0;
const BUTTON_LABEL_FONT_WEIGHT: f32 = 500.0;
/// A real line-height a 14sp label comfortably fits inside without
/// clipping ascenders/descenders (`text_align.rs`'s own real ink-
/// presence proof used the same kind of generous, non-tight box) --
/// deliberately smaller than the button's own `height`, so `align_
/// items: Center` on the container has real cross-axis slack to
/// vertically center the label within, not zero room to move it at
/// all.
const BUTTON_LABEL_LINE_HEIGHT: f32 = 20.0;
/// MD3's own real `Icon Button` anatomy constant: the icon glyph
/// itself stays 24dp regardless of the container's own touch-target
/// `size` (`add_icon_button`'s own doc comment).
const ICON_BUTTON_ICON_SIZE: f32 = 24.0;
/// MD3's own real `Extended FAB` anatomy constants, verified against
/// Material Web's own token source (`fab/internal/_fab.scss`'s real
/// CSS: `padding-inline: 16px 20px` with an icon slotted, `20px` both
/// sides without one) -- `Extended FAB` has exactly one real size
/// (unlike plain `FAB`'s three), 56dp tall with `corner-large` (16dp,
/// the same real token plain `FAB`'s own `"default"` size uses).
const EXTENDED_FAB_HEIGHT: f32 = 56.0;
const EXTENDED_FAB_CORNER_RADIUS: f64 = 16.0;
const EXTENDED_FAB_LEADING_PADDING_WITH_ICON: f32 = 16.0;
const EXTENDED_FAB_LEADING_PADDING_NO_ICON: f32 = 20.0;
const EXTENDED_FAB_TRAILING_PADDING: f32 = 20.0;
const EXTENDED_FAB_ICON_LABEL_GAP: f32 = 8.0;
/// MD3's own real `Segmented Button` anatomy constants, verified
/// against Material Web's own token source (`tokens/versions/v0_192/
/// _md-comp-outlined-segmented-button.scss`): 40dp tall, 1px real
/// outline (the same width drawn for the group's own shared outer
/// border *and* the internal dividers between segments -- MD3 uses
/// one token for both), `corner-full` shape (`height / 2.0`, the same
/// real token `Button`'s own default shape family uses), and a real
/// 18dp checkmark -- deliberately smaller than every other component's
/// own 24dp icon token in this catalog, confirmed from the real token
/// file rather than assumed consistent with `Icon Button`/`FAB`.
/// **One real number not found in this token file, so not claimed as
/// independently re-verified:** the segment's own horizontal label
/// padding and the icon-to-label gap -- both real, reasonable MD3
/// values, stated honestly rather than presented as verified against
/// the same primary source the others were.
const SEGMENTED_BUTTON_HEIGHT: f32 = 40.0;
const SEGMENTED_BUTTON_OUTLINE_WIDTH: f32 = 1.0;
const SEGMENTED_BUTTON_CHECKMARK_SIZE: f32 = 18.0;
const SEGMENTED_BUTTON_HORIZONTAL_PADDING: f32 = 12.0;
const SEGMENTED_BUTTON_ICON_LABEL_GAP: f32 = 4.0;

// M52 Phases 2+: every remaining `RetitheHook` builder function lives
// here, grouped together rather than scattered near each factory's own
// consts the way `resolve_button_colors`/`button_retheme_hook` (Phase
// 1, above) sit right next to `add_button`'s own real anatomy constants
// -- a deliberate, purely organizational choice for this milestone's
// remaining ~45 factories (a genuinely large, repetitive-pattern batch,
// not worth the per-factory file-navigation cost of colocating each one
// individually the way the file's pre-existing helpers do).

/// M52 Phase 2: `add_card`'s own hook -- bare `"card"` key, no variant,
/// matching `add_card` itself (corner radius is not variant-dependent
/// in real MD3 anatomy).
fn card_retheme_hook(id: NodeId, variant: String) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let Ok(colors) = resolve_card_colors(theme, &variant) else {
            return;
        };
        let corner_radius = theme.shape("card", None).unwrap_or(CARD_CORNER_RADIUS);
        if let Some(node) = tree.get_mut(id) {
            node.paint.background = Animated::new(colors.container);
            node.paint.corner_radius = Animated::new(corner_radius);
            node.paint.elevation = Animated::new(colors.elevation);
            node.paint.border_color = Animated::new(colors.border_color);
            node.paint.border_width = Animated::new(colors.border_width);
        }
    })
}

/// M52 Phase 2: `add_divider`'s own hook -- one role, no shape/
/// elevation (a divider has neither in real MD3 anatomy).
fn divider_retheme_hook(id: NodeId) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let color = if theme.is_set() {
            theme
                .role("outline_variant")
                .unwrap_or(Md3Baseline::OUTLINE_VARIANT)
        } else {
            Md3Baseline::OUTLINE_VARIANT
        };
        if let Some(node) = tree.get_mut(id) {
            node.paint.background = Animated::new(color);
        }
    })
}

/// M52 Phase 2: `add_tooltip`'s own hook -- only `corner_radius` is
/// theme-resolved (a real, confirmed, pre-existing M49-era gap, not
/// fixed by this milestone: `add_tooltip`'s own container/label colors
/// are hardcoded `Md3Baseline` constants with no `theme.role(...)` call
/// at all). This hook correctly does strictly less than its siblings.
fn tooltip_retheme_hook(container: NodeId) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let corner_radius = theme
            .shape("tooltip", None)
            .unwrap_or(TOOLTIP_CORNER_RADIUS);
        if let Some(node) = tree.get_mut(container) {
            node.paint.corner_radius = Animated::new(corner_radius);
        }
    })
}

/// M52 Phase 2: `add_toolbar`'s own hook -- both the container color
/// (branches on `color`, `"standard"` vs `"vibrant"`) and the shape/
/// elevation fallback (branches on `is_floating`/`vertical`/the
/// caller's own original `width`/`height`) must be reproduced exactly,
/// since `add_toolbar` itself computes conditional *defaults*, not
/// fixed constants, before ever consulting the theme override.
#[allow(clippy::too_many_arguments)]
fn toolbar_retheme_hook(
    bar: NodeId,
    variant: String,
    color: Option<String>,
    is_floating: bool,
    vertical: bool,
    width: Option<f32>,
    height: Option<f32>,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let role = |name: &str, fallback: Color| -> Color {
            if theme.is_set() {
                theme.role(name).unwrap_or(fallback)
            } else {
                fallback
            }
        };
        let container_color = match color.as_deref().unwrap_or("standard") {
            "standard" => role("surface_container", Md3Baseline::SURFACE_CONTAINER),
            "vibrant" => role("primary_container", Md3Baseline::PRIMARY_CONTAINER),
            _ => return,
        };
        let default_corner_radius = if is_floating {
            f64::from(if vertical {
                width.unwrap_or(TOOLBAR_HEIGHT)
            } else {
                height.unwrap_or(TOOLBAR_HEIGHT)
            }) / 2.0
        } else {
            0.0
        };
        let default_elevation = if is_floating {
            FAB_REST_ELEVATION_LEVEL
        } else {
            0.0
        };
        let corner_radius = theme
            .shape("toolbar", Some(&variant))
            .unwrap_or(default_corner_radius);
        let elevation = theme
            .elevation("toolbar", Some(&variant))
            .unwrap_or(default_elevation);
        if let Some(node) = tree.get_mut(bar) {
            node.paint.background = Animated::new(container_color);
            node.paint.corner_radius = Animated::new(corner_radius);
            node.paint.elevation = Animated::new(elevation);
        }
    })
}

/// M52 Phase 2: `add_spin_box`'s own hook, closing Phase 2 -- the
/// decrement/increment buttons deliberately reuse the `"icon_button"`
/// key (structurally the same real shape `add_icon_button` itself
/// produces), while the field gets its own distinct `"spin_box"` key.
fn spin_box_retheme_hook(
    field: NodeId,
    decrement: NodeId,
    decrement_icon: NodeId,
    increment: NodeId,
    increment_icon: NodeId,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let role = |theme: &crate::window::ThemeState, name: &str, fallback: Color| -> Color {
            if theme.is_set() {
                theme.role(name).unwrap_or(fallback)
            } else {
                fallback
            }
        };
        let field_color = role(
            theme,
            "surface_container_highest",
            Md3Baseline::SURFACE_CONTAINER_HIGHEST,
        );
        let text_color = theme.on_surface();
        let icon_color = role(theme, "on_surface_variant", Md3Baseline::ON_SURFACE_VARIANT);
        let icon_button_corner_radius = theme
            .shape("icon_button", None)
            .unwrap_or(SPIN_BOX_BUTTON_SIZE as f64 / 2.0);
        let field_corner_radius = theme.shape("spin_box", None).unwrap_or(CHIP_CORNER_RADIUS);
        for button in [decrement, increment] {
            if let Some(node) = tree.get_mut(button) {
                node.paint.corner_radius = Animated::new(icon_button_corner_radius);
            }
        }
        for icon in [decrement_icon, increment_icon] {
            if let Some(node) = tree.get_mut(icon)
                && let NodeKind::Icon(state) = &mut node.kind
            {
                state.tint = icon_color;
            }
        }
        if let Some(node) = tree.get_mut(field) {
            node.paint.background = Animated::new(field_color);
            node.paint.corner_radius = Animated::new(field_corner_radius);
            if let NodeKind::TextField(state) = &mut node.kind {
                state.text_tint = text_color;
            }
        }
    })
}

/// M52 Phase 2: `add_period_selector`'s own hook -- 2 options
/// (am/pm), each independently `is_selected`, its own distinct
/// `"period_selector"` key.
#[allow(clippy::too_many_arguments)]
fn period_selector_retheme_hook(
    am: NodeId,
    am_label: NodeId,
    am_selected: bool,
    pm: NodeId,
    pm_label: NodeId,
    pm_selected: bool,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let selected_fill = if theme.is_set() {
            theme
                .role("tertiary_container")
                .unwrap_or(Md3Baseline::TERTIARY_CONTAINER)
        } else {
            Md3Baseline::TERTIARY_CONTAINER
        };
        let selected_label = if theme.is_set() {
            theme
                .role("on_tertiary_container")
                .unwrap_or(Md3Baseline::ON_TERTIARY_CONTAINER)
        } else {
            Md3Baseline::ON_TERTIARY_CONTAINER
        };
        let unselected_label = theme.on_surface();
        let corner_radius = theme
            .shape("period_selector", None)
            .unwrap_or(CHIP_CORNER_RADIUS);
        for (option, label, is_selected) in
            [(am, am_label, am_selected), (pm, pm_label, pm_selected)]
        {
            let (fill, label_color) = if is_selected {
                (selected_fill, selected_label)
            } else {
                (TRANSPARENT, unselected_label)
            };
            if let Some(node) = tree.get_mut(option) {
                node.paint.background = Animated::new(fill);
                node.paint.corner_radius = Animated::new(corner_radius);
            }
            if let Some(node) = tree.get_mut(label) {
                node.paint.background = Animated::new(label_color);
            }
        }
    })
}

/// M52 Phase 2: `add_time_input_field`'s own hook -- `TextFieldState.
/// text_tint` is a plain field, not `Animated`, written directly.
/// `text_tint` reads `theme.on_surface()` unconditionally, without an
/// `is_set()` gate -- a real, pre-existing minor inconsistency vs.
/// `add_text_field`/`add_code_editor` (both gate it), confirmed
/// harmless (`on_surface()` has its own safe no-theme-set default
/// built in) and deliberately reproduced as-is here, not silently
/// "fixed" as part of this milestone's own narrower scope.
fn time_input_field_retheme_hook(id: NodeId) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let container_color = if theme.is_set() {
            theme
                .role("surface_container_highest")
                .unwrap_or(Md3Baseline::SURFACE_CONTAINER_HIGHEST)
        } else {
            Md3Baseline::SURFACE_CONTAINER_HIGHEST
        };
        let corner_radius = theme
            .shape("time_input_field", None)
            .unwrap_or(CHIP_CORNER_RADIUS);
        if let Some(node) = tree.get_mut(id) {
            node.paint.background = Animated::new(container_color);
            node.paint.corner_radius = Animated::new(corner_radius);
            if let NodeKind::TextField(state) = &mut node.kind {
                state.text_tint = theme.on_surface();
            }
        }
    })
}

/// M52 Phase 2: `add_date_picker_day`'s own hook -- reproduces the
/// exact same 4-outcome `selected`/`today`/`outside_month` branch
/// `add_date_picker_day` itself resolves at construction time.
fn date_picker_day_retheme_hook(
    cell: NodeId,
    label: NodeId,
    selected: bool,
    today: bool,
    outside_month: bool,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let role = |theme: &crate::window::ThemeState, name: &str, fallback: Color| -> Color {
            if theme.is_set() {
                theme.role(name).unwrap_or(fallback)
            } else {
                fallback
            }
        };
        let primary = role(theme, "primary", Md3Baseline::PRIMARY);
        let (fill, border_color, border_width, label_color) = if selected {
            (
                primary,
                TRANSPARENT,
                0.0,
                role(theme, "on_primary", Md3Baseline::ON_PRIMARY),
            )
        } else if today {
            (TRANSPARENT, primary, DATE_TODAY_OUTLINE_WIDTH, primary)
        } else if outside_month {
            (
                TRANSPARENT,
                TRANSPARENT,
                0.0,
                role(theme, "on_surface_variant", Md3Baseline::ON_SURFACE_VARIANT),
            )
        } else {
            (TRANSPARENT, TRANSPARENT, 0.0, theme.on_surface())
        };
        let corner_radius = theme
            .shape("date_picker_day", None)
            .unwrap_or(DATE_CELL_CORNER_RADIUS);
        if let Some(node) = tree.get_mut(cell) {
            node.paint.background = Animated::new(fill);
            node.paint.corner_radius = Animated::new(corner_radius);
            node.paint.border_color = Animated::new(border_color);
            node.paint.border_width = Animated::new(border_width);
        }
        if let Some(node) = tree.get_mut(label) {
            node.paint.background = Animated::new(label_color);
        }
    })
}

/// M52 Phase 2: `add_accordion_header`'s own hook -- never touches
/// `chevron`'s own `paint.transform` (the real, currently-set
/// expand/collapse flip state), only its `IconState.tint`.
fn accordion_header_retheme_hook(headline: NodeId, chevron: NodeId) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let on_surface_variant = if theme.is_set() {
            theme
                .role("on_surface_variant")
                .unwrap_or(Md3Baseline::ON_SURFACE_VARIANT)
        } else {
            Md3Baseline::ON_SURFACE_VARIANT
        };
        if let Some(node) = tree.get_mut(headline) {
            node.paint.background = Animated::new(theme.on_surface());
        }
        if let Some(node) = tree.get_mut(chevron)
            && let NodeKind::Icon(state) = &mut node.kind
        {
            state.tint = on_surface_variant;
        }
    })
}

/// M52 Phase 2: `add_tree_node`'s own hook -- `chevron` is `None` for a
/// leaf row (nothing to re-theme there); never touches its own
/// `paint.transform` either, the same real reason `add_accordion_
/// header`'s hook doesn't.
fn tree_node_retheme_hook(headline: NodeId, chevron: Option<NodeId>) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let on_surface_variant = if theme.is_set() {
            theme
                .role("on_surface_variant")
                .unwrap_or(Md3Baseline::ON_SURFACE_VARIANT)
        } else {
            Md3Baseline::ON_SURFACE_VARIANT
        };
        if let Some(node) = tree.get_mut(headline) {
            node.paint.background = Animated::new(theme.on_surface());
        }
        if let Some(id) = chevron
            && let Some(node) = tree.get_mut(id)
            && let NodeKind::Icon(state) = &mut node.kind
        {
            state.tint = on_surface_variant;
        }
    })
}

/// M52 Phase 2: `add_list_item`'s own hook -- the milestone's first
/// icon-touching factory: `IconState.tint` is a plain (non-`Animated`)
/// field, written directly rather than through `Animated::new`. No
/// shape/elevation (a list item has neither in real MD3 anatomy).
fn list_item_retheme_hook(
    headline: NodeId,
    supporting: Option<NodeId>,
    leading_icon: Option<NodeId>,
    trailing_icon: Option<NodeId>,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let on_surface_variant = if theme.is_set() {
            theme
                .role("on_surface_variant")
                .unwrap_or(Md3Baseline::ON_SURFACE_VARIANT)
        } else {
            Md3Baseline::ON_SURFACE_VARIANT
        };
        if let Some(node) = tree.get_mut(headline) {
            node.paint.background = Animated::new(theme.on_surface());
        }
        if let Some(id) = supporting
            && let Some(node) = tree.get_mut(id)
        {
            node.paint.background = Animated::new(on_surface_variant);
        }
        for icon in [leading_icon, trailing_icon].into_iter().flatten() {
            if let Some(node) = tree.get_mut(icon)
                && let NodeKind::Icon(state) = &mut node.kind
            {
                state.tint = on_surface_variant;
            }
        }
    })
}

/// M52 Phase 2: `add_status_bar`'s own hook -- 2 roles, no shape/
/// elevation.
fn status_bar_retheme_hook(bar: NodeId, label: NodeId) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let container = if theme.is_set() {
            theme
                .role("surface_container")
                .unwrap_or(Md3Baseline::SURFACE_CONTAINER)
        } else {
            Md3Baseline::SURFACE_CONTAINER
        };
        let on_surface_variant = if theme.is_set() {
            theme
                .role("on_surface_variant")
                .unwrap_or(Md3Baseline::ON_SURFACE_VARIANT)
        } else {
            Md3Baseline::ON_SURFACE_VARIANT
        };
        if let Some(node) = tree.get_mut(bar) {
            node.paint.background = Animated::new(container);
        }
        if let Some(node) = tree.get_mut(label) {
            node.paint.background = Animated::new(on_surface_variant);
        }
    })
}

/// M52 Phase 2: `add_node_graph`'s own hook -- one role, no shape/
/// elevation.
fn node_graph_retheme_hook(id: NodeId) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let color = if theme.is_set() {
            theme
                .role("surface_container_low")
                .unwrap_or(Md3Baseline::SURFACE_CONTAINER_LOW)
        } else {
            Md3Baseline::SURFACE_CONTAINER_LOW
        };
        if let Some(node) = tree.get_mut(id) {
            node.paint.background = Animated::new(color);
        }
    })
}

/// M52 Phase 2: `add_graph_node`'s own hook -- 3 nodes (wrapper/title_
/// bar/label), its own distinct `"graph_node"` key (not `"card"`, a
/// coincidentally-shared constant value). `title_bar`'s own fixed
/// `0`/`0` corner_radius/elevation are never theme-derived, so this
/// hook correctly never touches them.
fn graph_node_retheme_hook(
    wrapper: NodeId,
    title_bar: NodeId,
    label: NodeId,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let body = if theme.is_set() {
            theme
                .role("surface_container")
                .unwrap_or(Md3Baseline::SURFACE_CONTAINER)
        } else {
            Md3Baseline::SURFACE_CONTAINER
        };
        let title = if theme.is_set() {
            theme
                .role("surface_container_high")
                .unwrap_or(Md3Baseline::SURFACE_CONTAINER_HIGH)
        } else {
            Md3Baseline::SURFACE_CONTAINER_HIGH
        };
        let label_color = if theme.is_set() {
            theme
                .role("on_surface_variant")
                .unwrap_or(Md3Baseline::ON_SURFACE_VARIANT)
        } else {
            Md3Baseline::ON_SURFACE_VARIANT
        };
        let corner_radius = theme
            .shape("graph_node", None)
            .unwrap_or(CARD_CORNER_RADIUS);
        if let Some(node) = tree.get_mut(wrapper) {
            node.paint.background = Animated::new(body);
            node.paint.corner_radius = Animated::new(corner_radius);
        }
        if let Some(node) = tree.get_mut(title_bar) {
            node.paint.background = Animated::new(title);
        }
        if let Some(node) = tree.get_mut(label) {
            node.paint.background = Animated::new(label_color);
        }
    })
}

/// M52 Phase 2: `add_search_view`'s own hook -- its own distinct
/// `"search_view"` key, not `"dialog"` (a coincidentally-shared
/// constant *value*, not a reason to couple the overrides).
fn search_view_retheme_hook(id: NodeId) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let container_color = if theme.is_set() {
            theme
                .role("surface_container_high")
                .unwrap_or(Md3Baseline::SURFACE_CONTAINER_HIGH)
        } else {
            Md3Baseline::SURFACE_CONTAINER_HIGH
        };
        let corner_radius = theme
            .shape("search_view", None)
            .unwrap_or(DIALOG_CORNER_RADIUS);
        let elevation = theme
            .elevation("search_view", None)
            .unwrap_or(SEARCH_VIEW_ELEVATION);
        if let Some(node) = tree.get_mut(id) {
            node.paint.background = Animated::new(container_color);
            node.paint.corner_radius = Animated::new(corner_radius);
            node.paint.elevation = Animated::new(elevation);
        }
    })
}

/// M52 Phase 2: `add_popover`'s own hook -- 3 nodes (panel/subhead/
/// body), its own distinct `"popover"` key.
fn popover_retheme_hook(
    panel: NodeId,
    subhead: NodeId,
    body: NodeId,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let on_surface_variant = if theme.is_set() {
            theme
                .role("on_surface_variant")
                .unwrap_or(Md3Baseline::ON_SURFACE_VARIANT)
        } else {
            Md3Baseline::ON_SURFACE_VARIANT
        };
        let container_color = if theme.is_set() {
            theme
                .role("surface_container")
                .unwrap_or(Md3Baseline::SURFACE_CONTAINER)
        } else {
            Md3Baseline::SURFACE_CONTAINER
        };
        let corner_radius = theme
            .shape("popover", None)
            .unwrap_or(POPOVER_CORNER_RADIUS);
        let elevation = theme
            .elevation("popover", None)
            .unwrap_or(POPOVER_ELEVATION);
        if let Some(node) = tree.get_mut(panel) {
            node.paint.background = Animated::new(container_color);
            node.paint.corner_radius = Animated::new(corner_radius);
            node.paint.elevation = Animated::new(elevation);
        }
        if let Some(node) = tree.get_mut(subhead) {
            node.paint.background = Animated::new(on_surface_variant);
        }
        if let Some(node) = tree.get_mut(body) {
            node.paint.background = Animated::new(on_surface_variant);
        }
    })
}

/// M52 Phase 2: `add_link`'s own hook -- one role, no shape/elevation
/// (a link has neither in real MD3 anatomy).
fn link_retheme_hook(id: NodeId) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let color = if theme.is_set() {
            theme.role("primary").unwrap_or(Md3Baseline::PRIMARY)
        } else {
            Md3Baseline::PRIMARY
        };
        if let Some(node) = tree.get_mut(id) {
            node.paint.background = Animated::new(color);
        }
    })
}

/// M52 Phase 2: `add_dialog`'s own hook -- 4 nodes (scrim/panel/
/// headline/body), only 3 of which are theme-resolved (scrim's own
/// `opacity` is a fixed constant, `DIALOG_SCRIM_OPACITY`, never theme-
/// derived, so it's correctly never touched here).
fn dialog_retheme_hook(
    scrim: NodeId,
    panel: NodeId,
    headline: NodeId,
    body: NodeId,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let role = |theme: &crate::window::ThemeState, name: &str, fallback: Color| -> Color {
            if theme.is_set() {
                theme.role(name).unwrap_or(fallback)
            } else {
                fallback
            }
        };
        let scrim_color = role(theme, "scrim", Md3Baseline::SCRIM);
        let panel_color = role(
            theme,
            "surface_container_high",
            Md3Baseline::SURFACE_CONTAINER_HIGH,
        );
        let headline_color = theme.on_surface();
        let body_color = role(theme, "on_surface_variant", Md3Baseline::ON_SURFACE_VARIANT);
        let corner_radius = theme.shape("dialog", None).unwrap_or(DIALOG_CORNER_RADIUS);
        let elevation = theme.elevation("dialog", None).unwrap_or(DIALOG_ELEVATION);
        if let Some(node) = tree.get_mut(scrim) {
            node.paint.background = Animated::new(scrim_color);
        }
        if let Some(node) = tree.get_mut(panel) {
            node.paint.background = Animated::new(panel_color);
            node.paint.corner_radius = Animated::new(corner_radius);
            node.paint.elevation = Animated::new(elevation);
        }
        if let Some(node) = tree.get_mut(headline) {
            node.paint.background = Animated::new(headline_color);
        }
        if let Some(node) = tree.get_mut(body) {
            node.paint.background = Animated::new(body_color);
        }
    })
}

/// M52 Phase 2: `add_loading_indicator`'s own hook -- only ever
/// registered when the caller's own `color` argument was `None` (an
/// explicit literal color always wins, forever, and was never theme-
/// derived to begin with). Recomputes the identical `role("primary")`
/// fallback `add_loading_indicator` itself used at construction time.
/// Never touches `paint.shape` -- that field is the real, currently-
/// animating MD3 morph target (`Tree::tick_all`'s own automatic
/// advance), not something a retheme should ever clobber.
fn loading_indicator_retheme_hook(id: NodeId) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let tint = if theme.is_set() {
            theme.role("primary").unwrap_or(Md3Baseline::PRIMARY)
        } else {
            Md3Baseline::PRIMARY
        };
        if let Some(node) = tree.get_mut(id) {
            node.paint.background = Animated::new(tint);
        }
    })
}

// M52 Phase 3: Buttons & FAB family.

/// M52 Phase 3: `add_icon_button`'s own hook -- reuses `resolved_
/// variant` ("standard" already translated to "text") consistently for
/// both color and shape, the same real consistency fix `add_icon_
/// button` itself makes at construction time. `icon_id`'s own
/// `IconState.tint` = `colors.label`, matching the factory's own real
/// anatomy.
fn icon_button_retheme_hook(
    container: NodeId,
    icon_id: NodeId,
    resolved_variant: String,
    size: f32,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let Ok(colors) = resolve_button_colors(theme, &resolved_variant, "icon_button") else {
            return;
        };
        let corner_radius = theme
            .shape("icon_button", Some(&resolved_variant))
            .unwrap_or_else(|| f64::from(size) / 2.0);
        if let Some(node) = tree.get_mut(container) {
            node.paint.background = Animated::new(colors.container);
            node.paint.corner_radius = Animated::new(corner_radius);
            node.paint.elevation = Animated::new(colors.elevation);
            node.paint.border_color = Animated::new(colors.border_color);
            node.paint.border_width = Animated::new(colors.border_width);
        }
        if let Some(node) = tree.get_mut(icon_id)
            && let NodeKind::Icon(state) = &mut node.kind
        {
            state.tint = colors.label;
        }
    })
}

/// M52 Phase 3: `add_fab`'s own hook -- corner radius is keyed by
/// *size* (`fab_shape`'s own real fallback recomputed here too, since
/// each real size has its own distinct default), elevation by no
/// variant at all (real MD3 anatomy is flat regardless of size/color).
fn fab_retheme_hook(
    container: NodeId,
    icon_id: NodeId,
    variant: String,
    size: String,
    default_corner_radius: f32,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let Ok(colors) = resolve_fab_colors(theme, &variant) else {
            return;
        };
        let corner_radius = theme
            .shape("fab", Some(&size))
            .unwrap_or(f64::from(default_corner_radius));
        let elevation = theme
            .elevation("fab", None)
            .unwrap_or(FAB_REST_ELEVATION_LEVEL);
        if let Some(node) = tree.get_mut(container) {
            node.paint.background = Animated::new(colors.container);
            node.paint.corner_radius = Animated::new(corner_radius);
            node.paint.elevation = Animated::new(elevation);
        }
        if let Some(node) = tree.get_mut(icon_id)
            && let NodeKind::Icon(state) = &mut node.kind
        {
            state.tint = colors.icon;
        }
    })
}

/// M52 Phase 3: `add_extended_fab`'s own hook -- `icon_id` is `None`
/// when the factory was called with no `icon`; the label's own color
/// reuses `colors.icon` (real MD3 anatomy, matching `add_extended_fab`
/// itself, not a bug). Its own distinct `"extended_fab"` key, no
/// variant at all (no size variants for Extended FAB in real MD3).
fn extended_fab_retheme_hook(
    container: NodeId,
    icon_id: Option<NodeId>,
    label: NodeId,
    variant: String,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let Ok(colors) = resolve_fab_colors(theme, &variant) else {
            return;
        };
        let corner_radius = theme
            .shape("extended_fab", None)
            .unwrap_or(EXTENDED_FAB_CORNER_RADIUS);
        let elevation = theme
            .elevation("extended_fab", None)
            .unwrap_or(FAB_REST_ELEVATION_LEVEL);
        if let Some(node) = tree.get_mut(container) {
            node.paint.background = Animated::new(colors.container);
            node.paint.corner_radius = Animated::new(corner_radius);
            node.paint.elevation = Animated::new(elevation);
        }
        if let Some(id) = icon_id
            && let Some(node) = tree.get_mut(id)
            && let NodeKind::Icon(state) = &mut node.kind
        {
            state.tint = colors.icon;
        }
        if let Some(node) = tree.get_mut(label) {
            node.paint.background = Animated::new(colors.icon);
        }
    })
}

/// M52 Phase 3: `add_chip`'s own hook -- `selected` (in addition to
/// `variant`) is captured, since `resolve_chip_colors` resolves
/// differently depending on it. No elevation lookup (Chip has no real
/// elevation concept in MD3 anatomy). `leading`/`trailing` are each
/// independently `Option<NodeId>` -- a chip may have neither, either,
/// or both.
fn chip_retheme_hook(
    container: NodeId,
    label: NodeId,
    leading: Option<NodeId>,
    trailing: Option<NodeId>,
    variant: String,
    selected: bool,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let Ok(colors) = resolve_chip_colors(theme, &variant, selected) else {
            return;
        };
        let corner_radius = theme.shape("chip", None).unwrap_or(CHIP_CORNER_RADIUS);
        if let Some(node) = tree.get_mut(container) {
            node.paint.background = Animated::new(colors.container);
            node.paint.corner_radius = Animated::new(corner_radius);
            node.paint.border_color = Animated::new(colors.border_color);
            node.paint.border_width = Animated::new(colors.border_width);
        }
        if let Some(node) = tree.get_mut(label) {
            node.paint.background = Animated::new(colors.label);
        }
        for icon in [leading, trailing].into_iter().flatten() {
            if let Some(node) = tree.get_mut(icon)
                && let NodeKind::Icon(state) = &mut node.kind
            {
                state.tint = colors.icon;
            }
        }
    })
}

/// M52 Phase 3: `add_badge`'s own hook for its "dot" anatomy (no
/// label) -- its own `"badge.dot"` key, distinct from `"badge.
/// labeled"`.
fn badge_dot_retheme_hook(id: NodeId) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let color = if theme.is_set() {
            theme.role("error").unwrap_or(Md3Baseline::ERROR)
        } else {
            Md3Baseline::ERROR
        };
        let corner_radius = theme
            .shape("badge", Some("dot"))
            .unwrap_or_else(|| f64::from(BADGE_DOT_SIZE) / 2.0);
        if let Some(node) = tree.get_mut(id) {
            node.paint.background = Animated::new(color);
            node.paint.corner_radius = Animated::new(corner_radius);
        }
    })
}

/// M52 Phase 3: `add_badge`'s own hook for its "labeled" anatomy -- its
/// own `"badge.labeled"` key, distinct from `"badge.dot"`.
fn badge_labeled_retheme_hook(container: NodeId, label: NodeId) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let role = |theme: &crate::window::ThemeState, name: &str, fallback: Color| -> Color {
            if theme.is_set() {
                theme.role(name).unwrap_or(fallback)
            } else {
                fallback
            }
        };
        let error_color = role(theme, "error", Md3Baseline::ERROR);
        let on_error_color = role(theme, "on_error", Md3Baseline::ON_ERROR);
        let corner_radius = theme
            .shape("badge", Some("labeled"))
            .unwrap_or_else(|| f64::from(BADGE_LABELED_HEIGHT) / 2.0);
        if let Some(node) = tree.get_mut(container) {
            node.paint.background = Animated::new(error_color);
            node.paint.corner_radius = Animated::new(corner_radius);
        }
        if let Some(node) = tree.get_mut(label) {
            node.paint.background = Animated::new(on_error_color);
        }
    })
}

/// M52 Phase 3: `add_segmented_button`'s own hook -- the most
/// structurally involved of Phase 3's "single-factory" cases (`add_
/// split_button`/`add_button_group` are harder still, but for a
/// different, nested-reuse reason, handled separately below). Captures
/// each segment's own `(segment, label, check, is_selected)` at
/// construction time -- `is_selected` is real, app-owned state (Design
/// Principle 6), never re-derived, the same "fixed at construction,
/// only theme-tier properties re-resolve" convention `add_date_picker_
/// day`'s own hook already established. `corner_radii_override` is
/// reproduced using the *index within this Vec* (first/last), the
/// identical real branching `add_segmented_button` itself uses.
fn segmented_button_retheme_hook(
    frame: NodeId,
    segments: Vec<(NodeId, NodeId, Option<NodeId>, bool)>,
    dividers: Vec<NodeId>,
    height: f32,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let outline_color = if theme.is_set() {
            theme.role("outline").unwrap_or(Md3Baseline::OUTLINE)
        } else {
            Md3Baseline::OUTLINE
        };
        let on_surface = theme.on_surface();
        let secondary_container = if theme.is_set() {
            theme
                .role("secondary_container")
                .unwrap_or(Md3Baseline::SECONDARY_CONTAINER)
        } else {
            Md3Baseline::SECONDARY_CONTAINER
        };
        let on_secondary_container = if theme.is_set() {
            theme
                .role("on_secondary_container")
                .unwrap_or(Md3Baseline::ON_SECONDARY_CONTAINER)
        } else {
            Md3Baseline::ON_SECONDARY_CONTAINER
        };
        let corner = theme
            .shape("segmented_button", None)
            .unwrap_or_else(|| f64::from(height) / 2.0);
        if let Some(node) = tree.get_mut(frame) {
            node.paint.corner_radius = Animated::new(corner);
            node.paint.border_color = Animated::new(outline_color);
        }
        let n = segments.len();
        for (i, &(segment, label, check, is_selected)) in segments.iter().enumerate() {
            let corner_radii_override = if i == 0 {
                Some([corner, 0.0, 0.0, corner])
            } else if i == n - 1 {
                Some([0.0, corner, corner, 0.0])
            } else {
                None
            };
            if let Some(node) = tree.get_mut(segment) {
                node.paint.background = Animated::new(if is_selected {
                    secondary_container
                } else {
                    TRANSPARENT
                });
                node.paint.corner_radii_override = corner_radii_override;
            }
            let label_color = if is_selected {
                on_secondary_container
            } else {
                on_surface
            };
            if let Some(node) = tree.get_mut(label) {
                node.paint.background = Animated::new(label_color);
            }
            if let Some(id) = check
                && let Some(node) = tree.get_mut(id)
                && let NodeKind::Icon(state) = &mut node.kind
            {
                state.tint = on_secondary_container;
            }
        }
        for &divider in &dividers {
            if let Some(node) = tree.get_mut(divider) {
                node.paint.background = Animated::new(outline_color);
            }
        }
    })
}

/// M52 Phase 3: `add_split_button`'s own hook -- the milestone's first
/// nested-reuse case. `leading` already has its own `button_retheme_
/// hook` (registered by the internal `self.add_button(...)` call
/// `add_split_button` itself makes) correctly re-resolving its
/// background/corner_radius/elevation/border live -- this hook only
/// layers the *additional* shape-morph geometry on top (`paint.shape`/
/// `interactive_shape`, disjoint fields the plain button hook never
/// touches), reproducing the exact real bug-fix `add_split_button`'s
/// own doc comment states: `rest_radius` must be the *same* value
/// `leading`'s own background hook resolves, not an independently
/// recomputed one, or the painted shape could silently disagree with
/// the painted corner radius. `trailing`/`icon_id` never went through
/// `add_button` at all, so this hook recomputes their full paint from
/// scratch, the same `resolve_button_colors(theme, &variant, "button")`
/// call `add_split_button` itself makes (not a `"split_button"` key).
fn split_button_retheme_hook(
    leading: NodeId,
    trailing: NodeId,
    icon_id: NodeId,
    variant: String,
    width: f32,
    height: f32,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let Ok(colors) = resolve_button_colors(theme, &variant, "button") else {
            return;
        };
        let rest_radius = theme
            .shape("button", Some(&variant))
            .unwrap_or_else(|| f64::from(height) / 2.0);
        let tightened_radius = theme
            .shape("split_button", Some("tightened"))
            .unwrap_or(SPLIT_BUTTON_INNER_CORNER_RADIUS);

        let h = f64::from(height);
        let leading_w = f64::from(width);
        let leading_relaxed = ShapeKey::from_path(
            &RoundedRect::new(0.0, 0.0, leading_w, h, rest_radius).to_path(0.1),
        );
        let leading_tightened = ShapeKey::from_path(
            &RoundedRect::new(
                0.0,
                0.0,
                leading_w,
                h,
                (rest_radius, tightened_radius, tightened_radius, rest_radius),
            )
            .to_path(0.1),
        );
        if let Some(node) = tree.get_mut(leading) {
            node.paint.shape = Animated::new(leading_relaxed.clone());
            node.paint.interactive_shape = Some((leading_relaxed, leading_tightened));
        }

        let trailing_relaxed =
            ShapeKey::from_path(&RoundedRect::new(0.0, 0.0, h, h, rest_radius).to_path(0.1));
        let trailing_tightened = ShapeKey::from_path(
            &RoundedRect::new(
                0.0,
                0.0,
                h,
                h,
                (tightened_radius, rest_radius, rest_radius, tightened_radius),
            )
            .to_path(0.1),
        );
        if let Some(node) = tree.get_mut(trailing) {
            node.paint.background = Animated::new(colors.container);
            node.paint.corner_radius = Animated::new(rest_radius);
            node.paint.elevation = Animated::new(colors.elevation);
            node.paint.border_color = Animated::new(colors.border_color);
            node.paint.border_width = Animated::new(colors.border_width);
            node.paint.shape = Animated::new(trailing_relaxed.clone());
            node.paint.interactive_shape = Some((trailing_relaxed, trailing_tightened));
        }
        if let Some(node) = tree.get_mut(icon_id)
            && let NodeKind::Icon(state) = &mut node.kind
        {
            state.tint = colors.label;
        }
    })
}

/// M52 Phase 3: `add_button_group`'s own hook, closing Phase 3 -- the
/// same emergent-composition reasoning `add_split_button`'s own hook
/// uses: every child already has its own `button_retheme_hook`
/// (registered inside each `self.add_button(...)` call in the loop)
/// correctly re-resolving background/corner_radius/elevation/border
/// live. This hook only layers `paint.shape`/`press_interactive_shape`
/// on top (disjoint fields) for every child at once -- `press_
/// interactive_shape`, not `interactive_shape` (`add_split_button`'s
/// own field), the real, distinct field this component's own hover/
/// press mechanic uses. `tightened_radius`'s own fallback is a real
/// formula over `height` (`button_group_pressed_corner_radius`), not a
/// fixed constant, reproduced exactly.
fn button_group_retheme_hook(
    children: Vec<NodeId>,
    variant: String,
    width: f32,
    height: f32,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let group_h = f64::from(height);
        let group_w = f64::from(width);
        let rest_radius = theme
            .shape("button", Some(&variant))
            .unwrap_or(group_h / 2.0);
        let tightened_radius = theme
            .shape("button_group", Some("tightened"))
            .unwrap_or_else(|| button_group_pressed_corner_radius(group_h));
        let child_relaxed = ShapeKey::from_path(
            &RoundedRect::new(0.0, 0.0, group_w, group_h, rest_radius).to_path(0.1),
        );
        let child_tightened = ShapeKey::from_path(
            &RoundedRect::new(0.0, 0.0, group_w, group_h, tightened_radius).to_path(0.1),
        );
        for &child in &children {
            if let Some(node) = tree.get_mut(child) {
                node.paint.shape = Animated::new(child_relaxed.clone());
                node.paint.press_interactive_shape =
                    Some((child_relaxed.clone(), child_tightened.clone()));
            }
        }
    })
}

// M52 Phase 4: conditional/variable multi-node Containers & Navigation.

/// M52 Phase 4: `add_snackbar`'s own hook -- `action`/`close` are each
/// independently `Option<(container, child)>`, since a snackbar may
/// have neither, either, or both, fixed at construction time (a
/// snackbar's own anatomy never gains/loses these after being built).
fn snackbar_retheme_hook(
    container: NodeId,
    text: NodeId,
    action: Option<(NodeId, NodeId)>,
    close: Option<(NodeId, NodeId)>,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let role = |theme: &crate::window::ThemeState, name: &str, fallback: Color| -> Color {
            if theme.is_set() {
                theme.role(name).unwrap_or(fallback)
            } else {
                fallback
            }
        };
        let container_color = role(theme, "inverse_surface", Md3Baseline::INVERSE_SURFACE);
        let text_color = role(theme, "inverse_on_surface", Md3Baseline::INVERSE_ON_SURFACE);
        let action_color = role(theme, "inverse_primary", Md3Baseline::INVERSE_PRIMARY);
        let icon_color = text_color;
        let corner_radius = theme
            .shape("snackbar", None)
            .unwrap_or(SNACKBAR_CORNER_RADIUS);
        let elevation = theme
            .elevation("snackbar", None)
            .unwrap_or(SNACKBAR_ELEVATION);
        if let Some(node) = tree.get_mut(container) {
            node.paint.background = Animated::new(container_color);
            node.paint.corner_radius = Animated::new(corner_radius);
            node.paint.elevation = Animated::new(elevation);
        }
        if let Some(node) = tree.get_mut(text) {
            node.paint.background = Animated::new(text_color);
        }
        if let Some((_, label)) = action
            && let Some(node) = tree.get_mut(label)
        {
            node.paint.background = Animated::new(action_color);
        }
        if let Some((_, icon)) = close
            && let Some(node) = tree.get_mut(icon)
            && let NodeKind::Icon(state) = &mut node.kind
        {
            state.tint = icon_color;
        }
    })
}

/// M52 Phase 4: `add_side_sheet`'s own hook -- `scrim` is `Some` only
/// for the `modal=true` branch. Both branches share the identical
/// `corner_radii_override`/`elevation`/`variant_key` resolution
/// `add_side_sheet` itself computes once, before branching -- only
/// `container_color` genuinely differs by branch.
fn side_sheet_retheme_hook(
    panel: NodeId,
    scrim: Option<NodeId>,
    modal: bool,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let role = |theme: &crate::window::ThemeState, name: &str, fallback: Color| -> Color {
            if theme.is_set() {
                theme.role(name).unwrap_or(fallback)
            } else {
                fallback
            }
        };
        let (container_color, scrim_color) = if modal {
            (
                role(
                    theme,
                    "surface_container_low",
                    Md3Baseline::SURFACE_CONTAINER_LOW,
                ),
                role(theme, "scrim", Md3Baseline::SCRIM),
            )
        } else {
            (
                role(theme, "surface", Md3Baseline::SURFACE),
                Md3Baseline::SCRIM,
            )
        };
        let variant_key = if modal { "modal" } else { "standard" };
        let default_elevation = if modal {
            SIDE_SHEET_MODAL_ELEVATION
        } else {
            SIDE_SHEET_STANDARD_ELEVATION
        };
        let corner_radius = theme
            .shape("side_sheet", Some(variant_key))
            .unwrap_or(SIDE_SHEET_CORNER_RADIUS);
        let elevation = theme
            .elevation("side_sheet", Some(variant_key))
            .unwrap_or(default_elevation);
        if let Some(node) = tree.get_mut(panel) {
            node.paint.background = Animated::new(container_color);
            node.paint.elevation = Animated::new(elevation);
            node.paint.corner_radii_override = Some([corner_radius, 0.0, 0.0, corner_radius]);
        }
        if let Some(id) = scrim
            && let Some(node) = tree.get_mut(id)
        {
            node.paint.background = Animated::new(scrim_color);
        }
    })
}

/// M52 Phase 4: `add_navigation_drawer`'s own hook -- `scrim` is `Some`
/// only for `modal=true`. Each item's own `is_active` is real, app-
/// owned selection state (Design Principle 6), captured at
/// construction time, never re-derived -- matching `add_date_picker_
/// day`'s established convention. Its own real `font_weight` co-varies
/// with `is_active` but is never theme-derived, so this hook correctly
/// never touches it (the same real distinction the audit that scoped
/// this milestone already drew).
fn navigation_drawer_retheme_hook(
    panel: NodeId,
    scrim: Option<NodeId>,
    items: Vec<(NodeId, NodeId, NodeId, bool)>,
    modal: bool,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let role = |theme: &crate::window::ThemeState, name: &str, fallback: Color| -> Color {
            if theme.is_set() {
                theme.role(name).unwrap_or(fallback)
            } else {
                fallback
            }
        };
        let container_color = if modal {
            role(
                theme,
                "surface_container_low",
                Md3Baseline::SURFACE_CONTAINER_LOW,
            )
        } else {
            role(theme, "surface", Md3Baseline::SURFACE)
        };
        let indicator_color = role(
            theme,
            "secondary_container",
            Md3Baseline::SECONDARY_CONTAINER,
        );
        let active_color = role(
            theme,
            "on_secondary_container",
            Md3Baseline::ON_SECONDARY_CONTAINER,
        );
        let inactive_color = role(theme, "on_surface_variant", Md3Baseline::ON_SURFACE_VARIANT);
        let scrim_color = role(theme, "scrim", Md3Baseline::SCRIM);
        let variant_key = if modal { "modal" } else { "standard" };
        let default_elevation = if modal {
            SIDE_SHEET_MODAL_ELEVATION
        } else {
            SIDE_SHEET_STANDARD_ELEVATION
        };
        let corner_radius = theme
            .shape("navigation_drawer", Some(variant_key))
            .unwrap_or(SIDE_SHEET_CORNER_RADIUS);
        let elevation = theme
            .elevation("navigation_drawer", Some(variant_key))
            .unwrap_or(default_elevation);
        let indicator_corner_radius = theme
            .shape("navigation_drawer", Some("indicator"))
            .unwrap_or(NAV_DRAWER_INDICATOR_CORNER_RADIUS);
        if let Some(node) = tree.get_mut(panel) {
            node.paint.background = Animated::new(container_color);
            node.paint.elevation = Animated::new(elevation);
            node.paint.corner_radii_override = Some([0.0, corner_radius, corner_radius, 0.0]);
        }
        if let Some(id) = scrim
            && let Some(node) = tree.get_mut(id)
        {
            node.paint.background = Animated::new(scrim_color);
        }
        for &(indicator, icon, label, is_active) in &items {
            let fill = if is_active {
                indicator_color
            } else {
                TRANSPARENT
            };
            let item_color = if is_active {
                active_color
            } else {
                inactive_color
            };
            if let Some(node) = tree.get_mut(indicator) {
                node.paint.background = Animated::new(fill);
                node.paint.corner_radius = Animated::new(indicator_corner_radius);
            }
            if let Some(node) = tree.get_mut(icon)
                && let NodeKind::Icon(state) = &mut node.kind
            {
                state.tint = item_color;
            }
            if let Some(node) = tree.get_mut(label) {
                node.paint.background = Animated::new(item_color);
            }
        }
    })
}

/// M52 Phase 4: `add_top_app_bar`'s own hook -- the bar's own frame
/// stays un-themed (real MD3 anatomy is always square, deliberate, no
/// `theme.shape` call at all), only its `background` role is re-
/// resolved. `leading`/each `trailing` icon-button container reuses
/// the `"icon_button"` key, matching `add_top_app_bar` itself.
fn top_app_bar_retheme_hook(
    bar: NodeId,
    headline: NodeId,
    leading: Option<(NodeId, NodeId)>,
    trailing: Vec<(NodeId, NodeId)>,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let container_color = if theme.is_set() {
            theme.role("surface").unwrap_or(Md3Baseline::SURFACE)
        } else {
            Md3Baseline::SURFACE
        };
        let leading_icon_color = theme.on_surface();
        let trailing_icon_color = if theme.is_set() {
            theme
                .role("on_surface_variant")
                .unwrap_or(Md3Baseline::ON_SURFACE_VARIANT)
        } else {
            Md3Baseline::ON_SURFACE_VARIANT
        };
        let icon_button_corner_radius = theme
            .shape("icon_button", None)
            .unwrap_or(TOP_APP_BAR_ICON_BUTTON_SIZE as f64 / 2.0);
        if let Some(node) = tree.get_mut(bar) {
            node.paint.background = Animated::new(container_color);
        }
        if let Some(node) = tree.get_mut(headline) {
            node.paint.background = Animated::new(leading_icon_color);
        }
        if let Some((container, icon)) = leading {
            if let Some(node) = tree.get_mut(container) {
                node.paint.corner_radius = Animated::new(icon_button_corner_radius);
            }
            if let Some(node) = tree.get_mut(icon)
                && let NodeKind::Icon(state) = &mut node.kind
            {
                state.tint = leading_icon_color;
            }
        }
        for &(container, icon) in &trailing {
            if let Some(node) = tree.get_mut(container) {
                node.paint.corner_radius = Animated::new(icon_button_corner_radius);
            }
            if let Some(node) = tree.get_mut(icon)
                && let NodeKind::Icon(state) = &mut node.kind
            {
                state.tint = trailing_icon_color;
            }
        }
    })
}

/// M52 Phase 4: `add_navigation_rail`'s own hook -- the frame's own
/// corner radius stays un-themed (real MD3 anatomy is deliberately
/// "corner-none" here), only its `background` role and each item's own
/// indicator/icon/label are re-resolved. `is_active` is real, app-owned
/// selection state, captured at construction time, never re-derived.
fn navigation_rail_retheme_hook(
    frame: NodeId,
    items: Vec<(NodeId, NodeId, NodeId, bool)>,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let role = |theme: &crate::window::ThemeState, name: &str, fallback: Color| -> Color {
            if theme.is_set() {
                theme.role(name).unwrap_or(fallback)
            } else {
                fallback
            }
        };
        let container_color = role(theme, "surface", Md3Baseline::SURFACE);
        let indicator_color = role(
            theme,
            "secondary_container",
            Md3Baseline::SECONDARY_CONTAINER,
        );
        let active_icon_color = role(
            theme,
            "on_secondary_container",
            Md3Baseline::ON_SECONDARY_CONTAINER,
        );
        let on_surface_variant = role(theme, "on_surface_variant", Md3Baseline::ON_SURFACE_VARIANT);
        let active_label_color = theme.on_surface();
        let indicator_corner_radius = theme
            .shape("navigation_rail", Some("indicator"))
            .unwrap_or(NAV_RAIL_INDICATOR_CORNER_RADIUS);
        if let Some(node) = tree.get_mut(frame) {
            node.paint.background = Animated::new(container_color);
        }
        for &(indicator, icon, label, is_active) in &items {
            let fill = if is_active {
                indicator_color
            } else {
                TRANSPARENT
            };
            let icon_color = if is_active {
                active_icon_color
            } else {
                on_surface_variant
            };
            let label_color = if is_active {
                active_label_color
            } else {
                on_surface_variant
            };
            if let Some(node) = tree.get_mut(indicator) {
                node.paint.background = Animated::new(fill);
                node.paint.corner_radius = Animated::new(indicator_corner_radius);
            }
            if let Some(node) = tree.get_mut(icon)
                && let NodeKind::Icon(state) = &mut node.kind
            {
                state.tint = icon_color;
            }
            if let Some(node) = tree.get_mut(label) {
                node.paint.background = Animated::new(label_color);
            }
        }
    })
}

/// M52 Phase 4: `add_tabs`'s own hook -- the row's own frame stays
/// un-themed (real MD3 anatomy is deliberately flat/square), only its
/// `background` and each tab's own icon (optional)/label/indicator are
/// re-resolved. `is_active` is real, app-owned selection state,
/// captured at construction time.
fn tabs_retheme_hook(
    row: NodeId,
    tabs: Vec<(Option<NodeId>, NodeId, NodeId, bool)>,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let role = |theme: &crate::window::ThemeState, name: &str, fallback: Color| -> Color {
            if theme.is_set() {
                theme.role(name).unwrap_or(fallback)
            } else {
                fallback
            }
        };
        let container_color = role(theme, "surface", Md3Baseline::SURFACE);
        let active_color = role(theme, "primary", Md3Baseline::PRIMARY);
        let inactive_color = role(theme, "on_surface_variant", Md3Baseline::ON_SURFACE_VARIANT);
        let indicator_corner_radius = theme
            .shape("tabs", Some("indicator"))
            .unwrap_or(TAB_INDICATOR_CORNER_RADIUS);
        if let Some(node) = tree.get_mut(row) {
            node.paint.background = Animated::new(container_color);
        }
        for &(icon, label, indicator, is_active) in &tabs {
            let color = if is_active {
                active_color
            } else {
                inactive_color
            };
            if let Some(id) = icon
                && let Some(node) = tree.get_mut(id)
                && let NodeKind::Icon(state) = &mut node.kind
            {
                state.tint = color;
            }
            if let Some(node) = tree.get_mut(label) {
                node.paint.background = Animated::new(color);
            }
            if let Some(node) = tree.get_mut(indicator) {
                let indicator_fill = if is_active { active_color } else { TRANSPARENT };
                node.paint.background = Animated::new(indicator_fill);
                node.paint.corner_radii_override =
                    Some([indicator_corner_radius, indicator_corner_radius, 0.0, 0.0]);
            }
        }
    })
}

/// M52 Phase 4: `add_search_bar`'s own hook -- **a real, pre-existing
/// inconsistency reproduced as-is, not silently fixed:** the leading/
/// trailing icon-button containers' own `corner_radius` is a hardcoded
/// `SEARCH_ICON_BUTTON_SIZE / 2.0` literal at construction time, never
/// looked up via `theme.shape("icon_button", ...)` the way `add_top_
/// app_bar`/`add_spin_box`'s own visually-identical icon buttons are --
/// so this hook correctly never touches their `corner_radius` at all,
/// only their `IconState.tint`. `field_id`'s own `text_tint` reads
/// `on_surface()` unconditionally (no `is_set()` gate), the same minor
/// pre-existing pattern `add_time_input_field`'s hook already
/// reproduces as-is.
fn search_bar_retheme_hook(
    bar: NodeId,
    field: NodeId,
    leading: Option<(NodeId, NodeId)>,
    trailing: Vec<(NodeId, NodeId)>,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let role = |theme: &crate::window::ThemeState, name: &str, fallback: Color| -> Color {
            if theme.is_set() {
                theme.role(name).unwrap_or(fallback)
            } else {
                fallback
            }
        };
        let container_color = role(
            theme,
            "surface_container_high",
            Md3Baseline::SURFACE_CONTAINER_HIGH,
        );
        let leading_icon_color = theme.on_surface();
        let trailing_icon_color =
            role(theme, "on_surface_variant", Md3Baseline::ON_SURFACE_VARIANT);
        let corner_radius = theme
            .shape("search_bar", None)
            .unwrap_or(SEARCH_BAR_CORNER_RADIUS);
        let elevation = theme
            .elevation("search_bar", None)
            .unwrap_or(SEARCH_BAR_ELEVATION);
        if let Some(node) = tree.get_mut(bar) {
            node.paint.background = Animated::new(container_color);
            node.paint.corner_radius = Animated::new(corner_radius);
            node.paint.elevation = Animated::new(elevation);
        }
        if let Some(node) = tree.get_mut(field)
            && let NodeKind::TextField(state) = &mut node.kind
        {
            state.text_tint = theme.on_surface();
        }
        if let Some((_, icon)) = leading
            && let Some(node) = tree.get_mut(icon)
            && let NodeKind::Icon(state) = &mut node.kind
        {
            state.tint = leading_icon_color;
        }
        for &(_, icon) in &trailing {
            if let Some(node) = tree.get_mut(icon)
                && let NodeKind::Icon(state) = &mut node.kind
            {
                state.tint = trailing_icon_color;
            }
        }
    })
}

/// M52 Phase 4: `add_pagination`'s own hook, closing Phase 4 -- **a
/// real, pre-existing inconsistency reproduced as-is:** `previous`/
/// `next`'s own corner_radius shares the *same* single `"pagination"`
/// key every page item uses, not the `"icon_button"` key
/// `add_top_app_bar`/`add_spin_box`'s own visually-identical icon
/// buttons use -- confirmed at construction time and reproduced
/// unchanged here, not silently "fixed" to a different key.
/// `is_selected` is real, app-owned state, captured at construction.
fn pagination_retheme_hook(
    previous: NodeId,
    previous_icon: NodeId,
    next: NodeId,
    next_icon: NodeId,
    pages: Vec<(NodeId, NodeId, bool)>,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let role = |theme: &crate::window::ThemeState, name: &str, fallback: Color| -> Color {
            if theme.is_set() {
                theme.role(name).unwrap_or(fallback)
            } else {
                fallback
            }
        };
        let selected_fill = role(theme, "primary", Md3Baseline::PRIMARY);
        let selected_label = role(theme, "on_primary", Md3Baseline::ON_PRIMARY);
        let on_surface_variant = role(theme, "on_surface_variant", Md3Baseline::ON_SURFACE_VARIANT);
        let corner_radius = theme
            .shape("pagination", None)
            .unwrap_or(PAGE_ITEM_CORNER_RADIUS);
        for button in [previous, next] {
            if let Some(node) = tree.get_mut(button) {
                node.paint.corner_radius = Animated::new(corner_radius);
            }
        }
        for icon in [previous_icon, next_icon] {
            if let Some(node) = tree.get_mut(icon)
                && let NodeKind::Icon(state) = &mut node.kind
            {
                state.tint = on_surface_variant;
            }
        }
        for &(item, label, is_selected) in &pages {
            let (fill, label_color) = if is_selected {
                (selected_fill, selected_label)
            } else {
                (TRANSPARENT, on_surface_variant)
            };
            if let Some(node) = tree.get_mut(item) {
                node.paint.background = Animated::new(fill);
                node.paint.corner_radius = Animated::new(corner_radius);
            }
            if let Some(node) = tree.get_mut(label) {
                node.paint.background = Animated::new(label_color);
            }
        }
    })
}

/// M52 Phase 4: `add_menu_item`'s own hook -- `icon`/`chevron` are each
/// independently `Option<NodeId>`. **A real, confirmed, pre-existing
/// gap named but deliberately not fixed here:** `build_menu`'s own
/// panel (the real consumer of `add_menu_item`, not itself an `add_*`
/// factory) hardcodes `MENU_PANEL_CORNER_RADIUS`/`MENU_PANEL_ELEVATION`
/// with no `theme.shape`/`elevation` call at all -- there is nothing
/// for a retheme hook to recompute for the panel's own shape/elevation
/// today (only `add_menu_item`'s own real `background` roles are in
/// scope for this milestone).
fn menu_item_retheme_hook(
    label: NodeId,
    icon: Option<NodeId>,
    chevron: Option<NodeId>,
) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let icon_color = if theme.is_set() {
            theme
                .role("on_surface_variant")
                .unwrap_or(Md3Baseline::ON_SURFACE_VARIANT)
        } else {
            Md3Baseline::ON_SURFACE_VARIANT
        };
        if let Some(node) = tree.get_mut(label) {
            node.paint.background = Animated::new(theme.on_surface());
        }
        for id in [icon, chevron].into_iter().flatten() {
            if let Some(node) = tree.get_mut(id)
                && let NodeKind::Icon(state) = &mut node.kind
            {
                state.tint = icon_color;
            }
        }
    })
}

// M52 Phase 5: non-`PaintProperties` stateful components. **Real,
// verified-before-writing-code finding: `add_checkbox`/`add_slider`/
// `add_text_field`/`add_code_editor` need NO new hook at all.** Each
// one's own themed field (`CheckboxState.mark_tint`/`SliderState.
// track_tint`/`TextFieldState.text_tint`) is already the exact field
// `Tree::set_all_component_tints` (the pre-existing mechanism, left
// untouched since Phase 1) unconditionally overwrites with `on_
// surface()` on every real `set_theme` call -- confirmed by direct
// read of `set_all_component_tints`'s own match arms
// (`engine-core/src/tree.rs`) before writing anything here, not
// assumed. Writing a redundant hook for these four would duplicate
// work the existing mechanism already does correctly. Only the 5
// components `set_all_component_tints` was confirmed to NOT reach at
// all get real, new hooks below.

/// M52 Phase 5: `add_radio_button`'s own hook -- the real gap `Tree::
/// set_all_component_tints`'s own doc comment already names directly
/// ("a radio button genuinely needs two different real roles... which
/// that single-`Color`-parameter mechanism can't express"). Confirmed,
/// previously-undocumented: before this milestone, `Window.set_theme`
/// had zero effect on an already-built radio button at all.
fn radio_button_retheme_hook(id: NodeId) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let role = |theme: &crate::window::ThemeState, name: &str, fallback: Color| -> Color {
            if theme.is_set() {
                theme.role(name).unwrap_or(fallback)
            } else {
                fallback
            }
        };
        let unselected_tint = role(theme, "outline", Md3Baseline::OUTLINE);
        let selected_tint = role(theme, "primary", Md3Baseline::PRIMARY);
        if let Some(node) = tree.get_mut(id)
            && let NodeKind::RadioButton(state) = &mut node.kind
        {
            state.unselected_tint = unselected_tint;
            state.selected_tint = selected_tint;
        }
    })
}

/// M52 Phase 5: `add_switch`'s own hook -- 5 independent real roles,
/// confirmed previously completely untouched by `Window.set_theme`
/// (unlike `Checkbox`/`Slider`/`TextField`, `Switch` was never even
/// partially covered by the existing tint-push mechanism).
fn switch_retheme_hook(id: NodeId) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let role = |theme: &crate::window::ThemeState, name: &str, fallback: Color| -> Color {
            if theme.is_set() {
                theme.role(name).unwrap_or(fallback)
            } else {
                fallback
            }
        };
        let track_off_tint = role(
            theme,
            "surface_container_highest",
            Md3Baseline::SURFACE_CONTAINER_HIGHEST,
        );
        let track_on_tint = role(theme, "primary", Md3Baseline::PRIMARY);
        let track_outline_tint = role(theme, "outline", Md3Baseline::OUTLINE);
        let handle_off_tint = role(theme, "outline", Md3Baseline::OUTLINE);
        let handle_on_tint = role(theme, "on_primary", Md3Baseline::ON_PRIMARY);
        if let Some(node) = tree.get_mut(id)
            && let NodeKind::Switch(state) = &mut node.kind
        {
            state.track_off_tint = track_off_tint;
            state.track_on_tint = track_on_tint;
            state.track_outline_tint = track_outline_tint;
            state.handle_off_tint = handle_off_tint;
            state.handle_on_tint = handle_on_tint;
        }
    })
}

/// M52 Phase 5: `add_linear_progress`'s own hook -- confirmed
/// previously untouched by `Window.set_theme`.
fn linear_progress_retheme_hook(id: NodeId) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let role = |theme: &crate::window::ThemeState, name: &str, fallback: Color| -> Color {
            if theme.is_set() {
                theme.role(name).unwrap_or(fallback)
            } else {
                fallback
            }
        };
        let track_tint = role(
            theme,
            "surface_container_highest",
            Md3Baseline::SURFACE_CONTAINER_HIGHEST,
        );
        let indicator_tint = role(theme, "primary", Md3Baseline::PRIMARY);
        if let Some(node) = tree.get_mut(id)
            && let NodeKind::LinearProgress(state) = &mut node.kind
        {
            state.track_tint = track_tint;
            state.indicator_tint = indicator_tint;
        }
    })
}

/// M52 Phase 5: `add_circular_progress`'s own hook -- confirmed
/// previously untouched by `Window.set_theme`.
fn circular_progress_retheme_hook(id: NodeId) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let indicator_tint = if theme.is_set() {
            theme.role("primary").unwrap_or(Md3Baseline::PRIMARY)
        } else {
            Md3Baseline::PRIMARY
        };
        if let Some(node) = tree.get_mut(id)
            && let NodeKind::CircularProgress(state) = &mut node.kind
        {
            state.indicator_tint = indicator_tint;
        }
    })
}

/// M52 Phase 5: `add_time_picker_dial`'s own hook, closing Phase 5 --
/// confirmed previously untouched by `Window.set_theme`.
fn time_picker_dial_retheme_hook(id: NodeId) -> crate::window::RetitheHook {
    Box::new(move |theme, tree| {
        let role = |theme: &crate::window::ThemeState, name: &str, fallback: Color| -> Color {
            if theme.is_set() {
                theme.role(name).unwrap_or(fallback)
            } else {
                fallback
            }
        };
        let face_tint = role(
            theme,
            "surface_container_highest",
            Md3Baseline::SURFACE_CONTAINER_HIGHEST,
        );
        let hand_tint = role(theme, "primary", Md3Baseline::PRIMARY);
        if let Some(node) = tree.get_mut(id)
            && let NodeKind::TimePickerDial(state) = &mut node.kind
        {
            state.face_tint = face_tint;
            state.hand_tint = hand_tint;
        }
    })
}

#[pymethods]
impl PyWindow {
    /// §14 step 6's own "node creation" -- one shape (a colored rect, a
    /// child of this window's implicit root row) is the real minimal
    /// slice; unchanged by the `PyWindow` split, just moved here with
    /// `App` itself.
    /// M48 (§5, §7): `border_color`/`border_width` -- real `PaintProperties`
    /// fields since M30 Phase 1 -- were never settable anywhere in the
    /// imperative API, confirmed via grep before this change (only
    /// specific MD3 component factories, e.g. Outlined Button, ever
    /// baked a border in internally). `add_rect` is this file's one
    /// genuinely generic "colored box" factory (no `add_container`
    /// exists in the imperative API at all), so it's the representative
    /// case for this milestone; extending border kwargs to every other
    /// factory is a natural, separate follow-up.
    #[pyo3(signature = (background, width, height, x=None, y=None, border_color=None, border_width=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_rect(
        &self,
        background: (u8, u8, u8, u8),
        width: f32,
        height: f32,
        x: Option<f32>,
        y: Option<f32>,
        border_color: Option<(u8, u8, u8, u8)>,
        border_width: Option<f64>,
    ) -> Node {
        let (r, g, b, a) = background;
        let mut tree = self.tree.borrow_mut();
        let mut paint = PaintProperties::new(Color::from_rgba8(r, g, b, a), 0.0, 0.0, 1.0);
        if let Some((br, bg, bb, ba)) = border_color {
            paint.border_color = Animated::new(Color::from_rgba8(br, bg, bb, ba));
        }
        if let Some(border_width) = border_width {
            paint.border_width = Animated::new(border_width);
        }
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
            paint,
        );
        tree.add_child(self.root, id);
        self.wrap_node(id)
    }

    /// M27 Phase 2 (§5): a real, genuine gap found while building the
    /// showcase demo's component gallery screen -- `NodeKind::Text` has
    /// been fully real and renderable since §14 step 4 (`TextRenderer`,
    /// `engine-render`), and declarative `kind: Text` in a `view.yaml`
    /// has built it since §14 step 5, but `Window` (the imperative path)
    /// had no way to create one at all, confirmed via grep before this
    /// method existed. Mirrors `add_rect`'s own real shape exactly --
    /// `background` is repurposed as the glyph color for a plain
    /// `NodeKind::Text` (no visible box of its own), the identical real
    /// convention `paint_node`'s own `NodeKind::Text` arm and the
    /// declarative `required_background(..., "Text")` path both already
    /// establish -- not a new convention invented here. `width`/`height`
    /// are required, the same as every other `add_*` method except
    /// `add_icon` (a single `size`) -- no measure-function/intrinsic-
    /// sizing wiring exists for `Text` to lean on instead, confirmed
    /// before choosing this shape rather than assumed.
    #[pyo3(signature = (content, background, width, height, font_family="Roboto", font_weight=400.0, font_size=16.0, x=None, y=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_text(
        &self,
        content: &str,
        background: (u8, u8, u8, u8),
        width: f32,
        height: f32,
        font_family: &str,
        font_weight: f32,
        font_size: f32,
        x: Option<f32>,
        y: Option<f32>,
    ) -> Node {
        let (r, g, b, a) = background;
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Text(TextState {
                content: content.to_string(),
                font_family: font_family.to_string(),
                font_weight,
                font_size,
                align: TextAlign::Start,
            }),
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
        self.wrap_node(id)
    }

    /// M30 Phase 1 (§5, §7): `Button`, MD3's five real variants --
    /// the component this catalog's own research confirmed was, until
    /// now, hand-composed from `Rect`+`Text`+ripple in every example
    /// (`ripple_button.py`'s own real precedent), duplicated at every
    /// call site instead of built once. Real anatomy: a `Rect`
    /// container (background/border/elevation resolved by `variant`
    /// through `resolve_button_colors`, corner radius `height / 2.0` --
    /// MD3's own "Full" shape family every button variant uses) with
    /// one centered `Text` child (`TextAlign::Center`, M30 Phase 1's
    /// own new real capability -- see `engine-render/tests/text_align.
    /// rs`) sized to the container's own inner content width, `align_
    /// items: Center` giving the label real cross-axis room to center
    /// vertically too. Returns the *container* `Node` -- the same real
    /// thing every other `add_*` returns, so `set_on_click`/`enable_
    /// interaction`/`animate` all work on a button exactly like any
    /// other node, no new API surface needed for those. Deliberately
    /// does **not** auto-call `enable_interaction()` -- Design
    /// Principle 6's "only a node that opts in pays the cost" applies
    /// here exactly as it does to every other node `add_checkbox`/
    /// `add_slider`/etc already hand back un-interactive by default.
    #[pyo3(signature = (label, width, height, variant="filled", x=None, y=None))]
    fn add_button(
        &self,
        label: &str,
        width: f32,
        height: f32,
        variant: &str,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        let colors = resolve_button_colors(&self.theme.borrow(), variant, "button")?;
        let corner_radius = self
            .theme
            .borrow()
            .shape("button", Some(variant))
            .unwrap_or_else(|| f64::from(height) / 2.0);
        let mut tree = self.tree.borrow_mut();

        let mut container_paint =
            PaintProperties::new(colors.container, corner_radius, colors.elevation, 1.0);
        container_paint.border_color = Animated::new(colors.border_color);
        container_paint.border_width = Animated::new(colors.border_width);
        let mut container_style = positioned_style(
            Size {
                width: length(width),
                height: length(height),
            },
            x,
            y,
        );
        container_style.display = taffy::Display::Flex;
        container_style.align_items = Some(AlignItems::CENTER);
        let container = tree.insert(NodeKind::Rect, container_style, container_paint);

        let label_width = (width - 2.0 * BUTTON_HORIZONTAL_PADDING).max(0.0);
        let label_id = tree.insert(
            NodeKind::Text(TextState {
                content: label.to_string(),
                font_family: "Roboto".to_string(),
                font_weight: BUTTON_LABEL_FONT_WEIGHT,
                font_size: BUTTON_LABEL_FONT_SIZE,
                align: TextAlign::Center,
            }),
            Style {
                size: Size {
                    width: length(label_width),
                    height: length(BUTTON_LABEL_LINE_HEIGHT),
                },
                ..Default::default()
            },
            PaintProperties::new(colors.label, 0.0, 0.0, 1.0),
        );
        tree.add_child(container, label_id);
        tree.add_child(self.root, container);
        drop(tree);
        // M52 Phase 1 (§7.1, §7.3): registers this button's own live
        // re-theme hook, so a later `Window.set_theme(...)` recomputes
        // and overwrites `container`/`label_id`'s real color/corner_
        // radius/elevation in place, not just at construction time.
        self.retheme_hooks.borrow_mut().push(button_retheme_hook(
            container,
            label_id,
            variant.to_string(),
            height,
        ));
        Ok(self.wrap_node(container))
    }

    /// M30 Phase 1 (§5, §7): `Icon Button`, `Button`'s own real anatomy
    /// (`add_button`'s doc comment) with a centered `Icon` child
    /// (`Window.add_icon`'s own already-real curated icon set,
    /// `engine_md3::icons`) instead of `Text`. Real MD3 spec's own
    /// four variants -- Filled/Filled Tonal/Outlined/Standard, not
    /// `Button`'s five -- there is no "Elevated Icon Button" in MD3's
    /// own vocabulary, and MD3 calls its transparent variant
    /// "Standard" here, not "Text" (`Button`'s own name for the
    /// identical transparent-container/primary-tint anatomy). Reuses
    /// `resolve_button_colors` for the actual paint (`"standard"`
    /// translates to `resolve_button_colors`'s own `"text"` -- the two
    /// names describe the same real colors, kept distinct only because
    /// that's each component's own real MD3 terminology), rather than
    /// a second, near-duplicate color table. `size` is the container's
    /// own real touch-target box (MD3's own default is 40.0); the icon
    /// itself stays a fixed real MD3 token (24dp) regardless of `size`,
    /// centered on both axes via `justify_content`/`align_items`
    /// (`TextAlign::Center`'s own icon-anatomy counterpart isn't
    /// needed here -- an `Icon`'s own box is already exactly its own
    /// glyph's bounds, so plain two-axis flex centering is the real,
    /// sufficient answer, not a second alignment concept). Deliberately
    /// does **not** auto-call `enable_interaction()`, matching every
    /// other `add_*` precedent including `add_button` itself.
    #[pyo3(signature = (icon, size=40.0, variant="standard", x=None, y=None))]
    fn add_icon_button(
        &self,
        icon: &str,
        size: f32,
        variant: &str,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        let resolved_variant = match variant {
            "standard" => "text",
            "filled" | "filled_tonal" | "outlined" => variant,
            other => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "unknown icon button variant {other:?} -- expected one of \"filled\", \
                     \"filled_tonal\", \"outlined\", \"standard\""
                )));
            }
        };
        let colors = resolve_button_colors(&self.theme.borrow(), resolved_variant, "icon_button")?;
        let path = resolve_icon_path(icon)?;
        // Real, deliberate consistency fix caught before this shipped:
        // the shape key must use the same `resolved_variant` ("standard"
        // -> "text") the elevation lookup above already does, not the
        // raw, un-translated `variant` -- otherwise overriding
        // "icon_button.standard" would silently miss elevation (keyed
        // under "icon_button.text" instead), a real, confusing mismatch
        // for a theme author who only set one of the two.
        let corner_radius = self
            .theme
            .borrow()
            .shape("icon_button", Some(resolved_variant))
            .unwrap_or_else(|| f64::from(size) / 2.0);

        let mut tree = self.tree.borrow_mut();

        let mut container_paint =
            PaintProperties::new(colors.container, corner_radius, colors.elevation, 1.0);
        container_paint.border_color = Animated::new(colors.border_color);
        container_paint.border_width = Animated::new(colors.border_width);
        let mut container_style = positioned_style(
            Size {
                width: length(size),
                height: length(size),
            },
            x,
            y,
        );
        container_style.display = taffy::Display::Flex;
        container_style.justify_content = Some(JustifyContent::CENTER);
        container_style.align_items = Some(AlignItems::CENTER);
        let container = tree.insert(NodeKind::Rect, container_style, container_paint);

        let icon_id = tree.insert(
            NodeKind::Icon(IconState::new(path, colors.label)),
            Style {
                size: Size {
                    width: length(ICON_BUTTON_ICON_SIZE),
                    height: length(ICON_BUTTON_ICON_SIZE),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(container, icon_id);
        tree.add_child(self.root, container);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(icon_button_retheme_hook(
                container,
                icon_id,
                resolved_variant.to_string(),
                size,
            ));
        Ok(self.wrap_node(container))
    }

    /// M30 Phase 1 Step 3 (§5, §7): `FAB` (Floating Action Button),
    /// MD3's own real three sizes (`fab_shape`) and four color
    /// variants (`resolve_fab_colors`) -- `Icon Button`'s own anatomy
    /// (a `Rect` container, one centered `Icon` child) with the size/
    /// shape pairing and color system that are genuinely `FAB`'s own,
    /// not reused from `Button`/`Icon Button` (confirmed against
    /// Material Web's own token source before writing this, not
    /// assumed transferable). Unlike `Icon Button`, every real `FAB`
    /// variant carries a real rest-state elevation (level 3) --
    /// `FAB` is inherently an elevated component in MD3, `Icon
    /// Button` is not. Deliberately does **not** auto-call `enable_
    /// interaction()`, the same real contract `add_button`/`add_icon_
    /// button` already establish.
    #[pyo3(signature = (icon, size="default", variant="surface", x=None, y=None))]
    fn add_fab(
        &self,
        icon: &str,
        size: &str,
        variant: &str,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        let (container_size, default_corner_radius) = fab_shape(size)?;
        let colors = resolve_fab_colors(&self.theme.borrow(), variant)?;
        let path = resolve_icon_path(icon)?;
        // M50 Phase 2: corner radius is keyed by *size* (small/default/
        // large -- FAB's own real variant dimension for shape), not the
        // *color* variant `resolve_fab_colors` uses -- the two are
        // genuinely independent axes for this component. Elevation is
        // flat regardless of either (real MD3 anatomy, confirmed before
        // this milestone), so it's looked up with no variant at all --
        // `FabColors` deliberately doesn't carry it (unlike `ButtonColors`),
        // since bundling it into the color resolver would need a second,
        // unrelated key axis threaded through for no real benefit.
        let corner_radius = {
            let theme = self.theme.borrow();
            theme
                .shape("fab", Some(size))
                .unwrap_or(f64::from(default_corner_radius))
        };
        let elevation = self
            .theme
            .borrow()
            .elevation("fab", None)
            .unwrap_or(FAB_REST_ELEVATION_LEVEL);

        let mut tree = self.tree.borrow_mut();
        let container_paint = PaintProperties::new(colors.container, corner_radius, elevation, 1.0);
        let mut container_style = positioned_style(
            Size {
                width: length(container_size),
                height: length(container_size),
            },
            x,
            y,
        );
        container_style.display = taffy::Display::Flex;
        container_style.justify_content = Some(JustifyContent::CENTER);
        container_style.align_items = Some(AlignItems::CENTER);
        let container = tree.insert(NodeKind::Rect, container_style, container_paint);

        let icon_id = tree.insert(
            NodeKind::Icon(IconState::new(path, colors.icon)),
            Style {
                size: Size {
                    width: length(FAB_ICON_SIZE),
                    height: length(FAB_ICON_SIZE),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(container, icon_id);
        tree.add_child(self.root, container);
        drop(tree);
        self.retheme_hooks.borrow_mut().push(fab_retheme_hook(
            container,
            icon_id,
            variant.to_string(),
            size.to_string(),
            default_corner_radius,
        ));
        Ok(self.wrap_node(container))
    }

    /// M30 Phase 1 Step 3 (§5, §7): `Extended FAB`, `FAB`'s own real
    /// color/elevation system (`resolve_fab_colors`, level-3 rest
    /// elevation) with a real icon-plus-label anatomy instead of an
    /// icon alone -- one real MD3 size (56dp tall, `corner-large`
    /// 16dp, no small/large variants -- those are plain `FAB`-only
    /// concepts, confirmed against Material Web's own token source).
    /// `icon` is optional, matching real MD3's own label-only Extended
    /// FAB -- the real, verified padding actually changes between the
    /// two cases (16dp leading with an icon, 20dp without), not just a
    /// visual difference this implementation invented. No intrinsic
    /// text measurement exists anywhere in this engine (`add_text`'s
    /// own stated limitation), so `width` is a required real caller
    /// input, the same shape every other `add_*` method already uses.
    #[pyo3(signature = (label, width, icon=None, variant="primary", x=None, y=None))]
    fn add_extended_fab(
        &self,
        label: &str,
        width: f32,
        icon: Option<&str>,
        variant: &str,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        let colors = resolve_fab_colors(&self.theme.borrow(), variant)?;
        let icon_path = icon.map(resolve_icon_path).transpose()?;

        let leading_padding = if icon_path.is_some() {
            EXTENDED_FAB_LEADING_PADDING_WITH_ICON
        } else {
            EXTENDED_FAB_LEADING_PADDING_NO_ICON
        };
        // M50 Phase 2: its own distinct key from plain `"fab"` -- no
        // size variants for Extended FAB in real MD3, so no variant is
        // threaded through at all.
        let corner_radius = self
            .theme
            .borrow()
            .shape("extended_fab", None)
            .unwrap_or(EXTENDED_FAB_CORNER_RADIUS);
        let elevation = self
            .theme
            .borrow()
            .elevation("extended_fab", None)
            .unwrap_or(FAB_REST_ELEVATION_LEVEL);

        let mut tree = self.tree.borrow_mut();
        let container_paint = PaintProperties::new(colors.container, corner_radius, elevation, 1.0);
        let mut container_style = positioned_style(
            Size {
                width: length(width),
                height: length(EXTENDED_FAB_HEIGHT),
            },
            x,
            y,
        );
        container_style.display = taffy::Display::Flex;
        container_style.align_items = Some(AlignItems::CENTER);
        container_style.padding = TaffyRect {
            left: length(leading_padding),
            right: length(EXTENDED_FAB_TRAILING_PADDING),
            top: zero(),
            bottom: zero(),
        };
        container_style.gap = Size {
            width: length(EXTENDED_FAB_ICON_LABEL_GAP),
            height: length(0.0),
        };
        let container = tree.insert(NodeKind::Rect, container_style, container_paint);

        let mut extended_fab_icon_id: Option<NodeId> = None;
        if let Some(path) = icon_path {
            let icon_id = tree.insert(
                NodeKind::Icon(IconState::new(path, colors.icon)),
                Style {
                    size: Size {
                        width: length(FAB_ICON_SIZE),
                        height: length(FAB_ICON_SIZE),
                    },
                    ..Default::default()
                },
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
            );
            tree.add_child(container, icon_id);
            extended_fab_icon_id = Some(icon_id);
        }

        let icon_and_gap = if icon.is_some() {
            FAB_ICON_SIZE + EXTENDED_FAB_ICON_LABEL_GAP
        } else {
            0.0
        };
        let label_width =
            (width - leading_padding - icon_and_gap - EXTENDED_FAB_TRAILING_PADDING).max(0.0);
        let label_id = tree.insert(
            NodeKind::Text(TextState {
                content: label.to_string(),
                font_family: "Roboto".to_string(),
                font_weight: BUTTON_LABEL_FONT_WEIGHT,
                font_size: BUTTON_LABEL_FONT_SIZE,
                align: TextAlign::Start,
            }),
            Style {
                size: Size {
                    width: length(label_width),
                    height: length(BUTTON_LABEL_LINE_HEIGHT),
                },
                ..Default::default()
            },
            PaintProperties::new(colors.icon, 0.0, 0.0, 1.0),
        );
        tree.add_child(container, label_id);
        tree.add_child(self.root, container);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(extended_fab_retheme_hook(
                container,
                extended_fab_icon_id,
                label_id,
                variant.to_string(),
            ));
        Ok(self.wrap_node(container))
    }

    /// M30 Phase 1 Step 4 (§5, §7): `Segmented Button`, MD3's own real
    /// group-of-2-to-5-connected-segments anatomy -- one shared,
    /// continuous outline frame (`corner-full`, rounded only on the
    /// group's own outer left/right edges -- `PaintProperties.
    /// corner_radii_override`, this step's own new real capability,
    /// see `engine-render/tests/corner_radii_paint.rs`), one real
    /// divider between each pair of adjacent segments, and per-segment
    /// selected/unselected paint (selected: `secondary_container`
    /// background, `on_secondary_container` label plus a real 18dp
    /// checkmark; unselected: transparent background, `on_surface`
    /// label -- reusing `ThemeState::on_surface()` directly rather
    /// than a new `Md3Baseline` role, the same established un-themed
    /// fallback every other component already resolves `on_surface`
    /// through). Frame/dividers are added as real `Rect` children of
    /// the frame itself (not `self.root`), absolutely positioned
    /// relative to it via `positioned_style`'s own existing inset
    /// branch -- so the whole group only ever needs one real `x`/`y`
    /// placement, the frame's own.
    ///
    /// **Real, explicit design decision, not an oversight:**
    /// group-exclusivity (deselecting sibling segments on a real
    /// single-select click) is deliberately *not* built here --
    /// `BUILD_TRACKER.md`'s own Phase 2 scope for the future `Radio
    /// Button` already states this precisely: "group-exclusivity is
    /// application state... not engine-owned," per Design Principle 6.
    /// This method paints the real *initial* selected/unselected state
    /// from `selected` and returns every segment's own real container
    /// `Node` -- an app wires up live re-toggling with the exact same
    /// already-generic primitives every other component uses
    /// (`set_on_click`, `Node.animate`, `Node.add_child`/`Node.
    /// remove()` to swap the checkmark in or out), not a new
    /// component-specific toggle method invented here.
    #[pyo3(signature = (labels, width, selected=None, height=SEGMENTED_BUTTON_HEIGHT, x=None, y=None))]
    fn add_segmented_button(
        &self,
        labels: Vec<String>,
        width: f32,
        selected: Option<Vec<bool>>,
        height: f32,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Vec<Node>> {
        if labels.len() < 2 {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "add_segmented_button needs at least 2 labels (MD3's own real minimum \
                 segment count), got {}",
                labels.len()
            )));
        }
        let selected = match selected {
            Some(s) if s.len() == labels.len() => s,
            Some(s) => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "selected has {} entries but labels has {} -- they must match",
                    s.len(),
                    labels.len()
                )));
            }
            None => vec![false; labels.len()],
        };

        let (outline_color, on_surface) = {
            let theme = self.theme.borrow();
            (
                if theme.is_set() {
                    theme.role("outline").unwrap_or(Md3Baseline::OUTLINE)
                } else {
                    Md3Baseline::OUTLINE
                },
                theme.on_surface(),
            )
        };
        let secondary_container = {
            let theme = self.theme.borrow();
            if theme.is_set() {
                theme
                    .role("secondary_container")
                    .unwrap_or(Md3Baseline::SECONDARY_CONTAINER)
            } else {
                Md3Baseline::SECONDARY_CONTAINER
            }
        };
        let on_secondary_container = {
            let theme = self.theme.borrow();
            if theme.is_set() {
                theme
                    .role("on_secondary_container")
                    .unwrap_or(Md3Baseline::ON_SECONDARY_CONTAINER)
            } else {
                Md3Baseline::ON_SECONDARY_CONTAINER
            }
        };
        let check_path = resolve_icon_path("check")?;

        let n = labels.len();
        let divider_count = (n - 1) as f32;
        let segment_width = (width - divider_count * SEGMENTED_BUTTON_OUTLINE_WIDTH) / n as f32;
        // M50 Phase 2: no elevation override here -- the frame's own
        // elevation is always a literal `0.0` in real MD3 anatomy for
        // this component (confirmed before this milestone), not a real
        // themeable choice, the same "neither" classification `add_chip`
        // /`add_tooltip` share.
        let corner = self
            .theme
            .borrow()
            .shape("segmented_button", None)
            .unwrap_or_else(|| f64::from(height) / 2.0);

        let mut tree = self.tree.borrow_mut();

        let mut frame_style = positioned_style(
            Size {
                width: length(width),
                height: length(height),
            },
            x,
            y,
        );
        frame_style.display = taffy::Display::Flex;
        let mut frame_paint = PaintProperties::new(TRANSPARENT, corner, 0.0, 1.0);
        frame_paint.border_color = Animated::new(outline_color);
        frame_paint.border_width = Animated::new(f64::from(SEGMENTED_BUTTON_OUTLINE_WIDTH));
        let frame = tree.insert(NodeKind::Rect, frame_style, frame_paint);

        let mut segments = Vec::with_capacity(n);
        let mut segment_retheme_data: Vec<(NodeId, NodeId, Option<NodeId>, bool)> =
            Vec::with_capacity(n);
        let mut divider_ids: Vec<NodeId> = Vec::with_capacity(n.saturating_sub(1));
        let mut cursor = 0.0_f32;
        for (i, label) in labels.into_iter().enumerate() {
            let is_selected = selected[i];
            let corner_radii_override = if i == 0 {
                Some([corner, 0.0, 0.0, corner])
            } else if i == n - 1 {
                Some([0.0, corner, corner, 0.0])
            } else {
                None
            };

            let mut segment_paint = PaintProperties::new(
                if is_selected {
                    secondary_container
                } else {
                    TRANSPARENT
                },
                0.0,
                0.0,
                1.0,
            );
            segment_paint.corner_radii_override = corner_radii_override;
            let mut segment_style = positioned_style(
                Size {
                    width: length(segment_width),
                    height: length(height),
                },
                Some(cursor),
                Some(0.0),
            );
            segment_style.display = taffy::Display::Flex;
            segment_style.justify_content = Some(JustifyContent::CENTER);
            segment_style.align_items = Some(AlignItems::CENTER);
            segment_style.gap = Size {
                width: length(SEGMENTED_BUTTON_ICON_LABEL_GAP),
                height: length(0.0),
            };
            let segment = tree.insert(NodeKind::Rect, segment_style, segment_paint);

            let label_color = if is_selected {
                on_secondary_container
            } else {
                on_surface
            };
            let mut check_id: Option<NodeId> = None;
            if is_selected {
                let id = tree.insert(
                    NodeKind::Icon(IconState::new(check_path.clone(), on_secondary_container)),
                    Style {
                        size: Size {
                            width: length(SEGMENTED_BUTTON_CHECKMARK_SIZE),
                            height: length(SEGMENTED_BUTTON_CHECKMARK_SIZE),
                        },
                        ..Default::default()
                    },
                    PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
                );
                tree.add_child(segment, id);
                check_id = Some(id);
            }
            let label_width = (segment_width
                - 2.0 * SEGMENTED_BUTTON_HORIZONTAL_PADDING
                - if is_selected {
                    SEGMENTED_BUTTON_CHECKMARK_SIZE + SEGMENTED_BUTTON_ICON_LABEL_GAP
                } else {
                    0.0
                })
            .max(0.0);
            let label_id = tree.insert(
                NodeKind::Text(TextState {
                    content: label,
                    font_family: "Roboto".to_string(),
                    font_weight: BUTTON_LABEL_FONT_WEIGHT,
                    font_size: BUTTON_LABEL_FONT_SIZE,
                    align: TextAlign::Center,
                }),
                Style {
                    size: Size {
                        width: length(label_width),
                        height: length(BUTTON_LABEL_LINE_HEIGHT),
                    },
                    ..Default::default()
                },
                PaintProperties::new(label_color, 0.0, 0.0, 1.0),
            );
            tree.add_child(segment, label_id);
            tree.add_child(frame, segment);
            segments.push(segment);
            segment_retheme_data.push((segment, label_id, check_id, is_selected));

            cursor += segment_width;
            if i < n - 1 {
                let divider_id = tree.insert(
                    NodeKind::Rect,
                    positioned_style(
                        Size {
                            width: length(SEGMENTED_BUTTON_OUTLINE_WIDTH),
                            height: length(height),
                        },
                        Some(cursor),
                        Some(0.0),
                    ),
                    PaintProperties::new(outline_color, 0.0, 0.0, 1.0),
                );
                tree.add_child(frame, divider_id);
                divider_ids.push(divider_id);
                cursor += SEGMENTED_BUTTON_OUTLINE_WIDTH;
            }
        }

        tree.add_child(self.root, frame);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(segmented_button_retheme_hook(
                frame,
                segment_retheme_data,
                divider_ids,
                height,
            ));
        Ok(segments.into_iter().map(|id| self.wrap_node(id)).collect())
    }

    /// M30 Phase 2 Step 3 (§5, §7): `Chip`, MD3's four real variants
    /// (Assist/Filter/Input/Suggestion) -- a plain composition (`Rect`
    /// + optional leading `Icon` + `Text` + optional trailing `Icon`),
    /// not a new first-class `NodeKind` -- see `ChipColors`'s own doc
    /// comment for the real architectural reasoning. A selected
    /// Filter Chip's own real checkmark replaces any custom `icon`
    /// (showing both would be redundant -- real MD3 behavior, not
    /// this engine's own invention). `removable` adds a real trailing
    /// "close" icon (Input Chip's own real anatomy) -- independent of
    /// `variant`, since any chip can reasonably be made removable, not
    /// only Input specifically. `selected` only has a real visual
    /// effect on `"filter"` -- passed for any other variant, it's
    /// silently a no-op, matching `Checkbox`'s own established "an app
    /// can pass irrelevant state, the engine just doesn't act on it
    /// differently" tolerance rather than raising. Deliberately does
    /// **not** auto-call `enable_interaction()`, the same real
    /// contract every other `add_*` composite already establishes.
    #[pyo3(signature = (label, width, variant="assist", icon=None, selected=false, removable=false, x=None, y=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_chip(
        &self,
        label: &str,
        width: f32,
        variant: &str,
        icon: Option<&str>,
        selected: bool,
        removable: bool,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        let colors = resolve_chip_colors(&self.theme.borrow(), variant, selected)?;

        let show_checkmark = variant == "filter" && selected;
        let leading_icon_name = if show_checkmark { Some("check") } else { icon };
        let leading_path = leading_icon_name.map(resolve_icon_path).transpose()?;
        let trailing_path = if removable {
            Some(resolve_icon_path("close")?)
        } else {
            None
        };

        let leading_padding = if leading_path.is_some() {
            CHIP_LEADING_PADDING_WITH_ICON
        } else {
            CHIP_LEADING_PADDING_NO_ICON
        };
        let trailing_padding = if trailing_path.is_some() {
            CHIP_TRAILING_PADDING_WITH_ICON
        } else {
            CHIP_TRAILING_PADDING_NO_ICON
        };

        // M50 Phase 3: no elevation override -- Chip has no real
        // elevation concept in MD3 anatomy at all (confirmed before
        // this milestone, always a literal `0.0`), the same "neither"
        // classification `add_segmented_button`'s own doc comment
        // states.
        let corner_radius = self
            .theme
            .borrow()
            .shape("chip", None)
            .unwrap_or(CHIP_CORNER_RADIUS);
        let mut tree = self.tree.borrow_mut();
        let mut container_paint = PaintProperties::new(colors.container, corner_radius, 0.0, 1.0);
        container_paint.border_color = Animated::new(colors.border_color);
        container_paint.border_width = Animated::new(colors.border_width);
        let mut container_style = positioned_style(
            Size {
                width: length(width),
                height: length(CHIP_HEIGHT),
            },
            x,
            y,
        );
        container_style.display = taffy::Display::Flex;
        container_style.align_items = Some(AlignItems::CENTER);
        container_style.padding = TaffyRect {
            left: length(leading_padding),
            right: length(trailing_padding),
            top: zero(),
            bottom: zero(),
        };
        container_style.gap = Size {
            width: length(CHIP_ICON_GAP),
            height: length(0.0),
        };
        let container = tree.insert(NodeKind::Rect, container_style, container_paint);

        let mut icon_count = 0.0_f32;
        let mut chip_leading_id: Option<NodeId> = None;
        if let Some(path) = leading_path {
            icon_count += 1.0;
            let leading_id = tree.insert(
                NodeKind::Icon(IconState::new(path, colors.icon)),
                Style {
                    size: Size {
                        width: length(CHIP_ICON_SIZE),
                        height: length(CHIP_ICON_SIZE),
                    },
                    ..Default::default()
                },
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
            );
            tree.add_child(container, leading_id);
            chip_leading_id = Some(leading_id);
        }

        let label_width = (width
            - leading_padding
            - trailing_padding
            - icon_count * (CHIP_ICON_SIZE + CHIP_ICON_GAP))
            .max(0.0);
        let label_id = tree.insert(
            NodeKind::Text(TextState {
                content: label.to_string(),
                font_family: "Roboto".to_string(),
                font_weight: BUTTON_LABEL_FONT_WEIGHT,
                font_size: BUTTON_LABEL_FONT_SIZE,
                align: TextAlign::Start,
            }),
            Style {
                size: Size {
                    width: length(label_width),
                    height: length(BUTTON_LABEL_LINE_HEIGHT),
                },
                ..Default::default()
            },
            PaintProperties::new(colors.label, 0.0, 0.0, 1.0),
        );
        tree.add_child(container, label_id);

        let mut chip_trailing_id: Option<NodeId> = None;
        if let Some(path) = trailing_path {
            let trailing_id = tree.insert(
                NodeKind::Icon(IconState::new(path, colors.icon)),
                Style {
                    size: Size {
                        width: length(CHIP_ICON_SIZE),
                        height: length(CHIP_ICON_SIZE),
                    },
                    ..Default::default()
                },
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
            );
            tree.add_child(container, trailing_id);
            chip_trailing_id = Some(trailing_id);
        }

        tree.add_child(self.root, container);
        drop(tree);
        self.retheme_hooks.borrow_mut().push(chip_retheme_hook(
            container,
            label_id,
            chip_leading_id,
            chip_trailing_id,
            variant.to_string(),
            selected,
        ));
        Ok(self.wrap_node(container))
    }

    /// M30 Phase 2 Step 4 (§5, §7): one real MD3 menu row -- `Chip`'s
    /// own real composition shape (`Rect` + optional leading `Icon` +
    /// `Text`), reusing List Item's own real tokens (`add_menu_item`'s
    /// own module-level doc comment has the real "Material Web has no
    /// separate menu-item token file" finding). Not attached to a real
    /// menu panel until `build_menu` moves it there -- returned already
    /// attached to `self.root` like every other `add_*` node, the same
    /// "detach, then re-attach elsewhere" real mechanism `Tree::detach`
    /// already provides for exactly this kind of real re-parenting.
    ///
    /// M30 Phase 8 Step 6 (§11.3): `submenu` adds a real trailing
    /// `chevron_right` indicator -- MD3's own real convention for "this
    /// item opens a nested menu." A purely visual affordance, not a new
    /// interaction primitive: the real submenu itself is just *another*
    /// `Menu` (`build_menu` + `open_menu`), opened with this item's own
    /// returned `Node` as the anchor, exactly the way `Main Menu`
    /// submenus extend the already-real context-menu overlay mechanism
    /// (M4 Phase 7) rather than needing a new overlay kind -- confirmed
    /// directly (`Tree::open_overlay` takes any `NodeId` as its own real
    /// anchor already, no special-casing for "is this a menu item"
    /// anywhere), not assumed.
    #[pyo3(signature = (label, icon=None, submenu=false, width=200.0, x=None, y=None))]
    fn add_menu_item(
        &self,
        label: &str,
        icon: Option<&str>,
        submenu: bool,
        width: f32,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        let (label_color, icon_color) = {
            let theme = self.theme.borrow();
            let icon_color = if theme.is_set() {
                theme
                    .role("on_surface_variant")
                    .unwrap_or(Md3Baseline::ON_SURFACE_VARIANT)
            } else {
                Md3Baseline::ON_SURFACE_VARIANT
            };
            (theme.on_surface(), icon_color)
        };
        let icon_path = icon.map(resolve_icon_path).transpose()?;
        let chevron_path = if submenu {
            Some(resolve_icon_path("chevron_right")?)
        } else {
            None
        };

        let mut tree = self.tree.borrow_mut();
        let mut container_style = positioned_style(
            Size {
                width: length(width),
                height: length(MENU_ITEM_HEIGHT),
            },
            x,
            y,
        );
        container_style.display = taffy::Display::Flex;
        container_style.align_items = Some(AlignItems::CENTER);
        container_style.padding = TaffyRect {
            left: length(MENU_ITEM_LEADING_SPACE),
            right: length(MENU_ITEM_LEADING_SPACE),
            top: zero(),
            bottom: zero(),
        };
        container_style.gap = Size {
            width: length(MENU_ITEM_ICON_GAP),
            height: length(0.0),
        };
        let container = tree.insert(
            NodeKind::Rect,
            container_style,
            PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
        );

        let mut icon_count = 0.0_f32;
        let mut menu_item_icon_id: Option<NodeId> = None;
        if let Some(path) = icon_path {
            icon_count = 1.0;
            let icon_id = tree.insert(
                NodeKind::Icon(IconState::new(path, icon_color)),
                Style {
                    size: Size {
                        width: length(MENU_ITEM_ICON_SIZE),
                        height: length(MENU_ITEM_ICON_SIZE),
                    },
                    ..Default::default()
                },
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
            );
            tree.add_child(container, icon_id);
            menu_item_icon_id = Some(icon_id);
        }

        let trailing_reserved = if chevron_path.is_some() {
            MENU_ITEM_ICON_SIZE + MENU_ITEM_ICON_GAP
        } else {
            0.0
        };
        let label_width = (width
            - 2.0 * MENU_ITEM_LEADING_SPACE
            - icon_count * (MENU_ITEM_ICON_SIZE + MENU_ITEM_ICON_GAP)
            - trailing_reserved)
            .max(0.0);
        let label_id = tree.insert(
            NodeKind::Text(TextState {
                content: label.to_string(),
                font_family: "Roboto".to_string(),
                font_weight: BUTTON_LABEL_FONT_WEIGHT,
                font_size: BUTTON_LABEL_FONT_SIZE,
                align: TextAlign::Start,
            }),
            Style {
                size: Size {
                    width: length(label_width),
                    height: length(BUTTON_LABEL_LINE_HEIGHT),
                },
                ..Default::default()
            },
            PaintProperties::new(label_color, 0.0, 0.0, 1.0),
        );
        tree.add_child(container, label_id);

        let mut menu_item_chevron_id: Option<NodeId> = None;
        if let Some(path) = chevron_path {
            let chevron_id = tree.insert(
                NodeKind::Icon(IconState::new(path, icon_color)),
                Style {
                    size: Size {
                        width: length(MENU_ITEM_ICON_SIZE),
                        height: length(MENU_ITEM_ICON_SIZE),
                    },
                    ..Default::default()
                },
                PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
            );
            tree.add_child(container, chevron_id);
            menu_item_chevron_id = Some(chevron_id);
        }

        tree.add_child(self.root, container);
        drop(tree);
        self.retheme_hooks.borrow_mut().push(menu_item_retheme_hook(
            label_id,
            menu_item_icon_id,
            menu_item_chevron_id,
        ));
        Ok(self.wrap_node(container))
    }

    /// M30 Phase 2 Step 4 (§5, §7, §11.3): assembles `items` (each a
    /// real node from `add_menu_item`) into one real MD3 menu panel --
    /// `surface_container` fill, `corner-extra-small` (4dp), a real
    /// rest-state elevation (level 2), all verified against Material
    /// Web's own token source (`_md-comp-menu.scss`). Each item is
    /// **moved**, not copied -- detached from wherever it currently
    /// lives (`self.root`, if freshly created by `add_menu_item`) and
    /// re-attached under the returned panel, the real `Tree::detach`-
    /// then-`add_child` mechanism `Tree::close_overlay`'s own doc
    /// comment already establishes as this codebase's real re-
    /// parenting pattern. Returns the panel **not yet attached
    /// anywhere** -- `open_menu` is what actually shows it (`Tree::
    /// open_overlay`'s own real `add_child` call handles attachment at
    /// that point, the identical real contract `close_overlay`'s
    /// "detach, not destroy" leaves content ready for).
    #[pyo3(signature = (items, width=200.0))]
    fn build_menu(&self, items: Vec<PyRef<'_, Node>>, width: f32) -> PyResult<Node> {
        if items.is_empty() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "build_menu needs at least 1 item",
            ));
        }
        for item in &items {
            if !Rc::ptr_eq(&self.tree, &item.tree) {
                return Err(EngineError::ForeignNode.into());
            }
        }
        let panel_color = {
            let theme = self.theme.borrow();
            if theme.is_set() {
                theme
                    .role("surface_container")
                    .unwrap_or(Md3Baseline::SURFACE_CONTAINER)
            } else {
                Md3Baseline::SURFACE_CONTAINER
            }
        };

        let mut tree = self.tree.borrow_mut();
        let height = items.len() as f32 * MENU_ITEM_HEIGHT;
        let panel_style = Style {
            display: taffy::Display::Flex,
            flex_direction: taffy::FlexDirection::Column,
            size: Size {
                width: length(width),
                height: length(height),
            },
            ..Default::default()
        };
        let panel = tree.insert(
            NodeKind::Rect,
            panel_style,
            PaintProperties::new(
                panel_color,
                MENU_PANEL_CORNER_RADIUS,
                MENU_PANEL_ELEVATION,
                1.0,
            ),
        );

        for item in items {
            if let Some(parent) = tree.get(item.id).and_then(|node| node.parent) {
                tree.detach(parent, item.id);
            }
            tree.add_child(panel, item.id);
        }

        Ok(self.wrap_node(panel))
    }

    /// M30 Phase 2 Step 4 (§11.3): opens `menu` (from `build_menu`)
    /// anchored below `anchor`, via the real, already-existing `Tree::
    /// open_overlay` -- the exact same primitive `Node.set_context_
    /// menu`'s right-click path uses (`dispatch::open_context_menu`'s
    /// own real template, mirrored here), just exposed as a direct
    /// Python-callable method instead of gated behind synthetic
    /// secondary-button dispatch, since a real dropdown menu opens on
    /// a plain left click (or any app-chosen trigger), not a right-
    /// click. **Deliberately does not touch the context-menu mechanism
    /// itself at all** -- `overlay.rs`'s own module doc comment
    /// already states both are real uses of the identical one
    /// primitive ("menu bars, dropdown menus, context menus... are all
    /// the same missing primitive"), so this reuses it rather than
    /// building a second one. Same real reopen guard `open_context_
    /// menu` already has (checked via `overlay_meta`, a safe no-op if
    /// already open, not a double-`add_child`).
    fn open_menu(&self, anchor: PyRef<'_, Node>, menu: PyRef<'_, Node>) -> PyResult<()> {
        if !Rc::ptr_eq(&self.tree, &anchor.tree) || !Rc::ptr_eq(&self.tree, &menu.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        let mut tree = self.tree.borrow_mut();
        if tree.overlay_meta(menu.id).is_some() {
            return Ok(());
        }
        tree.open_overlay(
            self.root,
            anchor.id,
            menu.id,
            OverlayMeta {
                anchor: anchor.id,
                dismiss_on_outside_click: true,
                dismiss_on_escape: true,
                modal: false,
            },
        );
        Ok(())
    }

    /// M30 Phase 2 Step 4 (§11.3): `open_menu`'s own real close
    /// counterpart -- a thin wrapper over the already-real `Tree::
    /// close_overlay` (detach, not destroy, the same real contract
    /// context-menu dismissal already established).
    fn close_menu(&self, menu: PyRef<'_, Node>) -> PyResult<()> {
        if !Rc::ptr_eq(&self.tree, &menu.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        self.tree.borrow_mut().close_overlay(menu.id);
        Ok(())
    }

    /// M30 Phase 3 Step 1 (§5, §7): `Badge`, MD3's real two-size
    /// anatomy -- a plain `Rect` (no label: a real 6dp dot) or `Rect`
    /// + centered `Text` (with a label: a real 16dp pill, `TextAlign::
    /// Center` reused exactly the way `Button`'s own label already
    /// established). Not positioned relative to any other node by
    /// this method -- a real badge is always overlaid on a corner of
    /// some other component (an icon, an avatar), which is purely a
    /// caller-chosen `x`/`y` placement, the same real "absolute
    /// positioning is the caller's job" contract `add_rect`'s own
    /// `x`/`y` params already establish; no new overlay/anchoring
    /// machinery needed for something this simple. `width` only
    /// matters for the labeled variant (a dot ignores it, always
    /// square) -- defaults to `height` (a circle), since MD3's own
    /// real single-digit badge is exactly that; a real multi-digit
    /// badge needs a wider caller-supplied `width`, the same "no
    /// intrinsic text measurement anywhere in this engine" limitation
    /// `add_text`/`add_button`/etc already state.
    #[pyo3(signature = (label=None, width=None, x=None, y=None))]
    fn add_badge(
        &self,
        label: Option<&str>,
        width: Option<f32>,
        x: Option<f32>,
        y: Option<f32>,
    ) -> Node {
        let (error_color, on_error_color) = {
            let theme = self.theme.borrow();
            let role = |name: &str, fallback: Color| -> Color {
                if theme.is_set() {
                    theme.role(name).unwrap_or(fallback)
                } else {
                    fallback
                }
            };
            (
                role("error", Md3Baseline::ERROR),
                role("on_error", Md3Baseline::ON_ERROR),
            )
        };

        let mut tree = self.tree.borrow_mut();
        let Some(label) = label else {
            let corner_radius = self
                .theme
                .borrow()
                .shape("badge", Some("dot"))
                .unwrap_or_else(|| f64::from(BADGE_DOT_SIZE) / 2.0);
            let id = tree.insert(
                NodeKind::Rect,
                positioned_style(
                    Size {
                        width: length(BADGE_DOT_SIZE),
                        height: length(BADGE_DOT_SIZE),
                    },
                    x,
                    y,
                ),
                PaintProperties::new(error_color, corner_radius, 0.0, 1.0),
            );
            tree.add_child(self.root, id);
            drop(tree);
            self.retheme_hooks
                .borrow_mut()
                .push(badge_dot_retheme_hook(id));
            return self.wrap_node(id);
        };

        let badge_width = width.unwrap_or(BADGE_LABELED_HEIGHT);
        let corner_radius = self
            .theme
            .borrow()
            .shape("badge", Some("labeled"))
            .unwrap_or_else(|| f64::from(BADGE_LABELED_HEIGHT) / 2.0);
        let mut container_style = positioned_style(
            Size {
                width: length(badge_width),
                height: length(BADGE_LABELED_HEIGHT),
            },
            x,
            y,
        );
        container_style.display = taffy::Display::Flex;
        container_style.justify_content = Some(JustifyContent::CENTER);
        container_style.align_items = Some(AlignItems::CENTER);
        let container = tree.insert(
            NodeKind::Rect,
            container_style,
            PaintProperties::new(error_color, corner_radius, 0.0, 1.0),
        );

        let label_id = tree.insert(
            NodeKind::Text(TextState {
                content: label.to_string(),
                font_family: "Roboto".to_string(),
                font_weight: BADGE_LABEL_FONT_WEIGHT,
                font_size: BADGE_LABEL_FONT_SIZE,
                align: TextAlign::Center,
            }),
            Style {
                size: Size {
                    width: length(badge_width),
                    height: length(BADGE_LABEL_FONT_SIZE + 2.0),
                },
                ..Default::default()
            },
            PaintProperties::new(on_error_color, 0.0, 0.0, 1.0),
        );
        tree.add_child(container, label_id);
        tree.add_child(self.root, container);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(badge_labeled_retheme_hook(container, label_id));
        self.wrap_node(container)
    }

    /// M30 Phase 3 Step 2 (§5, §7): creates a real `NodeKind::
    /// LinearProgress`, mirroring `add_slider`'s own real shape --
    /// `value` seeds `LinearProgressState`'s own initial state
    /// (clamped `0.0..=1.0`, `LinearProgressState::new`'s own real
    /// contract). Colors always resolved here (through `theme.role`,
    /// gated on `theme.is_set()`, else `Md3Baseline`'s own real
    /// fallback), the same real reason `add_radio_button`/`add_switch`
    /// have no caller-supplied `background` either -- there's no
    /// independent "fill color" concept in real MD3 progress-
    /// indicator anatomy. `height` defaults to MD3's own real 4dp
    /// track/indicator height, verified against Material Web's own
    /// token source.
    #[pyo3(signature = (width, height=4.0, value=0.0, x=None, y=None))]
    fn add_linear_progress(
        &self,
        width: f32,
        height: f32,
        value: f64,
        x: Option<f32>,
        y: Option<f32>,
    ) -> Node {
        let mut state = LinearProgressState::new(value);
        {
            let theme = self.theme.borrow();
            let role = |name: &str, fallback: Color| -> Color {
                if theme.is_set() {
                    theme.role(name).unwrap_or(fallback)
                } else {
                    fallback
                }
            };
            state.track_tint = role(
                "surface_container_highest",
                Md3Baseline::SURFACE_CONTAINER_HIGHEST,
            );
            state.indicator_tint = role("primary", Md3Baseline::PRIMARY);
        }
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::LinearProgress(state),
            positioned_style(
                Size {
                    width: length(width),
                    height: length(height),
                },
                x,
                y,
            ),
            PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(linear_progress_retheme_hook(id));
        self.wrap_node(id)
    }

    /// M30 Phase 3 Step 2 (§5, §7): `LinearProgress`'s own real
    /// circular sibling -- `size` is a single square dimension (real
    /// MD3 circular indicators are always a circle, the same real
    /// "one dimension is the honest shape" reasoning `add_radio_
    /// button`'s own `size` param already established), defaulting to
    /// MD3's own real 48dp token.
    #[pyo3(signature = (size=48.0, value=0.0, x=None, y=None))]
    fn add_circular_progress(&self, size: f32, value: f64, x: Option<f32>, y: Option<f32>) -> Node {
        let mut state = CircularProgressState::new(value);
        {
            let theme = self.theme.borrow();
            state.indicator_tint = if theme.is_set() {
                theme.role("primary").unwrap_or(Md3Baseline::PRIMARY)
            } else {
                Md3Baseline::PRIMARY
            };
        }
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::CircularProgress(state),
            positioned_style(
                Size {
                    width: length(size),
                    height: length(size),
                },
                x,
                y,
            ),
            PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(circular_progress_retheme_hook(id));
        self.wrap_node(id)
    }

    /// M39 Phase 2 (§5, §7): a real, perpetually-looping MD3
    /// Expressive-style loading spinner -- see `engine_core::
    /// LoadingIndicatorState`'s own doc comment for the full real
    /// design (four real, procedurally-generated shapes, `Animated<
    /// ShapeKey>` morph, `Tree::tick_all`'s own automatic advance, no
    /// app-side wiring needed at all once constructed). `size` is a
    /// single square dimension, the identical real "one dimension is
    /// the honest shape" convention `add_circular_progress`'s own
    /// `size` param already establishes, defaulting to the same real
    /// MD3 48dp token. **Real, stated v1 simplification, on top of the
    /// simplified-shapes choice already scoped via `AskUserQuestion`:**
    /// the real shapes fill the whole `size x size` box directly, not
    /// MD3's own real "38dp shape inset within a 48dp container" ratio
    /// -- a minor visual refinement, not essential to the real "loops
    /// forever, morphs between real shapes" behavior this step exists
    /// for.
    #[pyo3(signature = (size=48.0, color=None, x=None, y=None))]
    fn add_loading_indicator(
        &self,
        size: f32,
        color: Option<(u8, u8, u8, u8)>,
        x: Option<f32>,
        y: Option<f32>,
    ) -> Node {
        let tint = match color {
            Some((r, g, b, a)) => Color::from_rgba8(r, g, b, a),
            None => {
                let theme = self.theme.borrow();
                if theme.is_set() {
                    theme.role("primary").unwrap_or(Md3Baseline::PRIMARY)
                } else {
                    Md3Baseline::PRIMARY
                }
            }
        };
        let state = LoadingIndicatorState::new(f64::from(size), f64::from(size));
        let mut paint = PaintProperties::new(tint, 0.0, 0.0, 1.0);
        // M38 Phase 4's own established real precedent: initialize
        // `shape` directly to the first real shape at construction,
        // not left at `ShapeKey::empty()` -- otherwise the very first
        // real tick would morph *from* nothing, a real, visible flash
        // bug.
        paint.shape = Animated::new(state.shapes[0].clone());
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::LoadingIndicator(state),
            positioned_style(
                Size {
                    width: length(size),
                    height: length(size),
                },
                x,
                y,
            ),
            paint,
        );
        tree.add_child(self.root, id);
        drop(tree);
        // M52 Phase 2: only register a hook when `color` was omitted --
        // an explicit literal color always wins, forever, and was never
        // theme-derived in the first place, so there is nothing for a
        // later `set_theme` to legitimately recompute.
        if color.is_none() {
            self.retheme_hooks
                .borrow_mut()
                .push(loading_indicator_retheme_hook(id));
        }
        self.wrap_node(id)
    }

    /// M39 Phase 2 Step 2 (§5, §7): a real MD3 Time Picker's own
    /// circular clock-face drag control -- see `engine_core::
    /// TimePickerDialState`'s own doc comment for the full real
    /// design and its stated v1 scope limits (no digit labels, no
    /// AM/PM toggle chrome, a plain rectangular drag hit region). A
    /// square `size x size` box, the same single-dimension convention
    /// `add_loading_indicator`/`add_circular_progress` already
    /// establish -- 256.0 is a practical default for a legible dial,
    /// not a cited MD3 token (this phase's own research located the
    /// dial's real *anatomy*, not a verified default diameter). `hour`
    /// is a real 24-hour value (`0..=23`), `minute` `0..=59`; both
    /// clamp the same way `TimePickerDialState::new` does.
    #[pyo3(signature = (hour=0, minute=0, size=256.0, x=None, y=None))]
    fn add_time_picker_dial(
        &self,
        hour: u8,
        minute: u8,
        size: f32,
        x: Option<f32>,
        y: Option<f32>,
    ) -> Node {
        let mut state = TimePickerDialState::new(hour, minute);
        {
            let theme = self.theme.borrow();
            let role = |name: &str, fallback: Color| -> Color {
                if theme.is_set() {
                    theme.role(name).unwrap_or(fallback)
                } else {
                    fallback
                }
            };
            state.face_tint = role(
                "surface_container_highest",
                Md3Baseline::SURFACE_CONTAINER_HIGHEST,
            );
            state.hand_tint = role("primary", Md3Baseline::PRIMARY);
        }
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::TimePickerDial(state),
            positioned_style(
                Size {
                    width: length(size),
                    height: length(size),
                },
                x,
                y,
            ),
            PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(time_picker_dial_retheme_hook(id));
        self.wrap_node(id)
    }

    /// M30 Phase 3 Step 3 (§5, §7): `Card`, MD3's three real variants
    /// (Elevated/Filled/Outlined). A plain `Rect` container -- see
    /// `CardColors`'s own doc comment for why this carries no fixed
    /// anatomy of its own; the app adds arbitrary content via `Node.
    /// add_child`, already generic for any node. Deliberately does
    /// **not** auto-call `enable_interaction()` -- a real MD3 card is
    /// not always clickable (many are purely a visual container), the
    /// same "only a node that opts in pays the cost" contract every
    /// other composite `add_*` in this catalog already establishes.
    #[pyo3(signature = (width, height, variant="elevated", x=None, y=None))]
    fn add_card(
        &self,
        width: f32,
        height: f32,
        variant: &str,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        let colors = resolve_card_colors(&self.theme.borrow(), variant)?;
        // M50 Phase 3: bare "card" key -- corner radius is not
        // variant-dependent in real MD3 anatomy (confirmed before this
        // milestone), so no variant is threaded through.
        let corner_radius = self
            .theme
            .borrow()
            .shape("card", None)
            .unwrap_or(CARD_CORNER_RADIUS);
        let mut tree = self.tree.borrow_mut();
        let mut paint =
            PaintProperties::new(colors.container, corner_radius, colors.elevation, 1.0);
        paint.border_color = Animated::new(colors.border_color);
        paint.border_width = Animated::new(colors.border_width);
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
            paint,
        );
        tree.add_child(self.root, id);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(card_retheme_hook(id, variant.to_string()));
        Ok(self.wrap_node(id))
    }

    /// M30 Phase 3 Step 4 (§5, §7): `Divider`, MD3's real 1dp
    /// separator line -- a plain `Rect`, `outline_variant`-colored,
    /// no shape/elevation/interaction of its own (a divider is purely
    /// decorative, never clickable in real MD3). `length`/`vertical`
    /// together give the real horizontal-or-vertical anatomy: a
    /// horizontal divider is `length` wide and `DIVIDER_THICKNESS`
    /// tall, a vertical one the reverse -- the same real single-
    /// dimension-plus-orientation shape a line naturally has, not two
    /// separate methods for what's really one real component.
    #[pyo3(signature = (length, vertical=false, x=None, y=None))]
    fn add_divider(&self, length: f32, vertical: bool, x: Option<f32>, y: Option<f32>) -> Node {
        let color = {
            let theme = self.theme.borrow();
            if theme.is_set() {
                theme
                    .role("outline_variant")
                    .unwrap_or(Md3Baseline::OUTLINE_VARIANT)
            } else {
                Md3Baseline::OUTLINE_VARIANT
            }
        };
        let (width, height) = if vertical {
            (DIVIDER_THICKNESS, length)
        } else {
            (length, DIVIDER_THICKNESS)
        };
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Rect,
            positioned_style(
                Size {
                    width: taffy::prelude::length(width),
                    height: taffy::prelude::length(height),
                },
                x,
                y,
            ),
            PaintProperties::new(color, 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(divider_retheme_hook(id));
        self.wrap_node(id)
    }

    /// M30 Phase 3 Step 5 (§5, §7, §11.3): `Tooltip` (Plain variant),
    /// closing Phase 3 -- a real `Rect` + centered `Text` panel
    /// (`TextAlign::Center`, `Button`'s own real label technique
    /// reused). **Real, deliberate reuse, not new overlay machinery:**
    /// returned genuinely unattached anywhere, the identical real
    /// contract `build_menu`'s own panel already has -- a tooltip
    /// shows and hides through the exact same `Window.open_menu`/
    /// `close_menu` this milestone's own Step 4 already built (itself
    /// a thin wrapper over `Tree::open_overlay`/`close_overlay`,
    /// `overlay.rs`'s own module doc comment already naming tooltips
    /// as a real intended consumer of that one primitive alongside
    /// dropdown/context menus), triggered from the app's own real
    /// `Node.set_on_hover_enter`/`set_on_hover_exit` (already generic,
    /// works on any `NodeKind`) rather than a dedicated `open_tooltip`/
    /// `close_tooltip` pair that would only ever duplicate them --
    /// `examples/tooltip.py` demonstrates the real end-to-end wiring.
    #[pyo3(signature = (text, width, x=None, y=None))]
    fn add_tooltip(&self, text: &str, width: f32, x: Option<f32>, y: Option<f32>) -> Node {
        let corner_radius = self
            .theme
            .borrow()
            .shape("tooltip", None)
            .unwrap_or(TOOLTIP_CORNER_RADIUS);
        let mut tree = self.tree.borrow_mut();
        let mut container_style = positioned_style(
            Size {
                width: length(width),
                height: length(TOOLTIP_HEIGHT),
            },
            x,
            y,
        );
        container_style.display = taffy::Display::Flex;
        container_style.justify_content = Some(JustifyContent::CENTER);
        container_style.align_items = Some(AlignItems::CENTER);
        container_style.padding = TaffyRect {
            left: length(TOOLTIP_HORIZONTAL_PADDING),
            right: length(TOOLTIP_HORIZONTAL_PADDING),
            top: zero(),
            bottom: zero(),
        };
        let container = tree.insert(
            NodeKind::Rect,
            container_style,
            PaintProperties::new(Md3Baseline::INVERSE_SURFACE, corner_radius, 0.0, 1.0),
        );

        let label_width = (width - 2.0 * TOOLTIP_HORIZONTAL_PADDING).max(0.0);
        let label_id = tree.insert(
            NodeKind::Text(TextState {
                content: text.to_string(),
                font_family: "Roboto".to_string(),
                font_weight: TOOLTIP_FONT_WEIGHT,
                font_size: TOOLTIP_FONT_SIZE,
                align: TextAlign::Center,
            }),
            Style {
                size: Size {
                    width: length(label_width),
                    height: length(TOOLTIP_FONT_SIZE + 2.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Md3Baseline::INVERSE_ON_SURFACE, 0.0, 0.0, 1.0),
        );
        tree.add_child(container, label_id);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(tooltip_retheme_hook(container));
        self.wrap_node(container)
    }

    /// M30 Phase 4 Step 1 (§5, §7, §11.3): `Dialog`, a real modal --
    /// this catalog's own first real difference from `open_overlay`'s
    /// existing anchor-relative dropdown/tooltip placement, which
    /// this method deliberately doesn't reuse for positioning (only
    /// for lifecycle/dismissal, via `open_dialog`). Real anatomy: a
    /// real, full-window scrim (`scrim` role at 32% opacity, the
    /// already-real `PaintProperties.opacity` field) with the actual
    /// dialog panel centered inside it via plain flex `justify_
    /// content`/`align_items` -- no absolute-position centering math
    /// needed, since the scrim itself already spans the whole window
    /// and taffy already centers children inside a flex parent for
    /// free. **Real, deliberate anatomy difference from every other
    /// `add_*` in this catalog: no `x`/`y` parameters at all** -- a
    /// real modal dialog is always centered, never caller-positioned
    /// (unlike `positioned_style`'s own `Position::Absolute` shape,
    /// which would pull the panel out of the centering flex layout
    /// entirely if used here). Returns the **scrim** node, genuinely
    /// unattached anywhere -- the same real contract `build_menu`'s
    /// own panel and `add_tooltip`'s own panel already have; `open_
    /// dialog` is what actually shows it.
    #[pyo3(signature = (headline, text, width, height))]
    fn add_dialog(&self, headline: &str, text: &str, width: f32, height: f32) -> Node {
        let (scrim_color, panel_color, headline_color, body_color) = {
            let theme = self.theme.borrow();
            let role = |name: &str, fallback: Color| -> Color {
                if theme.is_set() {
                    theme.role(name).unwrap_or(fallback)
                } else {
                    fallback
                }
            };
            (
                role("scrim", Md3Baseline::SCRIM),
                role(
                    "surface_container_high",
                    Md3Baseline::SURFACE_CONTAINER_HIGH,
                ),
                theme.on_surface(),
                role("on_surface_variant", Md3Baseline::ON_SURFACE_VARIANT),
            )
        };
        let (dialog_corner_radius, dialog_elevation) = {
            let theme = self.theme.borrow();
            (
                theme.shape("dialog", None).unwrap_or(DIALOG_CORNER_RADIUS),
                theme.elevation("dialog", None).unwrap_or(DIALOG_ELEVATION),
            )
        };

        let mut tree = self.tree.borrow_mut();

        let scrim_style = Style {
            size: Size {
                width: length(self.width.get() as f32),
                height: length(self.height.get() as f32),
            },
            display: taffy::Display::Flex,
            justify_content: Some(JustifyContent::CENTER),
            align_items: Some(AlignItems::CENTER),
            ..Default::default()
        };
        let scrim = tree.insert(
            NodeKind::Rect,
            scrim_style,
            PaintProperties::new(scrim_color, 0.0, 0.0, DIALOG_SCRIM_OPACITY),
        );

        let panel_style = Style {
            size: Size {
                width: length(width),
                height: length(height),
            },
            display: taffy::Display::Flex,
            flex_direction: taffy::FlexDirection::Column,
            padding: TaffyRect {
                left: length(DIALOG_PADDING),
                right: length(DIALOG_PADDING),
                top: length(DIALOG_PADDING),
                bottom: length(DIALOG_PADDING),
            },
            gap: Size {
                width: length(0.0),
                height: length(DIALOG_HEADLINE_GAP),
            },
            ..Default::default()
        };
        let panel = tree.insert(
            NodeKind::Rect,
            panel_style,
            PaintProperties::new(panel_color, dialog_corner_radius, dialog_elevation, 1.0),
        );
        tree.add_child(scrim, panel);

        let content_width = (width - 2.0 * DIALOG_PADDING).max(0.0);
        let headline_id = tree.insert(
            NodeKind::Text(TextState {
                content: headline.to_string(),
                font_family: "Roboto".to_string(),
                font_weight: DIALOG_HEADLINE_FONT_WEIGHT,
                font_size: DIALOG_HEADLINE_FONT_SIZE,
                align: TextAlign::Start,
            }),
            Style {
                size: Size {
                    width: length(content_width),
                    height: length(DIALOG_HEADLINE_FONT_SIZE + 4.0),
                },
                ..Default::default()
            },
            PaintProperties::new(headline_color, 0.0, 0.0, 1.0),
        );
        tree.add_child(panel, headline_id);

        let body_height = (height
            - 2.0 * DIALOG_PADDING
            - DIALOG_HEADLINE_GAP
            - (DIALOG_HEADLINE_FONT_SIZE + 4.0))
            .max(0.0);
        let body_id = tree.insert(
            NodeKind::Text(TextState {
                content: text.to_string(),
                font_family: "Roboto".to_string(),
                font_weight: DIALOG_BODY_FONT_WEIGHT,
                font_size: DIALOG_BODY_FONT_SIZE,
                align: TextAlign::Start,
            }),
            Style {
                size: Size {
                    width: length(content_width),
                    height: length(body_height),
                },
                ..Default::default()
            },
            PaintProperties::new(body_color, 0.0, 0.0, 1.0),
        );
        tree.add_child(panel, body_id);
        drop(tree);
        self.retheme_hooks.borrow_mut().push(dialog_retheme_hook(
            scrim,
            panel,
            headline_id,
            body_id,
        ));
        self.wrap_node(scrim)
    }

    /// M30 Phase 4 Step 1 (§11.3): opens `dialog` (from `add_dialog`)
    /// as a real modal -- `dismiss_on_outside_click: false` (a real
    /// dialog doesn't dismiss on scrim click, unlike a dropdown menu
    /// or tooltip), `modal: true` (this step's own new real
    /// capability, `crates/engine-core/src/overlay.rs`'s own doc
    /// comment has the full real investigation for why the existing
    /// mechanism alone couldn't express this). A zero-size, invisible
    /// synthetic anchor pinned at the window's own real origin is
    /// inserted here and used only for `Tree::open_overlay`'s own
    /// existing anchor-relative math (`inset.top = anchor_y + anchor_
    /// height`, which resolves to exactly `(0, 0)` for a zero-size
    /// anchor at the origin) -- real reuse of an existing primitive
    /// with a real, minimal input, not a second positioning mechanism;
    /// `close_dialog` removes it again on close, so repeated open/
    /// close cycles don't leak one every time.
    fn open_dialog(&self, dialog: PyRef<'_, Node>) -> PyResult<()> {
        if !Rc::ptr_eq(&self.tree, &dialog.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        let mut tree = self.tree.borrow_mut();
        if tree.overlay_meta(dialog.id).is_some() {
            return Ok(());
        }
        let anchor_style = positioned_style(
            Size {
                width: length(0.0),
                height: length(0.0),
            },
            Some(0.0),
            Some(0.0),
        );
        let anchor = tree.insert(
            NodeKind::Rect,
            anchor_style,
            PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, anchor);
        tree.open_overlay(
            self.root,
            anchor,
            dialog.id,
            OverlayMeta {
                anchor,
                dismiss_on_outside_click: false,
                dismiss_on_escape: true,
                modal: true,
            },
        );
        Ok(())
    }

    /// M30 Phase 4 Step 1 (§11.3): `open_dialog`'s own real close
    /// counterpart -- closes the real overlay (`Tree::close_overlay`,
    /// detach not destroy, the same real contract every other overlay
    /// dismissal already has) and also removes `open_dialog`'s own
    /// synthetic anchor node, which has no other purpose once the
    /// dialog it positioned is closed.
    fn close_dialog(&self, dialog: PyRef<'_, Node>) -> PyResult<()> {
        if !Rc::ptr_eq(&self.tree, &dialog.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        let mut tree = self.tree.borrow_mut();
        let anchor = tree.overlay_meta(dialog.id).map(|meta| meta.anchor);
        tree.close_overlay(dialog.id);
        if let Some(anchor) = anchor {
            tree.remove(anchor);
        }
        Ok(())
    }

    /// M30 Phase 4 Step 2 (§5, §7, §11.3): `Snackbar`, a real transient
    /// notification. Real anatomy verified against Material Web's own
    /// token source (`SNACKBAR_*` constants above have the full real
    /// finding chain, including tracing `inverse-primary` two files
    /// deeper to its real hex). Supporting text reuses Body Medium's
    /// own already-declared constants (`DIALOG_BODY_FONT_SIZE`/
    /// `_WEIGHT`, the identical real MD3 type role `Dialog`'s own body
    /// text already uses); the action label reuses Label Large's own
    /// already-declared constants (`BUTTON_LABEL_FONT_SIZE`/`_WEIGHT`/
    /// `_LINE_HEIGHT`, the same real role every other labeled
    /// component in this catalog already shares) -- neither is a new
    /// per-component type constant, both are real shared MD3 roles.
    ///
    /// **Real, deliberate anatomy departure from every prior component
    /// in this catalog: returns up to three independent real `Node`s,
    /// not one.** `Segmented Button`'s own real precedent (`Vec<Node>`,
    /// one per independently-selectable segment, Phase 1 Step 4)
    /// already established that a genuinely multi-interactive-region
    /// composite returns multiple real `Node`s rather than one --
    /// `Chip`'s own `removable` trailing icon is real precedent for
    /// the *other* case (a purely decorative sub-icon, never
    /// independently clickable, confirmed by direct re-read of
    /// `add_chip` before choosing this component's own shape): a real
    /// MD3 snackbar's action button must be independently clickable
    /// from the rest of the snackbar (which isn't itself a button), so
    /// the decorative-icon shape doesn't fit here -- this returns
    /// `(container, action, close)`, the last two `None` when not
    /// requested, each of `action`/`close` a real, separately
    /// `enable_interaction()`-able `Node` the same way every
    /// standalone interactive component already is. Returned genuinely
    /// unattached anywhere, the same real contract `add_dialog`'s own
    /// panel already has -- `open_snackbar` is what actually shows it.
    #[pyo3(signature = (text, width, action_label=None, closable=false))]
    fn add_snackbar(
        &self,
        text: &str,
        width: f32,
        action_label: Option<&str>,
        closable: bool,
    ) -> PyResult<(Node, Option<Node>, Option<Node>)> {
        let (container_color, text_color, action_color, icon_color) = {
            let theme = self.theme.borrow();
            let role = |name: &str, fallback: Color| -> Color {
                if theme.is_set() {
                    theme.role(name).unwrap_or(fallback)
                } else {
                    fallback
                }
            };
            (
                role("inverse_surface", Md3Baseline::INVERSE_SURFACE),
                role("inverse_on_surface", Md3Baseline::INVERSE_ON_SURFACE),
                role("inverse_primary", Md3Baseline::INVERSE_PRIMARY),
                role("inverse_on_surface", Md3Baseline::INVERSE_ON_SURFACE),
            )
        };
        let (corner_radius, elevation) = {
            let theme = self.theme.borrow();
            (
                theme
                    .shape("snackbar", None)
                    .unwrap_or(SNACKBAR_CORNER_RADIUS),
                theme
                    .elevation("snackbar", None)
                    .unwrap_or(SNACKBAR_ELEVATION),
            )
        };

        let mut tree = self.tree.borrow_mut();

        let container_style = Style {
            size: Size {
                width: length(width),
                height: length(SNACKBAR_HEIGHT),
            },
            display: taffy::Display::Flex,
            align_items: Some(AlignItems::CENTER),
            padding: TaffyRect {
                left: length(SNACKBAR_HORIZONTAL_PADDING),
                right: length(SNACKBAR_HORIZONTAL_PADDING),
                top: zero(),
                bottom: zero(),
            },
            gap: Size {
                width: length(SNACKBAR_GAP),
                height: length(0.0),
            },
            ..Default::default()
        };
        let container = tree.insert(
            NodeKind::Rect,
            container_style,
            PaintProperties::new(container_color, corner_radius, elevation, 1.0),
        );

        let text_id = tree.insert(
            NodeKind::Text(TextState {
                content: text.to_string(),
                font_family: "Roboto".to_string(),
                font_weight: DIALOG_BODY_FONT_WEIGHT,
                font_size: DIALOG_BODY_FONT_SIZE,
                align: TextAlign::Start,
            }),
            Style {
                flex_grow: 1.0,
                size: Size {
                    width: auto(),
                    height: length(DIALOG_BODY_FONT_SIZE + 4.0),
                },
                ..Default::default()
            },
            PaintProperties::new(text_color, 0.0, 0.0, 1.0),
        );
        tree.add_child(container, text_id);

        let mut snackbar_action_ids: Option<(NodeId, NodeId)> = None;
        let action = if let Some(label) = action_label {
            let action_container = tree.insert(
                NodeKind::Rect,
                Style {
                    size: Size {
                        width: length(SNACKBAR_ACTION_WIDTH),
                        height: length(BUTTON_LABEL_LINE_HEIGHT),
                    },
                    display: taffy::Display::Flex,
                    justify_content: Some(JustifyContent::CENTER),
                    align_items: Some(AlignItems::CENTER),
                    ..Default::default()
                },
                PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
            );
            let label_id = tree.insert(
                NodeKind::Text(TextState {
                    content: label.to_string(),
                    font_family: "Roboto".to_string(),
                    font_weight: BUTTON_LABEL_FONT_WEIGHT,
                    font_size: BUTTON_LABEL_FONT_SIZE,
                    align: TextAlign::Center,
                }),
                Style {
                    size: Size {
                        width: length(SNACKBAR_ACTION_WIDTH),
                        height: length(BUTTON_LABEL_LINE_HEIGHT),
                    },
                    ..Default::default()
                },
                PaintProperties::new(action_color, 0.0, 0.0, 1.0),
            );
            tree.add_child(action_container, label_id);
            tree.add_child(container, action_container);
            snackbar_action_ids = Some((action_container, label_id));
            Some(self.wrap_node(action_container))
        } else {
            None
        };

        let mut snackbar_close_ids: Option<(NodeId, NodeId)> = None;
        let close = if closable {
            let close_path = resolve_icon_path("close")?;
            let close_container = tree.insert(
                NodeKind::Rect,
                Style {
                    size: Size {
                        width: length(SNACKBAR_ICON_SIZE),
                        height: length(SNACKBAR_ICON_SIZE),
                    },
                    display: taffy::Display::Flex,
                    justify_content: Some(JustifyContent::CENTER),
                    align_items: Some(AlignItems::CENTER),
                    ..Default::default()
                },
                PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
            );
            let icon_id = tree.insert(
                NodeKind::Icon(IconState::new(close_path, icon_color)),
                Style {
                    size: Size {
                        width: length(SNACKBAR_ICON_SIZE),
                        height: length(SNACKBAR_ICON_SIZE),
                    },
                    ..Default::default()
                },
                PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
            );
            tree.add_child(close_container, icon_id);
            tree.add_child(container, close_container);
            snackbar_close_ids = Some((close_container, icon_id));
            Some(self.wrap_node(close_container))
        } else {
            None
        };

        drop(tree);
        self.retheme_hooks.borrow_mut().push(snackbar_retheme_hook(
            container,
            text_id,
            snackbar_action_ids,
            snackbar_close_ids,
        ));
        Ok((self.wrap_node(container), action, close))
    }

    /// M30 Phase 4 Step 2 (§11.3): opens `snackbar` (from
    /// `add_snackbar`) anchored to the real desktop bottom-left corner
    /// -- this milestone's own already-established desktop-adaptation
    /// rule for MD3's mobile-only full-width-at-bottom anatomy (§7's
    /// own scope text), not a new rule invented for this step.
    /// `dismiss_on_outside_click: false`/`dismiss_on_escape: false`/
    /// `modal: false`: a real snackbar is a passive notification, not
    /// an interaction-blocking overlay (unlike `Dialog`'s own real
    /// `modal: true`), and isn't dismissed by an accidental outside
    /// click or Escape -- only its own explicit action/close, or
    /// whatever timeout the app itself drives. **Real, stated,
    /// deliberate scope limit, not a silently missing feature:** this
    /// engine has no timer/scheduler primitive anywhere (confirmed by
    /// grep across `engine-core`/`engine-py` before scoping this step
    /// -- zero hits), so a real auto-dismiss-after-duration is the
    /// app's own responsibility (e.g. closing it after N frames of its
    /// own `App.run` loop), matching Design Principle 6 ("engine gives
    /// primitives, app composes behavior"). Reuses `open_dialog`'s own
    /// real synthetic-zero-size-anchor technique a second time, placed
    /// at `(SNACKBAR_LEFT_MARGIN, height - SNACKBAR_BOTTOM_MARGIN -
    /// SNACKBAR_HEIGHT)` instead of the origin -- the identical
    /// `inset.top = anchor_y + anchor_height` math `open_overlay`
    /// already does resolves directly to that corner for a zero-size
    /// anchor, no new positioning primitive needed a second time
    /// either.
    fn open_snackbar(&self, snackbar: PyRef<'_, Node>) -> PyResult<()> {
        if !Rc::ptr_eq(&self.tree, &snackbar.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        let mut tree = self.tree.borrow_mut();
        if tree.overlay_meta(snackbar.id).is_some() {
            return Ok(());
        }
        let anchor_x = SNACKBAR_LEFT_MARGIN;
        let anchor_y = self.height.get() as f32 - SNACKBAR_BOTTOM_MARGIN - SNACKBAR_HEIGHT;
        let anchor_style = positioned_style(
            Size {
                width: length(0.0),
                height: length(0.0),
            },
            Some(anchor_x),
            Some(anchor_y),
        );
        let anchor = tree.insert(
            NodeKind::Rect,
            anchor_style,
            PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, anchor);
        tree.open_overlay(
            self.root,
            anchor,
            snackbar.id,
            OverlayMeta {
                anchor,
                dismiss_on_outside_click: false,
                dismiss_on_escape: false,
                modal: false,
            },
        );
        Ok(())
    }

    /// M30 Phase 4 Step 2 (§11.3): `open_snackbar`'s own real close
    /// counterpart -- identical real contract to `close_dialog` (closes
    /// the real overlay, detach not destroy, and removes the synthetic
    /// anchor `open_snackbar` created so repeated open/close cycles
    /// don't leak one every time).
    fn close_snackbar(&self, snackbar: PyRef<'_, Node>) -> PyResult<()> {
        if !Rc::ptr_eq(&self.tree, &snackbar.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        let mut tree = self.tree.borrow_mut();
        let anchor = tree.overlay_meta(snackbar.id).map(|meta| meta.anchor);
        tree.close_overlay(snackbar.id);
        if let Some(anchor) = anchor {
            tree.remove(anchor);
        }
        Ok(())
    }

    /// M30 Phase 4 Step 3 (§5, §7, §11.3): `Side Sheet`, the real
    /// desktop counterpart to Bottom Sheet (excluded as a mobile
    /// pattern, this milestone's own scope). Real anatomy: see the
    /// `SIDE_SHEET_*` constants above for the full real token-source
    /// finding (Material Web has no dedicated side-sheet token file,
    /// reuses `Navigation Drawer`'s own real tokens instead). The real
    /// per-corner rounding (`corner-large-end`, rounded only on the
    /// two corners *away* from the docked edge) reuses `PaintProperties.
    /// corner_radii_override` -- `Segmented Button`'s own already-real
    /// universal capability (Phase 1 Step 4), not a new per-corner
    /// primitive invented a second time.
    ///
    /// **Real, deliberate behavioral fork by variant, not one uniform
    /// shape forced onto two genuinely different real MD3 lifecycles:**
    /// `Standard` (`modal=false`) is a real layout participant, not an
    /// overlay at all -- attached immediately to `self.root`, the
    /// identical real precedent `add_card` already establishes (a
    /// plain positioned `Rect`, optional `x`/`y`, the app re-parents
    /// it into its own layout via the already-generic `Node.add_child`/
    /// `Tree::try_add_child` the same way any `Card` content already
    /// does, or docks it via the existing real 5-zone `Dock` -- M4
    /// Phase 9, confirmed still the most capable real docking
    /// mechanism this codebase has, nothing new needed there) -- no
    /// scrim, no blocking, no open/close lifecycle needed. `Modal`
    /// (`modal=true`) genuinely floats over content and must block
    /// interaction behind it -- reuses `Dialog`'s own real full-window
    /// scrim (`DIALOG_SCRIM_OPACITY`, the same well-established 32%
    /// MD3 convention) and returns the **scrim** node unattached, the
    /// identical real contract `add_dialog` already has; `open_side_
    /// sheet`/`close_side_sheet` are what actually show/hide it,
    /// anchored to the real right edge (flex `justify_content:
    /// FLEX_END`, full height) instead of `Dialog`'s own centered
    /// placement.
    #[pyo3(signature = (width=SIDE_SHEET_WIDTH, height=None, modal=false, x=None, y=None))]
    fn add_side_sheet(
        &self,
        width: f32,
        height: Option<f32>,
        modal: bool,
        x: Option<f32>,
        y: Option<f32>,
    ) -> Node {
        let (container_color, scrim_color) = {
            let theme = self.theme.borrow();
            let role = |name: &str, fallback: Color| -> Color {
                if theme.is_set() {
                    theme.role(name).unwrap_or(fallback)
                } else {
                    fallback
                }
            };
            if modal {
                (
                    role("surface_container_low", Md3Baseline::SURFACE_CONTAINER_LOW),
                    role("scrim", Md3Baseline::SCRIM),
                )
            } else {
                (role("surface", Md3Baseline::SURFACE), Md3Baseline::SCRIM)
            }
        };
        let variant_key = if modal { "modal" } else { "standard" };
        let default_elevation = if modal {
            SIDE_SHEET_MODAL_ELEVATION
        } else {
            SIDE_SHEET_STANDARD_ELEVATION
        };
        let (corner_radius, elevation) = {
            let theme = self.theme.borrow();
            (
                theme
                    .shape("side_sheet", Some(variant_key))
                    .unwrap_or(SIDE_SHEET_CORNER_RADIUS),
                theme
                    .elevation("side_sheet", Some(variant_key))
                    .unwrap_or(default_elevation),
            )
        };

        let mut tree = self.tree.borrow_mut();
        let panel_height = height.unwrap_or(self.height.get() as f32);

        let mut panel_paint = PaintProperties::new(container_color, 0.0, elevation, 1.0);
        panel_paint.corner_radii_override = Some([corner_radius, 0.0, 0.0, corner_radius]);

        if modal {
            let scrim_style = Style {
                size: Size {
                    width: length(self.width.get() as f32),
                    height: length(self.height.get() as f32),
                },
                display: taffy::Display::Flex,
                justify_content: Some(JustifyContent::FLEX_END),
                ..Default::default()
            };
            let scrim = tree.insert(
                NodeKind::Rect,
                scrim_style,
                PaintProperties::new(scrim_color, 0.0, 0.0, DIALOG_SCRIM_OPACITY),
            );
            let panel_style = Style {
                size: Size {
                    width: length(width),
                    height: length(panel_height),
                },
                ..Default::default()
            };
            let panel = tree.insert(NodeKind::Rect, panel_style, panel_paint);
            tree.add_child(scrim, panel);
            drop(tree);
            self.retheme_hooks
                .borrow_mut()
                .push(side_sheet_retheme_hook(panel, Some(scrim), modal));
            self.wrap_node(scrim)
        } else {
            let panel_style = positioned_style(
                Size {
                    width: length(width),
                    height: length(panel_height),
                },
                x,
                y,
            );
            let panel = tree.insert(NodeKind::Rect, panel_style, panel_paint);
            tree.add_child(self.root, panel);
            drop(tree);
            self.retheme_hooks
                .borrow_mut()
                .push(side_sheet_retheme_hook(panel, None, modal));
            self.wrap_node(panel)
        }
    }

    /// M30 Phase 4 Step 3 (§11.3): opens a real *modal* side sheet
    /// (from `add_side_sheet(..., modal=True)`) -- identical real
    /// contract to `open_dialog` (`modal: true`, `dismiss_on_escape:
    /// true`, `dismiss_on_outside_click: false`, the same real
    /// synthetic-zero-size-anchor technique), the only real difference
    /// being the anchor's own position: pinned at the window's real
    /// top-right corner instead of the origin, since a side sheet's
    /// own scrim already right-aligns its panel via flex (`add_side_
    /// sheet`'s own `justify_content: FLEX_END`) and only needs a
    /// zero-size anchor to make `open_overlay`'s existing math resolve
    /// to `(0, 0)` relative to the scrim itself, the identical real
    /// reasoning `open_dialog`'s own doc comment already has. A real,
    /// explicit no-op if `side_sheet` was built with `modal=False` --
    /// a standard side sheet has no overlay lifecycle at all, already
    /// attached to `self.root` by `add_side_sheet` itself.
    fn open_side_sheet(&self, side_sheet: PyRef<'_, Node>) -> PyResult<()> {
        if !Rc::ptr_eq(&self.tree, &side_sheet.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        let mut tree = self.tree.borrow_mut();
        if tree.overlay_meta(side_sheet.id).is_some() {
            return Ok(());
        }
        if tree
            .get(side_sheet.id)
            .and_then(|node| node.parent)
            .is_some()
        {
            // A standard side sheet (`modal=False`) is already attached
            // to `self.root` by `add_side_sheet` -- opening it again
            // here would be a real double-attach, not a safe no-op the
            // way an already-open overlay is.
            return Ok(());
        }
        let anchor_style = positioned_style(
            Size {
                width: length(0.0),
                height: length(0.0),
            },
            Some(0.0),
            Some(0.0),
        );
        let anchor = tree.insert(
            NodeKind::Rect,
            anchor_style,
            PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, anchor);
        tree.open_overlay(
            self.root,
            anchor,
            side_sheet.id,
            OverlayMeta {
                anchor,
                dismiss_on_outside_click: false,
                dismiss_on_escape: true,
                modal: true,
            },
        );
        Ok(())
    }

    /// M30 Phase 4 Step 3 (§11.3): `open_side_sheet`'s own real close
    /// counterpart -- identical real contract to `close_dialog`. A
    /// real, explicit no-op for a standard (`modal=False`) side sheet,
    /// the same real reason `open_side_sheet` is.
    fn close_side_sheet(&self, side_sheet: PyRef<'_, Node>) -> PyResult<()> {
        if !Rc::ptr_eq(&self.tree, &side_sheet.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        let mut tree = self.tree.borrow_mut();
        let anchor = tree.overlay_meta(side_sheet.id).map(|meta| meta.anchor);
        tree.close_overlay(side_sheet.id);
        if let Some(anchor) = anchor {
            tree.remove(anchor);
        }
        Ok(())
    }

    /// M30 Phase 5 Step 1 (§5, §7): `Navigation Rail`, the real
    /// desktop counterpart to Navigation Bar (excluded as a mobile
    /// pattern, this milestone's own scope). Real anatomy verified
    /// against Material Web's own token source: see the `NAV_RAIL_*`
    /// constants above for the full real finding, including a real
    /// self-caught correction (a first token fetch misattributed the
    /// active-indicator's own color to the rail's container) and a
    /// real, genuine active/inactive *weight* difference in the label
    /// (500 vs. 700), not just a color change like every prior
    /// component in this catalog.
    ///
    /// **Real, deliberate architectural choice, not an oversight:**
    /// built as a plain composition (one `Rect` frame, `corner-none`
    /// per-item `Rect` containers each with an indicator `Rect` +
    /// `Icon` + `Text` label), not a new first-class `NodeKind` --
    /// the same real dividing line `Segmented Button`/`Filter Chip`
    /// already established (Phase 1 Step 4/Phase 2 Step 3): a rail's
    /// own "active item" is fundamentally the same group/app-owned
    /// single-select state as a segment's own, per Design Principle 6
    /// ("group-exclusivity is application state... not engine-
    /// owned"), not a new engine-owned toggle. Returns every item's
    /// own real container `Node` (`Vec<Node>`, `Segmented Button`'s
    /// own exact real return shape) -- the rail's own background
    /// frame is never returned, the identical real "frame stays
    /// internal" contract `Segmented Button`'s own frame/dividers
    /// already have. An app wires up live re-toggling with the same
    /// already-generic primitives every other component uses
    /// (`set_on_click`, `Node.animate`), not a new component-specific
    /// toggle method invented here.
    #[pyo3(signature = (labels, icons, selected=None, x=None, y=None))]
    fn add_navigation_rail(
        &self,
        labels: Vec<String>,
        icons: Vec<String>,
        selected: Option<usize>,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Vec<Node>> {
        if labels.is_empty() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "add_navigation_rail needs at least 1 item",
            ));
        }
        if labels.len() != icons.len() {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "add_navigation_rail needs one icon per label -- got {} labels and {} icons",
                labels.len(),
                icons.len()
            )));
        }
        if let Some(sel) = selected
            && sel >= labels.len()
        {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "selected index {sel} is out of range for {} items",
                labels.len()
            )));
        }
        let icon_paths: Vec<_> = icons
            .iter()
            .map(|name| resolve_icon_path(name))
            .collect::<PyResult<Vec<_>>>()?;

        let (
            container_color,
            indicator_color,
            active_icon_color,
            inactive_icon_color,
            active_label_color,
            inactive_label_color,
        ) = {
            let theme = self.theme.borrow();
            let role = |name: &str, fallback: Color| -> Color {
                if theme.is_set() {
                    theme.role(name).unwrap_or(fallback)
                } else {
                    fallback
                }
            };
            let on_surface_variant = role("on_surface_variant", Md3Baseline::ON_SURFACE_VARIANT);
            (
                role("surface", Md3Baseline::SURFACE),
                role("secondary_container", Md3Baseline::SECONDARY_CONTAINER),
                role(
                    "on_secondary_container",
                    Md3Baseline::ON_SECONDARY_CONTAINER,
                ),
                on_surface_variant,
                theme.on_surface(),
                on_surface_variant,
            )
        };
        // M50 Phase 4: the frame's own corner radius stays un-themed --
        // real MD3 anatomy deliberately uses "corner-none" here (a
        // genuine design choice, not a missing value), and the frame
        // is never returned to Python at all, matching `add_segmented_
        // button`'s own identical "internal frame, not a themeable
        // surface" precedent. Only the per-item indicator's real const
        // gets an override.
        let indicator_corner_radius = self
            .theme
            .borrow()
            .shape("navigation_rail", Some("indicator"))
            .unwrap_or(NAV_RAIL_INDICATOR_CORNER_RADIUS);

        let mut tree = self.tree.borrow_mut();
        let item_height =
            NAV_RAIL_INDICATOR_HEIGHT + NAV_RAIL_ITEM_GAP + NAV_RAIL_LABEL_FONT_SIZE + 4.0;
        let rail_height = NAV_RAIL_TOP_PADDING
            + (labels.len() as f32) * item_height
            + (labels.len().saturating_sub(1) as f32) * NAV_RAIL_ITEM_SPACING;

        let mut frame_style = positioned_style(
            Size {
                width: length(NAV_RAIL_WIDTH),
                height: length(rail_height),
            },
            x,
            y,
        );
        frame_style.display = taffy::Display::Flex;
        frame_style.flex_direction = taffy::FlexDirection::Column;
        frame_style.align_items = Some(AlignItems::CENTER);
        frame_style.padding = TaffyRect {
            left: zero(),
            right: zero(),
            top: length(NAV_RAIL_TOP_PADDING),
            bottom: zero(),
        };
        frame_style.gap = Size {
            width: length(0.0),
            height: length(NAV_RAIL_ITEM_SPACING),
        };
        let frame = tree.insert(
            NodeKind::Rect,
            frame_style,
            PaintProperties::new(container_color, 0.0, 0.0, 1.0),
        );

        let mut items = Vec::with_capacity(labels.len());
        let mut rail_item_data: Vec<(NodeId, NodeId, NodeId, bool)> =
            Vec::with_capacity(labels.len());
        for (i, (label, path)) in labels.into_iter().zip(icon_paths).enumerate() {
            let is_active = selected == Some(i);

            let item_style = Style {
                size: Size {
                    width: length(NAV_RAIL_WIDTH),
                    height: length(item_height),
                },
                display: taffy::Display::Flex,
                flex_direction: taffy::FlexDirection::Column,
                align_items: Some(AlignItems::CENTER),
                gap: Size {
                    width: length(0.0),
                    height: length(NAV_RAIL_ITEM_GAP),
                },
                ..Default::default()
            };
            let item = tree.insert(
                NodeKind::Rect,
                item_style,
                PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
            );

            let indicator_fill = if is_active {
                indicator_color
            } else {
                TRANSPARENT
            };
            let indicator_style = Style {
                size: Size {
                    width: length(NAV_RAIL_INDICATOR_WIDTH),
                    height: length(NAV_RAIL_INDICATOR_HEIGHT),
                },
                display: taffy::Display::Flex,
                justify_content: Some(JustifyContent::CENTER),
                align_items: Some(AlignItems::CENTER),
                ..Default::default()
            };
            let indicator = tree.insert(
                NodeKind::Rect,
                indicator_style,
                PaintProperties::new(indicator_fill, indicator_corner_radius, 0.0, 1.0),
            );
            // M30 Phase 5 Step 1 (§5, §7): the real, confirmed gap this
            // component surfaced -- the indicator pill sits squarely
            // over `item`'s own geometric center, and a plain `Rect`
            // always independently claims a hit (unlike `Text`/`Icon`),
            // permanently stealing every click meant for `item`'s own
            // registered handler. `Node.hit_testable`'s own doc comment
            // has the full real finding.
            tree.set_hit_testable(indicator, false);

            let icon_color = if is_active {
                active_icon_color
            } else {
                inactive_icon_color
            };
            let icon_id = tree.insert(
                NodeKind::Icon(IconState::new(path, icon_color)),
                Style {
                    size: Size {
                        width: length(NAV_RAIL_ICON_SIZE),
                        height: length(NAV_RAIL_ICON_SIZE),
                    },
                    ..Default::default()
                },
                PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
            );
            tree.add_child(indicator, icon_id);
            tree.add_child(item, indicator);

            let label_color = if is_active {
                active_label_color
            } else {
                inactive_label_color
            };
            let label_weight = if is_active {
                NAV_RAIL_LABEL_WEIGHT_ACTIVE
            } else {
                NAV_RAIL_LABEL_WEIGHT_INACTIVE
            };
            let label_id = tree.insert(
                NodeKind::Text(TextState {
                    content: label,
                    font_family: "Roboto".to_string(),
                    font_weight: label_weight,
                    font_size: NAV_RAIL_LABEL_FONT_SIZE,
                    align: TextAlign::Center,
                }),
                Style {
                    size: Size {
                        width: length(NAV_RAIL_WIDTH),
                        height: length(NAV_RAIL_LABEL_FONT_SIZE + 4.0),
                    },
                    ..Default::default()
                },
                PaintProperties::new(label_color, 0.0, 0.0, 1.0),
            );
            tree.add_child(item, label_id);

            tree.add_child(frame, item);
            rail_item_data.push((indicator, icon_id, label_id, is_active));
            items.push(self.wrap_node(item));
        }

        tree.add_child(self.root, frame);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(navigation_rail_retheme_hook(frame, rail_item_data));
        Ok(items)
    }

    /// M30 Phase 5 Step 2 (§5, §7, §11.3): `Navigation Drawer`, the
    /// real desktop counterpart to Bottom App Bar's own navigation
    /// role. Real container anatomy: identical to `Side Sheet`'s own
    /// (`SIDE_SHEET_*` constants, both real values come from this
    /// same `_md-comp-navigation-drawer.scss` source), mirrored to
    /// dock the real *left* edge instead of the right -- `corner-
    /// large-end` rounds the two corners away from the docked edge,
    /// so this reverses `Side Sheet`'s own `corner_radii_override`
    /// left-right (square on the left, touching the screen boundary;
    /// rounded on the right). Real per-item anatomy: see the
    /// `NAV_DRAWER_*` constants above for the full real finding.
    ///
    /// **Real, deliberate design avoiding a repeat of `Navigation
    /// Rail`'s own real hit-test bug, not found the hard way a second
    /// time:** each destination's active-indicator pill (336×56dp)
    /// already spans the item's *entire* real clickable anatomy
    /// (icon and label sit side by side *inside* it, not stacked with
    /// the indicator as a smaller decorative layer the way `Navigation
    /// Rail`'s taller icon-over-label item needed) -- so the indicator
    /// itself is directly what gets returned and made interactive, no
    /// separate outer wrapper competing for the same click point, and
    /// no `Tree::set_hit_testable` opt-out needed here at all.
    ///
    /// **Real, deliberate behavioral fork by variant, the identical
    /// real shape `Side Sheet` already established:** `Standard`
    /// (`modal=false`, the default) attaches immediately to `self.
    /// root` (`Card`'s own real precedent, optional `x`/`y`) -- no
    /// scrim, no blocking, no open/close lifecycle. `Modal` (`modal=
    /// true`) genuinely floats over content -- reuses `Dialog`'s own
    /// real full-window scrim and `OverlayMeta.modal` capability,
    /// left-aligned via flex `justify_content: FLEX_START` instead of
    /// `Side Sheet`'s own `FLEX_END`, with the identical real origin-
    /// anchored `open_overlay` technique `open_dialog`/`open_side_
    /// sheet` already use (the scrim's own flex does the real
    /// alignment work, so the synthetic anchor only ever needs to
    /// resolve the scrim's own inset to `(0, 0)`, not a docked-edge-
    /// specific offset). Returns `(container, items)` -- `Segmented
    /// Button`'s own real `Vec<Node>` shape for the destinations,
    /// `Dialog`'s own real single-`Node` shape for the container/
    /// scrim, combined the same real way `Snackbar`'s own multi-node
    /// return already did for a different reason.
    #[pyo3(signature = (labels, icons, selected=None, modal=false, width=SIDE_SHEET_WIDTH, height=None, x=None, y=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_navigation_drawer(
        &self,
        labels: Vec<String>,
        icons: Vec<String>,
        selected: Option<usize>,
        modal: bool,
        width: f32,
        height: Option<f32>,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<(Node, Vec<Node>)> {
        if labels.is_empty() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "add_navigation_drawer needs at least 1 item",
            ));
        }
        if labels.len() != icons.len() {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "add_navigation_drawer needs one icon per label -- got {} labels and {} icons",
                labels.len(),
                icons.len()
            )));
        }
        if let Some(sel) = selected
            && sel >= labels.len()
        {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "selected index {sel} is out of range for {} items",
                labels.len()
            )));
        }
        let icon_paths: Vec<_> = icons
            .iter()
            .map(|name| resolve_icon_path(name))
            .collect::<PyResult<Vec<_>>>()?;

        let (container_color, indicator_color, active_color, inactive_color, scrim_color) = {
            let theme = self.theme.borrow();
            let role = |name: &str, fallback: Color| -> Color {
                if theme.is_set() {
                    theme.role(name).unwrap_or(fallback)
                } else {
                    fallback
                }
            };
            let container = if modal {
                role("surface_container_low", Md3Baseline::SURFACE_CONTAINER_LOW)
            } else {
                role("surface", Md3Baseline::SURFACE)
            };
            (
                container,
                role("secondary_container", Md3Baseline::SECONDARY_CONTAINER),
                role(
                    "on_secondary_container",
                    Md3Baseline::ON_SECONDARY_CONTAINER,
                ),
                role("on_surface_variant", Md3Baseline::ON_SURFACE_VARIANT),
                role("scrim", Md3Baseline::SCRIM),
            )
        };
        let variant_key = if modal { "modal" } else { "standard" };
        let default_elevation = if modal {
            SIDE_SHEET_MODAL_ELEVATION
        } else {
            SIDE_SHEET_STANDARD_ELEVATION
        };
        // M50 Phase 3: `add_navigation_drawer`'s panel currently reuses
        // `SIDE_SHEET_CORNER_RADIUS`/`SIDE_SHEET_MODAL_ELEVATION`/
        // `SIDE_SHEET_STANDARD_ELEVATION` as its own real *constant
        // value*, but gets its own theme key (`"navigation_drawer"`,
        // not `"side_sheet"`) -- decoupling "shares a number today"
        // from "should always share a theme override," matching this
        // milestone's own established `components:` convention
        // (`crates/engine-spec/src/theme.rs`'s own doc comment).
        // `.indicator` is a second, genuinely distinct key for the
        // active-item indicator pill, which has no `side_sheet`
        // equivalent to inherit from at all.
        let (corner_radius, elevation, indicator_corner_radius) = {
            let theme = self.theme.borrow();
            (
                theme
                    .shape("navigation_drawer", Some(variant_key))
                    .unwrap_or(SIDE_SHEET_CORNER_RADIUS),
                theme
                    .elevation("navigation_drawer", Some(variant_key))
                    .unwrap_or(default_elevation),
                theme
                    .shape("navigation_drawer", Some("indicator"))
                    .unwrap_or(NAV_DRAWER_INDICATOR_CORNER_RADIUS),
            )
        };

        let mut tree = self.tree.borrow_mut();
        let panel_height = height.unwrap_or(self.height.get() as f32);

        let mut panel_paint = PaintProperties::new(container_color, 0.0, elevation, 1.0);
        panel_paint.corner_radii_override = Some([0.0, corner_radius, corner_radius, 0.0]);

        let mut panel_style = Style {
            size: Size {
                width: length(width),
                height: length(panel_height),
            },
            display: taffy::Display::Flex,
            flex_direction: taffy::FlexDirection::Column,
            align_items: Some(AlignItems::CENTER),
            padding: TaffyRect {
                left: zero(),
                right: zero(),
                top: length(NAV_DRAWER_TOP_PADDING),
                bottom: zero(),
            },
            gap: Size {
                width: length(0.0),
                height: length(NAV_DRAWER_ITEM_SPACING),
            },
            ..Default::default()
        };
        if !modal && (x.is_some() || y.is_some()) {
            panel_style.position = Position::Absolute;
            panel_style.inset = TaffyRect {
                left: length(x.unwrap_or(0.0)),
                top: length(y.unwrap_or(0.0)),
                right: auto(),
                bottom: auto(),
            };
        }
        let panel = tree.insert(NodeKind::Rect, panel_style, panel_paint);

        let mut items = Vec::with_capacity(labels.len());
        let mut item_retheme_data: Vec<(NodeId, NodeId, NodeId, bool)> =
            Vec::with_capacity(labels.len());
        for (i, (label, path)) in labels.into_iter().zip(icon_paths).enumerate() {
            let is_active = selected == Some(i);
            let fill = if is_active {
                indicator_color
            } else {
                TRANSPARENT
            };
            let label_color = if is_active {
                active_color
            } else {
                inactive_color
            };
            let icon_color = if is_active {
                active_color
            } else {
                inactive_color
            };
            let label_weight = if is_active {
                NAV_RAIL_LABEL_WEIGHT_ACTIVE
            } else {
                NAV_RAIL_LABEL_WEIGHT_INACTIVE
            };

            let indicator_style = Style {
                size: Size {
                    width: length(NAV_DRAWER_INDICATOR_WIDTH),
                    height: length(MENU_ITEM_HEIGHT),
                },
                display: taffy::Display::Flex,
                align_items: Some(AlignItems::CENTER),
                padding: TaffyRect {
                    left: length(MENU_ITEM_LEADING_SPACE),
                    right: length(MENU_ITEM_LEADING_SPACE),
                    top: zero(),
                    bottom: zero(),
                },
                gap: Size {
                    width: length(MENU_ITEM_ICON_GAP),
                    height: length(0.0),
                },
                ..Default::default()
            };
            let indicator = tree.insert(
                NodeKind::Rect,
                indicator_style,
                PaintProperties::new(fill, indicator_corner_radius, 0.0, 1.0),
            );

            let icon_id = tree.insert(
                NodeKind::Icon(IconState::new(path, icon_color)),
                Style {
                    size: Size {
                        width: length(MENU_ITEM_ICON_SIZE),
                        height: length(MENU_ITEM_ICON_SIZE),
                    },
                    ..Default::default()
                },
                PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
            );
            tree.add_child(indicator, icon_id);

            let label_width = (NAV_DRAWER_INDICATOR_WIDTH
                - 2.0 * MENU_ITEM_LEADING_SPACE
                - MENU_ITEM_ICON_SIZE
                - MENU_ITEM_ICON_GAP)
                .max(0.0);
            let label_id = tree.insert(
                NodeKind::Text(TextState {
                    content: label,
                    font_family: "Roboto".to_string(),
                    font_weight: label_weight,
                    font_size: BUTTON_LABEL_FONT_SIZE,
                    align: TextAlign::Start,
                }),
                Style {
                    size: Size {
                        width: length(label_width),
                        height: length(BUTTON_LABEL_LINE_HEIGHT),
                    },
                    ..Default::default()
                },
                PaintProperties::new(label_color, 0.0, 0.0, 1.0),
            );
            tree.add_child(indicator, label_id);

            tree.add_child(panel, indicator);
            item_retheme_data.push((indicator, icon_id, label_id, is_active));
            items.push(self.wrap_node(indicator));
        }

        let container = if modal {
            let scrim_style = Style {
                size: Size {
                    width: length(self.width.get() as f32),
                    height: length(self.height.get() as f32),
                },
                display: taffy::Display::Flex,
                justify_content: Some(JustifyContent::FLEX_START),
                ..Default::default()
            };
            let scrim = tree.insert(
                NodeKind::Rect,
                scrim_style,
                PaintProperties::new(scrim_color, 0.0, 0.0, DIALOG_SCRIM_OPACITY),
            );
            tree.add_child(scrim, panel);
            drop(tree);
            self.retheme_hooks
                .borrow_mut()
                .push(navigation_drawer_retheme_hook(
                    panel,
                    Some(scrim),
                    item_retheme_data,
                    modal,
                ));
            self.wrap_node(scrim)
        } else {
            tree.add_child(self.root, panel);
            drop(tree);
            self.retheme_hooks
                .borrow_mut()
                .push(navigation_drawer_retheme_hook(
                    panel,
                    None,
                    item_retheme_data,
                    modal,
                ));
            self.wrap_node(panel)
        };

        Ok((container, items))
    }

    /// M30 Phase 5 Step 2 (§11.3): opens a real *modal* navigation
    /// drawer (from `add_navigation_drawer(..., modal=True)`) --
    /// identical real contract to `open_side_sheet` (`modal: true`,
    /// `dismiss_on_escape: true`, `dismiss_on_outside_click: false`,
    /// the same real origin-anchored `open_overlay` technique -- the
    /// scrim's own `justify_content: FLEX_START` already does the
    /// real left-alignment work). A real, explicit no-op if `drawer`
    /// was built with `modal=False` -- a standard drawer has no
    /// overlay lifecycle at all, already attached to `self.root` by
    /// `add_navigation_drawer` itself.
    fn open_navigation_drawer(&self, drawer: PyRef<'_, Node>) -> PyResult<()> {
        if !Rc::ptr_eq(&self.tree, &drawer.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        let mut tree = self.tree.borrow_mut();
        if tree.overlay_meta(drawer.id).is_some() {
            return Ok(());
        }
        if tree.get(drawer.id).and_then(|node| node.parent).is_some() {
            return Ok(());
        }
        let anchor_style = positioned_style(
            Size {
                width: length(0.0),
                height: length(0.0),
            },
            Some(0.0),
            Some(0.0),
        );
        let anchor = tree.insert(
            NodeKind::Rect,
            anchor_style,
            PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, anchor);
        tree.open_overlay(
            self.root,
            anchor,
            drawer.id,
            OverlayMeta {
                anchor,
                dismiss_on_outside_click: false,
                dismiss_on_escape: true,
                modal: true,
            },
        );
        Ok(())
    }

    /// M30 Phase 5 Step 2 (§11.3): `open_navigation_drawer`'s own real
    /// close counterpart -- identical real contract to `close_side_
    /// sheet`. A real, explicit no-op for a standard (`modal=False`)
    /// drawer, the same real reason `open_navigation_drawer` is.
    fn close_navigation_drawer(&self, drawer: PyRef<'_, Node>) -> PyResult<()> {
        if !Rc::ptr_eq(&self.tree, &drawer.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        let mut tree = self.tree.borrow_mut();
        let anchor = tree.overlay_meta(drawer.id).map(|meta| meta.anchor);
        tree.close_overlay(drawer.id);
        if let Some(anchor) = anchor {
            tree.remove(anchor);
        }
        Ok(())
    }

    /// M30 Phase 5 Step 3 (§5, §7): `Top App Bar`, MD3's real *Small*
    /// variant (Medium/Large/Small-Centered are out of scope for this
    /// step -- each is its own real, separately-tokened variant, per
    /// this catalog's own established "real per-variant investigation,
    /// not one guessed formula" discipline; a future step can add them
    /// if needed). Real anatomy: see the `TOP_APP_BAR_*` constants
    /// above for the full real finding, including this step's own
    /// real filename-discovery correction (`_md-comp-top-app-bar.scss`
    /// 404s -- each variant has its own separate real token file).
    ///
    /// **Real, deliberate design reusing `Icon Button`'s own exact
    /// real anatomy for leading/trailing actions, not inventing a new
    /// shape:** each icon button is a small `Rect` container (`Icon
    /// Button`'s own real precedent, Phase 1 Step 2) with a centered
    /// `Icon` child -- the container itself is what's returned and
    /// made interactive, the `Icon` child correctly defers (Phase 1's
    /// own real fix), so no decorative intermediate layer and no
    /// `Navigation Rail`-style hit-test risk here either. Returns
    /// `(bar, leading, trailing)`: `leading` is `None` unless
    /// `leading_icon` was given; `trailing` is a `Vec<Node>`, one per
    /// requested trailing icon, empty if none -- the identical real
    /// "independently interactive sub-elements get their own real
    /// `Node`s" shape `Snackbar` already established for its own
    /// action/close.
    #[pyo3(signature = (title, leading_icon=None, trailing_icons=None, width=None, x=None, y=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_top_app_bar(
        &self,
        title: &str,
        leading_icon: Option<&str>,
        trailing_icons: Option<Vec<String>>,
        width: Option<f32>,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<(Node, Option<Node>, Vec<Node>)> {
        let trailing_icons = trailing_icons.unwrap_or_default();
        let leading_path = leading_icon.map(resolve_icon_path).transpose()?;
        let trailing_paths: Vec<_> = trailing_icons
            .iter()
            .map(|name| resolve_icon_path(name))
            .collect::<PyResult<Vec<_>>>()?;

        let (container_color, headline_color, leading_icon_color, trailing_icon_color) = {
            let theme = self.theme.borrow();
            let role = |name: &str, fallback: Color| -> Color {
                if theme.is_set() {
                    theme.role(name).unwrap_or(fallback)
                } else {
                    fallback
                }
            };
            (
                role("surface", Md3Baseline::SURFACE),
                theme.on_surface(),
                theme.on_surface(),
                role("on_surface_variant", Md3Baseline::ON_SURFACE_VARIANT),
            )
        };

        // M50 Phase 4: the bar itself stays un-themed -- always square
        // in real MD3 anatomy, a deliberate design choice, the same
        // "corner-none" precedent `add_navigation_rail`'s own frame
        // already establishes. The leading/trailing icon buttons reuse
        // `add_icon_button`'s own exact real anatomy (this factory's
        // own doc comment), so they share its `"icon_button"` key too,
        // rather than inventing a third, redundant key for the
        // identical real shape.
        let icon_button_corner_radius = self
            .theme
            .borrow()
            .shape("icon_button", None)
            .unwrap_or(TOP_APP_BAR_ICON_BUTTON_SIZE as f64 / 2.0);

        let mut tree = self.tree.borrow_mut();
        let bar_width = width.unwrap_or(self.width.get() as f32);

        let mut bar_style = positioned_style(
            Size {
                width: length(bar_width),
                height: length(TOP_APP_BAR_HEIGHT),
            },
            x,
            y,
        );
        bar_style.display = taffy::Display::Flex;
        bar_style.align_items = Some(AlignItems::CENTER);
        bar_style.padding = TaffyRect {
            left: length(TOP_APP_BAR_HORIZONTAL_PADDING),
            right: length(TOP_APP_BAR_HORIZONTAL_PADDING),
            top: zero(),
            bottom: zero(),
        };
        let bar = tree.insert(
            NodeKind::Rect,
            bar_style,
            PaintProperties::new(container_color, 0.0, 0.0, 1.0),
        );

        let mut top_app_bar_leading_ids: Option<(NodeId, NodeId)> = None;
        let leading = if let Some(path) = leading_path {
            let leading_container = tree.insert(
                NodeKind::Rect,
                Style {
                    size: Size {
                        width: length(TOP_APP_BAR_ICON_BUTTON_SIZE),
                        height: length(TOP_APP_BAR_ICON_BUTTON_SIZE),
                    },
                    display: taffy::Display::Flex,
                    justify_content: Some(JustifyContent::CENTER),
                    align_items: Some(AlignItems::CENTER),
                    ..Default::default()
                },
                PaintProperties::new(TRANSPARENT, icon_button_corner_radius, 0.0, 1.0),
            );
            let icon_id = tree.insert(
                NodeKind::Icon(IconState::new(path, leading_icon_color)),
                Style {
                    size: Size {
                        width: length(TOP_APP_BAR_ICON_SIZE),
                        height: length(TOP_APP_BAR_ICON_SIZE),
                    },
                    ..Default::default()
                },
                PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
            );
            tree.add_child(leading_container, icon_id);
            tree.add_child(bar, leading_container);
            top_app_bar_leading_ids = Some((leading_container, icon_id));
            Some(self.wrap_node(leading_container))
        } else {
            None
        };

        let headline_style = Style {
            flex_grow: 1.0,
            size: Size {
                width: auto(),
                height: length(TOP_APP_BAR_HEADLINE_FONT_SIZE + 4.0),
            },
            margin: TaffyRect {
                left: length(if leading.is_some() {
                    0.0
                } else {
                    TOP_APP_BAR_HEADLINE_START_PADDING
                }),
                right: zero(),
                top: zero(),
                bottom: zero(),
            },
            ..Default::default()
        };
        let headline_id = tree.insert(
            NodeKind::Text(TextState {
                content: title.to_string(),
                font_family: "Roboto".to_string(),
                font_weight: TOP_APP_BAR_HEADLINE_FONT_WEIGHT,
                font_size: TOP_APP_BAR_HEADLINE_FONT_SIZE,
                align: TextAlign::Start,
            }),
            headline_style,
            PaintProperties::new(headline_color, 0.0, 0.0, 1.0),
        );
        tree.add_child(bar, headline_id);

        let mut trailing = Vec::with_capacity(trailing_paths.len());
        let mut top_app_bar_trailing_ids: Vec<(NodeId, NodeId)> =
            Vec::with_capacity(trailing_paths.len());
        for path in trailing_paths {
            let trailing_container = tree.insert(
                NodeKind::Rect,
                Style {
                    size: Size {
                        width: length(TOP_APP_BAR_ICON_BUTTON_SIZE),
                        height: length(TOP_APP_BAR_ICON_BUTTON_SIZE),
                    },
                    display: taffy::Display::Flex,
                    justify_content: Some(JustifyContent::CENTER),
                    align_items: Some(AlignItems::CENTER),
                    margin: TaffyRect {
                        left: length(TOP_APP_BAR_TRAILING_ICON_GAP),
                        right: zero(),
                        top: zero(),
                        bottom: zero(),
                    },
                    ..Default::default()
                },
                PaintProperties::new(TRANSPARENT, icon_button_corner_radius, 0.0, 1.0),
            );
            let icon_id = tree.insert(
                NodeKind::Icon(IconState::new(path, trailing_icon_color)),
                Style {
                    size: Size {
                        width: length(TOP_APP_BAR_ICON_SIZE),
                        height: length(TOP_APP_BAR_ICON_SIZE),
                    },
                    ..Default::default()
                },
                PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
            );
            tree.add_child(trailing_container, icon_id);
            tree.add_child(bar, trailing_container);
            top_app_bar_trailing_ids.push((trailing_container, icon_id));
            trailing.push(self.wrap_node(trailing_container));
        }

        tree.add_child(self.root, bar);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(top_app_bar_retheme_hook(
                bar,
                headline_id,
                top_app_bar_leading_ids,
                top_app_bar_trailing_ids,
            ));
        Ok((self.wrap_node(bar), leading, trailing))
    }

    /// M35 Phase 1 (§5, §7): a real MD3 Toolbar -- "docked" (spans the
    /// full window width by default, square corners -- real MD3
    /// anatomy explicitly warns against rounding a docked toolbar's
    /// corners, "can imply the container expands... upon interaction")
    /// or "floating" (hugs its own content by default, always fully
    /// rounded, real elevation, horizontal or vertical). Real anatomy
    /// verified directly against the local MD3 spec mirror
    /// (`COMPONENT_TOOLBARS.md`) before writing any code: 64dp height
    /// for both variants (`TOOLBAR_HEIGHT`), `surface_container` fill
    /// for the "standard" color config, `primary_container` for
    /// "vibrant" -- the identical two real container roles the spec's
    /// own "1. Surface container" / "1. Primary container" color
    /// diagrams name. **Real, honest gap, stated directly:** the
    /// scraped spec names "Floating toolbars have elevation by
    /// default" but the elevation *level* itself isn't a discrete
    /// token anywhere in the mirrored pages (the identical honest
    /// caveat `TOP_APP_BAR_ICON_BUTTON_SIZE`'s own doc comment already
    /// states for a different constant) -- reuses `FAB_REST_ELEVATION_
    /// LEVEL` (3.0), this catalog's own closest real, already-verified
    /// "floating, elevated chrome" reference point, rather than
    /// guessing a new number.
    ///
    /// A real "container with configurable slots" per MD3's own
    /// anatomy section, verbatim -- deliberately not a specialized
    /// children-list parameter: the caller populates the returned
    /// `Node` with any already-built `Button`/`IconButton`/`TextField`/
    /// etc. node via the existing, generic `Node.add_child` (M6 Phase
    /// 1), the identical real "engine provides the primitive, app
    /// composes" split this catalog's own `clip_children` (M32 Phase
    /// 3) already established for an analogous "don't invent a new
    /// mechanism when composition already covers it" case. `width`/
    /// `height` follow every other `add_*` method's own established
    /// meaning (a literal override of that axis), just with real,
    /// per-variant/orientation defaults: a docked toolbar defaults its
    /// width to the whole window (`self.width`, matching `add_top_app_
    /// bar`'s own convention) and its height to 64dp; a floating
    /// toolbar defaults its *length* axis (width if horizontal, height
    /// if vertical) to `auto()` -- hugging whatever real children the
    /// caller adds, matching `Button Group`'s own real "the container
    /// hugs the width of the buttons inside" anatomy -- and its
    /// *thickness* axis to 64dp, which also doubles as the real corner
    /// radius input for "fully rounded" (`thickness / 2.0`, the
    /// identical `SIZE / 2.0` pill-shape convention `FAB`/`Chip`/`Icon
    /// Button` already establish elsewhere in this catalog).
    #[pyo3(signature = (variant="docked", orientation=None, color=None, width=None, height=None, x=None, y=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_toolbar(
        &self,
        variant: &str,
        orientation: Option<&str>,
        color: Option<&str>,
        width: Option<f32>,
        height: Option<f32>,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        let is_floating = match variant {
            "docked" => false,
            "floating" => true,
            other => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "unknown toolbar variant {other:?} -- expected one of \"docked\", \
                     \"floating\""
                )));
            }
        };
        let vertical = match orientation.unwrap_or("horizontal") {
            "horizontal" => false,
            "vertical" => true,
            other => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "unknown toolbar orientation {other:?} -- expected one of \"horizontal\", \
                     \"vertical\""
                )));
            }
        };
        if vertical && !is_floating {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "a docked toolbar is always horizontal -- MD3's own real anatomy has no \
                 vertical docked variant; pass variant=\"floating\" for a vertical toolbar",
            ));
        }

        let container_color = {
            let theme = self.theme.borrow();
            let role = |name: &str, fallback: Color| -> Color {
                if theme.is_set() {
                    theme.role(name).unwrap_or(fallback)
                } else {
                    fallback
                }
            };
            match color.unwrap_or("standard") {
                "standard" => role("surface_container", Md3Baseline::SURFACE_CONTAINER),
                "vibrant" => role("primary_container", Md3Baseline::PRIMARY_CONTAINER),
                other => {
                    return Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "unknown toolbar color {other:?} -- expected one of \"standard\", \
                         \"vibrant\""
                    )));
                }
            }
        };

        let mut tree = self.tree.borrow_mut();

        let size = if is_floating {
            if vertical {
                Size {
                    width: length(width.unwrap_or(TOOLBAR_HEIGHT)),
                    height: height.map(length).unwrap_or_else(auto),
                }
            } else {
                Size {
                    width: width.map(length).unwrap_or_else(auto),
                    height: length(height.unwrap_or(TOOLBAR_HEIGHT)),
                }
            }
        } else {
            Size {
                width: length(width.unwrap_or(self.width.get() as f32)),
                height: length(height.unwrap_or(TOOLBAR_HEIGHT)),
            }
        };

        let default_corner_radius = if is_floating {
            f64::from(if vertical {
                width.unwrap_or(TOOLBAR_HEIGHT)
            } else {
                height.unwrap_or(TOOLBAR_HEIGHT)
            }) / 2.0
        } else {
            0.0
        };
        let default_elevation = if is_floating {
            FAB_REST_ELEVATION_LEVEL
        } else {
            0.0
        };
        // M50 Phase 2: keyed by the real `variant` string ("docked"/
        // "floating") -- the two genuinely pick different shape/
        // elevation, unlike most of this catalog's other components.
        let corner_radius = self
            .theme
            .borrow()
            .shape("toolbar", Some(variant))
            .unwrap_or(default_corner_radius);
        let elevation = self
            .theme
            .borrow()
            .elevation("toolbar", Some(variant))
            .unwrap_or(default_elevation);

        let mut bar_style = positioned_style(size, x, y);
        bar_style.display = taffy::Display::Flex;
        bar_style.flex_direction = if vertical {
            taffy::FlexDirection::Column
        } else {
            taffy::FlexDirection::Row
        };
        bar_style.align_items = Some(AlignItems::CENTER);
        bar_style.justify_content = Some(JustifyContent::CENTER);
        bar_style.padding = TaffyRect {
            left: length(TOOLBAR_PADDING),
            right: length(TOOLBAR_PADDING),
            top: length(TOOLBAR_PADDING),
            bottom: length(TOOLBAR_PADDING),
        };
        bar_style.gap = Size {
            width: length(if vertical { 0.0 } else { TOOLBAR_ITEM_GAP }),
            height: length(if vertical { TOOLBAR_ITEM_GAP } else { 0.0 }),
        };

        let bar = tree.insert(
            NodeKind::Rect,
            bar_style,
            PaintProperties::new(container_color, corner_radius, elevation, 1.0),
        );
        tree.add_child(self.root, bar);
        drop(tree);
        self.retheme_hooks.borrow_mut().push(toolbar_retheme_hook(
            bar,
            variant.to_string(),
            color.map(str::to_string),
            is_floating,
            vertical,
            width,
            height,
        ));
        Ok(self.wrap_node(bar))
    }

    /// M35 Phase 2 (§5, §7, §8): a real MD3 Split Button -- a leading
    /// button (reuses `add_button` directly, unchanged, since a real
    /// Split Button's own leading button "uses the same color schemes
    /// as standard buttons," `COMPONENT_SPLIT_BUTTONS.md`'s own words)
    /// plus a real, separate trailing menu-icon button, joined by the
    /// real xsmall `between-space` token (`SPLIT_BUTTON_GAP`). Real
    /// anatomy has no separate "container" element at all (unlike
    /// `Toolbar`/`Button Group`) -- the spec's own anatomy diagram
    /// lists exactly "1. Leading button, 2. Icon, 3. Label text, 4.
    /// Trailing button," so this returns `(leading, trailing,
    /// trailing_icon)` with no wrapping node, matching that real
    /// anatomy exactly rather than inventing an unneeded container.
    ///
    /// `trailing_icon` is the real, individual `NodeKind::Icon` node
    /// itself, not its own button wrapper -- the one thing this
    /// component needs to expose beyond what `add_button`/`add_top_
    /// app_bar`'s own leading/trailing icons already return, since the
    /// real "spins 180° when opened and closed" behavior (`Node.
    /// animate("rotation", ...)`, this phase's own new `IconState.
    /// rotation` field) is something the caller drives directly --
    /// Design Principle 6's own "engine provides the mechanism, app
    /// decides when a real state change happens" split, the identical
    /// real shape `Checkbox.checked`/`RadioButton.selected` already
    /// establish (the engine never toggles either on click itself).
    /// The trailing icon is always real MD3's own required
    /// `expand_more` (already a real, curated icon in this catalog,
    /// `engine_md3::icons`) -- "the trailing button should always have
    /// the expand and collapse icon... avoid modifying the icon."
    ///
    /// M38 Phase 4 (§5, §7): the inner corners' own real hover-driven
    /// shape-tightening (`COMPONENT_SPLIT_BUTTONS.md`'s own "the inner
    /// corners change shape for hovered, focused, and pressed states")
    /// -- each of `leading`/`trailing` gets a real `PaintProperties.
    /// interactive_shape` pair (relaxed fully-round pill, tightened to
    /// `SPLIT_BUTTON_INNER_CORNER_RADIUS` on their own facing corners
    /// only), so `Tree::update_hover` morphs the real, already-general
    /// `shape: Animated<ShapeKey>` mechanism (M7 Phase 4) automatically
    /// while either button is hovered. **Real, deliberate v1 scope
    /// choice, stated directly:** tied to `hovered` only, not `focused`/
    /// `pressed` separately -- see `PaintProperties.interactive_shape`'s
    /// own doc comment for the full real reasoning (a mouse press can
    /// only ever land on an already-hovered node). The real per-size
    /// `xsmall` token set is used directly (`SPLIT_BUTTON_GAP`/
    /// `SPLIT_BUTTON_TRAILING_ICON_SIZE`/`SPLIT_BUTTON_INNER_CORNER_
    /// RADIUS`) rather than a discrete XS/S/M/L/XL size-class parameter,
    /// matching `add_button`'s own existing convention of always taking
    /// a literal real `width`/`height`, never a size enum.
    #[pyo3(signature = (label, width, height, variant="filled", x=None, y=None))]
    fn add_split_button(
        &self,
        label: &str,
        width: f32,
        height: f32,
        variant: &str,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<(Node, Node, Node)> {
        let leading = self.add_button(label, width, height, variant, x, y)?;

        let colors = resolve_button_colors(&self.theme.borrow(), variant, "button")?;
        let trailing_path = resolve_icon_path("expand_more")?;
        let trailing_x = x.map(|x| x + width + SPLIT_BUTTON_GAP);

        // M50 Phase 2: `rest_radius` recomputes the *exact* same
        // theme-or-formula value `add_button` above already resolved
        // for `leading` internally (same `"button"` key, same
        // `variant`, same `height` -- the two calls are guaranteed to
        // agree, not read back from the tree, which would be an
        // equivalent but more roundabout way to get the identical
        // number). **Real bug caught before this shipped, not by
        // inspection alone:** the shape-morph code below used to
        // recompute `h / 2.0` as a fresh, independent literal for
        // `leading_relaxed`/`trailing_relaxed` -- if a theme overrides
        // `"button"`'s corner radius, `leading`'s own `paint.corner_
        // radius` (set correctly, inside `add_button`) and its `paint.
        // shape` (used for the actual painted outline) would silently
        // disagree, the *shape* still painting the old, un-themed pill.
        // `tightened_radius` gets its own, genuinely `split_button`-
        // specific key -- there is no equivalent concept in plain
        // `add_button` to inherit it from.
        let rest_radius = self
            .theme
            .borrow()
            .shape("button", Some(variant))
            .unwrap_or_else(|| f64::from(height) / 2.0);
        let tightened_radius = self
            .theme
            .borrow()
            .shape("split_button", Some("tightened"))
            .unwrap_or(SPLIT_BUTTON_INNER_CORNER_RADIUS);

        let mut tree = self.tree.borrow_mut();

        // M38 Phase 4 (§5, §7): real inner-corner shape-tightening --
        // `leading`'s own two right corners (facing the trailing
        // button across `SPLIT_BUTTON_GAP`) tighten to `tightened_
        // radius` on hover; its two left (outer) corners stay fully
        // round always. `[top_left, top_right, bottom_right, bottom_
        // left]`, `PaintProperties.corner_radii_override`'s own real
        // ordering -- `RoundedRect::new`'s 4-tuple corner-radii
        // constructor takes the identical order (confirmed via
        // `GeometryCache::rounded_rect_fill_per_corner`'s own real
        // call, `geometry_cache.rs`).
        let h = f64::from(height);
        let leading_w = f64::from(width);
        let leading_relaxed = ShapeKey::from_path(
            &RoundedRect::new(0.0, 0.0, leading_w, h, rest_radius).to_path(0.1),
        );
        let leading_tightened = ShapeKey::from_path(
            &RoundedRect::new(
                0.0,
                0.0,
                leading_w,
                h,
                (rest_radius, tightened_radius, tightened_radius, rest_radius),
            )
            .to_path(0.1),
        );
        if let Some(leading_node) = tree.get_mut(leading.id) {
            leading_node.paint.shape = Animated::new(leading_relaxed.clone());
            leading_node.paint.interactive_shape = Some((leading_relaxed, leading_tightened));
        }

        let mut trailing_paint =
            PaintProperties::new(colors.container, rest_radius, colors.elevation, 1.0);
        trailing_paint.border_color = Animated::new(colors.border_color);
        trailing_paint.border_width = Animated::new(colors.border_width);
        // M38 Phase 4 (§5, §7): `trailing`'s own real inner-corner
        // sibling -- its two *left* corners face `leading` across the
        // same gap, so they tighten instead of the right ones.
        let trailing_relaxed =
            ShapeKey::from_path(&RoundedRect::new(0.0, 0.0, h, h, rest_radius).to_path(0.1));
        let trailing_tightened = ShapeKey::from_path(
            &RoundedRect::new(
                0.0,
                0.0,
                h,
                h,
                (tightened_radius, rest_radius, rest_radius, tightened_radius),
            )
            .to_path(0.1),
        );
        trailing_paint.shape = Animated::new(trailing_relaxed.clone());
        trailing_paint.interactive_shape = Some((trailing_relaxed, trailing_tightened));
        let mut trailing_style = positioned_style(
            Size {
                width: length(height),
                height: length(height),
            },
            trailing_x,
            y,
        );
        trailing_style.display = taffy::Display::Flex;
        trailing_style.justify_content = Some(JustifyContent::CENTER);
        trailing_style.align_items = Some(AlignItems::CENTER);
        let trailing = tree.insert(NodeKind::Rect, trailing_style, trailing_paint);

        let icon_id = tree.insert(
            NodeKind::Icon(IconState::new(trailing_path, colors.label)),
            Style {
                size: Size {
                    width: length(SPLIT_BUTTON_TRAILING_ICON_SIZE),
                    height: length(SPLIT_BUTTON_TRAILING_ICON_SIZE),
                },
                ..Default::default()
            },
            PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
        );
        tree.add_child(trailing, icon_id);
        tree.add_child(self.root, trailing);
        drop(tree);
        // M52 Phase 3: `leading` already got its own `button_retheme_
        // hook` registered *inside* the `self.add_button(...)` call
        // above -- that hook already correctly re-resolves `leading`'s
        // own background/corner_radius/elevation/border live. This new
        // hook only layers the *additional* shape-morph geometry on top
        // (disjoint fields: `paint.shape`/`interactive_shape`, never
        // touched by the plain button hook) plus everything `trailing`/
        // `icon_id` need, since neither went through `add_button` at
        // all. Two hooks firing for the same `leading.id`, writing
        // disjoint fields, is a real, deliberate emergent property of
        // the hook-vec design, not a workaround.
        self.retheme_hooks
            .borrow_mut()
            .push(split_button_retheme_hook(
                leading.id,
                trailing,
                icon_id,
                variant.to_string(),
                width,
                height,
            ));

        Ok((leading, self.wrap_node(trailing), self.wrap_node(icon_id)))
    }

    /// M35 Phase 3 (§5, §7, §11.7): a real MD3 Standard Button Group --
    /// an invisible container that adds real horizontal spacing
    /// between its own children and, when one of them is genuinely
    /// pressed, grows it while shrinking its immediate neighbors,
    /// real MD3's own distinctive "pressing a button also affects the
    /// width of adjacent buttons" mechanic (`COMPONENT_BUTTON_GROUPS.
    /// md`). The real mechanism itself lives in `engine-core::Tree::
    /// sync_button_group_layouts`, driven by the new `PaintProperties.
    /// button_group_reflow` marker this container's own paint carries
    /// -- see that field's own doc comment for the full real design
    /// reasoning (mirrors `Carousel`'s own `sync_carousel_layouts`
    /// shape, reads the already-existing, already-tracked `Tree.
    /// pressed` field, no new interaction wiring needed).
    ///
    /// Each real child is a plain `add_button` (reused directly,
    /// unchanged -- "avoid mixing color styles," `COMPONENT_BUTTON_
    /// GROUPS.md`'s own real usage guidance, so every child in one
    /// group shares the identical real `variant`), then reparented
    /// under the new group container via `Tree::try_add_child` (the
    /// identical real "move an already-attached node" mechanism
    /// `Node.add_child`'s own Python-facing method already uses) --
    /// `add_button` itself always parents fresh under `self.root`
    /// first, so this step is required, not an optimization. Real MD3
    /// anatomy: "the standard button group hugs the width of the
    /// buttons inside" -- the returned container's own real width is
    /// computed from the real, uniform `width`/`labels.len()`/gap
    /// this call is given, not measured after the fact.
    ///
    /// **Real, honest v1 scope limit:** no `Connected Button Group`
    /// variant here -- MD3's own spec states it directly replaces the
    /// already-built `Segmented Button` (M30 Phase 6), so `add_
    /// segmented_button` already covers that real anatomy; this method
    /// is deliberately Standard-only, the one genuinely new mechanic.
    /// Every child uses the same, real, uniform `width`/`height` --
    /// real MD3 anatomy states "by default, all buttons in a standard
    /// group should be the same size," so this isn't a missing
    /// per-child override, it's the real default behavior itself.
    #[pyo3(signature = (labels, width, height, variant="filled", x=None, y=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_button_group(
        &self,
        labels: Vec<String>,
        width: f32,
        height: f32,
        variant: &str,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<(Node, Vec<Node>)> {
        let n = labels.len();
        let group_width = if n == 0 {
            0.0
        } else {
            n as f32 * width + (n - 1) as f32 * BUTTON_GROUP_GAP
        };

        let mut group_paint = PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0);
        group_paint.button_group_reflow =
            Some((f64::from(BUTTON_GROUP_GROW), f64::from(BUTTON_GROUP_GAP)));
        let group_style = positioned_style(
            Size {
                width: length(group_width),
                height: length(height),
            },
            x,
            y,
        );
        let group_id = {
            let mut tree = self.tree.borrow_mut();
            let group_id = tree.insert(NodeKind::Container, group_style, group_paint);
            tree.add_child(self.root, group_id);
            group_id
        };

        // M38 Phase 5 (§5, §7): the real, uniform relaxed/tightened
        // shapes every child shares -- built once outside the loop
        // since every real Standard Button Group child uses the
        // identical `width`/`height` (real MD3 anatomy, `add_button_
        // group`'s own doc comment above). M50 Phase 2: `rest_radius`
        // recomputes the exact same theme-or-formula value each child's
        // own `self.add_button(...)` call already resolved internally
        // (same `"button"` key/`variant`/`height`) -- the identical
        // "recompute, don't read back" reasoning `add_split_button`'s
        // own doc comment states, and the identical real bug class it
        // caught (a themed `paint.corner_radius` silently disagreeing
        // with an un-themed `paint.shape`) if this weren't done.
        // `tightened_radius` gets its own `button_group`-specific key --
        // no equivalent concept in plain `add_button` to inherit it from.
        let group_h = f64::from(height);
        let group_w = f64::from(width);
        let rest_radius = self
            .theme
            .borrow()
            .shape("button", Some(variant))
            .unwrap_or(group_h / 2.0);
        let tightened_radius = self
            .theme
            .borrow()
            .shape("button_group", Some("tightened"))
            .unwrap_or_else(|| button_group_pressed_corner_radius(group_h));
        let child_relaxed = ShapeKey::from_path(
            &RoundedRect::new(0.0, 0.0, group_w, group_h, rest_radius).to_path(0.1),
        );
        let child_tightened = ShapeKey::from_path(
            &RoundedRect::new(0.0, 0.0, group_w, group_h, tightened_radius).to_path(0.1),
        );

        let mut children = Vec::with_capacity(n);
        let mut child_ids: Vec<NodeId> = Vec::with_capacity(n);
        for label in &labels {
            let button = self.add_button(label, width, height, variant, None, None)?;
            let mut tree = self.tree.borrow_mut();
            tree.try_add_child(group_id, button.id);
            if let Some(node) = tree.get_mut(button.id) {
                node.paint.shape = Animated::new(child_relaxed.clone());
                node.paint.press_interactive_shape =
                    Some((child_relaxed.clone(), child_tightened.clone()));
            }
            child_ids.push(button.id);
            children.push(button);
        }

        // M52 Phase 3: same emergent-composition reasoning `add_split_
        // button`'s own hook uses -- each child already got its own
        // `button_retheme_hook` registered inside `self.add_button(...)`
        // above, correctly re-resolving background/corner_radius/
        // elevation/border live. This new hook only layers the
        // additional `paint.shape`/`press_interactive_shape` geometry on
        // top (disjoint fields), for every child at once.
        self.retheme_hooks
            .borrow_mut()
            .push(button_group_retheme_hook(
                child_ids,
                variant.to_string(),
                width,
                height,
            ));

        Ok((self.wrap_node(group_id), children))
    }

    /// M30 Phase 5 Step 4 (§5, §7): `Tabs`, MD3's real *Primary
    /// Navigation Tab* variant (Secondary is its own separately-
    /// tokened real variant, deliberately out of scope for this step
    /// -- see the `TAB_*` constants above for the full real finding).
    /// Real anatomy: a `surface`-filled, flat (`level0`) 48dp-tall row
    /// divided evenly across `labels.len()` tabs, each a real 3dp
    /// active-indicator bar (`primary` fill when active, rounded only
    /// on its own top corners via `PaintProperties.corner_radii_
    /// override` -- `Segmented Button`'s own already-real universal
    /// capability) sitting at the tab's own bottom edge, below a
    /// centered optional icon + Title Small label (active: `primary`;
    /// inactive: `on_surface_variant`).
    ///
    /// **Real, deliberate architectural choice, the same real dividing
    /// line `Segmented Button`/`Filter Chip`/`Navigation Rail`/
    /// `Navigation Drawer` already established:** a tab's own "active"
    /// state is app-owned group-select state (Design Principle 6), not
    /// a new engine `NodeKind` -- returns `Vec<Node>` (`Segmented
    /// Button`'s own exact real return shape), the row's own frame
    /// never returned. **Real, confirmed repeat of `Navigation Rail`'s
    /// own hit-test bug, caught live by this step's own click-
    /// dispatch test, not avoided by geometry alone as first assumed:**
    /// the 3dp indicator itself never overlaps a tab's own geometric
    /// center, but the *content* wrapper around the icon/label (a
    /// plain `Rect`, used purely for its own real flex-centering
    /// layout) does -- and a plain `Rect` always independently claims
    /// a hit exactly like `Navigation Rail`'s indicator did. Fixed the
    /// identical real way: `Tree::set_hit_testable(content, false)`,
    /// this milestone's second real use of the capability `Navigation
    /// Rail` added, not a new one invented here.
    #[pyo3(signature = (labels, icons=None, selected=None, width=None, x=None, y=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_tabs(
        &self,
        labels: Vec<String>,
        icons: Option<Vec<String>>,
        selected: Option<usize>,
        width: Option<f32>,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Vec<Node>> {
        if labels.is_empty() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "add_tabs needs at least 1 item",
            ));
        }
        if let Some(icons) = &icons
            && icons.len() != labels.len()
        {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "add_tabs needs one icon per label when icons are given -- got {} labels and {} \
                 icons",
                labels.len(),
                icons.len()
            )));
        }
        if let Some(sel) = selected
            && sel >= labels.len()
        {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "selected index {sel} is out of range for {} items",
                labels.len()
            )));
        }
        let icon_paths: Option<Vec<_>> = icons
            .map(|icons| {
                icons
                    .iter()
                    .map(|name| resolve_icon_path(name))
                    .collect::<PyResult<Vec<_>>>()
            })
            .transpose()?;

        let (container_color, active_color, inactive_color) = {
            let theme = self.theme.borrow();
            let role = |name: &str, fallback: Color| -> Color {
                if theme.is_set() {
                    theme.role(name).unwrap_or(fallback)
                } else {
                    fallback
                }
            };
            (
                role("surface", Md3Baseline::SURFACE),
                role("primary", Md3Baseline::PRIMARY),
                role("on_surface_variant", Md3Baseline::ON_SURFACE_VARIANT),
            )
        };
        // M50 Phase 4: the row itself stays un-themed (always flat/
        // square, deliberate MD3 anatomy, the same "corner-none"
        // precedent as `add_navigation_rail`'s own frame) -- only the
        // active-indicator's real const gets an override.
        let indicator_corner_radius = self
            .theme
            .borrow()
            .shape("tabs", Some("indicator"))
            .unwrap_or(TAB_INDICATOR_CORNER_RADIUS);

        let mut tree = self.tree.borrow_mut();
        let row_width = width.unwrap_or(self.width.get() as f32);
        let tab_width = row_width / labels.len() as f32;

        let mut row_style = positioned_style(
            Size {
                width: length(row_width),
                height: length(TAB_HEIGHT),
            },
            x,
            y,
        );
        row_style.display = taffy::Display::Flex;
        let row = tree.insert(
            NodeKind::Rect,
            row_style,
            PaintProperties::new(container_color, 0.0, 0.0, 1.0),
        );

        let mut tabs = Vec::with_capacity(labels.len());
        let mut tab_retheme_data: Vec<(Option<NodeId>, NodeId, NodeId, bool)> =
            Vec::with_capacity(labels.len());
        for (i, label) in labels.into_iter().enumerate() {
            let is_active = selected == Some(i);
            let color = if is_active {
                active_color
            } else {
                inactive_color
            };

            let tab_style = Style {
                size: Size {
                    width: length(tab_width),
                    height: length(TAB_HEIGHT),
                },
                display: taffy::Display::Flex,
                flex_direction: taffy::FlexDirection::Column,
                ..Default::default()
            };
            let tab = tree.insert(
                NodeKind::Rect,
                tab_style,
                PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
            );

            let content_style = Style {
                flex_grow: 1.0,
                size: Size {
                    width: length(tab_width),
                    height: auto(),
                },
                display: taffy::Display::Flex,
                flex_direction: taffy::FlexDirection::Column,
                justify_content: Some(JustifyContent::CENTER),
                align_items: Some(AlignItems::CENTER),
                gap: Size {
                    width: length(0.0),
                    height: length(TAB_ICON_LABEL_GAP),
                },
                ..Default::default()
            };
            let content = tree.insert(
                NodeKind::Rect,
                content_style,
                PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
            );
            // M30 Phase 5 Step 4 (§5, §7): a real, confirmed repeat of
            // `Navigation Rail`'s own hit-test bug, caught live by
            // this step's own click-dispatch test, not avoided by
            // geometry alone as first assumed -- `content` is a
            // purely decorative layout wrapper around the icon/label,
            // and (like `Navigation Rail`'s indicator) a plain `Rect`
            // always independently claims a hit, stealing every click
            // meant for `tab`'s own registered handler. `Node.
            // hit_testable`'s own doc comment has the full real
            // finding this reuses a second time.
            tree.set_hit_testable(content, false);

            let mut tab_icon_id: Option<NodeId> = None;
            if let Some(paths) = &icon_paths {
                let icon_id = tree.insert(
                    NodeKind::Icon(IconState::new(paths[i].clone(), color)),
                    Style {
                        size: Size {
                            width: length(TAB_ICON_SIZE),
                            height: length(TAB_ICON_SIZE),
                        },
                        ..Default::default()
                    },
                    PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
                );
                tree.add_child(content, icon_id);
                tab_icon_id = Some(icon_id);
            }

            let label_id = tree.insert(
                NodeKind::Text(TextState {
                    content: label,
                    font_family: "Roboto".to_string(),
                    font_weight: TAB_LABEL_FONT_WEIGHT,
                    font_size: TAB_LABEL_FONT_SIZE,
                    align: TextAlign::Center,
                }),
                Style {
                    size: Size {
                        width: length(tab_width),
                        height: length(TAB_LABEL_FONT_SIZE + 4.0),
                    },
                    ..Default::default()
                },
                PaintProperties::new(color, 0.0, 0.0, 1.0),
            );
            tree.add_child(content, label_id);
            tree.add_child(tab, content);

            let indicator_fill = if is_active { active_color } else { TRANSPARENT };
            let mut indicator_paint = PaintProperties::new(indicator_fill, 0.0, 0.0, 1.0);
            indicator_paint.corner_radii_override =
                Some([indicator_corner_radius, indicator_corner_radius, 0.0, 0.0]);
            let indicator = tree.insert(
                NodeKind::Rect,
                Style {
                    size: Size {
                        width: length(tab_width),
                        height: length(TAB_INDICATOR_HEIGHT),
                    },
                    ..Default::default()
                },
                indicator_paint,
            );
            tree.add_child(tab, indicator);

            tree.add_child(row, tab);
            tab_retheme_data.push((tab_icon_id, label_id, indicator, is_active));
            tabs.push(self.wrap_node(tab));
        }

        tree.add_child(self.root, row);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(tabs_retheme_hook(row, tab_retheme_data));
        Ok(tabs)
    }

    /// M30 Phase 5 Step 5 (§5, §7): `Search Bar`, closing Phase 5's
    /// own component list. Real anatomy: see the `SEARCH_*` constants
    /// above for the full real finding, including the real design
    /// reusing `TextField`'s own existing `NodeKind` for the input
    /// (every one of its already-real typing/focus/selection/IME
    /// capabilities, for free) and `Icon Button`'s own real anatomy
    /// (Phase 1 Step 2) for the leading/trailing actions, the same
    /// real precedent `Top App Bar` (Step 3) already reused. Returns
    /// `(bar, text_field, leading, trailing)`: `leading` is `None`
    /// unless `leading_icon` was given; `trailing` a `Vec<Node>`, one
    /// per requested trailing icon -- the identical real "independently
    /// interactive sub-elements get their own real `Node`s" shape
    /// `Snackbar`/`Top App Bar` already established.
    #[pyo3(signature = (placeholder, width, leading_icon=None, trailing_icons=None, x=None, y=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_search_bar(
        &self,
        placeholder: &str,
        width: f32,
        leading_icon: Option<&str>,
        trailing_icons: Option<Vec<String>>,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<(Node, Node, Option<Node>, Vec<Node>)> {
        let trailing_icons = trailing_icons.unwrap_or_default();
        let leading_path = leading_icon.map(resolve_icon_path).transpose()?;
        let trailing_paths: Vec<_> = trailing_icons
            .iter()
            .map(|name| resolve_icon_path(name))
            .collect::<PyResult<Vec<_>>>()?;

        let (container_color, leading_icon_color, trailing_icon_color, input_color) = {
            let theme = self.theme.borrow();
            let role = |name: &str, fallback: Color| -> Color {
                if theme.is_set() {
                    theme.role(name).unwrap_or(fallback)
                } else {
                    fallback
                }
            };
            (
                role(
                    "surface_container_high",
                    Md3Baseline::SURFACE_CONTAINER_HIGH,
                ),
                theme.on_surface(),
                role("on_surface_variant", Md3Baseline::ON_SURFACE_VARIANT),
                theme.on_surface(),
            )
        };
        let (corner_radius, elevation) = {
            let theme = self.theme.borrow();
            (
                theme
                    .shape("search_bar", None)
                    .unwrap_or(SEARCH_BAR_CORNER_RADIUS),
                theme
                    .elevation("search_bar", None)
                    .unwrap_or(SEARCH_BAR_ELEVATION),
            )
        };

        let mut tree = self.tree.borrow_mut();

        let mut bar_style = positioned_style(
            Size {
                width: length(width),
                height: length(SEARCH_BAR_HEIGHT),
            },
            x,
            y,
        );
        bar_style.display = taffy::Display::Flex;
        bar_style.align_items = Some(AlignItems::CENTER);
        bar_style.padding = TaffyRect {
            left: length(SEARCH_BAR_HORIZONTAL_PADDING),
            right: length(SEARCH_BAR_HORIZONTAL_PADDING),
            top: zero(),
            bottom: zero(),
        };
        let bar = tree.insert(
            NodeKind::Rect,
            bar_style,
            PaintProperties::new(container_color, corner_radius, elevation, 1.0),
        );

        let mut search_bar_leading_ids: Option<(NodeId, NodeId)> = None;
        let leading = if let Some(path) = leading_path {
            let leading_container = tree.insert(
                NodeKind::Rect,
                Style {
                    size: Size {
                        width: length(SEARCH_ICON_BUTTON_SIZE),
                        height: length(SEARCH_ICON_BUTTON_SIZE),
                    },
                    display: taffy::Display::Flex,
                    justify_content: Some(JustifyContent::CENTER),
                    align_items: Some(AlignItems::CENTER),
                    ..Default::default()
                },
                PaintProperties::new(TRANSPARENT, SEARCH_ICON_BUTTON_SIZE as f64 / 2.0, 0.0, 1.0),
            );
            let icon_id = tree.insert(
                NodeKind::Icon(IconState::new(path, leading_icon_color)),
                Style {
                    size: Size {
                        width: length(SEARCH_ICON_SIZE),
                        height: length(SEARCH_ICON_SIZE),
                    },
                    ..Default::default()
                },
                PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
            );
            tree.add_child(leading_container, icon_id);
            tree.add_child(bar, leading_container);
            search_bar_leading_ids = Some((leading_container, icon_id));
            Some(self.wrap_node(leading_container))
        } else {
            None
        };

        let field_width = (width
            - 2.0 * SEARCH_BAR_HORIZONTAL_PADDING
            - if leading.is_some() {
                SEARCH_ICON_BUTTON_SIZE
            } else {
                0.0
            }
            - trailing_paths.len() as f32 * (SEARCH_ICON_BUTTON_SIZE + SEARCH_TRAILING_ICON_GAP))
            .max(0.0);
        let mut text_field_state = TextFieldState::new(
            placeholder,
            "Roboto".to_string(),
            SEARCH_INPUT_FONT_WEIGHT,
            SEARCH_INPUT_FONT_SIZE,
        );
        text_field_state.text_tint = input_color;
        let field_id = tree.insert(
            NodeKind::TextField(text_field_state),
            Style {
                flex_grow: 1.0,
                size: Size {
                    width: length(field_width),
                    height: length(SEARCH_INPUT_FONT_SIZE + 4.0),
                },
                ..Default::default()
            },
            PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
        );
        tree.set_access(
            field_id,
            AccessNodeData::new(Role::TextInput).with_action(Action::Focus),
        );
        tree.add_child(bar, field_id);
        let text_field = self.wrap_node(field_id);

        let mut trailing = Vec::with_capacity(trailing_paths.len());
        let mut search_bar_trailing_ids: Vec<(NodeId, NodeId)> =
            Vec::with_capacity(trailing_paths.len());
        for path in trailing_paths {
            let trailing_container = tree.insert(
                NodeKind::Rect,
                Style {
                    size: Size {
                        width: length(SEARCH_ICON_BUTTON_SIZE),
                        height: length(SEARCH_ICON_BUTTON_SIZE),
                    },
                    display: taffy::Display::Flex,
                    justify_content: Some(JustifyContent::CENTER),
                    align_items: Some(AlignItems::CENTER),
                    margin: TaffyRect {
                        left: length(SEARCH_TRAILING_ICON_GAP),
                        right: zero(),
                        top: zero(),
                        bottom: zero(),
                    },
                    ..Default::default()
                },
                PaintProperties::new(TRANSPARENT, SEARCH_ICON_BUTTON_SIZE as f64 / 2.0, 0.0, 1.0),
            );
            let icon_id = tree.insert(
                NodeKind::Icon(IconState::new(path, trailing_icon_color)),
                Style {
                    size: Size {
                        width: length(SEARCH_ICON_SIZE),
                        height: length(SEARCH_ICON_SIZE),
                    },
                    ..Default::default()
                },
                PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
            );
            tree.add_child(trailing_container, icon_id);
            tree.add_child(bar, trailing_container);
            search_bar_trailing_ids.push((trailing_container, icon_id));
            trailing.push(self.wrap_node(trailing_container));
        }

        tree.add_child(self.root, bar);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(search_bar_retheme_hook(
                bar,
                field_id,
                search_bar_leading_ids,
                search_bar_trailing_ids,
            ));
        Ok((self.wrap_node(bar), text_field, leading, trailing))
    }

    /// M30 Phase 5 Step 5 (§5, §7, §11.3): `Search View`, the real
    /// *docked* dropdown suggestions/results panel -- MD3's own real
    /// *full-screen* variant is a mobile pattern, excluded per this
    /// milestone's own desktop-adaptation rule. Real, deliberate
    /// design: a plain styled `Rect` container with no fixed content
    /// anatomy of its own, the identical real "engine gives primitives,
    /// app composes content" contract `Card` already established --
    /// the app populates it with its own real suggestion rows via the
    /// already-generic `Node.add_child`. Returned genuinely unattached
    /// anywhere -- shown/hidden via `Window.open_menu`/`close_menu`
    /// directly, the identical real reuse `Tooltip`'s own panel
    /// (Phase 3 Step 5) already established, not a new dedicated
    /// `open_search_view`/`close_search_view` pair duplicating them.
    #[pyo3(signature = (width, height, x=None, y=None))]
    fn add_search_view(&self, width: f32, height: f32, x: Option<f32>, y: Option<f32>) -> Node {
        let container_color = {
            let theme = self.theme.borrow();
            if theme.is_set() {
                theme
                    .role("surface_container_high")
                    .unwrap_or(Md3Baseline::SURFACE_CONTAINER_HIGH)
            } else {
                Md3Baseline::SURFACE_CONTAINER_HIGH
            }
        };
        // M50 Phase 3: its own key, not "dialog" -- reuses `DIALOG_
        // CORNER_RADIUS` as a real constant *value* today, but that's
        // a coincidence of the two components sharing a shape token,
        // not a reason to couple their theme overrides together.
        let (corner_radius, elevation) = {
            let theme = self.theme.borrow();
            (
                theme
                    .shape("search_view", None)
                    .unwrap_or(DIALOG_CORNER_RADIUS),
                theme
                    .elevation("search_view", None)
                    .unwrap_or(SEARCH_VIEW_ELEVATION),
            )
        };

        let mut tree = self.tree.borrow_mut();
        let style = positioned_style(
            Size {
                width: length(width),
                height: length(height),
            },
            x,
            y,
        );
        let id = tree.insert(
            NodeKind::Rect,
            style,
            PaintProperties::new(container_color, corner_radius, elevation, 1.0),
        );
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(search_view_retheme_hook(id));
        self.wrap_node(id)
    }

    /// M30 Phase 6 Step 1 (§5, §7): one real MD3 list item.
    /// `Chip`/`Menu Item`'s own real composition shape (`Rect` +
    /// optional leading `Icon` + `Text` + optional trailing `Icon`)
    /// extended with a real, new second real line -- see the
    /// `LIST_ITEM_TWO_LINE_HEIGHT` constant above for the full real
    /// finding. **Real, deliberate design avoiding a repeat of
    /// `Navigation Rail`/`Tabs`'s own hit-test bug, applied
    /// proactively this time, not found the hard way a third time:**
    /// the two-line variant's own headline+supporting-text block is a
    /// real, necessary `NodeKind::Container` wrapper (two stacked
    /// `Text` children need *some* grouping node) -- `Tree::
    /// set_hit_testable(text_block, false)` is applied immediately on
    /// insertion, the same real capability `Navigation Rail` added and
    /// `Tabs` already confirmed generalizes, verified again here by
    /// this step's own click-dispatch test rather than assumed safe.
    /// Attached to `self.root` immediately, matching `Menu Item`'s own
    /// real contract -- `add_list` is what re-parents it into an
    /// actual list frame, the identical real `Tree::detach`-then-
    /// `add_child` mechanism `build_menu` already established.
    #[pyo3(signature = (headline, leading_icon=None, trailing_icon=None, supporting_text=None, width=360.0, x=None, y=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_list_item(
        &self,
        headline: &str,
        leading_icon: Option<&str>,
        trailing_icon: Option<&str>,
        supporting_text: Option<&str>,
        width: f32,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        let leading_path = leading_icon.map(resolve_icon_path).transpose()?;
        let trailing_path = trailing_icon.map(resolve_icon_path).transpose()?;

        let (headline_color, icon_color, supporting_color) = {
            let theme = self.theme.borrow();
            let on_surface_variant = if theme.is_set() {
                theme
                    .role("on_surface_variant")
                    .unwrap_or(Md3Baseline::ON_SURFACE_VARIANT)
            } else {
                Md3Baseline::ON_SURFACE_VARIANT
            };
            (theme.on_surface(), on_surface_variant, on_surface_variant)
        };

        let mut tree = self.tree.borrow_mut();
        let height = if supporting_text.is_some() {
            LIST_ITEM_TWO_LINE_HEIGHT
        } else {
            MENU_ITEM_HEIGHT
        };

        let mut container_style = positioned_style(
            Size {
                width: length(width),
                height: length(height),
            },
            x,
            y,
        );
        container_style.display = taffy::Display::Flex;
        container_style.align_items = Some(AlignItems::CENTER);
        container_style.padding = TaffyRect {
            left: length(MENU_ITEM_LEADING_SPACE),
            right: length(MENU_ITEM_LEADING_SPACE),
            top: zero(),
            bottom: zero(),
        };
        container_style.gap = Size {
            width: length(MENU_ITEM_ICON_GAP),
            height: length(0.0),
        };
        let container = tree.insert(
            NodeKind::Rect,
            container_style,
            PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
        );

        let mut side_width = 0.0_f32;
        let mut leading_icon_id: Option<NodeId> = None;
        if let Some(path) = leading_path {
            side_width += MENU_ITEM_ICON_SIZE + MENU_ITEM_ICON_GAP;
            let icon_id = tree.insert(
                NodeKind::Icon(IconState::new(path, icon_color)),
                Style {
                    size: Size {
                        width: length(MENU_ITEM_ICON_SIZE),
                        height: length(MENU_ITEM_ICON_SIZE),
                    },
                    ..Default::default()
                },
                PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
            );
            tree.add_child(container, icon_id);
            leading_icon_id = Some(icon_id);
        }
        if trailing_path.is_some() {
            side_width += MENU_ITEM_ICON_SIZE + MENU_ITEM_ICON_GAP;
        }

        let text_width = (width - 2.0 * MENU_ITEM_LEADING_SPACE - side_width).max(0.0);
        let headline_node_id;
        let mut supporting_node_id: Option<NodeId> = None;
        if let Some(supporting) = supporting_text {
            let text_block = tree.insert(
                NodeKind::Container,
                Style {
                    display: taffy::Display::Flex,
                    flex_direction: taffy::FlexDirection::Column,
                    size: Size {
                        width: length(text_width),
                        height: length(LIST_ITEM_TWO_LINE_HEIGHT),
                    },
                    justify_content: Some(JustifyContent::CENTER),
                    ..Default::default()
                },
                PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
            );
            tree.set_hit_testable(text_block, false);

            let headline_id = tree.insert(
                NodeKind::Text(TextState {
                    content: headline.to_string(),
                    font_family: "Roboto".to_string(),
                    font_weight: BUTTON_LABEL_FONT_WEIGHT,
                    font_size: BUTTON_LABEL_FONT_SIZE,
                    align: TextAlign::Start,
                }),
                Style {
                    size: Size {
                        width: length(text_width),
                        height: length(BUTTON_LABEL_LINE_HEIGHT),
                    },
                    ..Default::default()
                },
                PaintProperties::new(headline_color, 0.0, 0.0, 1.0),
            );
            tree.add_child(text_block, headline_id);

            let supporting_id = tree.insert(
                NodeKind::Text(TextState {
                    content: supporting.to_string(),
                    font_family: "Roboto".to_string(),
                    font_weight: DIALOG_BODY_FONT_WEIGHT,
                    font_size: DIALOG_BODY_FONT_SIZE,
                    align: TextAlign::Start,
                }),
                Style {
                    size: Size {
                        width: length(text_width),
                        height: length(DIALOG_BODY_FONT_SIZE + 4.0),
                    },
                    ..Default::default()
                },
                PaintProperties::new(supporting_color, 0.0, 0.0, 1.0),
            );
            tree.add_child(text_block, supporting_id);
            tree.add_child(container, text_block);
            headline_node_id = headline_id;
            supporting_node_id = Some(supporting_id);
        } else {
            let headline_id = tree.insert(
                NodeKind::Text(TextState {
                    content: headline.to_string(),
                    font_family: "Roboto".to_string(),
                    font_weight: BUTTON_LABEL_FONT_WEIGHT,
                    font_size: BUTTON_LABEL_FONT_SIZE,
                    align: TextAlign::Start,
                }),
                Style {
                    size: Size {
                        width: length(text_width),
                        height: length(BUTTON_LABEL_LINE_HEIGHT),
                    },
                    ..Default::default()
                },
                PaintProperties::new(headline_color, 0.0, 0.0, 1.0),
            );
            tree.add_child(container, headline_id);
            headline_node_id = headline_id;
        }

        let mut trailing_icon_id: Option<NodeId> = None;
        if let Some(path) = trailing_path {
            let icon_id = tree.insert(
                NodeKind::Icon(IconState::new(path, icon_color)),
                Style {
                    size: Size {
                        width: length(MENU_ITEM_ICON_SIZE),
                        height: length(MENU_ITEM_ICON_SIZE),
                    },
                    ..Default::default()
                },
                PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
            );
            tree.add_child(container, icon_id);
            trailing_icon_id = Some(icon_id);
        }

        tree.add_child(self.root, container);
        drop(tree);
        self.retheme_hooks.borrow_mut().push(list_item_retheme_hook(
            headline_node_id,
            supporting_node_id,
            leading_icon_id,
            trailing_icon_id,
        ));
        Ok(self.wrap_node(container))
    }

    /// M30 Phase 6 Step 1 (§5, §7): `List`, a plain, non-virtualized
    /// vertical grouping of `add_list_item`-built rows -- the real,
    /// confirmed relationship to `VirtualList` this step's own scope
    /// note already named: small real collections, not large ones
    /// (`VirtualList`, already real since M4/M8, stays the real choice
    /// there). Real, deliberate design: no fill/elevation/shape of its
    /// own in real MD3 -- a bare structural grouping, `NodeKind::
    /// Container` (matching `AppShell`'s own real content-region
    /// precedent), individual items carry all the visual weight.
    /// **Real, deliberate reuse, not a new re-parenting mechanism:**
    /// moves each item (`Tree::detach` then `add_child`) into the
    /// returned frame, the identical real mechanism `build_menu`
    /// already established for `Menu Item`'s own real re-parenting.
    #[pyo3(signature = (items, width=360.0, x=None, y=None))]
    fn add_list(
        &self,
        items: Vec<PyRef<'_, Node>>,
        width: f32,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        if items.is_empty() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "add_list needs at least 1 item",
            ));
        }
        for item in &items {
            if !Rc::ptr_eq(&self.tree, &item.tree) {
                return Err(EngineError::ForeignNode.into());
            }
        }

        let mut tree = self.tree.borrow_mut();

        // Real, deliberate choice, not an oversight: height is `auto()`,
        // not summed from each item's own already-real height -- an
        // item's `Layout` is only ever meaningful after a real
        // `compute_layout` pass has run at least once, which this
        // method has no guarantee of (`add_list_item` only inserts and
        // attaches, it never computes layout itself). A flex column
        // with a definite cross-axis (`width`) and `auto()` main-axis
        // sizes itself from its own children's real heights during the
        // app's own next real layout pass -- the identical real
        // pattern `AppShell`'s own `content` region already uses
        // (`flex_grow: 1.0`, no explicit height).
        let mut frame_style = positioned_style(
            Size {
                width: length(width),
                height: auto(),
            },
            x,
            y,
        );
        frame_style.display = taffy::Display::Flex;
        frame_style.flex_direction = taffy::FlexDirection::Column;
        let frame = tree.insert(
            NodeKind::Container,
            frame_style,
            PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
        );

        for item in items {
            if let Some(parent) = tree.get(item.id).and_then(|node| node.parent) {
                tree.detach(parent, item.id);
            }
            tree.add_child(frame, item.id);
        }

        tree.add_child(self.root, frame);
        Ok(self.wrap_node(frame))
    }

    /// M30 Phase 6 Step 2 (§1, §3, §5, §7): `Accordion`'s own real
    /// header -- see the `ACCORDION_CHEVRON_ICON` constant above for
    /// the full real finding, including MD3's own real lack of an
    /// official Accordion page and the real, honest chevron-rotation
    /// workaround (`Affine::scale(-1.0)`, not a rotation primitive
    /// that doesn't exist). Real anatomy reuses List Item's own
    /// exactly (`MENU_ITEM_*`): 56dp height, `on_surface` headline
    /// (Label Large), a trailing 24dp chevron (`on_surface_variant`).
    /// Returns `(header, chevron)`: `header` is the real clickable
    /// row (`enable_interaction()`/`set_on_click()` toggle it exactly
    /// like any other component in this catalog -- group-exclusivity/
    /// expand-state is app-owned, Design Principle 6, the same real
    /// dividing line every other toggle-shaped component in this
    /// milestone already established); `chevron` is the real,
    /// independently-addressable `Node` the app flips via `Node.
    /// animate("transform", (0.0, 0.0, -1.0 if expanded else 1.0))`.
    /// `expanded` seeds the chevron's own real initial orientation --
    /// a true no-op (`scale: 1.0`, identity) when `false`.
    #[pyo3(signature = (title, expanded=false, width=360.0, x=None, y=None))]
    fn add_accordion_header(
        &self,
        title: &str,
        expanded: bool,
        width: f32,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<(Node, Node)> {
        let chevron_path = resolve_icon_path(ACCORDION_CHEVRON_ICON)?;

        let (headline_color, chevron_color) = {
            let theme = self.theme.borrow();
            let on_surface_variant = if theme.is_set() {
                theme
                    .role("on_surface_variant")
                    .unwrap_or(Md3Baseline::ON_SURFACE_VARIANT)
            } else {
                Md3Baseline::ON_SURFACE_VARIANT
            };
            (theme.on_surface(), on_surface_variant)
        };

        let mut tree = self.tree.borrow_mut();

        let mut header_style = positioned_style(
            Size {
                width: length(width),
                height: length(MENU_ITEM_HEIGHT),
            },
            x,
            y,
        );
        header_style.display = taffy::Display::Flex;
        header_style.align_items = Some(AlignItems::CENTER);
        header_style.padding = TaffyRect {
            left: length(MENU_ITEM_LEADING_SPACE),
            right: length(MENU_ITEM_LEADING_SPACE),
            top: zero(),
            bottom: zero(),
        };
        header_style.gap = Size {
            width: length(MENU_ITEM_ICON_GAP),
            height: length(0.0),
        };
        let header = tree.insert(
            NodeKind::Rect,
            header_style,
            PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
        );

        let headline_width =
            (width - 2.0 * MENU_ITEM_LEADING_SPACE - MENU_ITEM_ICON_SIZE - MENU_ITEM_ICON_GAP)
                .max(0.0);
        let headline_id = tree.insert(
            NodeKind::Text(TextState {
                content: title.to_string(),
                font_family: "Roboto".to_string(),
                font_weight: BUTTON_LABEL_FONT_WEIGHT,
                font_size: BUTTON_LABEL_FONT_SIZE,
                align: TextAlign::Start,
            }),
            Style {
                flex_grow: 1.0,
                size: Size {
                    width: length(headline_width),
                    height: length(BUTTON_LABEL_LINE_HEIGHT),
                },
                ..Default::default()
            },
            PaintProperties::new(headline_color, 0.0, 0.0, 1.0),
        );
        tree.add_child(header, headline_id);

        let mut chevron_paint = PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0);
        if expanded {
            chevron_paint.transform = Animated::new(Affine::scale(-1.0));
        }
        let chevron = tree.insert(
            NodeKind::Icon(IconState::new(chevron_path, chevron_color)),
            Style {
                size: Size {
                    width: length(MENU_ITEM_ICON_SIZE),
                    height: length(MENU_ITEM_ICON_SIZE),
                },
                ..Default::default()
            },
            chevron_paint,
        );
        tree.add_child(header, chevron);

        tree.add_child(self.root, header);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(accordion_header_retheme_hook(headline_id, chevron));
        Ok((self.wrap_node(header), self.wrap_node(chevron)))
    }

    /// M30 Phase 6 Step 3 (§1, §3, §5, §7): `Tree View`'s own real
    /// per-node row -- see the `TREE_NODE_INDENT_WIDTH` constant above
    /// for the full real finding, including the identical real
    /// grounding `Accordion` already established, applied recursively.
    /// Real anatomy: `Accordion`'s own header again (`MENU_ITEM_*`/
    /// `ACCORDION_CHEVRON_ICON` reused verbatim), with `depth *
    /// TREE_NODE_INDENT_WIDTH` real left padding added on top of the
    /// existing `MENU_ITEM_LEADING_SPACE` -- the one real difference
    /// "recursively" means here. Returns `(header, chevron)`, `chevron`
    /// `None` when `leaf` -- a childless node has nothing to expand,
    /// matching real desktop file-browser convention, not left for the
    /// app to fake with an invisible one. `expanded`/toggling reuse
    /// the identical real contract `Accordion`'s own header already
    /// has (group/expand state is app-owned, Design Principle 6; the
    /// chevron flips via `Node.animate("transform", ...)`, the same
    /// real `Affine::scale(-1.0)`-as-180°-flip substitute `Accordion`
    /// already established for the identical real reason -- this
    /// engine still has no rotation primitive).
    #[pyo3(signature = (title, depth=0, expanded=false, leaf=false, width=360.0, x=None, y=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_tree_node(
        &self,
        title: &str,
        depth: usize,
        expanded: bool,
        leaf: bool,
        width: f32,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<(Node, Option<Node>)> {
        let chevron_path = if leaf {
            None
        } else {
            Some(resolve_icon_path(ACCORDION_CHEVRON_ICON)?)
        };

        let (headline_color, chevron_color) = {
            let theme = self.theme.borrow();
            let on_surface_variant = if theme.is_set() {
                theme
                    .role("on_surface_variant")
                    .unwrap_or(Md3Baseline::ON_SURFACE_VARIANT)
            } else {
                Md3Baseline::ON_SURFACE_VARIANT
            };
            (theme.on_surface(), on_surface_variant)
        };

        let mut tree = self.tree.borrow_mut();
        let indent = depth as f32 * TREE_NODE_INDENT_WIDTH;
        let leading_padding = MENU_ITEM_LEADING_SPACE + indent;

        let mut header_style = positioned_style(
            Size {
                width: length(width),
                height: length(MENU_ITEM_HEIGHT),
            },
            x,
            y,
        );
        header_style.display = taffy::Display::Flex;
        header_style.align_items = Some(AlignItems::CENTER);
        header_style.padding = TaffyRect {
            left: length(leading_padding),
            right: length(MENU_ITEM_LEADING_SPACE),
            top: zero(),
            bottom: zero(),
        };
        header_style.gap = Size {
            width: length(MENU_ITEM_ICON_GAP),
            height: length(0.0),
        };
        let header = tree.insert(
            NodeKind::Rect,
            header_style,
            PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
        );

        let chevron_reserved = if chevron_path.is_some() {
            MENU_ITEM_ICON_SIZE + MENU_ITEM_ICON_GAP
        } else {
            0.0
        };
        let headline_width =
            (width - leading_padding - MENU_ITEM_LEADING_SPACE - chevron_reserved).max(0.0);

        let mut chevron_id: Option<NodeId> = None;
        let chevron = if let Some(path) = chevron_path {
            let mut chevron_paint = PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0);
            if expanded {
                chevron_paint.transform = Animated::new(Affine::scale(-1.0));
            }
            let id = tree.insert(
                NodeKind::Icon(IconState::new(path, chevron_color)),
                Style {
                    size: Size {
                        width: length(MENU_ITEM_ICON_SIZE),
                        height: length(MENU_ITEM_ICON_SIZE),
                    },
                    ..Default::default()
                },
                chevron_paint,
            );
            tree.add_child(header, id);
            chevron_id = Some(id);
            Some(self.wrap_node(id))
        } else {
            None
        };

        let headline_id = tree.insert(
            NodeKind::Text(TextState {
                content: title.to_string(),
                font_family: "Roboto".to_string(),
                font_weight: BUTTON_LABEL_FONT_WEIGHT,
                font_size: BUTTON_LABEL_FONT_SIZE,
                align: TextAlign::Start,
            }),
            Style {
                flex_grow: 1.0,
                size: Size {
                    width: length(headline_width),
                    height: length(BUTTON_LABEL_LINE_HEIGHT),
                },
                ..Default::default()
            },
            PaintProperties::new(headline_color, 0.0, 0.0, 1.0),
        );
        tree.add_child(header, headline_id);

        tree.add_child(self.root, header);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(tree_node_retheme_hook(headline_id, chevron_id));
        Ok((self.wrap_node(header), chevron))
    }

    /// M30 Phase 7 Step 1 (§5, §7): `Date Picker`'s own real day cell
    /// -- see the `DATE_CELL_*` constants above for the full real
    /// finding, including this step's own deliberate scope (only the
    /// cell, no engine-owned calendar arithmetic). Real, deliberate
    /// state precedence, matching MD3's own real visual priority:
    /// `selected` wins over `today` (a selected today still shows the
    /// filled `primary` circle, not the outline) -- both are real,
    /// independent booleans the app computes itself from its own real
    /// date model, not mutually exclusive at the type level.
    #[pyo3(signature = (day, selected=false, today=false, outside_month=false, x=None, y=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_date_picker_day(
        &self,
        day: u32,
        selected: bool,
        today: bool,
        outside_month: bool,
        x: Option<f32>,
        y: Option<f32>,
    ) -> Node {
        let (fill, border_color, border_width, label_color) = {
            let theme = self.theme.borrow();
            let role = |name: &str, fallback: Color| -> Color {
                if theme.is_set() {
                    theme.role(name).unwrap_or(fallback)
                } else {
                    fallback
                }
            };
            let primary = role("primary", Md3Baseline::PRIMARY);
            if selected {
                (
                    primary,
                    TRANSPARENT,
                    0.0,
                    role("on_primary", Md3Baseline::ON_PRIMARY),
                )
            } else if today {
                (TRANSPARENT, primary, DATE_TODAY_OUTLINE_WIDTH, primary)
            } else if outside_month {
                (
                    TRANSPARENT,
                    TRANSPARENT,
                    0.0,
                    role("on_surface_variant", Md3Baseline::ON_SURFACE_VARIANT),
                )
            } else {
                (TRANSPARENT, TRANSPARENT, 0.0, theme.on_surface())
            }
        };

        let corner_radius = self
            .theme
            .borrow()
            .shape("date_picker_day", None)
            .unwrap_or(DATE_CELL_CORNER_RADIUS);
        let mut tree = self.tree.borrow_mut();
        let mut cell_paint = PaintProperties::new(fill, corner_radius, 0.0, 1.0);
        cell_paint.border_color = Animated::new(border_color);
        cell_paint.border_width = Animated::new(border_width);
        let mut cell_style = positioned_style(
            Size {
                width: length(DATE_CELL_SIZE),
                height: length(DATE_CELL_SIZE),
            },
            x,
            y,
        );
        cell_style.display = taffy::Display::Flex;
        cell_style.justify_content = Some(JustifyContent::CENTER);
        cell_style.align_items = Some(AlignItems::CENTER);
        let cell = tree.insert(NodeKind::Rect, cell_style, cell_paint);

        let label_id = tree.insert(
            NodeKind::Text(TextState {
                content: day.to_string(),
                font_family: "Roboto".to_string(),
                font_weight: SEARCH_INPUT_FONT_WEIGHT,
                font_size: SEARCH_INPUT_FONT_SIZE,
                align: TextAlign::Center,
            }),
            Style {
                size: Size {
                    width: length(DATE_CELL_SIZE),
                    height: length(SEARCH_INPUT_FONT_SIZE + 4.0),
                },
                ..Default::default()
            },
            PaintProperties::new(label_color, 0.0, 0.0, 1.0),
        );
        tree.add_child(cell, label_id);

        tree.add_child(self.root, cell);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(date_picker_day_retheme_hook(
                cell,
                label_id,
                selected,
                today,
                outside_month,
            ));
        self.wrap_node(cell)
    }

    /// M30 Phase 7 Step 2 (§5, §7): `Time Picker`'s own real *Time
    /// Input* hour/minute field -- see the `TIME_FIELD_*` constants
    /// above for the full real finding, including the real scope
    /// decision this step made (Time Input, not the analog clock-face
    /// dial, which needs a genuinely new drag-to-angle engine
    /// capability this catalog doesn't have). Real, deliberate reuse
    /// of `TextField`'s own existing real `NodeKind`, the identical
    /// real design `Search Bar` already established -- every one of
    /// its already-real capabilities (typing, focus, selection) work
    /// for free; `add_text_field`'s own construction pattern is
    /// mirrored inline, not cross-called (this file's own unbroken
    /// convention).
    #[pyo3(signature = (value, x=None, y=None))]
    fn add_time_input_field(&self, value: &str, x: Option<f32>, y: Option<f32>) -> Node {
        let text_color = self.theme.borrow().on_surface();
        let mut text_field_state = TextFieldState::new(
            value,
            "Roboto".to_string(),
            TIME_DISPLAY_FONT_WEIGHT,
            TIME_DISPLAY_FONT_SIZE,
        );
        text_field_state.text_tint = text_color;

        let mut tree = self.tree.borrow_mut();
        let container_color = {
            let theme = self.theme.borrow();
            if theme.is_set() {
                theme
                    .role("surface_container_highest")
                    .unwrap_or(Md3Baseline::SURFACE_CONTAINER_HIGHEST)
            } else {
                Md3Baseline::SURFACE_CONTAINER_HIGHEST
            }
        };
        let mut field_style = positioned_style(
            Size {
                width: length(TIME_FIELD_WIDTH),
                height: length(TIME_FIELD_HEIGHT),
            },
            x,
            y,
        );
        field_style.display = taffy::Display::Flex;
        field_style.justify_content = Some(JustifyContent::CENTER);
        field_style.align_items = Some(AlignItems::CENTER);
        // M50 Phase 4: its own key, not "chip" -- reuses `CHIP_CORNER_
        // RADIUS` as a real constant *value* today, the same
        // "coincidental shared number, not a reason to couple theme
        // overrides" reasoning `add_search_view`'s own doc comment
        // already states relative to `"dialog"`.
        let corner_radius = self
            .theme
            .borrow()
            .shape("time_input_field", None)
            .unwrap_or(CHIP_CORNER_RADIUS);
        let id = tree.insert(
            NodeKind::TextField(text_field_state),
            field_style,
            PaintProperties::new(container_color, corner_radius, 0.0, 1.0),
        );
        tree.set_access(
            id,
            AccessNodeData::new(Role::TextInput).with_action(Action::Focus),
        );
        tree.add_child(self.root, id);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(time_input_field_retheme_hook(id));
        self.wrap_node(id)
    }

    /// M30 Phase 7 Step 2 (§5, §7): `Time Picker`'s own real AM/PM
    /// period selector -- see the `TIME_FIELD_*` constants above for
    /// the full real finding, including this step's own honest
    /// caveat about the parts the fetched token set didn't cover.
    /// Real, deliberate architectural choice, the same real dividing
    /// line `Segmented Button`/`Filter Chip` already established:
    /// AM/PM is a real 2-option exclusive toggle, app-owned selection
    /// state (Design Principle 6), not a new engine `NodeKind` --
    /// returns `(am, pm)`, both real, independently `enable_
    /// interaction()`-able `Node`s the app wires up itself.
    #[pyo3(signature = (selected="AM", x=None, y=None))]
    fn add_period_selector(
        &self,
        selected: &str,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<(Node, Node)> {
        if selected != "AM" && selected != "PM" {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "add_period_selector's own selected must be \"AM\" or \"PM\", got {selected:?}"
            )));
        }

        let (selected_fill, selected_label, unselected_label) = {
            let theme = self.theme.borrow();
            let tertiary_container = if theme.is_set() {
                theme
                    .role("tertiary_container")
                    .unwrap_or(Md3Baseline::TERTIARY_CONTAINER)
            } else {
                Md3Baseline::TERTIARY_CONTAINER
            };
            let on_tertiary_container = if theme.is_set() {
                theme
                    .role("on_tertiary_container")
                    .unwrap_or(Md3Baseline::ON_TERTIARY_CONTAINER)
            } else {
                Md3Baseline::ON_TERTIARY_CONTAINER
            };
            (
                tertiary_container,
                on_tertiary_container,
                theme.on_surface(),
            )
        };

        // M50 Phase 4: its own key, not "chip" -- same real reasoning
        // `add_time_input_field`'s own doc comment above states.
        let corner_radius = self
            .theme
            .borrow()
            .shape("period_selector", None)
            .unwrap_or(CHIP_CORNER_RADIUS);
        let mut tree = self.tree.borrow_mut();
        let base_x = x.unwrap_or(0.0);
        let base_y = y.unwrap_or(0.0);

        let mut build_option = |label: &str, is_selected: bool, offset_y: f32| {
            let (fill, label_color) = if is_selected {
                (selected_fill, selected_label)
            } else {
                (TRANSPARENT, unselected_label)
            };
            let mut option_style = positioned_style(
                Size {
                    width: length(PERIOD_SELECTOR_WIDTH),
                    height: length(PERIOD_OPTION_HEIGHT),
                },
                Some(base_x),
                Some(base_y + offset_y),
            );
            option_style.display = taffy::Display::Flex;
            option_style.justify_content = Some(JustifyContent::CENTER);
            option_style.align_items = Some(AlignItems::CENTER);
            let option = tree.insert(
                NodeKind::Rect,
                option_style,
                PaintProperties::new(fill, corner_radius, 0.0, 1.0),
            );
            let label_id = tree.insert(
                NodeKind::Text(TextState {
                    content: label.to_string(),
                    font_family: "Roboto".to_string(),
                    font_weight: BUTTON_LABEL_FONT_WEIGHT,
                    font_size: BUTTON_LABEL_FONT_SIZE,
                    align: TextAlign::Center,
                }),
                Style {
                    size: Size {
                        width: length(PERIOD_SELECTOR_WIDTH),
                        height: length(BUTTON_LABEL_LINE_HEIGHT),
                    },
                    ..Default::default()
                },
                PaintProperties::new(label_color, 0.0, 0.0, 1.0),
            );
            tree.add_child(option, label_id);
            tree.add_child(self.root, option);
            (option, label_id)
        };

        let (am, am_label) = build_option("AM", selected == "AM", 0.0);
        let (pm, pm_label) = build_option("PM", selected == "PM", PERIOD_OPTION_HEIGHT);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(period_selector_retheme_hook(
                am,
                am_label,
                selected == "AM",
                pm,
                pm_label,
                selected == "PM",
            ));

        Ok((self.wrap_node(am), self.wrap_node(pm)))
    }

    /// M30 Phase 8 Step 1 (§11.3): `Popover`, grounded in MD3's own
    /// real Rich Tooltip anatomy -- see the `POPOVER_*` constants
    /// above for the full real finding, including the real reuse of
    /// `Window.open_menu`/`close_menu` for its own genuinely
    /// *persistent* show/hide lifecycle, not a new dedicated method
    /// pair. Real anatomy: `surface_container` fill, real `corner-
    /// medium` shape, a real rest-state elevation (level 2); a
    /// subhead (Title Small, `on_surface_variant`) and supporting
    /// text (Body Medium, `on_surface_variant`) stacked in a padded
    /// column, the identical real layout shape `Dialog`'s own panel
    /// already established (headline + body). Returned genuinely
    /// unattached anywhere -- the same real contract `add_dialog`/
    /// `add_tooltip`'s own panels already have; pass it to `Window.
    /// open_menu(anchor, popover)` to actually show it.
    #[pyo3(signature = (subhead, text, width, height))]
    fn add_popover(&self, subhead: &str, text: &str, width: f32, height: f32) -> Node {
        let (subhead_color, body_color) = {
            let theme = self.theme.borrow();
            let on_surface_variant = if theme.is_set() {
                theme
                    .role("on_surface_variant")
                    .unwrap_or(Md3Baseline::ON_SURFACE_VARIANT)
            } else {
                Md3Baseline::ON_SURFACE_VARIANT
            };
            (on_surface_variant, on_surface_variant)
        };
        let container_color = {
            let theme = self.theme.borrow();
            if theme.is_set() {
                theme
                    .role("surface_container")
                    .unwrap_or(Md3Baseline::SURFACE_CONTAINER)
            } else {
                Md3Baseline::SURFACE_CONTAINER
            }
        };
        let (corner_radius, elevation) = {
            let theme = self.theme.borrow();
            (
                theme
                    .shape("popover", None)
                    .unwrap_or(POPOVER_CORNER_RADIUS),
                theme
                    .elevation("popover", None)
                    .unwrap_or(POPOVER_ELEVATION),
            )
        };

        let mut tree = self.tree.borrow_mut();

        let panel_style = Style {
            size: Size {
                width: length(width),
                height: length(height),
            },
            display: taffy::Display::Flex,
            flex_direction: taffy::FlexDirection::Column,
            padding: TaffyRect {
                left: length(POPOVER_PADDING),
                right: length(POPOVER_PADDING),
                top: length(POPOVER_PADDING),
                bottom: length(POPOVER_PADDING),
            },
            gap: Size {
                width: length(0.0),
                height: length(POPOVER_SUBHEAD_GAP),
            },
            ..Default::default()
        };
        let panel = tree.insert(
            NodeKind::Rect,
            panel_style,
            PaintProperties::new(container_color, corner_radius, elevation, 1.0),
        );

        let content_width = (width - 2.0 * POPOVER_PADDING).max(0.0);
        let subhead_id = tree.insert(
            NodeKind::Text(TextState {
                content: subhead.to_string(),
                font_family: "Roboto".to_string(),
                font_weight: POPOVER_SUBHEAD_FONT_WEIGHT,
                font_size: POPOVER_SUBHEAD_FONT_SIZE,
                align: TextAlign::Start,
            }),
            Style {
                size: Size {
                    width: length(content_width),
                    height: length(POPOVER_SUBHEAD_FONT_SIZE + 4.0),
                },
                ..Default::default()
            },
            PaintProperties::new(subhead_color, 0.0, 0.0, 1.0),
        );
        tree.add_child(panel, subhead_id);

        let body_height = (height
            - 2.0 * POPOVER_PADDING
            - POPOVER_SUBHEAD_GAP
            - (POPOVER_SUBHEAD_FONT_SIZE + 4.0))
            .max(0.0);
        let body_id = tree.insert(
            NodeKind::Text(TextState {
                content: text.to_string(),
                font_family: "Roboto".to_string(),
                font_weight: DIALOG_BODY_FONT_WEIGHT,
                font_size: DIALOG_BODY_FONT_SIZE,
                align: TextAlign::Start,
            }),
            Style {
                size: Size {
                    width: length(content_width),
                    height: length(body_height),
                },
                ..Default::default()
            },
            PaintProperties::new(body_color, 0.0, 0.0, 1.0),
        );
        tree.add_child(panel, body_id);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(popover_retheme_hook(panel, subhead_id, body_id));
        self.wrap_node(panel)
    }

    /// M30 Phase 8 Step 2 (§5, §7): `Link`, a real, standalone
    /// clickable label -- see the `LINK_*` constants above for the
    /// full real finding, including the real, new `NodeKind::Link`
    /// engine-core capability this step fulfills. Deliberately does
    /// *not* auto-call `enable_interaction()`, matching every other
    /// composite `add_*` in this catalog.
    #[pyo3(signature = (text, width, x=None, y=None))]
    fn add_link(&self, text: &str, width: f32, x: Option<f32>, y: Option<f32>) -> Node {
        let color = {
            let theme = self.theme.borrow();
            if theme.is_set() {
                theme.role("primary").unwrap_or(Md3Baseline::PRIMARY)
            } else {
                Md3Baseline::PRIMARY
            }
        };

        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Link(TextState {
                content: text.to_string(),
                font_family: "Roboto".to_string(),
                font_weight: LINK_FONT_WEIGHT,
                font_size: LINK_FONT_SIZE,
                align: TextAlign::Start,
            }),
            positioned_style(
                Size {
                    width: length(width),
                    height: length(LINK_FONT_SIZE + 4.0),
                },
                x,
                y,
            ),
            PaintProperties::new(color, 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        drop(tree);
        self.retheme_hooks.borrow_mut().push(link_retheme_hook(id));
        self.wrap_node(id)
    }

    /// M30 Phase 8 Step 3 (§5, §7): `SpinBox`, a real numeric
    /// increment control -- see the `SPIN_BOX_*` constants above for
    /// the full real finding, including the real, deliberate naming
    /// choice (not "Stepper", pyCopper's own real prior finding).
    /// Real, deliberate reuse of `TextField`'s own existing real
    /// `NodeKind` for the numeric display, the identical real design
    /// `Search Bar`/`Time Input` already established -- every one of
    /// its already-real capabilities (typing, focus, selection) work
    /// for free. Increment/decrement reuse `Icon Button`'s own exact
    /// real anatomy (Phase 1 Step 2, a `Rect` container with a
    /// centered, correctly-deferring `Icon` child) a second/third
    /// time this catalog already has (`Top App Bar`/`Search Bar`).
    /// Returns `(field, decrement, increment)` -- `field` is a real
    /// `NodeKind::TextField`, `decrement`/`increment` each a real,
    /// independently `enable_interaction()`-able `Node`, the app
    /// wiring real `+`/`-1` logic itself (Design Principle 6 -- the
    /// engine has no notion of the value's own real numeric semantics
    /// or bounds).
    #[pyo3(signature = (value, x=None, y=None))]
    fn add_spin_box(
        &self,
        value: &str,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<(Node, Node, Node)> {
        let minus_path = resolve_icon_path("remove")?;
        let plus_path = resolve_icon_path("add")?;

        let (field_color, text_color, icon_color) = {
            let theme = self.theme.borrow();
            let role = |name: &str, fallback: Color| -> Color {
                if theme.is_set() {
                    theme.role(name).unwrap_or(fallback)
                } else {
                    fallback
                }
            };
            (
                role(
                    "surface_container_highest",
                    Md3Baseline::SURFACE_CONTAINER_HIGHEST,
                ),
                theme.on_surface(),
                role("on_surface_variant", Md3Baseline::ON_SURFACE_VARIANT),
            )
        };

        // M50 Phase 4: the +/- buttons are genuinely icon-button-shaped
        // (`SPIN_BOX_BUTTON_SIZE` itself reuses `SEARCH_ICON_BUTTON_
        // SIZE` as a real constant value already), so they share the
        // `"icon_button"` key -- the same reuse `add_top_app_bar`'s own
        // leading/trailing icon buttons already establish. The field
        // gets its own `"spin_box"` key, not `"chip"`.
        let icon_button_corner_radius = self
            .theme
            .borrow()
            .shape("icon_button", None)
            .unwrap_or(SPIN_BOX_BUTTON_SIZE as f64 / 2.0);
        let field_corner_radius = self
            .theme
            .borrow()
            .shape("spin_box", None)
            .unwrap_or(CHIP_CORNER_RADIUS);

        let mut tree = self.tree.borrow_mut();
        let base_x = x.unwrap_or(0.0);
        let base_y = y.unwrap_or(0.0);
        let field_x = base_x + SPIN_BOX_BUTTON_SIZE + SPIN_BOX_GAP;

        let mut build_icon_button = |path: peniko::kurbo::BezPath, offset_x: f32| {
            let mut button_style = positioned_style(
                Size {
                    width: length(SPIN_BOX_BUTTON_SIZE),
                    height: length(SPIN_BOX_BUTTON_SIZE),
                },
                Some(base_x + offset_x),
                Some(base_y),
            );
            button_style.display = taffy::Display::Flex;
            button_style.justify_content = Some(JustifyContent::CENTER);
            button_style.align_items = Some(AlignItems::CENTER);
            let button = tree.insert(
                NodeKind::Rect,
                button_style,
                PaintProperties::new(TRANSPARENT, icon_button_corner_radius, 0.0, 1.0),
            );
            let icon_id = tree.insert(
                NodeKind::Icon(IconState::new(path, icon_color)),
                Style {
                    size: Size {
                        width: length(MENU_ITEM_ICON_SIZE),
                        height: length(MENU_ITEM_ICON_SIZE),
                    },
                    ..Default::default()
                },
                PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
            );
            tree.add_child(button, icon_id);
            tree.add_child(self.root, button);
            (button, icon_id)
        };

        let (decrement, decrement_icon) = build_icon_button(minus_path, 0.0);
        let (increment, increment_icon) = build_icon_button(
            plus_path,
            SPIN_BOX_BUTTON_SIZE + SPIN_BOX_GAP + SPIN_BOX_FIELD_WIDTH + SPIN_BOX_GAP,
        );

        let mut field_style = positioned_style(
            Size {
                width: length(SPIN_BOX_FIELD_WIDTH),
                height: length(SPIN_BOX_FIELD_HEIGHT),
            },
            Some(field_x),
            Some(base_y),
        );
        field_style.display = taffy::Display::Flex;
        field_style.justify_content = Some(JustifyContent::CENTER);
        field_style.align_items = Some(AlignItems::CENTER);
        let mut text_field_state = TextFieldState::new(
            value,
            "Roboto".to_string(),
            SEARCH_INPUT_FONT_WEIGHT,
            BUTTON_LABEL_FONT_SIZE,
        );
        text_field_state.text_tint = text_color;
        let field = tree.insert(
            NodeKind::TextField(text_field_state),
            field_style,
            PaintProperties::new(field_color, field_corner_radius, 0.0, 1.0),
        );
        tree.set_access(
            field,
            AccessNodeData::new(Role::TextInput).with_action(Action::Focus),
        );
        tree.add_child(self.root, field);
        drop(tree);
        self.retheme_hooks.borrow_mut().push(spin_box_retheme_hook(
            field,
            decrement,
            decrement_icon,
            increment,
            increment_icon,
        ));

        Ok((
            self.wrap_node(field),
            self.wrap_node(decrement),
            self.wrap_node(increment),
        ))
    }

    /// M30 Phase 8 Step 4 (§5, §7): `Pagination` -- see the `PAGE_
    /// ITEM_*` constants above for the full real finding. Real,
    /// deliberate architectural choice, the same real dividing line
    /// `Segmented Button`/`Filter Chip`/`Navigation Rail`/`Tabs`
    /// already established: "which page is current" is app-owned
    /// state (Design Principle 6), not a new engine `NodeKind` --
    /// returns `(previous, pages, next)`, `pages` a `Vec<Node>`
    /// (`Segmented Button`'s own exact real return shape) one per
    /// real page indicator, `previous`/`next` each `Icon Button`'s
    /// own exact real anatomy reused a fourth time this catalog
    /// already has.
    #[pyo3(signature = (page_count, current=0, x=None, y=None))]
    fn add_pagination(
        &self,
        page_count: usize,
        current: usize,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<(Node, Vec<Node>, Node)> {
        if page_count == 0 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "add_pagination needs at least 1 page",
            ));
        }
        if current >= page_count {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "current page {current} is out of range for {page_count} pages"
            )));
        }
        let back_path = resolve_icon_path("arrow_back")?;
        let forward_path = resolve_icon_path("arrow_forward")?;

        let (selected_fill, selected_label, unselected_label, icon_color) = {
            let theme = self.theme.borrow();
            let role = |name: &str, fallback: Color| -> Color {
                if theme.is_set() {
                    theme.role(name).unwrap_or(fallback)
                } else {
                    fallback
                }
            };
            let on_surface_variant = role("on_surface_variant", Md3Baseline::ON_SURFACE_VARIANT);
            (
                role("primary", Md3Baseline::PRIMARY),
                role("on_primary", Md3Baseline::ON_PRIMARY),
                on_surface_variant,
                on_surface_variant,
            )
        };
        let corner_radius = self
            .theme
            .borrow()
            .shape("pagination", None)
            .unwrap_or(PAGE_ITEM_CORNER_RADIUS);

        // A plain, non-capturing `fn` rather than a closure -- shared
        // across this method's own three real call sites (`previous`,
        // each page item's own label-less sibling would need it too,
        // `next`) without the closure-borrow conflict a `tree`-
        // capturing closure would hit once real code runs *between*
        // calls (`pages`'s own loop, in between `previous` and
        // `next`).
        #[allow(clippy::too_many_arguments)]
        fn build_icon_button(
            tree: &mut Tree,
            root: NodeId,
            path: peniko::kurbo::BezPath,
            icon_color: Color,
            base_x: f32,
            base_y: f32,
            offset_x: f32,
            corner_radius: f64,
        ) -> (NodeId, NodeId) {
            let mut style = positioned_style(
                Size {
                    width: length(PAGE_ITEM_SIZE),
                    height: length(PAGE_ITEM_SIZE),
                },
                Some(base_x + offset_x),
                Some(base_y),
            );
            style.display = taffy::Display::Flex;
            style.justify_content = Some(JustifyContent::CENTER);
            style.align_items = Some(AlignItems::CENTER);
            let button = tree.insert(
                NodeKind::Rect,
                style,
                PaintProperties::new(TRANSPARENT, corner_radius, 0.0, 1.0),
            );
            let icon_id = tree.insert(
                NodeKind::Icon(IconState::new(path, icon_color)),
                Style {
                    size: Size {
                        width: length(MENU_ITEM_ICON_SIZE),
                        height: length(MENU_ITEM_ICON_SIZE),
                    },
                    ..Default::default()
                },
                PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
            );
            tree.add_child(button, icon_id);
            tree.add_child(root, button);
            (button, icon_id)
        }

        let mut tree = self.tree.borrow_mut();
        let base_x = x.unwrap_or(0.0);
        let base_y = y.unwrap_or(0.0);

        let (previous, previous_icon) = build_icon_button(
            &mut tree,
            self.root,
            back_path,
            icon_color,
            base_x,
            base_y,
            0.0,
            corner_radius,
        );

        let mut pages = Vec::with_capacity(page_count);
        let mut page_retheme_data: Vec<(NodeId, NodeId, bool)> = Vec::with_capacity(page_count);
        for i in 0..page_count {
            let is_selected = i == current;
            let offset_x =
                PAGE_ITEM_SIZE + PAGE_ITEM_GAP + i as f32 * (PAGE_ITEM_SIZE + PAGE_ITEM_GAP);
            let (fill, label_color) = if is_selected {
                (selected_fill, selected_label)
            } else {
                (TRANSPARENT, unselected_label)
            };
            let mut item_style = positioned_style(
                Size {
                    width: length(PAGE_ITEM_SIZE),
                    height: length(PAGE_ITEM_SIZE),
                },
                Some(base_x + offset_x),
                Some(base_y),
            );
            item_style.display = taffy::Display::Flex;
            item_style.justify_content = Some(JustifyContent::CENTER);
            item_style.align_items = Some(AlignItems::CENTER);
            let item = tree.insert(
                NodeKind::Rect,
                item_style,
                PaintProperties::new(fill, corner_radius, 0.0, 1.0),
            );
            let label_id = tree.insert(
                NodeKind::Text(TextState {
                    content: (i + 1).to_string(),
                    font_family: "Roboto".to_string(),
                    font_weight: BUTTON_LABEL_FONT_WEIGHT,
                    font_size: BUTTON_LABEL_FONT_SIZE,
                    align: TextAlign::Center,
                }),
                Style {
                    size: Size {
                        width: length(PAGE_ITEM_SIZE),
                        height: length(BUTTON_LABEL_LINE_HEIGHT),
                    },
                    ..Default::default()
                },
                PaintProperties::new(label_color, 0.0, 0.0, 1.0),
            );
            tree.add_child(item, label_id);
            tree.add_child(self.root, item);
            page_retheme_data.push((item, label_id, is_selected));
            pages.push(self.wrap_node(item));
        }

        let next_offset =
            PAGE_ITEM_SIZE + PAGE_ITEM_GAP + page_count as f32 * (PAGE_ITEM_SIZE + PAGE_ITEM_GAP);
        let (next, next_icon) = build_icon_button(
            &mut tree,
            self.root,
            forward_path,
            icon_color,
            base_x,
            base_y,
            next_offset,
            corner_radius,
        );

        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(pagination_retheme_hook(
                previous,
                previous_icon,
                next,
                next_icon,
                page_retheme_data,
            ));
        Ok((self.wrap_node(previous), pages, self.wrap_node(next)))
    }

    /// M30 Phase 8 Step 5 (§5, §7, §11.2): `Status Bar`'s own real,
    /// styled content -- see the `STATUS_BAR_*` constants above for
    /// the full real finding, including the real reuse of `AppShell`
    /// (`build_shell`)'s own already-real `status_bar` region -- pass
    /// the returned `Node` directly to `build_shell`'s own existing
    /// `status_bar` parameter, no new shell-level wiring needed.
    #[pyo3(signature = (text, width=None))]
    fn add_status_bar(&self, text: &str, width: Option<f32>) -> Node {
        let (container_color, text_color) = {
            let theme = self.theme.borrow();
            let container = if theme.is_set() {
                theme
                    .role("surface_container")
                    .unwrap_or(Md3Baseline::SURFACE_CONTAINER)
            } else {
                Md3Baseline::SURFACE_CONTAINER
            };
            let on_surface_variant = if theme.is_set() {
                theme
                    .role("on_surface_variant")
                    .unwrap_or(Md3Baseline::ON_SURFACE_VARIANT)
            } else {
                Md3Baseline::ON_SURFACE_VARIANT
            };
            (container, on_surface_variant)
        };

        let mut tree = self.tree.borrow_mut();
        let bar_width = width.unwrap_or(self.width.get() as f32);

        let mut bar_style = Style {
            size: Size {
                width: length(bar_width),
                height: length(STATUS_BAR_HEIGHT),
            },
            display: taffy::Display::Flex,
            align_items: Some(AlignItems::CENTER),
            padding: TaffyRect {
                left: length(STATUS_BAR_PADDING),
                right: length(STATUS_BAR_PADDING),
                top: zero(),
                bottom: zero(),
            },
            ..Default::default()
        };
        bar_style.flex_shrink = 0.0;
        let bar = tree.insert(
            NodeKind::Rect,
            bar_style,
            PaintProperties::new(container_color, 0.0, 0.0, 1.0),
        );

        let label_width = (bar_width - 2.0 * STATUS_BAR_PADDING).max(0.0);
        let label_id = tree.insert(
            NodeKind::Text(TextState {
                content: text.to_string(),
                font_family: "Roboto".to_string(),
                font_weight: BADGE_LABEL_FONT_WEIGHT,
                font_size: BADGE_LABEL_FONT_SIZE,
                align: TextAlign::Start,
            }),
            Style {
                size: Size {
                    width: length(label_width),
                    height: length(BADGE_LABEL_FONT_SIZE + 2.0),
                },
                ..Default::default()
            },
            PaintProperties::new(text_color, 0.0, 0.0, 1.0),
        );
        tree.add_child(bar, label_id);

        tree.add_child(self.root, bar);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(status_bar_retheme_hook(bar, label_id));
        self.wrap_node(bar)
    }

    /// M14 Phase 1 (§5, §7.3): creates a real `NodeKind::Checkbox`,
    /// mirroring `add_rect`'s own real shape exactly -- `background`
    /// same as any other node), `checked` seeds `CheckboxState`'s own
    /// initial state (and its `check_progress` starting already at the
    /// matching `1.0`/`0.0`, `CheckboxState::new`'s own real contract).
    /// The already-generic `set_on_click`/`enable_interaction()` work
    /// on this exactly like any other node -- no new interaction wiring
    /// needed here.
    #[pyo3(signature = (background, width, height, checked=false, x=None, y=None))]
    fn add_checkbox(
        &self,
        background: (u8, u8, u8, u8),
        width: f32,
        height: f32,
        checked: bool,
        x: Option<f32>,
        y: Option<f32>,
    ) -> Node {
        let (r, g, b, a) = background;
        // M20 Phase 1 (§7.1, §7.3): a Checkbox created *after* `Window.
        // set_theme` must start genuinely themed, not stuck with the
        // plain white default until another `set_theme` call happens
        // to re-push it -- the same real intent `Node.enable_
        // interaction`'s own construction-time read already has. Real,
        // necessary difference from that precedent: `ThemeState::
        // on_surface()`'s own no-theme-set default is black, but
        // `CheckboxState`'s own real default is white -- reading it
        // unconditionally would silently replace an un-themed
        // checkbox's real white mark with black. Gated on `theme.
        // is_set()` so the real historical default survives untouched
        // until an app genuinely calls `set_theme`.
        let mut checkbox_state = CheckboxState::new(checked);
        {
            let theme = self.theme.borrow();
            if theme.is_set() {
                checkbox_state.mark_tint = theme.on_surface();
            }
        }
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Checkbox(checkbox_state),
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
        self.wrap_node(id)
    }

    /// M30 Phase 2 Step 1 (§5, §7.3): creates a real `NodeKind::
    /// RadioButton`, mirroring `add_checkbox`'s own real shape --
    /// `selected` seeds `RadioButtonState`'s own initial state (and
    /// its `select_progress` starting already at the matching
    /// `1.0`/`0.0`, `RadioButtonState::new`'s own real contract). One
    /// real, necessary difference from `add_checkbox`: there is no
    /// caller-supplied `background` -- a radio button's whole real
    /// visual comes from `unselected_tint`/`selected_tint`, always
    /// resolved here (through `theme.role`, gated on `theme.is_set()`
    /// exactly like every other themed component, else `Md3Baseline`'s
    /// own real fallback) rather than left as caller-supplied paint,
    /// since (unlike `Checkbox`'s box) there is no independent "fill
    /// color" concept in real MD3 radio-button anatomy at all. `size`
    /// is a single square dimension (MD3's own real circle is 20dp by
    /// default) -- `Checkbox`'s own separate `width`/`height` params
    /// would only ever be called equal in practice for a real radio
    /// button, so one real parameter is the honest shape, not two that
    /// invite an inconsistent oval. **Real, explicit scope limit, not
    /// an oversight:** unlike `Checkbox`/`Slider`/`TextField`, a radio
    /// button created *before* `Window.set_theme` is **not**
    /// retroactively re-tinted by a later `set_theme` call --
    /// `Tree::set_all_component_tints` deliberately reuses one shared
    /// `on_surface` tint across every component it touches (its own
    /// stated scope choice, `window.rs`'s `set_theme` doc comment), but
    /// a radio button genuinely needs two *different* real roles
    /// (`outline`/`primary`), which that single-`Color`-parameter
    /// mechanism can't express without contradicting its own already-
    /// documented simplification. Every radio button still starts
    /// correctly themed at construction time, the same real contract
    /// `add_button`/`add_fab`/etc already have.
    #[pyo3(signature = (size=20.0, selected=false, x=None, y=None))]
    fn add_radio_button(&self, size: f32, selected: bool, x: Option<f32>, y: Option<f32>) -> Node {
        let mut radio_state = RadioButtonState::new(selected);
        {
            let theme = self.theme.borrow();
            let role = |name: &str, fallback: Color| -> Color {
                if theme.is_set() {
                    theme.role(name).unwrap_or(fallback)
                } else {
                    fallback
                }
            };
            radio_state.unselected_tint = role("outline", Md3Baseline::OUTLINE);
            radio_state.selected_tint = role("primary", Md3Baseline::PRIMARY);
        }
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::RadioButton(radio_state),
            positioned_style(
                Size {
                    width: length(size),
                    height: length(size),
                },
                x,
                y,
            ),
            PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(radio_button_retheme_hook(id));
        self.wrap_node(id)
    }

    /// M30 Phase 2 Step 2 (§5, §7.3): creates a real `NodeKind::
    /// Switch`, mirroring `add_radio_button`'s own real shape --
    /// every real color/shape token always resolved here (through
    /// `theme.role`, gated on `theme.is_set()`, else `Md3Baseline`'s
    /// own real fallback), never caller-supplied, the identical real
    /// reason `add_radio_button` has no `background` parameter either.
    /// `width`/`height` default to MD3's own real track dimensions
    /// (52dp x 32dp, verified against Material Web's own token
    /// source) -- unlike `add_radio_button`'s single `size`, a switch
    /// track is genuinely non-square in real MD3, so two real
    /// parameters is the honest shape here.
    #[pyo3(signature = (width=52.0, height=32.0, on=false, x=None, y=None))]
    fn add_switch(
        &self,
        width: f32,
        height: f32,
        on: bool,
        x: Option<f32>,
        y: Option<f32>,
    ) -> Node {
        let mut switch_state = SwitchState::new(on);
        {
            let theme = self.theme.borrow();
            let role = |name: &str, fallback: Color| -> Color {
                if theme.is_set() {
                    theme.role(name).unwrap_or(fallback)
                } else {
                    fallback
                }
            };
            switch_state.track_off_tint = role(
                "surface_container_highest",
                Md3Baseline::SURFACE_CONTAINER_HIGHEST,
            );
            switch_state.track_on_tint = role("primary", Md3Baseline::PRIMARY);
            switch_state.track_outline_tint = role("outline", Md3Baseline::OUTLINE);
            switch_state.handle_off_tint = role("outline", Md3Baseline::OUTLINE);
            switch_state.handle_on_tint = role("on_primary", Md3Baseline::ON_PRIMARY);
        }
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Switch(switch_state),
            positioned_style(
                Size {
                    width: length(width),
                    height: length(height),
                },
                x,
                y,
            ),
            PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(switch_retheme_hook(id));
        self.wrap_node(id)
    }

    /// M14 Phase 2 (§5, §7.3): creates a real `NodeKind::Slider`,
    /// mirroring `add_checkbox`'s own real shape exactly -- `background`
    /// is the thumb's own real fill color (universal `PaintProperties`,
    /// same as any other node); `value` seeds `SliderState`'s own
    /// initial `thumb_position` (clamped `0.0..=1.0`, `SliderState::
    /// new`'s own real contract). The real drag-to-set interaction is
    /// entirely internal to `Tree::dispatch` (M14 Phase 2's own real
    /// finding, mirroring how `Splitter` dragging already works) -- no
    /// Python-facing wiring needed for that half at all.
    #[pyo3(signature = (background, width, height, value=0.0, x=None, y=None))]
    fn add_slider(
        &self,
        background: (u8, u8, u8, u8),
        width: f32,
        height: f32,
        value: f64,
        x: Option<f32>,
        y: Option<f32>,
    ) -> Node {
        let (r, g, b, a) = background;
        // M20 Phase 1 (§7.1, §7.3): same real "themed-at-construction,
        // gated on a real theme actually being set" reasoning as
        // `add_checkbox`, above -- `SliderState`'s own real default
        // track color is gray, not `on_surface()`'s own black default.
        let mut slider_state = SliderState::new(value);
        {
            let theme = self.theme.borrow();
            if theme.is_set() {
                slider_state.track_tint = theme.on_surface();
            }
        }
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Slider(slider_state),
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
        // M24 Phase 1 (§10): a real, necessary connected fix, found
        // only by actually trying the new arrow-key increment end to
        // end from Python, not assumed -- `Tree::dispatch`'s own new
        // `dispatch_slider_key` requires a real focused slider to ever
        // reach it at all, but before this a `Slider` had no `access.
        // actions` set anywhere, so `collect_interactive`'s own real
        // Tab-order predicate never included one (`enable_interaction`
        // only ever touched ripple/hover tint, not focusability). A
        // real `Slider` now opts into keyboard focus at construction
        // the identical way `add_text_field` already does -- §10's own
        // "keyboard operability ships from day one" text, applied to
        // the one real component this milestone's own scope covers.
        tree.set_access(
            id,
            AccessNodeData::new(Role::Slider).with_action(Action::Focus),
        );
        tree.add_child(self.root, id);
        self.wrap_node(id)
    }

    /// M22 Phase 1 (§5): creates a real `NodeKind::Image`, loaded from
    /// a real file on disk. Unlike `add_rect`/`add_checkbox`/
    /// `add_slider`, deliberately does *not* take a `background` param
    /// -- mirrors `add_canvas`'s own real precedent instead (a
    /// hardcoded transparent `PaintProperties` fill), since there's no
    /// meaningful "behind the content" color this phase scopes for a
    /// node whose entire content is a loaded image, the same "fully
    /// custom-drawn kind doesn't expose a separate background" reasoning
    /// `add_canvas` already established.
    ///
    /// Decoding is the real crate-boundary work this method does that
    /// `engine-core` deliberately never does itself (`ImageState`'s own
    /// doc comment) -- `image::open` reads and decodes the file
    /// (whatever real format its own magic-byte sniffing detects among
    /// this crate's enabled `png`/`jpeg` features), `.to_rgba8()` gives
    /// real straight-alpha (unpremultiplied) 8-bit RGBA pixels, and
    /// those raw bytes become a `peniko::ImageData` via `peniko::Blob`'s
    /// own real `From<Vec<u8>>` impl -- zero copying beyond what
    /// `to_rgba8()` itself already allocates.
    ///
    /// M22 Phase 2 (§16.1): `fit` (`"cover"`/`"contain"`/`"fill"`,
    /// default `"fill"` -- byte-for-byte Phase 1's own only behavior)
    /// sets `ImageState.content_fit`, the identical field `kind: Image`
    /// in a real `view.yaml`'s own `image.fit:` sets -- kept symmetric
    /// with the declarative path rather than leaving this imperative
    /// entry point stuck at `Fill` forever.
    #[pyo3(signature = (path, width, height, fit="fill", x=None, y=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_image(
        &self,
        path: &str,
        width: f32,
        height: f32,
        fit: &str,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        let content_fit = parse_content_fit(fit)?;
        let decoded = image::open(path)
            .map_err(|e| EngineError::ImageLoadFailed {
                path: path.to_string(),
                reason: e.to_string(),
            })?
            .to_rgba8();
        let (img_width, img_height) = decoded.dimensions();
        let image_data = peniko::ImageData {
            data: peniko::Blob::from(decoded.into_raw()),
            format: peniko::ImageFormat::Rgba8,
            alpha_type: peniko::ImageAlphaType::Alpha,
            width: img_width,
            height: img_height,
        };
        let mut image_state = ImageState::new(image_data);
        image_state.content_fit = content_fit;

        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Image(image_state),
            positioned_style(
                Size {
                    width: length(width),
                    height: length(height),
                },
                x,
                y,
            ),
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        Ok(self.wrap_node(id))
    }

    /// M30 Phase 9 Step 1 (§5): creates a real `NodeKind::Image` node
    /// meant to be updated live via `Node.push_frame` -- see that
    /// method's own doc comment for the full real design (a "frame
    /// sink," not a decoder, directly grounded in the sibling
    /// `pyCopper` project's own real `Video` widget). No official MD3
    /// page exists for Video (confirmed via the same real directory-
    /// listing technique this milestone already uses throughout), and
    /// unlike `add_image` there is no file to load or decode here at
    /// all -- `width`/`height` are the node's own real, fixed box
    /// (exactly `add_image`'s own contract), initialized with a single
    /// fully-transparent placeholder pixel so the node paints as
    /// genuinely empty until the app's own first real `push_frame`
    /// call, the same real "nothing to show yet" contract `add_image`
    /// would have for pixel data if it allowed loading nothing. `fit`
    /// (`"cover"`/`"contain"`/`"fill"`, default `"fill"`) is `add_image`'s
    /// own identical real `ContentFit` parameter, reused verbatim --
    /// a pushed frame's own resolution is resolved against this node's
    /// fixed box the exact same way a loaded image's is.
    #[pyo3(signature = (width, height, fit="fill", x=None, y=None))]
    fn add_video(
        &self,
        width: f32,
        height: f32,
        fit: &str,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        let content_fit = parse_content_fit(fit)?;
        let placeholder = peniko::ImageData {
            data: peniko::Blob::from(vec![0u8, 0, 0, 0]),
            format: peniko::ImageFormat::Rgba8,
            alpha_type: peniko::ImageAlphaType::Alpha,
            width: 1,
            height: 1,
        };
        let mut image_state = ImageState::new(placeholder);
        image_state.content_fit = content_fit;

        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Image(image_state),
            positioned_style(
                Size {
                    width: length(width),
                    height: length(height),
                },
                x,
                y,
            ),
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        Ok(self.wrap_node(id))
    }

    /// M30 Phase 9 Step 2 (§5): the real pannable/zoomable viewport
    /// half of `Node Graph`, closing the real gap M5 Phase 4's own
    /// `PLAN.md` named and `examples/positioned_graph.py` (M6 Phase 3)
    /// only ever demonstrated as raw composition -- see `add_graph_
    /// node`'s own doc comment for the full real design. No official
    /// MD3 page exists (confirmed via the same directory-listing
    /// technique this milestone already uses). `surface_container_low`
    /// -- the identical real "recessed backdrop" role this project's
    /// own `Md3Baseline` already declares -- is the one real styling
    /// choice this method makes; everything else is exactly `add_rect`
    /// under a name that documents its own real, intended use as
    /// `add_graph_node`'s own `graph` parameter.
    #[pyo3(signature = (width, height, x=None, y=None))]
    fn add_node_graph(&self, width: f32, height: f32, x: Option<f32>, y: Option<f32>) -> Node {
        let container_color = {
            let theme = self.theme.borrow();
            if theme.is_set() {
                theme
                    .role("surface_container_low")
                    .unwrap_or(Md3Baseline::SURFACE_CONTAINER_LOW)
            } else {
                Md3Baseline::SURFACE_CONTAINER_LOW
            }
        };
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
            PaintProperties::new(container_color, 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(node_graph_retheme_hook(id));
        self.wrap_node(id)
    }

    /// M30 Phase 9 Step 2 (§5): `Node Graph`'s own real "first-class
    /// authored component," closing the real gap M5 Phase 4's own
    /// `PLAN.md` named and `examples/positioned_graph.py` (M6 Phase 3)
    /// only ever demonstrated as raw composition -- interchangeable
    /// circles, no real node anatomy, no reparent/position automation
    /// an app didn't have to hand-roll itself. No official MD3 page
    /// exists (confirmed via the same directory-listing technique this
    /// milestone already uses); designed from real, directly-
    /// applicable precedent in the sibling `pyCopper` project's own
    /// `NodeGraph` widget (a title-bar-styled, ported node composed
    /// from the framework's own existing primitives, not a bespoke
    /// region system).
    ///
    /// **Real anatomy, not independently MD3-token-verified (no spec
    /// exists to verify against):** a `surface_container` body
    /// (`Menu`'s own panel role) topped with a `surface_container_high`
    /// title strip (`Popover`/`Search View`'s own "slightly elevated
    /// header on a recessed body" convention -- the identical real
    /// tiered-surface choice pyCopper's own `NodeElement` independently
    /// made too), `CARD_CORNER_RADIUS` reused verbatim. Title label
    /// reuses Title Small's own already-declared constants
    /// (`TAB_LABEL_FONT_SIZE`/`_WEIGHT` -- the same real numeric
    /// coincidence `Popover`'s own subhead already reused a second
    /// time, this is the third), `on_surface_variant` -- this project
    /// has no independently-verified plain `on_surface` role yet, so
    /// this reuses the same already-verified role every other label in
    /// this file does, a real, stated honest choice rather than
    /// inventing an unverified new color.
    ///
    /// **Real, deliberate reuse of already-existing primitives, needing
    /// zero new engine-core/engine-render capability:** `graph` is
    /// typically an `add_node_graph`-created viewport -- this method
    /// attaches the composed node DIRECTLY under `graph`, not under
    /// `self.root` the way every other `add_*` method here does, since
    /// a graph node's real intended parent is the graph viewport, not
    /// the window root. Confirmed via a real, empirical hit-testing
    /// check before relying on it (not assumed from reading `taffy`'s
    /// own docs alone): `Position::Absolute`'s `x`/`y` inset re-
    /// resolves relative to whatever node is its real parent at layout
    /// time, so a node attached directly under `graph` is positioned
    /// `x`/`y` relative to `graph`'s own origin, exactly the local
    /// coordinate space an app places nodes in. Panning/zooming the
    /// whole graph is `graph.animate("transform", (dx, dy, scale),
    /// ...)` -- already fully real since M6 Phase 2, and already
    /// proven at the engine-core level to compose correctly through
    /// nested ancestors (`absolute_position_follows_an_ancestor_
    /// transform`) -- no new API needed for it. Moving one node is the
    /// identical mechanism, called on the node itself.
    ///
    /// **Real, deliberate scope boundary, stated not silently
    /// dropped:** no dedicated edges API. Nodes and edges live in the
    /// same coordinate space once both are real children of `graph` --
    /// an `add_canvas` node reparented into `graph` alongside its
    /// nodes, stroking segments between each node's own known local
    /// position, exactly `examples/positioned_graph.py`'s own already-
    /// proven real pattern (M6 Phase 3). A per-edge API was considered
    /// and rejected: drawing an edge needs every node's own position
    /// at once to redraw correctly, which fits one app-owned `Canvas`
    /// far more naturally than a collection of individually-tracked
    /// edge nodes would. No drag-to-move mouse gesture either -- a
    /// real, confirmed gap, not an oversight: this codebase has no
    /// Python-facing `PointerMoved`-while-pressed hook at all
    /// (`set_on_click`/`set_on_hover_*` are the only generic input
    /// hooks that exist), so genuine mouse-follows-node dragging isn't
    /// buildable from Python today -- the same real constraint
    /// `Docking`'s own M4 Phase 9 "press+release only, no mid-drag
    /// tracking" scope boundary already found and documented.
    /// `node.animate("transform", ...)` still lets an app reposition a
    /// node programmatically (a toolbar action, a layout algorithm, a
    /// keyboard nudge) -- the real, available mechanism.
    #[pyo3(signature = (graph, label, x, y, width, height))]
    #[allow(clippy::too_many_arguments)]
    fn add_graph_node(
        &self,
        graph: PyRef<'_, Node>,
        label: &str,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> PyResult<Node> {
        if !Rc::ptr_eq(&self.tree, &graph.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        let (body_color, title_color, label_color) = {
            let theme = self.theme.borrow();
            let body = if theme.is_set() {
                theme
                    .role("surface_container")
                    .unwrap_or(Md3Baseline::SURFACE_CONTAINER)
            } else {
                Md3Baseline::SURFACE_CONTAINER
            };
            let title = if theme.is_set() {
                theme
                    .role("surface_container_high")
                    .unwrap_or(Md3Baseline::SURFACE_CONTAINER_HIGH)
            } else {
                Md3Baseline::SURFACE_CONTAINER_HIGH
            };
            let label = if theme.is_set() {
                theme
                    .role("on_surface_variant")
                    .unwrap_or(Md3Baseline::ON_SURFACE_VARIANT)
            } else {
                Md3Baseline::ON_SURFACE_VARIANT
            };
            (body, title, label)
        };

        // M50 Phase 4: its own key, not "card" -- reuses `CARD_CORNER_
        // RADIUS` as a real constant *value* today, the same
        // "coincidental shared number" reasoning `add_search_view`'s
        // own doc comment already states.
        let corner_radius = self
            .theme
            .borrow()
            .shape("graph_node", None)
            .unwrap_or(CARD_CORNER_RADIUS);
        let mut tree = self.tree.borrow_mut();
        let title_height = NODE_GRAPH_TITLE_HEIGHT.min(height);

        let wrapper = tree.insert(
            NodeKind::Rect,
            positioned_style(
                Size {
                    width: length(width),
                    height: length(height),
                },
                Some(x),
                Some(y),
            ),
            PaintProperties::new(body_color, corner_radius, 0.0, 1.0),
        );

        let title_bar = tree.insert(
            NodeKind::Rect,
            Style {
                position: Position::Absolute,
                inset: TaffyRect {
                    left: zero(),
                    top: zero(),
                    right: auto(),
                    bottom: auto(),
                },
                size: Size {
                    width: length(width),
                    height: length(title_height),
                },
                ..Default::default()
            },
            PaintProperties::new(title_color, 0.0, 0.0, 1.0),
        );
        tree.add_child(wrapper, title_bar);
        // Real, confirmed bug found live while verifying this method
        // end to end, not predicted in advance: `title_bar` is a real
        // `Rect` covering the node's own top band, including its own
        // real vertical center (a 60dp-tall node's own center sits
        // inside a 32dp title strip) -- without this, a click on the
        // node genuinely lands on `title_bar` first (`hit_test_at`
        // recurses into children first) and never reaches `wrapper`'s
        // own registered handler. The same real bug class, and the
        // same real fix, `Navigation Rail`'s active-indicator pill and
        // `Tabs`'s content wrapper already found and fixed this
        // milestone (`Node.hit_testable`'s own doc comment).
        tree.set_hit_testable(title_bar, false);

        let label_width = (width - 2.0 * NODE_GRAPH_TITLE_PADDING).max(0.0);
        let label_id = tree.insert(
            NodeKind::Text(TextState {
                content: label.to_string(),
                font_family: "Roboto".to_string(),
                font_weight: TAB_LABEL_FONT_WEIGHT,
                font_size: TAB_LABEL_FONT_SIZE,
                align: TextAlign::Start,
            }),
            Style {
                position: Position::Absolute,
                inset: TaffyRect {
                    left: length(NODE_GRAPH_TITLE_PADDING),
                    top: length((title_height - TAB_LABEL_FONT_SIZE) / 2.0),
                    right: auto(),
                    bottom: auto(),
                },
                size: Size {
                    width: length(label_width),
                    height: length(TAB_LABEL_FONT_SIZE + 2.0),
                },
                ..Default::default()
            },
            PaintProperties::new(label_color, 0.0, 0.0, 1.0),
        );
        tree.add_child(wrapper, label_id);

        tree.add_child(graph.id, wrapper);
        drop(tree);
        self.retheme_hooks
            .borrow_mut()
            .push(graph_node_retheme_hook(wrapper, title_bar, label_id));
        Ok(self.wrap_node(wrapper))
    }

    /// M23 Phase 1 (§1, §3): creates a real `NodeKind::Icon` from one
    /// of this project's own real curated Material Symbols icons
    /// (`engine_md3::icons::path_for`) -- deliberately takes one
    /// square `size`, not `width`+`height` the way every other
    /// `add_*` method does: Material Symbols icons are a real,
    /// uniformly square icon system by design (every fetched icon's
    /// own SVG `width`/`height` attributes are identical), so a
    /// single size parameter is a genuine ergonomic fit, not an
    /// invented shortcut. `color` is the icon's own real, plain fill
    /// tint -- MD3 icons have no separate "background" the way a
    /// boxed component does, so unlike `add_rect`/`add_checkbox` this
    /// takes no `background` param at all (mirroring `add_canvas`/
    /// `add_image`'s own real precedent for a kind with no meaningful
    /// separate background). An unknown `name` is a real, clear
    /// `PyValueError` -- `parse_dock_side`/`parse_content_fit`'s own
    /// established "fail loudly at the boundary" pattern, not routed
    /// through `EngineError` since this is a pure name-lookup failure
    /// with no I/O involved, the same reason those two live directly
    /// here rather than in `error.rs`.
    #[pyo3(signature = (name, color, size, x=None, y=None))]
    fn add_icon(
        &self,
        name: &str,
        color: (u8, u8, u8, u8),
        size: f32,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        let path = resolve_icon_path(name)?;
        let (r, g, b, a) = color;

        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Icon(IconState::new(path, Color::from_rgba8(r, g, b, a))),
            positioned_style(
                Size {
                    width: length(size),
                    height: length(size),
                },
                x,
                y,
            ),
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        Ok(self.wrap_node(id))
    }

    /// M15 Phase 1 (§5, §16.7): creates a real `NodeKind::TextField`,
    /// mirroring `add_checkbox`/`add_slider`'s own real shape --
    /// `background` is the field's own real box fill (universal
    /// `PaintProperties`, same as any other node); `content`/
    /// `font_family`/`font_weight`/`font_size` seed `TextFieldState`
    /// directly (`TextFieldState::new`'s own real contract: `cursor`
    /// starts at `content`'s own end). **Real finding (see `PLAN.md`):**
    /// this is the first real `engine-py` caller of `Tree::set_access`
    /// anywhere -- every other `add_*` method leaves a node at the
    /// default `Role::Unknown`/no actions, confirmed via grep before
    /// this method. A `TextField` is inherently interactive (unlike a
    /// plain `Rect`, which only becomes Tab-reachable as a side effect
    /// of `set_on_click`), so it opts into `Role::TextInput` +
    /// `Action::Focus` right here at construction, not deferred to a
    /// later opt-in call.
    #[pyo3(signature = (background, width, height, content="", font_family="Roboto", font_weight=400.0, font_size=16.0, x=None, y=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_text_field(
        &self,
        background: (u8, u8, u8, u8),
        width: f32,
        height: f32,
        content: &str,
        font_family: &str,
        font_weight: f32,
        font_size: f32,
        x: Option<f32>,
        y: Option<f32>,
    ) -> Node {
        let (r, g, b, a) = background;
        // M20 Phase 2 (§7.1, §7.3): same real "themed-at-construction,
        // gated on a real theme actually being set" reasoning as
        // `add_checkbox`/`add_slider` (M20 Phase 1) -- `TextFieldState`
        // 's own real default text color is dark (`0x1C1B1F`), not
        // `on_surface()`'s own black no-theme default.
        let mut text_field_state =
            TextFieldState::new(content, font_family, font_weight, font_size);
        {
            let theme = self.theme.borrow();
            if theme.is_set() {
                text_field_state.text_tint = theme.on_surface();
            }
        }
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::TextField(text_field_state),
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
        tree.set_access(
            id,
            AccessNodeData::new(Role::TextInput).with_action(Action::Focus),
        );
        tree.add_child(self.root, id);
        self.wrap_node(id)
    }

    /// M30 Phase 9 Step 3 (§5, §8, §10): `Code Editor`, a real
    /// multiline `NodeKind::TextField` -- reuses `add_text_field`'s
    /// own exact real shape verbatim, differing only in `TextField
    /// State.multiline = true` (this step's own new field). No
    /// official MD3 page exists (confirmed via the same directory-
    /// listing technique this milestone already uses); designed from
    /// pyCopper's own real `CodeEditor` widget, checked directly, not
    /// assumed -- its own doc comment names the real reasons a genuine
    /// code editor is built alongside `TextField`/`TextFieldElement`
    /// rather than as a thin subclass of it (M3-specific chrome a code
    /// surface has none of), the same real split this step's own
    /// `multiline` flag honors instead by staying inside the one
    /// shared `NodeKind`, since `engine-core`'s own real editing model
    /// (cursor/selection/undo-free keyboard mutation) is already
    /// exactly what both need, unlike pyCopper's own M3-styled paint
    /// chrome, which this codebase's `TextField` doesn't have either.
    ///
    /// **Real, honestly-scoped v1 at this step -- since substantially
    /// widened, see M31 immediately below, not a full IDE-grade editor
    /// even now:** closed, at this step, the one genuinely load-bearing
    /// gap (real multiline editing: `Enter` inserts `\n`, `Home`/`End`
    /// operate per-line, `ArrowUp`/`ArrowDown` navigate by line
    /// preserving column -- `TextFieldState.multiline`'s own doc
    /// comment, `Tree::dispatch_text_field_key`'s own real logic).
    ///
    /// **M31 (§5, §8, §10), all 6 phases: real syntax highlighting, a
    /// real composed line-number gutter, and real Tab-key indentation
    /// capture all closed the three real gaps this doc comment used to
    /// name here as deliberately deferred -- corrected directly rather
    /// than left stale, confirmed by direct re-check of the live source
    /// before writing this correction, not assumed from the tracker
    /// alone.** Real per-token syntax coloring: `Node.set_syntax_spans`
    /// (`TextFieldState.syntax_spans`, app-side tokenization only --
    /// Design Principle 6, `engine-core` never interprets the ranges
    /// itself, the identical "no engine-bundled lexer" split pyCopper's
    /// own optional-Pygments design already established). Real
    /// line-number gutter: composed from existing primitives, not a
    /// new engine capability -- an ordinary sibling `Text` node with
    /// the editor's own identical `font_family`/`font_weight`/
    /// `font_size` lines up with the editor's own real per-line Y
    /// positions *by construction*, both built through the identical
    /// real `shaped_layout` path; `examples/code_editor_gutter.py` has
    /// the real, working, live-updating pattern to copy. Real Tab-key
    /// indentation capture: a focused *multiline* field now claims
    /// `Tab` before ordinary focus traversal and inserts a literal
    /// `\t` (`Tree::dispatch_text_field_key`'s own real "first refusal"
    /// contract, widened) -- a single-line `TextField` keeps its prior
    /// real behavior byte-for-byte, `Tab` still moves focus there.
    /// M31 also added real visible space/tab glyphs (`show_whitespace`,
    /// set `true` below) and real code folding (`Node.set_folded_
    /// ranges`) -- see each field's own doc comment for the full real
    /// design. **Still real, stated, deliberately deferred, matching
    /// pyCopper's own identical v1 scope for the identical reasons:**
    /// multi-cursor editing, a minimap, bracket auto-closing/matching,
    /// and any language-server (LSP) integration -- "an LSP client is
    /// an *application* concern, never a widget's own hard dependency"
    /// (`BUILD_TRACKER.md`'s own M31 closing note).
    ///
    /// **M38 Phase 7 (§5, §8): real vertical scroll+clip for content
    /// taller than the box, with real caret-follow, closes the gap
    /// this doc comment used to name here.** `TextFieldState.scroll_
    /// offset`'s own doc comment has the full real design (a dedicated
    /// mechanism, not `ScrollView` wrapping -- scoped via
    /// `AskUserQuestion` since that would need real `taffy` measure-
    /// function integration this codebase has no precedent for).
    /// **M39 Phase 1 (§5, §8): real horizontal scroll+clip for a line
    /// wider than the box, with real horizontal caret-follow, closes
    /// the real, separate gap this doc comment used to name here
    /// too.** `TextFieldState.horizontal_scroll_offset`'s own doc
    /// comment has the full real design -- the identical dedicated-
    /// mechanism shape, parallel to `scroll_offset`'s own vertical one.
    ///
    /// **M32 Phase 1 (§5, §8, §10):** always shapes with the real
    /// bundled monospace face (`engine_render::MONOSPACE_FONT_FAMILY`,
    /// "Hack Nerd Font Mono") rather than the general-purpose `Roboto`
    /// this step originally had to fall back to -- a genuinely monospace
    /// editor at last, not an approximation. `font_family` still isn't
    /// exposed as a param here (unlike `add_text_field`): a code editor
    /// composed from a proportional face would defeat the whole point
    /// of this widget class, so there is still only one real correct
    /// choice, now a bundled one rather than an absent one.
    #[pyo3(signature = (content, background, width, height, font_weight=400.0, font_size=14.0, x=None, y=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_code_editor(
        &self,
        content: &str,
        background: (u8, u8, u8, u8),
        width: f32,
        height: f32,
        font_weight: f32,
        font_size: f32,
        x: Option<f32>,
        y: Option<f32>,
    ) -> Node {
        let (r, g, b, a) = background;
        let mut text_field_state =
            TextFieldState::new(content, MONOSPACE_FONT_FAMILY, font_weight, font_size);
        text_field_state.multiline = true;
        // M31 Phase 3 (§5, §8): a real Code Editor shows space/tab as
        // visible glyphs by default -- the same real convention every
        // real code editor (VS Code, Sublime Text) already has for
        // this exact widget class, unlike an ordinary `add_text_field`
        // form input, where a visible indicator would be real, unasked-
        // for visual noise.
        text_field_state.show_whitespace = true;
        {
            let theme = self.theme.borrow();
            if theme.is_set() {
                text_field_state.text_tint = theme.on_surface();
            }
        }
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::TextField(text_field_state),
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
        tree.set_access(
            id,
            AccessNodeData::new(Role::TextInput).with_action(Action::Focus),
        );
        tree.add_child(self.root, id);
        self.wrap_node(id)
    }

    /// M30 Phase 9 Step 4 (§5, §8, §10): `Terminal`, a real, live
    /// pseudo-terminal -- spawns `shell` on a real PTY (`portable_pty`)
    /// and parses its real byte stream with a real VT100 parser
    /// (`vt100`), the identical real "spawn a real pseudo-terminal is
    /// OS-specific process management; interpreting its byte stream is
    /// the VT/ANSI state machine every real terminal emulator
    /// implements identically -- neither is this widget's own concern
    /// to reinvent" split the sibling `pyCopper` project's own real
    /// `Terminal` widget already established (`crates/engine-py/src/
    /// terminal.rs`'s own module doc comment has the full real design).
    /// No official MD3 page exists (confirmed via the same directory-
    /// listing technique this milestone already uses).
    ///
    /// `width`/`height` are computed from `cols`/`rows` via the real
    /// bundled monospace face's own measured cell size (M32 Phase 1,
    /// §5, §8, §10: `TextRenderer::monospace_cell_size`, replacing the
    /// old `engine_core::terminal_cell_size` analytic estimate) -- the
    /// identical real per-cell grid `engine-render`'s own `draw_
    /// terminal` positions every cell on, so the node's own box always
    /// exactly fits its own real grid, no manual size bookkeeping for
    /// the app. **Real, confirmed POSIX-only v1**, the identical real
    /// scope pyCopper's own `Terminal` already chose for the same
    /// stated reason (`terminal.rs`'s own doc comment).
    ///
    /// M32 Phase 5 (§4, §8): `scrollback_lines` is a real, retained
    /// history length (`0` for none) -- `Node.scroll_terminal`/a real
    /// mouse wheel over a focused terminal move the viewport into it.
    #[pyo3(signature = (shell, cols, rows, background, font_size=14.0, scrollback_lines=1000, x=None, y=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_terminal(
        &self,
        shell: &str,
        cols: u16,
        rows: u16,
        background: (u8, u8, u8, u8),
        font_size: f32,
        scrollback_lines: usize,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        let session =
            TerminalSession::spawn(shell, cols, rows, scrollback_lines).map_err(|reason| {
                EngineError::TerminalSpawnFailed {
                    shell: shell.to_string(),
                    reason,
                }
            })?;

        let (r, g, b, a) = background;
        // M32 Phase 1 (§5, §8, §10): a throwaway `TextRenderer` solely
        // to measure the real bundled monospace face -- a real, one-
        // time cost per `add_terminal` call (font registration, not a
        // per-frame cost), the identical real "font discovery is a
        // one-time cost" reasoning `TextRenderer::new`'s own doc comment
        // already states, just paid here rather than amortized across
        // an app's whole lifetime the way the real render-loop's own
        // `TextRenderer` instance is.
        let (cell_width, cell_height) =
            TextRenderer::new().monospace_cell_size(MONOSPACE_FONT_FAMILY, font_size);
        let width = cell_width * f32::from(cols);
        let height = cell_height * f32::from(rows);

        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Terminal(engine_core::TerminalState::new(
                cols,
                rows,
                MONOSPACE_FONT_FAMILY,
                font_size,
            )),
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
        tree.set_access(
            id,
            AccessNodeData::new(Role::TextInput).with_action(Action::Focus),
        );
        tree.add_child(self.root, id);
        drop(tree);

        self.terminals.borrow_mut().insert(id, session);
        Ok(self.wrap_node(id))
    }

    /// M32 Phase 1 (§5, §8, §10): the real per-`font_size` cell size
    /// (`width`, `height`) of the bundled monospace face (`Hack Nerd
    /// Font Mono`) that `add_terminal`/`add_code_editor` themselves now
    /// use internally -- exposed here so app-level layout code (a
    /// gutter's own per-line click target, `examples/code_editor_
    /// folding.py`'s own toggle affordance) can size against the exact
    /// real value actually painted, rather than reaching for its own
    /// approximation the way `LINE_HEIGHT = FONT_SIZE * 1.3` (that
    /// example's own prior real placeholder, `terminal_cell_size`'s own
    /// now-removed doc comment) had to before a real font existed to
    /// measure.
    fn get_monospace_cell_size(&self, font_size: f32) -> (f32, f32) {
        TextRenderer::new().monospace_cell_size(MONOSPACE_FONT_FAMILY, font_size)
    }

    /// M33 Phase 1 (§4, §5, §8): resizes a real, live `Terminal`'s own
    /// grid -- the real capability `TerminalSession::resize` (spawned
    /// at M30 Phase 9 Step 4, kept `#[allow(dead_code)]` until this
    /// phase actually needed it) always had, finally given a real
    /// caller. Resizes the real kernel-level PTY and the `vt100`
    /// parser's own screen buffer (`TerminalSession::resize`'s own doc
    /// comment has the full real reasoning, including the one real,
    /// honest shell-redraw caveat), then recomputes this node's own
    /// real box from `cols`/`rows` the identical way `add_terminal`
    /// itself does at construction time, via `Tree::set_layout_style`
    /// (the one real, correct way to push a style update back into
    /// `taffy`, confirmed the hard way by M32 Phase 2's own real bug:
    /// mutating `layout_style` directly desyncs taffy's own internal
    /// copy). Raises `ValueError` if `node` isn't a real `Terminal`
    /// this `Window` created.
    fn resize_terminal(&self, node: PyRef<'_, Node>, cols: u16, rows: u16) -> PyResult<()> {
        if !Rc::ptr_eq(&self.tree, &node.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        let (font_family, font_size) = {
            let tree = self.tree.borrow();
            match tree.get(node.id).map(|n| &n.kind) {
                Some(NodeKind::Terminal(state)) => (state.font_family.clone(), state.font_size),
                _ => return Err(EngineError::NotATerminal.into()),
            }
        };
        if !self.terminals.borrow().contains_key(&node.id) {
            return Err(EngineError::NotATerminal.into());
        }

        let (cell_width, cell_height) =
            TextRenderer::new().monospace_cell_size(&font_family, font_size);
        let width = cell_width * f32::from(cols);
        let height = cell_height * f32::from(rows);
        {
            let mut tree = self.tree.borrow_mut();
            let mut style = tree.get(node.id).unwrap().layout_style.clone();
            style.size = Size {
                width: length(width),
                height: length(height),
            };
            tree.set_layout_style(node.id, style);
        }

        if let Some(session) = self.terminals.borrow_mut().get_mut(&node.id) {
            session.resize(&mut self.tree.borrow_mut(), node.id, cols, rows);
        }
        Ok(())
    }

    /// M30 Phase 9 Step 5 (§5, §7, §11.7): a real MD3 carousel --
    /// mirrors `add_rect`'s own real shape (an explicit `width`/
    /// `height`, the same real convention every other MD3 component
    /// factory here already follows, since a `NodeKind::Carousel`'s
    /// own children are absolutely positioned and so contribute nothing
    /// to its real auto-sizing, confirmed by `Tree::sync_carousel_
    /// layouts`'s own doc comment). Returns an empty strip -- real
    /// items are added the same generic way any other container's
    /// children are, via the already-real `Node.add_child` (M4 Phase 1
    /// step 3), not a bespoke "carousel item" factory of its own.
    #[pyo3(signature = (layout, width, height, background, x=None, y=None))]
    fn add_carousel(
        &self,
        layout: &str,
        width: f32,
        height: f32,
        background: (u8, u8, u8, u8),
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        let carousel_layout = parse_carousel_layout(layout)?;
        let (r, g, b, a) = background;
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Carousel(engine_core::CarouselState::new(carousel_layout)),
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
        Ok(self.wrap_node(id))
    }

    /// M36 Phase 1 (§5, §7, §11.7): a real, general scrollable viewport
    /// over exactly one child -- see `ScrollViewState`'s own doc
    /// comment for the full real design (grounded directly in the
    /// sibling `pyCopper` project's own `ScrollViewElement`). Returns
    /// a plain container `Node`; the caller composes their own real
    /// content in via the existing, generic `Node.add_child` (M6
    /// Phase 1), the identical real "engine provides the primitive,
    /// app composes" split `add_toolbar` (M35 Phase 1) already
    /// established for the same real reason -- a real content node
    /// (often a flex column of many rows) needs its own real, explicit
    /// size on the scroll axis matching its own true content extent,
    /// the same "caller supplies a real explicit size" convention
    /// every other `add_*` factory in this codebase already has; this
    /// call has no opinion about what that content actually is.
    /// `horizontal=false` (the default) scrolls vertically; `true`
    /// scrolls horizontally -- never both at once, the identical
    /// single-axis-at-a-time real scope `ScrollViewState`'s own doc
    /// comment already states. Real wheel scrolling and `Window.
    /// scroll` both already work with zero other changes: `ScrollView`
    /// joins `Tree::dispatch`'s existing "walk up to the nearest
    /// scrollable ancestor" mechanism the same way `VirtualList`/
    /// `Carousel` already do.
    #[pyo3(signature = (width, height, horizontal=false, x=None, y=None))]
    fn add_scroll_view(
        &self,
        width: f32,
        height: f32,
        horizontal: bool,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::ScrollView(engine_core::ScrollViewState::new(horizontal)),
            positioned_style(
                Size {
                    width: length(width),
                    height: length(height),
                },
                x,
                y,
            ),
            PaintProperties::new(TRANSPARENT, 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        Ok(self.wrap_node(id))
    }

    /// M13 Phase 1 (§11.2): a real, one-call way to build `AppShell`'s
    /// own named regions -- `self.root` itself stays a plain `Flex Row`
    /// (every other `add_*` method's own implicit flow depends on that,
    /// confirmed by direct read of `PyWindow::new`), so this creates one
    /// new dedicated `Container` child of `self.root`, `Flex Column`,
    /// sized to the window's own real width/height -- the one new
    /// structural node this phase adds. `menu_bar`/`toolbar`/`status_
    /// bar` are already-built `Node`s the app supplies (this method is
    /// a composition convenience, not a content-authoring one, matching
    /// `AppShell`'s own struct sketch: it names *which* node serves
    /// which chrome role, it doesn't build that node's own content) --
    /// each is re-parented into the shell container via `Tree::try_add_
    /// child`, not the cheap `Tree::add_child` the freshly-inserted
    /// `shell`/`content` nodes below use -- **real finding while writing
    /// this phase's own example:** every `add_*` method already
    /// attaches its result to `self.root` immediately, so `menu_bar`/
    /// `toolbar`/`status_bar` always already have a real parent by the
    /// time this runs; the cheap `add_child` only ever detaches nothing,
    /// leaving a node listed as a child of *both* its old parent and the
    /// shell -- real tree corruption `examples/app_shell.py`'s own live
    /// `accesskit` validation caught as a duplicate-child panic, not any
    /// pytest test (none render a real frame). `try_add_child` is the
    /// real, checked counterpart that detaches first, the same mechanism
    /// `Node.add_child`'s own pyo3 wrapper already uses. A new, empty
    /// `content` `Container` is created here, `flex_grow: 1.0` so it
    /// fills whatever vertical space the given chrome regions don't take
    /// -- the one handle the caller needs to keep, for Phase 2's own
    /// real navigation.
    #[pyo3(signature = (menu_bar=None, toolbar=None, status_bar=None))]
    fn build_shell(
        &mut self,
        menu_bar: Option<PyRef<'_, Node>>,
        toolbar: Option<PyRef<'_, Node>>,
        status_bar: Option<PyRef<'_, Node>>,
    ) -> PyResult<Node> {
        for region in [&menu_bar, &toolbar, &status_bar].into_iter().flatten() {
            if !Rc::ptr_eq(&self.tree, &region.tree) {
                return Err(EngineError::ForeignNode.into());
            }
        }

        let mut tree = self.tree.borrow_mut();
        let shell = tree.insert(
            NodeKind::Container,
            Style {
                display: taffy::Display::Flex,
                flex_direction: taffy::FlexDirection::Column,
                size: Size {
                    width: length(self.width.get() as f32),
                    height: length(self.height.get() as f32),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, shell);

        // menu_bar/toolbar/status_bar are pre-existing nodes -- every
        // add_* method already attaches its result to self.root
        // immediately, so each already has a real parent here. The
        // cheap, unchecked `Tree::add_child` above is only ever correct
        // for a freshly-inserted node with no parent yet (confirmed via
        // direct read of its own doc comment) -- reusing it for an
        // already-attached node would leave it listed as a child of
        // *both* its old parent and the shell, real tree corruption
        // caught live by accesskit's own duplicate-child panic while
        // writing this example, not by any pytest test (none render a
        // real frame). `try_add_child` is the real, checked counterpart
        // that detaches first, the same mechanism `Node.add_child`'s
        // own pyo3 wrapper already uses.
        if let Some(menu_bar) = &menu_bar {
            tree.try_add_child(shell, menu_bar.id);
        }
        if let Some(toolbar) = &toolbar {
            tree.try_add_child(shell, toolbar.id);
        }

        let content = tree.insert(
            NodeKind::Container,
            Style {
                flex_grow: 1.0,
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(shell, content);

        if let Some(status_bar) = &status_bar {
            tree.try_add_child(shell, status_bar.id);
        }

        drop(tree);
        Ok(self.wrap_node(content))
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
        self.wrap_node(id)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::window::ThemeState;

    fn leaf() -> (NodeId, Tree) {
        let mut tree = Tree::new();
        let id = tree.insert(
            NodeKind::Rect,
            Style::default(),
            PaintProperties::new(Color::TRANSPARENT, 0.0, 0.0, 1.0),
        );
        (id, tree)
    }

    /// M52 Phase 1: `button_retheme_hook` -- the milestone's own proof
    /// of concept. Exact-`Color`/`f64`-value assertions, not "does not
    /// raise" -- possible here (unlike this suite's pytest coverage,
    /// which has no Python-facing getter for a node's `background`)
    /// because the hook-builder function is plain, testable Rust,
    /// confirmed GIL-free.
    #[test]
    fn recomputes_container_and_label_color_when_the_theme_changes() {
        let (container, mut tree) = leaf();
        let label = tree.insert(
            NodeKind::Text(TextState {
                content: "Save".to_string(),
                font_family: "Roboto".to_string(),
                font_weight: BUTTON_LABEL_FONT_WEIGHT,
                font_size: BUTTON_LABEL_FONT_SIZE,
                align: TextAlign::Center,
            }),
            Style::default(),
            PaintProperties::new(Color::TRANSPARENT, 0.0, 0.0, 1.0),
        );
        let hook = button_retheme_hook(container, label, "filled".to_string(), 40.0);

        let theme_a = ThemeState::for_test(Color::from_rgba8(0x67, 0x50, 0xA4, 0xFF));
        hook(&theme_a, &mut tree);
        let container_after_a = tree.get(container).unwrap().paint.background.current;
        assert_eq!(
            container_after_a,
            theme_a.role("primary").unwrap(),
            "expected the exact real resolved 'primary' role for the filled variant"
        );

        let theme_b = ThemeState::for_test(Color::from_rgba8(0x00, 0x66, 0x00, 0xFF));
        hook(&theme_b, &mut tree);
        let container_after_b = tree.get(container).unwrap().paint.background.current;
        let label_after_b = tree.get(label).unwrap().paint.background.current;

        // "filled" resolves container/label from the "primary"/
        // "on_primary" roles directly -- exact-value assertions against
        // the second theme's own real resolution, not an inequality
        // check against the first: MD3's tone mapping can legitimately
        // resolve "on_primary" to the identical white for two different
        // hues that are both dark enough, so "must differ from theme_a"
        // is not a valid general assumption (caught by actually running
        // this test with two real, dark seeds -- both resolved on_
        // primary to white).
        assert_ne!(
            container_after_a, container_after_b,
            "a real seed change must produce a real, different resolved container color"
        );
        assert_eq!(
            container_after_b,
            theme_b.role("primary").unwrap(),
            "expected the exact real resolved 'primary' role for the filled variant"
        );
        assert_eq!(
            label_after_b,
            theme_b.role("on_primary").unwrap(),
            "expected the exact real resolved 'on_primary' role, re-resolved against the new theme"
        );
    }

    /// The hook's own `corner_radius` half -- a real `components:`
    /// override must apply live too, not just color, and a later call
    /// with no override at all must fall back to `height / 2.0` again
    /// (the exact same "each `set_theme` is a complete, fresh selection"
    /// convention `View.set_theme` (M51) already established).
    #[test]
    fn recomputes_corner_radius_from_a_components_override_when_the_theme_changes() {
        let (container, mut tree) = leaf();
        let label = tree.insert(
            NodeKind::Text(TextState {
                content: "Save".to_string(),
                font_family: "Roboto".to_string(),
                font_weight: BUTTON_LABEL_FONT_WEIGHT,
                font_size: BUTTON_LABEL_FONT_SIZE,
                align: TextAlign::Center,
            }),
            Style::default(),
            PaintProperties::new(Color::TRANSPARENT, 0.0, 0.0, 1.0),
        );
        let hook = button_retheme_hook(container, label, "filled".to_string(), 40.0);
        let seed = Color::from_rgba8(0x67, 0x50, 0xA4, 0xFF);

        let mut overrides = HashMap::new();
        overrides.insert(
            "button.filled".to_string(),
            engine_spec::ComponentOverride {
                corner_radius: Some(4.0),
                elevation: None,
            },
        );
        let themed = ThemeState::for_test_with_components(seed, overrides);
        hook(&themed, &mut tree);
        assert_eq!(
            tree.get(container).unwrap().paint.corner_radius.current,
            4.0,
            "expected the real components: override, not height / 2.0"
        );

        let unthemed = ThemeState::for_test(seed);
        hook(&unthemed, &mut tree);
        assert_eq!(
            tree.get(container).unwrap().paint.corner_radius.current,
            20.0,
            "a later set_theme with no override must reset to height / 2.0 (40.0 / 2.0), \
             not silently keep the previous override"
        );
    }

    /// A hook whose `NodeId`s were since removed (`Node.remove()`) must
    /// be a safe no-op, the same contract `window::tests` already
    /// proved for the generic mechanism -- re-proved here at the real
    /// `button_retheme_hook` call site, not just the abstract one.
    #[test]
    fn is_a_safe_no_op_after_both_nodes_are_removed() {
        let (container, mut tree) = leaf();
        let label = tree.insert(
            NodeKind::Text(TextState {
                content: "Save".to_string(),
                font_family: "Roboto".to_string(),
                font_weight: BUTTON_LABEL_FONT_WEIGHT,
                font_size: BUTTON_LABEL_FONT_SIZE,
                align: TextAlign::Center,
            }),
            Style::default(),
            PaintProperties::new(Color::TRANSPARENT, 0.0, 0.0, 1.0),
        );
        tree.remove(container);
        tree.remove(label);
        let hook = button_retheme_hook(container, label, "filled".to_string(), 40.0);
        let theme = ThemeState::for_test(Color::from_rgba8(0x67, 0x50, 0xA4, 0xFF));
        hook(&theme, &mut tree);
        assert!(tree.get(container).is_none());
        assert!(tree.get(label).is_none());
    }

    /// M52 Phase 5: `radio_button_retheme_hook` -- real, exact-value
    /// proof that a gap `Tree::set_all_component_tints` itself was
    /// never able to close (its own doc comment: "a radio button
    /// genuinely needs two different real roles... which that single-
    /// `Color`-parameter mechanism can't express") is now genuinely
    /// closed by a real per-component hook.
    #[test]
    fn radio_button_hook_recomputes_both_real_roles_when_the_theme_changes() {
        let mut tree = Tree::new();
        let id = tree.insert(
            NodeKind::RadioButton(engine_core::RadioButtonState::new(false)),
            Style::default(),
            PaintProperties::new(Color::TRANSPARENT, 0.0, 0.0, 1.0),
        );
        let hook = radio_button_retheme_hook(id);
        let theme = ThemeState::for_test(Color::from_rgba8(0x67, 0x50, 0xA4, 0xFF));
        hook(&theme, &mut tree);
        let NodeKind::RadioButton(state) = &tree.get(id).unwrap().kind else {
            panic!("expected a RadioButton node");
        };
        assert_eq!(state.unselected_tint, theme.role("outline").unwrap());
        assert_eq!(state.selected_tint, theme.role("primary").unwrap());
    }

    /// M52 Phase 5: `switch_retheme_hook` -- exact-value proof across
    /// all 5 real, independent roles, the component with the most of
    /// any in this catalog, confirmed previously completely untouched
    /// by `Window.set_theme` (unlike `Checkbox`/`Slider`, never even
    /// partially covered by the pre-existing tint-push mechanism).
    #[test]
    fn switch_hook_recomputes_all_five_real_roles_when_the_theme_changes() {
        let mut tree = Tree::new();
        let id = tree.insert(
            NodeKind::Switch(engine_core::SwitchState::new(false)),
            Style::default(),
            PaintProperties::new(Color::TRANSPARENT, 0.0, 0.0, 1.0),
        );
        let hook = switch_retheme_hook(id);
        let theme = ThemeState::for_test(Color::from_rgba8(0x67, 0x50, 0xA4, 0xFF));
        hook(&theme, &mut tree);
        let NodeKind::Switch(state) = &tree.get(id).unwrap().kind else {
            panic!("expected a Switch node");
        };
        assert_eq!(
            state.track_off_tint,
            theme.role("surface_container_highest").unwrap()
        );
        assert_eq!(state.track_on_tint, theme.role("primary").unwrap());
        assert_eq!(state.track_outline_tint, theme.role("outline").unwrap());
        assert_eq!(state.handle_off_tint, theme.role("outline").unwrap());
        assert_eq!(state.handle_on_tint, theme.role("on_primary").unwrap());
    }
}
