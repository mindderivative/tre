# Demo: Phase 12 Step 12.6 -- Real Parallel Recording (`tre-python`)

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase12_step12_6
../../.venv/bin/python demo.py
```

**What this proves.** `renderer.render_parallel(registries)` -- Python
supplies the work (a list of `ShapeRegistry`s); every threading decision
(how many real OS threads, when they run, how results merge) is made in
Rust. Each registry is flattened into its own real `SubCanvas` on its
own real OS thread, GIL released for the whole span, then every
`SubCanvas` is stitched into one shared `FrameArena` and submitted as a
single frame.

This is genuine parallelism, not just "produces correct pixels" (which
serial execution would too): the demo builds 8 registries, each holding
one deliberately expensive `Path` (a 400-segment wavy closed loop, real
non-trivial `lyon` fill tessellation work), times calling `render()`
once per registry sequentially, then times one `render_parallel()` call
across all 8 -- and asserts a real speedup (`> 1.3x`; a representative
run on this machine measured **3.52x** with 8 registries).

**The real concurrency-safety story** (verified by reading the actual
code before building this, not assumed): `SubCanvas::stitch_into`'s own
`ScatterArena` reservations are lock-free; the real per-shape GPU
style-buffer writes (`RhiDevice::shape_style_buffer`) are mutex-protected
in the real Vulkan backend (`VulkanRingBuffer::write`); the shared text
atlas's own request/lookup path is lock-free by design (Phase 4). None
of `ShapeRegistry::flatten_into`'s own real work needed any *new*
synchronization to run concurrently -- the engine was already built for
this (`RenderingCanvas::create_sub_canvas`'s own doc comment always said
"intended to be moved into a real worker thread"), it just wasn't
reachable from Python before this step.

**A real, worth-noting implementation detail:** `PyRefMut` (PyO3's own
runtime borrow guard) is itself `!Send` -- it carries a GIL token
internally, so it cannot cross into the GIL-released closure at all,
even just captured and unused. Each registry's guard stays on the main
thread for this whole call (keeping its own borrow-flag set, so any
*other* concurrent Python-side access to the same registry is correctly
rejected); only a plain `&mut ShapeRegistry` reborrowed out of each
guard -- ordinary Rust data with no GIL ties -- actually moves into a
worker thread. No `unsafe` was needed anywhere in this feature.

**Full workspace verification performed:**

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets --
  -D warnings` / `cargo build --workspace --all-targets` / `cargo test
  --workspace` -- all clean in debug.
- In `--release`, the same 5 pre-existing, unrelated `tre-engine`/
  `tre-memory` test failures already disclosed in prior steps' READMEs --
  still flagged as separate follow-up work, not fixed here.
- `demo.py` run against real GPU hardware via `maturin develop --release`:
  exits 0, real measured speedup printed.
- A second, ad hoc check confirmed `render_parallel` also works through
  `WindowedRenderer` (15 real on-screen frames across 4 concurrently
  flattened registries, no errors).

**Real, disclosed scope boundary:** `render_parallel`'s own combined
`FrameArena` has a fixed element capacity (comfortably large, but not
yet configurable); a caller whose combined parallel scene is genuinely
larger gets a clean `TreError`, not silent truncation or a crash.
