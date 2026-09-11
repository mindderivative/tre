# Demo: Phase 13 Step 13.4 -- Real Blur-Based Shadows

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase13_step13_4
../../.venv/bin/python demo.py
```

**What this proves.** The sixth section of the Phase 13 plan: real
drop shadows for renderable shapes, answering the project owner's own
question of whether "copy the shape, apply a gradient" is the best way
to do it -- **it is not**. `tre_engine::LayerDesc` already had a real
`blur: bool` field applying a genuine, already-shipped Dual-Kawase blur
(Step 7.2.2) to a layer's own content before compositing, and
`canvas.layer(x, y, width, height, blur=True)` has been real, tested,
Python-exposed API since Phase 12 Step 12.5. **No new rendering path was
needed for shadows at all** -- the only real gap was the tedious
geometry of sizing/positioning the shadow's own offscreen layer, which
this step closes with one new pure function:
`tre.shadow_layer_bounds(x, y, width, height, offset_x, offset_y,
blur_margin=24.0) -> (x, y, width, height)`, the union of the shape's
own bounding box and its offset copy, expanded by `blur_margin` pixels
on every side so the blur has room to spread without being clipped at
the layer's own edge.

**A real, disclosed coordinate-space finding made while building this
demo**: content drawn inside a `canvas.layer(...)` block uses
coordinates **local** to the layer's own top-left corner, not the outer
canvas's absolute coordinates -- confirmed directly against
`tre-engine`'s own RHI doc comment (`begin_render_to_texture`'s NDC
mapping is driven only by the layer's own `logical_width`/
`logical_height`, with no offset parameter at all) and then verified
empirically before trusting it in this demo's own shadow-positioning
math.

**The real proof a gradient-copy approach could never produce**: the
demo draws a shadow rectangle, blurs its own layer, and scans pixel
darkness across the shadow's own *original, pre-blur* hard edge. A
solid alpha-blended duplicate (the gradient-copy approach) would jump
straight from its interior color to exact background right at that
edge. A **real** blur instead shows a genuine, non-linear gradient
straddling it -- this demo asserts the pixel *at* that former edge is
strictly between the interior and background values, and that pixels
even 5px *past* the shape's own original silhouette still carry
measurable darkness (blur spreading beyond the hard edge, which a copy
+ gradient could only fake by hand-authoring a matching gradient shape
per shadow), while pixels far enough away are exactly background again
(blur doesn't spread indefinitely).

**Full workspace verification performed:**

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets
  -- -D warnings` / `cargo build --workspace --all-targets` / `cargo
  test --workspace` -- all clean in debug (`tre-engine` 158 -> 160
  tests: two new `shadow_layer_bounds` tests -- the general case and the
  zero-offset/zero-margin identity case).
- In `--release`, the same 5 pre-existing, unrelated `tre-engine`/
  `tre-memory` test failures already disclosed in every prior step's
  README -- still flagged as separate follow-up work, not fixed here.
- `demo.py` run via `maturin develop --release`: exits 0, every
  assertion (including the real blur-gradient proof) passes.

**Real, disclosed scope limits carried forward from `LayerDesc.blur`
itself**: blur softness is fixed (the existing Dual-Kawase chain has no
tunable radius yet, per `LayerDesc`'s own doc comment) -- every shadow
in this step has the same softness; a tunable radius is real, separate
future RHI work, not part of this step. A convenience that automates
"insert a shape with a shadow" as one call (rather than the caller
manually building the shadow-colored silhouette registry themselves, as
this demo does explicitly) is a real, straightforward follow-up once a
concrete calling pattern from `pySilver` emerges -- not built
speculatively here.
