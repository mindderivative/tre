//! `WidgetSpec` -> `engine_core::Tree` (§14 step 5). This is the crate's
//! actual reason to exist for this step: de-risk parsing/validation/
//! mapping in isolation, per Design Principle 5, before anything depends
//! on it working -- rendered through the real Phase 2 pipeline (steps
//! 1-4) unchanged, since a built `Tree` is indistinguishable here from
//! one built imperatively.

use engine_core::{
    Animated, CheckboxState, CircularProgressState, IconState, LinearProgressState,
    LoadingIndicatorState, NodeId, NodeKind, PaintProperties, RadioButtonState, SliderState,
    SwitchState, TextAlign, TextFieldState, TextState, TimePickerDialState, Tree,
};
use engine_md3::ColorScheme;
use peniko::Color;
use taffy::prelude::{
    AlignItems, JustifyContent, Rect as TaffyRect, Size, Style, auto, length, zero,
};
use taffy::style_helpers::FromLength;

use crate::cascade::{Stylesheet, resolve_style_layered};
use crate::spec::{
    AlignItemsSpec, ContentFitSpec, FlexDirectionSpec, JustifyContentSpec, NodeKindSpec,
    ShapeOrElevationSpec, SpacingSpec, StyleSpec, TextSpec, WidgetSpec, parse_view,
    parse_view_json,
};

#[derive(Debug, thiserror::Error)]
pub enum SpecError {
    #[error("failed to parse view YAML: {0}")]
    Parse(#[from] serde_yaml_ng::Error),
    /// tre issue #3, Part A (Tier 2): `Parse`'s own JSON-front-end
    /// sibling -- a distinct variant, not a reused `Parse`, since
    /// `serde_json::Error` and `serde_yaml_ng::Error` are different
    /// types and `#[from]` only supports one source type per variant.
    #[error("failed to parse view JSON: {0}")]
    ParseJson(#[from] serde_json::Error),
    /// M90: `style.background` set inline on a kind whose paint color
    /// is its glyph or text -- the pre-M90 spelling. Names the
    /// replacement instead of silently dropping the value.
    #[error(
        "widget \"{id}\": {kind} has no fill, so style.background doesn't apply -- its \
         glyph/text color is style.foreground"
    )]
    NotAFill { id: String, kind: &'static str },
    /// M90: the wrong one of `checked`/`selected` for this kind --
    /// `Checkbox` takes `checked`, `Switch`/`RadioButton` take `selected`.
    #[error("widget \"{id}\": {kind} takes `{expected}:`, not `{given}:`")]
    WrongStateField {
        id: String,
        kind: &'static str,
        given: &'static str,
        expected: &'static str,
    },
    #[error("widget \"{id}\": {kind} requires {field}, none given")]
    MissingField {
        id: String,
        kind: &'static str,
        field: &'static str,
    },
    #[error("widget \"{id}\": invalid style.{field} \"{value}\": {source}")]
    InvalidColor {
        id: String,
        field: &'static str,
        value: String,
        #[source]
        source: peniko::color::ParseError,
    },
    /// M61 (§16.3): `style.corner_radius`/`elevation` (or a theme's own
    /// `components:` override, `engine-py::window.rs`) named a token
    /// that doesn't resolve against `engine_md3::shape`'s own real
    /// vocabulary -- a real, stated authoring error, not a silent
    /// fallback to `0.0`, the same "fail loudly at the boundary"
    /// reasoning `InvalidColor` above already follows for `background`.
    #[error("widget \"{id}\": unknown style.{field} token {token:?}")]
    UnknownShapeToken {
        id: String,
        field: &'static str,
        token: String,
    },
    /// M62 Phase 4 (§7.1, §16.3): `text.role` named a token that
    /// doesn't resolve against `engine_md3::type_style_named`'s own
    /// real 15-role vocabulary -- `UnknownShapeToken`'s own real
    /// typography sibling, the identical "fail loudly at the boundary"
    /// reasoning.
    #[error("widget \"{id}\": unknown text.typography_role {role:?}")]
    UnknownTypographyRole { id: String, role: String },
    /// `icon.name` named something outside `engine_md3::icons`'s own
    /// curated vocabulary -- the identical real "fail loudly, name what
    /// was expected" shape `UnknownTypographyRole` already follows,
    /// mirroring `Window.add_icon`'s own imperative `ValueError`.
    #[error("widget \"{id}\": unknown icon {name:?}")]
    UnknownIcon { id: String, name: String },
    /// M19 Phase 2 (§16.6): `include: {path}` appeared but no `base_dir`
    /// was given to resolve it against -- a real, stated error, not a
    /// silent no-op (an include with nowhere to resolve from must fail
    /// loudly, the same "fail loudly at the boundary" reasoning every
    /// other `SpecError` variant already follows).
    #[error("include: {path:?} requires a base directory to resolve against, none given")]
    IncludeNoBaseDir { path: String },
    /// `include:`'s own value must be a plain string path -- any other
    /// YAML shape (a number, a nested mapping, ...) is a real,
    /// stated authoring error, not silently coerced or ignored.
    #[error("include: value must be a plain string path, got {value:?}")]
    IncludeValueNotString { value: String },
    /// An `include:` mapping with any other key alongside it -- real,
    /// deliberately strict: §16.6's own illustration is always a bare
    /// single-key `{include: path}` mapping, so extra keys are almost
    /// certainly an author mistake worth failing on, not silently
    /// ignoring either the include or the extra keys.
    #[error(
        "include: {path:?} must be the only key in its own mapping, found {extra_keys:?} alongside it"
    )]
    IncludeNotSoleKey {
        path: String,
        extra_keys: Vec<String>,
    },
    /// Real path confinement (ARCHITECTURE.md §16.6's own stated
    /// requirement): an `include:` path that resolves outside its own
    /// base directory (an absolute path, or a real `../` escape,
    /// checked via `Path::canonicalize` so a symlink can't evade it
    /// either).
    #[error("include: {path:?} resolves outside the view directory it was included from")]
    IncludePathEscapesBase { path: String },
    /// A file cannot transitively include itself.
    #[error("include: {path:?} would create a real include cycle")]
    IncludeCycle { path: std::path::PathBuf },
    /// A real, stated depth limit -- not manufactured ahead of a real
    /// need, but a genuinely unbounded include chain (accidental or
    /// adversarial) needs a hard stop somewhere.
    #[error("include chain exceeded the maximum depth of {limit}")]
    IncludeDepthExceeded { limit: usize },
    /// Wraps a real filesystem failure reading an included file (not
    /// found, permission denied, ...) with the path that failed, since
    /// `std::io::Error` alone doesn't carry it.
    #[error("failed to read included view {path:?}: {source}")]
    IncludeReadFailed {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// M22 Phase 2 (§16.1): `kind: Image`'s own real `image.src:` path
    /// needs a base directory to resolve against, the identical real
    /// requirement `include:` already has (`IncludeNoBaseDir`'s own
    /// sibling) -- a distinct variant, not a reused `Include*` one, so
    /// the error message names the real YAML key an author actually
    /// wrote (`image.src`, not `include`).
    #[error("image.src: {path:?} requires a base directory to resolve against, none given")]
    ImageSrcNoBaseDir { path: String },
    /// Real path confinement, the identical real requirement `include:`
    /// already has (`IncludePathEscapesBase`'s own sibling).
    #[error("image.src: {path:?} resolves outside the view directory it was loaded from")]
    ImageSrcEscapesBase { path: String },
    /// Wraps a real filesystem failure resolving/reading `image.src:`
    /// (not found, permission denied, a real `../` escape whose
    /// canonicalization itself fails, ...).
    #[error("failed to read image {path:?}: {source}")]
    ImageReadFailed {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// A real file existed and was readable but wasn't a real,
    /// decodable image (`image::open`'s own real format-sniffing
    /// failure) -- distinct from `ImageReadFailed`, the same real
    /// "I/O failure" vs. "content failure" distinction `engine-py::
    /// EngineError::ImageLoadFailed` already draws for `Window.
    /// add_image`.
    #[error("failed to decode image {path:?}: {source}")]
    ImageDecodeFailed {
        path: std::path::PathBuf,
        #[source]
        source: image::ImageError,
    },
}

/// M22 Phase 2 (§16.1): `kind: Image`'s own real `image.src:` path
/// resolution -- the identical real canonicalization-based, symlink-
/// escape-resistant confinement `include.rs`'s own private `resolve_
/// confined` already established for `include:`, duplicated here
/// (rather than widened and shared) only because the two need
/// genuinely different `SpecError` variants so a real error message
/// names the YAML key an author actually wrote (`include:` vs.
/// `image.src:`) -- the same small, localized duplication this
/// codebase already tolerates elsewhere over a premature shared
/// abstraction for two call sites.
fn resolve_image_src(
    base_dir: &std::path::Path,
    src: &str,
) -> Result<std::path::PathBuf, SpecError> {
    if std::path::Path::new(src).is_absolute() {
        return Err(SpecError::ImageSrcEscapesBase {
            path: src.to_string(),
        });
    }
    let joined = base_dir.join(src);
    let canon_base = base_dir
        .canonicalize()
        .map_err(|source| SpecError::ImageReadFailed {
            path: base_dir.to_path_buf(),
            source,
        })?;
    let canon_joined = joined
        .canonicalize()
        .map_err(|source| SpecError::ImageReadFailed {
            path: joined.clone(),
            source,
        })?;
    if !canon_joined.starts_with(&canon_base) {
        return Err(SpecError::ImageSrcEscapesBase {
            path: src.to_string(),
        });
    }
    Ok(canon_joined)
}

/// Parses `yaml` and builds it into `tree`, returning the new subtree's
/// root `NodeId`. No stylesheet, no MD3 token resolution -- §14 step 5's
/// original literal-values-only behavior, kept exactly as-is for its one
/// existing caller (`engine-render/tests/spec_view.rs`). `load_styled_view`
/// below is the step-12 entry point that resolves both.
pub fn load_view(tree: &mut Tree, yaml: &str) -> Result<NodeId, SpecError> {
    let spec = parse_view(yaml)?;
    // M22 Phase 2 (§16.1): `None` is real and valid here, the identical
    // "no base directory, so a relative path fails clearly instead of
    // silently" contract `include:`'s own `base_dir: None` already
    // established -- `load_view`'s own public signature stays
    // unchanged for its one existing caller; a `kind: Image` loaded
    // this way (no base directory to resolve `src:` against at all)
    // gets a real, clear `SpecError::ImageSrcNoBaseDir`.
    build_tree(tree, &spec, None, None, None, None, None)
}

/// tre issue #3, Part A (Tier 2): `load_view`'s own JSON sibling --
/// identical behavior, `parse_view_json` instead of `parse_view`.
pub fn load_view_json(tree: &mut Tree, json: &str) -> Result<NodeId, SpecError> {
    let spec = parse_view_json(json)?;
    build_tree(tree, &spec, None, None, None, None, None)
}

/// The full §16.3 path: parses `yaml`, resolves every widget's style
/// through `sheet`'s cascade, and resolves any MD3 token name
/// (`background: primary`) against `scheme` -- falling back to literal
/// color parsing (`background: "#6750A4"`) for anything that isn't a
/// recognized role name. `base_dir` (M22 Phase 2, §16.1) resolves any
/// real `kind: Image` `image.src:` path -- `None` is real and valid,
/// the same `include:`-established contract `load_view`'s own doc
/// comment states. M49 Phase 3: `default_theme`/`custom_theme` are the
/// two new layers `resolve_style_layered` cascades *beneath* `sheet`,
/// both optional -- omitting both is byte-for-byte this function's own
/// pre-M49 behavior.
pub fn load_styled_view(
    tree: &mut Tree,
    yaml: &str,
    default_theme: Option<&Stylesheet>,
    custom_theme: Option<&Stylesheet>,
    sheet: &Stylesheet,
    scheme: &ColorScheme,
    base_dir: Option<&std::path::Path>,
) -> Result<NodeId, SpecError> {
    let spec = parse_view(yaml)?;
    build_tree(
        tree,
        &spec,
        default_theme,
        custom_theme,
        Some(sheet),
        Some(scheme),
        base_dir,
    )
}

/// tre issue #3, Part A (Tier 2): `load_styled_view`'s own JSON sibling
/// -- identical behavior, `parse_view_json` instead of `parse_view`.
#[allow(clippy::too_many_arguments)]
pub fn load_styled_view_json(
    tree: &mut Tree,
    json: &str,
    default_theme: Option<&Stylesheet>,
    custom_theme: Option<&Stylesheet>,
    sheet: &Stylesheet,
    scheme: &ColorScheme,
    base_dir: Option<&std::path::Path>,
) -> Result<NodeId, SpecError> {
    let spec = parse_view_json(json)?;
    build_tree(
        tree,
        &spec,
        default_theme,
        custom_theme,
        Some(sheet),
        Some(scheme),
        base_dir,
    )
}

/// Recursively inserts `spec` and its `children` into `tree`, wiring
/// each parent/child edge with `Tree::add_child` as it goes. `sheet`/
/// `scheme` are `None` for the plain literal-values-only path
/// (`load_view`), `Some` for the full styled path (`load_styled_view`).
/// `base_dir` (M22 Phase 2, §16.1) is the same real base directory
/// `include:` resolution already uses (`include::parse_view_with_
/// includes`) -- threaded here too so a real `kind: Image` `image.
/// src:` resolves against the identical directory. M49 Phase 3:
/// `default_theme`/`custom_theme` are resolved via `resolve_style_
/// layered` instead of the plain single-sheet `resolve_style` --
/// each present layer entirely supersedes the layer below it,
/// regardless of any layer's own internal selector specificity
/// (`resolve_style_layered`'s own doc comment, `cascade.rs`).
#[allow(clippy::too_many_arguments)]
pub fn build_tree(
    tree: &mut Tree,
    spec: &WidgetSpec,
    default_theme: Option<&Stylesheet>,
    custom_theme: Option<&Stylesheet>,
    sheet: Option<&Stylesheet>,
    scheme: Option<&ColorScheme>,
    base_dir: Option<&std::path::Path>,
) -> Result<NodeId, SpecError> {
    let resolved_style = resolve_style_layered(spec, default_theme, custom_theme, sheet);
    let layout_style = layout_style(&resolved_style);
    let (kind, paint) = node_kind_and_paint(spec, &resolved_style, scheme, base_dir)?;
    let id = tree.insert(kind, layout_style, paint);

    for child_spec in &spec.children {
        let child_id = build_tree(
            tree,
            child_spec,
            default_theme,
            custom_theme,
            sheet,
            scheme,
            base_dir,
        )?;
        tree.add_child(id, child_id);
    }

    Ok(id)
}

/// §16.4's own text: "a change that only touches styling patches
/// `PaintProperties`/`layout_style` directly." Recomputes `spec`'s kind/
/// paint/layout the same way `build_tree` does for a brand-new node,
/// then overwrites an *existing* node's fields in place -- `id` itself,
/// `parent`, `children`, `access`, and `interaction` are left
/// completely untouched, which is what lets focus and any in-flight
/// `ActiveAnimation` on those untouched fields survive a reload
/// (§16.4's own claim). Called only for a node `engine_spec::reconcile`
/// has already determined actually changed; an unchanged node is never
/// patched at all, so its `PaintProperties` (mid-animation or not) is
/// never touched in the first place.
#[allow(clippy::too_many_arguments)]
pub(crate) fn patch_node(
    tree: &mut Tree,
    id: NodeId,
    spec: &WidgetSpec,
    default_theme: Option<&Stylesheet>,
    custom_theme: Option<&Stylesheet>,
    sheet: Option<&Stylesheet>,
    scheme: Option<&ColorScheme>,
    base_dir: Option<&std::path::Path>,
) -> Result<(), SpecError> {
    let resolved_style = resolve_style_layered(spec, default_theme, custom_theme, sheet);
    let new_layout_style = layout_style(&resolved_style);
    let (kind, paint) = node_kind_and_paint(spec, &resolved_style, scheme, base_dir)?;

    let node = tree
        .get_mut(id)
        .expect("patch_node: NodeId must already exist in this Tree");
    node.kind = kind;
    node.paint = paint;
    node.layout_style = new_layout_style;
    Ok(())
}

/// M59 (§5, §16.3): resolves a `SpacingSpec` (or its absence) into a
/// real per-side `taffy::Rect` -- shared by `padding`/`margin` below,
/// generic over `T: FromLength` since `padding`'s own real taffy type
/// (`LengthPercentage`) and `margin`'s (`LengthPercentageAuto`) differ.
/// `None`/omitted per-side fields resolve to `0.0`, matching a real CSS
/// `padding: {top: 4px}` shorthand's own "unset sides are zero"
/// behavior.
fn spacing_to_rect<T: FromLength>(spec: Option<SpacingSpec>) -> TaffyRect<T> {
    let (top, right, bottom, left) = match spec {
        None => (0.0, 0.0, 0.0, 0.0),
        Some(SpacingSpec::Uniform(v)) => (v, v, v, v),
        Some(SpacingSpec::PerSide {
            top,
            right,
            bottom,
            left,
        }) => (top, right, bottom, left),
    };
    TaffyRect {
        left: length(left),
        right: length(right),
        top: length(top),
        bottom: length(bottom),
    }
}

/// M59 (§5, §16.3): `AlignItemsSpec`/`JustifyContentSpec` -> taffy's own
/// real `AlignItems`/`JustifyContent` associated consts -- a plain,
/// exhaustive match, not a lookup table, so a new variant on either
/// enum fails to compile here until this function is updated too.
fn align_items(spec: Option<AlignItemsSpec>) -> Option<AlignItems> {
    spec.map(|s| match s {
        AlignItemsSpec::Start => AlignItems::START,
        AlignItemsSpec::End => AlignItems::END,
        AlignItemsSpec::FlexStart => AlignItems::FLEX_START,
        AlignItemsSpec::FlexEnd => AlignItems::FLEX_END,
        AlignItemsSpec::Center => AlignItems::CENTER,
        AlignItemsSpec::Baseline => AlignItems::BASELINE,
        AlignItemsSpec::Stretch => AlignItems::STRETCH,
    })
}

fn justify_content(spec: Option<JustifyContentSpec>) -> Option<JustifyContent> {
    spec.map(|s| match s {
        JustifyContentSpec::Start => JustifyContent::START,
        JustifyContentSpec::End => JustifyContent::END,
        JustifyContentSpec::FlexStart => JustifyContent::FLEX_START,
        JustifyContentSpec::FlexEnd => JustifyContent::FLEX_END,
        JustifyContentSpec::Center => JustifyContent::CENTER,
        JustifyContentSpec::Stretch => JustifyContent::STRETCH,
        JustifyContentSpec::SpaceBetween => JustifyContent::SPACE_BETWEEN,
        JustifyContentSpec::SpaceAround => JustifyContent::SPACE_AROUND,
        JustifyContentSpec::SpaceEvenly => JustifyContent::SPACE_EVENLY,
    })
}

fn layout_style(style: &StyleSpec) -> Style {
    Style {
        display: taffy::Display::Flex,
        // `Horizontal`/`Vertical` are this crate's own real, deliberately
        // un-taffy-matching names (`spec.rs`'s own `FlexDirectionSpec`
        // doc comment has the full real reasoning) -- taffy's own
        // `FlexDirection` still uses `Row`/`Column` underneath, unchanged.
        flex_direction: match style.flex_direction {
            Some(FlexDirectionSpec::Horizontal) | None => taffy::FlexDirection::Row,
            Some(FlexDirectionSpec::Vertical) => taffy::FlexDirection::Column,
        },
        size: Size {
            width: style.width.map_or_else(auto, length),
            height: style.height.map_or_else(auto, length),
        },
        padding: spacing_to_rect(style.padding),
        margin: spacing_to_rect(style.margin),
        gap: style.gap.map_or_else(zero, |g| Size {
            width: length(g),
            height: length(g),
        }),
        flex_grow: style.flex_grow.unwrap_or(0.0),
        flex_shrink: style.flex_shrink.unwrap_or(1.0),
        flex_basis: style.flex_basis.map_or_else(auto, length),
        align_items: align_items(style.align_items),
        justify_content: justify_content(style.justify_content),
        ..Default::default()
    }
}

/// M61 (§16.3): resolves a `StyleSpec.corner_radius`/`elevation` value
/// (a literal or a named token) to a real `f64`, or `default` when
/// unset -- `spec`/`field` exist purely to build a real, specific
/// `SpecError::UnknownShapeToken` if the token name doesn't resolve,
/// matching `resolve_color`'s own identical "fail loudly with the
/// widget's own id and field name" contract for `background`.
fn resolve_shape_value(
    spec: &WidgetSpec,
    field: &'static str,
    value: Option<&ShapeOrElevationSpec>,
    default: f64,
    is_elevation: bool,
) -> Result<f64, SpecError> {
    let Some(value) = value else {
        return Ok(default);
    };
    value.resolve(is_elevation).ok_or_else(|| {
        let ShapeOrElevationSpec::TokenRef(token) = value else {
            unreachable!("ShapeOrElevationSpec::Literal always resolves")
        };
        SpecError::UnknownShapeToken {
            id: spec.id.clone(),
            field,
            token: token.clone(),
        }
    })
}

/// M62 Phase 4 (§7.1, §16.3): resolves a `TextSpec`'s real, final
/// `(font_family, font_weight, font_size, line_height)` -- if `role` is
/// given, it supplies each of the 4 as a real default (`engine_md3::
/// type_style_named`, an unrecognized name a real, clear `SpecError::
/// UnknownTypographyRole`); any of `text_spec`'s own literal fields, if
/// *also* given, override just that one field on top of the role's own
/// default. With no `role` at all, behaves exactly as before this
/// milestone: `font_weight` unset falls back to `400.0` (CSS/OpenType
/// "normal," the same real number `default_font_weight` used to supply
/// at parse time), `font_family`/`font_size` unset is a real, build-time
/// `SpecError::MissingField` (previously caught by serde's own
/// "required field" check instead -- a real, inherent consequence of
/// widening both to genuinely role-derivable `Option`s, not silently
/// different behavior for a case that used to succeed). `kind` is
/// threaded through purely so the resulting error names the real
/// `NodeKindSpec` variant that called this (`"Text"` vs. `"TextField"`),
/// matching `required_background`'s own identical parameter.
fn resolve_text_style(
    spec: &WidgetSpec,
    text_spec: &TextSpec,
    kind: &'static str,
) -> Result<(String, f32, f32, Option<f32>), SpecError> {
    let role_style = text_spec
        .typography_role
        .as_deref()
        .map(|role| {
            engine_md3::type_style_named(role).ok_or_else(|| SpecError::UnknownTypographyRole {
                id: spec.id.clone(),
                role: role.to_string(),
            })
        })
        .transpose()?;

    let font_family = text_spec
        .font_family
        .clone()
        .or_else(|| role_style.map(|s| s.font_family.to_string()))
        .ok_or_else(|| SpecError::MissingField {
            id: spec.id.clone(),
            kind,
            field: "text.font_family (or text.typography_role)",
        })?;
    let font_weight = text_spec
        .font_weight
        .or(role_style.map(|s| s.font_weight))
        .unwrap_or(400.0);
    let font_size = text_spec
        .font_size
        .or(role_style.map(|s| s.font_size))
        .ok_or_else(|| SpecError::MissingField {
            id: spec.id.clone(),
            kind,
            field: "text.font_size (or text.typography_role)",
        })?;
    let line_height = text_spec.line_height.or(role_style.map(|s| s.line_height));

    Ok((font_family, font_weight, font_size, line_height))
}

fn node_kind_and_paint(
    spec: &WidgetSpec,
    style: &StyleSpec,
    scheme: Option<&ColorScheme>,
    base_dir: Option<&std::path::Path>,
) -> Result<(NodeKind, PaintProperties), SpecError> {
    let corner_radius = resolve_shape_value(
        spec,
        "corner_radius",
        style.corner_radius.as_ref(),
        0.0,
        false,
    )?;
    let opacity = f64::from(style.opacity.unwrap_or(1.0));

    let (kind, mut paint) =
        node_kind_and_base_paint(spec, style, scheme, corner_radius, opacity, base_dir)?;

    // M48 (§5, §7): border is universal across every `NodeKind`, the
    // same real reason `corner_radius`/`opacity` are computed once
    // above rather than per-arm -- applied after the match instead of
    // threaded into every arm's own `PaintProperties::new(...)` call,
    // which takes no border params (`engine-core/src/node.rs`'s own
    // signature, unchanged by this milestone). `Animated::new` (not
    // `animate_to`) since this is the node's real starting value, the
    // identical "just set it" shape `PaintProperties::new` itself uses
    // for `background`/`corner_radius`, not a live-eased transition.
    if let Some(raw) = &style.border_color {
        paint.border_color = Animated::new(resolve_color(spec, "border_color", raw, scheme)?);
    }
    if let Some(border_width) = style.border_width {
        paint.border_width = Animated::new(f64::from(border_width));
    }
    // M49 Phase 2: the identical real "universal, applied once after
    // the match" shape border just established above -- `elevation`
    // was a real, existing `PaintProperties` field never reachable from
    // `StyleSpec` at all until this milestone.
    if style.elevation.is_some() {
        let elevation =
            resolve_shape_value(spec, "elevation", style.elevation.as_ref(), 0.0, true)?;
        paint.elevation = Animated::new(elevation);
    }

    // Tesserae M27: the identical "universal, applied once after the
    // match" shape border/elevation above already use -- `LoadingIndicator`
    // is the one real kind needing a `PaintProperties` field the match
    // itself has no other reason to touch. `add_loading_indicator`'s own
    // real body proves this step is required, not optional: without it,
    // `paint.shape` starts at `ShapeKey::empty()` and the very first real
    // tick morphs *from* nothing, a real, visible flash bug.
    if let NodeKind::LoadingIndicator(state) = &kind {
        paint.shape = Animated::new(state.shapes[0].clone());
    }

    Ok((kind, paint))
}

fn node_kind_and_base_paint(
    spec: &WidgetSpec,
    style: &StyleSpec,
    scheme: Option<&ColorScheme>,
    corner_radius: f64,
    opacity: f64,
    base_dir: Option<&std::path::Path>,
) -> Result<(NodeKind, PaintProperties), SpecError> {
    match &spec.kind {
        NodeKindSpec::Rect => {
            let background = required_background(spec, style, scheme, "Rect")?;
            Ok((
                NodeKind::Rect,
                PaintProperties::new(background, corner_radius, 0.0, opacity),
            ))
        }
        NodeKindSpec::Text => {
            let background = required_foreground(spec, style, scheme, "Text")?;
            let text_spec = spec.text.as_ref().ok_or_else(|| SpecError::MissingField {
                id: spec.id.clone(),
                kind: "Text",
                field: "text",
            })?;
            let (font_family, font_weight, font_size, line_height) =
                resolve_text_style(spec, text_spec, "Text")?;
            Ok((
                NodeKind::Text(TextState {
                    content: text_spec.content.clone(),
                    font_family,
                    font_weight,
                    font_size,
                    align: TextAlign::Start,
                    line_height,
                }),
                PaintProperties::new(background, corner_radius, 0.0, opacity),
            ))
        }
        NodeKindSpec::Container => {
            // Unlike Rect/Text, a Container commonly paints nothing --
            // build_tree_scene never reads its background (only
            // Rect/Text do) -- so an unset color defaults to fully
            // transparent rather than erroring, matching every
            // Container this codebase has built by hand so far
            // (rect_window.rs, layout_tree.rs).
            let background = match &style.background {
                Some(raw) => resolve_color(spec, "background", raw, scheme)?,
                None => Color::from_rgba8(0, 0, 0, 0),
            };
            Ok((
                NodeKind::Container,
                PaintProperties::new(background, corner_radius, 0.0, opacity),
            ))
        }
        // M14 Phase 3 (§5, §7.3): `checked`/`value` are the widget's
        // own real initial state -- immediately overwritten by a real
        // one-way `bindings: {checked: ...}`/`{value: ...}` resolution
        // if one exists, the same way a static `style.opacity` already
        // is by a bound one (`View::_attach`'s own established order:
        // build the tree first, resolve bindings after).
        NodeKindSpec::Checkbox => {
            let background = required_background(spec, style, scheme, "Checkbox")?;
            Ok((
                NodeKind::Checkbox(CheckboxState::new(checkbox_checked(spec)?)),
                PaintProperties::new(background, corner_radius, 0.0, opacity),
            ))
        }
        NodeKindSpec::Slider => {
            let background = required_background(spec, style, scheme, "Slider")?;
            Ok((
                NodeKind::Slider(SliderState::new(spec.value)),
                PaintProperties::new(background, corner_radius, 0.0, opacity),
            ))
        }
        // M15 Phase 3 (§16.7): reuses `spec.text` verbatim -- the same
        // real `TextSpec` block `kind: Text` already requires, since
        // `TextFieldState`'s own font/content fields are byte-for-byte
        // identical (see `TextSpec`'s own doc comment). `checked: bool`/
        // `value: f64` each got their own dedicated `WidgetSpec` field
        // when Checkbox/Slider needed one (M14 Phase 3) because neither
        // already had a matching sibling block to reuse -- `TextField`
        // does, so it uses that instead of adding a third.
        NodeKindSpec::TextField => {
            let background = required_background(spec, style, scheme, "TextField")?;
            let text_spec = spec.text.as_ref().ok_or_else(|| SpecError::MissingField {
                id: spec.id.clone(),
                kind: "TextField",
                field: "text",
            })?;
            let (font_family, font_weight, font_size, _line_height) =
                resolve_text_style(spec, text_spec, "TextField")?;
            Ok((
                NodeKind::TextField(TextFieldState::new(
                    text_spec.content.clone(),
                    font_family,
                    font_weight,
                    font_size,
                )),
                PaintProperties::new(background, corner_radius, 0.0, opacity),
            ))
        }
        // M22 Phase 2 (§16.1, §5): mirrors `Window.add_image`'s own
        // real decode-then-build shape exactly -- `image.src:` resolved
        // and confined against `base_dir` first (`resolve_image_src`'s
        // own doc comment), then read/decoded via the `image` crate.
        // `PaintProperties`'s own `background` stays hardcoded
        // transparent, the identical real reason `add_image` itself
        // takes no `background` param (this module's `ImageSpec` has
        // none either, deliberately, for the same reason) -- `style.
        // background`, if an author sets one anyway, is silently
        // unused here, matching `Canvas`'s own real precedent (no
        // declarative `kind: Canvas` exists to compare against, but
        // `add_canvas`'s own hardcoded transparent fill is the
        // identical imperative-API precedent this mirrors).
        NodeKindSpec::Image => {
            let image_spec = spec.image.as_ref().ok_or_else(|| SpecError::MissingField {
                id: spec.id.clone(),
                kind: "Image",
                field: "image",
            })?;
            let mut image_state = match &image_spec.src {
                Some(src) => {
                    let base_dir = base_dir
                        .ok_or_else(|| SpecError::ImageSrcNoBaseDir { path: src.clone() })?;
                    let resolved_path = resolve_image_src(base_dir, src)?;
                    let decoded = image::open(&resolved_path)
                        .map_err(|source| SpecError::ImageDecodeFailed {
                            path: resolved_path.clone(),
                            source,
                        })?
                        .to_rgba8();
                    let (img_width, img_height) = decoded.dimensions();
                    engine_core::ImageState::new(peniko::ImageData {
                        data: peniko::Blob::from(decoded.into_raw()),
                        format: peniko::ImageFormat::Rgba8,
                        alpha_type: peniko::ImageAlphaType::Alpha,
                        width: img_width,
                        height: img_height,
                    })
                }
                // tre issue #2: no `src:` at all -- the same synthetic
                // 1x1 transparent placeholder `Window.add_video`'s own
                // real default builds, letting a declarative fragment
                // express that shape without a backing file.
                None => engine_core::ImageState::blank(),
            };
            image_state.content_fit = match image_spec.fit {
                ContentFitSpec::Cover => engine_core::ContentFit::Cover,
                ContentFitSpec::Contain => engine_core::ContentFit::Contain,
                ContentFitSpec::Fill => engine_core::ContentFit::Fill,
            };
            Ok((
                NodeKind::Image(image_state),
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), corner_radius, 0.0, opacity),
            ))
        }
        NodeKindSpec::Icon => {
            let icon_spec = spec.icon.as_ref().ok_or_else(|| SpecError::MissingField {
                id: spec.id.clone(),
                kind: "Icon",
                field: "icon",
            })?;
            let svg_data = engine_md3::icons::path_for(&icon_spec.name).ok_or_else(|| {
                SpecError::UnknownIcon {
                    id: spec.id.clone(),
                    name: icon_spec.name.clone(),
                }
            })?;
            let path = peniko::kurbo::BezPath::from_svg(svg_data).unwrap_or_else(|e| {
                panic!(
                    "engine_md3::icons's own curated path data for {:?} must parse: {e}",
                    icon_spec.name
                )
            });
            let tint = required_foreground(spec, style, scheme, "Icon")?;
            Ok((
                NodeKind::Icon(IconState::new(path, tint)),
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, opacity),
            ))
        }
        // Tesserae M27: identical shape to `Text` above -- `Link`'s own
        // `NodeKind::Link(TextState { .. })` is byte-for-byte the same
        // struct, just a different outer `NodeKind` variant. Deliberate
        // divergence from `add_link`'s own imperative behavior, stated
        // here rather than left implicit: the imperative factory always
        // hardcodes `body_large` typography with no override at all,
        // but every other declarative text-bearing kind already lets an
        // author choose `text.role`/explicit font fields via the shared
        // `resolve_text_style` -- staying consistent with that existing
        // contract (a real widening of capability, not an oversight)
        // rather than special-casing `Link` to forbid what `Text`/
        // `TextField` both already allow.
        NodeKindSpec::Link => {
            let background = required_foreground(spec, style, scheme, "Link")?;
            let text_spec = spec.text.as_ref().ok_or_else(|| SpecError::MissingField {
                id: spec.id.clone(),
                kind: "Link",
                field: "text",
            })?;
            let (font_family, font_weight, font_size, line_height) =
                resolve_text_style(spec, text_spec, "Link")?;
            Ok((
                NodeKind::Link(TextState {
                    content: text_spec.content.clone(),
                    font_family,
                    font_weight,
                    font_size,
                    align: TextAlign::Start,
                    line_height,
                }),
                PaintProperties::new(background, corner_radius, 0.0, opacity),
            ))
        }
        // Tesserae M27: `spec.checked` doubles as this kind's own
        // initial `selected` -- both real values are a plain `bool`,
        // the identical value `RadioButtonState::new` takes. Its entire
        // real visual lives in these 2 internal tint fields (confirmed
        // via `add_radio_button`'s own real body: hardcoded `TRANSPARENT`
        // `PaintProperties.background`), not `style.background` at all.
        NodeKindSpec::RadioButton => {
            let mut state = RadioButtonState::new(md3_selected(spec, "RadioButton")?);
            state.unselected_tint = resolve_role_or_fallback(scheme, "outline", BASELINE_OUTLINE);
            state.selected_tint = resolve_role_or_fallback(scheme, "primary", BASELINE_PRIMARY);
            Ok((
                NodeKind::RadioButton(state),
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, opacity),
            ))
        }
        // Tesserae M27: `spec.checked` doubles as this kind's own
        // initial `on`, the identical real reuse `RadioButton` above
        // makes. 5 internal tint fields, confirmed via `add_switch`'s
        // own real body -- the most tint fields of any of these 7.
        NodeKindSpec::Switch => {
            let mut state = SwitchState::new(md3_selected(spec, "Switch")?);
            state.track_off_tint = resolve_role_or_fallback(
                scheme,
                "surface_container_highest",
                BASELINE_SURFACE_CONTAINER_HIGHEST,
            );
            state.track_on_tint = resolve_role_or_fallback(scheme, "primary", BASELINE_PRIMARY);
            state.track_outline_tint =
                resolve_role_or_fallback(scheme, "outline", BASELINE_OUTLINE);
            state.handle_off_tint = resolve_role_or_fallback(scheme, "outline", BASELINE_OUTLINE);
            state.handle_on_tint =
                resolve_role_or_fallback(scheme, "on_primary", BASELINE_ON_PRIMARY);
            Ok((
                NodeKind::Switch(state),
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, opacity),
            ))
        }
        // Tesserae M27: `spec.value` doubles as this kind's own initial
        // progress fraction -- the identical real `f64` value `Slider`
        // already reuses this same field for. Simplest of the 5
        // tint-heavy kinds: exactly one internal tint field.
        NodeKindSpec::CircularProgress => {
            let mut state = CircularProgressState::new(spec.value);
            state.indicator_tint = resolve_role_or_fallback(scheme, "primary", BASELINE_PRIMARY);
            Ok((
                NodeKind::CircularProgress(state),
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, opacity),
            ))
        }
        // Tesserae M27: `spec.value` doubles as this kind's own initial
        // progress fraction, the identical real reuse `CircularProgress`
        // above makes.
        NodeKindSpec::LinearProgress => {
            let mut state = LinearProgressState::new(spec.value);
            state.track_tint = resolve_role_or_fallback(
                scheme,
                "surface_container_highest",
                BASELINE_SURFACE_CONTAINER_HIGHEST,
            );
            state.indicator_tint = resolve_role_or_fallback(scheme, "primary", BASELINE_PRIMARY);
            Ok((
                NodeKind::LinearProgress(state),
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, opacity),
            ))
        }
        // Tesserae M27: no `WidgetSpec` field of its own -- `style.
        // width`/`height` (already resolved into this function's own
        // `Style`, separately from this match) feed `LoadingIndicatorState
        // ::new` directly at the real call site below, the identical
        // real shape `add_loading_indicator`'s own `size` param already
        // has. Its "background" is really the glyph tint (`required_
        // background`, matching `Icon`'s own identical real semantic),
        // not a second, parallel color field. The one required extra
        // step `add_loading_indicator`'s own real body already proves
        // necessary: `paint.shape` must be seeded to the real first
        // shape at construction, or the very first real tick visibly
        // flashes (confirmed real bug class, not hypothetical) --
        // applied via `node_kind_and_paint`'s own real post-match hook
        // below, the same "universal, applied once after the match"
        // shape border/elevation already use, since this is the one
        // real kind needing a `PaintProperties` field the match itself
        // has no other reason to touch.
        NodeKindSpec::LoadingIndicator => {
            let tint = required_foreground(spec, style, scheme, "LoadingIndicator")?;
            let width = style.width.ok_or_else(|| SpecError::MissingField {
                id: spec.id.clone(),
                kind: "LoadingIndicator",
                field: "style.width",
            })?;
            let height = style.height.ok_or_else(|| SpecError::MissingField {
                id: spec.id.clone(),
                kind: "LoadingIndicator",
                field: "style.height",
            })?;
            Ok((
                NodeKind::LoadingIndicator(LoadingIndicatorState::new(
                    f64::from(width),
                    f64::from(height),
                )),
                PaintProperties::new(tint, 0.0, 0.0, opacity),
            ))
        }
        // Tesserae M27: the one real exception among these 7 -- needs
        // the new `spec.hour`/`spec.minute` fields, not a reuse of an
        // existing one (`TimePickerDialState::new` itself clamps both,
        // matching `add_time_picker_dial`'s own real doc comment, so no
        // extra validation happens here).
        NodeKindSpec::TimePickerDial => {
            let mut state = TimePickerDialState::new(spec.hour, spec.minute);
            state.face_tint = resolve_role_or_fallback(
                scheme,
                "surface_container_highest",
                BASELINE_SURFACE_CONTAINER_HIGHEST,
            );
            state.hand_tint = resolve_role_or_fallback(scheme, "primary", BASELINE_PRIMARY);
            Ok((
                NodeKind::TimePickerDial(state),
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, opacity),
            ))
        }
    }
}

fn required_background(
    spec: &WidgetSpec,
    style: &StyleSpec,
    scheme: Option<&ColorScheme>,
    kind: &'static str,
) -> Result<Color, SpecError> {
    match &style.background {
        Some(raw) => resolve_color(spec, "background", raw, scheme),
        None => Err(SpecError::MissingField {
            id: spec.id.clone(),
            kind,
            field: "style.background",
        }),
    }
}

/// M90: a `Checkbox`'s initial `checked`; `selected:` on a `Checkbox` is
/// the wrong field and says so.
fn checkbox_checked(spec: &WidgetSpec) -> Result<bool, SpecError> {
    if spec.selected.is_some() {
        return Err(SpecError::WrongStateField {
            id: spec.id.clone(),
            kind: "Checkbox",
            given: "selected",
            expected: "checked",
        });
    }
    Ok(spec.checked.unwrap_or(false))
}

/// M90: a `Switch`/`RadioButton`'s initial `selected`; `checked:` on
/// either is the pre-M90 spelling and says so.
fn md3_selected(spec: &WidgetSpec, kind: &'static str) -> Result<bool, SpecError> {
    if spec.checked.is_some() {
        return Err(SpecError::WrongStateField {
            id: spec.id.clone(),
            kind,
            given: "checked",
            expected: "selected",
        });
    }
    Ok(spec.selected.unwrap_or(false))
}

/// M90: the glyph/text color of `Text`/`Link`/`Icon`/`LoadingIndicator`
/// -- `style.foreground`, required. These kinds have no fill: an inline
/// `style.background` on the widget itself is the pre-M90 spelling and
/// fails with a hint naming `foreground`. A `background` reaching them
/// only through the cascade (e.g. a selector-less baseline stylesheet
/// rule meant for every widget) is ignored rather than an error.
fn required_foreground(
    spec: &WidgetSpec,
    style: &StyleSpec,
    scheme: Option<&ColorScheme>,
    kind: &'static str,
) -> Result<Color, SpecError> {
    if spec.style.background.is_some() {
        return Err(SpecError::NotAFill {
            id: spec.id.clone(),
            kind,
        });
    }
    match &style.foreground {
        Some(raw) => resolve_color(spec, "foreground", raw, scheme),
        None => Err(SpecError::MissingField {
            id: spec.id.clone(),
            kind,
            field: "style.foreground",
        }),
    }
}

/// Resolves one `style.background`-shaped string, per §16.1: `engine-md3`
/// resolves MD3 token names (`background: surface`) against the active
/// color scheme (§7.1); anything that isn't a recognized role name
/// falls back to literal color parsing (`background: "#6750A4"`) --
/// tried in that order so a token name always wins over a same-named
/// coincidental CSS color, and a literal color still works with no
/// scheme at all (§14 step 5's original path, still exercised by
/// `load_view`).
/// Tesserae M27: the real published MD3 baseline seed-color tokens
/// `engine-py::window_factory.rs`'s own `Md3Baseline` already hardcodes
/// (identical values, confirmed by direct read before copying) -- the
/// fallback a `RadioButton`/`Switch`/progress-indicator/`TimePickerDial`
/// falls back to when no theme is set at all, since these 5 kinds carry
/// their entire real visual in internal tint fields `required_
/// background`'s own single `style.background` can't reach. Not shared
/// as a common cross-crate constant with `engine-py`'s own copy --
/// `engine-py` depends on this crate, not the other way around, and a
/// real shared-constants refactor is bigger scope than this milestone's
/// own stated "additive only" shape.
const BASELINE_PRIMARY: Color = Color::from_rgba8(0x67, 0x50, 0xA4, 0xFF);
const BASELINE_ON_PRIMARY: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF);
const BASELINE_OUTLINE: Color = Color::from_rgba8(0x79, 0x74, 0x7E, 0xFF);
const BASELINE_SURFACE_CONTAINER_HIGHEST: Color = Color::from_rgba8(0xE6, 0xE0, 0xE9, 0xFF);

/// Tesserae M27: the declarative-build-time sibling of `window_factory.
/// rs`'s own `let role = |name, fallback| if theme.is_set() { theme.
/// role(name).unwrap_or(fallback) } else { fallback };` closure, reused
/// identically at all 5 of this milestone's own new tint-heavy call
/// sites instead of duplicating that closure inline 5 times. `scheme`
/// is `None` outright (not "set but empty") exactly when no theme was
/// given to `View::new`/`Reconciler::load`, the identical real
/// condition `theme.is_set()` gates on imperatively -- `role_name` is
/// always a fixed, code-chosen string here, never user input, so unlike
/// `resolve_color` this is infallible.
fn resolve_role_or_fallback(
    scheme: Option<&ColorScheme>,
    role_name: &str,
    fallback: Color,
) -> Color {
    scheme.and_then(|s| s.role(role_name)).unwrap_or(fallback)
}

fn resolve_color(
    spec: &WidgetSpec,
    field: &'static str,
    raw: &str,
    scheme: Option<&ColorScheme>,
) -> Result<Color, SpecError> {
    if let Some(scheme) = scheme
        && let Some(color) = scheme.role(raw)
    {
        return Ok(color);
    }

    peniko::color::parse_color(raw)
        .map(|dynamic| dynamic.to_alpha_color::<peniko::color::Srgb>())
        .map_err(|source| SpecError::InvalidColor {
            id: spec.id.clone(),
            field,
            value: raw.to_string(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::ShapeKey;
    use taffy::prelude::AvailableSpace;

    const VIEW: &str = r##"
id: root
kind: Container
style: {flex_direction: horizontal, padding: 10, gap: 5, width: 220, height: 100}
children:
  - id: swatch
    kind: Rect
    style: {width: 100, height: 80, background: "#6750A4", corner_radius: 8}
  - id: label
    kind: Text
    text: {content: "Hi", font_family: Roboto, font_size: 16}
    style: {width: 90, height: 30, foreground: white}
"##;

    #[test]
    fn load_view_json_builds_the_same_real_tree_load_view_does() {
        // tre issue #3, Part A (Tier 2): the identical real construction
        // `load_view_builds_a_real_tree_matching_the_spec` below proves
        // for YAML, proven here for JSON.
        let json = r##"{
            "id": "root",
            "kind": "Container",
            "style": {"flex_direction": "horizontal", "padding": 10, "gap": 5, "width": 220, "height": 100},
            "children": [
                {
                    "id": "swatch",
                    "kind": "Rect",
                    "style": {"width": 100, "height": 80, "background": "#6750A4", "corner_radius": 8}
                }
            ]
        }"##;
        let mut tree = Tree::new();
        let root = load_view_json(&mut tree, json).expect("valid view JSON must build");
        let root_node = tree.get(root).expect("root must exist");
        assert!(matches!(root_node.kind, NodeKind::Container));
        assert_eq!(root_node.children.len(), 1);
        let swatch_node = tree.get(root_node.children[0]).unwrap();
        assert_eq!(swatch_node.paint.corner_radius.current, 8.0);
        assert_eq!(
            swatch_node.paint.background.current,
            Color::from_rgba8(0x67, 0x50, 0xA4, 0xFF)
        );
    }

    #[test]
    fn load_view_builds_a_real_tree_matching_the_spec() {
        let mut tree = Tree::new();
        let root = load_view(&mut tree, VIEW).expect("valid view must build");

        let root_node = tree.get(root).expect("root must exist");
        assert!(matches!(root_node.kind, NodeKind::Container));
        assert_eq!(root_node.children.len(), 2);

        let swatch = root_node.children[0];
        let swatch_node = tree.get(swatch).unwrap();
        assert!(matches!(swatch_node.kind, NodeKind::Rect));
        assert_eq!(swatch_node.paint.corner_radius.current, 8.0);
        assert_eq!(
            swatch_node.paint.background.current,
            Color::from_rgba8(0x67, 0x50, 0xA4, 0xFF)
        );

        let label = root_node.children[1];
        let label_node = tree.get(label).unwrap();
        let NodeKind::Text(text) = &label_node.kind else {
            panic!("expected a Text node");
        };
        assert_eq!(text.content, "Hi");
        assert_eq!(text.font_size, 16.0);

        // Positions actually come from taffy, not just "a Tree with the
        // right node count" -- the same discipline as step 3's own
        // deterministic layout test.
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(220.0),
                height: AvailableSpace::Definite(100.0),
            },
        );
        let swatch_layout = tree.layout(swatch);
        assert_eq!(swatch_layout.location.x, 10.0); // root's own padding
        let label_layout = tree.layout(label);
        assert_eq!(label_layout.location.x, 10.0 + 100.0 + 5.0); // padding + swatch width + gap
    }

    /// M14 Phase 3 (§5, §7.3): `NodeKindSpec::Checkbox`/`Slider` must
    /// build real `NodeKind::Checkbox`/`Slider` nodes seeded from the
    /// widget's own `checked`/`value` fields -- the same real-tree proof
    /// `load_view_builds_a_real_tree_matching_the_spec` already gives
    /// `Container`/`Rect`/`Text`, extended to the two kinds this phase
    /// makes declarable in `view.yaml` for the first time.
    #[test]
    fn load_view_builds_real_checkbox_and_slider_nodes_seeded_from_their_spec() {
        let yaml = r##"
id: root
kind: Container
children:
  - id: agree
    kind: Checkbox
    checked: true
    style: {width: 24, height: 24, background: "#6750A4"}
  - id: volume
    kind: Slider
    value: 0.75
    style: {width: 180, height: 32, background: "#03DAC6"}
"##;
        let mut tree = Tree::new();
        let root = load_view(&mut tree, yaml).expect("valid Checkbox/Slider view must build");
        let root_node = tree.get(root).unwrap();

        let checkbox = tree.get(root_node.children[0]).unwrap();
        let NodeKind::Checkbox(state) = &checkbox.kind else {
            panic!("expected a Checkbox node");
        };
        assert!(
            state.checked,
            "checked: true in the spec must seed real CheckboxState.checked"
        );

        let slider = tree.get(root_node.children[1]).unwrap();
        let NodeKind::Slider(state) = &slider.kind else {
            panic!("expected a Slider node");
        };
        assert_eq!(
            state.thumb_position.current, 0.75,
            "value: 0.75 in the spec must seed real SliderState.thumb_position"
        );
    }

    #[test]
    fn load_view_builds_a_real_text_field_node_seeded_from_its_own_text_block() {
        let yaml = r##"
id: username
kind: TextField
text: {content: "jane", font_family: Roboto, font_size: 16}
style: {width: 200, height: 32, background: "#EEEEEE"}
"##;
        let mut tree = Tree::new();
        let root = load_view(&mut tree, yaml).expect("valid TextField view must build");
        let node = tree.get(root).unwrap();
        let NodeKind::TextField(state) = &node.kind else {
            panic!("expected a TextField node");
        };
        assert_eq!(state.content, "jane");
        assert_eq!(state.font_family, "Roboto");
        assert_eq!(state.font_size, 16.0);
        assert_eq!(
            state.cursor, 4,
            "the built TextFieldState must seed its cursor at content's own real end, \
             the same TextFieldState::new contract the imperative API uses"
        );
    }

    #[test]
    fn text_field_without_background_is_a_clear_error_not_a_default() {
        let yaml = r#"
id: username
kind: TextField
text: {content: "jane", font_family: Roboto, font_size: 16}
style: {width: 200, height: 32}
"#;
        let mut tree = Tree::new();
        let err = load_view(&mut tree, yaml).expect_err("a colorless TextField must fail to build");
        assert!(matches!(
            err,
            SpecError::MissingField { id, kind: "TextField", field: "style.background" } if id == "username"
        ));
    }

    #[test]
    fn text_field_without_a_text_block_is_a_clear_error_not_a_default() {
        let yaml = r#"
id: username
kind: TextField
style: {width: 200, height: 32, background: "white"}
"#;
        let mut tree = Tree::new();
        let err =
            load_view(&mut tree, yaml).expect_err("a TextField with no text: block must fail");
        assert!(matches!(
            err,
            SpecError::MissingField { id, kind: "TextField", field: "text" } if id == "username"
        ));
    }

    #[test]
    fn checkbox_without_background_is_a_clear_error_not_a_default() {
        let yaml = r#"
id: root
kind: Checkbox
style: {width: 24, height: 24}
"#;
        let mut tree = Tree::new();
        let err = load_view(&mut tree, yaml).expect_err("a colorless Checkbox must fail to build");
        assert!(matches!(
            err,
            SpecError::MissingField { id, kind: "Checkbox", field: "style.background" } if id == "root"
        ));
    }

    #[test]
    fn rect_without_background_is_a_clear_error_not_a_default() {
        let yaml = r#"
id: root
kind: Rect
style: {width: 10, height: 10}
"#;
        let mut tree = Tree::new();
        let err = load_view(&mut tree, yaml).expect_err("a colorless Rect must fail to build");
        assert!(matches!(
            err,
            SpecError::MissingField { id, kind: "Rect", field: "style.background" } if id == "root"
        ));
    }

    #[test]
    fn invalid_color_names_the_offending_widget_and_value() {
        let yaml = r#"
id: bad-swatch
kind: Rect
style: {width: 10, height: 10, background: "not-a-color"}
"#;
        let mut tree = Tree::new();
        let err = load_view(&mut tree, yaml).expect_err("an invalid color string must be rejected");
        let SpecError::InvalidColor { id, value, .. } = err else {
            panic!("expected InvalidColor, got {err:?}");
        };
        assert_eq!(id, "bad-swatch");
        assert_eq!(value, "not-a-color");
    }

    #[test]
    fn container_without_background_defaults_to_transparent() {
        let yaml = r#"
id: root
kind: Container
"#;
        let mut tree = Tree::new();
        let root = load_view(&mut tree, yaml).expect("a bare Container must build");
        let node = tree.get(root).unwrap();
        assert_eq!(node.paint.background.current, Color::from_rgba8(0, 0, 0, 0));
    }

    /// The actual §16.3 end-to-end claim: an MD3 token name in a real
    /// `view.yaml`, resolved through a real `DynamicTheme`, must produce
    /// the exact same color that theme's own `ColorScheme.primary` field
    /// holds -- not merely "some color came out."
    #[test]
    fn load_styled_view_resolves_an_md3_token_name_to_the_real_scheme_color() {
        let theme = engine_md3::DynamicTheme::from_seed(Color::from_rgba8(0x67, 0x50, 0xA4, 0xFF));
        let sheet = crate::cascade::parse_stylesheet("styles: []\n").unwrap();
        let yaml = r#"
id: swatch
kind: Rect
style: {width: 10, height: 10, background: primary}
"#;
        let mut tree = Tree::new();
        let root = load_styled_view(&mut tree, yaml, None, None, &sheet, &theme.light, None)
            .expect("a token-named background must resolve against the given scheme");
        let node = tree.get(root).unwrap();
        assert_eq!(node.paint.background.current, theme.light.primary);
    }

    /// A literal hex color must still work even when a real scheme is
    /// active -- token resolution is tried first (per `resolve_color`'s
    /// own doc comment) but must fall through cleanly, not treat every
    /// styled load as token-only.
    #[test]
    fn load_styled_view_still_accepts_literal_colors_alongside_a_scheme() {
        let theme = engine_md3::DynamicTheme::from_seed(Color::from_rgba8(0x67, 0x50, 0xA4, 0xFF));
        let sheet = crate::cascade::parse_stylesheet("styles: []\n").unwrap();
        let yaml = r##"
id: swatch
kind: Rect
style: {width: 10, height: 10, background: "#112233"}
"##;
        let mut tree = Tree::new();
        let root = load_styled_view(&mut tree, yaml, None, None, &sheet, &theme.light, None)
            .expect("a literal color must still parse with a scheme active");
        let node = tree.get(root).unwrap();
        assert_eq!(
            node.paint.background.current,
            Color::from_rgba8(0x11, 0x22, 0x33, 0xFF)
        );
    }

    /// Without a scheme at all (`load_view`), a token name is just an
    /// arbitrary string that isn't a valid CSS color -- it must fail
    /// loudly, the same "fail at the boundary" discipline as every other
    /// `InvalidColor` case, not silently resolve to black or transparent.
    #[test]
    fn md3_token_name_without_a_scheme_is_a_clear_error_not_a_silent_default() {
        let yaml = r#"
id: swatch
kind: Rect
style: {width: 10, height: 10, background: primary}
"#;
        let mut tree = Tree::new();
        let err = load_view(&mut tree, yaml)
            .expect_err("a token name with no scheme to resolve it against must fail");
        assert!(matches!(err, SpecError::InvalidColor { .. }));
    }

    /// M61 (§16.3): the actual end-to-end claim for shape tokens,
    /// mirroring `load_styled_view_resolves_an_md3_token_name_to_the_
    /// real_scheme_color` above -- a real `corner_radius: small` string
    /// in a real `view.yaml`, resolved through a real `load_view` call,
    /// must produce the exact same value `engine_md3::named`
    /// itself returns for `"small"`, not merely "some number came out."
    #[test]
    fn corner_radius_token_name_resolves_to_the_real_named_constant() {
        let yaml = r#"
id: swatch
kind: Rect
style: {width: 10, height: 10, background: red, corner_radius: small}
"#;
        let mut tree = Tree::new();
        let root = load_view(&mut tree, yaml).expect("a token-named corner_radius must resolve");
        let node = tree.get(root).unwrap();
        assert_eq!(
            node.paint.corner_radius.current,
            engine_md3::named("small").unwrap()
        );
    }

    /// The `elevation` sibling of the test above -- a real `level_3`
    /// string must resolve to `engine_md3::elevation_named`'s own
    /// real constant.
    #[test]
    fn elevation_token_name_resolves_to_the_real_named_constant() {
        let yaml = r#"
id: swatch
kind: Rect
style: {width: 10, height: 10, background: red, elevation: level_3}
"#;
        let mut tree = Tree::new();
        let root = load_view(&mut tree, yaml).expect("a token-named elevation must resolve");
        let node = tree.get(root).unwrap();
        assert_eq!(
            node.paint.elevation.current,
            engine_md3::elevation_named("level_3").unwrap()
        );
    }

    /// An unrecognized shape token must be a real, clear
    /// `SpecError::UnknownShapeToken` naming the widget's own id, the
    /// field it appeared on, and the bad token itself -- not a silent
    /// fallback to `0.0`, the same "fail loudly at the boundary"
    /// discipline `invalid_color_names_the_offending_widget_and_value`
    /// above already establishes for `background`.
    #[test]
    fn unknown_corner_radius_token_names_the_offending_widget_field_and_token() {
        let yaml = r#"
id: bad-shape
kind: Rect
style: {width: 10, height: 10, background: red, corner_radius: smol}
"#;
        let mut tree = Tree::new();
        let err =
            load_view(&mut tree, yaml).expect_err("an unrecognized corner_radius token must fail");
        let SpecError::UnknownShapeToken { id, field, token } = err else {
            panic!("expected UnknownShapeToken, got {err:?}");
        };
        assert_eq!(id, "bad-shape");
        assert_eq!(field, "corner_radius");
        assert_eq!(token, "smol");
    }

    /// The `elevation` sibling of the test above.
    #[test]
    fn unknown_elevation_token_names_the_offending_widget_field_and_token() {
        let yaml = r#"
id: bad-elevation
kind: Rect
style: {width: 10, height: 10, background: red, elevation: level_9}
"#;
        let mut tree = Tree::new();
        let err =
            load_view(&mut tree, yaml).expect_err("an unrecognized elevation token must fail");
        let SpecError::UnknownShapeToken { id, field, token } = err else {
            panic!("expected UnknownShapeToken, got {err:?}");
        };
        assert_eq!(id, "bad-elevation");
        assert_eq!(field, "elevation");
        assert_eq!(token, "level_9");
    }

    /// A plain numeric literal must still work exactly as before M61 --
    /// token resolution is only attempted for a bare YAML string, never
    /// forced onto a widget that just wants a fixed number.
    #[test]
    fn corner_radius_literal_still_works_alongside_token_support() {
        let yaml = r#"
id: swatch
kind: Rect
style: {width: 10, height: 10, background: red, corner_radius: 6.0}
"#;
        let mut tree = Tree::new();
        let root = load_view(&mut tree, yaml).expect("a literal corner_radius must still parse");
        let node = tree.get(root).unwrap();
        assert_eq!(node.paint.corner_radius.current, 6.0);
    }

    /// M62 Phase 1 (§7.1, §16.3): `text: {line_height: ...}`'s own real
    /// end-to-end declarative round-trip -- a real number in the YAML
    /// must reach `TextState.line_height` as `Some(...)`, the exact
    /// value that later drives a real, wider per-line advance
    /// (`engine-render::text`'s own `a_larger_line_height_genuinely_
    /// widens_the_real_per_line_advance` proves that half; this test
    /// proves the parse-to-`TextState` half).
    #[test]
    fn text_line_height_reaches_the_real_text_state() {
        let yaml = r#"
id: label
kind: Text
text: {content: "Hi", font_family: Roboto, font_size: 16, line_height: 1.5}
style: {width: 90, height: 30, foreground: white}
"#;
        let mut tree = Tree::new();
        let root = load_view(&mut tree, yaml).expect("a real line_height must parse");
        let node = tree.get(root).unwrap();
        let NodeKind::Text(text) = &node.kind else {
            panic!("expected a Text node");
        };
        assert_eq!(text.line_height, Some(1.5));
    }

    /// The pre-M62 implicit behavior -- omitting `line_height` entirely
    /// -- must still parse and produce `None`, not a manufactured
    /// default number.
    #[test]
    fn text_line_height_defaults_to_none_when_omitted() {
        let yaml = r#"
id: label
kind: Text
text: {content: "Hi", font_family: Roboto, font_size: 16}
style: {width: 90, height: 30, foreground: white}
"#;
        let mut tree = Tree::new();
        let root = load_view(&mut tree, yaml).expect("a view with no line_height must still parse");
        let node = tree.get(root).unwrap();
        let NodeKind::Text(text) = &node.kind else {
            panic!("expected a Text node");
        };
        assert_eq!(text.line_height, None);
    }

    /// M62 Phase 4 (§7.1, §16.3): the actual end-to-end claim for
    /// typography roles, mirroring `corner_radius_token_name_resolves_
    /// to_the_real_named_constant` above -- a real `role: title_medium`
    /// in a real `view.yaml`, resolved through a real `load_view` call,
    /// must produce the exact same `font_family`/`font_weight`/
    /// `font_size`/`line_height` `engine_md3::type_style_named` itself
    /// returns for `"title_medium"`.
    #[test]
    fn text_role_resolves_every_field_to_the_real_named_type_style() {
        let yaml = r#"
id: label
kind: Text
text: {content: "Hi", typography_role: title_medium}
style: {width: 90, height: 30, foreground: white}
"#;
        let mut tree = Tree::new();
        let root = load_view(&mut tree, yaml).expect("a real typography role must resolve");
        let node = tree.get(root).unwrap();
        let NodeKind::Text(text) = &node.kind else {
            panic!("expected a Text node");
        };
        let expected = engine_md3::type_style_named("title_medium").unwrap();
        assert_eq!(text.font_family, expected.font_family);
        assert_eq!(text.font_weight, expected.font_weight);
        assert_eq!(text.font_size, expected.font_size);
        assert_eq!(text.line_height, Some(expected.line_height));
    }

    /// A literal field alongside `role:` must override just that one
    /// field, the rest still resolving from the role's own real
    /// default -- proves the per-field-override cascade, not just "role
    /// works in isolation."
    #[test]
    fn a_literal_field_overrides_just_that_one_field_on_top_of_the_role() {
        let yaml = r#"
id: label
kind: Text
text: {content: "Hi", typography_role: title_medium, font_size: 20}
style: {width: 90, height: 30, foreground: white}
"#;
        let mut tree = Tree::new();
        let root = load_view(&mut tree, yaml).expect("role plus a literal override must resolve");
        let node = tree.get(root).unwrap();
        let NodeKind::Text(text) = &node.kind else {
            panic!("expected a Text node");
        };
        let expected = engine_md3::type_style_named("title_medium").unwrap();
        assert_eq!(text.font_size, 20.0, "the literal override must win");
        assert_eq!(
            text.font_weight, expected.font_weight,
            "every field not literally overridden must still come from the role"
        );
    }

    /// An unrecognized role name must be a real, clear
    /// `SpecError::UnknownTypographyRole` naming the widget's own id
    /// and the bad role -- not a silent fallback to some default role.
    #[test]
    fn unknown_text_role_names_the_offending_widget_and_role() {
        let yaml = r#"
id: bad-role
kind: Text
text: {content: "Hi", typography_role: subtitle_huge}
style: {width: 90, height: 30, foreground: white}
"#;
        let mut tree = Tree::new();
        let err = load_view(&mut tree, yaml).expect_err("an unrecognized role must fail");
        let SpecError::UnknownTypographyRole { id, role } = err else {
            panic!("expected UnknownTypographyRole, got {err:?}");
        };
        assert_eq!(id, "bad-role");
        assert_eq!(role, "subtitle_huge");
    }

    /// With neither `role` nor a literal `font_family`/`font_size`,
    /// the real, final "missing required field" error must still fire
    /// -- now at build time (`resolve_text_style`) rather than serde's
    /// own automatic parse-time check, a real, inherent consequence of
    /// widening both to genuinely role-derivable `Option`s.
    #[test]
    fn neither_role_nor_literal_font_fields_is_a_clear_missing_field_error() {
        let yaml = r#"
id: label
kind: Text
text: {content: "Hi"}
style: {width: 90, height: 30, foreground: white}
"#;
        let mut tree = Tree::new();
        let err =
            load_view(&mut tree, yaml).expect_err("no role and no literal font fields must fail");
        assert!(matches!(err, SpecError::MissingField { kind: "Text", .. }));
    }

    // M22 Phase 2 (§16.1): a real, tiny, decodable 4x4 PNG -- the
    // identical real bytes `examples/image.py`/`tests/test_image.py`
    // already embed, reused here so every real image-loading test in
    // this workspace (Rust and Python) trusts the same one real,
    // hand-verified file rather than three separately-maintained ones.
    const TINY_PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x04, 0x08, 0x06, 0x00, 0x00, 0x00, 0xA9,
        0xF1, 0x9E, 0x7E, 0x00, 0x00, 0x00, 0x4F, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x01, 0x44,
        0x00, 0xBB, 0xFF, 0x00, 0xF4, 0x43, 0x36, 0xFF, 0xFF, 0x98, 0x00, 0xFF, 0xFF, 0xEB, 0x3B,
        0xFF, 0x4C, 0xAF, 0x50, 0xFF, 0x01, 0x00, 0xBC, 0xD4, 0xFF, 0x21, 0xDA, 0x1F, 0x00, 0x1E,
        0xBB, 0xC2, 0x00, 0x5D, 0xD6, 0xFB, 0x00, 0x00, 0xE9, 0x1E, 0x63, 0xFF, 0x79, 0x55, 0x48,
        0xFF, 0x60, 0x7D, 0x8B, 0xFF, 0x00, 0x00, 0x00, 0x80, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0x00,
        0xC2, 0x08, 0x00, 0x8C, 0x02, 0x43, 0x00, 0xDC, 0x77, 0x6D, 0x00, 0xBD, 0x52, 0x20, 0xA1,
        0x64, 0xD1, 0xE1, 0x93, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60,
        0x82,
    ];

    /// A fresh, real, uniquely-named temp directory per test -- the
    /// identical real-filesystem discipline `include.rs`'s own tests
    /// already established, not a mocked one.
    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "engine_spec_build_test_{name}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    #[test]
    fn kind_image_builds_a_real_node_with_the_real_decoded_image_and_fit() {
        let dir = temp_dir("image_basic");
        std::fs::write(dir.join("logo.png"), TINY_PNG).expect("write test PNG");
        let yaml = r#"
id: logo
kind: Image
image: {src: logo.png, fit: cover}
style: {width: 40, height: 40}
"#;
        let mut tree = Tree::new();
        let root = load_view_with_base_dir(&mut tree, yaml, Some(&dir))
            .expect("a real, decodable image must build");
        let NodeKind::Image(state) = &tree.get(root).unwrap().kind else {
            panic!("expected an Image node");
        };
        assert_eq!(state.image.width, 4);
        assert_eq!(state.image.height, 4);
        assert_eq!(state.content_fit, engine_core::ContentFit::Cover);
    }

    #[test]
    fn kind_image_with_no_fit_defaults_to_fill() {
        let dir = temp_dir("image_default_fit");
        std::fs::write(dir.join("logo.png"), TINY_PNG).expect("write test PNG");
        let yaml = r#"
id: logo
kind: Image
image: {src: logo.png}
style: {width: 40, height: 40}
"#;
        let mut tree = Tree::new();
        let root = load_view_with_base_dir(&mut tree, yaml, Some(&dir))
            .expect("an image with no fit: must still build");
        let NodeKind::Image(state) = &tree.get(root).unwrap().kind else {
            panic!("expected an Image node");
        };
        assert_eq!(state.content_fit, engine_core::ContentFit::Fill);
    }

    #[test]
    fn kind_image_with_no_src_builds_the_same_blank_placeholder_add_video_uses() {
        // tre issue #2: `src:` is optional -- omitted entirely, no
        // base_dir required at all (there's no file to resolve), and
        // the resulting node is the identical 1x1 transparent
        // placeholder `Window.add_video`'s own default already builds
        // (`ImageState::blank`).
        let yaml = r#"
id: placeholder
kind: Image
image: {fit: cover}
style: {width: 40, height: 40}
"#;
        let mut tree = Tree::new();
        let root = load_view(&mut tree, yaml)
            .expect("kind: Image with no src: must build, no base_dir needed");
        let NodeKind::Image(state) = &tree.get(root).unwrap().kind else {
            panic!("expected an Image node");
        };
        let blank = engine_core::ImageState::blank();
        assert_eq!(state.image.width, blank.image.width);
        assert_eq!(state.image.height, blank.image.height);
        // `peniko::Blob`'s own `PartialEq` compares by allocation
        // identity, not byte content -- two independently-built blobs
        // with identical bytes aren't `==`, so compare the real bytes.
        assert_eq!(state.image.data.data(), blank.image.data.data());
        assert_eq!(state.content_fit, engine_core::ContentFit::Cover);
    }

    #[test]
    fn kind_image_with_no_image_block_is_a_clear_error_not_a_panic() {
        let yaml = "id: logo\nkind: Image\nstyle: {width: 40, height: 40}\n";
        let mut tree = Tree::new();
        let err = load_view(&mut tree, yaml)
            .expect_err("kind: Image with no image: block must fail clearly");
        assert!(matches!(err, SpecError::MissingField { .. }));
    }

    #[test]
    fn kind_image_with_no_base_dir_is_a_clear_error_not_a_panic() {
        let yaml = r#"
id: logo
kind: Image
image: {src: logo.png}
style: {width: 40, height: 40}
"#;
        let mut tree = Tree::new();
        let err = load_view(&mut tree, yaml)
            .expect_err("kind: Image with no base_dir to resolve src: against must fail");
        assert!(matches!(err, SpecError::ImageSrcNoBaseDir { .. }));
    }

    #[test]
    fn kind_image_src_escaping_base_dir_is_rejected() {
        let dir = temp_dir("image_escape");
        let inner = dir.join("inner");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(dir.join("secret.png"), TINY_PNG).expect("write test PNG");
        let yaml = r#"
id: logo
kind: Image
image: {src: "../secret.png"}
style: {width: 40, height: 40}
"#;
        let mut tree = Tree::new();
        let err = load_view_with_base_dir(&mut tree, yaml, Some(&inner))
            .expect_err("a real ../ escape outside base_dir must be rejected");
        assert!(matches!(err, SpecError::ImageSrcEscapesBase { .. }));
    }

    #[test]
    fn kind_image_with_an_undecodable_file_is_a_clear_error_not_a_panic() {
        let dir = temp_dir("image_bogus");
        std::fs::write(dir.join("logo.png"), b"not a real png").expect("write bogus file");
        let yaml = r#"
id: logo
kind: Image
image: {src: logo.png}
style: {width: 40, height: 40}
"#;
        let mut tree = Tree::new();
        let err = load_view_with_base_dir(&mut tree, yaml, Some(&dir))
            .expect_err("an undecodable file must fail clearly, not panic");
        assert!(matches!(err, SpecError::ImageDecodeFailed { .. }));
    }

    /// `load_view`'s own real public signature stays exactly as its doc
    /// comment states (unchanged, for its one existing caller); these
    /// tests need a real `base_dir`, so this local helper reaches
    /// `build_tree` directly the same way `load_view`/`load_styled_view`
    /// themselves do, rather than widening `load_view`'s own contract.
    fn load_view_with_base_dir(
        tree: &mut Tree,
        yaml: &str,
        base_dir: Option<&std::path::Path>,
    ) -> Result<NodeId, SpecError> {
        let spec = parse_view(yaml)?;
        build_tree(tree, &spec, None, None, None, None, base_dir)
    }

    #[test]
    fn kind_icon_builds_a_real_icon_node_with_the_named_glyph_and_color() {
        let yaml = r##"
id: gear
kind: Icon
icon: {name: settings}
style: {width: 24, height: 24, foreground: "#1C1B1FFF"}
"##;
        let mut tree = Tree::new();
        let root = load_view(&mut tree, yaml).expect("a real, known icon name must build");
        let node = tree.get(root).expect("root must exist");
        let NodeKind::Icon(icon) = &node.kind else {
            panic!("expected an Icon node");
        };
        assert_eq!(icon.tint.current, Color::from_rgba8(0x1C, 0x1B, 0x1F, 0xFF));
        assert!(
            !icon.path.elements().is_empty(),
            "the real curated SVG path data for \"settings\" must have parsed into real path elements"
        );
    }

    #[test]
    fn kind_icon_with_no_icon_block_is_a_clear_missing_field_error() {
        let yaml = r##"
id: gear
kind: Icon
style: {width: 24, height: 24, foreground: "#1C1B1FFF"}
"##;
        let mut tree = Tree::new();
        let err = load_view(&mut tree, yaml).expect_err("a Icon with no icon: block must fail");
        assert!(matches!(
            err,
            SpecError::MissingField {
                kind: "Icon",
                field: "icon",
                ..
            }
        ));
    }

    #[test]
    fn kind_icon_with_an_unknown_name_is_a_clear_error_naming_it() {
        let yaml = r##"
id: gear
kind: Icon
icon: {name: not_a_real_icon}
style: {width: 24, height: 24, foreground: "#1C1B1FFF"}
"##;
        let mut tree = Tree::new();
        let err = load_view(&mut tree, yaml).expect_err("an unknown icon name must fail clearly");
        match err {
            SpecError::UnknownIcon { name, .. } => assert_eq!(name, "not_a_real_icon"),
            other => panic!("expected UnknownIcon, got {other:?}"),
        }
    }

    #[test]
    fn kind_icon_with_no_foreground_is_a_clear_missing_field_error() {
        // M90: the glyph's color is `style.foreground`, required the same
        // way `kind: Text`'s is (`required_foreground`).
        let yaml = r#"
id: gear
kind: Icon
icon: {name: settings}
style: {width: 24, height: 24}
"#;
        let mut tree = Tree::new();
        let err = load_view(&mut tree, yaml).expect_err("Icon with no foreground must fail");
        assert!(matches!(
            err,
            SpecError::MissingField {
                kind: "Icon",
                field: "style.foreground",
                ..
            }
        ));
    }

    // Tesserae M27: real, repeatable coverage for all 7 new declarative
    // primitives -- one "builds correctly, real colors resolved" test
    // per kind (mirroring `kind_icon_builds_a_real_icon_node_with_the_
    // named_glyph_and_color`'s own style), plus the couple of real edge
    // cases each kind's own scoping actually named (no theme set falls
    // back to the real MD3 baseline; `LoadingIndicator` needs `style.
    // width`/`height`).

    #[test]
    fn kind_link_reuses_text_and_resolves_role_or_literal_background() {
        let yaml = r##"
id: docs
kind: Link
text: {content: "Docs", font_family: Roboto, font_size: 14}
style: {width: 60, height: 20, foreground: "#6750A4FF"}
"##;
        let mut tree = Tree::new();
        let root = load_view(&mut tree, yaml).expect("a real Link must build");
        let node = tree.get(root).expect("root must exist");
        let NodeKind::Link(text) = &node.kind else {
            panic!("expected a Link node");
        };
        assert_eq!(text.content, "Docs");
    }

    #[test]
    fn kind_link_with_no_text_block_is_a_clear_missing_field_error() {
        let yaml = r##"
id: docs
kind: Link
style: {width: 60, height: 20, foreground: "#6750A4FF"}
"##;
        let mut tree = Tree::new();
        let err = load_view(&mut tree, yaml).expect_err("a Link with no text: block must fail");
        assert!(matches!(
            err,
            SpecError::MissingField {
                kind: "Link",
                field: "text",
                ..
            }
        ));
    }

    #[test]
    fn kind_radio_button_reuses_checked_as_selected_with_baseline_tints() {
        let yaml = r#"
id: opt
kind: RadioButton
selected: true
style: {width: 20, height: 20}
"#;
        let mut tree = Tree::new();
        let root = load_view(&mut tree, yaml).expect("a real RadioButton must build with no theme");
        let node = tree.get(root).expect("root must exist");
        let NodeKind::RadioButton(state) = &node.kind else {
            panic!("expected a RadioButton node");
        };
        assert!(
            state.selected,
            "selected: true must seed RadioButtonState.selected"
        );
        assert_eq!(state.selected_tint, BASELINE_PRIMARY);
        assert_eq!(state.unselected_tint, BASELINE_OUTLINE);
    }

    #[test]
    fn kind_switch_reuses_checked_as_on_with_all_5_baseline_tints() {
        let yaml = r#"
id: toggle
kind: Switch
selected: false
style: {width: 52, height: 32}
"#;
        let mut tree = Tree::new();
        let root = load_view(&mut tree, yaml).expect("a real Switch must build with no theme");
        let node = tree.get(root).expect("root must exist");
        let NodeKind::Switch(state) = &node.kind else {
            panic!("expected a Switch node");
        };
        assert!(
            !state.on,
            "selected: false must seed SwitchState.on = false"
        );
        assert_eq!(state.track_off_tint, BASELINE_SURFACE_CONTAINER_HIGHEST);
        assert_eq!(state.track_on_tint, BASELINE_PRIMARY);
        assert_eq!(state.track_outline_tint, BASELINE_OUTLINE);
        assert_eq!(state.handle_off_tint, BASELINE_OUTLINE);
        assert_eq!(state.handle_on_tint, BASELINE_ON_PRIMARY);
    }

    #[test]
    fn kind_circular_progress_reuses_value_with_one_baseline_tint() {
        let yaml = r#"
id: spinner
kind: CircularProgress
value: 0.4
style: {width: 48, height: 48}
"#;
        let mut tree = Tree::new();
        let root =
            load_view(&mut tree, yaml).expect("a real CircularProgress must build with no theme");
        let node = tree.get(root).expect("root must exist");
        let NodeKind::CircularProgress(state) = &node.kind else {
            panic!("expected a CircularProgress node");
        };
        assert_eq!(state.value.current, 0.4);
        assert_eq!(state.indicator_tint, BASELINE_PRIMARY);
    }

    #[test]
    fn kind_linear_progress_reuses_value_with_both_baseline_tints() {
        let yaml = r#"
id: bar
kind: LinearProgress
value: 0.75
style: {width: 200, height: 4}
"#;
        let mut tree = Tree::new();
        let root =
            load_view(&mut tree, yaml).expect("a real LinearProgress must build with no theme");
        let node = tree.get(root).expect("root must exist");
        let NodeKind::LinearProgress(state) = &node.kind else {
            panic!("expected a LinearProgress node");
        };
        assert_eq!(state.value.current, 0.75);
        assert_eq!(state.track_tint, BASELINE_SURFACE_CONTAINER_HIGHEST);
        assert_eq!(state.indicator_tint, BASELINE_PRIMARY);
    }

    #[test]
    fn kind_loading_indicator_seeds_shape_and_resolves_glyph_tint() {
        let yaml = r##"
id: spinner
kind: LoadingIndicator
style: {width: 48, height: 48, foreground: "#6750A4FF"}
"##;
        let mut tree = Tree::new();
        let root = load_view(&mut tree, yaml).expect("a real LoadingIndicator must build");
        let node = tree.get(root).expect("root must exist");
        assert!(matches!(node.kind, NodeKind::LoadingIndicator(_)));
        assert_ne!(
            node.paint.shape.current,
            ShapeKey::empty(),
            "paint.shape must be seeded to the real first shape at construction, not left \
             empty, or the very first real tick would visibly flash"
        );
    }

    #[test]
    fn kind_loading_indicator_with_no_width_is_a_clear_missing_field_error() {
        let yaml = r##"
id: spinner
kind: LoadingIndicator
style: {height: 48, foreground: "#6750A4FF"}
"##;
        let mut tree = Tree::new();
        let err = load_view(&mut tree, yaml)
            .expect_err("a LoadingIndicator with no style.width must fail");
        assert!(matches!(
            err,
            SpecError::MissingField {
                kind: "LoadingIndicator",
                field: "style.width",
                ..
            }
        ));
    }

    #[test]
    fn kind_time_picker_dial_uses_the_new_hour_and_minute_fields() {
        let yaml = r#"
id: dial
kind: TimePickerDial
hour: 13
minute: 45
style: {width: 256, height: 256}
"#;
        let mut tree = Tree::new();
        let root =
            load_view(&mut tree, yaml).expect("a real TimePickerDial must build with no theme");
        let node = tree.get(root).expect("root must exist");
        let NodeKind::TimePickerDial(state) = &node.kind else {
            panic!("expected a TimePickerDial node");
        };
        assert_eq!(state.hour, 13);
        assert_eq!(state.minute, 45);
        assert_eq!(state.face_tint, BASELINE_SURFACE_CONTAINER_HIGHEST);
        assert_eq!(state.hand_tint, BASELINE_PRIMARY);
    }

    /// M90: an inline `style.background` on a glyph kind is the pre-0.3.3
    /// spelling -- a clear error naming `foreground`, for every such kind.
    #[test]
    fn inline_background_on_a_glyph_kind_is_an_error_naming_foreground() {
        for (kind, extra) in [
            (
                "Text",
                "text: {content: Hi, font_family: Roboto, font_size: 16}",
            ),
            (
                "Link",
                "text: {content: Hi, font_family: Roboto, font_size: 16}",
            ),
            ("Icon", "icon: {name: settings}"),
            ("LoadingIndicator", ""),
        ] {
            let yaml = format!(
                "id: w\nkind: {kind}\n{extra}\nstyle: {{width: 24, height: 24, background: \"#000000\"}}\n"
            );
            let err = load_view(&mut Tree::new(), &yaml).expect_err("background on a glyph kind");
            assert!(
                matches!(err, SpecError::NotAFill { .. }),
                "{kind}: expected NotAFill, got {err:?}"
            );
            assert!(
                err.to_string().contains("style.foreground"),
                "{kind}: {err}"
            );
        }
    }

    /// M90: a `background` reaching a `Text` only through the cascade
    /// (a selector-less baseline rule meant for every widget) is ignored,
    /// not an error -- only the widget's own inline style is checked.
    #[test]
    fn a_cascaded_background_is_ignored_on_a_glyph_kind() {
        let sheet =
            crate::cascade::parse_stylesheet("styles:\n  - style: {background: \"#FF0000\"}\n")
                .unwrap();
        let yaml = "id: t\nkind: Text\ntext: {content: Hi, font_family: Roboto, font_size: 16}\n\
                    style: {width: 50, height: 20, foreground: \"#00FF00\"}\n";
        let spec = crate::spec::parse_view(yaml).unwrap();
        let mut tree = Tree::new();
        let root = build_tree(&mut tree, &spec, None, None, Some(&sheet), None, None)
            .expect("a cascaded background must not fail a Text");
        let paint = &tree.get(root).unwrap().paint;
        assert_eq!(
            paint.background.current,
            Color::from_rgba8(0, 0xFF, 0, 0xFF)
        );
    }

    /// M90: `Checkbox` takes `checked`, `Switch`/`RadioButton` take
    /// `selected`; the wrong one is a clear error naming the right one.
    #[test]
    fn the_wrong_state_field_for_a_kind_is_an_error_naming_the_right_one() {
        for (yaml, expected) in [
            ("id: s\nkind: Switch\nchecked: true\n", "selected"),
            ("id: r\nkind: RadioButton\nchecked: true\n", "selected"),
            (
                "id: c\nkind: Checkbox\nselected: true\nstyle: {width: 20, height: 20, background: \"#000000\"}\n",
                "checked",
            ),
        ] {
            let err = load_view(&mut Tree::new(), yaml).expect_err("wrong state field");
            assert!(
                matches!(err, SpecError::WrongStateField { expected: e, .. } if e == expected),
                "expected WrongStateField naming {expected:?}, got {err:?}"
            );
        }
    }
}
