# PLAN — M30 Phase 6 Step 2: Accordion

## Goal
Add `Window.add_accordion_header` — MD3 has no official Accordion
page; only the header (title + expand/collapse chevron) gets real new
anatomy, grounded in the Lists guideline's own "expand and collapse"
text.

## Steps
1. Check the curated icon set (only 8 existed: home/search/menu/
   close/check/arrow_back/add/settings) -- no chevron. Fetch the real
   `expand_more` SVG path data verbatim from Google's CDN, add as the
   ninth curated icon.
2. Investigate whether Node.animate("transform", ...) supports
   rotation -- confirmed it doesn't (translate+scale only). Design
   the honest workaround: a uniform negative scale (-1.0) is
   mathematically identical to a 180° rotation for the point-
   symmetric chevron glyph.
3. Decide scope: only the header is new anatomy; content region has
   no MD3 styling, app composes it from existing containers -- no
   dedicated add_accordion_content method.
4. Implement `add_accordion_header` in `window_factory.rs`, reusing
   List Item's own anatomy (MENU_ITEM_*).
5. Add `.pyi` stub.
6. Write `tests/test_accordion.py`. Discover Node.get() only supports
   scalar properties (transform isn't readable) -- simplify test to
   proving the animate call succeeds, not reading the result.
7. Write `examples/accordion.py`. Discover Node.remove() is full,
   irreversible destruction, not a detach -- switch to opacity
   animation for show/hide instead.
8. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all examples, showcase demo, mypy
   --strict.
9. Update `BUILD_TRACKER.md` (Top Metrics row, step line), regenerate
   + republish the Build Tracker artifact.
10. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (390 pytest
passed/1 skipped, 53 examples, showcase demo, 43 Rust test binaries).
Header's independent-click test passed on the first run (no
decorative wrapper needed).
