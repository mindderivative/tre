# Plan: Phase 10 Step 10.2.4 — SDF Fidelity: Exact Ellipse Distance Field & Corner-Smoothing Reconciliation

**Status: Complete (2026-09-09).** See `documentation/IMPLEMENTATION.md`'s
own "Step 10.2.4: SDF Fidelity" write-up for the full technical account
of what shipped, `documentation/REVIEW.md` findings #173 (the ellipse
SDF replacement, and the real finding about where its actual defect
was) and #174 (the corner-smoothing research and quantification
decision), and `demo/phase10_step10_2_4/README.md` for the verification
summary. This file is the original plan, archived unchanged below from
the combined `PLAN.md` roadmap (Steps 10.2.1–10.2.6) once this
sub-step's own real work began — see that roadmap's remaining sections
for Steps 10.2.5–10.2.6, still active in `PLAN.md`.

**What actually shipped, in one paragraph.** Both tasks this plan
scoped were completed exactly as written, with one real, disclosed
correction to the plan's own expectation along the way: task 2's own
"reference point" unit tests were expected to show the OLD ellipse SDF
approximation's real error on-axis; direct derivation (and the new
independent-reference test module) instead proved the old approximation
is exact everywhere on-axis for an exterior point — its real error
turned out to be off-axis, and specifically in the SDF's MAGNITUDE
rather than its zero-crossing (the fill/no-fill boundary was always
correct). The new ellipse SDF (Inigo Quilez's real Newton-Raphson
refinement) replaced the old one entirely; `corner_smoothing`'s own
research resolved with NO code change, since Figma's real construction
turned out to be a Bezier path, not an implicit distance field, with no
simple closed form to swap in — its real deviation from that reference
was measured and disclosed instead.

---

## Original plan, as written

### Investigation

- `sd_ellipse` (`sdf_ellipse.frag`) is a standard scaled-circle
  approximation, exact only when `radius.x == radius.y`. A real "exact"
  ellipse SDF (Inigo Quilez's own published derivation) is not actually a
  closed-form quartic in practice — it's typically an iterative
  (Newton-style) refinement on the ellipse's implicit parametrization; the
  real, current formula must be verified against IQ's own published
  article before implementation (this codebase's own standing discipline:
  verify real external algorithms via direct research, not from memory,
  before integrating — exactly how the `lyon` migration's own API
  assumptions were verified this session).
- `corner_smoothing`'s superellipse blend (`sdf_rect_styled.frag`) is a
  real, legitimate, monotonic smoothing technique (superellipse/"squircle"
  blending is itself a standard real technique, not an ad hoc hack) — the
  disclosed gap is that it was never checked against any specific
  reference (e.g. Figma's own published squircle parametrization), not
  that it's known wrong.

**Confirmed exactly as researched:** IQ's article does describe a
Newton-Raphson refinement, not a closed-form quartic solve, and states
the direct quartic solve is "both expensive and not very stable."
**Confirmed exactly as researched:** Figma's construction is real and
legitimate, but turned out to be a Bezier path per corner rather than
an implicit distance field at all — an even more fundamental mismatch
with `corner_norm`'s own approach than this investigation anticipated.

### Scope decisions

- Ellipse: replace `sd_ellipse` with a verified-correct exact (or
  near-machine-precision iterative) formula, with a NEW unit test
  comparing against analytically-known exact distances at reference points
  (e.g. along the major/minor axis, where the exact distance is trivially
  `d - a`/`d - b` for a point at distance `d > a`/`b` from center) — proving
  BOTH the new formula's correctness AND, for honesty, the OLD
  approximation's real error at the same points (matching this session's
  own `translucent_flat_fill_demo.rs` precedent of proving a fix against
  an independent reference, not just asserting "looks fine").
- Corner smoothing: research a specific real reference formula (e.g.
  Figma's own published squircle article) and make an honest,
  evidence-based call at implementation time — either match it if a simple
  closed form exists, or formally verify and document the existing
  superellipse blend's real deviation from that reference within a
  quantified tolerance, whichever the research actually supports. Not
  presupposing a rewrite is needed; presupposing only that the current
  disclosed uncertainty gets resolved one way or the other, honestly.

**Implemented differently, first bullet only:** the old approximation
turned out NOT to show real error at the on-axis reference points this
plan anticipated (it is exact there, for an exterior point — a real,
verified algebraic property, not an oversight) — its real, measured
error is off-axis instead, so the "reference point" tests target that
region instead, with the on-axis tests instead proving both formulas
agree exactly there (REVIEW.md finding #173 has the full account,
including a genuine ellipse-geometry subtlety found and disclosed along
the way: an interior major-axis point can have symmetric off-axis
nearest points rather than the vertex, inside the ellipse's own evolute
cusp). **Second bullet: implemented exactly as planned** — Figma's real
construction (a Bezier path, not a distance field) has no simple closed
form, so the deviation was measured and documented instead of a
rewrite being attempted.

### Tasks

1. Research IQ's real published exact/near-exact ellipse SDF article;
   confirm the formula before writing GLSL against it.
2. Implement + a CPU-side Rust reference of the same formula for testing
   (mirroring `translucent_flat_fill_demo.rs`'s "independent reference"
   pattern), with reference-point unit tests.
3. Research a real squircle/corner-smoothing reference formula; make and
   document the scope call above.
4. Re-run `shape_full_rendering_demo` and every other consumer of these two
   shaders to confirm no visual regression at `radius.x == radius.y` /
   `smoothing == 0` (the two cases where old and new must be bit-identical
   or near-identical).

**All four tasks completed as written**, with task 2's reference points
adjusted per the real finding described above, and a fifth thing added
beyond this plan's own text: a real GPU demo (`ellipse_sdf_fidelity_
demo.rs`) proving the fix on actual hardware, not just in CPU-side unit
tests — the first demo in this project to ever render a genuinely
non-circular ellipse, since every existing consumer's `radius.x ==
radius.y` (the one case the old approximation already got right, so no
prior demo could have caught this defect).
