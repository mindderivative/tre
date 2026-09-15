# Plan: Phase 3, Step 3.1 -- SIMD Affine Matrix Math

## Scope decision (confirmed with the project owner, 2026-09-06)

Corresponds to IMPLEMENTATION.md's Step 3.1. Tasks 1 (`UiVertex`) and 3 (its
32-byte compile-time assertion) are **already done** -- both were built in
Phase 0's walking skeleton (`crates/tre-engine/src/lib.rs`'s `UiVertex`
struct and its `const _: () = assert!(...)`). This step's real remaining
work is task 2: SIMD-accelerated $3\times3$ affine transform composition in
the still-empty `tre-math` crate.

**Chosen over Steps 3.2/3.3, deliberately, as the phase opener:** this is
the first Phase 3 step to plan, chosen specifically because it is
self-contained, pure-CPU work with zero GPU/Vulkan dependency -- verified
by real, fast unit tests rather than another validation-layer demo. Every
step in this project so far has been GPU-heavy; this is a genuine change of
pace and a lower-risk way to start a new phase. Step 3.2 (SDF rounded
rects, real shader work) and Step 3.3 (SVG tessellation -- by far the
largest, riskiest piece of this phase) remain for later steps.

**No scene graph exists yet to consume this, and that's fine.**
TECHNICAL.md Section 7.2 describes batching parent-child transforms "during
the scene graph flattening phase" -- but no scene-graph tree exists in this
codebase (`RenderingCanvas` is a flat IR list; hierarchical node transforms
are UI-framework territory, much later phases). This mirrors the same
"build the real, tested primitive before its exact consumer exists"
pattern this project already used for `tre_memory::SpscRingBuffer` (Phase
1, built before any real second thread existed) and the dynamic ring
buffer/transient pool (Phase 2 Step 1, before Phase 6's execution stage
exists to feed them). This step's batch-compose API operates on plain
slices of transforms, not a tree -- wiring a real scene graph into it is
explicitly future work.

**Representation: 6 floats (2x3), not 9 (3x3).** TECHNICAL.md 7.2 writes
the matrix as $3\times3$ for mathematical clarity, but the bottom row is
always $[0, 0, 1]$ for any genuine affine transform -- storing it would
waste memory and SIMD lanes for zero benefit. `Affine2` (named for what it
represents -- a 2D affine transform -- rather than a generic `Mat3`, which
would wrongly suggest no structural assumption) stores exactly the six
meaningful values, matching TECHNICAL.md 7.2's layout: row 0 = `[a, b, tx]`,
row 1 = `[c, d, ty]`.

**Zero-allocation discipline applies here too.** If parent-child batch
composition runs every frame (the eventual scene-graph-flattening use
case), a function that returns a freshly heap-allocated `Vec` every call
would violate TECHNICAL.md Section 1's $0\text{ bytes/frame}$ steady-state
budget -- the exact rule that motivated Phase 2's entire ring-buffer/
transient-pool design. `compose_batch` therefore writes into a
caller-provided `&mut [Affine2]` output slice rather than allocating
internally.

**SIMD results are compared with a tolerance, not bit-for-bit equality.**
`wide::f32x8::mul_add` is true hardware FMA (a single rounding) wherever
the target supports it, and a separate multiply-then-add (two roundings)
otherwise, per TECHNICAL.md Section 2.2's own wording -- so the SIMD batch
path and a plain scalar reference implementation of the identical formula
can legitimately differ in the last bit or two of an `f32`. Tests assert
approximate equality (a small epsilon) against the scalar reference, not
exact equality, which is the honest way to verify this rather than a test
that happens to pass only on today's specific CPU.

**Verified on x86_64/AVX2 only.** This dev machine and the CI runner are
both x86_64; `wide`'s NEON (ARM64) code path is exercised by the exact same
source but is not run on real ARM hardware here -- an honest gap, same
category as this project's existing unverified-on-real-hardware notes
(e.g. Wayland input synthesis in Phase 1 Step 2), not a silent claim of
full cross-platform verification.

## Goal

`tre-math` gains a real `Affine2` type implementing TECHNICAL.md Section
7.2's exact translation/rotation/scale formula, matrix composition, and
point transformation -- plus a SIMD-batched composition function that
processes 8 parent-child pairs at a time via `wide::f32x8`, with a correct
scalar remainder path for lengths not a multiple of 8. Proven correct by
unit tests comparing the SIMD path against a scalar reference
implementation across a range of transforms, not just "it compiles and
doesn't panic."

## Tasks

1. **Add `wide` as a real dependency** of `tre-math` (the commented-out
   placeholder in its `Cargo.toml` already anticipates this).

2. **`Affine2` struct** (`#[derive(Debug, Clone, Copy, PartialEq)]`,
   `#[repr(C)]`): fields `a, b, tx, c, d, ty: f32`, matching TECHNICAL.md
   7.2's row layout exactly. Associated constant `IDENTITY`.

3. **Scalar constructors**, matching TECHNICAL.md 7.2's formula exactly:
   `from_translation(tx, ty)`, `from_rotation(theta)`, `from_scale(sx,
   sy)`, and the general `from_translation_rotation_scale(translation,
   rotation, scale)` composing all three in one call (the common case for
   a single UI node's local transform).

4. **`compose(&self, child: &Affine2) -> Affine2`** (`parent.compose(&child)`
   -- child's local transform applied first, then parent's, matching
   "parent-child world transform" terminology) and **`transform_point(&self,
   point: [f32; 2]) -> [f32; 2]`**, both plain scalar arithmetic -- the
   reference implementation the SIMD path is checked against, and the
   right tool for the common single-transform case where batching 8 at
   once has no one to batch with.

5. **`compose_batch(parents: &[Affine2], children: &[Affine2], out: &mut
   [Affine2])`**: asserts equal lengths (a length mismatch is a programmer
   error, not a recoverable runtime condition -- no `Result` needed here,
   unlike the GPU-resource-limited errors Phase 2 added). Processes input
   in chunks of 8 via `wide::f32x8` (loading each of the six `Affine2`
   fields across 8 parents/8 children into separate `f32x8` lanes, computing
   the six output components with `mul_add`, writing back), with a scalar
   fallback loop (task 4's `compose`) for the final `len % 8` remainder.

6. **Unit tests** (in `tre-math`, no GPU/display server involved -- `cargo
   test -p tre-math` alone proves this step, a first for this project):
   - Scalar formula tests: identity, pure translation, pure rotation (at
     several angles including $0$, $\pi/2$, $\pi$), pure scale (including
     negative scale, a legitimate flip), and a combined
     translate+rotate+scale case checked against hand-computed expected
     values.
   - `compose`/`transform_point` sanity: composing with `IDENTITY` is a
     no-op; composing a translation with a rotation is NOT commutative
     (order matters) and produces the documented result, not the reverse.
   - `compose_batch` vs. scalar reference, across slice lengths `0, 1, 7,
     8, 9, 16, 17` (every remainder case relative to the SIMD width) --
     comparing every output `Affine2` field with an epsilon tolerance, not
     exact equality (see the scope decision on FMA above).

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace -- `cargo test -p tre-math` specifically exercises this step's
  actual correctness claim, no GPU or display server required.
- No new example/demo needed this step: unlike every Phase 2 step, there
  is no GPU-facing behavior to screenshot or read back pixels from. The
  unit tests themselves are the verification artifact.

## Explicitly out of scope for this step

- Any real scene-graph/node-tree type, or wiring `Canvas`/`RenderingCanvas`
  to use `Affine2` at all -- no consumer exists yet; this step builds and
  proves the primitive, matching this project's established precedent.
- NEON/ARM64 verification on real hardware -- compiles via the same
  `wide`-provided source, not independently run here.
- `bytemuck::Pod`/`Zeroable` derives or any GPU-upload path for `Affine2`
  -- add if and when a real consumer needs to upload transforms to the
  GPU, not speculatively now.
- Steps 3.2 (SDF rounded rectangles) and 3.3 (SVG tessellation), planned
  separately once this step is complete.
