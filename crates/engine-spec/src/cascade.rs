//! §16.3's stylesheet cascade (§14 step 12, Stage A). Precedence,
//! borrowed directly from `pyCopper`'s own already-tested ordering:
//! baseline (no selector) → `kind:` → `classes:` (more classes beat
//! fewer) → `id:` → the node's own inline `style:`. Resolved once, when
//! a view is loaded or reconciled -- never per frame, so it costs
//! nothing against §6's frame budget.
//!
//! Selectors are structured fields (`kind`/`classes`/`id`), not
//! CSS-like selector strings -- §16.3's own reasoning: a bare `#id`
//! string would need quoting in YAML (`#` opens a comment).

use serde::Deserialize;

use crate::spec::{NodeKindSpec, StyleSpec, WidgetSpec};

/// A parsed stylesheet document: `{styles: [...]}` -- the exact shape
/// §16.3's own example shows, `deny_unknown_fields` for the same
/// "typo'd key is a load-time error" reasoning `WidgetSpec` already
/// applies (§16.1).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stylesheet {
    #[serde(default)]
    pub styles: Vec<StyleRule>,
}

/// One cascade rule: a selector (`kind`/`classes`/`id`, any or none of
/// them set) plus the `StyleSpec` it contributes when it matches. A
/// rule with no selector fields set at all is the "baseline" tier --
/// it matches every widget.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StyleRule {
    #[serde(default)]
    pub kind: Option<NodeKindSpec>,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub id: Option<String>,
    pub style: StyleSpec,
}

pub fn parse_stylesheet(yaml: &str) -> Result<Stylesheet, serde_yaml_ng::Error> {
    serde_yaml_ng::from_str(yaml)
}

/// Resolves `spec`'s final `StyleSpec` by applying every matching rule
/// in `sheet`, in §16.3's precedence order, then the widget's own
/// inline `style:` last. Each tier does a per-field merge (a field
/// already set by an earlier, lower-precedence tier survives if a
/// later tier leaves it unset) -- not a whole-struct replacement, which
/// is what makes "cascade" a meaningful word here rather than "last
/// writer wins entirely."
pub fn resolve_style(spec: &WidgetSpec, sheet: &Stylesheet) -> StyleSpec {
    let mut resolved = StyleSpec::default();

    // Baseline: a rule naming no selector at all applies to everything.
    for rule in &sheet.styles {
        if rule.kind.is_none() && rule.classes.is_empty() && rule.id.is_none() {
            merge(&mut resolved, &rule.style);
        }
    }

    // kind:
    for rule in &sheet.styles {
        if rule.kind == Some(spec.kind) {
            merge(&mut resolved, &rule.style);
        }
    }

    // classes: -- a rule matches if every class it names is present on
    // the widget (a subset match, the usual multi-class-selector
    // meaning). Applied in ascending order by how many classes each
    // rule names, so a rule matching more of the widget's classes is
    // strictly more specific and is applied last -- it wins any
    // conflict with a less-specific class rule, per §16.3's own "more
    // classes beat fewer" text.
    let mut class_rules: Vec<&StyleRule> = sheet
        .styles
        .iter()
        .filter(|rule| {
            !rule.classes.is_empty() && rule.classes.iter().all(|c| spec.classes.contains(c))
        })
        .collect();
    class_rules.sort_by_key(|rule| rule.classes.len());
    for rule in class_rules {
        merge(&mut resolved, &rule.style);
    }

    // id:
    for rule in &sheet.styles {
        if rule.id.as_deref() == Some(spec.id.as_str()) {
            merge(&mut resolved, &rule.style);
        }
    }

    // The widget's own inline style -- highest precedence of all.
    merge(&mut resolved, &spec.style);

    resolved
}

fn merge(base: &mut StyleSpec, overlay: &StyleSpec) {
    if overlay.width.is_some() {
        base.width = overlay.width;
    }
    if overlay.height.is_some() {
        base.height = overlay.height;
    }
    if overlay.flex_direction.is_some() {
        base.flex_direction = overlay.flex_direction;
    }
    if overlay.padding.is_some() {
        base.padding = overlay.padding;
    }
    if overlay.gap.is_some() {
        base.gap = overlay.gap;
    }
    if overlay.background.is_some() {
        base.background.clone_from(&overlay.background);
    }
    if overlay.corner_radius.is_some() {
        base.corner_radius = overlay.corner_radius;
    }
    if overlay.opacity.is_some() {
        base.opacity = overlay.opacity;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn widget(id: &str, kind: NodeKindSpec, classes: &[&str]) -> WidgetSpec {
        WidgetSpec {
            id: id.to_string(),
            kind,
            classes: classes.iter().map(|s| s.to_string()).collect(),
            style: StyleSpec::default(),
            text: None,
            checked: false,
            value: 0.0,
            image: None,
            bindings: std::collections::HashMap::new(),
            handlers: std::collections::HashMap::new(),
            two_way: None,
            children: Vec::new(),
        }
    }

    #[test]
    fn baseline_rule_applies_to_every_widget() {
        let sheet = parse_stylesheet("styles:\n  - style: {opacity: 0.9}\n").unwrap();
        let resolved = resolve_style(&widget("any", NodeKindSpec::Rect, &[]), &sheet);
        assert_eq!(resolved.opacity, Some(0.9));
    }

    #[test]
    fn kind_rule_overrides_baseline() {
        let sheet = parse_stylesheet(
            "styles:\n  - style: {corner_radius: 4}\n  - kind: Rect\n    style: {corner_radius: 20}\n",
        )
        .unwrap();
        let resolved = resolve_style(&widget("r", NodeKindSpec::Rect, &[]), &sheet);
        assert_eq!(resolved.corner_radius, Some(20.0));

        // A Text widget doesn't match the `kind: Rect` rule -- baseline still applies.
        let resolved_text = resolve_style(&widget("t", NodeKindSpec::Text, &[]), &sheet);
        assert_eq!(resolved_text.corner_radius, Some(4.0));
    }

    #[test]
    fn more_specific_class_rule_wins_over_a_less_specific_one() {
        let sheet = parse_stylesheet(
            "styles:\n\
             - classes: [primary]\n  style: {corner_radius: 8}\n\
             - classes: [primary, large]\n  style: {corner_radius: 24}\n",
        )
        .unwrap();
        let resolved = resolve_style(
            &widget("btn", NodeKindSpec::Rect, &["primary", "large"]),
            &sheet,
        );
        assert_eq!(
            resolved.corner_radius,
            Some(24.0),
            "the two-class rule is more specific and must win"
        );

        // A widget with only `primary` doesn't match the two-class rule at all.
        let resolved_small =
            resolve_style(&widget("btn2", NodeKindSpec::Rect, &["primary"]), &sheet);
        assert_eq!(resolved_small.corner_radius, Some(8.0));
    }

    #[test]
    fn id_rule_overrides_kind_and_class_rules() {
        let sheet = parse_stylesheet(
            "styles:\n\
             - kind: Rect\n  style: {opacity: 0.5}\n\
             - id: special\n  style: {opacity: 1.0}\n",
        )
        .unwrap();
        let resolved = resolve_style(&widget("special", NodeKindSpec::Rect, &[]), &sheet);
        assert_eq!(resolved.opacity, Some(1.0));
    }

    #[test]
    fn inline_style_wins_over_every_stylesheet_rule() {
        let sheet = parse_stylesheet("styles:\n  - id: w\n    style: {opacity: 1.0}\n").unwrap();
        let mut w = widget("w", NodeKindSpec::Rect, &[]);
        w.style.opacity = Some(0.2);
        let resolved = resolve_style(&w, &sheet);
        assert_eq!(resolved.opacity, Some(0.2));
    }

    #[test]
    fn unset_fields_fall_through_the_cascade_instead_of_being_clobbered() {
        // The kind rule sets corner_radius only; the id rule sets
        // opacity only. Both must survive in the final resolved style
        // -- this is the actual "cascade" claim, not "last match wins
        // entirely."
        let sheet = parse_stylesheet(
            "styles:\n\
             - kind: Rect\n  style: {corner_radius: 12}\n\
             - id: w\n  style: {opacity: 0.7}\n",
        )
        .unwrap();
        let resolved = resolve_style(&widget("w", NodeKindSpec::Rect, &[]), &sheet);
        assert_eq!(resolved.corner_radius, Some(12.0));
        assert_eq!(resolved.opacity, Some(0.7));
    }
}
