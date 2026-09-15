# Demo: Phase 13 Step 13.7 -- SMIL SVG Animation Parsing (`tre.parse_smil`)

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase13_step13_7
../../.venv/bin/python demo.py
```

**What this proves.** Q4's "SMIL/animation parsing for animated SVGs."
`usvg` -- the parser `tre-svg` already uses for everything else -- does
not process SMIL animation elements at all: it's a static-resolution
parser by design (the same real library `resvg` itself uses for static
rendering), confirmed by reading its own source before writing this.
Real SMIL support therefore needed a separate, direct XML pass: new
`crates/tre-svg/src/smil.rs`, via `roxmltree` (already a transitive
dependency of `usvg` itself, now pinned directly), extracting real
`<animate>`/`<animateTransform type="translate">` directives -- exposed
to Python as `tre.parse_smil(data) -> ParsedSmil`.

**Real, disclosed v1 scope**, deliberately not full SMIL spec
compliance (a large surface: motion-path animation, complex `begin`/
sync timing, `<animateColor>`, additive/accumulative animation): a
single scalar `<animate>` attribute (e.g. `opacity`) and
`<animateTransform type="translate">` (a 2D point), each with either
`values="a;b;c"` (a real keyframe list) or `from`/`to` (a real
2-keyframe list), plus `dur` (`"Ns"`, `"Nms"`, or a bare number treated
as seconds). `type="scale"`/`"rotate"`, `begin`, `repeatCount`, and
`calcMode` are real, disclosed gaps.

**No new engine machinery needed to drive it.** `parse_smil` only
extracts the real data an SVG author already wrote -- this demo composes
the extracted keyframes directly with the already-shipped `tre.Tween`/
`tre.Easing` (Step 13.2) to animate a real `tre.Svg` shape's position,
proving the full real pipeline end to end: SMIL extraction -> Tween
sampling -> real shape positioning -> real render, not just raw
extraction in isolation.

Every assertion checks an exact or provably-real property:

- A real `<animate attributeName="opacity" from="0" to="1" dur="2s"/>`
  extracts to exactly `keyframes=[0.0, 1.0]`, `duration_seconds=2.0`.
- A real `<animateTransform type="translate" from="10 10" to="150 150"
  dur="1s"/>` extracts to exactly `keyframes=[(10.0, 10.0), (150.0,
  150.0)]`, `duration_seconds=1.0`.
- A real static (non-animated) SVG document returns empty results, not
  an error.
- Malformed XML raises a real `ValueError` naming the actual parse
  failure.
- Driving the extracted translate keyframes through a real `Tween` and
  re-rendering a real `Svg` shape at 5 sampled points produces a
  monotonically moving, always-correctly-positioned real render (the
  shape's own fill is found exactly where its current SMIL-driven
  position says it should be, at every sampled frame).

**Full workspace verification performed:**

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets
  -- -D warnings` / `cargo build --workspace --all-targets` / `cargo
  test --workspace` -- all clean in debug (`tre-svg` 20 -> 28 tests:
  real `<animate>`/`<animateTransform>` extraction with both `from`/`to`
  and `values`, `dur` unit parsing (`s`/`ms`/bare), a real
  type="rotate" being correctly ignored, missing-attribute elements
  being skipped rather than erroring, and malformed-XML rejection).
- In `--release`, the same 5 pre-existing, unrelated `tre-engine`/
  `tre-memory` test failures already disclosed in every prior step's
  README -- still flagged as separate follow-up work, not fixed here.
- `demo.py` run via `maturin develop --release`: exits 0, every
  assertion (including the full SMIL-to-render integration check)
  passes.

**Real, disclosed scope limits carried forward**: only equal-topology
morphing concerns don't apply here (this is attribute/transform
animation, not vertex morphing -- see Step 13.6 for that), but the same
"common case, not full spec" discipline applies: `type="scale"`/
`"rotate"` on `<animateTransform>`, `<animateMotion>`, `begin`/
`repeatCount`/`calcMode`, and `<animateColor>` are all real gaps, not
attempted in this pass.
