# LOG — Milestone 86, Phase 1: Theme and Stylesheet Specs

- User-directed: "Start M86" (scoped in `c257580` after the user said
  "Tesserae should not be pushing files directly to tre. It should be
  pushing spec information and handling the files itself.")

## What shipped

1. `view.rs`: `resolve_theme_input` turns a `(path, dict)` argument
   pair into one `Option<ThemeSpec>`; `resolve_stylesheet_input` does
   the same for `Stylesheet`; `shipped_default_theme_spec` replaces two
   duplicated inline parses of the shipped default. Mutual exclusion
   reuses M81's `require_at_most_one_content_source`.
2. `resolve_theme_layers` now takes resolved `ThemeSpec`s instead of
   paths, so the file and dict forms share every step after input
   resolution.
3. New kwargs: `View(stylesheet_spec=, default_theme_spec=,
   custom_theme_spec=)`, `View.set_theme(default_theme_spec=,
   custom_theme_spec=)`, `Window.set_theme(default_theme_spec=,
   custom_theme_spec=)`. All appended after existing params, so no
   positional caller changes.
4. `_core.pyi` stubs updated for all three.
- Tests: 4 new Rust tests (dict/YAML parity on resolved roles, both-
  given error, unknown-key rejection, stylesheet both-given error); 18
  new pytest cases in `tests/test_theme_spec.py`, including a `View`
  built from data alone across all four cascade tiers.
- Verification: `cargo fmt --check`/`clippy -D warnings` clean;
  `maturin develop --release`; `pytest tests/` 908 passed, 2 skipped
  (was 890); `mypy --strict` on `_core.pyi` clean.

## Status

**Phase 1 complete.** Next: Phase 2, font registration
(`tre.register_font`).
