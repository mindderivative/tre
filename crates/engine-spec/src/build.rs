//! `WidgetSpec` -> `engine_core::Tree` (§14 step 5). This is the crate's
//! actual reason to exist for this step: de-risk parsing/validation/
//! mapping in isolation, per Design Principle 5, before anything depends
//! on it working -- rendered through the real Phase 2 pipeline (steps
//! 1-4) unchanged, since a built `Tree` is indistinguishable here from
//! one built imperatively.

use engine_core::{NodeId, NodeKind, PaintProperties, TextState, Tree};
use engine_md3::ColorScheme;
use peniko::Color;
use taffy::prelude::{Rect as TaffyRect, Size, Style, auto, length, zero};

use crate::cascade::{Stylesheet, resolve_style};
use crate::spec::{FlexDirectionSpec, NodeKindSpec, StyleSpec, WidgetSpec, parse_view};

#[derive(Debug, thiserror::Error)]
pub enum SpecError {
    #[error("failed to parse view YAML: {0}")]
    Parse(#[from] serde_yaml_ng::Error),
    #[error("widget \"{id}\": {kind} requires {field}, none given")]
    MissingField {
        id: String,
        kind: &'static str,
        field: &'static str,
    },
    #[error("widget \"{id}\": invalid style.background \"{value}\": {source}")]
    InvalidColor {
        id: String,
        value: String,
        #[source]
        source: peniko::color::ParseError,
    },
}

/// Parses `yaml` and builds it into `tree`, returning the new subtree's
/// root `NodeId`. No stylesheet, no MD3 token resolution -- §14 step 5's
/// original literal-values-only behavior, kept exactly as-is for its one
/// existing caller (`engine-render/tests/spec_view.rs`). `load_styled_view`
/// below is the step-12 entry point that resolves both.
pub fn load_view(tree: &mut Tree, yaml: &str) -> Result<NodeId, SpecError> {
    let spec = parse_view(yaml)?;
    build_tree(tree, &spec, None, None)
}

/// The full §16.3 path: parses `yaml`, resolves every widget's style
/// through `sheet`'s cascade, and resolves any MD3 token name
/// (`background: primary`) against `scheme` -- falling back to literal
/// color parsing (`background: "#6750A4"`) for anything that isn't a
/// recognized role name.
pub fn load_styled_view(
    tree: &mut Tree,
    yaml: &str,
    sheet: &Stylesheet,
    scheme: &ColorScheme,
) -> Result<NodeId, SpecError> {
    let spec = parse_view(yaml)?;
    build_tree(tree, &spec, Some(sheet), Some(scheme))
}

/// Recursively inserts `spec` and its `children` into `tree`, wiring
/// each parent/child edge with `Tree::add_child` as it goes. `sheet`/
/// `scheme` are `None` for the plain literal-values-only path
/// (`load_view`), `Some` for the full styled path (`load_styled_view`).
pub fn build_tree(
    tree: &mut Tree,
    spec: &WidgetSpec,
    sheet: Option<&Stylesheet>,
    scheme: Option<&ColorScheme>,
) -> Result<NodeId, SpecError> {
    let resolved_style = match sheet {
        Some(sheet) => resolve_style(spec, sheet),
        None => spec.style.clone(),
    };
    let layout_style = layout_style(&resolved_style);
    let (kind, paint) = node_kind_and_paint(spec, &resolved_style, scheme)?;
    let id = tree.insert(kind, layout_style, paint);

    for child_spec in &spec.children {
        let child_id = build_tree(tree, child_spec, sheet, scheme)?;
        tree.add_child(id, child_id);
    }

    Ok(id)
}

fn layout_style(style: &StyleSpec) -> Style {
    Style {
        display: taffy::Display::Flex,
        flex_direction: match style.flex_direction {
            Some(FlexDirectionSpec::Row) | None => taffy::FlexDirection::Row,
            Some(FlexDirectionSpec::Column) => taffy::FlexDirection::Column,
        },
        size: Size {
            width: style.width.map_or_else(auto, length),
            height: style.height.map_or_else(auto, length),
        },
        padding: style.padding.map_or_else(TaffyRect::zero, |p| TaffyRect {
            left: length(p),
            right: length(p),
            top: length(p),
            bottom: length(p),
        }),
        gap: style.gap.map_or_else(zero, |g| Size {
            width: length(g),
            height: length(g),
        }),
        ..Default::default()
    }
}

fn node_kind_and_paint(
    spec: &WidgetSpec,
    style: &StyleSpec,
    scheme: Option<&ColorScheme>,
) -> Result<(NodeKind, PaintProperties), SpecError> {
    let corner_radius = f64::from(style.corner_radius.unwrap_or(0.0));
    let opacity = f64::from(style.opacity.unwrap_or(1.0));

    match &spec.kind {
        NodeKindSpec::Rect => {
            let background = required_background(spec, style, scheme, "Rect")?;
            Ok((
                NodeKind::Rect,
                PaintProperties::new(background, corner_radius, 0.0, opacity),
            ))
        }
        NodeKindSpec::Text => {
            let background = required_background(spec, style, scheme, "Text")?;
            let text_spec = spec.text.as_ref().ok_or_else(|| SpecError::MissingField {
                id: spec.id.clone(),
                kind: "Text",
                field: "text",
            })?;
            Ok((
                NodeKind::Text(TextState {
                    content: text_spec.content.clone(),
                    font_family: text_spec.font_family.clone(),
                    font_weight: text_spec.font_weight,
                    font_size: text_spec.font_size,
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
                Some(raw) => resolve_color(spec, raw, scheme)?,
                None => Color::from_rgba8(0, 0, 0, 0),
            };
            Ok((
                NodeKind::Container,
                PaintProperties::new(background, corner_radius, 0.0, opacity),
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
        Some(raw) => resolve_color(spec, raw, scheme),
        None => Err(SpecError::MissingField {
            id: spec.id.clone(),
            kind,
            field: "style.background",
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
fn resolve_color(
    spec: &WidgetSpec,
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
            value: raw.to_string(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use taffy::prelude::AvailableSpace;

    const VIEW: &str = r##"
id: root
kind: Container
style: {flex_direction: Row, padding: 10, gap: 5, width: 220, height: 100}
children:
  - id: swatch
    kind: Rect
    style: {width: 100, height: 80, background: "#6750A4", corner_radius: 8}
  - id: label
    kind: Text
    text: {content: "Hi", font_family: Roboto, font_size: 16}
    style: {width: 90, height: 30, background: white}
"##;

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
        let root = load_styled_view(&mut tree, yaml, &sheet, &theme.light)
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
        let root = load_styled_view(&mut tree, yaml, &sheet, &theme.light)
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
}
