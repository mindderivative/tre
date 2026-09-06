# Log: Phase 4, Step 4.1 -- HarfBuzz & FreeType Integration

## Finding (caught while writing this step's own unit tests): a "surely
this plain-text font lacks emoji" assumption was simply false

The fallback-cascade test for "a codepoint the primary font lacks resolves
to the emoji fallback" originally used U+1F600 (the classic grinning-face
emoji) as the "DejaVu Sans surely doesn't have this" codepoint. It failed:
`covers(&dejavu_sans_bytes, "\u{1F600}")` returned `true`, not `false`.

Investigated via `fc-query`'s own charset dump (not guessed): DejaVu
Sans's real, installed charset genuinely includes the classic "Emoticons"
Unicode block (U+1F600-U+1F61F) as real, mapped glyph entries -- a
long-standing feature of DejaVu's unusually broad Unicode coverage,
unrelated to whether the glyphs are color emoji (they aren't; DejaVu has
no COLR/CPAL or CBDT color tables, so the rendered result is presumably a
plain monochrome glyph, but the `cmap` entry is real and `charmap().map()`
correctly reports it).

**Fix:** re-verified via the same `fc-query` charset-dump technique before
picking a replacement, and switched to U+1F9E0 (the "brain" emoji, a newer
Unicode block) -- independently confirmed absent from both DejaVu Sans's
*and* Noto Sans's charset dumps (the two most likely `sans-serif`
resolutions on this project's dev machine and CI image respectively) and
present in Noto Color Emoji's. Not a bug in the fallback logic itself
(`covers`/`resolve_font_index` were both already doing exactly what they
were supposed to) -- a bug in the test's own unverified assumption about
a real font's actual coverage, caught immediately by the test itself
before it could ship as a false "fallback works" claim resting on the
wrong reason.

## What worked without needing further iteration

- The all-pure-Rust font stack (`rustybuzz` for shaping, `skrifa` for
  outline extraction) built and linked cleanly on the first attempt, with
  every version pin resolving against this workspace's `rust-version =
  1.75` on the first `cargo add --dry-run` check (no repeated trial and
  error the way `usvg`'s pin needed in Step 3.3.1).
- Bidi + script run segmentation (`unicode-bidi` + `unicode-script`)
  correctly reordered a real mixed Latin/Hebrew string into visual run
  order on the first real shaping run against a real installed font --
  matched the hand-worked-out expectation (Hebrew run's glyphs in
  descending/reversed cluster order) exactly.
- `fontconfig`-driven cascade discovery (`sans-serif`, `Noto Sans`,
  `emoji` generic families) resolved to real, sensible font files
  (`NotoSans-Regular.ttf`, `NotoColorEmoji.ttf`, deduplicated correctly
  since `sans-serif` and `Noto Sans` happen to resolve to the same file on
  this machine) on the first run.
- Glyph outline extraction + `tre-svg` flatten-function reuse + the
  pre-existing ear-clipping/flat-color pipeline rendered a real 'L' glyph
  from a real font correctly on the very first GPU run -- both the
  independently-computed point-in-polygon probes and the actual rendered
  pixels agreed immediately, and the output PNG is visually a clean,
  correctly-proportioned 'L'.
- Making `tre-svg`'s `flatten_cubic`/`flatten_quad` `pub` (Step 3.3.1's
  private-to-the-crate helpers) required no other change anywhere in
  `tre-svg` -- a purely additive visibility widening, confirmed by all 24
  pre-existing `tre-svg` tests plus all 10 pre-existing Vulkan examples
  continuing to pass unmodified.

## Verification performed

- `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D
  warnings` / `cargo test --workspace`: all clean, including 11 new
  `tre-text` unit tests (bidi+script segmentation, fallback resolution
  against real installed fonts, outline extraction against a real glyph)
  and `tre-svg`'s existing 24 tests unaffected by the `flatten` visibility
  change.
- `text_shaping_demo` run manually against the real GPU (AMD/Radeon,
  Wayland session) under the Vulkan validation layer: bidi+script
  shaping, fallback resolution, and both outline-render pixel-probe
  assertions all pass; output PNG visually inspected and shows a clean,
  correctly-proportioned white 'L' on the clear-color background.
- **All 11 pre-existing examples** (7 headless + 3 windowed + 1 from
  Step 3.3.3) re-run manually after adding `tre-text` to the workspace,
  zero validation errors and zero regressions -- this step added no new
  RHI/GPU surface at all (it reuses `create_pipeline` and the existing
  flat-color pipeline unmodified), so this mostly confirms the new
  dependency graph didn't disturb anything already built.
