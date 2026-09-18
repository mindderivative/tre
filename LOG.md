# LOG — M30 Phase 8 Step 2: Link

- Confirmed no official MD3 Link token page exists via the same
  directory-listing technique used throughout this milestone.
- Added a genuine new `NodeKind::Link(TextState)` to
  `crates/engine-core/src/node.rs`, reusing `TextState` verbatim as
  its own payload rather than a new struct — Link's real content/font
  shape is identical to Text's.
- Compiled to find every exhaustive match needing a new arm:
  `engine-render/src/lib.rs`'s `paint_node` (fixed with an or-pattern,
  `NodeKind::Text(state) | NodeKind::Link(state) =>`, since both share
  identical paint logic) and `engine-py/src/node.rs`'s `kind_name`.
  `Tree::hit_test_at` needed zero changes — by not matching
  `NodeKind::Text(_) => false`, Link falls through to the existing
  `_ => rect_contains(...)` catch-all automatically.
- Added a direct `engine-core` unit test
  (`link_independently_claims_a_hit_where_text_would_defer`) proving
  the real contrast: identical geometry built twice, a bare Text
  child (defers, parent claims the hit) vs a Link child (claims it
  directly). Passed on the first run.
- Implemented `add_link` in `crates/engine-py/src/window_factory.rs`
  (primary color, Body Large label reusing
  `SEARCH_INPUT_FONT_SIZE`/`_WEIGHT`).
- Added `.pyi` stub.
- Wrote `tests/test_link.py` (4 tests) — all passed on the first run,
  reproducing the Text-vs-Link contrast through the real Python FFI
  (a bare `add_text` label never independently claims its click; a
  Link does).
- Wrote `examples/link.py` (headless-CI-safe) — clean on the first
  run, `mypy --strict` clean too.
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --workspace --release` (engine-core 150 up from
  149), `maturin develop --release`, `pytest tests/` (424 passed, 1
  skipped), all 58 examples clean, showcase demo clean.
- Updated `BUILD_TRACKER.md`, regenerated and republished the Build
  Tracker artifact at https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
