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

Both are freely redistributable; neither license requires attribution
beyond retaining their own license text, available from
<https://fonts.google.com/specimen/Roboto> and
<https://fonts.google.com/noto/specimen/Noto+Sans+Arabic> respectively.
