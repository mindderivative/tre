//! M62 Phase 2 (§7.1, §16.3): MD3's real, published type scale,
//! centralized for the first time -- the identical real "give it one
//! clean home" move `shape.rs` itself made for corner-radius/elevation
//! at M49, mirrored here for typography. Before this, these exact real
//! numbers existed only as ~40 scattered, independently-named per-
//! factory constants across `engine-py::window_factory.rs` (e.g.
//! `BUTTON_LABEL_FONT_SIZE: f32 = 14.0` / `BUTTON_LABEL_FONT_WEIGHT: f32
//! = 500.0`, confirmed via direct grep before writing this module, not
//! assumed) -- exactly the "pre-centralization" state `shape.rs`'s own
//! doc comment describes `window_factory.rs`'s shape constants having
//! been in before M49. **Data only, this phase:** `window_factory.rs`'s
//! own existing constants are left exactly as they are -- migrating
//! individual factories to reference these instead is Phase 4's real,
//! separate job (`TextSpec`/imperative role-reference wiring), not
//! attempted here.
//!
//! **Real values, verified against a real published source, not
//! recalled from memory:** fetched directly from Flutter's own
//! `packages/flutter/lib/src/material/typography.dart`
//! (`_M3Typography.englishLike`, the real Material 3 (2021) English-like
//! type scale Flutter ships) -- chosen over trying to scrape
//! `m3.material.io`'s own JS-rendered page (which returned no literal
//! numbers) or `material-web`'s `_md-sys-typescale.scss` (which only
//! references computed values, not literals). Flutter's own source is a
//! second, independent, already-shipped implementation of the identical
//! real MD3 spec, the same "trust a real implementation over a
//! description" reasoning this codebase already applies elsewhere.
//! `family` is `"Roboto"` for every role -- MD3's own real default type
//! scale is one family throughout (Flutter's own `englishLike` maps to
//! its "Plain" baseline, not a `"Brand"` variant), and matches this
//! codebase's own pre-existing, unconditional `"Roboto"` default at
//! every real text-creating call site (confirmed via grep), so this
//! phase introduces no naming split that didn't already exist.

/// One MD3 type-scale role's real, resolved values -- everything a
/// `TextState` needs except `content`/`align` (both genuinely per-node,
/// never role data). `line_height` is a font-size-relative multiplier
/// (`TextState.line_height`'s own real shape, M62 Phase 1), computed
/// from Flutter's own real `height` field (already expressed as a
/// ratio, not an absolute sp value) -- the identical real number, not
/// re-derived.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypeStyle {
    pub font_family: &'static str,
    pub font_weight: f32,
    pub font_size: f32,
    pub line_height: f32,
}

pub const DISPLAY_LARGE: TypeStyle = TypeStyle {
    font_family: "Roboto",
    font_weight: 400.0,
    font_size: 57.0,
    line_height: 1.12,
};
pub const DISPLAY_MEDIUM: TypeStyle = TypeStyle {
    font_family: "Roboto",
    font_weight: 400.0,
    font_size: 45.0,
    line_height: 1.16,
};
pub const DISPLAY_SMALL: TypeStyle = TypeStyle {
    font_family: "Roboto",
    font_weight: 400.0,
    font_size: 36.0,
    line_height: 1.22,
};
pub const HEADLINE_LARGE: TypeStyle = TypeStyle {
    font_family: "Roboto",
    font_weight: 400.0,
    font_size: 32.0,
    line_height: 1.25,
};
pub const HEADLINE_MEDIUM: TypeStyle = TypeStyle {
    font_family: "Roboto",
    font_weight: 400.0,
    font_size: 28.0,
    line_height: 1.29,
};
pub const HEADLINE_SMALL: TypeStyle = TypeStyle {
    font_family: "Roboto",
    font_weight: 400.0,
    font_size: 24.0,
    line_height: 1.33,
};
pub const TITLE_LARGE: TypeStyle = TypeStyle {
    font_family: "Roboto",
    font_weight: 400.0,
    font_size: 22.0,
    line_height: 1.27,
};
pub const TITLE_MEDIUM: TypeStyle = TypeStyle {
    font_family: "Roboto",
    font_weight: 500.0,
    font_size: 16.0,
    line_height: 1.50,
};
pub const TITLE_SMALL: TypeStyle = TypeStyle {
    font_family: "Roboto",
    font_weight: 500.0,
    font_size: 14.0,
    line_height: 1.43,
};
pub const BODY_LARGE: TypeStyle = TypeStyle {
    font_family: "Roboto",
    font_weight: 400.0,
    font_size: 16.0,
    line_height: 1.50,
};
pub const BODY_MEDIUM: TypeStyle = TypeStyle {
    font_family: "Roboto",
    font_weight: 400.0,
    font_size: 14.0,
    line_height: 1.43,
};
pub const BODY_SMALL: TypeStyle = TypeStyle {
    font_family: "Roboto",
    font_weight: 400.0,
    font_size: 12.0,
    line_height: 1.33,
};
pub const LABEL_LARGE: TypeStyle = TypeStyle {
    font_family: "Roboto",
    font_weight: 500.0,
    font_size: 14.0,
    line_height: 1.43,
};
pub const LABEL_MEDIUM: TypeStyle = TypeStyle {
    font_family: "Roboto",
    font_weight: 500.0,
    font_size: 12.0,
    line_height: 1.33,
};
pub const LABEL_SMALL: TypeStyle = TypeStyle {
    font_family: "Roboto",
    font_weight: 500.0,
    font_size: 11.0,
    line_height: 1.45,
};

/// The real name a theme YAML author (or an imperative `typography_role=`
/// kwarg, Phase 4) writes to reference one of the 15 roles above by
/// name -- lowercase-snake-case, matching this codebase's own established
/// runtime-string-lookup convention (`engine_md3::shape::named`'s own
/// identical real shape). `None` for an unrecognized name -- the caller
/// turns that into its own real, crate-appropriate error, never a
/// silent fallback to some arbitrary role.
pub fn type_style_named(name: &str) -> Option<TypeStyle> {
    match name {
        "display_large" => Some(DISPLAY_LARGE),
        "display_medium" => Some(DISPLAY_MEDIUM),
        "display_small" => Some(DISPLAY_SMALL),
        "headline_large" => Some(HEADLINE_LARGE),
        "headline_medium" => Some(HEADLINE_MEDIUM),
        "headline_small" => Some(HEADLINE_SMALL),
        "title_large" => Some(TITLE_LARGE),
        "title_medium" => Some(TITLE_MEDIUM),
        "title_small" => Some(TITLE_SMALL),
        "body_large" => Some(BODY_LARGE),
        "body_medium" => Some(BODY_MEDIUM),
        "body_small" => Some(BODY_SMALL),
        "label_large" => Some(LABEL_LARGE),
        "label_medium" => Some(LABEL_MEDIUM),
        "label_small" => Some(LABEL_SMALL),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Not a tautology -- catches an accidental typo'd/reordered value
    /// (e.g. `TITLE_MEDIUM`/`BODY_LARGE`'s real, deliberate size
    /// coincidence swapped for the wrong role) by asserting the real,
    /// known MD3-published relative ordering holds across the display/
    /// headline/title tiers (each tier's own "large" role is always its
    /// own tier's biggest size; each tier is smaller than the tier above
    /// it).
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn the_display_headline_title_tiers_are_strictly_decreasing() {
        assert!(DISPLAY_LARGE.font_size > DISPLAY_MEDIUM.font_size);
        assert!(DISPLAY_MEDIUM.font_size > DISPLAY_SMALL.font_size);
        assert!(DISPLAY_SMALL.font_size > HEADLINE_LARGE.font_size);
        assert!(HEADLINE_LARGE.font_size > HEADLINE_MEDIUM.font_size);
        assert!(HEADLINE_MEDIUM.font_size > HEADLINE_SMALL.font_size);
        assert!(HEADLINE_SMALL.font_size > TITLE_LARGE.font_size);
    }

    /// `title`/`body`/`label` -- MD3's real "medium" weight (500) tiers
    /// -- must actually differ from the "regular" (400) tiers by real
    /// weight, not just size, the real distinguishing feature between
    /// e.g. `title_medium` (16sp/500) and `body_large` (16sp/400)
    /// sharing the identical real font size.
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn title_medium_and_body_large_share_a_size_but_real_different_weights() {
        assert_eq!(TITLE_MEDIUM.font_size, BODY_LARGE.font_size);
        assert_ne!(TITLE_MEDIUM.font_weight, BODY_LARGE.font_weight);
        assert_eq!(TITLE_MEDIUM.font_weight, 500.0);
        assert_eq!(BODY_LARGE.font_weight, 400.0);
    }

    #[test]
    fn type_style_named_resolves_every_real_role_to_its_own_real_constant() {
        assert_eq!(type_style_named("display_large"), Some(DISPLAY_LARGE));
        assert_eq!(type_style_named("display_medium"), Some(DISPLAY_MEDIUM));
        assert_eq!(type_style_named("display_small"), Some(DISPLAY_SMALL));
        assert_eq!(type_style_named("headline_large"), Some(HEADLINE_LARGE));
        assert_eq!(type_style_named("headline_medium"), Some(HEADLINE_MEDIUM));
        assert_eq!(type_style_named("headline_small"), Some(HEADLINE_SMALL));
        assert_eq!(type_style_named("title_large"), Some(TITLE_LARGE));
        assert_eq!(type_style_named("title_medium"), Some(TITLE_MEDIUM));
        assert_eq!(type_style_named("title_small"), Some(TITLE_SMALL));
        assert_eq!(type_style_named("body_large"), Some(BODY_LARGE));
        assert_eq!(type_style_named("body_medium"), Some(BODY_MEDIUM));
        assert_eq!(type_style_named("body_small"), Some(BODY_SMALL));
        assert_eq!(type_style_named("label_large"), Some(LABEL_LARGE));
        assert_eq!(type_style_named("label_medium"), Some(LABEL_MEDIUM));
        assert_eq!(type_style_named("label_small"), Some(LABEL_SMALL));
        assert_eq!(
            type_style_named("subtitle_large"),
            None,
            "an unrecognized role name must not silently resolve"
        );
    }
}
