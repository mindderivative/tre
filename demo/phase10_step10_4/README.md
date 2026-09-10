# Demo: Phase 10 Step 10.4 -- Direct PyO3 Python Bindings (`tre-python`)

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase10_step10_4
../../.venv/bin/python demo.py
```

**What this proves.** A real, end-to-end round trip through the new
`tre-python` crate (IMPLEMENTATION.md Phase 10 Step 10.4): pure Python
constructs a `tre_python.ShapeRegistry` with three shape kinds
(`Rectangle`/`Circle`/`Polygon`), renders it via `tre_python.HeadlessRenderer`
against a real Vulkan device, and gets back real `bytes` read off the GPU --
not a mock, not a stub. `demo.py` asserts *exact* expected BGRA8 pixel
values at each shape's own center (pure-primary colors are chosen
deliberately: 0 and 255 are the two fixed points of the sRGB transfer
function, so their readback bytes are exact regardless of the pipeline's
internal linear/sRGB handling) and writes `phase10_step10_4_output.png`
for visual inspection.

**Scope of this first pass** (see REVIEW.md's "Phase 10 Step 10.4
Implementation" section and IMPLEMENTATION.md's own write-up for the full
account): solid fill only (no `Gradient`/`Texture` fill exposed yet),
headless rendering only (no windowed/event-loop integration yet), and
`Canvas.save()`/`restore()` only (no direct immediate-mode drawing methods
exposed yet). `tre_python.rgba8(r, g, b, a)` is provided as the Python
equivalent of `tre_engine::rgba8`'s exact little-endian packing, so callers
don't have to replicate that bit layout by hand.

**Two real bugs found and fixed while building this demo** (not merely
while compiling the binding -- see REVIEW.md findings #193-194 for the
full account):

- A real segfault at Python interpreter shutdown, caused by
  `PyHeadlessRenderer`'s struct fields being declared in the wrong order
  (Rust drops struct fields in *declaration* order, the opposite of local
  variables) -- `device` was destroyed before `swapchain`/`pipelines`
  still held live Vulkan handles against it.
- `Circle`'s `x`/`y` fields mean the bounding-box top-left (matching
  `Rectangle`'s own convention), not the center -- this demo's own first
  draft assumed "center" and got a wrong pixel color, caught by asserting
  an exact expected value.

**Full workspace verification performed during implementation:**

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets --
  -D warnings` / `cargo build --workspace --all-targets` / `cargo test
  --workspace` -- all clean, in both debug and `--release` profiles, zero
  failures across the whole workspace.
- `demo.py` run against real GPU hardware via `maturin develop --release`,
  exits 0 with every pixel assertion passing.

**Not yet done, deliberately deferred:** a CI job for the Python-binding
test suite (IMPLEMENTATION.md Step 10.4 task 4); a zero-copy
buffer-protocol return type (would require expanding the project's
`unsafe`-permitted closed set further than the one call this step already
needed to add `tre-python` to it for).

**Update (2026-09-10):** a `/review-project` pass found and fixed a
critical bug this demo's own single `render()` call had not caught: a
*second* `render()` call on the same, unmutated registry produced an
empty frame and crashed in release builds. `demo.py` now also asserts
that a second `render()` call reproduces the exact same frame as the
first -- see REVIEW.md findings #196-199 for the full account (also:
input validation on `HeadlessRenderer` dimensions and `Polygon` side
count, and a `&mut self` fix closing a real concurrent-call GPU-state
race).
