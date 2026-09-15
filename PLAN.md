# Plan: M3 Phase 5, Step 11 — Wire `material-colors` (§14 step 11, §7.1)

Corresponds to `BUILD_TRACKER.md` M3 Phase 5, step 11 of 4 (steps 8-11) -- closes Phase 5.

## Goal

Per §14 step 11: "Wire `material-colors` for a full dynamic color
theme." §7.1: HCT color space, tonal palette generation, scheme
generation from a seed color, a full light/dark scheme; `engine-md3`
maps scheme roles (primary, on-primary, surface, etc.) onto
`peniko::Color` values.

**§7.1's own acceptance gate, taken literally, not as a "compiles and
looks plausible" checkbox:** "whichever crate is chosen must pass
Material Color Utilities' own published reference test vectors (HCT
round-trip conversions, tonal palette values, contrast ratios) before
it's pinned."

## Verifying the gate before pinning

Resolved `material-colors 0.4.2` via `cargo add --dry-run` first (not
assumed). Fetched its real source and inspected its own test suite
directly rather than trusting its README:
- `src/hct/mod.rs`'s CAM16 tests assert the exact published reference
  values for RED/GREEN/BLUE/BLACK/WHITE (e.g. `cam.j = 46.445, cam.chroma
  = 113.357, cam.hue = 27.408` for red) -- these are Google's own MCU
  reference constants, recognizable directly (the same numbers appear
  in every MCU port's own test suite), not something this crate made up.
- `src/palette/core.rs`'s `CorePalette::of(0xff0000ff)` tone tests assert
  exact published tonal-palette hex values (tone 100 = `0xffffffff`,
  tone 95 = `0xfff1efff`, tone 90 = `0xffe0e0ff`, ...).
- `src/contrast.rs` and `dynamic_color::tests::test_contrast_pairs`
  cover contrast-ratio math.

Then **ran the crate's own test suite for real**, on the exact pinned
version and this project's own toolchain, rather than trusting that
source inspection alone: `cargo test --lib` inside the vendored
`material-colors-0.4.2` source directory -- **129 passed, 0 failed**.
This is the actual acceptance-gate evidence: not "the crate claims to
port MCU," but "the exact pinned version's own port of MCU's reference
vectors passes, checked directly, right now."

## Scope

In scope:
- `engine-md3::color` (or similarly named module): `ColorScheme` --
  every role field from `material_colors::scheme::Scheme` (49 fields:
  primary/on_primary/..., surface tiers, the newer `*_fixed` roles,
  outline, inverse_*, shadow, scrim), each mapped to a `peniko::Color`.
  Mapped as a complete, mechanical 1:1 field conversion, not an
  arbitrarily truncated subset -- omitting some roles for "don't build
  ahead of need" would just mean a real future caller hits an
  inexplicably-missing field for no principled reason, since there's no
  extra logic or judgment call per field to defer.
- `DynamicTheme::from_seed(seed: peniko::Color) -> DynamicTheme { light:
  ColorScheme, dark: ColorScheme }`: builds a real `material_colors::
  theme::ThemeBuilder::with_source(...).build()` from the seed and maps
  both schemes.
- Real, non-tautological tests: the `peniko::Color <-> material_colors::
  color::Argb` channel conversions verified in isolation first (so a
  channel-order bug can't hide inside a round-trip test that would still
  pass with a consistent bug on both sides), then the full `from_seed`
  pipeline cross-checked against `material-colors`' own native `Theme`
  output for the identical seed -- proving the role mapping is correct
  field-by-field, not just "produces *some* plausible-looking colors."

Out of scope (deferred, not a gap): wiring `winit`'s `ThemeChanged`
event through `AppHandler`/`InputEvent` for live theme switching. That
dispatch mechanism still doesn't exist anywhere in this codebase --
checked directly, same finding as steps 7 and 9 -- so there's nothing
to wire live switching *to* yet. This step builds and proves the real
`DynamicTheme::from_seed` mechanism `engine-platform` will call once
that dispatch exists, matching the identical scope narrowing already
applied twice this phase. Also out of scope: consuming `ColorScheme` in
`PaintProperties`/a real MD3 component (`engine-spec`'s token resolution,
§16.3, is a later step that explicitly waits on this one).

## Verification

`cargo test -p engine-md3` passes, including the cross-checked
role-mapping test. `cargo test --workspace`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo fmt --check` all clean.
