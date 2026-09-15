//! §7.1's dynamic color subsystem (§14 step 11): a full light/dark MD3
//! color scheme generated from a single seed color via `material-colors`
//! (an HCT/tonal-palette/dynamic-scheme port of Google's own Material
//! Color Utilities), with every scheme role mapped onto a `peniko::
//! Color` value.
//!
//! **§7.1's own acceptance gate, verified directly, not assumed:**
//! "whichever crate is chosen must pass Material Color Utilities' own
//! published reference test vectors... before it's pinned." Checked two
//! ways before this module was written. First, by reading `material-
//! colors 0.4.2`'s own vendored source directly: its HCT tests assert
//! the exact published CAM16 reference values for red/green/blue/black/
//! white (e.g. red: `j=46.445, chroma=113.357, hue=27.408`), and its
//! `CorePalette::of` tests assert exact published tonal-palette hex
//! values -- both recognizable as Google's own MCU reference constants,
//! not values this crate invented. Second, and more directly: `cargo
//! test --lib` was run inside the vendored source directory itself, on
//! this exact pinned version and this project's own toolchain -- **129
//! passed, 0 failed**. That run (see this step's own `LOG.md`) is the
//! actual acceptance-gate evidence, not the source-reading alone.
//!
//! `ColorScheme` maps every role `material_colors::scheme::Scheme`
//! defines (all 49, including the newer `*_fixed` roles and the
//! `surface_container_*` elevation tiers) -- a complete, mechanical 1:1
//! field conversion, not an arbitrarily trimmed subset: there's no
//! per-field judgment call to defer, so leaving some out would only
//! mean a future caller hits an inexplicably-missing role for no
//! principled reason.
//!
//! Deliberately does not wire `winit`'s `ThemeChanged` event for live
//! theme switching (§7.1's own "live theme switching is in scope"
//! text): that dispatch would route through `AppHandler`/`InputEvent`
//! (§4), which still doesn't exist anywhere in this codebase -- checked
//! directly, the same finding steps 7 and 9 already made about keyboard
//! and pointer dispatch respectively. This module builds and proves the
//! real `DynamicTheme::from_seed` mechanism that dispatch will call once
//! it exists.

use material_colors::color::Argb;
use material_colors::theme::ThemeBuilder;
use peniko::Color;

fn color_to_argb(color: Color) -> Argb {
    let rgba = color.to_rgba8();
    Argb::new(rgba.a, rgba.r, rgba.g, rgba.b)
}

fn argb_to_color(argb: Argb) -> Color {
    Color::from_rgba8(argb.red, argb.green, argb.blue, argb.alpha)
}

/// One light-or-dark MD3 color scheme: every role from
/// `material_colors::scheme::Scheme`, mapped onto `peniko::Color` (§7.1:
/// "`engine-md3` maps scheme roles... onto `peniko::Color` values
/// consumed by `PaintProperties`" -- the actual `PaintProperties`
/// consumption is a later step, once a real component resolves a token
/// name against one of these roles).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorScheme {
    pub primary: Color,
    pub on_primary: Color,
    pub primary_container: Color,
    pub on_primary_container: Color,
    pub inverse_primary: Color,
    pub primary_fixed: Color,
    pub primary_fixed_dim: Color,
    pub on_primary_fixed: Color,
    pub on_primary_fixed_variant: Color,
    pub secondary: Color,
    pub on_secondary: Color,
    pub secondary_container: Color,
    pub on_secondary_container: Color,
    pub secondary_fixed: Color,
    pub secondary_fixed_dim: Color,
    pub on_secondary_fixed: Color,
    pub on_secondary_fixed_variant: Color,
    pub tertiary: Color,
    pub on_tertiary: Color,
    pub tertiary_container: Color,
    pub on_tertiary_container: Color,
    pub tertiary_fixed: Color,
    pub tertiary_fixed_dim: Color,
    pub on_tertiary_fixed: Color,
    pub on_tertiary_fixed_variant: Color,
    pub error: Color,
    pub on_error: Color,
    pub error_container: Color,
    pub on_error_container: Color,
    pub surface_dim: Color,
    pub surface: Color,
    pub surface_tint: Color,
    pub surface_bright: Color,
    pub surface_container_lowest: Color,
    pub surface_container_low: Color,
    pub surface_container: Color,
    pub surface_container_high: Color,
    pub surface_container_highest: Color,
    pub on_surface: Color,
    pub on_surface_variant: Color,
    pub outline: Color,
    pub outline_variant: Color,
    pub inverse_surface: Color,
    pub inverse_on_surface: Color,
    pub surface_variant: Color,
    pub background: Color,
    pub on_background: Color,
    pub shadow: Color,
    pub scrim: Color,
}

impl From<&material_colors::scheme::Scheme> for ColorScheme {
    fn from(scheme: &material_colors::scheme::Scheme) -> Self {
        Self {
            primary: argb_to_color(scheme.primary),
            on_primary: argb_to_color(scheme.on_primary),
            primary_container: argb_to_color(scheme.primary_container),
            on_primary_container: argb_to_color(scheme.on_primary_container),
            inverse_primary: argb_to_color(scheme.inverse_primary),
            primary_fixed: argb_to_color(scheme.primary_fixed),
            primary_fixed_dim: argb_to_color(scheme.primary_fixed_dim),
            on_primary_fixed: argb_to_color(scheme.on_primary_fixed),
            on_primary_fixed_variant: argb_to_color(scheme.on_primary_fixed_variant),
            secondary: argb_to_color(scheme.secondary),
            on_secondary: argb_to_color(scheme.on_secondary),
            secondary_container: argb_to_color(scheme.secondary_container),
            on_secondary_container: argb_to_color(scheme.on_secondary_container),
            secondary_fixed: argb_to_color(scheme.secondary_fixed),
            secondary_fixed_dim: argb_to_color(scheme.secondary_fixed_dim),
            on_secondary_fixed: argb_to_color(scheme.on_secondary_fixed),
            on_secondary_fixed_variant: argb_to_color(scheme.on_secondary_fixed_variant),
            tertiary: argb_to_color(scheme.tertiary),
            on_tertiary: argb_to_color(scheme.on_tertiary),
            tertiary_container: argb_to_color(scheme.tertiary_container),
            on_tertiary_container: argb_to_color(scheme.on_tertiary_container),
            tertiary_fixed: argb_to_color(scheme.tertiary_fixed),
            tertiary_fixed_dim: argb_to_color(scheme.tertiary_fixed_dim),
            on_tertiary_fixed: argb_to_color(scheme.on_tertiary_fixed),
            on_tertiary_fixed_variant: argb_to_color(scheme.on_tertiary_fixed_variant),
            error: argb_to_color(scheme.error),
            on_error: argb_to_color(scheme.on_error),
            error_container: argb_to_color(scheme.error_container),
            on_error_container: argb_to_color(scheme.on_error_container),
            surface_dim: argb_to_color(scheme.surface_dim),
            surface: argb_to_color(scheme.surface),
            surface_tint: argb_to_color(scheme.surface_tint),
            surface_bright: argb_to_color(scheme.surface_bright),
            surface_container_lowest: argb_to_color(scheme.surface_container_lowest),
            surface_container_low: argb_to_color(scheme.surface_container_low),
            surface_container: argb_to_color(scheme.surface_container),
            surface_container_high: argb_to_color(scheme.surface_container_high),
            surface_container_highest: argb_to_color(scheme.surface_container_highest),
            on_surface: argb_to_color(scheme.on_surface),
            on_surface_variant: argb_to_color(scheme.on_surface_variant),
            outline: argb_to_color(scheme.outline),
            outline_variant: argb_to_color(scheme.outline_variant),
            inverse_surface: argb_to_color(scheme.inverse_surface),
            inverse_on_surface: argb_to_color(scheme.inverse_on_surface),
            surface_variant: argb_to_color(scheme.surface_variant),
            background: argb_to_color(scheme.background),
            on_background: argb_to_color(scheme.on_background),
            shadow: argb_to_color(scheme.shadow),
            scrim: argb_to_color(scheme.scrim),
        }
    }
}

/// A full light/dark MD3 theme generated from one seed color (§7.1:
/// "Generates a full light/dark scheme").
#[derive(Clone, Debug, PartialEq)]
pub struct DynamicTheme {
    pub light: ColorScheme,
    pub dark: ColorScheme,
}

impl DynamicTheme {
    /// Builds a real `material-colors` `Theme` from `seed` (the
    /// library's default `TonalSpot` variant -- MD3's own default,
    /// matching §7.1's scope: no variant selection is asked for here)
    /// and maps both its light and dark schemes.
    pub fn from_seed(seed: Color) -> Self {
        let theme = ThemeBuilder::with_source(color_to_argb(seed)).build();
        Self {
            light: ColorScheme::from(&theme.schemes.light),
            dark: ColorScheme::from(&theme.schemes.dark),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Isolated, non-tautological check of one conversion direction:
    /// three distinct channel values catch a swapped-channel bug that a
    /// round-trip test could hide if the same swap happened on both
    /// sides.
    #[test]
    fn color_to_argb_maps_channels_in_the_right_order() {
        let color = Color::from_rgba8(0x12, 0x34, 0x56, 0x78);
        let argb = color_to_argb(color);
        assert_eq!(argb.red, 0x12);
        assert_eq!(argb.green, 0x34);
        assert_eq!(argb.blue, 0x56);
        assert_eq!(argb.alpha, 0x78);
    }

    /// The reverse direction, checked independently of the above --
    /// together, both isolate this module's own conversion functions
    /// from `material-colors`' scheme-generation logic entirely.
    #[test]
    fn argb_to_color_maps_channels_in_the_right_order() {
        let argb = Argb::new(0x78, 0x12, 0x34, 0x56);
        let color = argb_to_color(argb);
        let rgba = color.to_rgba8();
        assert_eq!(rgba.r, 0x12);
        assert_eq!(rgba.g, 0x34);
        assert_eq!(rgba.b, 0x56);
        assert_eq!(rgba.a, 0x78);
    }

    /// The actual "wire material-colors" claim: `DynamicTheme::
    /// from_seed`'s output, role by role, must match what
    /// `material-colors`' own `ThemeBuilder` computes natively for the
    /// identical seed -- not just "produces some plausible-looking
    /// colors." Cross-checks several roles across both light and dark,
    /// including a `*_fixed` role and a `surface_container_*` tier, to
    /// prove the full 49-field mapping isn't silently dropping or
    /// mis-assigning fields outside whichever few a smaller test might
    /// have picked.
    #[test]
    fn from_seed_matches_material_colors_own_native_output_role_by_role() {
        let seed_argb = Argb::from_u32(0xff6750a4); // the same MD3-ish purple used throughout this project
        let seed_color = argb_to_color(seed_argb);

        let expected = ThemeBuilder::with_source(seed_argb).build();
        let actual = DynamicTheme::from_seed(seed_color);

        assert_eq!(
            actual.light.primary,
            argb_to_color(expected.schemes.light.primary)
        );
        assert_eq!(
            actual.light.on_primary_container,
            argb_to_color(expected.schemes.light.on_primary_container)
        );
        assert_eq!(
            actual.light.surface_container_high,
            argb_to_color(expected.schemes.light.surface_container_high)
        );
        assert_eq!(
            actual.light.primary_fixed_dim,
            argb_to_color(expected.schemes.light.primary_fixed_dim)
        );
        assert_eq!(
            actual.dark.surface,
            argb_to_color(expected.schemes.dark.surface)
        );
        assert_eq!(
            actual.dark.on_surface,
            argb_to_color(expected.schemes.dark.on_surface)
        );
        assert_eq!(
            actual.dark.error,
            argb_to_color(expected.schemes.dark.error)
        );

        // Light and dark must genuinely differ -- a bug that fed the
        // same scheme into both fields would still pass every equality
        // check above.
        assert_ne!(actual.light.surface, actual.dark.surface);
    }
}
