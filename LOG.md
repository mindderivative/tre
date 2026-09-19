# LOG — M30 Phase 9 Step 3: Code Editor

- Read pyCopper's own real `CodeEditor` widget directly
  (`/home/phil/pyDev/projects/pyCopper/src/pycopper/widgets/
  codeeditor.py`): a real, syntax-highlighted, line-numbered multiline
  editor built ALONGSIDE `TextField`, not on top of it -- its own real
  reason: `TextField`'s own paint methods are saturated with M3-
  specific chrome (floating label, focus indicator stroke, filled/
  outlined containers) a code surface has none of, while the
  UNDERLYING editing model (cursor/selection/undo-free mutation) is
  genuinely shared. Real, stated v1 deferrals there too: no syntax
  highlighting/gutter/minimap/multi-cursor/bracket-matching, Pygments
  optional and lazily imported, a bundled monospace font
  (`Hack Nerd Font Mono`), and a new dispatcher-level `CAPTURES_TAB`
  opt-in flag so Tab could be intercepted for indentation.
- Confirmed via direct source read: TRE's own `TextField` is
  deliberately single-line only -- `Tree::dispatch_text_field_key`'s
  own `Key::Enter => Some(DispatchOutcome::None)` arm, with a real,
  stated comment ("single-line scope, not a general multiline text
  area"). A genuine Code Editor needs this closed for real.
- Design decision, unlike pyCopper's own separate widget class: reuse
  `NodeKind::TextField` directly, adding `TextFieldState.multiline:
  bool` (new field, `pub`, default `false` -- every existing
  construction site uses `::new()`, confirmed via grep, so this is
  byte-for-byte backward compatible with zero call-site changes).
  `engine-core`'s own editing model has no M3-specific chrome to
  strip the way pyCopper's own `TextFieldElement` did, so the real
  reason pyCopper built a separate class doesn't apply here.
- Confirmed via direct source read: `engine_core::Key` has no
  `ArrowUp`/`ArrowDown` variants at all (only `ArrowLeft`/`ArrowRight`)
  -- a real, load-bearing gap, not just new match arms on an existing
  vocabulary. Added both, verified real via winit's own vendored
  `keyboard.rs` before using them.
- Designed line navigation (`ArrowUp`/`ArrowDown`) as pure string/
  column logic entirely within `engine-core` -- no `parley`/layout
  access needed (respecting §4's crate boundary), using CHARACTER
  column (not byte offset, UTF-8-safe like `ArrowLeft`/`Right`'s own
  `char_indices` stepping already is). Correct, expected behavior for
  a genuinely monospace editor; this codebase has no bundled monospace
  font yet, a real, separate, stated gap (`add_code_editor`'s own doc
  comment).
- Added `Key::ArrowUp`/`ArrowDown` to the enum; fixed the two resulting
  exhaustive `match key` compile errors in `tree.rs` (the top-level
  `Tree::dispatch` match's own no-op catch-all group, and
  `dispatch_text_field_key`'s own real new arms); threaded through
  `engine_platform::translate_key` (real winit `NamedKey::ArrowUp`/
  `ArrowDown` mapping, confirmed real via winit's own vendored source)
  and `Window.press_key`'s own `"up"`/`"down"` synthetic-testing
  strings; updated `translate_key`'s own two existing tests (one had
  been asserting `ArrowDown` was OUTSIDE the vocabulary -- now real,
  moved into the "maps exactly" test, replaced with `PageDown` as the
  still-outside-vocabulary example).
- Implemented `Enter` (multiline: `delete_selection` then insert
  `\n`), `Home`/`End` (multiline: new `Tree::line_start`/`line_end`
  helpers, pure `rfind`/`find` on `\n`), and `ArrowUp`/`ArrowDown`
  (multiline: new `Tree::move_to_line`, column-preserving,
  clamped to the target line's own real length) in
  `dispatch_text_field_key`.
- Wrote 7 new `engine-core` unit tests. **Caught and fixed a real math
  error before trusting an assertion**: a draft test expected a second
  consecutive `ArrowUp` (through a shorter intervening line) to
  preserve the ORIGINAL column from before the first hop -- verified
  via a standalone Python simulation of `move_to_line`'s own exact
  logic that the real, correct answer is different: this v1 has no
  persistent "goal column" memory, so the second `ArrowUp` preserves
  whatever column the FIRST hop actually landed at (the shorter line's
  own real length), not the original. Fixed the test's own expected
  value and documented this real, deliberate v1 simplification
  directly in the test's own comment, not silently papered over.
- Fixed `engine-render`'s own layout for multiline: confirmed via
  direct source read that `Layout::break_all_lines(Some(f32::MAX))`
  and `break_all_lines(None)` are the identical real no-wrap value
  (`max_advance.unwrap_or(f32::MAX)`) -- no new `Option`-typed
  plumbing needed through `shaped_layout`/`build_field_layout`'s own
  existing `f32` parameter, just a new `field_max_width` helper
  picking the right value at the two real call sites (`draw_field`/
  `hit_test_position`) that know `state.multiline`. Hit a real
  ownership error first try (`TextPlacement` isn't `Copy`) -- fixed by
  passing `at.max_width` (a plain `f32`) into the helper instead of
  the whole struct.
- Wrote a new, GPU-free `engine-render` test
  (`a_multiline_fields_own_newline_produces_a_real_second_layout_
  line`, `text_field_paint.rs`) using `hit_test_position` (pure CPU,
  no `Scene`/`Resources` needed, the same real technique the file's
  own pre-existing `hit_test_position_*` tests already established) --
  a click well below the first line resolves inside the second line's
  own real content, proving a real `\n` genuinely produces a second,
  vertically-stacked layout line. Passed on the first run.
- Implemented `Window.add_code_editor(content, background, width,
  height, font_weight, font_size, x, y)` in `window_factory.rs`,
  reusing `add_text_field`'s exact real pattern (theme resolution via
  `theme.on_surface()`, `Role::TextInput`/`Action::Focus` access) plus
  `TextFieldState.multiline = true`. Deliberately does NOT expose
  `font_family` (unlike `add_text_field`) -- confirmed via direct
  source read of `TextRenderer::new`'s own "system font discovery is
  deliberately OFF" doc comment that only the three registered fonts
  (Roboto Regular/Medium, Noto Sans Arabic) would actually resolve, so
  exposing the param would falsely imply any name works.
- Added `.pyi` stub.
- **Applied the lesson from Phase 9 Step 2's own mistake**
  ([[feedback_check_before_new_example_file]]): checked `git status`/
  `ls` for both `tests/test_code_editor.py` and `examples/
  code_editor.py` before writing either -- confirmed no collision.
- Wrote `tests/test_code_editor.py` (9 tests) -- all passed on the
  first run, including real Enter/Home/End/ArrowUp/ArrowDown proofs
  through the real `press_key`/`type_text`/`get_text` round-trip.
- Ran a real, direct empirical check via a synchronous Python script
  before writing the test suite -- typed a 3-line buffer, pressed
  Home/Up/Up/Home and typed again, confirmed the new text landed
  exactly on the first line as expected. Real, working end to end on
  the first try.
- Wrote `examples/code_editor.py` -- a real 3-line buffer, split via a
  real mid-buffer Enter, edited via Home/ArrowDown/End navigation.
  Clean on the first run, `mypy --strict` clean too.
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --workspace --release` (44 binaries green,
  `engine-core` 158 up from 151, `engine-render` `text_field_paint` 9
  up from 8), `maturin develop --release`, `pytest tests/` (474
  passed, 1 skipped, up from 465), all 65 examples clean, showcase
  demo clean, `mypy --strict` clean against `examples/code_editor.py`.
- Updated `BUILD_TRACKER.md` (Top Metrics row now 95%, Step 3 line,
  "Just closed"/"Up next" trailer), regenerated and republished the
  Build Tracker artifact at
  https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
