# Demo: Phase 12 Step 12.9 -- `border_enabled` + Real Scale/Rotation Exposure

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase12_step12_9
../../.venv/bin/python demo.py
```

**What this proves.** Two real, disclosed follow-up fixes from the
GUI-framework Q&A pass:

1. **`border_enabled: bool`.** Before this fix, `border_thickness > 0.0`
   was the ONLY way `Rectangle`/`Circle`/`Polygon`/`Path` decided whether
   to draw a border -- there was no way to turn a border off without
   discarding its configured width. Every shape now carries a real
   `border_enabled` field (`tre_engine::Rectangle::border_enabled` and
   its three siblings), gating the *effective* thickness used at render
   time (`if shape.border_enabled { shape.border_thickness } else {
   0.0 }`) without ever touching the stored value. Proven by an
   exact-byte comparison: `border_thickness=8.0, border_enabled=False`
   renders byte-identical to no border configured at all, and toggling
   `border_enabled` off then back on restores the exact same pixels
   while `border_thickness` stays `8.0` throughout.

2. **`scale_x`/`scale_y`/`rotation`.** `tre_engine::Transform2D` already
   carried real `scale`/`rotation` fields (`Transform2D::to_affine2`
   composes them into a real `Affine2`, used by every native Rust demo),
   but `tre-python`'s own `common()` helper silently hardcoded both to
   identity for every Python-inserted shape -- a real, previously
   undisclosed gap this project's own GUI-framework gap assessment
   surfaced (`shapes.rs`'s own top-level doc comment used to say so
   directly). `Rectangle`/`Circle`/`Polygon`/`Path`/`Text`/`Svg` all now
   accept real `scale_x`/`scale_y`/`rotation` (radians) parameters,
   defaulting to `1.0`/`1.0`/`0.0`. Proven with exact-pixel assertions
   against the real transform math, not just "it doesn't crash":
   - `scale_x=2.0` stretches a rectangle's footprint out to exactly
     `x = 50 + 60*2 = 170`, confirmed by sampling a pixel only the
     scaled version reaches.
   - `rotation=pi/2` sweeps a wide, short rectangle into an entirely
     different bounding region (`Transform2D::to_affine2`'s own
     composition order -- scale, then rotate, then translate -- and
     `Affine2::from_rotation`'s own sign convention, both confirmed by
     reading `tre-math`/`tre-engine`'s own source and its own tests, not
     assumed), verified against hand-derived corner coordinates.

**A real, reusable lesson this demo's own history disclosed**:
`HeadlessRenderer` opens a real `PlatformConnection` -- a genuine winit
`EventLoop` under the hood (confirmed via `tre-platform`'s own
`ConnectionFailed` error text) -- and winit enforces a real, hard
OS-level rule: **at most one `EventLoop` per process, ever**. An earlier
draft of this demo created a fresh `HeadlessRenderer` per render call and
failed non-deterministically-looking (actually fully deterministic: the
first renderer in the process always succeeded, every subsequent one
always failed) with `RuntimeError: failed to connect to the display
server`. The fix: create exactly one `HeadlessRenderer` per process and
reuse it for every render, which every `tre-python` renderer's own
`render(registry)` method already supports.

**Full workspace verification performed:**

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets
  -- -D warnings` / `cargo build --workspace --all-targets` / `cargo
  test --workspace` -- all clean in debug.
- In `--release`, the same 5 pre-existing, unrelated `tre-engine`/
  `tre-memory` test failures already disclosed in prior steps' READMEs --
  still flagged as separate follow-up work, not fixed here.
- `demo.py` run against real GPU hardware via `maturin develop --release`:
  exits 0, every assertion passes.

**Not yet done, and out of the current scope:** `Path` still has no
`x`/`y` translation exposed at all (its own commands are already
authored in absolute coordinates) -- `scale_x`/`scale_y`/`rotation` were
still added to it for consistency, applied about the fixed local origin
`(0, 0)` its commands are drawn relative to, but a real translated
`Path` remains a separate, disclosed gap.
