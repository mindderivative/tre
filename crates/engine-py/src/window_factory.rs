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
    AccessNodeData, Action, Animated, CheckboxState, ContentFit, IconState, ImageState, NodeKind,
    PaintProperties, Role, SliderState, SplitterState, TextAlign, TextFieldState, TextState,
};
use peniko::Color;
use pyo3::prelude::*;
use taffy::prelude::{AlignItems, JustifyContent, Rect as TaffyRect, Size, Style, length, zero};

use crate::error::EngineError;
use crate::node::Node;
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
    const SURFACE_CONTAINER_LOW: Color = Color::from_rgba8(0xF7, 0xF2, 0xFA, 0xFF);
    const SURFACE_CONTAINER_HIGH: Color = Color::from_rgba8(0xEC, 0xE6, 0xF0, 0xFF);
    const OUTLINE: Color = Color::from_rgba8(0x79, 0x74, 0x7E, 0xFF);
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
fn resolve_button_colors(
    theme: &crate::window::ThemeState,
    variant: &str,
) -> PyResult<ButtonColors> {
    let role = |name: &str, fallback: Color| -> Color {
        if theme.is_set() {
            theme.role(name).unwrap_or(fallback)
        } else {
            fallback
        }
    };
    match variant {
        "elevated" => Ok(ButtonColors {
            container: role("surface_container_low", Md3Baseline::SURFACE_CONTAINER_LOW),
            label: role("primary", Md3Baseline::PRIMARY),
            border_color: TRANSPARENT,
            border_width: 0.0,
            elevation: 1.0,
        }),
        "filled" => Ok(ButtonColors {
            container: role("primary", Md3Baseline::PRIMARY),
            label: role("on_primary", Md3Baseline::ON_PRIMARY),
            border_color: TRANSPARENT,
            border_width: 0.0,
            elevation: 0.0,
        }),
        "filled_tonal" => Ok(ButtonColors {
            container: role("secondary_container", Md3Baseline::SECONDARY_CONTAINER),
            label: role(
                "on_secondary_container",
                Md3Baseline::ON_SECONDARY_CONTAINER,
            ),
            border_color: TRANSPARENT,
            border_width: 0.0,
            elevation: 0.0,
        }),
        "outlined" => Ok(ButtonColors {
            container: TRANSPARENT,
            label: role("primary", Md3Baseline::PRIMARY),
            border_color: role("outline", Md3Baseline::OUTLINE),
            border_width: 1.0,
            elevation: 0.0,
        }),
        "text" => Ok(ButtonColors {
            container: TRANSPARENT,
            label: role("primary", Md3Baseline::PRIMARY),
            border_color: TRANSPARENT,
            border_width: 0.0,
            elevation: 0.0,
        }),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown button variant {other:?} -- expected one of \"elevated\", \"filled\", \
             \"filled_tonal\", \"outlined\", \"text\""
        ))),
    }
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

#[pymethods]
impl PyWindow {
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
        let colors = resolve_button_colors(&self.theme.borrow(), variant)?;
        let mut tree = self.tree.borrow_mut();

        let mut container_paint = PaintProperties::new(
            colors.container,
            f64::from(height) / 2.0,
            colors.elevation,
            1.0,
        );
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
        let colors = resolve_button_colors(&self.theme.borrow(), resolved_variant)?;
        let path = resolve_icon_path(icon)?;

        let mut tree = self.tree.borrow_mut();

        let mut container_paint = PaintProperties::new(
            colors.container,
            f64::from(size) / 2.0,
            colors.elevation,
            1.0,
        );
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
            NodeKind::Icon(IconState {
                path,
                tint: colors.label,
            }),
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
        let (container_size, corner_radius) = fab_shape(size)?;
        let colors = resolve_fab_colors(&self.theme.borrow(), variant)?;
        let path = resolve_icon_path(icon)?;

        let mut tree = self.tree.borrow_mut();
        let container_paint = PaintProperties::new(
            colors.container,
            f64::from(corner_radius),
            FAB_REST_ELEVATION_LEVEL,
            1.0,
        );
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
            NodeKind::Icon(IconState {
                path,
                tint: colors.icon,
            }),
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

        let mut tree = self.tree.borrow_mut();
        let container_paint = PaintProperties::new(
            colors.container,
            EXTENDED_FAB_CORNER_RADIUS,
            FAB_REST_ELEVATION_LEVEL,
            1.0,
        );
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

        if let Some(path) = icon_path {
            let icon_id = tree.insert(
                NodeKind::Icon(IconState {
                    path,
                    tint: colors.icon,
                }),
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
        Ok(self.wrap_node(container))
    }

    /// M14 Phase 1 (§5, §7.3): creates a real `NodeKind::Checkbox`,
    /// mirroring `add_rect`'s own real shape exactly -- `background`
    /// is the box's own real fill color (universal `PaintProperties`,
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
            NodeKind::Icon(IconState {
                path,
                tint: Color::from_rgba8(r, g, b, a),
            }),
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
                    width: length(self.width as f32),
                    height: length(self.height as f32),
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
