//! `WidgetSpec` -> `engine_core::Tree` (§14 step 5). This is the crate's
//! actual reason to exist for this step: de-risk parsing/validation/
//! mapping in isolation, per Design Principle 5, before anything depends
//! on it working -- rendered through the real Phase 2 pipeline (steps
//! 1-4) unchanged, since a built `Tree` is indistinguishable here from
//! one built imperatively.

use engine_core::{NodeId, NodeKind, PaintProperties, TextState, Tree};
use peniko::Color;
use taffy::prelude::{Rect as TaffyRect, Size, Style, auto, length, zero};

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
/// root `NodeId`. The one-call convenience most callers want; `parse_view`
/// and `build_tree` (below) stay separately callable for anyone who
/// needs the intermediate `WidgetSpec` (a future reconciliation pass,
/// §16.4, diffs two of these before touching the `Tree` at all).
pub fn load_view(tree: &mut Tree, yaml: &str) -> Result<NodeId, SpecError> {
    let spec = parse_view(yaml)?;
    build_tree(tree, &spec)
}

/// Recursively inserts `spec` and its `children` into `tree`, wiring
/// each parent/child edge with `Tree::add_child` as it goes.
pub fn build_tree(tree: &mut Tree, spec: &WidgetSpec) -> Result<NodeId, SpecError> {
    let layout_style = layout_style(&spec.style);
    let (kind, paint) = node_kind_and_paint(spec)?;
    let id = tree.insert(kind, layout_style, paint);

    for child_spec in &spec.children {
        let child_id = build_tree(tree, child_spec)?;
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

fn node_kind_and_paint(spec: &WidgetSpec) -> Result<(NodeKind, PaintProperties), SpecError> {
    let corner_radius = f64::from(spec.style.corner_radius.unwrap_or(0.0));
    let opacity = f64::from(spec.style.opacity.unwrap_or(1.0));

    match &spec.kind {
        NodeKindSpec::Rect => {
            let background = required_background(spec, "Rect")?;
            Ok((
                NodeKind::Rect,
                PaintProperties::new(background, corner_radius, 0.0, opacity),
            ))
        }
        NodeKindSpec::Text => {
            let background = required_background(spec, "Text")?;
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
            let background = match &spec.style.background {
                Some(raw) => parse_background(spec, raw)?,
                None => Color::from_rgba8(0, 0, 0, 0),
            };
            Ok((
                NodeKind::Container,
                PaintProperties::new(background, corner_radius, 0.0, opacity),
            ))
        }
    }
}

fn required_background(spec: &WidgetSpec, kind: &'static str) -> Result<Color, SpecError> {
    match &spec.style.background {
        Some(raw) => parse_background(spec, raw),
        None => Err(SpecError::MissingField {
            id: spec.id.clone(),
            kind,
            field: "style.background",
        }),
    }
}

fn parse_background(spec: &WidgetSpec, raw: &str) -> Result<Color, SpecError> {
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
}
