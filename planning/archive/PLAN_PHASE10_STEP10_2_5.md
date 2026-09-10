# Plan: Phase 10 Step 10.2.5 — Rounded Stroke Caps on Partial-Arc Circles/Ellipses

**Status: Complete (2026-09-09).** See `documentation/IMPLEMENTATION.md`'s
own "Step 10.2.5: Rounded Stroke Caps" write-up for the full technical
account of what shipped, `documentation/REVIEW.md` finding #175, and
`demo/phase10_step10_2_5/README.md` for the verification summary. This
file is the original plan, archived unchanged below from the combined
`PLAN.md` roadmap (Steps 10.2.1–10.2.6) once this sub-step's own real
work began — see that roadmap's remaining section for Step 10.2.6,
still active in `PLAN.md`.

**What actually shipped, in one paragraph.** Implemented exactly as
planned: a new `cap_sdf` function computing the signed distance to a
real circle of radius `border_thickness / 2`, centered on the border
band's own centerline at each of the arc's two cut angles, unioned into
the existing sector-clipped ellipse SDF via `min()`. The one real
addition beyond this plan's own text: since there is no independent CPU
formula for "the correct antialiased 2D render of a rounded cap" the
way `sd_ellipse`'s own distance value had one in Step 10.2.4, a real GPU
demo (`arc_rounded_cap_demo.rs`) was built to prove the fix directly on
real pixels, rather than relying solely on a visual re-check of
`shape_full_rendering_demo`'s own existing scene.

---

## Original plan, as written

### Investigation

- `sdf_ellipse.frag`'s sector cutoff (`if (relative > arc_sweep_angle) { d
  = max(d, 0.001); }`) hard-clips the fill/stroke past the arc's own sweep
  -- no cap geometry, so a partial-arc progress-ring-style `Circle`'s two
  cut edges are flat, not rounded, even when `stroke_line_cap` (already a
  real field, borrowed from `Path`'s own convention conceptually) would
  imply otherwise.
- A real alternative considered and rejected: route partial-arc circles
  through `lyon`'s own tessellated stroke path (flatten the arc into a
  polyline, let `lyon`'s real `LineCap::Round` handle the caps) — this
  session's own precedent for "don't hand-roll what lyon already solves."
  Rejected here specifically because `draw_flat_polygon`'s tessellated
  triangles have NO antialiasing (confirmed this session: hard triangle
  edges only), and circles/rings are a highly AA-sensitive, extremely
  common real UI element (progress indicators) — trading the SDF
  pipeline's existing `fwidth`-based smooth edges for jagged tessellated
  ones would be a real visual regression for a common case, not a neutral
  implementation-detail swap. An analytic SDF cap keeps the existing
  antialiasing.

**Confirmed exactly as researched** -- the tessellated-stroke
alternative was never revisited; the analytic SDF cap approach shipped
directly.

### Scope decisions

- Real analytic rounded caps: at each of the arc's two cut angles, union
  (`min()`) the existing sector-clipped ellipse SDF with two small circle
  SDFs of radius `border_thickness / 2`, centered at the point where the
  stroke band's own centerline meets that cut angle on the ellipse
  boundary — the standard 2D "rounded line/capsule" SDF technique, applied
  at the two cut points instead of a straight segment's two ends.
- Disclosed approximation carried over from `sd_ellipse` itself for a true
  (non-circular) `Ellipse`: the cap-center placement uses the LOCAL
  boundary point at each cut angle (exact for a `Circle`, a real,
  consistent approximation for a non-uniform-radius `Ellipse`, in the same
  spirit as `sd_ellipse`'s own existing disclosed approximation, now
  narrowed by 10.2.4 to only this one remaining case).
- Only applies when `border_thickness > 0.0` and `arc_sweep_angle < TAU`
  (a full ellipse or a borderless partial arc needs no cap geometry at
  all — matches the shader's own existing early-out for a full sweep).

**Implemented exactly as planned**, all three bullets.

### Tasks

1. Derive and implement the two-cap-circle SDF union in `sd_ellipse`'s
   sector-cutoff branch.
2. New tests: a partial-arc `Circle` with a real border, pixel-sampled at
   both cut edges, confirming a real rounded (not flat) transition — a
   direct visual/pixel proof, matching this session's own established
   demo discipline.
3. Re-verify `shape_full_rendering_demo`'s own existing arc-wedge exclusion
   test still passes unchanged (a full sweep and a borderless arc must be
   bit-identical to before).

**All three tasks completed as written.** Task 2's "pixel-sampled" tests
took the form of a new real GPU demo (`arc_rounded_cap_demo.rs`), since
no CPU-side reference formula for the actual antialiased render exists
to unit-test against the way earlier sub-steps' shader math did — the
real per-pixel shader behavior IS the thing being proven here. Task 3's
own re-verification also caught a genuine visual improvement in the
existing demo's own render (its 270°-sweep bordered circle now shows
real rounded caps where it previously had flat ones), not just a
"no regression" confirmation.
