//! M49 Phase 3: a *theme* document -- distinct from a `Stylesheet`
//! (`cascade.rs`), even though it shares that module's `StyleRule`
//! shape for its own `styles:` section. A `Stylesheet` is an app's own
//! widget-specific styling; a `ThemeSpec` is either the engine's shipped
//! default or a user's override layer, cascaded *beneath* the app's own
//! `Stylesheet` (`resolve_style_layered`, this same crate). Two real,
//! separate concerns in one document: `colors:` (role-name -> literal
//! color string overrides, applied to a `ColorScheme` via `engine_md3::
//! ColorScheme::apply_overrides`) and `styles:` (per-widget-kind default
//! `StyleSpec`s, the identical `StyleRule` shape `Stylesheet` already
//! uses -- `kind:`-only rules are the intended, recommended use, though
//! `classes:`/`id:` aren't rejected, since the type doesn't need a
//! second, narrower one just to enforce that convention).

use std::collections::HashMap;

use serde::Deserialize;

use crate::cascade::StyleRule;
use crate::spec::ShapeOrElevationSpec;

/// A parsed theme document: `{seed, dark, colors, styles}`, every field
/// optional so a theme can override just one thing (a single role, say)
/// while leaving everything else -- `deny_unknown_fields` for the same
/// "typo'd key is a load-time error" reasoning `WidgetSpec`/`Stylesheet`
/// already apply.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeSpec {
    /// A hex (`"#6750A4"`) or CSS-named seed color -- when present,
    /// this theme's own seed supersedes whatever seed the caller passed
    /// directly (the user's own stated "supersede the default
    /// properties they choose to while keeping other default parts").
    /// Parsed the same real hex/CSS parser every other color string in
    /// this codebase already uses, at the point this `ThemeSpec` is
    /// applied (`engine-py`), not here -- this struct stays literal
    /// `String` data, matching `StyleSpec.background`'s own established
    /// "parsed at apply time, not parse time" precedent.
    #[serde(default)]
    pub seed: Option<String>,
    #[serde(default)]
    pub dark: Option<bool>,
    /// Role name -> hex/CSS color string. Applied via `engine_md3::
    /// ColorScheme::apply_overrides` -- reaches both the declarative
    /// YAML path and the imperative MD3 component catalog, since both
    /// already resolve colors through `ColorScheme::role`.
    #[serde(default)]
    pub colors: HashMap<String, String>,
    /// Per-widget-kind default styles -- the identical `StyleRule`
    /// shape `Stylesheet.styles` already uses, cascaded independently
    /// (`resolve_style_within_sheet`) before being layered under the
    /// app's own `Stylesheet` and the widget's own inline `style:`
    /// (`resolve_style_layered`).
    #[serde(default)]
    pub styles: Vec<StyleRule>,
    /// M50: shape/elevation overrides for the *imperative* MD3 catalog
    /// (`Window.add_button`/`add_fab`/etc., `engine-py::window_
    /// factory.rs`) -- a deliberately separate namespace from `styles:`
    /// above, which only ever reaches the 7 declarative `NodeKindSpec`
    /// kinds. Keyed by `"<component>"` (the factory name minus `add_`,
    /// e.g. `"card"`, applies regardless of variant) or
    /// `"<component>.<variant>"` (e.g. `"fab.small"`, overrides just
    /// that variant) -- resolved with a real 2-tier lookup (`engine-py::
    /// window::ThemeState::shape`/`elevation`), not here: this struct
    /// stays plain data, matching every other `ThemeSpec` field's own
    /// "parsed at apply time, not parse time" precedent. `View` never
    /// consults this field (it has no imperative factories) -- present
    /// in the shared struct so one theme file can serve both surfaces.
    #[serde(default)]
    pub components: HashMap<String, ComponentOverride>,
    /// M62 Phase 3 (§7.1, §16.3): per-role overrides for `engine_md3::
    /// typography`'s own real, shipped MD3 type scale -- keyed by one
    /// of its 15 real role names (`"body_large"`, `"headline_small"`,
    /// etc.), the identical real "parse-time data, resolved at apply
    /// time" precedent `components:` above already establishes. A given
    /// role's own shipped default (`engine_md3::type_style_named`)
    /// still applies to every field an override here leaves unset --
    /// each `TypographyOverride` field is independently optional so a
    /// theme can, say, only widen `body_large`'s own `font_family`
    /// without touching its real MD3 size/weight/line-height at all.
    #[serde(default)]
    pub typography: HashMap<String, TypographyOverride>,
}

/// M50: one imperative MD3 component's shape/elevation override --
/// both fields optional so a theme can set just one (e.g. only
/// `corner_radius`) while leaving the other at its existing hardcoded
/// default, the same per-field-optional shape `StyleSpec` already
/// establishes. M61 (§16.3): both widened to `crate::spec::
/// ShapeOrElevationSpec` (a literal or a named token, e.g.
/// `corner_radius: small`) -- no longer `Copy` (a `TokenRef` carries a
/// real owned `String`), resolved into plain `f64`s once, at real
/// theme-load time (`engine-py::window.rs`'s `Window.set_theme`), not
/// here -- this struct stays parse-time data, matching every other
/// `ThemeSpec` field's own "parsed at apply time, not parse time"
/// precedent.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentOverride {
    #[serde(default)]
    pub corner_radius: Option<ShapeOrElevationSpec>,
    #[serde(default)]
    pub elevation: Option<ShapeOrElevationSpec>,
}

/// M62 Phase 3 (§7.1, §16.3): one MD3 type-scale role's own real
/// override -- every field optional, the same per-field-optional shape
/// `ComponentOverride` above already establishes, so a theme can widen
/// just one real attribute of a role (e.g. only `font_size`) while
/// leaving the rest at `engine_md3::type_style_named`'s own shipped
/// default. Resolution (start from the named role's real default, then
/// apply whichever of these 4 fields are `Some`) happens at apply time
/// (Phase 4), not here -- this struct stays plain parse-time data.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypographyOverride {
    #[serde(default)]
    pub font_family: Option<String>,
    #[serde(default)]
    pub font_weight: Option<f32>,
    #[serde(default)]
    pub font_size: Option<f32>,
    #[serde(default)]
    pub line_height: Option<f32>,
}

/// Parses a theme document -- mirrors `cascade::parse_stylesheet`'s own
/// exact shape (a thin `serde_yaml_ng::from_str` wrapper, the parse
/// error itself already carries a real line/column).
pub fn parse_theme(yaml: &str) -> Result<ThemeSpec, serde_yaml_ng::Error> {
    serde_yaml_ng::from_str(yaml)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::NodeKindSpec;

    #[test]
    fn an_empty_theme_document_parses_with_every_field_at_its_default() {
        let theme = parse_theme("{}").unwrap();
        assert_eq!(theme.seed, None);
        assert_eq!(theme.dark, None);
        assert!(theme.colors.is_empty());
        assert!(theme.styles.is_empty());
    }

    #[test]
    fn a_full_theme_document_parses_every_field() {
        let theme = parse_theme(
            r##"
seed: "#6750A4"
dark: true
colors:
  primary: "#FF0000"
styles:
  - kind: Checkbox
    style: {corner_radius: 2}
"##,
        )
        .unwrap();
        assert_eq!(theme.seed.as_deref(), Some("#6750A4"));
        assert_eq!(theme.dark, Some(true));
        assert_eq!(
            theme.colors.get("primary").map(String::as_str),
            Some("#FF0000")
        );
        assert_eq!(theme.styles.len(), 1);
        assert_eq!(theme.styles[0].kind, Some(NodeKindSpec::Checkbox));
    }

    #[test]
    fn an_unknown_top_level_key_is_a_clear_load_time_error() {
        let err = parse_theme("nope: true").unwrap_err();
        assert!(err.to_string().contains("nope"));
    }

    // --- M50: components: ---

    #[test]
    fn components_section_parses_bare_and_variant_keys() {
        let theme = parse_theme(
            "components:\n  card: {corner_radius: 16}\n  fab.small: {corner_radius: 12, elevation: 2}\n",
        )
        .unwrap();
        assert_eq!(theme.components.len(), 2);
        assert_eq!(
            theme.components["card"],
            ComponentOverride {
                corner_radius: Some(ShapeOrElevationSpec::Literal(16.0)),
                elevation: None,
            }
        );
        assert_eq!(
            theme.components["fab.small"],
            ComponentOverride {
                corner_radius: Some(ShapeOrElevationSpec::Literal(12.0)),
                elevation: Some(ShapeOrElevationSpec::Literal(2.0)),
            }
        );
    }

    #[test]
    fn an_empty_theme_documents_components_section_is_empty() {
        let theme = parse_theme("{}").unwrap();
        assert!(theme.components.is_empty());
    }

    #[test]
    fn a_component_override_with_an_unknown_field_is_a_clear_error() {
        let err = parse_theme("components:\n  card: {not_a_real_field: 1}\n").unwrap_err();
        assert!(err.to_string().contains("not_a_real_field"));
    }

    // --- M62 Phase 3: typography: ---

    #[test]
    fn typography_section_parses_a_real_role_override_with_some_fields_set() {
        let theme = parse_theme(
            "typography:\n  body_large: {font_family: Inter}\n  headline_small: {font_size: 26, line_height: 1.4}\n",
        )
        .unwrap();
        assert_eq!(theme.typography.len(), 2);
        assert_eq!(
            theme.typography["body_large"],
            TypographyOverride {
                font_family: Some("Inter".to_string()),
                font_weight: None,
                font_size: None,
                line_height: None,
            }
        );
        assert_eq!(
            theme.typography["headline_small"],
            TypographyOverride {
                font_family: None,
                font_weight: None,
                font_size: Some(26.0),
                line_height: Some(1.4),
            }
        );
    }

    #[test]
    fn an_empty_theme_documents_typography_section_is_empty() {
        let theme = parse_theme("{}").unwrap();
        assert!(theme.typography.is_empty());
    }

    #[test]
    fn a_typography_override_with_an_unknown_field_is_a_clear_error() {
        let err = parse_theme("typography:\n  body_large: {not_a_real_field: 1}\n").unwrap_err();
        assert!(err.to_string().contains("not_a_real_field"));
    }
}
