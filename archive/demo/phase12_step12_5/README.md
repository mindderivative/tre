# Demo: Phase 12 Step 12.5 -- `Canvas` Redesign (`tre-python`)

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase12_step12_5
../../.venv/bin/python demo.py
```

**What this proves.** `Canvas`, redesigned around what `tre-engine`
actually does well -- not a 1:1 port of `RenderingCanvas`'s ~20 methods.
`ShapeRegistry` stays the primary way to describe *what exists*;
`Canvas` becomes the real scene-assembly/compositing context:

- `renderer.flatten_into(canvas, registry)` -- the real seam letting more
  than one registry share one canvas.
- `with canvas.clip(x, y, width, height):` / `with canvas.layer(...):` --
  real scissor clipping and offscreen compositing, as Python context
  managers (pushed immediately when called, popped on block exit) rather
  than bare `push_clip`/`pop_clip` methods a caller could mismatch.
- `canvas.tag_accessibility_node(...)` -- real AT-SPI/UIA-consumable
  output.
- `renderer.render_canvas(canvas)` -- submits an assembled canvas;
  `renderer.render(registry)`'s own existing single-registry convenience
  wrapper is now implemented in terms of this same seam.

The demo composes a full-canvas red background registry with a second,
green-filled registry flattened *inside* a `canvas.clip(...)` block that
only exposes a small sub-region -- real pixel assertions confirm green
inside the clip rect and unclipped red immediately outside it on every
side checked. `render(registry)`'s convenience wrapper is re-verified
as a regression guard (a genuine, if brief, test-script mistake surfaced
here first -- `Circle`'s `x`/`y` are its bounding-box top-left, not its
center, the same real gotcha Phase 10 Step 10.4's own demo history
already documents; fixed in the demo script, not in the binding, which
was correct throughout).

**Full workspace verification performed:**

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets --
  -D warnings` / `cargo build --workspace --all-targets` / `cargo test
  --workspace` -- all clean in debug.
- In `--release`, the same 5 pre-existing, unrelated `tre-engine`/
  `tre-memory` test failures already disclosed in prior steps' READMEs --
  still flagged as separate follow-up work, not fixed here.
- `demo.py` run against real GPU hardware via `maturin develop --release`:
  exits 0, real clip boundaries verified pixel-exact.
- A second, ad hoc check confirmed the identical `flatten_into`/
  `render_canvas` seam also works through `WindowedRenderer` (20 real
  on-screen frames, no errors).

**This completes all five sections of the approved Phase 12 plan** —
input events + windowed rendering (12.1), `Text` as a first-class
`ShapePrimitive` (12.2), the `Font`/`Text` Python API (12.3), gradient/
texture fill (12.4), and this `Canvas` redesign (12.5).
