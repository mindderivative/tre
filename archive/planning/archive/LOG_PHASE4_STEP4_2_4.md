# Log: Phase 4, Step 4.2.4 -- Multi-Window Atlas Concurrency

## Finding 1 (caught by this step's own demo): `thread::yield_now()` is
only a scheduler hint, not a real wait -- a tight polling loop using it
can burn through its entire retry budget without the OS ever actually
switching to the thread being waited on

The demo's first draft polled `AtlasOwnerHandle::lookup` for each
letter's result in a loop backed by `thread::yield_now()` between
attempts (100,000 attempts). Every single lookup failed -- not because
the concurrency primitives were wrong (`tre-memory`'s own unit tests,
including real multi-threaded stress tests, all passed on the first
attempt and still do), but because this environment's scheduler
apparently never actually context-switched to the atlas owner's
background thread across those 100,000 `yield_now()` calls, letting the
whole polling budget exhaust in what was probably a handful of
milliseconds of pure CPU time on the calling thread alone.

Confirmed by adding a diagnostic that switched the same loop to a real
`thread::sleep(Duration::from_millis(2))` between attempts instead: every
letter resolved correctly, almost immediately. `yield_now()`'s own
documentation is explicit that it is "a hint" the OS scheduler is free to
ignore entirely -- this project's own producer-request retry loop (also
originally `yield_now()`-based) had the identical latent problem, fixed
the same way.

**Change:** both the producer's own `request_insert` retry loop and the
result-polling loop now use a real `sleep`-based backoff. Not a defect in
`MpscRingBuffer`, `SwmrSlotTable`, or `AtlasOwner` themselves -- confirmed
separately via `tre-atlas`'s own unit tests, which use real
`thread::spawn` and passed without needing this fix, since those tests'
consumer-side polling loop was written with a bounded retry count backed
by `thread::yield_now()` too, but at a scale (tens of iterations) where
this environment's scheduler happened not to starve it. This demo's much
larger 100,000-iteration budget made the gap between "hint" and
"guarantee" actually visible.

## Finding 2 (caught by this step's own demo, a repeat of a lesson from
Step 4.1): a glyph's bounding-box *center* is not guaranteed to land on
real glyph material

The demo's first draft verified each letter actually rendered by
checking only its on-screen bounding-box center pixel against the
background. `'G'` failed this check -- its center pixel genuinely was
background, because `'G'`'s open counter can place empty space exactly at
its own bbox center, the identical property `'L'` already demonstrated in
Step 4.1's own demo (there, the *fix* was picking a hand-verified interior
point instead of the bbox center; this step re-discovered the same root
cause from a different angle since a *generic, per-letter* verification
loop can't hand-pick a good interior point for six different, arbitrary
real letters the way a single-letter demo can).

**Change:** rewrote the check to scan each letter's entire on-screen
quad for *any* pixel that differs from the background, rather than
relying on one specific point being reliably interior -- correct
regardless of a given letter's own shape, and the more appropriate check
for a demo that doesn't know in advance which specific letters it will
be asked to verify.

## What worked without needing further iteration

- `tre_memory::MpscRingBuffer<T>` (Dmitry Vyukov's bounded MPMC ring
  buffer design, simplified for a single consumer) passed its own real
  8-thread, 80,000-item concurrent stress test on the very first attempt
  -- no lost or duplicated items, no data races.
- `tre_memory::SwmrSlotTable<K>` (open-addressed, zero `unsafe`) passed
  every test on the first attempt, including a real two-thread test
  confirming a reader can never observe a key before its fully-published
  value.
- `AtlasOwner`'s own real multi-threaded round-trip test (6 producer
  threads x 5 requests each, 30 total, checked for correct resolution and
  zero overlap) passed on the first attempt.
- The actual packing/generation/publish pipeline, once given a *correct*
  wait mechanism (Finding 1's fix), worked immediately: all 6 real
  letters' MSDFs were generated correctly, packed without overlap, and
  matched byte-for-byte against independently regenerated reference MSDFs
  on the very first successful run.

## Verification performed

- `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D
  warnings` / `cargo test --workspace`: all clean, including 6 new
  `tre-memory` unit tests (`MpscRingBuffer`) and 11 new tests across
  `tre-atlas` (`SwmrSlotTable`'s open-addressing/publish-ordering tests,
  the `AtlasKey` packing round trip, and `AtlasOwner`'s own real
  multi-threaded round trip).
- `atlas_concurrency_demo` run manually against the real GPU (AMD/Radeon,
  Wayland session) under the Vulkan validation layer: all 6 letters
  resolved, non-overlapping, byte-identical to independently regenerated
  MSDFs, and rendered as real non-background pixels; output PNG visually
  inspected and clearly reads "GLYPHS".
- **All 15 pre-existing examples** re-run manually after this step's
  changes, zero validation errors and zero regressions.
