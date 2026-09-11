# Demo: Phase 12 Step 12.8 -- SVG Binding (`tre-python`)

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase12_step12_8
../../.venv/bin/python demo.py
```

**What this proves.** `tre.Svg.parse(data, fill_color, fill_rule=...)`
-- a real, previously-missing capability found completely working in
Rust (`crates/tre-svg`) during a GUI-framework gap assessment, but
totally unreachable from Python. The real pipeline: `usvg` parses and
resolves the DOM, `tre_svg::parse_svg` flattens curves into absolute-
coordinate polygons, `tre_svg::tessellate_fill` runs `lyon`'s real
sweep-line fill tessellator (self-intersection, compound shapes, both
winding rules, all handled directly), and the result renders through a
new, minimal `tre_engine::Svg` primitive that reuses the *existing*
`RenderingCanvas::draw_flat_polygon`/`FlatColor` pipeline -- no new
shader, no new GPU path.

Two real documents, both with exact-pixel assertions:

- A simple square -- inside reads exact black, outside reads exact
  background.
- A ring with a real punched-out hole (`fill-rule="evenodd"`) -- the
  ring body reads exact black, the hole's dead center reads exact
  background (a real hole, not a second overlapping fill).

**Why `tre-engine` doesn't just depend on `tre-svg` directly:** it
can't -- `tre-svg` already depends on `tre-engine` for `UiVertex`, so
the reverse would be a real dependency cycle (confirmed in
`tre-engine`'s own `Cargo.toml`, which gives the identical reason for
why `Path`/`Polygon` have their own separate `lyon` tessellation
instead of reusing `tre-svg`'s). `tre_engine::Svg` sidesteps this
entirely: it just stores plain, already-tessellated positions and
triangle indices (no `tre-svg` type embedded at all), and `tre-python`
-- a leaf crate with no such cycle -- does the real parsing and
tessellation before handing over the result.

**Real, disclosed scope, inherited from `tre-svg` itself, not
introduced here:** only `<path>` fill geometry is extracted -- strokes,
`<image>`/`<text>` nodes, and (confirmed by reading `tre-svg`'s own
parsing loop directly) **each path's own individual fill color** are
all discarded. A caller supplies **one** solid `fill_color` for the
whole parsed document. A real multi-color SVG icon will render as a
single flat color, not its original per-path colors -- a real
limitation of today's `tre-svg`, not a choice made in this binding.
Gradients aren't parsed either, for the same reason.

**Full workspace verification performed:**

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets --
  -D warnings` / `cargo build --workspace --all-targets` / `cargo test
  --workspace` -- all clean in debug (`tre-engine` 156 -> 157 tests, a
  new, real test proving `ShapePrimitive::Svg` renders through
  `flatten_into`'s dispatch).
- In `--release`, the same 5 pre-existing, unrelated `tre-engine`/
  `tre-memory` test failures already disclosed in prior steps' READMEs --
  still flagged as separate follow-up work, not fixed here.
- `demo.py` run against real GPU hardware via `maturin develop --release`:
  exits 0, both documents render exact-pixel-correct.

**Not yet done:** per-path multi-color extraction and gradient support
(both real `tre-svg` limitations, not this binding's); a `Canvas`-level
seam for animated/morphed SVG (`tre_svg::morph`/`morph_into` exist and
work in Rust, Rust-only, driven by a caller-supplied `t` each frame --
not yet exposed to Python).
