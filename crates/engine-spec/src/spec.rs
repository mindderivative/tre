//! `WidgetSpec` and friends (ARCHITECTURE.md §16.1). `bindings`/
//! `handlers` were originally omitted entirely here at §14 step 5
//! ("parse one static `view.yaml` (no `bindings:`/`handlers:` yet)"),
//! since nothing could resolve them without `BindingResolver` (§16.2).
//! Added back at step 12, additively, now that `engine-spec::binding`
//! defines that trait and `engine-py`'s `View._attach()` implements the
//! resolution side. `bindings`/`handlers` values stay raw `String`s
//! here (a `"{{ ... }}"` expression, a plain method name) -- parsing a
//! binding into an `Expression` happens lazily, in `engine-py`, only
//! for the bindings a `View._attach()` call actually walks; a widget
//! tree that's never attached to a `ViewModel` pays nothing beyond
//! carrying the raw strings.
//!
//! `StyleSpec` is similarly narrower than §16.3's eventual stylesheet
//! model: literal values only (`background: "#6750A4"`), not MD3 token
//! names (`background: surface`) -- token resolution needs
//! `engine-md3`'s color-scheme machinery, which doesn't exist until step
//! 11 (`material-colors`). `background` is parsed via
//! `peniko::color::parse_color`, which already accepts hex and CSS named
//! colors -- reused rather than hand-rolling a hex parser.
//!
//! **Real finding:** `NodeKindSpec` is a plain, unit-only enum
//! (`Rect`/`Container`/`Text`) with `Text`'s own fields living in a
//! sibling `text:` block on `WidgetSpec`, not `Text(TextSpec)` as a
//! data-carrying enum variant under `kind:`. First attempt used the
//! latter -- the classic serde external-tagging shape (`kind: {Text:
//! {...}}`) -- and `serde_yaml_ng` rejected it: unlike `serde_json`,
//! its `deserialize_enum` only accepts a bare scalar (unit variants) or
//! YAML's own `!Tag` syntax for a data-carrying variant, not a
//! single-key mapping. Confirmed directly in its source
//! (`de.rs`'s `deserialize_enum`), not worked around with a `!Text` tag
//! in every `view.yaml` -- that's an obscure YAML convention with no
//! reason to ask a view author to know it. The flatter `kind: Text` +
//! `text: {...}` shape sidesteps the whole issue and reads more like
//! the declarative UI YAML this section is modeled on (§16, pyCopper).

use std::collections::HashMap;

use serde::Deserialize;

/// One node in a `view.yaml` tree. `deny_unknown_fields` makes a typo'd
/// key a load-time error with a line number, not a silently-ignored
/// style -- the same "fail loudly at the boundary" reasoning §16.1 gives
/// for `EngineError` (§8).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WidgetSpec {
    /// Author-assigned, stable -- distinct from the runtime `NodeId`
    /// (§5), which has no meaning across a reload. Reconciliation
    /// (§16.4) is what actually matches on this; step 5 doesn't
    /// reconcile anything yet, but the field is part of §16.1's own
    /// struct shape, not step-5-specific.
    pub id: String,
    pub kind: NodeKindSpec,
    /// This widget's own applied classes -- §16.3's `classes:` selector
    /// matches against these, not against anything on the stylesheet
    /// side alone.
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub style: StyleSpec,
    /// Required (and validated as such at tree-build time, not here)
    /// when `kind: Text`; ignored for every other kind.
    #[serde(default)]
    pub text: Option<TextSpec>,
    /// M14 Phase 3 (§5, §7.3): this widget's own initial `checked` --
    /// only meaningful for `kind: Checkbox`, ignored otherwise, the
    /// same "required/meaningful for one kind, ignored for others"
    /// shape `text` already has. Overwritten immediately by a real
    /// `bindings: {checked: ...}` one-way resolution if one exists, the
    /// same way a static `style.opacity` is overwritten by a bound one.
    #[serde(default)]
    pub checked: bool,
    /// M14 Phase 3 (§5, §7.3): this widget's own initial `thumb_
    /// position` -- only meaningful for `kind: Slider`, same shape as
    /// `checked` above.
    #[serde(default)]
    pub value: f64,
    /// M22 Phase 2 (§16.1, §5): required (and validated as such at
    /// tree-build time, matching `text`'s own contract) when `kind:
    /// Image`; ignored otherwise.
    #[serde(default)]
    pub image: Option<ImageSpec>,
    /// `property name -> "{{ expression }}"` (§16.2). Raw strings --
    /// see this module's own doc comment for why parsing is deferred to
    /// whoever actually attaches a `ViewModel`.
    #[serde(default)]
    pub bindings: HashMap<String, String>,
    /// `event name -> ViewModel method name` (§16.2), e.g. `{on_click:
    /// "bump"}`.
    #[serde(default)]
    pub handlers: HashMap<String, String>,
    /// M14 Phase 3 (§16.7): names which of this widget's own `bindings`
    /// keys (if any) is also a real two-way binding -- writes back to
    /// its bound `Signal` on a real `Change`. `None` (the default)
    /// means every binding here stays one-way, matching every binding
    /// before this phase. **A deliberate, real deviation** from
    /// ARCHITECTURE.md §16.7's own inline illustration (`bindings:
    /// {text: "{{ username }}", two_way: true}`, a `two_way` key mixed
    /// into the same map as per-property expressions) -- that shape
    /// can't cleanly deserialize into this struct's own flat `bindings:
    /// HashMap<String, String>` without either a mixed-type value enum
    /// or losing `deny_unknown_fields`' own "fail loudly on a typo"
    /// guarantee across the whole map. A separate, explicitly-named
    /// sibling field is simpler, equally expressive for this
    /// framework's own real bindable components (one natural edit
    /// property per widget -- `Slider.value`/`Checkbox.checked`, not
    /// several at once), and fully backward-compatible with every
    /// existing `view.yaml`.
    #[serde(default)]
    pub two_way: Option<String>,
    #[serde(default)]
    pub children: Vec<WidgetSpec>,
}

/// Maps to `engine_core::NodeKind` (§5). `Rect`/`Container`/`Text`
/// matched step 3/4's own original scope; `Checkbox`/`Slider` (M14
/// Phase 3) are real now -- this comment used to name them as landing
/// "whenever `engine_core::NodeKind` itself grows them," which it has.
/// `TextField` (M15 Phase 3) is real too. `Image` (M22 Phase 2) is real
/// now too -- `Canvas` remains a real, un-scoped future candidate (its
/// content is a Python draw callback, §11.10/§11.11, with no obvious
/// static YAML representation the way a file-backed `Image` has).
/// Deliberately unit-only -- see the module doc comment for why
/// `Text`'s own fields live in a sibling `WidgetSpec::text` instead of
/// here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum NodeKindSpec {
    Rect,
    Container,
    Text,
    Checkbox,
    Slider,
    TextField,
    Image,
}

/// M22 Phase 2 (§16.1, §5): `kind: Image`'s own sibling block, the
/// identical "required/meaningful for one kind, ignored for others"
/// shape `text`/`checked`/`value` already established. `src` is a path
/// relative to the owning `view.yaml` file's own directory -- resolved
/// and confined the same way `include:` already confines its own
/// paths (`include.rs`'s `resolve_confined`, reused rather than a
/// second path-confinement scheme), not relative to the current
/// working directory or the running process's own location.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageSpec {
    pub src: String,
    #[serde(default)]
    pub fit: ContentFitSpec,
}

/// Mirrors `engine_core::ContentFit` exactly -- see `ImageState.
/// content_fit`'s own doc comment for what each variant means.
/// `#[default] Fill` matches `ContentFit::default()`'s own real
/// choice, so an `image:` block with no `fit:` at all keeps
/// `Window.add_image`'s own byte-for-byte default behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub enum ContentFitSpec {
    Cover,
    Contain,
    #[default]
    Fill,
}

/// Mirrors `engine_core::TextState` exactly (§14 step 4) -- no new
/// fields invented here, since this crate's job is mapping to that
/// struct, not extending it. M15 Phase 3 (§16.7): also the real,
/// deliberately-reused shape `kind: TextField`'s own `text:` block
/// uses -- `TextFieldState`'s own font/content fields are byte-for-byte
/// the same four `TextState` already has, so a second, parallel spec
/// struct would just be a duplicate, not a real distinction.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextSpec {
    pub content: String,
    /// M62 Phase 4 (§7.1, §16.3): a real MD3 typography role name
    /// (`"body_large"`, etc., `engine_md3::type_style_named`'s own real
    /// vocabulary) -- when given, supplies `font_family`/`font_weight`/
    /// `font_size`/`line_height` as real defaults, resolved at build
    /// time (`build.rs`), not here (this struct stays parse-time data,
    /// the same "parsed at apply time" precedent every other role/token
    /// reference in this codebase already follows, e.g. `StyleSpec.
    /// corner_radius`'s own `ShapeOrElevationSpec`). Any of the 4 fields
    /// below, if *also* given, override just that one field on top of
    /// the role's own resolved default -- the identical per-field-
    /// override shape `TypographyOverride` (`theme.rs`) already
    /// establishes for a theme's own role overrides.
    #[serde(default)]
    pub role: Option<String>,
    /// M62 Phase 4: widened from a plain required `String` to
    /// `Option<String>` -- now derivable from `role` above, so no
    /// longer unconditionally required. Still required in the real,
    /// final sense (a build-time `SpecError::MissingField` if *neither*
    /// this nor `role` supplies one), just validated one stage later
    /// than serde's own automatic "missing required field" check used
    /// to catch it -- an inherent, real consequence of making the field
    /// genuinely derivable, not an oversight.
    #[serde(default)]
    pub font_family: Option<String>,
    /// M62 Phase 4: widened from a defaulted `f32` (`default_font_
    /// weight`, always `400.0` when unset) to `Option<f32>` -- unset
    /// now means "derive from `role`, or fall back to `400.0`" instead
    /// of "always `400.0`," resolved at build time so a role's own real
    /// weight (e.g. `title_medium`'s real `500.0`) can supply it.
    #[serde(default)]
    pub font_weight: Option<f32>,
    /// M62 Phase 4: the same real widening `font_family` above got, and
    /// for the identical reason.
    #[serde(default)]
    pub font_size: Option<f32>,
    /// M62 Phase 1 (§7.1, §16.3): `engine_core::TextState.line_height`'s
    /// own real declarative counterpart -- only reachable from `kind:
    /// Text` (`build.rs`'s `TextField` arm builds a `TextFieldState`,
    /// which has no equivalent field at all, matching `engine-render::
    /// draw_field`'s own out-of-scope decision for this milestone).
    /// Absent/`None` means exactly what it always has: `parley`'s own
    /// real font-metrics-relative default, not a new fallback number --
    /// unless `role` supplies one, the identical real "role supplies a
    /// default, a literal field overrides it" resolution the 3 fields
    /// above now also follow.
    #[serde(default)]
    pub line_height: Option<f32>,
}

/// Row/Column only -- `taffy::style::FlexDirection` also has
/// `RowReverse`/`ColumnReverse`, not exposed here since nothing in this
/// step's own scope needs them; additive to add later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum FlexDirectionSpec {
    Row,
    Column,
}

/// M59 (§5, §16.3): a `padding`/`margin` value that accepts *either* a
/// bare scalar (applied uniformly to all four sides, the real shape
/// `StyleSpec.padding` already had before this milestone) *or* a real
/// per-side `{top, right, bottom, left}` object -- CSS's own "shorthand
/// vs. longhand" duality, and the first real `#[serde(untagged)]` union
/// in this file (no existing scalar-or-object precedent to mirror; a
/// bare number and a YAML mapping are structurally distinct enough that
/// `serde_yaml_ng`'s own untagged-enum support resolves them
/// unambiguously, confirmed by this milestone's own new unit tests
/// below, not assumed). Every per-side field defaults to `0.0` when
/// omitted, matching a real CSS `padding: {top: 4px}` shorthand's own
/// "unset sides are zero" behavior, not "unset sides keep whatever the
/// uniform value would have been" (there is no uniform value once the
/// per-side form is chosen).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum SpacingSpec {
    Uniform(f32),
    PerSide {
        #[serde(default)]
        top: f32,
        #[serde(default)]
        right: f32,
        #[serde(default)]
        bottom: f32,
        #[serde(default)]
        left: f32,
    },
}

/// M59 (§5, §16.3): the real, common flexbox `align-items` vocabulary
/// -- deliberately the same bounded subset `FlexDirectionSpec` already
/// established the precedent for (skips taffy's own `Self*`/`Safe*`
/// overflow-position variants, real CSS features nothing in this
/// codebase's own scope needs yet, additive to add later).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum AlignItemsSpec {
    Start,
    End,
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
    Stretch,
}

/// M59 (§5, §16.3): `justify-content`'s own real vocabulary -- a
/// superset of `AlignItemsSpec`'s (taffy's own `AlignContent`/
/// `JustifyContent` type adds the real space-distribution keywords
/// `AlignItems` doesn't have), the same bounded-subset precedent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum JustifyContentSpec {
    Start,
    End,
    FlexStart,
    FlexEnd,
    Center,
    Stretch,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// M61 (§16.3): a `corner_radius`/`elevation` value that accepts
/// *either* a bare literal number (the real shape both fields already
/// had) *or* a named token string (e.g. `"small"`), resolved against
/// `engine_md3::shape`'s own real named constants at build time
/// (`resolve_shape_value`, below). Mirrors `StyleSpec.background`'s own
/// real "role name or literal" duality (`resolve_color`) -- but as a
/// genuinely new type, since these two fields are numeric, not
/// `String`, unlike `background`. Stores `f64` uniformly (both fields
/// were already promoted to `f64` at their one real point of use in
/// `build.rs`, so this loses no real precision versus the previous
/// `Option<f32>` shape).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum ShapeOrElevationSpec {
    Literal(f64),
    TokenRef(String),
}

impl ShapeOrElevationSpec {
    /// Resolves to a real `f64` -- a literal passes through unchanged;
    /// a token name is looked up against `engine_md3::shape`'s own real
    /// constants (`named` for shape, `elevation_named` for elevation --
    /// a genuinely different vocabulary, never confused: the caller
    /// states which via `is_elevation`). `None` for an unrecognized
    /// token name -- every real caller turns that into its own crate-
    /// appropriate error (`SpecError::UnknownShapeToken` here in
    /// `engine-spec`; a Python `ValueError` in `engine-py`'s own
    /// `Window.set_theme` resolution of `ComponentOverride`), never a
    /// silent fallback.
    pub fn resolve(&self, is_elevation: bool) -> Option<f64> {
        match self {
            ShapeOrElevationSpec::Literal(v) => Some(*v),
            ShapeOrElevationSpec::TokenRef(name) => {
                if is_elevation {
                    engine_md3::elevation_named(name)
                } else {
                    engine_md3::named(name)
                }
            }
        }
    }
}

/// Literal-value styling for one widget. Every field is optional so a
/// `view.yaml` author only states what a node actually needs -- a
/// `Container` typically sets `flex_direction`/`padding`/`gap` and no
/// `background`; a `Rect` typically sets `background`/`corner_radius`
/// and no `flex_direction`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StyleSpec {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub flex_direction: Option<FlexDirectionSpec>,
    pub padding: Option<SpacingSpec>,
    /// M59 (§5, §16.3): real, previously entirely-absent field -- `taffy
    /// ::Style.margin` has existed since day one but was never reachable
    /// from static YAML at all (confirmed via grep before this change),
    /// unlike `padding`. Same `SpacingSpec` shape (scalar or per-side).
    pub margin: Option<SpacingSpec>,
    pub gap: Option<f32>,
    /// M59 (§5, §16.3): real taffy-level gaps, confirmed unpopulated
    /// from any source anywhere in this codebase before this milestone
    /// (`grep`'d, not assumed). `flex_basis` mirrors `width`/`height`'s
    /// own plain-`f32` shape (a real pixel length, not a percentage --
    /// nothing in this codebase's own scope needs percentage `flex_
    /// basis` yet, additive to add later).
    pub flex_grow: Option<f32>,
    pub flex_shrink: Option<f32>,
    pub flex_basis: Option<f32>,
    pub align_items: Option<AlignItemsSpec>,
    pub justify_content: Option<JustifyContentSpec>,
    /// A hex (`"#6750A4"`, `"#6750A4FF"`) or CSS named (`"transparent"`,
    /// `"white"`) color string -- anything `peniko::color::parse_color`
    /// accepts. Parsed at tree-build time (`build.rs`), not here: a
    /// parse failure needs the owning widget's `id` in its error
    /// message, which this struct alone doesn't have context for.
    pub background: Option<String>,
    /// M61 (§16.3): widened from a bare `Option<f32>` to also accept a
    /// named shape token (e.g. `corner_radius: small`), resolved
    /// against `engine_md3::shape`'s own real named constants
    /// (`resolve_shape_value`, `build.rs`) -- a plain literal number
    /// still parses exactly as before.
    pub corner_radius: Option<ShapeOrElevationSpec>,
    pub opacity: Option<f32>,
    /// M48 (§5, §7): `PaintProperties.border_width`/`border_color` have
    /// existed since M30 Phase 1 but were never reachable from static
    /// YAML -- confirmed via direct read of this struct's own field
    /// list before this change. Parsed the same hex/CSS-name/MD3-token
    /// way `background` already is (`node_kind_and_paint`, `build.rs`).
    pub border_width: Option<f32>,
    pub border_color: Option<String>,
    /// M49 Phase 2: `PaintProperties.elevation` has existed since day
    /// one but was never exposed here at all -- confirmed via direct
    /// read of this struct's own field list before this change, the
    /// same real gap class `border_width`/`border_color` were in before
    /// M48. Lets a theme's own per-kind default styles (M49 Phase 3)
    /// set a real elevation, not just corner radius/opacity. M61
    /// (§16.3): widened the same way `corner_radius` just was, to also
    /// accept a named elevation-level token (e.g. `elevation: level_3`).
    pub elevation: Option<ShapeOrElevationSpec>,
}

/// Parses one `view.yaml` document's raw text into a `WidgetSpec` tree.
/// Structural errors (an unknown key, a wrong type, a missing required
/// field) come back as `serde_yaml_ng::Error`, which already carries a
/// line/column -- exactly the "load-time error with a line number" §16.1
/// promises, for free from `deny_unknown_fields` plus serde's own
/// deserialization error reporting.
pub fn parse_view(yaml: &str) -> Result<WidgetSpec, serde_yaml_ng::Error> {
    serde_yaml_ng::from_str(yaml)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_nested_widget_tree() {
        let yaml = r##"
id: root
kind: Container
style:
  flex_direction: Row
  padding: 12
  gap: 8
children:
  - id: swatch
    kind: Rect
    style:
      width: 40
      height: 40
      background: "#6750A4"
      corner_radius: 8
  - id: label
    kind: Text
    text:
      content: "Hello"
      font_family: Roboto
      font_size: 16
"##;
        let spec = parse_view(yaml).expect("valid view.yaml must parse");
        assert_eq!(spec.id, "root");
        assert!(matches!(spec.kind, NodeKindSpec::Container));
        assert_eq!(spec.style.padding, Some(SpacingSpec::Uniform(12.0)));
        assert_eq!(spec.children.len(), 2);

        assert_eq!(spec.children[0].id, "swatch");
        assert!(matches!(spec.children[0].kind, NodeKindSpec::Rect));
        assert_eq!(
            spec.children[0].style.background.as_deref(),
            Some("#6750A4")
        );

        assert_eq!(spec.children[1].id, "label");
        assert_eq!(spec.children[1].kind, NodeKindSpec::Text);
        let text = spec.children[1]
            .text
            .as_ref()
            .expect("kind: Text must carry a text: block");
        assert_eq!(text.content, "Hello");
        // M62 Phase 4: the real 400.0 (normal) default now resolves at
        // build time (`resolve_text_style`); at parse time, unset is a
        // real `None`.
        assert_eq!(text.font_weight, None);
    }

    #[test]
    fn parses_checkbox_and_slider_widgets_with_checked_value_and_two_way() {
        let yaml = r##"
id: root
kind: Container
children:
  - id: agree
    kind: Checkbox
    checked: true
    bindings: {checked: "{{ agreed }}"}
    two_way: checked
    style: {width: 24, height: 24, background: "#6750A4"}
  - id: volume
    kind: Slider
    value: 0.5
    style: {width: 180, height: 32, background: "#03DAC6"}
"##;
        let spec = parse_view(yaml).expect("Checkbox/Slider widgets must parse");
        assert_eq!(spec.children.len(), 2);

        let checkbox = &spec.children[0];
        assert!(matches!(checkbox.kind, NodeKindSpec::Checkbox));
        assert!(checkbox.checked);
        assert_eq!(checkbox.two_way.as_deref(), Some("checked"));

        let slider = &spec.children[1];
        assert!(matches!(slider.kind, NodeKindSpec::Slider));
        assert_eq!(slider.value, 0.5);
        assert_eq!(
            slider.two_way, None,
            "two_way is per-widget and opt-in -- a widget that never names it stays one-way"
        );
    }

    #[test]
    fn checked_and_value_and_two_way_all_default_when_omitted() {
        let yaml = r##"
id: root
kind: Checkbox
style: {width: 24, height: 24, background: "#6750A4"}
"##;
        let spec = parse_view(yaml).expect("a Checkbox with no checked:/two_way: must still parse");
        assert!(!spec.checked, "checked must default to false");
        assert_eq!(spec.value, 0.0, "value must default to 0.0");
        assert_eq!(spec.two_way, None, "two_way must default to None");
    }

    #[test]
    fn parses_a_text_field_widget_reusing_the_same_text_block_kind_text_uses() {
        let yaml = r##"
id: username
kind: TextField
text: {content: "jane", font_family: Roboto, font_size: 16}
bindings: {text: "{{ name.get() }}"}
two_way: text
style: {width: 200, height: 32, background: "#EEEEEE"}
"##;
        let spec = parse_view(yaml).expect("a TextField widget must parse");
        assert!(matches!(spec.kind, NodeKindSpec::TextField));
        let text = spec
            .text
            .as_ref()
            .expect("kind: TextField must carry a text: block");
        assert_eq!(text.content, "jane");
        assert_eq!(text.font_family.as_deref(), Some("Roboto"));
        // M62 Phase 4: font_weight defaulting to 400 (normal) when
        // unset is now a build-time resolution (`resolve_text_style`),
        // not a parse-time serde default -- at parse time, unset is a
        // real `None`, the same as every other now-Option TextSpec
        // field.
        assert_eq!(text.font_weight, None);
        assert_eq!(spec.two_way.as_deref(), Some("text"));
    }

    #[test]
    fn unknown_field_is_a_load_time_error_not_silently_ignored() {
        let yaml = r#"
id: root
kind: Rect
sytle: {}
"#; // "sytle" -- a real typo, not "style"
        let err = parse_view(yaml).expect_err("a typo'd key must fail to parse, not be ignored");
        let message = err.to_string();
        assert!(
            message.contains("sytle") || message.contains("unknown field"),
            "error message {message:?} doesn't name the actual problem"
        );
    }

    // --- M59 (§5, §16.3): layout API breadth ---------------------------

    #[test]
    fn padding_and_margin_each_accept_a_bare_scalar() {
        let yaml = r#"
id: root
kind: Container
style: {padding: 16, margin: 4}
"#;
        let spec = parse_view(yaml).expect("a bare scalar padding/margin must parse");
        assert_eq!(spec.style.padding, Some(SpacingSpec::Uniform(16.0)));
        assert_eq!(spec.style.margin, Some(SpacingSpec::Uniform(4.0)));
    }

    #[test]
    fn padding_and_margin_each_accept_a_real_per_side_object() {
        let yaml = r#"
id: root
kind: Container
style: {padding: {top: 4, right: 8}, margin: {bottom: 2, left: 6}}
"#;
        let spec = parse_view(yaml).expect("a per-side padding/margin object must parse");
        assert_eq!(
            spec.style.padding,
            Some(SpacingSpec::PerSide {
                top: 4.0,
                right: 8.0,
                bottom: 0.0,
                left: 0.0,
            }),
            "an omitted per-side field must default to 0.0, not the sibling scalar shorthand"
        );
        assert_eq!(
            spec.style.margin,
            Some(SpacingSpec::PerSide {
                top: 0.0,
                right: 0.0,
                bottom: 2.0,
                left: 6.0,
            })
        );
    }

    #[test]
    fn flex_and_align_fields_parse_their_real_yaml_string_forms() {
        let yaml = r#"
id: root
kind: Container
style:
  flex_grow: 1
  flex_shrink: 0
  flex_basis: 100
  align_items: Center
  justify_content: SpaceBetween
"#;
        let spec = parse_view(yaml).expect("the new flex/align fields must parse");
        assert_eq!(spec.style.flex_grow, Some(1.0));
        assert_eq!(spec.style.flex_shrink, Some(0.0));
        assert_eq!(spec.style.flex_basis, Some(100.0));
        assert_eq!(spec.style.align_items, Some(AlignItemsSpec::Center));
        assert_eq!(
            spec.style.justify_content,
            Some(JustifyContentSpec::SpaceBetween)
        );
    }

    #[test]
    fn an_unknown_align_items_variant_is_a_load_time_error_not_silently_ignored() {
        let yaml = r#"
id: root
kind: Container
style: {align_items: Sideways}
"#;
        let err = parse_view(yaml).expect_err("an unknown align_items keyword must fail to parse");
        assert!(err.to_string().contains("Sideways"));
    }
}
