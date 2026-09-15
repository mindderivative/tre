# Demo: Phase 3, Step 3.2 -- Analytical SDF Rounded Rectangles

```bash
./demo/phase3_step3_2/run_sdf_rounded_rect_demo.sh
```

**What's actually new here:** every prior demo's rounded rect was really a
flat-colored quad -- `RenderingCanvas::draw_rounded_rect` has emitted
exactly 4 vertices/6 indices since Phase 0, but the fragment shader never
evaluated an actual signed distance field, so every corner was a hard
right angle regardless of what a caller asked for. This step adds a real,
dedicated `sdf_rounded_rect.{vert,frag}` shader pair that evaluates
TECHNICAL.md Section 5.2's exact formula
($d(\mathbf{p}) = \Vert\max(\mathbf{q},0)\Vert + \min(\max(q_x,q_y),0) - r$)
and its `fwidth`-based anti-aliasing, and `draw_rounded_rect` gained a real
`radius: f32` parameter to drive it.

**Verified by reading back actual pixels, not just "it compiles":**

- A deep-interior point is exactly the foreground color (pure white) --
  alpha clamps to exactly 1.0.
- A point well inside the bounding box's corner but clearly outside the
  rounding arc is exactly the background clear color -- alpha clamps to
  exactly 0.0, and premultiplied blending reproduces the background
  exactly.
- A block of pixels around the rounded corner's arc contains at least one
  genuinely blended pixel (neither foreground nor background) -- a real
  checked anti-aliased transition, not an assumption.

**A real, worth-knowing discovery from building this demo:** the first
version of the third check scanned the rect's *flat* left edge instead of
the corner, and found nothing but hard 0/1 transitions. This rect's flat
edges sit at exact integer canvas coordinates, so their entire 1px-wide
analytical AA ramp falls exactly between two pixel centers (at the
standard half-integer sample offsets) with no fractional-coverage sample
landing inside it -- an inherent property of this technique on a
pixel-aligned axis-aligned edge, not a bug in the shader or in `fwidth`.
The rounded corner's non-axis-aligned gradient has no such alignment and
reliably produces several genuinely partial-alpha pixels, which is also
the more representative place to check anyway, since the rounding itself
is what this step exists to prove (see REVIEW.md findings #84/#85 for the
full account, including the real `params` vertex-attribute gap this step
also found and fixed).

**Uniform corner radius only, not per-corner `CornerRadii`** -- the
formula above takes a single scalar $r$; DESIGN.md's eventual per-corner
API is deferred until a `Canvas` surface that actually needs it exists.
