# Log: Phase 4, Step 4.3.1 -- SwmrSlotTable Tombstone Deletion & Recency Tracking

## Real bug found and fixed during implementation

**`insert`'s original tombstone-reuse logic only ever claimed a
tombstoned slot when the probe *also* reached a genuine `EMPTY_KEY`
first.** The initial implementation tracked `first_available` (the
earliest tombstone seen) but only actually claimed it inside the
`existing == EMPTY_KEY` branch. Once a table has been through enough
remove/insert cycles that every slot is either a live key or a tombstone
-- **zero remaining `EMPTY_KEY` slots at all** -- the loop runs its full
`capacity` iterations, never hits that branch, and falls through to
`false`, incorrectly reporting the table full even though a tombstoned
slot was available and should have been reused.

Caught immediately by the new
`a_slot_freed_by_remove_does_not_permanently_shrink_capacity` unit test
(a capacity-2 table, fully occupied by two real keys with no spare
`EMPTY_KEY` slot at all -- exactly the condition that triggers the bug):

```
thread 'swmr::tests::a_slot_freed_by_remove_does_not_permanently_shrink_capacity' panicked at crates/tre-memory/src/swmr.rs:412:9:
the slot Key(1) vacated must be reusable by a different key
```

**Fix:** restructured `insert` to check `first_available` once, after the
probe loop ends (whether it ended via an early `break` on a genuine
`EMPTY_KEY`, or by exhausting all `capacity` slots without ever finding
one) -- not only inside the `EMPTY_KEY` branch. This is the correct
general case: a tombstone found anywhere along the probe sequence is a
valid claim target the moment the sequence is confirmed not to contain
`key` itself, regardless of whether a genuine empty slot ever turns up.

This is exactly the kind of gap the project's own testing philosophy
exists to catch before it reaches a real caller -- a table with no spare
`EMPTY_KEY` slots left is not a contrived edge case; it's the *steady
state* any long-running atlas will reach once eviction has cycled through
its slots even once, making the original bug the single most likely
condition for a future caller to actually hit.

## What else was verified, not just unit-tested in isolation

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets
  -- -D warnings` / `cargo test --workspace`: all clean, zero warnings,
  zero failures, across every crate (not just `tre-memory`).
- All 25 `tre-memory` unit tests pass, including two new stress-style
  concurrency tests matching this module's own established testing
  discipline (not a new testing style introduced for this sub-step):
  `concurrent_readers_never_see_a_torn_value_while_a_remove_races_them`
  (6 reader threads doing 20,000 rounds of `get_and_touch` each while one
  key is concurrently removed) and the existing
  `concurrent_readers_see_a_fully_published_value_never_a_default` test,
  unaffected by this sub-step's changes.
- No RHI/example surface touched at all this sub-step (per the plan's own
  scope) -- all 15 pre-existing Vulkan examples still build and pass
  unmodified; not re-run individually since nothing in their dependency
  graph changed behavior (`tre-atlas`'s own code doesn't yet call any of
  the new `SwmrSlotTable` methods -- that's Step 4.3.3's job).
