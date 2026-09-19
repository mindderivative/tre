# PLAN — M38 Phase 3: Fold-Aware Cursor Navigation

## Goal
Close the real, previously-documented v1 gap: `Tree::dispatch_text_
field_key`'s `Home`/`End`/`ArrowUp`/`ArrowDown` moved through
`TextFieldState.content`'s own real, unfolded bytes with no awareness
of `folded_ranges` at all, so a real cursor could land somewhere
genuinely invisible (inside a collapsed "⋯" region) -- `folded_
ranges`'s own doc comment named this exact gap directly, four times
across the codebase (`engine-core::node`, `engine-render::text`,
`engine-py::node`, `python/tre/_core.pyi`).

## Steps
1. Confirmed the exact existing paint-time precedent to mirror:
   `engine-render::text::to_display_offset_folded`'s own doc comment
   already states the real convention for "an offset lands inside a
   fold": resolve to right after that fold's own real marker (i.e.
   the fold's own `range.end`). Cursor navigation should apply the
   identical rule, not invent a second one.
2. New `Tree::snap_out_of_fold(cursor, content, folded) -> usize`
   (`crates/engine-core/src/tree.rs`): walks `folded_ranges` with the
   same defensive normalization `engine-render`'s own `fold_segments`
   already applies (skip malformed/overlapping/out-of-bounds ranges,
   `engine-core` never validates `folded_ranges` itself); if the
   candidate cursor position falls *strictly* inside a real range
   (`range.start < cursor < range.end`), returns `range.end`; a
   position exactly at a fold's own `start` or `end` is left alone
   (both are real, visible boundaries).
3. Wired into all four real landing computations in `dispatch_text_
   field_key`: `Home`, `End`, and the `move_to_line` result inside
   both `ArrowUp`/`ArrowDown` -- each now passes its own computed
   target through `snap_out_of_fold` before assigning `state.cursor`.
4. Corrected four now-stale doc comments that explicitly named "cursor
   navigation is not fold-aware" as a real, deliberate v1 limitation:
   `TextFieldState.folded_ranges` (`node.rs`), `to_display_offset_
   folded` (`engine-render/src/text.rs` -- noted the paint-time clamp
   is still a genuinely necessary fallback for other paths like mouse
   click, not made redundant), `engine-py::node::set_folded_ranges`,
   and its pyo3 stub in `python/tre/_core.pyi`.
5. Added three new decisive Rust tests: a real `ArrowDown` landing
   strictly inside a deliberately non-line-aligned fold that snaps
   forward; `Home`/`End` both snapping out of a fold from a cursor
   already inside it; a real boundary case proving a landing exactly
   at a fold's own `start` is left alone, not force-moved.
6. Checked the Python API for a real reproduction path before
   assuming one didn't exist (the correct process this session's own
   M37 case first established, applied properly this time rather than
   skipped): `Node.set_folded_ranges` is a real, existing pyo3
   binding -- reused the same `type_text`-after-navigation-then-
   `get_text()` positional probe the goal-column tests already use.
   One new pytest test added to `tests/test_code_editor.py`.
7. Full verification chain: `cargo check`/`clippy -D warnings`/`fmt`/
   `test --workspace --release`, `maturin develop --release`, full
   `pytest tests/`, all 75 examples, showcase demo.
8. `BUILD_TRACKER.md` Phase 3 flipped to done (terse step-bullet note,
   full writeup here in `PLAN.md`/`LOG.md`), Top Metrics row updated
   to 3-of-7, artifact regenerated (38/122/212, unchanged) and
   republished.

## Status
Complete. Full verification chain green (`cargo test --workspace
--release`: `engine-core` 188 passed, up from 185, +3 new tests;
`pytest tests/`: 559 passed/1 skipped, up from 558, +1 new test; all
75 examples + showcase demo clean). **M38 Phase 3 -- Fold-Aware
Cursor Navigation is now complete. M38 itself remains open: 4 phases
remain (Split Button inner-corner shape-tightening, Button Group
per-child shape change, ScrollView scrollbar thumb, real scroll+clip
for Code Editor).**
