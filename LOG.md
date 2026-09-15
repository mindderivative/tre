# Log: M3 Phase 5, Step 11 — Wire `material-colors` (§14 step 11, §7.1)

Corresponds to `PLAN.md` / `BUILD_TRACKER.md` M3 Phase 5, step 11 of 4 (steps 8-11) -- closes Phase 5.

## What happened

**Treated §7.1's acceptance gate as a real gate to pass, not a checkbox
to note.** Its own text: "whichever crate is chosen must pass Material
Color Utilities' own published reference test vectors... before it's
pinned -- not just 'compiles and the colors look plausible.'" Resolved
`material-colors 0.4.2` via `cargo add --dry-run` first (not assumed),
then fetched its real source and read its own test files directly
before writing a line of this project's code: `src/hct/mod.rs`'s CAM16
tests assert the exact published reference values for red/green/blue/
black/white (e.g. red: `j=46.445, chroma=113.357, hue=27.408`) --
recognizable directly as Google's own MCU reference constants, the same
numbers every MCU port's own test suite carries, not values this crate
invented. `src/palette/core.rs`'s `CorePalette::of` tests assert exact
published tonal-palette hex values. `src/contrast.rs` and
`dynamic_color::tests::test_contrast_pairs` cover contrast-ratio math.

**Then actually ran it**, rather than trusting source-reading alone:
`cargo test --lib` inside the vendored `material-colors-0.4.2` source
directory, on this exact pinned version and this project's own
toolchain (1.98.0) -- **129 passed, 0 failed**. This is the real
acceptance-gate evidence this step's own `color.rs` module doc comment
cites, matching the project's "verify the real thing, not the claim
about the thing" discipline applied consistently since `pyo3`/
`accesskit` API verification.

**`engine_md3::color::ColorScheme`**: every one of `material_colors::
scheme::Scheme`'s 49 role fields (primary/on_primary/..., the newer
`*_fixed` roles, all five `surface_container_*` elevation tiers,
outline, inverse_*, shadow, scrim), each mapped onto a `peniko::Color`
via `Argb <-> peniko::Color` conversions using `AlphaColor::to_rgba8`/
`Color::from_rgba8` (verified directly in the `color` crate's own
source: `peniko::Color` is `AlphaColor<Srgb>`, whose inherent
`to_rgba8()` gives `Rgba8{r,g,b,a}` directly). Generated the field list
and the mechanical `From` impl body with a small throwaway script
rather than hand-typing 49 lines twice, specifically to eliminate the
transcription-typo risk that many similarly-named fields (`primary` vs
`primary_container` vs `primary_fixed` vs `primary_fixed_dim`) invites.

**`DynamicTheme::from_seed`**: one seed `peniko::Color` in, a real
`material_colors::theme::ThemeBuilder::with_source(...).build()` call,
both light and dark schemes mapped out. Uses the library's default
`TonalSpot` variant -- MD3's own default; variant selection isn't
asked for by §7.1's scope.

**Real, non-tautological tests, not a round-trip that could hide a
channel-swap bug.** `color_to_argb`/`argb_to_color` are each tested in
isolation first, with three distinct channel values (`0x12,0x34,0x56`)
so a swapped-channel bug can't hide behind a round-trip test using the
same (buggy) conversion on both ends. Only once those are independently
proven correct does the actual "wire material-colors" claim get tested:
`DynamicTheme::from_seed`'s output cross-checked field-by-field against
`material-colors`' own native `ThemeBuilder` output for the identical
seed -- several roles across both light and dark, including a
`*_fixed` role and a `surface_container_*` tier, so the check isn't
only exercising whichever handful of fields a smaller test might have
picked. A final assertion that light and dark actually differ rules out
a bug that fed the same scheme into both output fields.

**Deliberately did not wire live theme switching** (§7.1's own "live
theme switching is in scope" text) -- that dispatch routes through
`AppHandler`/`InputEvent` (§4), checked directly and still absent
anywhere in this codebase, the same finding steps 7 and 9 already made
for keyboard and pointer dispatch respectively. This module builds and
proves the real mechanism that dispatch will call once it exists.

## Verification

```
$ (cd ~/.cargo/registry/src/*/material-colors-0.4.2 && cargo test --lib)
test result: ok. 129 passed; 0 failed; 0 ignored   # the actual §7.1 acceptance gate

$ cargo test -p engine-md3 -- --nocapture
running 8 tests
test color::tests::argb_to_color_maps_channels_in_the_right_order ... ok
test color::tests::color_to_argb_maps_channels_in_the_right_order ... ok
test color::tests::from_seed_matches_material_colors_own_native_output_role_by_role ... ok
test shape_morph::tests::... (5 tests, unchanged from step 10) ... ok

$ cargo test --workspace             # all green
$ cargo clippy --workspace --all-targets -- -D warnings   # clean
$ cargo fmt --check                  # clean
```

## Next

`BUILD_TRACKER.md` updated: Phase 5 (steps 8-11) fully done -- M3 now
5 of 7 phases complete. Next: Phase 6 (§14 step 12) -- `engine-spec`,
full: `BindingResolver` + the `ViewModel`/`View._attach()` model
(§16.2), the stylesheet cascade with real MD3 token resolution (§16.3,
now that this step gives it an actual color scheme to resolve
`background: primary`-style tokens against instead of a stub), and
reconciliation/hot-reload (§16.4). Deliberately sequenced after both
`engine-py` (step 6) and this step, since it needs both.
