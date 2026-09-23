# LOG — M66: Zero-Allocation Cache-Hit Path in `shaped_layout`

- Found by the same `/review-project` Performance-lens pass that
  surfaced M64/M65, adversarially verified before scoping. `TextRenderer
  ::shaped_layout` -- the one real, working text-shaping cache in this
  codebase, keyed by `NodeId` -- unconditionally built an owned `Layout
  CacheKey` (`content.to_string()`, `font_family.to_string()`, `spans.
  to_vec()`) *before* the staleness check that decides whether a cache
  hit even needs it at all. Called once per visible `Text`/`Link` node
  (via `draw`) and once per `TextField` (via `draw_field`) on every
  dirty-frame scene rebuild, so the intended-cheap common case (an
  unchanged node whose shaping is genuinely being reused, the exact
  case the cache's own doc comment already claimed was cheap) still
  paid a full content/spans clone every single time, just to build a
  comparison key that got thrown away the instant the comparison
  confirmed nothing changed.

## What shipped (single milestone, both phases)

1. `shaped_layout` restructured: the staleness check now compares the
   *borrowed* new inputs (`content: &str`, `spans: &[(Range<usize>,
   Color)]`, etc.) directly against the cached key's own already-
   stored fields -- `String: PartialEq<str>` and `Vec<T>: PartialEq<
   &[U]>` (std's own blanket impls) make this a real, zero-allocation
   comparison, not a workaround. `LayoutCacheKey` kept its own
   `#[derive(PartialEq)]` -- it's simply no longer compared as one
   whole owned value against another, only used to construct/store the
   real cache entry itself.
2. The owned `LayoutCacheKey` (and the reshape itself) now only ever
   gets constructed inside the real stale/miss branch, right where
   `layout_cache.insert` happens -- exactly the "compare borrowed
   inputs before ever allocating an owned key" technique M64's own new
   `shaped_terminal_run` was deliberately written with from the start
   (its own doc comment named `shaped_layout`'s pattern directly as
   the one *not* to repeat), now applied back to the original code
   that pattern was written to avoid.
- Tests: 1 new Rust unit test, `shaped_layout_reshapes_on_a_change_to_
  any_single_real_field` -- real, per-field regression coverage for
  the new hand-written comparison. Shapes a baseline `Layout` once,
  then changes exactly one of the 9 real shaping inputs at a time
  (`content`/`font_family`/`font_weight`/`font_size`/`max_width`/
  `align`/`line_height`/`spans`/`default_color`), keeping every other
  one byte-for-byte identical, and confirms the cached key's own field
  actually updated after each individual change -- real, direct proof
  of a genuine reshape per field, not just "the cache didn't grow"
  (which a bug that silently *always* reshapes would also satisfy).
  This protects against the one concrete risk a hand-written
  comparison has that a derived `PartialEq` doesn't: a field silently
  missing from the manual check would quietly stop invalidating the
  cache on that one input, with nothing else in the type system
  catching it.
- `BUILD_TRACKER.md`: full Milestone 66 section, Top Metrics row at
  100%, Just-closed/Up-next refreshed to point at M67 as the one
  remaining item in the backlog. Tracker regenerated (18 milestones/55
  phases/137 items/3 known gaps/25 fixed gaps), artifact republished.
- Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean; `cargo test --workspace --release` (`engine-render` 33, up
  from 32, +1; every other crate's own count unchanged -- notably
  including all 32 pre-existing tests already in this same test module,
  which pass completely unchanged, real proof this restructuring
  didn't alter external behavior anywhere, not just "it compiles");
  `maturin develop --release`; `pytest tests/` 831 passed, unchanged
  from the pre-milestone baseline -- pure internal Rust-side caching
  with no new Python-facing surface, the identical "Rust-tested only"
  precedent M64/M65 already established; every file in `examples/` ran
  clean; `demo/showcase.py` (all 5 phases, exit 0).

## Status

**M66 is complete, both phases.** The third of the four `/review-
project` performance findings is closed with zero regression to any
existing test, example, or the showcase demo, and with strong direct
evidence (32 pre-existing tests passing unchanged, plus a new per-field
test targeting the exact class of bug a hand-written comparison risks)
that the restructuring is a pure internal change. Committing locally
now; push deferred pending explicit user confirmation. Next: M67
(`draw_field`/`hit_test_position`'s redundant full-content clones
during code folding) -- the last of the four review findings.
