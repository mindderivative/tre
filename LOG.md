# LOG — M30 Phase 5 Step 5: Search (closes Phase 5)

- Fetched the real GitHub directory listing for "search" — found
  `_md-comp-search-bar.scss`/`_md-comp-search-view.scss` exactly as
  the step's own "bar and view" name implies.
- Fetched real Search Bar tokens: `surface_container_high` fill,
  `corner-full` (56dp height), elevation level 3, leading icon
  `on_surface`, trailing icon `on_surface_variant`, input text Body
  Large (`on_surface`).
- Fetched real Search View tokens: `surface_container_high`,
  elevation level 3, real *docked* variant `corner-extra-large`
  (reused `DIALOG_CORNER_RADIUS`'s own confirmed 28dp value directly)
  vs. the mobile-only *full-screen* variant (excluded).
- Traced Body Large's real numeric values (1rem = 16px,
  weight-regular = 400).
- Designed to reuse `TextField`'s own existing `NodeKind` for the
  search bar's input (mirroring `add_text_field`'s own construction
  inline, not cross-calling it — no `add_*` method in this file calls
  another, avoiding re-entrant `self.tree.borrow_mut()`), and to
  reuse `Window.open_menu`/`close_menu` directly for the search
  view's lifecycle (Tooltip's own real precedent), no new dedicated
  open/close pair.
- Implemented `add_search_bar`/`add_search_view` in
  `crates/engine-py/src/window_factory.rs`.
- Wrote `tests/test_search.py` — caught my own mistake before running:
  `kind_name` isn't exposed to Python at all (it's an internal Rust
  helper); switched to `get_text()`, the real, already-established
  discriminator `test_text_field.py`'s own
  `test_get_text_rejects_a_non_text_field_node` uses from the other
  direction. All 10 tests passed after the fix.
- Wrote `examples/search.py` — found two real behavioral facts while
  writing it, not assumed: (1) `TextFieldState`'s cursor seeds at the
  end of its initial content, so typing after focus *appends* rather
  than replaces (`field.set_text("")` first, matching real UX of
  starting a fresh search); (2) `window.click()` alone didn't
  reliably move focus in this sequence — switched to
  `window.press_key("tab")`, matching `test_text_field.py`'s own
  established focus technique exactly; (3) opening the search view
  before testing an unrelated trailing-icon click let
  `dismiss_on_outside_click` consume that click (real, correct
  overlay behavior, not a bug) — reordered the example's own
  interactions to respect it rather than working around it.
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --workspace --release` (43 binaries green,
  unchanged), `maturin develop --release`, `pytest tests/` (374
  passed, 1 skipped), all 51 examples clean, showcase demo clean.
- Updated `BUILD_TRACKER.md` (Phase 5 heading now ✅, Step 5 and Step
  6 both closed), regenerated and republished the Build Tracker
  artifact at https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
- Phase 5 (Navigation) is now fully complete, all 6 steps.
