# Demo: Phase 12 Step 12.3 -- `Font`/`Text` Python API (`tre-python`)

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase12_step12_3
../../.venv/bin/python demo.py
```

**What this proves.** `tre.Font`/`tre.Text` (new): a real system-cascade
font, loaded via real fontconfig discovery (`tre.Font.system_cascade()`),
inserted as a real retained-mode `Text` shape (`registry.insert_text(...)`)
alongside `Rectangle`/`Circle`/`Polygon`/`Path`, and rendered headlessly.
Mirrors `canvas_draw_text_demo.rs`'s own real two-phase contract: the
first several `render()` calls are real cache misses (nothing visible
yet), and `HeadlessRenderer`'s own `TextAtlas` only refreshes its live
GPU texture from the atlas's current pixel buffer every 15 frames -- so
this demo renders enough real frames, with real sleeps between them, for
the background atlas thread to resolve every glyph and the periodic
refresh to catch up, then confirms real, non-background pixels in the
text's own bounding region.

**Real, disclosed limitation carried over from Step 12.2/12.3's own
engine work:** there is still no way to update an existing GPU texture's
pixels in place, so every atlas refresh consumes a new slot from the
bindless array's fixed 4096-slot capacity. The 15-frame refresh interval
bounds how *often* this can happen for an app whose text keeps
introducing genuinely new glyphs forever -- not a fix, a real, disclosed
mitigation. Most real UI text settles on a bounded glyph vocabulary
quickly, so this is an acceptable first-pass tradeoff, not a silent
landmine for the common case.

**Full workspace verification performed:**

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets --
  -D warnings` / `cargo build --workspace --all-targets` / `cargo test
  --workspace` -- all clean in debug.
- In `--release`, the same 5 pre-existing, unrelated `tre-engine`/
  `tre-memory` test failures already disclosed in Step 12.2's own README
  (a `debug_assert!`-only guard that's a no-op in release) -- flagged as
  separate follow-up work, not fixed here.
- `demo.py` run against real GPU hardware via `maturin develop --release`:
  exits 0, prints when real text pixels first appear (15 real `render()`
  calls in the run that produced this README).
- A second, ad hoc check confirmed the identical `Text` shape also
  renders correctly through `WindowedRenderer` (30 real on-screen
  frames, no errors) -- not part of this automated demo script, but real
  verification nonetheless.

**Not yet done, deliberately deferred to Phase 12's remaining sections:**
gradient/texture fill (for both shapes and text), and the `Canvas`
redesign around clip/layer/accessibility semantics.
