# Plan: Phase 6, Step 6.1 -- Real Pipeline State Registry

## Scope decisions (confirmed with the project owner, 2026-09-08)

**Phase 6 ("Sorting, Batching, & RHI Execution") is split into several
independently-plannable steps, the same way Phases 3-5 were.** A
pre-planning investigation (real code, not just docs) found the phase's
own name is misleading about how much work remains: the 64-bit sort key
and real batch-merge logic are already built and proven (Step 5.1.3,
extended for multi-threaded stitching in 5.2 and accessibility nodes in
5.3) -- `FlattenedFrame::commands` is real, sorted, merged output today.
What's actually missing is everything *downstream* of that: nothing
generically consumes a `FlattenedFrame` and drives real RHI submission
through it. The planned shape of the rest of the phase, for context (each
gets its own plan when reached, per this project's standing process):

- **6.1 (this plan):** a real pipeline-id -> pipeline-object registry,
  replacing the one hardcoded special case that exists today.
- **6.2 (next):** the generic frame executor itself -- the real "take a
  `FlattenedFrame`, drive `RhiCommandBuffer` calls batch by batch" logic,
  built on 6.1's registry, replacing the two demos' currently-duplicated
  hardcoded dispatch loops.
- **6.3:** real `PushScissor`/`PopScissor` execution (`set_scissor` is a
  real, working RHI method today with zero callers).
- **6.4:** real `PushLayer`/`PopLayer` execution -- real transient-target
  acquisition and a straight alpha composite back (blur/filters
  explicitly deferred; `Canvas::push_layer`/`pop_layer` are called
  nowhere today except `tre-engine`'s own unit tests).
- **6.5:** a combining capstone -- one real scene exercising multiple
  pipelines and real clipping together in one submitted frame (layer
  compositing joins once 6.4 -- see below -- makes it real), matching
  Step 5.3.3's own precedent of a final capstone proving previously-
  separate pieces together.

**Correction, found while writing up this step's own docs, not during
planning:** `IMPLEMENTATION.md` already has a pre-existing, never-yet-
executed master outline for Phase 6 ("Step 6.1: The 64-Bit Radix
Batching Engine," "Step 6.2: Dynamic Index Stitching") and a *separate*
**Phase 7: Color Management & Compositing** (Step 7.1 linear sRGB/HDR,
Step 7.2 `PushLayer` visual filters/blur) that this plan's own earlier
research -- real code via an Explore agent, not this older outline text
-- never surfaced. Reconciled with the project owner (2026-09-08):
- The color/HDR work originally sketched as this plan's own 6.5/6.6
  moves back to Phase 7, matching the pre-existing outline, rather than
  staying folded into Phase 6 -- Phase 6 stays scoped to real IR
  execution (registry, executor, scissor, layers, combining capstone);
  color/HDR gets its own plan when Phase 6 closes.
  `PushLayer`'s own blur/filter wiring was already noted above as
  deferred past 6.4 for the same reason it's the pre-existing outline's
  own Step 7.2, not a new deferral.
- This step keeps the "Step 6.1" name despite colliding with the
  original outline's own already-different "Step 6.1" -- see
  `IMPLEMENTATION.md`'s Phase 6 section for the correction/supersession
  note explaining why (the original sketch's own Step 6.1 content is
  substantially complete, pulled forward into Step 5.1.3, before any of
  this plan's own numbering existed).

**Why the registry goes first, and why this step's scope is narrower
than "register every existing pipeline kind."** Investigation confirmed
`Canvas` itself has exactly two real drawing methods today --
`draw_rounded_rect` (Phase 3) and `draw_text` (Phase 5.1.2) -- backed by
exactly two pipeline kinds: an implicit, undocumented id `0` (SDF rounded
rect) and `PIPELINE_MSDF_TEXT = 1` (`tre-engine/src/lib.rs:129`). Three
other real pipeline kinds exist in `tre-rhi-vulkan`
(`create_pipeline`/`create_stencil_and_cover_pipelines`,
`tre-rhi-vulkan/src/lib.rs:817,998`) -- a plain bindless-textured quad, a
walking-skeleton-style flat-vertex-color quad, and the stencil/cover
pair -- but every demo that uses them (`bindless_textures_demo`,
`svg_tessellation_demo`, `svg_morph_demo`, `stencil_and_cover_demo`)
bypasses `Canvas`/`flatten()` entirely, building vertex/index data and
issuing raw RHI calls directly. Registering ids for pipeline kinds
nothing can reach through `Canvas` would be exactly the kind of
speculative, hypothetical-future-requirement work this project's own
standing discipline avoids -- reaching them requires new `Canvas` API
surface (`draw_image`/`draw_path`/`draw_svg`) that doesn't exist and
isn't this phase's job to build. Stencil-and-cover is additionally a
structurally different case even setting that aside: it's two draw calls
(stencil pass, then cover pass) per logical shape, which doesn't fit the
one-command-one-pipeline model the registry/executor are built around --
wiring it through the real IR is real, separate future work once a
`Canvas` fill-path primitive exists to need it, not a Step 6.1 concern.

**This step also fixes a real, previously-undiscovered `NO_TEXTURE`
sentinel mismatch, found while investigating the two duplicated demo
loops this step exists to replace.** `draw_rounded_rect` emits
`UiDrawCommand::texture_handle: 0` for "no texture bound" (every
non-MSDF command-construction site in `tre-engine/src/lib.rs` sets this
literal `0`), while `canvas_batch_flattening_demo.rs`/
`canvas_sub_canvas_demo.rs` each independently define their own
`const NO_TEXTURE: u32 = u32::MAX` for the RHI-binding side and use it
in a hardcoded `if pipeline_state_id == PIPELINE_MSDF_TEXT {...} else
{ bind_texture(0, NO_TEXTURE) }` branch -- two different sentinels for
the same concept, reconciled today only by that branch's own existence.
`0` is not safe to treat as "no texture" going forward: it collides with
a legitimate real bindless index `0` a future textured pipeline could
validly use. Unifying on one true sentinel (matching the demos' own
`u32::MAX` convention) is what lets Step 6.2's executor bind
`command.texture_handle` unconditionally for every `DrawGeometry`
command, with no per-pipeline-kind special case -- this step's registry
work is not meaningfully separable from fixing this, since the registry
exists specifically to replace that same hardcoded branch.

## Goal

A real `PipelineRegistry` (new, in `tre-engine`, generic over the
existing `RhiPipelineState` trait -- no new RHI trait surface) maps a
pipeline id to its real pipeline object; a real, type-safe id for each
of the two `Canvas`-emittable pipeline kinds today (`SdfRoundedRect`,
`MsdfText`) replaces `PIPELINE_MSDF_TEXT`'s status as the lone named
constant; and a single, unified `NO_TEXTURE` sentinel (`u32::MAX`,
promoted from the two demos' own duplicated local constants into a real
`tre-engine` constant) is what every non-texture-sampling `Canvas`
drawing method emits, replacing the current `0`. Proven by a unit test
building a registry, registering both real kinds, and resolving each id
back to the pipeline object registered under it -- this step has no new
visual/demo capstone of its own (nothing yet queries the registry at
render time; that's Step 6.2's job), matching this project's own
established precedent (Steps 5.2.1/5.3.1/5.3.2) of deferring visual
proof to the step that gives foundational work a real consumer.

## Tasks

1. **`PipelineKind` enum** (`tre-engine`), `#[repr(u16)]`, covering
   exactly the two real, `Canvas`-emittable kinds:
   ```rust
   #[repr(u16)]
   pub enum PipelineKind {
       SdfRoundedRect = 0,
       MsdfText = 1,
   }
   ```
   `PIPELINE_MSDF_TEXT: u16 = 1` stays defined (existing call sites --
   `tre-engine/src/lib.rs:1214,1218,2502,2885` and the two demos --
   keep working unchanged), documented as an alias
   (`PipelineKind::MsdfText as u16`) rather than removed outright, since
   nothing about this step requires touching every existing call site
   just to rename it.

2. **`pub const NO_TEXTURE: u32 = u32::MAX;`** (`tre-engine`, next to
   `UiDrawCommand`/`PIPELINE_MSDF_TEXT`). Every `texture_handle: 0`
   command-construction site that does not sample a texture (all of
   them except `draw_text`'s, per the grep above) changes to
   `texture_handle: NO_TEXTURE`. `draw_text`'s real
   `texture_handle: atlas_context.texture_handle` is unaffected.

3. **`PipelineRegistry` struct** (`tre-engine`):
   ```rust
   pub struct PipelineRegistry {
       pipelines: std::collections::HashMap<u16, Box<dyn RhiPipelineState>>,
   }
   impl PipelineRegistry {
       pub fn new() -> Self { ... }
       pub fn register(&mut self, id: u16, pipeline: Box<dyn RhiPipelineState>);
       pub fn get(&self, id: u16) -> Option<&dyn RhiPipelineState>;
   }
   ```
   Takes ownership of registered pipelines (`Box<dyn RhiPipelineState>`,
   the trait `tre-engine` already defines, `ARCHITECTURE.md` Section 6) --
   a backend/demo builds its real Vulkan pipeline objects exactly as it
   does today (`VulkanDevice::create_pipeline`, unchanged), then hands
   ownership to the registry once at startup, rather than the registry
   knowing anything Vulkan-specific. `HashMap<u16, _>` over a fixed-size
   array: only 2 real entries exist today and the id space (`u16`, per
   the 64-bit sort key's own Pipeline ID field) is far too sparse for an
   array to make sense, and nothing about frame-time `get()` is a
   measured hot path yet (that's Step 6.2's concern, if profiling ever
   shows it matters -- not a reason to over-build this step).
   ### Panics
   `register` panics on a duplicate id -- two pipelines silently
   registered under the same id is a programmer error, matching this
   project's established `pop_layer`/`restore`-style precedent for
   unbalanced/invalid state, not a `Result` a caller would ever
   meaningfully recover from.

4. **Unit tests** (`tre-engine`, extending its own established
   hand-computed-expected-value style):
   - Registering two distinct ids and resolving each via `get()` returns
     the exact object registered under it (a test `RhiPipelineState`
     double with a distinguishable `raw_handle()`/`layout_handle()`, not
     a real Vulkan pipeline -- this crate has no Vulkan dependency and
     shouldn't gain one just for this test).
   - `get()` on an unregistered id returns `None`, not a panic --
     distinct from `register`'s own panic-on-duplicate above, since a
     command referencing an unknown pipeline id is a real runtime
     condition an executor should be able to detect and report, not
     necessarily a programmer error at registration time.
   - `register` panics on a duplicate id.
   - Every non-`draw_text` `UiDrawCommand`-emitting method
     (`draw_rounded_rect` at minimum) now emits `texture_handle:
     NO_TEXTURE`, not `0` -- extends whichever existing test already
     checks that method's emitted command fields.

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace.
- The two existing demos that read `command.texture_handle` in their own
  hardcoded branch (`canvas_batch_flattening_demo.rs`,
  `canvas_sub_canvas_demo.rs`) are **not** changed in this step (that's
  Step 6.2's job, once the registry has a real consumer) -- re-run
  manually to confirm `NO_TEXTURE`'s value change (their own local
  `u32::MAX` constant is unaffected; `tre-engine`'s emitted `0` becoming
  `NO_TEXTURE` doesn't change what they compare against, since they
  never read the SDF-rect branch's `command.texture_handle` at all
  today) produces zero behavior change -- confirming this step is a real
  no-op for existing render output before Step 6.2 starts actually
  depending on the new unified sentinel.
- All pre-existing examples re-run manually end to end, zero
  regressions (this step touches no RHI/shader code, only `tre-engine`'s
  own IR-construction and a new, currently-unused-at-render-time
  registry type).

## Explicitly out of scope for this sub-step

- The generic frame executor that actually queries the registry at
  render time -- Step 6.2. This step only builds the registry and proves
  it in isolation via unit tests.
- Registering the plain-textured-quad, flat-vertex-color, stencil, or
  cover pipeline kinds -- none are reachable through `Canvas` today (see
  "Scope decisions" above); registering them now would have no real
  consumer and no way to be genuinely proven.
- Any change to `tre-rhi-vulkan`'s pipeline/shader code, or to how
  pipelines are actually created -- this step is `tre-engine`-side IR/
  registry-type work only.
- `PushScissor`/`PushLayer` execution -- Steps 6.3/6.4. Color/HDR work
  moved to its own Phase 7, per this plan's own "Scope decisions"
  correction above.
