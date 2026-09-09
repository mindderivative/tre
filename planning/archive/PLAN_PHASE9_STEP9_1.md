# Plan: Phase 9, Step 9.1 -- Correctness Test Suite

## Goal

Build the real correctness test suite IMPLEMENTATION.md Phase 9 Step
9.1 specifies: adversarial sort-key testing, atlas fragmentation/
eviction-churn testing, a batching-equivalence pixel-diff test, and
adversarial SVG-input testing. Step 9.2 (zero-allocation/balance CI
gates) is explicitly out of scope for this pass (confirmed with the
project owner) -- picked up separately once this closes.

## Real discrepancies found during pre-work, each confirmed with the
## project owner before proceeding

1. **No radix sort exists.** TECHNICAL.md Section 4 / ARCHITECTURE.md
   Section 4.1 both specify a linear $O(N)$, 4-pass radix sort for the
   64-bit draw-command key; the real code (`flatten_run`) has always
   used `sort_unstable_by_key` (a comparison sort) instead. Confirmed:
   **build the real radix sort now**, matching the documented design,
   then write the adversarial tests against it -- fulfilling Step
   9.1's own real intent, not just testing whatever happened to exist.
2. **The documented atlas-exhaustion placeholder fallback was never
   built.** DESIGN.md Section 2.6 describes falling back to "a
   lower-fidelity placeholder (e.g., a bounding-box glyph or
   solid-color swatch)" when eviction can't free enough space; the
   real code (`process_insert`) silently drops the request instead,
   with a prior session's own comment already reasoning that this
   satisfies the "report, don't block" contract. Confirmed: **correct
   DESIGN.md to describe the real, already-shipped drop behavior**,
   and test that -- no new placeholder-rendering logic.

## Scope decisions

1. **Radix sort: generic, 4-pass, 16-bit-per-pass LSD, built in
   `tre-engine`.** Signature `fn radix_sort_by_key<T: Copy>(items:
   &mut [T], scratch: &mut [T], key_fn: impl Fn(&T) -> u64)` -- generic
   over the key-extraction closure purely for clean, direct
   unit-testability (matching `sort_unstable_by_key`'s own signature
   shape), not published as a new cross-crate primitive: `UiDrawCommand`
   sorting is its only real consumer today, and this project's own
   established precedent (`planning/archive/PLAN_PHASE7_STEP7_2_1_
   NONBINDLESS_CONVERSION.md`'s explicit scope decision) is that a
   primitive earns promotion to a shared crate (`tre-memory`/`tre-math`)
   once a second real caller needs it, not speculatively. `scratch` is
   caller-provided so no per-call heap allocation is needed --
   `segment_and_flatten` allocates one reusable buffer sized to the
   whole frame's own command count (an upper bound for any single run)
   and passes a sub-slice into each `flatten_run` call, matching this
   project's own "allocate once, reuse across the frame" discipline
   (transient pools, ring buffers) rather than allocating fresh scratch
   space per run.
2. **No small-N comparison-sort fallback threshold.** TECHNICAL.md's
   own specification is unconditional; Step 9.1 is explicitly a
   correctness pass, not a performance-tuning one (that's TECHNICAL.md
   Section 9.2's own criterion-based benchmark suite, separate,
   unstarted work) -- adding a hybrid threshold now would be a real
   performance-tuning decision with no benchmark data behind it.
   Disclosed as a known, deliberate scope boundary in the code, not
   silently decided.
3. **Atlas-exhaustion behavior: document reality, don't build a new
   fallback** (confirmed with the project owner) -- DESIGN.md Section
   2.6 corrected to describe the real silent-drop behavior;
   `process_insert`'s own existing code comment already independently
   reasoned through why this satisfies the section's "report, don't
   block" contract, so this is a documentation fix, not new logic.
4. **Batching-equivalence test needs one small, new, clearly-scoped
   `tre-engine` API addition**: `RenderingCanvas::flatten_unbatched`,
   identical to `flatten()` except it skips `flatten_run`'s own merge
   step (still uses the real sort) -- isolates *batching* specifically
   as the variable under test, matching the task's own literal
   wording ("a batching or sort-key bug, not a performance
   regression"). Clearly documented as a validation-only utility, not
   a production rendering path. A new real GPU demo
   (`batching_equivalence_demo.rs`) renders the same real scene both
   ways and pixel-diffs the two outputs, asserting exact equality.
5. **SVG fuzzing via `proptest`, not `cargo-fuzz`.** This environment
   has no nightly Rust toolchain installed (`cargo-fuzz` requires
   nightly + libFuzzer); `proptest` is a well-established,
   stable-Rust-compatible property-testing crate that directly serves
   the task's real intent (adversarial/malformed input generation,
   bounded-behavior assertions) without a new toolchain dependency.
   Added as a new `tre-svg` dev-dependency, disclosed plainly as an
   environment-driven substitution, not a silent scope reduction --
   real coverage-guided fuzzing via `cargo-fuzz` remains a genuine,
   separate future upgrade once nightly Rust is available in CI.

## Tasks

1. `crates/tre-engine/src/lib.rs`: `radix_sort_by_key` (generic, 4-pass
   16-bit LSD); `segment_and_flatten` allocates one reusable scratch
   buffer per frame; `flatten_run` calls `radix_sort_by_key` instead of
   `sort_unstable_by_key`.
2. Adversarial radix-sort unit tests: all-identical keys, reverse-sorted
   input, keys clustered at each field's own bit boundary, maximum
   Depth ID (`0xFFFFF`), empty/single-element runs, and a differential
   property test against `sort_unstable_by_key` on randomized inputs
   (proving equivalence, not just spot-checking specific cases).
3. `crates/tre-atlas/src/owner.rs`: new adversarial fragmentation +
   sustained insert/evict churn tests, plus a real test for the
   eviction-insufficient/request-dropped path.
4. `documentation/DESIGN.md` Section 2.6: correct the atlas-exhaustion
   bullet to describe the real drop behavior. `documentation/
   TECHNICAL.md`/`ARCHITECTURE.md` Section 4/4.1: mark the radix sort
   real/implemented once built. `documentation/IMPLEMENTATION.md`
   Phase 9 Step 9.1: real write-up documenting both discrepancies found
   and how each was resolved.
5. `RenderingCanvas::flatten_unbatched` (`tre-engine`); new demo
   `crates/tre-rhi-vulkan/examples/batching_equivalence_demo.rs`
   (real GPU pixel-diff, exact equality assertion). Add to `ci.yml`'s
   `vulkan-validation` job.
6. `crates/tre-svg/Cargo.toml`: add `proptest` dev-dependency;
   property tests generating adversarial/malformed path/SVG data
   against `parse_svg`/`triangulate`/tessellation, asserting bounded
   time and no panics.
7. `demo/phase9_step9_1/`: README + run script + output screenshot for
   the batching-equivalence demo, matching every prior demo-bearing
   step.

## Verification plan

- `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the
  workspace.
- The new radix-sort tests and atlas tests pass; the differential
  property test against `sort_unstable_by_key` passes across many
  randomized runs, not just once.
- `batching_equivalence_demo.rs`'s own real GPU pixel-diff assertion
  passes, re-run manually at least twice to confirm not a one-off.
- Full regression sweep: every pre-existing demo re-run manually, zero
  regressions -- every existing `execute_frame`/`flatten()` caller
  specifically, since the sort algorithm underneath changed.
- Commit; push only on explicit "push it"; `gh run watch` after any
  push (`accessibility-validation`'s pre-existing, documented failure
  expected and unrelated).

## Explicitly out of scope

- Step 9.2 (zero-allocation debug guard, balance-assertion CI gating)
  -- confirmed with the project owner as a separate future pass.
- A small-N comparison-sort fallback threshold for the radix sort --
  real, deliberate, disclosed future optimization once real benchmark
  data justifies it (Step 9.2/TECHNICAL.md Section 9.2's own concern,
  not this correctness-focused step's).
- Building the DESIGN.md-documented placeholder-glyph atlas-exhaustion
  fallback -- confirmed with the project owner as real, separate,
  not-yet-needed future work; this step documents and tests the real,
  already-shipped drop behavior instead.
- Real coverage-guided `cargo-fuzz` fuzzing -- this environment has no
  nightly Rust toolchain; `proptest`-based property testing substitutes
  for now, disclosed plainly, not silently.
