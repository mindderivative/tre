# Log: M3 Phase 2, Step 4 — `parley` Typography Spike (§14 step 4)

Corresponds to `PLAN.md` / `BUILD_TRACKER.md` M3 Phase 2, step 4 of 4 (closes Phase 2).

## What happened

**Traced the real shaping-to-drawing path before writing anything,** since
`vello_hybrid`'s own "text" feature depends on `glifo` (a low-level
glyph-atlas/rasterizer), not `parley` -- confirmed directly in
`vello_hybrid`'s own `Cargo.toml` and its internal glyph test
(`scene.rs`). The real pipeline is: `parley` shapes text into a `Layout`
of positioned glyph runs; for each run, `glifo::Glyph { id, x, y }` values
plus a `peniko::FontData` and font size get handed to
`Scene::glyph_run(&mut resources, &font).font_size(..).fill_glyphs(..)`.
Added `parley` (0.11.1) and `glifo` (0.3.0, already resolved at this
exact version transitively via `vello_hybrid` -- confirmed zero new
dependency-graph entries) to `engine-render`. `Cargo.lock`'s kurbo/peniko
counts stayed at 1 after adding `parley` -- the Linebender-family
version-coupling risk (§3) hasn't bitten here, unlike wgpu in step 1.

**Vendored three fonts** (`crates/engine-render/assets/fonts/`: Roboto
Regular/Medium, Noto Sans Arabic Regular) rather than relying on system
font discovery, so the typography spike is hermetic -- the same
headless-CI-safe discipline this codebase already applies to GPU/display
absence. `TextRenderer` registers them directly via `fontique::Collection`
with `system_fonts: false`.

**`engine-core::node`: added `NodeKind::Text(TextState)`** -- `content`,
`font_family`, `font_weight`, `font_size`, deliberately no `Animated`
fields (documented: no MD3 component in scope yet animates a text
property). `engine-render::text::TextRenderer` shapes via `parley`'s
`RangedBuilder`, breaks lines to the node's taffy box width, and draws
every `GlyphRun` through `glifo`. `build_tree_scene` now threads
`&mut Resources`/`&mut TextRenderer` through its recursive paint walk;
`FrameRenderer::resources_mut()` was added because glyph atlasing happens
during scene *construction* (inside `Scene::glyph_run`), before
`FrameRenderer::render` is ever called -- both need the same `Resources`
instance, which previously only `render()` could see.

**Two real, non-obvious findings, both fixed, not worked around:**

1. *Font weight is not selectable by family name alone.* First attempt
   used `font_family: "Roboto Medium"` for the Headline role, matching
   how `fc-list` names it -- and it drew nothing. Checked Roboto-Medium's
   actual name table directly (`fontTools`): its *typographic* family
   (OpenType name ID 16) is `"Roboto"`, same as the Regular face; only
   the legacy name (ID 1) says `"Roboto Medium"`. `fontique` (correctly)
   prefers the typographic name, so both files register under one family
   `"Roboto"` with two weight variants -- weight has to be selected via
   `StyleProperty::FontWeight(FontWeight::new(500.0))`, not a second
   family string. Added `font_weight: f32` to `TextState`.

2. *RTL positioning needs an explicit alignment pass.* The Arabic string
   initially rendered with all its ink flush against the *left* edge of
   its box, not the right -- `Layout::break_all_lines` alone doesn't
   apply paragraph-direction-aware positioning; that's a separate
   `Layout::align(Alignment::Start, ..)` call, where `Alignment::Start`
   is direction-aware (left for LTR, right for RTL). Added that call to
   `TextRenderer::draw`, unconditionally (correct for LTR text too, it
   was simply always missing).

**`tests/text_layout.rs`** (headless, render-to-texture-then-readback,
same discipline as `layout_tree.rs`/`animated_rect.rs`, calibrated for
text's anti-aliased edges: "ink exists somewhere in this node's box"
rather than an exact pixel match) proves: Body (Roboto Regular, 16px) and
Headline (Roboto Medium, 32px) each draw real ink in their own laid-out
box, and the Arabic string's ink sits near the right edge of its box
while the leftmost 40px stays untouched -- the actual geometric proof of
BiDi positioning, not just "a glyph rendered somewhere."

**Extended the windowed demo** (`tests/rect_window.rs`) with a text block
below the step 3 animated-rects row: the same Body/Headline/Arabic
content, live. Ran for real -- 60 frames, no panic, no missing-font
fallback failures.

## Verification

```
$ cargo test --workspace
    ...
     Running tests/text_layout.rs (engine_render)
test type_roles_and_rtl_string_render_real_ink ... ok

     Running tests/frame_budget.rs (engine_render)
test frame_pipeline_fits_the_16_6ms_budget ... ignored, perf benchmark -- ...

     Running tests/rect_window.rs (harness = false)
engine-render §14 step 4: first frame presented, 420x280, 4 laid-out rects animating independently, plus a Body/Headline/Arabic text block
engine-render §14 step 4: animation ran for 0.84s across 60 frames
engine-render §14 step 4: exited cleanly after 60 frames
    ... (all other crates/tests green)

$ cargo test -p engine-render --test frame_budget --release -- --ignored --nocapture
engine-render §14 step 3 frame budget: 300 nodes, median 0.396ms, max 0.429ms ...
test frame_pipeline_fits_the_16_6ms_budget ... ok

$ cargo clippy --workspace --all-targets   # clean (one too_many_arguments
                                            # warning on TextRenderer::draw,
                                            # fixed by bundling x/y/max_width/
                                            # color into a TextPlacement struct)
$ cargo fmt --check                        # clean
```

## Next

`BUILD_TRACKER.md` updated: Phase 2 (§14 steps 1-4) is now fully ✅, M3
at 29%. Next: Phase 3 (§14 step 5) -- `engine-spec` parses one static
`view.yaml`. Also, per explicit user request this same session: set up
real CI (this repo has a GitHub remote, `mindderivative/tre`, but no
`.github/workflows/` yet) and run the whole existing test history through
it.
