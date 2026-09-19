# Vendored fonts

Embedded via `include_bytes!` in `tests/text_layout.rs` and
`tests/rect_window.rs` (§14 step 4) so the typography spike is hermetic
-- no dependency on whatever fonts happen to be installed on the machine
running `cargo test`, matching this project's headless-CI-safe testing
discipline everywhere else.

- `Roboto-Regular.ttf`, `Roboto-Medium.ttf` -- Google's Roboto, MD3's own
  default typeface (§14 step 4 asks for "real weights/sizes"). Apache
  License 2.0.
- `NotoSansArabic-Regular.ttf` -- Google's Noto Sans Arabic, used for the
  step's required non-Latin/BiDi string. SIL Open Font License 1.1.
- `HackNerdFontMono-Regular.ttf` -- the Hack project's Nerd Font Mono
  patch (real family name embedded in the font's own `name` table,
  confirmed by direct read: "Hack Nerd Font Mono"), M32 Phase 1's real
  bundled monospace face for `Terminal`/`Code Editor`. MIT License
  (`LICENSE-HackNerdFontMono.txt`, Copyright 2018 Source Foundry
  Authors; the same file also documents the DejaVu (public domain) and
  Bitstream Vera Sans Mono (Bitstream Vera License) components Hack
  itself derives from). Reused directly from the sibling `pyCopper`
  project's own already-vetted copy
  (`pycopper/assets/fonts/HackNerdFontMono-Regular.ttf`), which made the
  identical real choice for its own `Terminal` widget for the identical
  real reason: broad glyph coverage (box-drawing, Powerline, Nerd Font
  glyphs) a plain monospace face like DejaVu Sans Mono lacks, avoiding
  the "tofu" (missing-glyph boxes) real prompt themes and TUI apps rely
  on.

All four are freely redistributable; none requires attribution beyond
retaining their own license text, available from
<https://fonts.google.com/specimen/Roboto>,
<https://fonts.google.com/noto/specimen/Noto+Sans+Arabic>, and
<https://www.nerdfonts.com/font-downloads> (Hack) respectively.
