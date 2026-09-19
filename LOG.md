# LOG — M38 Phase 3: Fold-Aware Cursor Navigation

- User's own explicit instruction: "Let's knock out the known gaps"
  -- M38's own third phase, per the milestone's own scoping order.
- Confirmed the real gap and its own already-established real
  convention before writing any code: `TextFieldState.folded_ranges`'s
  own doc comment (`crates/engine-core/src/node.rs`) explicitly named
  "cursor navigation is not fold-aware" as a real, deliberate v1
  simplification -- four separate doc comments across the codebase
  (`engine-core::node`, `engine-render::text`, `engine-py::node`,
  `python/tre/_core.pyi`) all repeated this same stated gap.
  `engine-render::text::to_display_offset_folded`'s own doc comment
  already states the real convention this phase needed to mirror for
  cursor navigation too: an offset landing inside a fold resolves to
  right after that fold's own real marker (`range.end`).
- New `Tree::snap_out_of_fold(cursor, content, folded) -> usize`
  (`tree.rs`): the same defensive normalization `engine-render`'s own
  `fold_segments` already applies (skip malformed/overlapping/out-of-
  bounds ranges via a `consumed` watermark, mirroring `fold_segments`'
  own `cursor` watermark) -- if the candidate position falls strictly
  inside a real range, returns `range.end`; a position exactly at a
  fold's own `start` or `end` is left alone (both real, visible
  boundaries, not hidden content).
- Wired into all four real landing computations in `dispatch_text_
  field_key`: `Home`'s and `End`'s own `target` (both the multiline
  and single-line branches, though single-line fields rarely carry
  folds in practice) now route through `snap_out_of_fold` before
  assignment; `ArrowUp`/`ArrowDown`'s own `move_to_line` result does
  too. `goal_column` itself is deliberately left unmodified by the
  snap -- the user's own intended column persists even when the
  actual landing had to move, the identical "goal survives a real
  detour" reasoning Phase 2's own shorter-line clamp already
  established.
- Corrected four now-stale doc comments claiming "cursor navigation is
  not fold-aware" as a real, permanent v1 limitation, since it no
  longer is: `TextFieldState.folded_ranges` (`node.rs`) now documents
  the real M38 Phase 3 fix directly; `engine-render::text::to_
  display_offset_folded`'s own comment (`text.rs`) explicitly notes
  its own clamp is *not* made redundant by this -- it's still a real,
  necessary fallback for every path that doesn't go through `Tree::
  snap_out_of_fold` (a real click via `Tree::set_text_field_cursor`'s
  own hit-test, deliberately unchanged, per Phase 3's own scoping
  naming only `Home`/`End`/`ArrowUp`/`ArrowDown`); `engine-py::node::
  set_folded_ranges`'s own doc comment and its `python/tre/_core.pyi`
  stub both updated to match.
- Added three new, decisive Rust tests to `tree.rs`, using a new
  `set_folded_ranges` test-scene helper (direct private-field access
  via `mod tests` being a child module of `tree.rs`'s own top-level
  module -- the same real access every other direct-state test setup
  in this file already uses):
  1. `arrow_down_snaps_the_cursor_out_of_a_folded_range_it_would_
     otherwise_land_inside` -- "one\ntwo\nthree\nfour" with a
     deliberately non-real-line-aligned fold `4..15` ("two\nthree\nfo"
     -- folds are arbitrary app-supplied byte ranges with no line-
     alignment guarantee per `folded_ranges`'s own doc comment, so
     this is a real case, not just the tidy aligned one). `ArrowDown`
     from real column 2 in "one" lands naturally on byte 6 (strictly
     inside the fold) and must snap to byte 15.
  2. `home_and_end_also_snap_the_cursor_out_of_a_folded_range` -- from
     a cursor already inside the fold (byte 10, inside "three"), both
     `Home`'s own natural landing (byte 8) and `End`'s own natural
     landing (byte 13) are each strictly inside `4..15` and must both
     snap to byte 15.
  3. `landing_exactly_at_a_folds_own_boundary_is_left_alone` -- a
     narrower, real-line-aligned fold `4..7` (just "two"); `ArrowDown`
     from real column 0 lands exactly on the fold's own start
     boundary (byte 4) and must be left there, not force-moved.
  Hit one real clippy issue while writing these: `vec![4..15]`/
  `vec![4..7]` (and `Vec::from([...])`, tried as a first fix) both
  trip `clippy::single_range_in_vec_init` (a real lint catching the
  common `vec![a..b]` typo for `(a..b).collect()`) -- fixed properly
  by changing `set_folded_ranges`'s own test-helper signature from
  `Vec<Range<usize>>` to a plain `Range<usize>` parameter (every real
  call site only ever needed one range anyway), wrapping it in
  `vec![range]` *inside* the helper where `range` is a variable, not
  a literal `a..b`, which the lint doesn't fire on.
- Real, honest verification-surface check done properly this time
  (the same discipline Phase 2 established, not reverting to the
  reflexive "assume no Python path exists" default): checked `python/
  tre/_core.pyi` first and found `Node.set_folded_ranges` is a real,
  already-existing pyo3 binding (`engine-py/src/node.rs:1004`) --
  reused the identical `type_text`-after-navigation-then-`get_text()`
  positional probe the goal-column tests already established. One new
  pytest test, `test_arrow_down_snaps_the_cursor_out_of_a_folded_
  range_it_would_otherwise_land_inside` (`tests/test_code_editor.py`),
  reusing the exact same "one\ntwo\nthree\nfour" / fold `(4, 15)`
  scene as the Rust-level test for direct cross-checking -- asserts
  `get_text() == "one\ntwo\nthree\nfXour"` (the marker landing right
  after 'f', at byte 15, mirroring the Rust test's own byte-15 proof).
  Passed on the first run.
- Full verification: `cargo check --workspace --all-targets`/`cargo
  clippy --workspace --all-targets -D warnings`/`cargo fmt --check`
  clean; `cargo test --workspace --release` clean (`engine-core` 188
  passed, up from 185, exactly the 3 new tests, zero regressions);
  `maturin develop --release` rebuilt; `pytest tests/` 559 passed/1
  skipped, up from 558, exactly the 1 new test; all 75 examples and
  the showcase demo re-run clean.
- Updated `BUILD_TRACKER.md`: Phase 3 flipped `⬜` -> `✅` with a
  terse step-bullet note; milestone status line and Top Metrics row
  updated to "Phase 3 of 7 done" / 43%. Verified the parser's own
  reported item count unchanged before/after (38/122/212 both times).
  Regenerated and republished the Build Tracker artifact.
  **This closes M38 Phase 3. M38 itself remains open -- 4 phases
  remain (Split Button inner-corner shape-tightening, Button Group
  per-child shape change on press/select, ScrollView scrollbar thumb,
  real scroll+clip for Code Editor with caret-follow).**
