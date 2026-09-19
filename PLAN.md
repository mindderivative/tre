# PLAN — M31 Phase 2: Tab-Key Indentation Capture

## Goal
Let a focused *multiline* `TextField` (Code Editor) claim `Tab` before
`Tree::dispatch`'s top-level focus-traversal handling, inserting a
real `\t`. A single-line `TextField` must keep its own prior real
behavior (Tab still moves focus) byte-for-byte.

## Steps
1. Located the real chokepoint: `Tree::dispatch_text_field_key`'s own
   `Key::Tab | Key::Escape => None` catch-all — the exact "first
   refusal, `None` means not mine" contract already used for
   `ArrowLeft`/`ArrowRight`/`Home`/`End`/`Enter`.
2. Split it into a real `Key::Tab` arm: multiline inserts a literal
   `\t` (replacing a real active selection first, via the same shared
   `delete_selection` helper `Space`/`Enter` already use) and returns
   `Some(Changed(field))`; single-line returns `None` unchanged,
   falling through to the existing `move_focus` handling. `Escape`
   stays its own separate, always-`None` arm.
3. Updated the two stale doc comments that described Tab as always
   falling through (`dispatch_text_field_key`'s own doc comment and
   `Tree::dispatch`'s `KeyPressed` call-site comment).
4. Wrote 2 new engine-core unit tests (Tab inserts `\t` and keeps
   focus on a multiline field; Tab replaces a real active selection
   rather than inserting beside it) and re-ran the pre-existing
   single-line "Tab still moves focus" test to confirm zero regression.
5. Ran a real, direct empirical script before writing any pytest: a
   real Tab keypress on a focused multiline Code Editor inserts `\t`
   and keeps focus; the identical keypress on a focused single-line
   `TextField` still moves focus away.
6. Extended `tests/test_code_editor.py` (2 new tests) and
   `examples/code_editor.py` (a real Tab-key indentation step added to
   the existing live-edit sequence) rather than new files — a real
   enhancement to Code Editor's own existing coverage, not a separate
   capability.
7. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all 68 examples, showcase demo, mypy
   --strict.
8. Update `BUILD_TRACKER.md` — verified the parser's own reported item
   count before/after (191, unchanged), regenerate + republish the
   Build Tracker artifact.
9. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (`engine-core`
165 up from 163, `pytest tests/` 500 passed/1 skipped up from 498, all
68 examples, showcase demo). A real Tab keypress genuinely indents a
focused multiline Code Editor in place without losing focus, while a
single-line `TextField`'s own prior behavior stays byte-for-byte
unchanged, confirmed by direct empirical observation.
