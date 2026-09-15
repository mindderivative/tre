//! `WidgetSpec` and friends (ARCHITECTURE.md §16.1), narrowed to §14 step
//! 5's own stated scope: "parse one static `view.yaml` (no `bindings:`/
//! `handlers:` yet)". The full §16.1 struct also carries `bindings:
//! HashMap<String, Expression>` and `handlers: HashMap<String, String>`
//! -- both omitted here entirely, not stubbed, since nothing resolves
//! them without `BindingResolver` (§16.2), which doesn't exist until
//! step 12 and needs `engine-py`'s GIL access this crate deliberately
//! never has. Adding them back is a additive change to this struct, not
//! a redesign, when step 12 actually needs them.
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
    #[serde(default)]
    pub children: Vec<WidgetSpec>,
}

/// Maps to `engine_core::NodeKind` (§5). Only the three variants
/// `engine-core` currently has -- `Rect`/`Container`/`Text` -- match
/// step 3/4's own scope exactly; `Image`/`Slider`/`Checkbox`/`Canvas`
/// land here whenever `engine-core::NodeKind` itself grows them.
/// Deliberately unit-only -- see the module doc comment for why `Text`'s
/// own fields live in a sibling `WidgetSpec::text` instead of here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum NodeKindSpec {
    Rect,
    Container,
    Text,
}

/// Mirrors `engine_core::TextState` exactly (§14 step 4) -- no new
/// fields invented here, since this crate's job is mapping to that
/// struct, not extending it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextSpec {
    pub content: String,
    pub font_family: String,
    #[serde(default = "default_font_weight")]
    pub font_weight: f32,
    pub font_size: f32,
}

fn default_font_weight() -> f32 {
    400.0 // CSS/OpenType "normal" -- matches parley::FontWeight::NORMAL.
}

/// Row/Column only -- `taffy::style::FlexDirection` also has
/// `RowReverse`/`ColumnReverse`, not exposed here since nothing in this
/// step's own scope needs them; additive to add later.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub enum FlexDirectionSpec {
    Row,
    Column,
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
    pub padding: Option<f32>,
    pub gap: Option<f32>,
    /// A hex (`"#6750A4"`, `"#6750A4FF"`) or CSS named (`"transparent"`,
    /// `"white"`) color string -- anything `peniko::color::parse_color`
    /// accepts. Parsed at tree-build time (`build.rs`), not here: a
    /// parse failure needs the owning widget's `id` in its error
    /// message, which this struct alone doesn't have context for.
    pub background: Option<String>,
    pub corner_radius: Option<f32>,
    pub opacity: Option<f32>,
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
        assert_eq!(spec.style.padding, Some(12.0));
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
        assert_eq!(
            text.font_weight, 400.0,
            "unset font_weight must default to 400 (normal)"
        );
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
}
