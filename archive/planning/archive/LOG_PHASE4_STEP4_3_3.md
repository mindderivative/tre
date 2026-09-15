# Log: Phase 4, Step 4.3.3 -- Atlas LRU Eviction Policy & Wiring

## What worked without needing further iteration

The closing sub-step of Step 4.3 worked correctly end to end on the
first real run -- no bugs, matching 4.3.2's own pace rather than 4.3.1's
tombstone-reuse bug. Every design decision reasoned through during
planning held up exactly as expected once real code and a real demo
exercised it:

- The atlas-geometry design (a 64x64 atlas exactly holding four 32x32
  MSDF glyphs with zero leftover space) packed exactly as predicted --
  all four glyphs placed with no fragmentation, reaching precisely 100%
  capacity, confirming the "all pieces stay axis-aligned multiples of 32"
  reasoning from planning was correct.
- The cold-start fix (stamping a freshly-inserted entry's recency to its
  own creation frame via `get_and_touch` immediately after `insert`)
  worked on the first run: `atlas_eviction_demo`'s 'G' glyph, inserted at
  frame 0 and explicitly touched again at frame 700, survived the
  eviction pass exactly as expected, while 'L'/'Y'/'P' (inserted at frame
  0, never touched again) were correctly evicted when the fifth request
  arrived at frame 700 with the atlas already at 100% capacity.
- The paired `slots.remove`/`packer.remove` eviction logic left no
  leaked space and no resurrected keys -- the fifth glyph ('H') landed in
  real, reused atlas space with zero overlap against the surviving 'G'.
- The three new `tre-atlas` unit tests (eviction gated by capacity not
  just age, the cold-start regression test, and a full evict-then-insert
  scenario) all passed on the first run, each precisely engineered during
  planning to exercise the exact numeric thresholds (`0.85`, `600`
  frames) rather than approximate ones.
- `atlas_eviction_demo`'s real GPU render (uploading the finished atlas,
  drawing 'G' and 'H' through the existing, unmodified `msdf.frag`
  pipeline) confirmed both glyphs rendered real, non-background pixels on
  the very first run, with zero Vulkan validation errors.

## A deliberate API break, not an oversight

`AtlasOwnerHandle::request_insert` and `::lookup` both gained a
`current_frame: u64` parameter -- a real signature break to the one real
implementation and its one real caller (`atlas_concurrency_demo`,
updated to pass `0` since it doesn't exercise eviction), not an additive
change. This was the refinement this step's own plan called out in
advance: 4.3.1 only anticipated `lookup` needing a frame number, but
deciding whether to evict has to happen on the owner's own thread, which
needed a frame number threaded in via `request_insert` too.

## Verification performed

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets
  -- -D warnings` / `cargo test --workspace`: all clean, zero warnings,
  zero failures across every crate. `tre-atlas` now has 19 unit tests (up
  from 16 before this sub-step).
- `atlas_eviction_demo` (the new capstone example) run manually against
  the real GPU (Vulkan validation layer enabled): zero errors, all
  assertions pass, output PNG confirms both surviving glyphs render
  correctly.
- **All 16 examples** (the 15 pre-existing plus the new
  `atlas_eviction_demo`) re-run manually end to end: zero validation
  errors, zero regressions from the `request_insert`/`lookup` signature
  change.
- `atlas_eviction_demo` added to `.github/workflows/ci.yml`'s
  `vulkan-validation` job.

This closes Step 4.3 (4.3.1-4.3.3) and, with it, the Phase 1-4
comprehensive review's finding #114: a full atlas now degrades
gracefully via real LRU eviction instead of dropping new requests
forever.
