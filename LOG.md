# LOG — M30 Phase 7 Step 2: Time Picker (closes Phase 7)

- Confirmed the real MD3 Time variants (time-input vs time-picker)
  via the same directory listing Date Picker's own investigation
  already found. Decided scope: Time Input only — the analog
  clock-face dial needs a genuinely new drag-to-angle engine
  capability (converting a circular hit point into a time value),
  a real, separate, much larger undertaking.
- Fetched real Time Input tokens via `_md-comp-time-input.scss`:
  96x72dp field, `surface_container_highest`, real `corner-small`
  shape (reused `CHIP_CORNER_RADIUS`'s own confirmed 8dp value
  directly), Display Medium label (2.8125rem = 45px, weight-regular
  = 400); period selector 52x72dp, `corner-small`,
  `tertiary_container` selected fill.
- Designed to reuse `TextField`'s own existing NodeKind for the
  hour/minute field (Search Bar's own precedent); period selector as
  two independent Rect+Text buttons stacked vertically — honestly
  caveated that the exact label type role and any shared-outline
  detail beyond overall container dimensions weren't in the fetched
  token set, so Label Large and a plain two-button anatomy are
  reasonable, stated choices.
- Implemented `add_time_input_field`/`add_period_selector` in
  `crates/engine-py/src/window_factory.rs`. Fixed a real compile
  issue before it landed: an explicit `-> NodeId` closure return type
  needed an unused import; simplified by letting Rust infer the type
  instead.
- Added `.pyi` stubs for both.
- Wrote `tests/test_time_picker.py` (8 tests) — all passed on the
  first run, including a real `press_key`/`type_text`/`get_text`
  round-trip proving the TextField reuse actually works end to end.
- Wrote `examples/time_picker.py` (headless-CI-safe), clean on the
  first run, `mypy --strict` clean too.
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --workspace --release` (43 binaries green,
  unchanged), `maturin develop --release`, `pytest tests/` (415
  passed, 1 skipped), all 56 examples clean, showcase demo clean.
- Updated `BUILD_TRACKER.md` (Phase 7 heading now ✅, Step 2 and Step
  3 both closed), regenerated and republished the Build Tracker
  artifact at https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
- Phase 7 (Date & Time) is now fully complete, all 3 steps.
