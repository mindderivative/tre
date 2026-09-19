# PLAN — M31 Phase 1: Line-Number Gutter

## Goal
Add a real line-number gutter for `Window.add_code_editor`. The
BUILD_TRACKER's own scoping note flagged a real open question:
aligning each gutter number to its own real painted editor line needs
either a new per-line-position read-back or an assumed fixed
line-height (stated as fragile) — a real design decision to make when
this phase actually started, not resolved by the scoping pass.

## Steps
1. Read pyCopper's own real, much more advanced `CodeEditor` widget
   (a dedicated class, not a styled `TextField`) for its own real
   gutter design — confirmed "never wraps" (the same real property
   TRE's own multiline `TextField` already has), a real gutter width
   formula, and a real Pygments-based syntax highlighter (relevant to
   Phase 4, not this one).
2. Investigated TRE's own real text-shaping code directly before
   assuming a new API was needed: `TextRenderer::draw` (plain `Text`)
   and `TextRenderer::draw_field` (`TextField`) both build their real
   `parley::Layout` through the exact same private `shaped_layout`
   method. This means a gutter composed as an ordinary sibling `Text`
   node — same `font_family`/`font_weight`/`font_size` as the editor,
   wide enough never to wrap — lines up with the editor's own real
   per-line Y positions *by construction*, since both go through
   identical shaping code. No new engine-py/engine-core capability
   needed at all.
3. Proved this precisely at the Rust level with a new `engine-render`
   unit test comparing real `parley::Layout::lines()`' own
   `block_min_coord` geometry between a Text-shaped and a
   TextField-shaped layout of the same content/font — identical.
4. Wired the real live-update half: `editor.set_on_change(...)`
   (already real and Python-facing since M14 Phase 3) recomputes the
   gutter's own content from `editor.get_text().count("\n") + 1` on
   every real keystroke edit.
5. Wrote `tests/test_code_editor_gutter.py` and
   `examples/code_editor_gutter.py` — checked for filename collisions
   first.
6. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all 68 examples, showcase demo, mypy
   --strict.
7. Update `BUILD_TRACKER.md` — verified the parser's own reported item
   count before/after (191, unchanged), regenerate + republish the
   Build Tracker artifact.
8. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (`engine-render`
5 tests up from 4, `pytest tests/` 498 passed/1 skipped up from 494,
all 68 examples, showcase demo). A real gutter genuinely tracks a
live-edited buffer's own line count and lines up with the editor's own
real lines, proven by direct geometry comparison, not assumed. Zero
new Rust-level engine-py/engine-core API was needed for this phase.
