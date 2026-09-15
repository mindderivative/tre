# Log: Phase 4, Step 4.3.2 -- AtlasPacker Free-Rectangle Reclamation

## What worked without needing further iteration

`remove`/`used_fraction` both worked correctly on the first real test
run -- no bugs, no rework, unlike Step 4.3.1's own `insert` tombstone-reuse
bug found one commit ago. The design decisions locked into the plan
(push the freed rect unmerged, cache `total_area` at construction rather
than trying to recover it later, `saturating_sub` as a cheap guard
against a caller-bug underflow) each held up exactly as reasoned through
during planning, with nothing surprising turning up during implementation:

- `remove_lets_a_full_atlas_accept_a_matching_size_again` and
  `exact_fill_then_free_then_refill_worked_example` both passed
  immediately -- confirming a genuinely full atlas (verified via a failed
  `insert(1, 1)`) accepts a matching-size request again after `remove`,
  and that the refilled/remaining placements still don't overlap.
- `remove_does_not_silently_merge_adjacent_free_rectangles` passed
  immediately too -- confirming the deliberate no-merge decision is a
  real, tested property (a request sized to two removed adjacent pieces'
  *combined* area correctly still fails), not just an unstated gap.
- `used_fraction_matches_hand_computed_values_across_insert_and_remove`'s
  hand-computed fractions (0.0 -> 0.04 -> 0.05 -> 0.04 across a 100x100
  atlas) matched on the first run, confirming the `total_area`-cached
  approach avoids the "the original free rectangle no longer exists once
  split" trap the plan called out in advance.

## Verification performed

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets
  -- -D warnings` / `cargo test --workspace`: all clean, zero warnings,
  zero failures across every crate. `tre-atlas`'s own test count is now
  16 (up from 11 before this sub-step): 5 new tests for `remove`/
  `used_fraction`, all passing immediately.
- `tre-atlas` keeps `#![forbid(unsafe_code)]` -- this sub-step needed no
  `unsafe` at all, consistent with the rest of the crate.
- No new example this sub-step, matching 4.3.1's own precedent -- the
  real end-to-end proof (a live eviction happening during a real GPU
  render) is Step 4.3.3's job.
- `atlas_packing_demo` and `atlas_concurrency_demo` (the two existing
  examples that exercise `AtlasPacker`/`AtlasOwner` directly) re-run for
  real against actual Vulkan hardware: zero validation errors, all
  existing assertions still pass -- neither yet calls `remove`/
  `used_fraction`, so this is a pure regression check, and it came back
  clean.
