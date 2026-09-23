# LOG — M67: Eliminate Redundant Full-Content Clones in `draw_field`/`hit_test_position`

- Found by the same `/review-project` Performance-lens pass that
  surfaced M64/M65/M66, adversarially verified before scoping — the
  independent verification pass found the real cost was *worse* than
  first reported. `draw_field` (`crates/engine-render/src/text.rs`)
  unconditionally called `elide_folded_ranges(&state.content, &state.
  folded_ranges)`, which itself always allocated `String::with_capacity
  (content.len())` and copied the full content even when
  `folded_ranges` was empty -- then, on the common no-preedit path,
  cloned that already-fresh `folded_content` a second time into
  `display_content`. The verifier additionally found a *third*, wholly
  wasted clone: `state.content.clone()` in the no-preedit match arm,
  shadowed and discarded before use, never even reaching a caller.
  Real investigation, confirmed before scoping: the identical
  `elide_folded_ranges(...)` + conditional `.clone()` pattern also
  exists at `hit_test_position`, called on every click/hit-test against
  a `TextField` -- the fix needed to cover both call sites, not just
  `draw_field`.

## What shipped (single milestone, both phases)

1. Removed the dead clone. `draw_field`'s first match used to compute
   `display_content` directly, with a no-preedit arm that cloned
   `state.content` only to have it immediately shadowed and discarded.
   Restructured to return `Option<String>` (`preedit_display`) instead
   -- the no-preedit arm now does no work at all, and the real
   `display_content` is computed once, later, from whichever source
   actually applies.
2. `Cow`-ified the folding path. `elide_folded_ranges`'s signature
   widened from `fn(&str, &[Range<usize>]) -> String` to `fn<'a>(&'a
   str, &[Range<usize>]) -> Cow<'a, str>`, with an early `if folded.
   is_empty() { return Cow::Borrowed(content); }` -- zero allocation in
   the common unfolded case -- and the existing build logic wrapped in
   `Cow::Owned(out)` only when something is genuinely folded.
3. Both real call sites updated for the new return type: `hit_test_
   position`'s `content` binding is now `Cow<'_, str>` (`Cow::Owned
   (substitute_whitespace(&folded_content))` when showing whitespace,
   or the already-`Cow` `folded_content.clone()` otherwise -- a cheap
   pointer copy for the `Borrowed` variant, not a content clone);
   `draw_field`'s second `display_content` computation is now a 3-way
   match over `preedit_display` first, falling to the same Cow-typed
   whitespace/plain logic only when there's no preedit.
- **Design decision, deliberately not pursued further:** considered
  moving `folded_content` into `content` in `hit_test_position`'s
  `else` branch to avoid even the cheap `Cow::Borrowed` copy, but
  `folded_content` is referenced again later in a separate `if state.
  show_whitespace` block the borrow checker doesn't correlate with the
  first `if` -- a move there would be flagged as used-after-move.
  Stayed within the scoped fix rather than over-engineering past it.
- Tests: existing `hit_test_position`/selection/cursor test suite
  re-ran and passed completely unchanged as the real regression bar --
  not just a new test -- plus 1 new test, `elide_folded_ranges_borrows_
  when_nothing_is_folded_and_owns_when_something_is`, asserting the
  `Cow::Borrowed`/`Cow::Owned` variant directly via `matches!` for the
  empty-vs-non-empty `folded_ranges` cases.
- `BUILD_TRACKER.md`: full Milestone 67 section, Top Metrics row at
  100%, Just-closed/Up-next refreshed -- Up-next now states nothing is
  currently scoped, since this closes the last of the four `/review-
  project` findings. Tracker regenerated (18 milestones/55 phases/137
  items/3 known gaps/25 fixed gaps), artifact republished.
- Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean; `cargo test --workspace --release` (`engine-render` 34, up
  from 33, +1; every other crate's own count unchanged -- engine-core
  229, engine-md3 24, engine-platform 11, engine-py 30, engine-spec 83
  -- notably including every pre-existing test already in this same
  test module, which pass completely unchanged, real proof this
  restructuring didn't alter external behavior anywhere, not just "it
  compiles"); `maturin develop --release`; `pytest tests/` 831 passed,
  2 skipped, unchanged from the pre-milestone baseline -- pure internal
  Rust-side change with no new Python-facing surface, the identical
  "Rust-tested only" precedent M64/M65/M66 already established; every
  file in `examples/` ran clean; `demo/showcase.py` (all 5 phases, exit
  0).

## Status

**M67 is complete, both phases.** The fourth and last of the four
`/review-project` performance findings is closed with zero regression
to any existing test, example, or the showcase demo, and with direct
evidence (the full pre-existing cursor/selection/hit-test test suite
passing unchanged, plus a new test targeting the exact `Cow` variant)
that the restructuring is a pure internal change. Committing locally
now; push deferred pending explicit user confirmation.

All four milestones scoped from the `/review-project` audit (M64-M67)
are now complete. The one Architecture finding from that same review
was already fixed as a pure doc correction (`ARCHITECTURE.md` §8);
Security and Modernization returned zero findings. Nothing further is
currently scoped -- next steps are the user's to direct.
