# LOG — M30 Phase 7 Step 1: Date Picker

- Found the real Date/Time Picker token files via a GitHub directory
  listing: `_md-comp-date-input-modal.scss`,
  `_md-comp-date-picker-docked.scss`,
  `_md-comp-date-picker-modal.scss`, `_md-comp-time-input.scss`,
  `_md-comp-time-picker.scss`. Chose the real *docked* Date Picker
  variant (desktop-appropriate, the same real "docked over
  full-screen" precedent Search View already made).
- Fetched the real docked-variant day-cell tokens: 48x48dp,
  corner-full; selected = primary fill/on_primary label; today (not
  selected) = 1dp primary outline/primary label, no fill; unselected
  = on_surface label, or on_surface_variant for a day outside the
  current month.
- Decided scope: only the day cell gets real new anatomy. A real
  calendar grid needs date arithmetic (month lengths, weekday-of-
  month, leap years) that's genuinely application logic, already
  trivially available via Python's own `calendar`/`datetime`
  modules — no engine-owned calendar primitive invented.
- Implemented `add_date_picker_day` in
  `crates/engine-py/src/window_factory.rs`, reusing the already-real
  `PaintProperties.border_color`/`border_width` (Phase 1 Step 1) for
  the today-outline and Body Large's already-declared constants
  (`SEARCH_INPUT_FONT_SIZE`/`_WEIGHT`) for the day label.
- Added `.pyi` stub.
- Wrote `tests/test_date_picker.py` (8 tests) — all passed on the
  first run, including a real test building a genuine September 2026
  calendar grid (30 real days) using nothing but Python's own
  `calendar.monthcalendar` plus this one primitive.
- Wrote `examples/date_picker.py` (headless-CI-safe), applying the
  `Callable` return-type annotation proactively — clean on the first
  run (`mypy --strict` too).
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --workspace --release` (43 binaries green,
  unchanged), `maturin develop --release`, `pytest tests/` (407
  passed, 1 skipped), all 55 examples clean, showcase demo clean.
- Updated `BUILD_TRACKER.md` (Phase 7 heading now 🚧, Step 1 closed),
  regenerated and republished the Build Tracker artifact at
  https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
