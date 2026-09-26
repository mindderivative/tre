//! M98: the theme documents `Window.set_theme` reads -- a YAML file
//! (`default_theme=`/`custom_theme=`) or a dict (`*_spec=`) -- moved here
//! from the deleted `engine-spec` crate, which `View` shared them with.
//! Transitional: M99 removes `set_theme` with the MD3 widgets it themes,
//! and this module, `assets/default_theme.yaml`, `serde_yaml_ng`, and
//! `pythonize` with it.

use std::collections::HashMap;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use serde::Deserialize;
use serde::de::IgnoredAny;

/// The engine's own default theme, embedded at compile time -- what an
/// omitted `default_theme`/`default_theme_spec` means.
const SHIPPED_DEFAULT_THEME_YAML: &str = include_str!("../assets/default_theme.yaml");

/// One theme document.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ThemeSpec {
    #[serde(default)]
    pub seed: Option<String>,
    /// Role overrides, applied to both the light and the dark scheme.
    #[serde(default)]
    pub colors: HashMap<String, String>,
    /// `View`'s own fields -- a light/dark choice and a stylesheet.
    /// `Window.set_theme` never read either (it takes `dark` as an
    /// argument), and `View` is gone; still accepted, so theme files load.
    #[serde(default)]
    #[allow(dead_code)]
    pub dark: IgnoredAny,
    #[serde(default)]
    #[allow(dead_code)]
    pub styles: IgnoredAny,
    /// `component` or `component.variant` -> its corner radius/elevation.
    #[serde(default)]
    pub components: HashMap<String, ComponentOverride>,
    /// Type-scale role -> its overridden fields.
    #[serde(default)]
    pub typography: HashMap<String, TypographyOverride>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ComponentOverride {
    #[serde(default)]
    pub corner_radius: Option<ShapeOrElevationSpec>,
    #[serde(default)]
    pub elevation: Option<ShapeOrElevationSpec>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TypographyOverride {
    #[serde(default)]
    pub font_family: Option<String>,
    #[serde(default)]
    pub font_weight: Option<f32>,
    #[serde(default)]
    pub font_size: Option<f32>,
    #[serde(default)]
    pub line_height: Option<f32>,
}

/// A corner radius or elevation: a number, or an MD3 token name
/// (`medium`, `level_3`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub(crate) enum ShapeOrElevationSpec {
    Literal(f64),
    TokenRef(String),
}

impl ShapeOrElevationSpec {
    /// The value, looking a token up in the elevation scale or the shape
    /// scale; `None` for an unknown token.
    pub(crate) fn resolve(&self, is_elevation: bool) -> Option<f64> {
        match self {
            Self::Literal(v) => Some(*v),
            Self::TokenRef(name) if is_elevation => engine_md3::elevation_named(name),
            Self::TokenRef(name) => engine_md3::named(name),
        }
    }
}

fn parse_theme(yaml: &str) -> Result<ThemeSpec, serde_yaml_ng::Error> {
    serde_yaml_ng::from_str(yaml)
}

/// The shipped default theme, parsed.
pub(crate) fn shipped_default_theme_spec() -> ThemeSpec {
    parse_theme(SHIPPED_DEFAULT_THEME_YAML)
        .expect("the engine's own shipped default_theme.yaml must always parse")
}

/// A theme argument pair -- a file path or a dict -- as a `ThemeSpec`;
/// `None` when neither was given, a `ValueError` when both were.
pub(crate) fn resolve_theme_input(
    py: Python<'_>,
    method: &str,
    (path_kwarg, path): (&str, Option<&str>),
    (spec_kwarg, spec): (&str, Option<&Py<PyAny>>),
) -> PyResult<Option<ThemeSpec>> {
    match (path, spec) {
        (Some(_), Some(_)) => Err(PyValueError::new_err(format!(
            "{method}() takes at most one of {path_kwarg}, {spec_kwarg} -- \
             pass one real content source, not {path_kwarg} and {spec_kwarg}"
        ))),
        (Some(path), None) => {
            let yaml = std::fs::read_to_string(path).map_err(|e| {
                PyRuntimeError::new_err(format!("failed to read theme {path:?}: {e}"))
            })?;
            parse_theme(&yaml)
                .map(Some)
                .map_err(|e| PyValueError::new_err(e.to_string()))
        }
        (None, Some(obj)) => pythonize::depythonize(obj.bind(py))
            .map(Some)
            .map_err(|e| PyValueError::new_err(format!("{spec_kwarg}: {e}"))),
        (None, None) => Ok(None),
    }
}

/// A theme's own `seed:`, parsed as a hex or CSS-named color.
pub(crate) fn theme_spec_seed(theme: &ThemeSpec) -> PyResult<Option<(u8, u8, u8, u8)>> {
    theme
        .seed
        .as_deref()
        .map(|raw| {
            let color = peniko::color::parse_color(raw)
                .map(|c| c.to_alpha_color::<peniko::color::Srgb>())
                .map_err(|e| {
                    PyValueError::new_err(format!(
                        "theme seed color: {raw:?} isn't a valid color: {e}"
                    ))
                })?;
            let [r, g, b, a] = color.to_rgba8().to_u8_array();
            Ok((r, g, b, a))
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_default_theme_parses() {
        let theme = shipped_default_theme_spec();
        assert!(theme.components.contains_key("card"));
        assert_eq!(theme.seed, None);
    }

    #[test]
    fn a_full_theme_parses_every_field_and_still_accepts_styles() {
        let theme = parse_theme(
            "seed: \"#6750A4\"\ndark: true\ncolors: {primary: \"#FF0000\"}\n\
             styles: [{kind: Checkbox, style: {corner_radius: 2}}]\n\
             components:\n  card: {corner_radius: medium}\n  fab.small: {elevation: 2}\n\
             typography:\n  body_large: {font_size: 18}\n",
        )
        .unwrap();
        assert_eq!(theme.seed.as_deref(), Some("#6750A4"));
        assert_eq!(theme.colors["primary"], "#FF0000");
        let card = theme.components["card"].corner_radius.as_ref().unwrap();
        assert_eq!(card.resolve(false), Some(12.0));
        let fab = theme.components["fab.small"].elevation.as_ref().unwrap();
        assert_eq!(fab.resolve(true), Some(2.0));
        assert_eq!(theme.typography["body_large"].font_size, Some(18.0));
    }

    #[test]
    fn an_unknown_key_or_token_is_caught() {
        assert!(parse_theme("nope: true").is_err());
        assert!(parse_theme("components:\n  card: {nope: 1}\n").is_err());
        let token = ShapeOrElevationSpec::TokenRef("huge".into());
        assert_eq!(token.resolve(false), None);
    }
}
