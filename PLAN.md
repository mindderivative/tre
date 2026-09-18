# PLAN — M30 Phase 5 Step 5: Search (closes Phase 5)

## Goal
Add `Window.add_search_bar`/`add_search_view` — closing Phase 5's own
component list.

## Steps
1. Verify real Search Bar/View tokens via WebFetch (found via a real
   GitHub directory listing: `_md-comp-search-bar.scss`/
   `_md-comp-search-view.scss`).
2. Trace Body Large's real numeric size/weight.
3. Design: reuse TextField's own existing NodeKind for the input
   (not a bare styled box), mirroring add_text_field's own
   construction inline (no cross-calling factory methods, avoiding
   re-entrant tree borrow). Reuse open_menu/close_menu directly for
   the search view's own lifecycle (Tooltip's own precedent), no new
   dedicated open/close pair.
4. Implement `add_search_bar`/`add_search_view` in
   `window_factory.rs`.
5. Add `.pyi` stubs for both.
6. Write `tests/test_search.py`. Fix a real gate-naming mistake
   (kind_name isn't Python-exposed; get_text() is the real
   discriminator, matching test_text_field.py's own precedent).
7. Write `examples/search.py`, headless-CI-safe. Fix two real
   findings along the way: TextField's cursor seeds at content-end
   (typing appends, not replaces -- use set_text("") first), and
   dismiss_on_outside_click consumes a click on an unrelated node
   while the view is open (reorder interactions to respect it).
8. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all examples, showcase demo, mypy
   --strict.
9. Update `BUILD_TRACKER.md` (Top Metrics row, Phase 5 heading now
   ✅, Step 5 and Step 6 both closed), regenerate + republish the
   Build Tracker artifact. Closes Phase 5 entirely.
10. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (374 pytest
passed/1 skipped, 51 examples, showcase demo, 43 Rust test binaries).
Phase 5 (Navigation) is now fully complete, all 6 steps.
