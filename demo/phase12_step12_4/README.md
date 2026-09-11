# Demo: Phase 12 Step 12.4 -- Gradient/Texture Fill (`tre-python`)

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase12_step12_4
../../.venv/bin/python demo.py
```

**What this proves.** Every shape's `fill_color` now accepts a real
`int | GradientId | Texture` union, resolved into the matching
`tre_engine::FillStyle` variant at `insert_*` time
(`PyShapeRegistry::resolve_fill`). One scene exercises all three in a
single render:

- A plain `int` solid fill (`tre.Rectangle(..., GREEN)`) -- a regression
  check that existing code using a bare RGBA8 int keeps working
  unchanged.
- A linear gradient (`tre.Gradient.linear(...)` ->
  `registry.create_gradient(...)` -> a real `GradientId`, passed
  straight in as `fill_color`) -- asserted to actually vary across the
  rectangle (not just "differs from background").
- A texture (`renderer.create_texture(4, 4, tre.TextureFormat.Rgba8Unorm,
  pixels)` -> a real `Texture`, passed straight in as `fill_color`) -- a
  solid-magenta 4x4 texture, asserted to read back as *exact* magenta at
  the circle's center.

**Full workspace verification performed:**

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets --
  -D warnings` / `cargo build --workspace --all-targets` / `cargo test
  --workspace` -- all clean in debug.
- In `--release`, the same 5 pre-existing, unrelated `tre-engine`/
  `tre-memory` test failures already disclosed in Step 12.2/12.3's own
  READMEs -- still flagged as separate follow-up work, not fixed here.
- `demo.py` run against real GPU hardware via `maturin develop --release`:
  exits 0, all three fill kinds verified with real pixel assertions.

**Not yet done, deliberately deferred to Phase 12's remaining section:**
the `Canvas` redesign around clip/layer/accessibility semantics (Section
5) -- the last section of the approved plan.
