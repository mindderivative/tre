# Documentation Review — September 2026

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: `DESIGN.md`, `TECHNICAL.md`, `ARCHITECTURE.md`, `IMPLEMENTATION.md`, reviewed in that order. All findings below have been implemented directly in those four files; this document is the record of what was found and what changed.

Status: **All findings implemented.** See "Follow-up: Rust/Python Language Migration," "Review of Rust-Specific Additions," "Full Documentation Review," "Engineering Decisions: Suggested Improvements Actioned," "Phase 0 Implementation" (2026-09-04), "Phase 1 Step 1 Implementation," "Pre-Phase-1-Step-2 Doc Check," "Phase 1 Step 2 Implementation," "Phase 1 Review," "Phase 2 Step 1 Implementation," "Phase 2 Step 2 Implementation," "Phase 2 Step 2.1 Implementation," "Phase 2 Code Review" (2026-09-05), "Phase 2 Step 2.3 Implementation," and "Phase 2 Step 2.3 Code Review" (2026-09-06) below for subsequent, out-of-band work not part of this original review. "Phase 1 Review"'s finding #51 has since been fixed; #52-53 remain deliberately deferred to Phase 2 (not yet revisited) — see that section for disposition. "Phase 2 Code Review"'s findings #66-70/#72/#73/#75 have since been fixed and re-verified (fmt/clippy/build/test clean, all six examples re-run with zero validation errors, #66 additionally proven via a deliberate-bug run); #71 fixed (VulkanSwapchain's matching #56 remains separately open); #74 documented as deliberate rather than changed; #76 left unfixed (no safe way to determine correct CI package pins from this environment) — see that section for full disposition. All of Phase 2 (Steps 2.1, 2.2, 2.4, and now 2.3) is complete as of 2026-09-06. "Phase 2 Step 2.3 Code Review"'s findings #78-82 have since been fixed and re-verified (fmt/clippy/build/test clean, all seven examples re-run with zero validation errors, `gc_demo` re-run three more times confirming consistent behavior under the new admission cap); #83 is not a defect — see that section for full disposition. "Phase 3 Step 3.1 Implementation", "Phase 3 Step 3.2 Implementation", "Phase 3 Step 3.3.1 Implementation", and "Phase 3 Step 3.3.2 Implementation" (2026-09-06) are also complete, below. Step 3.2's finding #84 has been fixed and re-verified (all 7 pre-existing examples re-run with zero validation errors, plus the new `sdf_rounded_rect_demo`); #85 is not a defect — see that section for full disposition. Step 3.3.1's findings #86-87 have both been fixed and re-verified (fmt/clippy/build/test clean, all 7 pre-existing examples re-run with zero validation errors, plus the new `svg_tessellation_demo`) — see that section for full disposition. Step 3.3.2's finding #88 (a unit-test-only issue) has been fixed and re-verified (all 8 pre-existing examples re-run with zero validation errors, plus the new `svg_morph_demo`) — see that section for full disposition. "Phase 3 Step 3.3.3 Implementation" (2026-09-06) is also complete, below, and with it all of IMPLEMENTATION.md Step 3.3 (3.3.1-3.3.3). Findings #89-90 have both been fixed and re-verified (fmt/clippy/build/test clean, all 10 pre-existing examples re-run with zero validation errors, plus the new `stencil_and_cover_demo`) — see that section for full disposition. "Phase 4 Step 4.1 Implementation" (2026-09-06) is also complete, below. Finding #91 (a unit-test-only issue) has been fixed and re-verified (all 11 pre-existing examples re-run with zero validation errors, plus the new `text_shaping_demo`) — see that section for full disposition. "Phase 4 Step 4.2.1 Implementation" (2026-09-06) is also complete, below (the first of Step 4.2's four sub-steps). Finding #92 (a real, engine-wide sRGB gamma gap, already independently scheduled as Step 7.1) is documented and deliberately not fixed here; #93 (a self-authored demo bug) has been fixed and re-verified (all 12 pre-existing examples re-run with zero validation errors, plus the new `atlas_packing_demo`) — see that section for full disposition. "Phase 4 Step 4.2.2 Implementation" (2026-09-06) is also complete, below (the second of Step 4.2's four sub-steps). Findings #94-95 (both minor, non-architectural) have been fixed and re-verified (all 13 pre-existing examples re-run with zero validation errors, plus the new `msdf_generation_demo`) — see that section for full disposition. "Phase 4 Step 4.2.3 Implementation" (2026-09-06) is also complete, below (the third of Step 4.2's four sub-steps, and the one that resolves the jagged-'X' observation from Step 4.1) — no findings this step; see that section for full disposition. "Phase 4 Step 4.2.4 Implementation" (2026-09-06) is also complete, below (the fourth and closing sub-step of Step 4.2, and with it all of Phase 4). Findings #96-97 (both minor, non-architectural, both in demo code) have been fixed and re-verified (all 15 pre-existing examples re-run with zero validation errors, plus the new `atlas_concurrency_demo`) — see that section for full disposition. "Phase 1-4 Comprehensive Review" (2026-09-06) is also complete, below — a 6-dimension, twice-checked (independent Find + adversarial Verify) audit of everything built across Phases 1-4, requested once Phase 4 closed. All 19 findings (#98-116) were independently confirmed by their verifier; 12 were fixed directly (#98-100, #104-108, #110-112, #115) and re-verified (fmt/clippy/test clean across the workspace, all 15 pre-existing GPU examples plus `msdf_generation_demo` re-run for real with zero validation errors); 7 whose real fix is substantial new feature work rather than a bug fix (#101-103, #109, #113-114, #116) were deliberately left as clearly disclosed, documented gaps instead of being built opportunistically inside this pass — see that section for full disposition of each. "Phase 4 Step 4.3.1 Implementation" (2026-09-06) is also complete, below — the first of Step 4.3's three sub-steps, closing finding #114 above. Finding #117 (a real bug in the new tombstone-reuse logic, caught by a dedicated unit test before this code had any real caller) has been fixed and re-verified — see that section for full disposition. "Phase 4 Step 4.3.2 Implementation" (2026-09-06) is also complete, below — the second of Step 4.3's three sub-steps, no findings this time. "Phase 4 Step 4.3.3 Implementation" (2026-09-06) is also complete, below — the third and closing sub-step of Step 4.3, wiring the previous two sub-steps' primitives into a real eviction policy in `AtlasOwner` and finally closing finding #114. No numbered findings this sub-step; a cold-start recency hazard was identified and designed around during planning itself rather than discovered afterward — see that section for full detail. "Phase 5 Step 5.1.1 Implementation" (2026-09-07) is also complete, below — the first of Step 5.1's three sub-steps, opening Phase 5. Findings #118 (a real bug: alpha-only vertex scaling made `set_alpha()` invisible) and #119 (a demo test-design bug) have both been fixed and re-verified — see that section for full disposition.

**Extending the index above (REVIEW.md finding #145: the paragraph above had gone stale, silently omitting the most consequential unresolved content in this document).** "Phase 5 Step 5.1.2 Implementation," "Phase 5 Step 5.1.3 Implementation," "Phase 5 Step 5.2.1 Implementation," "Phase 5 Step 5.2.2 Implementation," and "Phase 5 Step 5.2.3 Implementation" (2026-09-07) are also complete, below — no numbered findings in 5.1.2/5.2.1/5.2.2 (each states so directly); 5.1.3 fixed and re-verified real bugs in its own demo assertions; 5.2.3's own capstone found and fixed two real bugs during development, both documented in that section. "Phase 5 Step 5.3.1 Implementation" and "Phase 5 Step 5.3.2 Implementation" (2026-09-07/08) are also complete, below — see those sections for disposition. **"Phase 5 Step 5.3.3 Implementation" and "Phase 5 Step 5.3.3 CI Verification" (2026-09-08) are NOT complete — Step 5.3 stays open.** The capstone demo's own code and logic are real, verified live on a real desktop session, and found/fixed two real bugs along the way (a role-mapping regression, a `thread::scope` panic-hang) — but CI's own `accessibility-validation` job does not pass, and a nineteen-real-push investigation (recorded in full below, ending at "STOPPING POINT (nineteenth push...)") found the real, precise root cause (a D-Bus proxy built against the session bus instead of the a11y bus) and paused there, unfixed, at the user's own explicit instruction. Two earlier, now-superseded diagnoses in this same investigation were each found and explicitly disproven before the real cause was located — read the CI Verification section in full for that history, not just its ending. "Phase 6 Step 6.1 Implementation," "Phase 6 Step 6.4.1 Implementation," and "Phase 6 Step 6.4.2 Implementation" (2026-09-08) are also complete, below, with findings #127-129 fixed and re-verified in each. **Phase 6 Step 6.5 (the Phase 6 closing capstone) has no REVIEW.md section of its own at all** — a real gap in this document's own indexing discipline, found only during the Phase 1-8 Comprehensive Review below (finding #147); IMPLEMENTATION.md's own Step 6.5 write-up states "No bugs found — passed on its first real run," so nothing was ever lost, only never indexed here. **"Phase 7 Step 7.2.1 Investigation" (2026-09-08) is STOPPED, unfixed, and Step 7.2 stays open.** Finding #130 records a real Dual-Kawase blur RHI bug (sampling a bindless texture while the render target is offscreen reads back all-zero) with twelve independent hypotheses tested and ruled out; one real, independently-correct fix (`begin_render_to_texture_no_end`, closing a genuine double-`cmd_end_rendering` bug found along the way) is kept, but the sampling failure itself remains unresolved. **Phase 8 (Steps 8.1.1 and 8.1.2) is also complete, but — like Phase 6 Step 6.5 — has no REVIEW.md section of its own** (finding #147); both real gaps IMPLEMENTATION.md's own Step 8.1.2 write-up describes (`execute_frame`'s hardcoded zero buffer offset; the atlas owner's no-live-peek limitation) are recorded properly there, just never indexed here until now. **"Phase 1-8 Comprehensive Review" (2026-09-08), below, is the newest section** — a second full six-dimension, twice-checked (independent Find + adversarial Verify) audit, this time across everything built through Phase 8, requested once the project owner wanted a second pass while an external review of the Step 7.2.1 blocker ran in parallel. All 21 findings (#131-151) were independently confirmed (several strengthened, not merely rubber-stamped, by their verifier); the fully-triaged disposition (which were fixed directly vs. left as disclosed gaps, and why) is recorded in that section's own summary table. **"Phase 9 Step 9.1 Implementation" (2026-09-09) is also complete, below** — real pre-work investigation found two of the step's own task-list premises no longer matched the real codebase (findings #154-155: a documented-but-never-built radix sort, and a documented-but-never-built atlas-exhaustion placeholder-glyph fallback), both resolved by building the real missing primitive (radix sort) or correcting the documentation to match reality (atlas fallback), each confirmed with the project owner via `AskUserQuestion` before any code was written. "Phase 9 Step 9.2 Implementation" (2026-09-09) is also complete, below — a real zero-allocation guard (`tre_memory::DebugAllocGuard`/`RenderTickGuard`) found and fixed a real per-frame allocation bug (finding #157) and disclosed two more real gaps left unfixed (#156, #158), plus a criterion perf-budget gate deliberately left red on a real, honestly-measured gap (#159). "Phase 10 Step 10.1 Implementation" (2026-09-09) is also complete, below — real Rectangle-only shape-primitive rendering, with one real discrepancy found and corrected during implementation (finding #161: `Path` fill needs new geometry work, not a wiring task). **"Phase 10 Step 10.2 Implementation" (2026-09-09), below, is the newest section** — full shape rendering support (non-uniform corners/borders/smoothing on `Rectangle`, real `Circle`/`Ellipse`, `Polygon` fill, real Bezier-flattening for `Path`, and real hit-testing for all four shape kinds). Finding #162 is a real GPU bug (a subnormal-float style-buffer index silently flushed to zero by real hardware) found by observing wrong pixels and fixed; #163 and #164 are disclosed-not-fixed gaps (`Path` fill blocked by a real circular-crate-dependency constraint; a pre-existing, newly-exposed non-premultiplied-alpha gap in the Phase-0-era `walking_skeleton.frag` shader).

**Further extending the index (2026-09-09).** "Phase 10 Step 10.2 Follow-up: `lyon` Migration," "Phase 10 Step 10.2 Completion Roadmap," "Phase 10 Step 10.2.1 Implementation," and "Phase 10 Step 10.2.2 Implementation" are also complete, below — see each section's own summary table for disposition (findings #165, #166 [decisions], #167 [fixed same-day], #168-169 [decisions]). "Phase 10 Step 10.2.3 Implementation" (2026-09-09) is also complete, below — real non-`Normal` `BlendMode` rendering for `Polygon`/`Path` solid fill. Finding #170 is a plan-invalidating discovery, not a bug: `VK_EXT_blend_operation_advanced` (this project's own written primary path, and finding #166's own prior conclusion) is not implemented by RADV, this project's own real dev GPU/driver — confirmed via `vulkaninfo` and independently corroborated via Mesa's own release notes — so the step pivoted to the real, portable `VK_KHR_dynamic_rendering_local_read` alternative, at the user's own explicit direction, rather than the originally-planned hardware `VkBlendOp` path. Findings #171-172 are two real regressions this step's own first full GPU demo-regression sweep caught (a windowed swapchain's surface not being guaranteed to support `INPUT_ATTACHMENT` usage; `resume_swapchain_rendering`'s hardcoded stale layout breaking every `PushLayer`/`PopLayer` demo) — both fixed same-day and re-verified. "Phase 10 Step 10.2.4 Implementation" (2026-09-09) is also complete, below — the ellipse SDF's disclosed "scaled circle" approximation replaced with Inigo Quilez's own published, verified-exact Newton-Raphson formula (finding #173), and `corner_smoothing`'s own disclosed "unverified against any reference" gap resolved via real research into Figma's actual squircle construction — a Bezier path, not an implicit distance field, so no code changed, only a real, quantified deviation now disclosed (finding #174, a decision, not a fix). "Phase 10 Step 10.2.5 Implementation" (2026-09-09) is also complete, below — real, analytic, bounded rounded stroke caps at a partial arc's own two cut angles (finding #175), a circle-SDF union of radius `border_thickness / 2` at each cut, proven on real GPU pixels a few degrees past each cut edge. "Phase 10 Step 10.2.6 Implementation" (2026-09-09) is also complete, below — and closes all six of Step 10.2's own disclosed gaps. Real, live zero-allocation proof for a `ShapeRegistry`-driven scene (`tre_memory::RenderTickGuard`), closing ARCHITECTURE.md Section 7.5's own disclosed gap; being the first real check of `Polygon`/texture-fill rendering under allocation pressure, it found and fixed two real, previously-undetected per-frame allocations (finding #176: `generate_polygon_points`/`fan_from_center` and `bounding_box_uvs` each returned a freshly heap-allocated `Vec` on every call), while disclosing one real, deeper, deliberately-unfixed gap (`lyon`-backed `Path`/bordered-`Polygon` tessellation, the same category as `main_loop_demo.rs`'s own Step 9.2 exclusions). **"Phase 11 Step 11.1 Implementation" (2026-09-10), below, is the newest section.** `tre-platform`'s two hand-rolled Wayland/X11 protocol backends were replaced by a single `winit`-backed implementation, at the project owner's own explicit direction ("a hand-rolled approach for windowing is not the right path"), with `PlatformConnection`'s public API preserved exactly. One incidental fix (finding #177: a hardcoded Wayland `app_id` leftover from Phase 0 disappeared along with the deleted backend it lived in) and one deliberate, disclosed-not-fixed decision (finding #179: `winit` materially increases the crate's dependency footprint even with a trimmed feature set) round out this step. A real, incidental hardening also landed: the crate needed no `unsafe` of its own after the migration and now carries `#![forbid(unsafe_code)]`, removed from TECHNICAL.md Section 9.1's closed set. **Finding #178** (`scale_factor`'s preserved `i32` signature rounding away real per-window fractional-DPI precision) was initially left as a deliberate, scope-bounded decision, then fixed same-day at the project owner's own explicit follow-up direction -- a workspace-wide grep confirmed the only real caller anywhere was a diagnostic print, so the "future API-breaking step" this finding originally anticipated turned out to touch one real call site, not the 40 originally feared.

**Further extending the index (2026-09-10).** "Full Workspace API Audit" is also complete, below (not previously indexed here) -- a full pass over every public API in the workspace for window-chrome gaps, stale documentation, and undocumented panics. Findings #180 (window-chrome methods, see Phase 11 Step 11.1 note above), #181-182 (stale/misattached doc comments), #183-184 (missing convenience-constructor/accessor API symmetry), #185 (stale comments naming a deleted function), #186-187 (missing `# Panics` docs and small API-completeness gaps) were all fixed and re-verified; #188-189 are documented, deliberately-not-fixed gaps (field/variant-level doc-comment completeness judged out of this pass's scope; two RHI trait methods that swallow a recoverable `Err` into a panic, whose real fix needs a trait-signature change out of scope for an audit pass). **"Phase 10 Step 10.4 Implementation" (2026-09-10), below, is the newest section** -- the first real slice of direct PyO3 Python bindings (`tre-python`), scoped by the project owner's own explicit choice (a generic wrapper over `tre-engine`'s existing API, not an attempt to match the separate `pySilver` project's own incompatible rendering contract, investigated and disclosed via `AskUserQuestion` before any code was written). Findings #190 (a real, release-build-only `unused_mut` warning, previously undetected because this session's own verification had only ever run in debug mode) and #191 (`EngineError` was the only error type in the workspace missing `Display`/`Error` impls) were both incidental fixes needed to unblock this step, not part of its own scope. Finding #192 is the step's central real defect: `RhiPipelineState` lacked `Send + Sync`, blocking GIL release entirely; fixed via the trait bound, not by marking the whole renderer `unsendable`. Finding #193 is a real segfault at Python interpreter shutdown, caused by a struct's field-declaration order being the opposite of the safe order every RHI example's own local-variable drop order gets for free. Finding #194 is a real, undocumented API-convention trap (`Circle`'s `x`/`y` mean bounding-box top-left, not center) that the step's own demo script got wrong on first use before being corrected and documented. Finding #195 records this pass's deliberate, disclosed scope boundaries (solid-fill-only shapes, no zero-copy buffer-protocol type, no CI job yet). **"`/review-project` Pass on `tre-python`" (2026-09-10), below, is the newest section** -- a four-lens (Performance/Architecture/Security/Modernizer) multi-agent review run at the user's own explicit instruction immediately after Step 10.4 landed. Found and fixed a critical, previously-undetected defect (#196: any second `render()` call on an unmutated registry produced an empty frame and crashed in release builds -- caught only because the review's own performance lens reasoned about the interaction between `flatten_into`'s incremental-dirty semantics and a fresh canvas built on every call, not by any test this step had already written), plus two real input-validation gaps (#197-198: unchecked renderer dimensions and polygon side count reaching raw Vulkan/engine calls with no bound) and one real concurrency hazard (#199: `render()` took `&self` over single-buffered GPU state reachable from two threads at once, now `&mut self` so PyO3's own borrow check enforces exclusivity). "Workspace-Wide `submit_frame` Migration" (2026-09-10) is also complete, below -- closes the one item the review above left open: real scope was 41 files, not the ~15 first estimated (confirmed via `AskUserQuestion` before proceeding), a new `tre_engine::submit_frame` helper now covers every real GPU-submitting call site in the workspace, findings #200-201 record the migration's own real script-missed edge cases and how each was fixed. **"Full-Project `/review-project` Pass" (2026-09-10), below, is the newest section** -- the same four-lens review, this time run across the entire ~25,000-line workspace at the user's explicit instruction ("Perform a full project wide /review-project"), not just one crate. Fixed one real, workspace-input-boundary finding (#202: `tre-python`'s shape fields accepted `NaN`/`+-inf`/negative values with no validation, the same class of gap #197-198 closed for two other fields, now generalized to every numeric field and `Path` coordinate). Three real findings left disclosed, not fixed in that pass, as genuine design decisions rather than unambiguous fixes: #203 (`render()`'s per-call re-flatten and unpooled buffer allocation), #204 (`tre-python` bypassing the `RhiDevice` trait abstraction), and #205 (`tre-engine/src/lib.rs`'s 6,834-line, four-concerns-in-one-file size). All three are since resolved, each at the user's own explicit follow-up instruction: "Fix render() re-tessellates and re-allocates GPU buffers on every call with Option B" closed #203 by adopting `RhiDynamicRingBuffer` (the project's own established ring-buffer pattern), which -- as predicted -- closed #204 for free at the same time (`tre-python`'s direct `ash` dependency removed entirely, since `create_dynamic_ring_buffer` is a real `RhiDevice` trait method); "Split lib.rs into modules" closed #205, splitting `lib.rs` into `input.rs`/`canvas.rs`/`rhi.rs` with zero changes needed anywhere outside `tre-engine` itself.

---

## How to read this

Each finding lists: severity, the document(s) touched, what was wrong, and what changed. Severities:

- **Critical** — a real gap in the architecture itself (not just the writing), left unaddressed it causes production incidents or silent correctness/perf regressions.
- **Should-fix** — a real risk or inconsistency, not immediately fatal, but will cause confusion or rework later.
- **Nice-to-have** — cheap insurance, low cost to add, meaningful payoff if it ever triggers.

---

## DESIGN.md

### 1. [Critical] No failure-mode / degradation principle
The five core principles (§2) covered the happy path only — zero-alloc, separation of concerns, frame budget, deterministic order, resolution independence — with no statement of what happens when any of those assumptions breaks.

**Change:** Added §2.6 "Explicit Failure Modes & Graceful Degradation," enumerating the five failure classes that must have a documented response before a subsystem ships: device loss / swapchain acquire failure, atlas exhaustion beyond LRU capacity, malformed SVG input, ring buffer / transient pool starvation, and shader compilation failure. Ties back to the project's no-exceptions rule (`std::expected`/error codes only).

### 2. [Should-fix] Ambiguous heterogeneous-primitive batching model
§8.1.2 claimed text (MSDF, R8/RGB8) and color icons (RGBA8) can share a draw call via bindless textures, but MSDF and RGBA sampling are different shader logic — normally a different `PipelineStateId`, which is itself a sort-key field, so the claim as written was self-contradictory.

**Change:** Added a "Shader Unification" clarification: `PipelineStateId` selects a shader *family*; within a family, a per-vertex shader-mode tag (packed into a spare `params` lane) branches between SDF-rect, plain-texture, and MSDF evaluation. One pipeline, one draw call, a cheap per-fragment branch instead of a state switch.

### 3. [Nice-to-have] Animation state ownership unspecified
§12.3 defined spring/lerp-decay math but never said who persists `x`, `v`, `x_target` across frames.

**Change:** Added §12.4 "Animation State Ownership" — the UI framework's widget tree owns this state; the Vector Math Engine is a stateless evaluation library. Prevents a second, drifting source of truth.

### 4. Documentation hygiene
§11.1's sRGB↔linear formula now points to TECHNICAL.md §6.2 (canonical) instead of restating it. See "Cross-Cutting" below.

---

## TECHNICAL.md

### 5. [Bug] Malformed budget table
§1's table had a header row but no `|---|---|---|` delimiter — breaks under strict CommonMark rendering.

**Change:** Delimiter row added.

### 6. [Should-fix] SPMC claimed, only one consumer described
§8 specified the event queue as Single-Producer *Multi*-Consumer, but DESIGN.md §5.1 describes exactly one consumer (the UI framework's logic tick). SPMC lock-free queues carry materially more complexity (consumer-side CAS races, ABA hazards) than SPSC for no benefit if there's truly one consumer.

**Change:** Corrected to SPSC, with a note that upgrading to SPMC requires naming the second consumer explicitly when one is actually added.

### 7. [Critical] No zero-allocation enforcement mechanism
The headline "0 bytes/frame" constraint had no verification method — an aspirational rule with no way to know if it's ever violated.

**Change:** Added §3.4 "Zero-Allocation Enforcement": debug/profile builds override `operator new`/`delete` to assert against a thread-local "render tick active" flag; CI runs the full suite under this guard as a hard gate (not just a benchmark), compiled out entirely in release builds.

### 8. [Critical] No shader cross-compilation strategy
Three backends (Vulkan/DX12/Metal) targeted, zero documents said how one shader source reaches SPIR-V, DXIL, and MSL.

**Change:** Added §9.3 "Shader Authoring & Cross-Compilation": single HLSL source, DXC to SPIR-V (Vulkan) and DXIL (DX12) natively, SPIRV-Cross to MSL (Metal), all at build time via CMake — never at runtime in a shipping build.

### 9. Bit-layout & documentation hygiene
§4 (sort key) and §5.1 (vertex format) now state only the numeric budget and reference ARCHITECTURE.md as canonical. See "Cross-Cutting."

---

## ARCHITECTURE.md

### 10. [Critical] Depth ID headroom too thin
The sort key's Depth ID field was 16 bits (65,536 slots) against a stated `>10,000 node` design target — only 6.5x margin, no overflow behavior specified.

**Change:** Rebalanced the 64-bit key: Layer 16 / Pipeline 16 / **Texture 12** / **Depth 20** bits (Texture only needed 12 bits — the engine maintains a few dozen atlases, not thousands). Depth ID now has 1,048,576 slots. Added a debug-build overflow assert and a release-build fallback (split into two sequential sub-frame passes rather than wrapping the counter). This bit-layout change was propagated to TECHNICAL.md and IMPLEMENTATION.md as well.

### 11. [Should-fix] Batching guarantee was traversal-order-dependent, not structural
§4.2 required both matching sort-key bits *and* matching `clipBounds` to merge into a batch, but `clipBounds` isn't in the key — same-clip commands are only contiguous post-sort because UI traversal happens to correlate with clip nesting. That correlation breaks under z-index overrides (DESIGN.md §7.1), which the docs already acknowledge as a case that reorders paint order.

**Change:** Documented this explicitly as a known, deliberate limitation: correctness is preserved (the explicit `clipBounds` compare prevents any incorrect merge), but the "single-digit draw call" target becomes a soft target under clip/z-index interleaving. Also added: Depth ID is now assigned *after* z-index resolution (true final-paint-order index), and a documented future fix (clip-bucketing secondary pass) is named rather than silently deferred.

### 12. [Should-fix] Virtual RHI dispatch vs. the project's own "no virtual in tight loops" standard
`IRhiDevice`/`IRhiCommandBuffer` (§6) are pure-virtual interfaces called on every batch — technically virtual dispatch, which the project's coding standards flag on review, with no justification recorded.

**Change:** Added an explicit note: dispatch is per-*batch* (single-digit to low-hundreds calls/frame), not per-primitive or per-vertex; overhead is amortized and negligible against the CPU budget. This is a bounded, deliberate exception — per-primitive code paths must still avoid virtual calls entirely.

### 13. [Nice-to-have] No PSO blend/depth-state specification
Painter's-algorithm ordering by Depth ID implies depth-test-off / blend-on, but this was never stated — a future contributor could "fix" this by enabling depth testing and silently break transparency ordering.

**Change:** Added §6.1 "Default Pipeline State (PSO) Configuration": depth test/write disabled, premultiplied-alpha blending in linear space, culling disabled.

---

## IMPLEMENTATION.md

### 14. [Critical — process risk] No walking skeleton before first pixel
Phases 1–5 (platform, RHI backends, memory pools, geometry/SVG, typography, multi-threaded canvas) build entirely before Phase 6 produces a single visible pixel — five phases of integration risk accumulating with zero end-to-end validation.

**Change:** Added Phase 0 "Walking Skeleton": a single-backend, single-threaded, minimal path from `Canvas::DrawRoundedRect` through a trivial one-element sort/flatten to `DrawIndexed` and present — validates the Canvas→IR→RHI contract shape before deeper investment in any one subsystem.

### 15. [Should-fix] No SVG input hardening
Phase 3.3's tessellator had no stated defense against adversarial SVG (recursive `<use>` bombs, unbounded point counts, deep group nesting) despite creative-workstation apps being a named target audience.

**Change:** Added a hardening task: hard caps on recursion depth, point count, and nesting depth, rejected via `std::expected` before tessellation begins — with an explicit call-out that a "trusted-SVG-only" integration must state that assumption itself rather than inherit it silently.

### 16. [Should-fix] No correctness testing strategy, only performance
TECHNICAL §9.2 and the implementation plan covered perf regression only — nothing validated that the batching/sort pipeline produces *correct* output, which is the more dangerous failure mode (fast but wrong, silently).

**Change:** Added Phase 9 "Testing & Validation Strategy": adversarial radix-sort unit tests, atlas-packer fragmentation/eviction tests, a batching-equivalence pixel-diff test (batched vs. naive per-primitive reference render), SVG fuzz testing, and CI gates for both the zero-allocation guard and the transient-pool balance assertion below.

### 17. [Nice-to-have] No transient-pool leak detection
An unbalanced `PushLayer`/`PopLayer` pair would starve the transient render target pool silently over many frames with no attributable failure point.

**Change:** Added a debug-mode balance assertion (Phase 2, Step 2.2): depth counter per `Canvas`, asserted zero at frame boundary.

---

## Cross-cutting: documentation hygiene

The 64-bit sort-key formula, the 32-byte `UiVertex` struct, the sRGB↔linear formula, and the MSDF opacity formula were each restated near-verbatim across three or four of these documents. That's real technical debt in the doc set itself — change one bit-field width, and four files need synchronized edits or they silently drift (which is exactly what had already happened: the Depth ID fix above would otherwise have needed hand-applying in four places).

**Change:** Established single canonical locations and made every other document reference them instead of restating:

- **Sort key bit layout & rationale** → canonical in `ARCHITECTURE.md` §4.1. `TECHNICAL.md` §4 and `IMPLEMENTATION.md` Step 6.1 now state only the numeric budget / task, with a reference.
- **`UiVertex` struct** → canonical in `ARCHITECTURE.md` §3.1. `TECHNICAL.md` §5.1 and `IMPLEMENTATION.md` Step 3.1 reference it.
- **sRGB ↔ Linear conversion formula** → canonical in `TECHNICAL.md` §6.2. `DESIGN.md` §11.1 and `IMPLEMENTATION.md` Step 7.1 reference it.
- **MSDF opacity formula** → canonical in `TECHNICAL.md` §5.3. `IMPLEMENTATION.md` Step 4.2 references it.

Going forward: if a number or struct field needs to change, edit the canonical section first, then check the three cross-reference notes still make sense. Don't restate the value in a second document — that's exactly the pattern that let the old 16-bit Depth ID ship unnoticed in three places.

---

## Follow-up: Rust/Python Language Migration (2026-09-04)

Status: **Implemented.** This is a subsequent project decision, not a finding from the September 2026 review above — recorded here as a follow-up entry rather than folded into the numbered findings, since the review itself is closed.

### 18. [Decision] Engine implementation language changed from C++ to Rust; Python UI framework added as the reference integration

All four documents previously specified C++20 throughout: compiler targets, `std::expected`-based error handling, C++ struct/class definitions for `UiVertex`, `UiDrawCommand`, and the `IRhiDevice`/`IRhiCommandBuffer` interfaces, and a CMake-based build system.

**Change:** All four documents updated to specify **Rust** as the engine's implementation language, with the project's own high-level UI framework built in **Python** as the engine's first consumer, while keeping the engine itself language-agnostic for whatever UI framework binds to it. Per-document changes:

- **DESIGN.md:** Executive Summary reworded to state the engine is implemented in Rust with Python as the first consumer. Section 2.6 restated in terms of `Result<T, EngineError>` and panic-vs-FFI-unwind safety instead of `std::expected`/C++ exceptions. New Section 2.7 "Implementation Language & Cross-Language Boundary" added, establishing the engine as language-agnostic via a stable C-ABI, with Python explicitly framed as the reference integration rather than a privileged special case. Section 3 gets a pointer naming the Python UI framework as the reference/dogfooding integration.
- **TECHNICAL.md:** Section 9.1 rewritten for the Rust toolchain (edition/MSRV, `unsafe` policy, no-dynamic-type-inspection-in-hot-paths as the RTTI-ban equivalent, no-unwinding-across-FFI rule). Section 9.2 rewritten for a Cargo workspace, `rustfmt`/`clippy`, and `cargo bench`/`cargo test` CI in place of CMake/clang-format/clang-tidy. Section 9.3's build-integration step updated from a CMake custom step to a Cargo `build.rs`. New Section 9.4 "Cross-Language FFI & Python Bindings" added, defining the ABI shape, error propagation, memory-ownership, and PyO3 binding rules.
- **ARCHITECTURE.md:** `UiVertex` (Section 3.1) and `UiDrawCommand`/`CommandType` (Section 3.2) struct/enum definitions translated to Rust (`#[repr(C, align(16))]`, `#[repr(u8)]`, const-assert size checks in place of `static_assert`). The `IRhiDevice`/`IRhiCommandBuffer` C++ virtual-class interfaces (Section 6) rewritten as Rust traits (`RhiDevice`, `RhiCommandBuffer`); the dispatch-exception rationale from finding #12 above is reworded from "C++ virtual dispatch" to "`dyn Trait` dispatch" but the underlying justification (once-per-batch, not per-vertex) is unchanged.
- **IMPLEMENTATION.md:** Platform-bridge, RHI-backend, and text-shaping tasks (Phases 1, 2, 4) annotated with their Rust binding crates (`windows-rs`, `wayland-client`/`x11rb`, `objc2`/`objc2-app-kit`/`objc2-metal`, `ash`, `harfbuzz_rs`, `freetype-rs`). The FMA-intrinsic and vertex-size-assertion tasks (Phase 3, Step 3.1) updated to Rust equivalents (`core::arch::x86_64`, `const _: () = assert!(...)`). The Architectural Decision Matrix gets a new "Implementation Language" row. New **Phase 10** "Cross-Language Bindings & Python UI Framework Integration" added, covering the `tre-ffi` C-ABI crate and PyO3-based Python bindings.

**Rationale (as given by the project):** Rust's ownership model provides compile-time memory- and data-race-safety for the zero-allocation, lock-free multi-threaded design (DESIGN.md Sections 2.1, 6.3) without a runtime GC that could threaten the frame budget (DESIGN.md Section 2.3). The C-ABI boundary keeps the engine's public surface usable by any UI framework language, not only Python — the Python UI framework exercises that boundary as proof it holds, rather than being a reason to special-case it.

**Note for future reviewers:** This entry is a language-migration record, not a re-review. The structural/architectural findings from the original September 2026 review (the sort-key bit layout, the batching model, the failure-mode taxonomy, etc.) are unaffected by the language change — only their code-level expression (C++ syntax → Rust syntax) was updated. A dedicated review of the Rust-specific additions introduced by this migration was performed on 2026-09-04 — see "Review of Rust-Specific Additions" below (findings #19–22), which caught a critical panic-strategy contradiction in exactly the FFI-safety mechanism this note flagged as unreviewed.

---

## Review of Rust-Specific Additions (2026-09-04)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: the Rust-specific mechanisms introduced by the language migration above (#18) — the `unsafe` policy, the FFI panic-safety mechanism, the `tre-ffi`/`cdylib` build model, and the PyO3 binding layer — across `TECHNICAL.md` and `IMPLEMENTATION.md`. This is the follow-up review flagged as outstanding in #18's original "Note for future reviewers."

Status: **All findings implemented.**

### 19. [Critical] Panic strategy contradicts its own FFI-safety mechanism
TECHNICAL.md Section 9.1 stated the `cdylib`/`staticlib` build targets set `panic = "abort"`, while that same paragraph — plus DESIGN.md Section 2.7 and IMPLEMENTATION.md Phase 10 Step 10.2 (renumbered from 10.1 on 2026-09-09 when Step 10.1 became the new shape-primitive step; this finding's own content is about the `tre-ffi` crate, still the step this reference means) — relies on `std::panic::catch_unwind` at every FFI entry point to convert a panic into a recoverable `EngineError`. These are mutually exclusive: `panic = "abort"` terminates the process the instant a panic fires, before any stack unwinding occurs, so `catch_unwind` can never trigger and silently becomes dead code. As written, any panic anywhere in the engine crashes the host Python process outright — precisely the outcome Section 2.7 says the `catch_unwind` wrapper exists to prevent.

**Change:** TECHNICAL.md Section 9.1 corrected: the `tre-ffi` crate and its full dependency graph must build with the default `panic = "unwind"` strategy, and `panic = "abort"` is now explicitly called out as prohibited on any profile used to build the shipped `cdylib`/`staticlib`, with the reasoning (it makes `catch_unwind` a no-op) stated inline so a future contributor optimizing binary size doesn't reintroduce it.

### 20. [Should-fix] "Only crate compiled into the cdylib" is imprecise to the point of being misleading
TECHNICAL.md Section 9.2 and IMPLEMENTATION.md Phase 10 Step 10.2 (renumbered from 10.1 on 2026-09-09, see finding #19's own note) both stated that `tre-ffi` is "the only crate compiled into the shipped `cdylib`/`staticlib`." Taken literally this is false: `tre-engine` and the RHI backend crates' code must be statically linked into that same binary for the engine to function at all. The intended meaning — that `tre-ffi` is the only crate whose items are exported as public `extern "C"` symbols — was never actually stated.

**Change:** Both documents reworded to distinguish *linked into* (true of every crate in the dependency graph) from *exports symbols from* (true of `tre-ffi` alone), so the symbol-hiding goal is stated accurately instead of implying the engine and RHI backends aren't part of the shipped binary at all.

### 21. [Should-fix] `unsafe` policy omitted the crate that needs it most
TECHNICAL.md Section 9.1's `unsafe` policy named only the RHI backend crates and the ring-buffer/arena allocators as permitted `unsafe` locations. The `tre-ffi` crate — described in Section 9.4 and IMPLEMENTATION.md Phase 10 as converting opaque raw pointers back into Rust references and manually transferring buffer ownership across the C-ABI boundary — is one of the most `unsafe`-code-dense crates in the workspace, yet the policy as written never authorized it to contain any.

**Change:** Added `tre-ffi` to the permitted-`unsafe` list, and made the rest of the workspace's stance explicit: every other crate, including `tre-engine` itself, carries `#![forbid(unsafe_code)]` — the policy now names a complete, closed set of allowed locations rather than leaving an implicit "everywhere else, presumably not" gap.

### 22. [Nice-to-have] Unspecified ordering between GIL release and panic-catching
TECHNICAL.md Section 9.4 documented GIL release (`Python::allow_threads`) around blocking engine calls and, separately, `catch_unwind`-based panic recovery (Section 9.1), but never stated which wraps which. If a panic fired while the GIL was released and `catch_unwind` were scoped only around the inner blocking call, the conversion to an `EngineError` would happen without the GIL held — unsafe for any subsequent PyO3/CPython API use.

**Change:** Section 9.4 now states explicitly that the `catch_unwind` guard wraps the entire PyO3-facing call, including the `allow_threads` scope, so a caught panic is converted to an `EngineError` only after the GIL has been reacquired on scope exit.

---

## Full Documentation Review (2026-09-04)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: a complete pass over `DESIGN.md`, `TECHNICAL.md`, `ARCHITECTURE.md`, `IMPLEMENTATION.md` in full, checking cross-references, verifying technical/API claims against fact rather than plausibility, and specifically auditing the Rust migration (#18) for completeness -- it had only touched the sections called out explicitly in its own change list, and the rest of each document was never re-swept for leftover C++.

Status: **All findings implemented.**

### 23. [Critical] Non-existent Rust API referenced
TECHNICAL.md Section 8 constrained sub-canvas concurrency to `std::thread::hardware_concurrency() - 1`. `hardware_concurrency()` is a C++11 `std::thread` member function; **Rust's `std::thread` module has no function of that name.** The correct Rust API is `std::thread::available_parallelism()` (stable since Rust 1.59), returning `io::Result<NonZeroUsize>`.

**Change:** Corrected to `std::thread::available_parallelism()` minus one, with the C++ name noted parenthetically so the mapping is traceable.

### 24. [Critical] Leftover, untranslated C++ syntax throughout
The Rust migration (#18) rewrote the two struct/trait code blocks in ARCHITECTURE.md but never swept the rest of the document set for C++ syntax embedded in prose and code snippets. Found and fixed:
- `std::atomic<size_t>::fetch_add` / `std::atomic::fetch_add` -- raw C++ template syntax (TECHNICAL.md §8, ARCHITECTURE.md §2.2, IMPLEMENTATION.md's Architectural Decision Matrix) -- corrected to `AtomicUsize::fetch_add`.
- A full C++ code line, `size_t writeOffset = globalCommandCounter.fetch_add(N_i, std::memory_order_relaxed);` (IMPLEMENTATION.md Step 5.2) -- corrected to `let write_offset = global_command_counter.fetch_add(n_i, Ordering::Relaxed);`.
- `memcpy` (IMPLEMENTATION.md Step 5.2) -- corrected to `copy_from_slice`.
- `uint64_t lastFrameUsed` (TECHNICAL.md §3.3, IMPLEMENTATION.md Step 2.3) -- corrected to `u64 last_frame_used`.
- `std::expected` as the error type for SVG-hardening rejections (IMPLEMENTATION.md Phase 3.3), directly contradicting DESIGN.md §2.6's own `Result<T, EngineError>` rule -- corrected to `Result<T, EngineError>`.

### 25. [Critical] Pervasive PascalCase C++/C#-style API naming across all four documents
Every prose reference to the engine's own API -- `Canvas::DrawRoundedRect`, `Canvas::TagAccessibilityNode`, `RhiDevice::BeginFrame`, `IRhiCommandBuffer::DrawIndexed`, `Canvas::PushLayer`/`PopLayer`, `CreateSubCanvas`, `BeginOverlay`, and more -- was left in PascalCase-with-`::` C++/C# style, and one interface kept its C++ "I"-prefix (`IRhiCommandBuffer`) even though ARCHITECTURE.md §6's own canonical trait definition (part of #18's fix) had already renamed it to plain `RhiCommandBuffer`. This isn't cosmetic: TECHNICAL.md §9.2 commits the project to `clippy` with `-D warnings`, and `non_snake_case` is a default rustc lint -- a codebase matching these docs literally would not build clean under its own stated CI gate. Also caught in the same sweep: `SVGDocumentHandle` (DESIGN.md §6.1) violates the `clippy::upper_case_acronyms` convention Rust type names follow elsewhere in the same docs (`RhiDevice`, not `RHIDevice`) -- corrected to `SvgDocumentHandle`.

**Change:** Every such reference across DESIGN.md, TECHNICAL.md, ARCHITECTURE.md, and IMPLEMENTATION.md renamed to snake_case methods on plain (non-`I`-prefixed) trait/type names -- roughly twenty individual sites. Real third-party API names quoted verbatim from Vulkan/DX12/PyO3 (`vkCmdDrawIndexed`, `ID3D12GraphicsCommandList::DrawIndexedInstanced`, `Python::allow_threads`) were left untouched, since those are correctly-cased external APIs, not TRE's own surface.

### 26. [Should-fix] "WinUI" mislabeled as the Windows accessibility bridge
DESIGN.md §5's architecture diagram labeled the accessibility bridge box "WinUI/NSAccess/AT-SPI." WinUI is Microsoft's separate native UI toolkit (WinUI 3 / Windows App SDK) -- unrelated to accessibility. The correct name, **UI Automation (UIA)**, is used correctly two sections later in the very same document (§5.2: "Windows UI Automation"), making this an internal inconsistency as well as a factual error.

**Change:** Diagram label corrected to "UIA/NSAccess/AT-SPI2" (also fixing "AT-SPI" to the more precise "AT-SPI2" used consistently elsewhere).

### 27. [Should-fix] Dangling cross-reference
DESIGN.md §2.6's SVG-hardening bullet cited "see Section 9.1 and IMPLEMENTATION.md Phase 3.3" -- but DESIGN.md §9.1 ("SVG Capability Scope") only describes *what SVG features are supported*, never malformed-input handling or tessellation-cost bounding. The citation pointed readers to a section that doesn't contain what it claims to.

**Change:** Removed the inaccurate "Section 9.1" half of the citation; the accurate IMPLEMENTATION.md Phase 3.3 reference (which does contain the hardening detail) is kept.

### 28. [Should-fix] Missing failure mode: transient render-target pool miss
DESIGN.md §2.6 enumerates five failure classes every subsystem must handle, but never addresses what happens when `Canvas::push_layer` requests an offscreen size/format the transient pool (TECHNICAL.md §3.2) doesn't already have -- despite this being at least as common in practice as several modes that *are* listed (any window resize, zoom-level change, or animated blur radius can produce a size never requested before).

**Change:** Added a sixth DESIGN.md §2.6 bullet: pool entries are bucketed to fixed size breakpoints so nearby requests share an entry, and a genuine miss borrows the next-larger already-pooled entry for that frame while a correctly-sized target grows into the pool asynchronously. Cross-referenced from TECHNICAL.md §3.2 and IMPLEMENTATION.md Step 2.2.

### 29. [Should-fix] `unsafe` policy gap reintroduced by its own prior fix
TECHNICAL.md §9.1's `unsafe` policy (established by finding #21) named a *closed* set of three permitted-unsafe locations -- RHI backends, ring-buffer/arena allocators, `tre-ffi`. It omitted wherever the SIMD/vector-math intrinsics live (IMPLEMENTATION.md Phase 3.1's `core::arch::x86_64::_mm256_fmadd_ps` call), even though that code is explicitly required to sit inside an `unsafe` block. As written, that code has nowhere it's allowed to exist.

**Change:** Added the vector-math/SIMD crate as a fourth permitted-`unsafe` location, with the reasoning (`core::arch` intrinsics are inherently `unsafe` in Rust) stated inline.

### 30. [Should-fix] "Strictly 256-bit SIMD" isn't achievable on a target this same document requires
TECHNICAL.md §5.4 required SVG path-morphing interpolation to use "strictly ... 256-bit SIMD processing." 256-bit SIMD (AVX2/YMM) exists on x86_64; ARM64 NEON is 128-bit, and there is no portable 256-bit SIMD width on ARM64. Section 2.2 of the very same document requires ARM64/NEON support, so "strictly 256-bit" was never satisfiable on half the engine's stated CPU targets.

**Change:** Reworded to require "the widest SIMD width available on the target," with the concrete 256-bit-vs-128-bit split spelled out and the implication (this cannot be one shared intrinsic-level code path across architectures) stated explicitly rather than left implicit.

### 31. [Nice-to-have] Metal version / macOS floor pairing looks mismatched
TECHNICAL.md §2.1 paired "Metal 2.4+" with "macOS 10.14+." Metal 2.4 and Argument Buffers Tier 2 shipped years after Mojave (10.14, 2018); asserting a specific corrected macOS version here without checking Apple's current Metal Feature Set tables risks swapping one unverified number for another.

**Change:** Rather than guess a replacement, the doc now flags this pairing explicitly as unverified and instructs implementers to check Apple's current Feature Set tables before building against it.

### 32. [Nice-to-have] Crate-name imprecision
IMPLEMENTATION.md referred to "the `windows-rs` crate" and "the `freetype-rs` binding crate." Those are the names of the upstream GitHub projects; the packages actually published to crates.io are named `windows` and `freetype` respectively.

**Change:** Both references clarified to name the actual crates.io package alongside the project name.

---

## Engineering Decisions: Suggested Improvements Actioned (2026-09-04)

Following the full documentation review above, seven engineering suggestions raised during that review were directed by the project owner and implemented. These are design decisions, not bug fixes -- recorded here in the same numbered sequence for traceability.

### 33. [Decision] Multi-window atlas race resolved with a lock-free MPSC request queue + SWMR publish table
Two windows on independent render timelines could both discover a missing glyph in the same tick and need to mutate the single shared atlas; nothing in the docs specified how that's made safe. Directed: resolve with a lock-free structure for performance, not a mutex.

**Change:** Added DESIGN.md Section 10.3 (principle: single atlas owner, no window ever blocks on another) and ARCHITECTURE.md Section 2.3 (concrete design: a bounded MPSC ring buffer carries insertion requests from any window to the one atlas owner; the owner is the only code that ever touches the Guillotine free-rectangle list; completed results publish into a fixed-capacity single-writer/multi-reader `AtomicU64` slot table that every window reads lock-free via `Ordering::Acquire`/`Release`). TECHNICAL.md Section 8 specifies the concurrency primitives. A window whose glyph isn't yet published falls back to the existing placeholder-glyph degradation path (DESIGN.md Section 2.6) for that one frame rather than stalling.

### 34. [Decision] Clip-bucketing bit budget corrected: steal from Pipeline ID, not Depth ID
The originally-suggested fix for ARCHITECTURE.md Section 4.2's deferred clip-bucketing enhancement didn't specify which sort-key field would donate bits. Directed: take them from Pipeline ID.

**Change:** ARCHITECTURE.md Section 4.2 now states explicitly that Depth ID must not be touched -- it was widened from 16 to 20 bits in finding #10 specifically because 16 bits' 6.5x margin was judged too thin, and reusing that budget would silently reintroduce the exact problem that fix closed. Pipeline ID (16 bits / 65,536 states) has substantial real slack for a UI-focused engine's realistic pipeline-family count; trimming it to roughly 10-12 bits frees 4-6 bits for a future clip-group field. The live 64-bit layout is unchanged -- this only corrects the guidance for whenever clip-bucketing is actually implemented.

### 35. [Decision] Adopt the `wide` crate instead of hand-written duplicate AVX2/NEON code
Directed: use `wide` to avoid maintaining two hand-written SIMD implementations.

**Change:** TECHNICAL.md Sections 2.2, 5.4, and 7.2, and IMPLEMENTATION.md Steps 3.1 and 3.3, updated to specify `wide`'s portable `f32x4`/`f32x8` types as the primary SIMD path -- one shared, safe source-level implementation that compiles to native AVX2 on x86_64 and transparently emulates 256-bit operations as paired 128-bit NEON operations on ARM64. Because `wide`'s public API is safe Rust, this also *removed* the vector-math crate from TECHNICAL.md Section 9.1's `unsafe`-permitted list (previously added by finding #29) -- raw `core::arch` intrinsics are no longer needed for this code path, so the crate doesn't need `unsafe` at all.

### 36. [Decision] Wire native GPU API validation layers into debug/CI builds
Directed: add this CI gate.

**Change:** Added IMPLEMENTATION.md Phase 2 Step 2.4 "GPU API Validation in Debug & CI Builds" (Vulkan validation layers, the D3D12 debug layer, Metal API validation, each gated to debug/CI only and failing the build on any validation error) and a corresponding TECHNICAL.md Section 9.2 CI bullet. This specifically covers the one class of bug the existing CPU-side gates (zero-allocation guard, `clippy`, batching-equivalence tests) cannot see: misuse at the `unsafe` FFI boundary into the raw graphics APIs.

### 37. [Decision] `FxHashMap`/`ahash` for the transient-pool and atlas hot-path lookups
Directed: use a faster non-cryptographic hasher for these two structures.

**Change:** TECHNICAL.md Section 3.2 and IMPLEMENTATION.md Steps 2.2 and 4.2 updated to specify `FxHashMap`/`ahash` in place of `std::collections::HashMap`'s default SipHash for the transient render-target pool and the atlas owner's internal LRU bookkeeping. Made explicit in the same edits: this is a distinct structure from the lock-free `AtomicU64` slot table added by #33 -- a hasher swap makes single-threaded lookups faster, it does not make anything safe for concurrent access.

### 38. [Decision] Opaque-first depth-tested pre-pass flagged as a profiling-gated future consideration
Directed: note this for future profiling rather than building it now.

**Change:** Added a "Future consideration -- not implemented" note to ARCHITECTURE.md Section 6.1, describing the technique (a front-to-back depth-tested pre-pass for provably-opaque batches, reclaiming early-Z rejection the current depth-test-off design forgoes) and its costs, with an explicit instruction not to build it until profiling a representative overdraw-heavy scene confirms GPU time, not CPU submission time, is the actual bottleneck.

### 39. [Decision] Replaced ACES filmic tone mapping with a UI-appropriate default curve
Directed: this is a desktop UI engine, not a photo/film/image tool -- ACES's cinematic contrast and desaturation shaping is the wrong default; use something gentler.

**Change:** Added a new canonical formula, TECHNICAL.md Section 6.3 "HDR-to-SDR Tone Mapping": identity below standard white (every ordinary UI color reaches the screen bit-for-bit unchanged) and a continuous, monotonic Reinhard-style compression of only the content explicitly authored above white (audio meter peaks, HDR video preview, brightness indicators, per DESIGN.md Section 11.2), parameterized by the display's actual reported headroom rather than a fixed constant. ACES remains available as an explicit opt-in style choice for creative-workstation/DAW integrations that specifically want it for embedded video content, but is no longer the default. DESIGN.md Section 11.2 and IMPLEMENTATION.md Section 7.1 updated to reference the new canonical formula.

---

## Phase 0 Implementation (2026-09-04)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: actually implementing IMPLEMENTATION.md's Phase 0 walking skeleton -- a real Cargo workspace (`crates/tre-engine`, `crates/tre-rhi-vulkan`), not documentation. Recorded here because Phase 0's own rationale is explicitly to surface interface mismatches "while it is still cheap to change," and it did: real gaps in the ARCHITECTURE.md trait sketch and real bugs, all found and fixed during implementation, verified against a real GPU (AMD Radeon 890M / RADV) with the Vulkan validation layer enabled, not by inspection.

Status: **Phase 0 complete.** `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace; 120 frames presented with zero validation-layer errors; a screenshot confirms the rendered rectangle's color and position match the `Canvas::draw_rounded_rect` call that produced it.

### 40. [Critical] ARCHITECTURE.md Section 6's RHI trait sketch was incomplete in a way that blocked implementation
`RhiBuffer`, `RhiTexture`, `RhiPipelineState`, and `RhiSwapchain` were referenced as `&dyn Rhi*` parameters but never given method signatures. Worse, actually wiring `begin_frame`/`submit_and_present` together exposed that there was no way for a `RhiDevice` to recover its own concrete backend state from a `Box<dyn RhiCommandBuffer>` it had handed out, short of `std::any::Any` downcasting -- which TECHNICAL.md Section 9.1 explicitly bans from the per-frame path.

**Change:** Defined all four traits in `tre-engine`, using an opaque-`u64`-handle pattern (a Vulkan handle reinterpreted via `ash::vk::Handle::as_raw`/`from_raw`, conceptually identical to how Vulkan itself represents every object) so concrete implementations exchange state through ordinary trait-method calls and return values, never through downcasting. ARCHITECTURE.md Section 6 updated in place with the real, validated trait definitions -- see that section for the full code.

### 41. [Critical] `begin_frame`/`submit_and_present` had no error return type, contradicting DESIGN.md Section 2.6
DESIGN.md Section 2.6 requires device-loss/swapchain-out-of-date conditions to be "detected at `RhiDevice::begin_frame` and surfaced as a recoverable error." ARCHITECTURE.md's sketch gave `begin_frame` a bare `Box<dyn RhiCommandBuffer>` return with no way to report failure at all.

**Change:** Both methods now return `Result<_, EngineError>`. `begin_frame` additionally returns the acquired swapchain image alongside the command buffer, since the command buffer needs to know which image it's rendering into.

### 42. [Should-fix] A `u32` RGBA hex literal does not pack the way it visually reads
Writing `0xE0_A0_40_FFu32` and expecting byte 0 = `0xE0` (R) is wrong: little-endian storage places the *last* two hex digits at the *lowest* address, so the literal actually produces `[0xFF, 0x40, 0xA0, 0xE0]` in memory -- backwards for an `R8G8B8A8`-format vertex attribute. This was caught visually: a screenshot of the walking skeleton showed a pink rectangle where an amber one was requested.

**Change:** Added `tre_engine::rgba8(r, g, b, a) -> u32`, which packs correctly via `u32::from_le_bytes`, so no caller has to reason about endianness by hand. Locked in with a unit test that reads the packed value back through `to_le_bytes` rather than asserting a specific numeric constant.

### 43. [Critical] Three real Vulkan object-lifecycle bugs, none caught by code review -- only by running it
* **Freeing a command buffer immediately after submitting it.** `vkFreeCommandBuffers` on a still-pending buffer is undefined behavior per spec; the Vulkan validation layer caught it immediately (`VUID-vkFreeCommandBuffers-pCommandBuffers-00047`). Fixed by allocating one command buffer once and reusing it every frame (`vkResetCommandBuffer`) instead of allocate-then-free per frame -- which the command pool was already created with `RESET_COMMAND_BUFFER` to support, unused until this fix.
* **Reusing one `render_finished` semaphore across every frame.** The CPU-side fence `begin_frame` waits on covers the queue submit's completion, not the separate, asynchronous present operation's -- so a shared semaphore could still be referenced by a not-yet-retired present when the next frame tried to re-signal it (`VUID-vkQueueSubmit-pSignalSemaphores-00067`). Fixed with one `render_finished` semaphore per swapchain image, threaded through `AcquiredImage`.
* **Struct field drop order destroying dependencies before dependents, twice.** Rust drops a struct's own fields in *declaration* order, not reverse -- the opposite of local-variable drop order, and easy to get backwards. This surfaced as validation errors (destroying a device while buffers/pipeline still referenced it) and then as a SIGSEGV inside `libwayland-client.so` (destroying a window's surface before the swapchain built on it was destroyed) once the first issue was fixed. Root-caused via `coredumpctl gdb`'s backtrace, not guessed. Fixed by reordering both structs so dependencies are declared (and therefore dropped) before what they depend on, plus an explicit `vkDeviceWaitIdle` in a custom `Drop` impl before any of it runs.

**Technical Rationale (all of #43):** None of these are exotic -- they're the standard first-timer's set of Vulkan object-lifetime mistakes, and exactly why "does it compile" is a weak substitute for "does it run under the validation layer against a real GPU." Phase 9's testing strategy (batching-equivalence pixel-diff, adversarial radix-sort tests) already establishes this project's own bias toward runtime verification over inspection; this is the same principle applied to Phase 0.

---

## Phase 1 Step 1 Implementation (2026-09-05)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 1.1's native windowing and multi-window/headless support -- scoped to Linux only (Wayland + X11 via XWayland, both confirmed testable on the dev machine), per an explicit scope decision with the project owner; Windows/macOS bridges deferred to their own later steps. Full detail in `planning/archive/PLAN_PHASE1_STEP1.md` and `LOG_PHASE1_STEP1.md`; this is the summary for the documentation's own record.

Status: **Linux complete.** New `tre-platform` crate (native Wayland + X11/XCB windowing), `VulkanDevice::create_surface` extracted for multi-window sharing, `HeadlessSwapchain` added. Three examples (`walking_skeleton` migrated off `winit`, `multi_window`, `headless`) all verified against real hardware with `VK_LAYER_KHRONOS_validation` enabled: zero errors.

### 44. [Critical] `RhiDevice::submit_and_present`'s post-render layout transition assumed every `RhiSwapchain` is a real presentable swapchain
`VulkanDevice::submit_and_present` unconditionally transitions the rendered image `COLOR_ATTACHMENT_OPTIMAL -> PRESENT_SRC_KHR` before ending its command buffer -- correct for `VulkanSwapchain`, meaningless for `HeadlessSwapchain`'s plain (non-presentable) image. Caught immediately by the Vulkan validation layer as a layout mismatch when `HeadlessSwapchain::present`'s own barrier assumed the image was still in `COLOR_ATTACHMENT_OPTIMAL`.

**Change:** `HeadlessSwapchain::present`'s barrier now starts from the layout the image is actually in (`PRESENT_SRC_KHR`) rather than the layout a windowed swapchain would leave it in -- tagging a non-swapchain image with `PRESENT_SRC_KHR` transiently is unusual but valid, since it is only a layout tag, not proof of swapchain object identity. This is an interim fix, not the real one: the underlying design issue -- a swapchain-specific transition hardcoded into the *shared* `RhiDevice` code, when different `RhiSwapchain` implementations need different post-render handling -- should be resolved by letting each concrete swapchain control its own transition before more swapchain variants (e.g. a future DX12/Metal headless backend) get built on top of the current pattern.

### 45. [Should-fix] Leaked `VkSurfaceKHR` in the headless demo
`VulkanDevice::new` requires a window purely to probe present support while selecting a physical device -- there is no surface-less device-selection path. This is awkward for headless mode, which conceptually has no window at all; the headless demo's throwaway probe window/surface was never explicitly destroyed, and the validation layer caught the leak at `vkDestroyInstance`.

**Change:** the demo now calls `surface_loader.destroy_surface` on the probe surface immediately after device creation. The underlying awkwardness (headless mode needing a real, if invisible, window just to bootstrap a device) is a real API gap -- a genuinely surface-less physical-device-selection path is deferred to Phase 2's device-selection work rather than solved here.

### 46. [Nice-to-have] Two confirmed non-bugs, worth recording so they aren't re-investigated as bugs later
* A Wayland surface with no buffer attached renders nothing at all (unlike X11, which shows a blank mapped window backed by a real pixmap) -- this is correct `xdg-shell` protocol behavior, not a failed window open. A pre-Vulkan windowing-only smoke test produced an invisible Wayland window and a visible X11 one for exactly this reason; wiring up Vulkan (which attaches real buffers) made both visible as expected.
* `xdg-shell` gives clients no mechanism to request a top-level window's screen position (X11 does). The multi-window demo's two unpositioned windows can land at the same compositor-chosen spot and visually overlap in a screenshot -- a window-manager placement artifact, not evidence the shared-`RhiDevice` multi-window model is broken (independently confirmed via terminal output and zero validation errors across the full run).

---

## Pre-Phase-1-Step-2 Doc Check (2026-09-05)

### 47. [Should-fix] IMPLEMENTATION.md Step 1.2 still said "SPMC," never updated when TECHNICAL.md's canonical description was corrected to SPSC
TECHNICAL.md Section 8 was corrected from SPMC to SPSC in the original September 2026 review (the engine has exactly one consumer -- DESIGN.md Section 5.1's UI-framework logic tick). IMPLEMENTATION.md Step 1.2's task 1 restated the queue design instead of referencing the canonical section, so it kept the pre-correction "SPMC" value and drifted silently -- undetected until planning this step, since nothing had implemented Step 1.2 yet to surface the mismatch.

**Change:** Step 1.2's task now points to TECHNICAL.md Section 8 as canonical instead of restating the queue's producer/consumer model, with the drift's cause noted inline so the same restatement pattern isn't repeated.

---

## Phase 1 Step 2 Implementation (2026-09-05)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 1.2's input event pipeline -- `tre-platform` consolidated to one `PlatformConnection` per backend (owning multiple windows via `WindowId`), a real `tre_memory::SpscRingBuffer<T>`, `tre_engine::{InputEvent, InputEventQueue}` with pointer-move coalescing, and pointer/keyboard translation on both Wayland (`wl_seat`) and X11. Full detail in `planning/archive/PLAN_PHASE1_STEP2.md` and `LOG_PHASE1_STEP2.md`; this is the summary for the documentation's own record.

Status: **Linux complete.** All three Step 1.1 examples plus `smoke_test` migrated to `PlatformConnection`; new `input_demo` (two windows, `demo/phase1_step2/`) proves input works and routes correctly. `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace. All Vulkan examples verified against real hardware with `VK_LAYER_KHRONOS_validation` enabled: zero errors. Real pointer/button/key input synthesized via the X11 XTEST extension and shown to translate correctly, including correct `WindowId` routing across two simultaneously-open windows with zero cross-window leakage.

### 48. [Nice-to-have, process] A genuine data-race hazard in the pointer-move coalescing design was found and avoided before it was ever built
The first design considered for coalescing had the producer find the most-recently-*published* ring-buffer slot for a given window and overwrite it in place. This is unsound whenever the queue holds exactly one unconsumed item: the underlying slot at `head - 1` equals `tail` in that case, meaning a concurrent consumer could be mid-`assume_init_read()` of that exact slot while the producer tries to overwrite it -- a real torn-read/data-race hazard, not merely a style concern, and one that would only surface once a genuine second consumer thread was introduced (undetectable under this step's own single-threaded producer/consumer scope).

**Change:** `tre_engine::InputEventQueue` instead stages the pending move in an ordinary (non-atomic) struct field that is producer-exclusive until explicitly flushed via a normal `push()` call -- the shared `SpscRingBuffer` itself is never touched by the coalescing logic, so it stays sound if a real second consumer thread is introduced later, matching that type's own "no redesign needed" design goal. Recorded here because the hazard was reasoned out at design time rather than caught by a crash or a validation layer, and is exactly the kind of subtle SPSC mistake worth a written record so it isn't reintroduced later.

### 49. [Nice-to-have] Live compositor-level input synthesis was verified for X11 but not Wayland
Genuine end-to-end verification (not just code review) requires driving real OS-level input into a running window and checking the translated `InputEvent`s. The X11 backend was verified this way using the XTEST extension (the same mechanism `xdotool`/`ydotool` use) -- synthesized pointer motion, a button click, and a key press/release all translated correctly, including correct per-window routing when two windows were open simultaneously. No equivalent mechanism was available for Wayland in this session: the compositor (KWin) does not advertise `org_kde_kwin_fake_input`, and wlroots-specific virtual-pointer/virtual-keyboard protocols do not apply to KWin.

**Disposition:** not fixed, since there is nothing to fix in the product -- this is a verification-environment gap. Wayland's pointer/keyboard translation code was verified by careful code review and structural parity with the XTEST-verified X11 implementation (identical event model, identical coalescing path through the shared `InputEventQueue`). Recorded as an honest limitation rather than silently claimed as fully verified; live Wayland input synthesis (e.g., via a compositor that supports `wlr-virtual-pointer`/`virtual-keyboard-unstable-v1`, or a KWin session with fake-input enabled) is a reasonable follow-up if stronger verification is ever needed.

### 50. [Nice-to-have] Unhinted window placement causes same-position stacking on X11 too, not just Wayland
Step 1.1 (finding #46) already recorded that Wayland's `xdg-shell` gives clients no control over top-level window position, so unpositioned windows can visually overlap. The same default-placement behavior was observed on X11 via KWin's XWayland window management while verifying multi-window input routing: two same-size, unpositioned windows landed at the same screen location, so whichever was topmost received pointer input regardless of which window's own reported geometry the test harness had targeted.

**Disposition:** not a `tre-platform` defect -- confirmed by explicitly raising and focusing the intended target window before synthesizing input, after which routing was unambiguous and correct (A, then B, then A again, each tagged correctly with zero leakage). This is a test-harness/window-manager-placement concern, not a product one; recorded so it isn't mistaken for an `InputEvent` routing bug if noticed again.

---

## Summary table

| # | Finding | Doc(s) | Severity | Status |
|---|---|---|---|---|
| 1 | No failure-mode principle | DESIGN | Critical | Fixed |
| 2 | Heterogeneous batching model ambiguous | DESIGN | Should-fix | Fixed |
| 3 | Animation state ownership unspecified | DESIGN | Nice-to-have | Fixed |
| 5 | Malformed budget table | TECHNICAL | Bug | Fixed |
| 6 | SPMC claimed, one consumer | TECHNICAL | Should-fix | Fixed |
| 7 | No zero-alloc enforcement | TECHNICAL | Critical | Fixed |
| 8 | No shader cross-compilation strategy | TECHNICAL | Critical | Fixed |
| 10 | Depth ID headroom too thin | ARCHITECTURE (+TECHNICAL, IMPLEMENTATION) | Critical | Fixed |
| 11 | Batching guarantee traversal-order-dependent | ARCHITECTURE | Should-fix | Documented as known limitation + mitigation path |
| 12 | Virtual RHI dispatch vs. coding standard | ARCHITECTURE | Should-fix | Fixed (justified + scoped) |
| 13 | No PSO blend/depth-state spec | ARCHITECTURE | Nice-to-have | Fixed |
| 14 | No walking skeleton before first pixel | IMPLEMENTATION | Critical (process) | Fixed |
| 15 | No SVG input hardening | IMPLEMENTATION | Should-fix | Fixed |
| 16 | No correctness testing strategy | IMPLEMENTATION | Should-fix | Fixed |
| 17 | No transient-pool leak detection | IMPLEMENTATION | Nice-to-have | Fixed |
| — | Formula/struct duplication across docs | All four | Should-fix (docs debt) | Fixed — canonical locations established |
| 18 | Engine language migrated C++ → Rust; Python UI framework added | All four | Decision (follow-up, 2026-09-04) | Implemented — see "Follow-up: Rust/Python Language Migration" |
| 19 | `panic = "abort"` makes the `catch_unwind` FFI-safety mechanism a no-op | TECHNICAL (+DESIGN, IMPLEMENTATION reference it) | Critical | Fixed — profile switched to `panic = "unwind"`, abort explicitly prohibited |
| 20 | "Only crate compiled into the cdylib" is literally false | TECHNICAL, IMPLEMENTATION | Should-fix | Fixed — reworded to "linked into" vs. "exports symbols from" |
| 21 | `unsafe` policy omits the `tre-ffi` crate | TECHNICAL | Should-fix | Fixed — `tre-ffi` added; rest of workspace now explicitly `forbid(unsafe_code)` |
| 22 | GIL-release vs. panic-catch ordering unspecified | TECHNICAL | Nice-to-have | Fixed — `catch_unwind` now documented as wrapping the `allow_threads` scope |
| 23 | `std::thread::hardware_concurrency()` doesn't exist in Rust | TECHNICAL | Critical | Fixed — corrected to `std::thread::available_parallelism()` |
| 24 | Leftover untranslated C++ syntax (`uint64_t`, `size_t`, `memcpy`, `std::atomic<size_t>`, `std::expected`) | TECHNICAL, ARCHITECTURE, IMPLEMENTATION | Critical | Fixed — all converted to Rust equivalents |
| 25 | Pervasive PascalCase/`I`-prefix API naming contradicts the docs' own clippy CI gate | All four | Critical | Fixed — ~20 sites renamed to snake_case / de-prefixed |
| 26 | "WinUI" mislabeled as the Windows accessibility bridge | DESIGN | Should-fix | Fixed — corrected to "UIA" |
| 27 | Dangling cross-reference to DESIGN §9.1 for SVG hardening | DESIGN | Should-fix | Fixed — inaccurate half of citation removed |
| 28 | No failure mode for a transient render-target pool miss | DESIGN (+TECHNICAL, IMPLEMENTATION) | Should-fix | Fixed — pool bucketing + next-larger-entry fallback added |
| 29 | `unsafe` policy (from #21) omits the SIMD/vector-math crate | TECHNICAL | Should-fix | Fixed — added as a fourth permitted location |
| 30 | "Strictly 256-bit SIMD" unachievable on the ARM64 target this doc requires | TECHNICAL | Should-fix | Fixed — reworded to per-architecture width |
| 31 | Metal 2.4 / macOS 10.14 version pairing looks mismatched | TECHNICAL | Nice-to-have | Flagged in-doc as unverified rather than guessing a fix |
| 32 | "windows-rs"/"freetype-rs" are project names, not crate names | IMPLEMENTATION | Nice-to-have | Fixed — actual crates.io names clarified |
| 33 | Multi-window shared-atlas glyph-insertion race | DESIGN, ARCHITECTURE, TECHNICAL | Decision | Implemented — lock-free MPSC request queue + SWMR `AtomicU64` publish table |
| 34 | Clip-bucketing bit source corrected | ARCHITECTURE | Decision | Implemented — guidance points to Pipeline ID, not Depth ID; live layout unchanged |
| 35 | Adopt `wide` crate for SIMD | TECHNICAL, IMPLEMENTATION | Decision | Implemented — also let #29's `unsafe` grant to the vector-math crate be removed |
| 36 | GPU API validation layers in debug/CI | TECHNICAL, IMPLEMENTATION | Decision | Implemented — new IMPLEMENTATION Step 2.4 + TECHNICAL §9.2 CI bullet |
| 37 | `FxHashMap`/`ahash` for pool + atlas lookups | TECHNICAL, IMPLEMENTATION | Decision | Implemented — distinguished from the unrelated #33 concurrency fix |
| 38 | Opaque-first depth-tested pre-pass | ARCHITECTURE | Decision | Implemented — documented as a profiling-gated future consideration only |
| 39 | UI-appropriate tone-mapping curve replaces ACES default | TECHNICAL, DESIGN, IMPLEMENTATION | Decision | Implemented — new canonical formula, TECHNICAL §6.3 |
| 40 | ARCHITECTURE §6's RHI trait sketch was incomplete (undefined sub-traits, `Any`-downcast trap) | ARCHITECTURE | Critical | Fixed — real traits defined using an opaque-handle pattern, no downcasting |
| 41 | `begin_frame`/`submit_and_present` had no error return, contradicting DESIGN §2.6 | ARCHITECTURE | Critical | Fixed — both now return `Result<_, EngineError>` |
| 42 | `u32` RGBA hex literal packs backwards from how it visually reads | tre-engine (code) | Should-fix | Fixed — `rgba8()` helper + locking unit test |
| 43 | Three real Vulkan lifecycle bugs (command-buffer free-while-pending, shared present semaphore, struct drop order) | tre-rhi-vulkan (code) | Critical | Fixed — found via validation layer + `coredumpctl` backtrace, not inspection |
| 44 | `submit_and_present`'s post-render transition assumes every swapchain is presentable | tre-rhi-vulkan (code) | Critical | Interim fix applied; real fix (per-swapchain transition) deferred |
| 45 | Leaked `VkSurfaceKHR` in the headless demo's probe window | tre-rhi-vulkan (code) | Should-fix | Fixed — explicit `destroy_surface`; underlying API gap deferred to Phase 2 |
| 46 | Two confirmed non-bugs (invisible bufferless Wayland surface; no client-side window positioning) | tre-platform (code) | Nice-to-have | Recorded, not fixed — expected protocol behavior |
| 47 | IMPLEMENTATION.md Step 1.2 restated "SPMC," never updated to match TECHNICAL §8's SPSC correction | IMPLEMENTATION | Should-fix | Fixed — now references TECHNICAL §8 instead of restating |
| 48 | Coalescing-in-the-ring-buffer design would race a concurrent consumer | tre-engine / tre-memory (design) | Nice-to-have (process) | Avoided at design time — staged in a producer-exclusive field instead |
| 49 | Live Wayland input synthesis unverified (KWin lacks fake-input protocols) | tre-platform (verification) | Nice-to-have | Recorded as an honest gap — X11 verified via XTEST; Wayland via code review + structural parity |
| 50 | Unhinted window placement stacks windows on X11 too, not just Wayland | tre-platform (verification) | Nice-to-have | Confirmed non-bug — harness now raises/focuses target window explicitly |

Note on #11: this one is deliberately documented rather than "solved," per the finding's own conclusion — folding `clipBounds` into the sort key isn't possible without shrinking Layer, Pipeline, or the now-widened Depth field, and the risk is a performance regression (more batches than optimal), not a correctness bug. A clip-bucketing secondary pass is named as the future fix if profiling ever shows it matters.

---

## Phase 1 Review (2026-09-05)

Reviewer: two sub-agents (Rust correctness, security), per the project's standing "review each completed phase before the next begins" process. Scope: everything Phase 1 touched -- `tre-platform` (native windowing + input), `tre-memory` (the new `SpscRingBuffer`), `tre-engine`'s new `InputEvent`/`InputEventQueue` types, and `tre-rhi-vulkan`'s existing surface/window integration.

Status: **No Critical or High severity findings.** Both reviewers independently confirmed the SPSC ring buffer's atomic ordering and the `InputEventQueue` coalescing design are sound. Findings below are Medium/Low process and robustness gaps, plus one pre-existing (not introduced this phase) documentation-policy violation.

### 51. [Should-fix, pre-existing] `tre-rhi-vulkan` had zero `SAFETY:` comments across roughly 65 `unsafe` blocks
TECHNICAL.md Section 9.1 requires "every `unsafe` block requires an adjacent `// SAFETY:` comment stating the invariant being upheld," and `tre-memory`/`tre-platform` both comply. `tre-rhi-vulkan/src/lib.rs` and `src/headless.rs` did not -- every `unsafe` block in both files (introduced across Phase 0 and Step 1.1, not by Step 2) lacked one. Not a correctness bug by itself, but a real, systemic policy violation that gets more expensive to fix the longer it's left, since Phase 2 adds substantially more Vulkan code on top of this base.

**Change:** 44 comments added to `lib.rs`, 19 to `headless.rs` (matching every `unsafe` block in both files), each stating the specific invariant relied on for that call (e.g. "handle was just created above on this same device," "fence wait above guarantees the GPU is done with prior work") rather than repeated generic text. Purely additive -- confirmed via `git diff --stat` (227 insertions, 0 deletions) and a clean `cargo build`/`clippy -D warnings`/`fmt --check`/`test` pass, plus a re-run of `walking_skeleton` under `VK_LAYER_KHRONOS_validation` with zero errors, to confirm no behavior changed.

### 56. [Nice-to-have] `Drop for VulkanSwapchain` doesn't call `device_wait_idle` before destroying its resources, unlike `HeadlessSwapchain`
Found while adding SAFETY comments (finding #51): `Drop for VulkanSwapchain` destroys semaphores, image views, the swapchain, and the surface directly with no `device_wait_idle()` call first, while `Drop for HeadlessSwapchain` does call it. If `VulkanSwapchain` were ever dropped while the GPU still had in-flight work referencing these resources, this could be a use-after-free at the Vulkan level.

**Disposition:** not fixed -- Phase 0/1's single-frame-in-flight model (a fence wait at the start of every `begin_frame`) likely makes this benign in the current control flow, but the inconsistency with `HeadlessSwapchain` is worth resolving explicitly as the synchronization model evolves in Phase 2, rather than relying on it being accidentally safe by construction.

### 52. [Should-fix] `SpscRingBuffer`'s API doesn't statically enforce the single-producer/single-consumer contract its soundness depends on
`push`/`pop` both take `&self`, and the type is `unsafe impl Sync`. Today only one thread ever calls either (this step defers real thread separation), so it's sound in practice, but nothing stops two threads from both calling `.push()` on a shared `Arc<SpscRingBuffer<T>>` -- which would be genuine, unsynchronized UB (not just a logic bug), and Phase 2 is explicitly where a second real thread is expected to appear.

**Disposition:** not fixed now -- recommended fix (split `Producer<T>`/`Consumer<T>` handles from a `split()` constructor, matching `crossbeam`/`ringbuf`'s pattern) is Phase 2 work, since that's when a real second thread and the actual producer/consumer split would exist to design the handle types around.

### 53. [Should-fix] Both platform backends silently swallow connection-level errors in their polling loop
`WaylandConnection::poll_events` discards `connection.flush()`/`dispatch_pending()` errors (`let _ = ...`); `X11Connection::poll_events`'s `while let Ok(Some(event)) = poll_for_event()` silently exits on any `Err`. A live compositor/X-server crash becomes indistinguishable from "no events this frame" -- `poll_events() -> Vec<InputEvent>` has no channel to signal connection death, contradicting the project's own "recoverable failures surface as `Result`" philosophy used everywhere else (e.g. `EngineError`).

**Disposition:** not fixed now -- would require changing `poll_events`'s signature to `Result<Vec<InputEvent>, PlatformError>` (or adding a `connection_lost()` query), rippling through every example. Recorded as a known gap; low practical likelihood in normal dev use, but worth fixing before a real application is built on this layer.

### 54. [Nice-to-have] Two small robustness gaps, low likelihood, not fixed
* `tre-rhi-vulkan/src/lib.rs`'s surface-format selection falls back to `formats[0]`, which panics if a driver ever returns an empty format list, instead of surfacing `EngineError::DeviceLost` the way the rest of the codebase handles device/surface failures.
* `InputEventQueue::push`/`flush_pending_move` silently drop an event when the 256-capacity ring buffer is full -- intentional for `PointerMoved` (documented), but applies uniformly, so a large-enough input burst could in principle drop a `CloseRequested`. Effectively unreachable under normal human/OS input at one drain per frame.

### 55. [Nice-to-have] No dependency vulnerability scanning in CI
This phase added several FFI-heavy, security-relevant dependencies (`wayland-client` with the `system` backend, `x11rb` with `allow-unsafe-code`, `ash`/`ash-window`). CI currently runs `fmt`/`clippy`/`build`/`test` but no `cargo audit`/`cargo deny`. Recommended as a follow-up CI job now that this dependency set exists, so future CVEs are caught automatically rather than only during manual phase reviews.

## Summary table (Phase 1 Review)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 51 | `tre-rhi-vulkan` had ~65 `unsafe` blocks with zero `SAFETY:` comments (pre-existing) | tre-rhi-vulkan (code) | Should-fix | Fixed — 63 comments added (44 + 19), purely additive, verified with validation layers |
| 52 | `SpscRingBuffer` doesn't statically enforce SPSC (both ends take `&self`) | tre-memory (code) | Should-fix | Deferred to Phase 2 — fix requires the real producer/consumer split to design around |
| 53 | Both platform backends silently swallow connection-level errors in `poll_events` | tre-platform (code) | Should-fix | Deferred — needs a `poll_events` signature change rippling through all examples |
| 54 | Driver-empty-format-list panic; input-queue overflow can drop `CloseRequested` | tre-rhi-vulkan, tre-engine (code) | Nice-to-have | Recorded, not fixed — both low-likelihood |
| 55 | No `cargo audit`/`cargo deny` in CI despite new FFI-heavy deps this phase | CI | Nice-to-have | Recommended follow-up, not yet added |
| 56 | `Drop for VulkanSwapchain` skips `device_wait_idle`, unlike `HeadlessSwapchain` | tre-rhi-vulkan (code) | Nice-to-have | Recorded, not fixed — likely benign under Phase 0/1's sync model, revisit in Phase 2 |

---

## Phase 2 Step 1 Implementation (2026-09-05)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 2.2's ring buffer/transient pool -- `tre_engine::RhiDynamicRingBuffer`/`InputEventQueue`-style pool, `tre-rhi-vulkan`'s `VulkanRingBuffer`/`VulkanTexture`, and `RenderingCanvas::push_layer`/`pop_layer`. Full detail in `planning/archive/PLAN_PHASE2_STEP1.md` and `LOG_PHASE2_STEP1.md`; this is the summary for the documentation's own record.

Status: **Complete**, with two scope deviations recorded in IMPLEMENTATION.md's Step 2.2 status section (64-byte thread-boundary padding and `push_layer`'s direct pool-hook, both deliberately deferred). `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace. A new `demo/phase2_step1/` example verified against real hardware with `VK_LAYER_KHRONOS_validation` enabled: zero errors, after fixing two real bugs the validation layer caught along the way (both below).

### 57. [Critical] An initial fence-rotation design broke the existing single-command-buffer examples
Building the ring buffer's "which segment is current" tracking, the first implementation gave `VulkanDevice` 3 separate fences (one per frame-in-flight slot) and rotated which one gated the persistent command buffer's reuse. This is unsound: `VulkanDevice` reuses ONE physical command buffer every frame regardless of which ring-buffer segment is logically current, so waiting on a *different* (trivially already-signaled) fence than the one that command buffer's own last submission actually signaled does not prove the GPU is done with it. Caught immediately on actually running `walking_skeleton`/`multi_window` under `VK_LAYER_KHRONOS_validation`: `VUID-vkResetCommandBuffer-commandBuffer-00045`, `VUID-vkBeginCommandBuffer-commandBuffer-00049`, `VUID-vkQueueSubmit-pCommandBuffers-00071`, and `VUID-vkAcquireNextImageKHR-semaphore-01779` all fired -- not from static analysis, from execution.

**Change:** reverted to a single real fence for command-buffer gating (identical semantics to Phase 0's `in_flight_fence`), and added a SEPARATE, purely informational `AtomicUsize` counter (`FrameSync::frame_index`) that only `VulkanRingBuffer` reads, to pick its current segment. This is sound without its own per-segment fence precisely because the single real fence already fully synchronizes every frame -- by the time the counter cycles back to a given value, at least two other fully-synchronous frames have completed since that segment was last written. Re-verified: all five Vulkan examples (`walking_skeleton`, `multi_window`, `headless`, `input_demo`, the new `memory_pools_demo`) pass with zero validation errors.

### 58. [Should-fix] Textures still checked into the transient pool at teardown were never destroyed
`VulkanDevice` had no logic to destroy pooled (checked-in) `VulkanTexture`s before destroying the device itself. Caught by the validation layer on the new demo's very first run: `VUID-vkDestroyDevice-device-05137`, 6 leaked objects (2 textures' image/view/memory each). A naive fix (just adding the pool as a normal struct field) would have been WORSE, not better: Rust drops a struct's other fields only after an explicit `Drop::drop` body returns, so the pool's own automatic drop would have run its `VulkanTexture`s' destructors AFTER `destroy_device` already executed -- a genuine use-after-free.

**Change:** `Drop for VulkanDevice` now explicitly clears the pool (dropping every pooled `VulkanTexture`, which runs their own correct image/view/memory destruction) BEFORE the existing fence/command-pool/device/instance teardown, fixing both the leak and the ordering hazard a less careful fix would have introduced.

### 59. [Nice-to-have] Two scope deviations from IMPLEMENTATION.md Step 2.2's literal task wording, both deliberate
* Task 3's "64 bytes for CPU thread boundaries" (false-sharing protection) is not implemented -- no multi-threaded canvas writer exists yet to need it (Phase 5's `SubCanvas`). Deferred until a real concurrent writer exists to verify against.
* Task 4's "hook this into `Canvas::push_layer` for immediate zero-allocation acquisition" was not done as literally worded -- `push_layer`/`pop_layer` record IR markers and the balance counter only, never calling `RhiDevice::acquire_transient_target` directly, preserving DESIGN.md Section 2.2's architectural separation (`Canvas` stays backend-agnostic, no RHI device reference). Nothing downstream of `Canvas` consumes a transient target yet (Phase 6's sort/batch/execute pipeline is what would); wiring `push_layer` to the real pool is deferred to whichever phase builds that consumer.

## Summary table (Phase 2 Step 1)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 57 | Rotating fence-per-segment design broke existing single-command-buffer examples | tre-rhi-vulkan (code) | Critical | Fixed — single real fence restored; segment selection uses a separate non-fence counter |
| 58 | Pooled transient textures never destroyed at device teardown (leak + would-be use-after-free) | tre-rhi-vulkan (code) | Should-fix | Fixed — pool explicitly cleared before device/instance destruction |
| 59 | Two scope deviations from Step 2.2's literal task wording (thread-boundary padding, push_layer pool hook) | IMPLEMENTATION | Nice-to-have | Both deliberate and documented, not defects |

---

## Phase 2 Step 2 Implementation (2026-09-05)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 2.4 (Vulkan validation, automatic in debug builds, plus a new CI job that actually exercises it). Full detail in `planning/archive/PLAN_PHASE2_STEP2.md`/`LOG_PHASE2_STEP2.md`; this is the summary for the documentation's own record.

Status: **Complete for Vulkan; DX12/Metal deferred** (neither backend exists). Two real bugs found via actual testing, one in the new feature itself and one a pre-existing, unrelated regression this step's verification work happened to surface.

### 60. [Critical] The debug messenger's error handler hung instead of terminating
The first implementation called `std::process::exit(1)` on an `ERROR`-severity validation message. Verified by deliberately triggering a real validation error (a zero-byte `VkBuffer`, guaranteed `VUID-VkBufferCreateInfo-size-00912`) rather than assumed from reading the docs: the process hung indefinitely instead of exiting, confirmed with a hard `timeout` wrapper (exit code 124 -- killed by timeout, not a clean nonzero exit). Root cause: `exit()` runs registered `atexit` handlers before terminating; the GPU driver's own handler appears to deadlock trying to reacquire a lock the still-on-the-stack Vulkan call that triggered the very callback calling `exit()` is holding.

**Change:** switched to `std::process::abort()`, which raises `SIGABRT` directly and skips `atexit` entirely. Re-verified with the same deliberate trigger: exit code 134 (SIGABRT, core dumped), both via the raw binary and via `cargo run` -- confirmed twice, locally and again in the real CI environment (lavapipe + xvfb) before the fix was accepted.

### 61. [Critical, pre-existing] CI has been failing since Phase 1 Step 1, undetected for three commits
Discovered while verifying this step's new CI job for the first time: `cargo build`/`clippy`/`test` had all been failing on every push since "Phase 1, Step 1: Linux Native Windowing..." (`gh run list` shows `failure` for that commit, the SAFETY-comments fix, and Phase 2 Step 1 -- three consecutive pushes). Root cause: three system dependencies the workspace needs to even compile -- `libwayland-dev` (`wayland-client`'s "system" feature, added Phase 1 Step 1), `libxcb1-dev` (`x11rb`'s XCB FFI, same step), and `glslc` (`tre-rhi-vulkan`'s shader build script, Phase 0) -- were never installed on GitHub's `ubuntu-latest` runners. Every prior step's local verification (build/test/clippy/fmt, all real, all passing) never surfaced this, because it's purely an environment gap specific to the hosted CI runner, not the local dev machine.

**Why this went unnoticed:** after the initial CI-setup work early in the project, no later step's workflow included going back to check `gh run list`/`gh run view` after pushing -- local verification was thorough throughout, but CI's *own* status was never re-checked once it was believed to be working. This step's new job needing `cargo build` to succeed at all is what finally forced a look.

**Change:** `libwayland-dev`, `libxcb1-dev`, and `glslc` added to the `clippy`, `build`, `test`, and new `vulkan-validation` jobs' `apt-get install` steps, as a fix committed separately from this step's actual feature work (a pre-existing, unrelated regression, not something Step 2.4 introduced). Verified via `gh run view` on a scratch branch: all five jobs (`rustfmt`, `clippy`, `build`, `test`, `vulkan-validation`) pass clean.

**Process gap to close going forward:** check `gh run list --branch main --limit 1` after every push that's expected to affect CI, not just when a job is suspected of being broken.

### 62. [Nice-to-have] The new CI gate was proven to actually catch a failure, not just exist
A CI gate that has never been seen to fire is unproven -- code review and "it compiles" don't establish that a validation error genuinely fails the job end-to-end (right package versions, right runtime behavior under a software renderer, right propagation of a Rust process's exit code through `xvfb-run`/`cargo run`/the Actions runner). Verified directly: a deliberate zero-byte buffer was pushed to a scratch branch (`verify/step2-2-ci-gate`), confirmed via `gh run view --log-failed` to produce the exact expected message (`[Vulkan ERROR VALIDATION] ... VUID-VkBufferCreateInfo-size-00912 ...`) and fail the job with exit code 134, then reverted and confirmed the same job passes clean. The scratch branch was deleted after use.

## Summary table (Phase 2 Step 2)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 60 | Debug messenger's `std::process::exit()` hung instead of terminating on an error | tre-rhi-vulkan (code) | Critical | Fixed — switched to `std::process::abort()`, verified via deliberate trigger locally and in CI |
| 61 | CI has been failing since Phase 1 Step 1 (3 commits), undetected — missing system deps | CI | Critical (process) | Fixed — libwayland-dev/libxcb1-dev/glslc installed; process gap noted for future steps |
| 62 | New CI validation gate proven to actually catch a real failure, not just assumed to work | CI | Nice-to-have (process) | Verified via a deliberate, reverted bug on a scratch branch |

---

## Phase 2 Step 2.1 Implementation (2026-09-05)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 2.1's Vulkan bindless texture array (`VK_EXT_descriptor_indexing`) -- `tre_engine::RhiTexture::bindless_index`/`RhiDevice::create_texture`, `tre-rhi-vulkan`'s persistent bindless descriptor set and `VulkanTexture::from_pixels`, and a real `RhiCommandBuffer::bind_texture`. Full detail in `planning/archive/PLAN_PHASE2_STEP2_1.md`/`LOG_PHASE2_STEP2_1.md`; this is the summary for the documentation's own record.

Status: **Complete for Vulkan; DX12/Metal deferred** (neither backend exists, per Phase 2's standing precedent). Two real bugs in the new descriptor-set setup and one design lesson in the new demo, all caught by the validation layer or by pixel-content assertions actually running the code, not by review.

### 63. [Critical] Missing `descriptorBindingSampledImageUpdateAfterBind` feature request
The first implementation requested `VK_EXT_descriptor_indexing`'s general binding flags (`descriptorBindingPartiallyBound`, `descriptorBindingVariableDescriptorCount`, `descriptorBindingUpdateUnusedWhilePending`, `runtimeDescriptorArray`, `shaderSampledImageArrayNonUniformIndexing`) but not the per-descriptor-type feature that actually gates `UPDATE_AFTER_BIND` on a `SAMPLED_IMAGE` binding. Caught on the very first run of the new demo: `vkCreateDescriptorSetLayout` failed with `descriptorBindingSampledImageUpdateAfterBind was not enabled`.

**Change:** added `descriptor_binding_sampled_image_update_after_bind(true)` to the requested feature set.

### 64. [Critical] `VARIABLE_DESCRIPTOR_COUNT` placed on the wrong binding
The initial layout put the unbounded texture array at binding 0 and the fixed immutable sampler at binding 1, matching IMPLEMENTATION.md's prose order ("an unbounded array of textures ... [and] a separate ... shared sampler"). Vulkan requires `VARIABLE_DESCRIPTOR_COUNT` to be on the *highest-numbered* binding in the set, unconditionally -- caught on the second run: `vkCreateDescriptorSetLayout` failed with exactly that message, naming binding 0.

**Change:** swapped binding numbers (sampler at 0, texture array at 1) on both the Rust side (layout, pool sizes, the `vkUpdateDescriptorSets` write's `dst_binding`) and the GLSL side (`bindless_textured.frag`'s `layout(set = 0, binding = ...)` declarations) together. Note the two sides aren't checked against each other by the compiler or the validation layer -- a mismatch here would have been a silent wrong-texture-sampled bug, not a caught error, which is exactly why the new demo asserts actual output pixel colors rather than just checking for a clean exit.

### 65. [Nice-to-have] A design lesson from the demo's own first draft, not an RHI defect
The demo originally proved its "no texture bound" fallback by simply never calling `bind_texture` for the fourth draw, assuming that meant "sentinel." `bind_texture`'s bound index is ordinary command-buffer state that persists across draws until explicitly changed -- exactly like the pipeline, vertex buffer, or scissor rect already do -- so skipping the call after already binding `blue` for the previous draw left `blue` still bound. The resulting quad silently rendered the wrong (but plausible-looking) color; caught immediately by the pixel-color assertion, not by any crash.

**Change:** no RHI code changed (the behavior is correct and intentional). The demo now explicitly rebinds the sentinel (`bind_texture(0, u32::MAX)`) before its fourth draw.

## Summary table (Phase 2 Step 2.1)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 63 | Missing `descriptorBindingSampledImageUpdateAfterBind` feature request | tre-rhi-vulkan (code) | Critical | Fixed — feature added, verified via validation layer |
| 64 | `VARIABLE_DESCRIPTOR_COUNT` placed on a non-highest-numbered binding | tre-rhi-vulkan (code + shader) | Critical | Fixed — bindings swapped (sampler 0, array 1) on both Rust and GLSL sides |
| 65 | Demo assumed skipping `bind_texture` resets to "no texture"; it doesn't (persistent state) | tre-rhi-vulkan (example) | Nice-to-have | Fixed in the demo; not an RHI defect — caught by pixel-content assertion |

---

## Phase 2 Code Review (2026-09-05)

Reviewer: two independent Claude sub-agents (Rust correctness, security), per project standing instructions -- the phase-level review due once Phase 2's steps (2.1, 2.2, 2.4) were complete, mirroring the identical two-agent review done after Phase 1. Scope: everything in commits `32482a6..5e8fee1` (Phase 2 Steps 1/2/2.1 and the CI dependency fix). Findings already recorded and fixed during development (#57-65 above) were explicitly excluded from both agents' scope.

Both agents independently found the same critical gap (#66/#67) from different angles -- one reasoning from "what if a caller passes bad input," the other from "what does this code's own comments promise" -- which is a strong signal it's real, not a false positive from either framing.

**Scope decision (confirmed with the project owner):** fix everything, including #71/#72 -- the two findings that most resemble the "defer until a real second consumer/thread exists" pattern already used for #52/#56. Unlike those, #71/#72 were cheap and self-contained to close immediately rather than genuinely requiring a not-yet-built consumer to design against.

### 66. [Critical] `create_texture` never validates `pixels.len()` against `width`/`height`/`format`
`VulkanTexture::from_pixels` sizes the staging buffer from `pixels.len()` (via the existing `upload_buffer` helper) but sizes the subsequent `vkCmdCopyBufferToImage` region purely from `width`/`height`/`format`, independent of the actual staging-buffer size. A `pixels` slice shorter than `width * height * bytes_per_pixel(format)` implies -- including the trivial empty-slice case -- creates an undersized staging buffer and then instructs the GPU to read past its end: a genuine out-of-bounds read at the driver level. A zero-length slice hits the same `VUID-VkBufferCreateInfo-size-00912` condition finding #60 already demonstrated triggers the debug messenger's `std::process::abort()` -- so in debug/CI builds this is a reliable one-call process abort; in release builds (no validation layer) it's undefined driver behavior instead. Since `create_texture` is architecturally intended to eventually be reachable across the `tre-ffi` C-ABI boundary, this is a real trust-boundary gap, not just an internal-demo footgun.

**Change:** `VulkanTexture::from_pixels` now rejects `width == 0 || height == 0` and any `pixels.len()` that doesn't exactly equal `width * height * bytes_per_pixel(format)` (a new `bytes_per_pixel` helper) with `Err(EngineError::InvalidTextureData)`, checked before any GPU call. Verified with a deliberate empty-buffer call: correctly returns `Err(InvalidTextureData)` with zero validation-layer errors (previously this exact input triggered `VUID-VkBufferCreateInfo-size-00912` and `abort()`).

### 67. [Critical] `RhiDevice::create_texture`'s infallible signature converts recoverable failures into panics
`BindlessRegistry::allocate`'s own doc comment calls array exhaustion "a real, reportable condition, not something to paper over," and `VulkanTexture::from_pixels` does correctly return `Err(EngineError::DeviceLost)` for it. But the `RhiDevice::create_texture` trait signature returns a bare `Box<dyn RhiTexture>`, not a `Result` -- matching neither `begin_frame`/`submit_and_present`'s existing fallible pattern nor DESIGN.md Section 2.6's explicit failure-mode principle ("atlas exhaustion beyond LRU capacity" is literally one of that section's five named failure classes). The Vulkan backend's only option is `.expect(...)`, turning both bindless-array exhaustion and finding #66's bad-input case into an unconditional panic instead of a caller-recoverable error.

**Change:** `RhiDevice::create_texture` now returns `Result<Box<dyn RhiTexture>, EngineError>`. Added `EngineError::InvalidTextureData` (finding #66) and `EngineError::BindlessArrayExhausted` (a dedicated variant, not overloading `DeviceLost`) -- `VulkanTexture::from_pixels`'s exhaustion path now maps to the latter. `bindless_textures_demo`'s three real call sites updated to `.expect(...)` the result (a demo, not exercising the error path itself).

### 68. [Should-fix] `from_pixels` leaks GPU resources on every error path
`image`/`view`/`memory` (and, for earlier failures, the temporary upload command buffer/fence) are plain `vk::*` handles with no drop guard local to the function. Every fallible call after they're created -- including finding #67's exhaustion check, which runs *after* the image is fully uploaded -- returns `Err` via `?` without destroying them. Currently unreachable in practice (nothing calls `create_texture` in a retry loop yet), but becomes a real, repeatable leak the moment finding #67 is fixed and callers can legitimately retry after a failure.

**Change:** two small RAII guards, `PendingImage` (holds `image`/`memory: Option<_>`/`view: Option<_>`, destroys whichever exist on early drop) and `PendingCommandBuffer` (frees the command buffer on early drop), now wrap `from_pixels`'s fallible middle section; each is deliberately released via an explicit `into_parts`/`into_inner` call only once nothing further can fail. The upload fence's own narrow failure window (submit/wait) is left unguarded -- documented inline as an accepted scope boundary, since a submit/wait failure is itself an effectively unrecoverable device-lost condition where a leaked fence handle is moot.

### 69. [Should-fix] `bind_texture` accepts an unchecked, unbounded `u32` index
The only check present is `debug_assert_eq!(slot, 0, ...)` (compiled out in release, and checks the wrong parameter). Nothing validates `bindless_index` is either the sentinel or `< bindless_capacity` (the real, runtime-clamped array size, potentially well below the 4,096 ceiling on constrained devices). `PARTIALLY_BOUND`/`UPDATE_AFTER_BIND` make *declared-but-unused* slots safe to skip -- they do not extend well-defined behavior past the actually-allocated `VARIABLE_DESCRIPTOR_COUNT`. Because the index is a fully dynamic per-draw value, the validation layer as currently configured (no GPU-assisted validation) cannot catch an out-of-range value; the failure mode is driver-defined (garbage sample, GPU page fault, or worse).

**Change:** `VulkanCommandBuffer` now carries `bindless_capacity` (populated from `VulkanDevice::bindless_capacity`, cached at construction to avoid locking `bindless_registry`). `bind_texture` validates `bindless_index == BINDLESS_TEXTURE_SENTINEL || bindless_index < bindless_capacity`, loudly via `debug_assert!` in debug builds and falling back to the safe sentinel in release builds instead of passing an out-of-range value through to the GPU.

### 70. [Should-fix] `release_transient_target` has no guard against a bindless (`create_texture`) texture being passed to it by mistake
Both agents flagged this independently. `release_transient_target` unconditionally reconstructs a `VulkanTexture` with `bindless_index: None, bindless_registry: None` from whatever `Box<dyn RhiTexture>` it receives -- nothing in the trait boundary distinguishes a texture that came from `acquire_transient_target` (never bindless) from one that came from `create_texture` (usually bindless). If a caller mixes them up, the real bindless slot is never returned to `BindlessRegistry`'s free list (permanently stranded), and the texture is checked into the transient pool despite having `SAMPLED | TRANSFER_DST` usage rather than `COLOR_ATTACHMENT` -- silent corruption, not a caught error.

**Change:** `release_transient_target` now checks `texture.bindless_index().is_some()` first; if true (misuse), it loudly `debug_assert!`s and lets `texture` drop normally instead of reconstructing it -- `VulkanTexture`'s own `Drop` correctly destroys the GPU resources AND releases the bindless slot, which is the right behavior in both debug and release builds, not just a debug-only diagnostic.

### 71. [Should-fix] `Drop for VulkanDevice` still has no `vkDeviceWaitIdle`, and this phase substantially raised the stakes
Finding #56 (Phase 1 Review) already flagged the identical gap in `VulkanSwapchain`'s `Drop` and deferred it "as the synchronization model evolves in Phase 2." `VulkanDevice::Drop` has the same gap and was directly touched by both Step 2.2 (added the transient-pool clear) and Step 2.1 (added the whole bindless descriptor apparatus's teardown) without closing it -- so `Drop` now unconditionally destroys substantially more live GPU state with no wait for the GPU to finish the last submitted frame. It currently "works" only because every windowed example happens to call `device_wait_idle()` manually at the end of `main()` first -- a convention, not a guarantee.

**Change:** `Drop for VulkanDevice` now calls `device_wait_idle()` unconditionally as its first action, ignoring the result (a failure here means the device is already lost, so there is nothing further to usefully wait for, and panicking inside `Drop` is itself undesirable). `VulkanSwapchain`'s identical gap (finding #56) remains open -- not touched by this phase's work, so left for whichever future step actually revisits it.

### 72. [Should-fix] The shared `command_pool`/persistent `command_buffer` has no synchronization guard, unlike `transient_pool`/`bindless_registry`
Step 2.2 and Step 2.1 each deliberately `Mutex`- (and `Arc<Mutex<_>>`-) wrapped their own new shared state specifically "so `VulkanDevice` stays genuinely `Sync`-shareable across threads later," per their own doc comments. But `VulkanTexture::from_pixels` allocates/frees a command buffer from the *same* `vk::CommandPool` the main render loop resets/begins/ends every frame, with no exclusion guarding the pool itself -- and Vulkan's host-synchronization rules require external synchronization on a command pool shared this way. Not yet triggered (every demo calls all its `create_texture`s before its first `begin_frame`, single-threaded), but a real gap in an otherwise carefully-reasoned forward-looking design, in the same spirit as the already-deferred finding #52.

**Change:** `create_texture`'s upload path no longer touches the frame loop's `command_pool` at all -- `VulkanDevice` gained a second, dedicated `upload_command_pool: Mutex<vk::CommandPool>` (created `TRANSIENT`, since every buffer from it is recorded once and freed immediately), fully eliminating the shared-resource hazard rather than merely serializing access to the existing one. Concurrent `create_texture` calls from multiple threads now serialize safely against each other via the `Mutex`, with zero interaction with frame submission.

### 73. [Should-fix] `next_power_of_two()` on caller-supplied `width`/`height` panics/wraps above `2^31 - 1`
Both `acquire_transient_target` and `release_transient_target` call `.next_power_of_two()` directly on caller-supplied `u32` values with no bounds check first. Per its documented behavior, an input above `2^31 - 1` panics in debug builds and silently wraps to `0` in release builds. Unrealistic for a real render-target request, but a zero-cost fix and directly on-theme with finding #66's integer-overflow question.

**Change:** both call sites now clamp with `.min(1 << 30)` before rounding up -- a no-op for every realistic texture request (`1 << 30` is already a power of two, so clamping to it can never itself overflow) and unconditionally safe in both debug and release builds.

### 74. [Nice-to-have] `Drop for VulkanDevice` silently swallows a poisoned `transient_pool` mutex
Every other lock site in the file panics via `.expect("... poisoned")` on a poisoned mutex; `Drop`'s `if let Ok(mut pool) = self.transient_pool.lock() { ... }` silently does nothing instead. Low likelihood (requires a prior panic while holding the lock) but worth either a comment explaining the deliberate divergence (likely: avoiding a double-panic during unwind) or aligning it with the file's own convention.

**Change:** documented as deliberate rather than changed to `.expect()` -- panicking inside `Drop` during an unwind already in progress would abort the process instead of completing that unwind, which is worse than skipping this one cleanup step. Changing behavior here would have been a regression dressed up as a fix.

### 75. [Nice-to-have] `bind_texture`'s `slot != 0` contract is `debug_assert!`-only
A caller passing `slot: 1` gets no diagnostic in a release build and silently overwrites the same `texture_index` binding 0 uses -- consistent with this codebase's existing `debug_assert!` conventions elsewhere, so low priority, but worth tracking as the `bind_texture` API surface grows (e.g. a future second bindless array).

**Change:** `bind_texture` now early-returns (a release-safe no-op) when `slot != 0`, in addition to the existing `debug_assert_eq!`, fixed together with finding #69 in the same function.

### 76. [Nice-to-have] CI's new `apt-get install` steps are unpinned
`libwayland-dev`/`libxcb1-dev`/`glslc`/`libvulkan1`/`mesa-vulkan-drivers`/`vulkan-validationlayers`/`xvfb` all install without version pins. Low risk (GitHub's own `ubuntu-latest` image, official mirrors, no curl-pipe-to-shell) -- only worth addressing if build reproducibility over time becomes a stated goal.

**Disposition:** left unpinned. This local dev machine is Arch/CachyOS, not Ubuntu, so there is no way to determine correct, currently-valid Ubuntu package version strings from this environment without guessing -- and a wrong guess would break CI a third time this project (see findings #45/#61's own precedent for exactly that class of mistake). Not fixed; the finding's own text already frames this as optional.

## Summary table (Phase 2 Code Review)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 66 | `create_texture` doesn't validate `pixels.len()` vs. `width`/`height`/`format` — OOB GPU read, guaranteed abort on empty input | tre-rhi-vulkan (code) | Critical | Fixed — validated before any GPU call, verified via deliberate-bug proof |
| 67 | `RhiDevice::create_texture`'s infallible signature turns recoverable failures into panics | tre-engine / tre-rhi-vulkan (code) | Critical | Fixed — `create_texture` now returns `Result`, two new `EngineError` variants |
| 68 | `from_pixels` leaks GPU resources on every error path | tre-rhi-vulkan (code) | Should-fix | Fixed — `PendingImage`/`PendingCommandBuffer` RAII guards |
| 69 | `bind_texture` accepts an unchecked, unbounded bindless index | tre-rhi-vulkan (code) | Should-fix | Fixed — bounds-checked, safe sentinel fallback in release builds |
| 70 | `release_transient_target` has no guard against a bindless texture passed by mistake | tre-rhi-vulkan (code) | Should-fix | Fixed — misuse detected, texture dropped correctly instead of pooled |
| 71 | `Drop for VulkanDevice` still has no `vkDeviceWaitIdle` (echoes deferred #56) | tre-rhi-vulkan (code) | Should-fix | Fixed — unconditional `device_wait_idle()` added; `VulkanSwapchain`'s #56 still open |
| 72 | Shared `command_pool` has no synchronization guard, unlike this phase's other shared state | tre-rhi-vulkan (code) | Should-fix | Fixed — dedicated `Mutex`-guarded `upload_command_pool`, decoupled from the frame loop |
| 73 | `next_power_of_two()` panics/wraps above `2^31 - 1` with no upstream check | tre-rhi-vulkan (code) | Should-fix | Fixed — inputs clamped to `1 << 30` before rounding up |
| 74 | `Drop for VulkanDevice` silently swallows a poisoned mutex, unlike the rest of the file | tre-rhi-vulkan (code) | Nice-to-have | Documented as deliberate (avoids abort-during-unwind), not changed |
| 75 | `bind_texture`'s `slot != 0` contract is `debug_assert!`-only | tre-rhi-vulkan (code) | Nice-to-have | Fixed — release-safe no-op added alongside the existing debug assertion |
| 76 | CI's new `apt-get install` steps are unpinned | CI | Nice-to-have | Not fixed — no safe way to determine correct pins from this (non-Ubuntu) environment |

---

## Phase 2 Step 2.3 Implementation (2026-09-06)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 2.3 (generational GC), verified against the transient render-target pool rather than the not-yet-built atlas/SVG cache the step's literal wording targets. Full detail in `planning/archive/PLAN_PHASE2_STEP2_3.md`/`LOG_PHASE2_STEP2_3.md`; this is the summary for the documentation's own record.

Status: **Complete.** This step also introduces the engine's first genuine background OS thread -- both scope questions (target resource, threading model) were explicitly put to the project owner before implementation, since both were real forks from every prior step's precedent (deferring what doesn't have a real consumer yet; staying single-threaded until a real second thread is needed). The owner chose to build the real mechanism now and to build a genuine thread rather than defer either.

### 77. [Nice-to-have] A real, explained interaction between the GC eviction count and Step 2.2's pool-growth queuing
`gc_demo` checks 25 distinct transient-target sizes into the pool, but `transient_pool_stats()` reports 50 evictions once the GC thread runs, not 25. Root cause: `acquire_transient_target`'s cold-miss path (Step 2.2) both cold-allocates a texture to return immediately AND queues that same bucket into `pending_growth` for the next frame's `grow_pending_transient_targets` to *also* allocate. The demo's "acquire, immediately release, never re-request that size" access pattern is exactly the pattern that never lets the queued growth serve any purpose -- every one of the 25 sizes ends up with a duplicate, equally-idle texture in the pool. Confirmed real (not a GC bug) by inspecting `acquire_transient_target`'s existing code; the GC thread evicted exactly what was genuinely stale.

**Disposition:** not fixed -- this is a Step 2.2 pool-efficiency question (should a bucket just cold-allocated for also be queued for growth?), not a Step 2.3 correctness one. Recorded for whichever future step next touches `acquire_transient_target`.

## Summary table (Phase 2 Step 2.3)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 77 | 25 checked-in sizes evict as 50 -- a real, explained interaction with Step 2.2's pool-growth queuing | tre-rhi-vulkan (code) | Nice-to-have | Not fixed — a Step 2.2 pool-efficiency question, out of this step's scope |

---

## Phase 2 Step 2.3 Code Review (2026-09-06)

Reviewer: two independent Claude sub-agents (Rust correctness, security), per project standing instructions -- a second phase-level review pass, this time requested specifically for Step 2.3 rather than all of Phase 2 (already covered by the "Phase 2 Code Review" section above, findings #66-76). Scope: exactly commit `de7fb8d` ("Phase 2 Step 2.3: generational GC via a real background thread"). Finding #77 above was explicitly excluded from both agents' scope.

Both agents independently found the same critical bug (#78) -- the strongest possible signal it's real, matching the pattern that first surfaced findings #66/#67 in the earlier Phase 2 review. (One agent also reported, and correctly disregarded, an injected "system-reminder" mid-review attempting to redirect it with unrelated tool instructions -- noted here for the record, not a finding about this codebase.)

### 78. [Critical] `deferred_release` queue is never cleared before `destroy_device` in `Drop for VulkanDevice` -- a real use-after-destroy on shutdown
`Drop for VulkanDevice` explicitly clears `transient_pool`'s free list before destroying the device, with a comment recording exactly why: Rust drops a struct's fields (in declaration order) only *after* the explicit `Drop::drop` body finishes, so leaving pooled `VulkanTexture`s to drop automatically would run their real `vkDestroy*` calls after `destroy_device` already ran -- the same "6 leaked objects" class of bug Step 2.2 first found and fixed this way. That fix was never extended to `deferred_release`, a field this very commit added. `Arc<Mutex<VecDeque<DeferredRelease>>>`'s own implicit drop does real work -- it drops every remaining `DeferredRelease`, which drops its `VulkanTexture`, calling `destroy_image_view`/`destroy_image`/`free_memory` against a `VkDevice` (and by then `VkInstance`) that's already destroyed.

Reachable in ordinary, non-panicking shutdown: any time the GC thread has evicted something but the app closes (or simply stops calling `begin_frame`) within the next few frames, before that entry's 3-frame grace period elapses. `gc_demo` never exposed this because its loop breaks the instant `stats.destroyed > 0`, and because finding #77's duplicate-eviction quirk means all ~50 entries share one `evicted_at_frame`, the very next successful drain happens to empty the whole queue before the demo exits -- the test's own timing coincidentally masked a real bug, not proof the bug doesn't exist.

**Change:** `Drop for VulkanDevice` now clears `deferred_release` (`if let Ok(mut queue) = self.deferred_release.lock() { queue.clear(); }`) immediately alongside the existing `transient_pool.free.clear()`, both before `destroy_device` -- ignoring the grace period entirely at this point, since `device_wait_idle()` (already called earlier in `Drop`) makes it moot.

### 79. [Critical] A GC-thread panic silently poisons the shared `transient_pool` mutex, cascading into main-thread panics with no diagnostic trail
`gc_thread_loop` and the main thread share the same `Mutex<TransientPool>`. If the GC thread ever panics while holding it (a thread panic in Rust doesn't crash the process by default -- it's caught at the thread boundary), every subsequent main-thread `.lock().expect("transient pool poisoned")` call -- i.e. every future `acquire_transient_target`/`release_transient_target`/`begin_frame` -- panics too, with no link back to the real root cause: the GC thread died invisibly first. `Drop for VulkanDevice`'s `let _ = handle.join();` discards the panic payload entirely, so even at final teardown there's no trace the background thread ever failed. The one realistic trigger identified: `pool.total_free_bytes -= evicted.iter().map(...).sum::<u64>()` is an unchecked subtraction with no invariant check anywhere in the five call sites that maintain `total_free_bytes` by hand -- a future accounting bug there underflows-and-panics in debug (poisoning the mutex per the above) or silently wraps to near-`u64::MAX` in release (permanently pinning the pool above the GC trigger, evicting everything eligible on every scan forever, silently).

**Change:** all five `total_free_bytes` mutation sites now use `saturating_sub`/an explicit non-panicking computation instead of a bare `-=`, so a future accounting bug degrades to a bounded-but-wrong value rather than a panic-and-poison or a release-mode wraparound. `Drop for VulkanDevice` now downcasts and prints the GC thread's panic payload (`&str`/`String`, the common cases) via `eprintln!` instead of silently discarding `handle.join()`'s `Err`, so a failure leaves at least one trace even at final teardown.

### 80. [Should-fix] The GC is reclaim-only, not an enforced cap -- despite budget-sounding constants, nothing gates admission of new pool growth
`GC_TRIGGER_THRESHOLD_BYTES`/`DYNAMIC_VRAM_BUDGET_BYTES` read like enforced limits, but neither `acquire_transient_target`'s cold-miss path nor `grow_pending_transient_targets` checks them before allocating -- the GC thread only ever reclaims idle (600+ frame) entries; it cannot claw back VRAM from a caller that keeps enough distinct buckets in active rotation to never go idle. Since these entry points are architecturally intended to eventually be reachable across the `tre-ffi` C-ABI boundary, this could easily be mistaken for real enforcement when it isn't.

**Change:** `RhiDevice::acquire_transient_target` now returns `Result<Box<dyn RhiTexture>, EngineError>` (a new `EngineError::TransientPoolBudgetExceeded` variant). The cold-allocate path -- the one place that requests genuinely new GPU memory, not a reuse of already-idle bytes -- now checks `pool.total_free_bytes` against the full `DYNAMIC_VRAM_BUDGET_BYTES` before allocating and returns the new error instead of proceeding unconditionally. Documented as a real but imperfect gate: it compares against idle free-list bytes only, not total bytes including whatever's currently checked out, so it catches "many distinct sizes cycling through mostly idle" but not "many sizes permanently checked out simultaneously" -- an honest limitation, not a claim of exact enforcement. `gc_demo` and `memory_pools_demo`'s three call sites updated for the new `Result`; `gc_demo` specifically now treats hitting the cap as an expected, graceful stopping point (it already needs to exceed the pool's 85% GC-trigger threshold well before reaching the 100% admission cap to prove eviction, and verified it does: 22 of its 25 candidate sizes are admitted before the cap stops it, ~128 MB checked in).

### 81. [Should-fix] The GC thread holds `transient_pool`'s lock across a full, unbounded free-list scan, contending with the main thread's per-frame pool calls
Once triggered, the scan evicts every eligible entry in one pass while holding the single mutex `acquire_transient_target`/`release_transient_target` also need on the render thread. Cost is O(entries scanned), not O(entries evicted); as the free list grows (more distinct sizes, or a future atlas/SVG cache sharing this same pool) a scan landing mid-frame could stall the render thread for the scan's full duration -- in tension with this project's deterministic per-frame cost goal. The same held lock also means `Drop`'s shutdown latency is bounded by `GC_SCAN_INTERVAL` *plus one full scan*, not just the scan interval alone.

**Change:** added `GC_MAX_EVICTIONS_PER_SCAN` (64), a throughput cap, not a "stop once under budget" one -- a scan now evicts at most that many entries per wake-up (`break`ing out of the scan early once reached) and picks up any remaining backlog on the next `GC_SCAN_INTERVAL`-spaced wake. Bounds both the render thread's worst-case contention on `transient_pool` and `Drop`'s shutdown latency to a small, constant amount of work regardless of total pool size, at the cost of a large backlog draining over a few extra 100ms cycles instead of one -- harmless, since eviction was never time-critical.

### 82. [Nice-to-have] Evicted buckets leave empty `Vec` entries as live `HashMap` keys forever
Harmless at today's small, bounded key space; worth a `pool.free.retain(|_, v| !v.is_empty())` after the eviction loop if this pool is later reused for a larger, more varied key space (the atlas/SVG cache Step 2.3's literal wording targets).

**Change:** added -- `pool.free.retain(|_, textures| !textures.is_empty())` runs once per scan that evicted anything, immediately after the eviction loop.

### 83. [Nice-to-have] `total_frame_count`'s `Acquire`/`Release` orderings are stronger than the actual contract needs
Verified: no underflow/double-count risk exists in `total_free_bytes` bookkeeping (every subtraction site pairs 1:1 with the addition that put a texture in the pool, and all pool/byte-count access already happens under `transient_pool`'s mutex, which supplies the real happens-before edges). Given that, `total_frame_count`'s own `Acquire`/`Release` is more conservative than needed -- `Relaxed` would suffice for a single monotonically-increasing counter with no other data being published through it.

**Disposition:** correct as written, not a defect -- `Acquire`/`Release` costs nothing measurable at this call frequency; not worth the risk of a future misreading if "just make it Relaxed" is applied somewhere the ordering *does* matter. No change planned regardless of the scope decision below.

## Summary table (Phase 2 Step 2.3 Code Review)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 78 | `deferred_release` never cleared before `destroy_device` -- real use-after-destroy on shutdown | tre-rhi-vulkan (code) | Critical | Fixed — cleared alongside `transient_pool` before device teardown |
| 79 | GC-thread panic poisons the shared mutex, cascading into main-thread panics with no diagnostic trail | tre-rhi-vulkan (code) | Critical | Fixed — `saturating_sub` throughout; `join()` panic now logged |
| 80 | GC is reclaim-only, not an enforced cap, despite budget-sounding constant names | tre-rhi-vulkan (code) | Should-fix | Fixed — `acquire_transient_target` returns `Result`, real (if imperfect) admission cap |
| 81 | GC thread holds the pool lock across a full, unbounded scan, contending with the render thread | tre-rhi-vulkan (code) | Should-fix | Fixed — `GC_MAX_EVICTIONS_PER_SCAN` throughput cap |
| 82 | Evicted buckets leave empty `Vec` entries as live `HashMap` keys forever | tre-rhi-vulkan (code) | Nice-to-have | Fixed — `retain` after the eviction loop |
| 83 | `total_frame_count`'s atomic orderings are stronger than strictly needed | tre-rhi-vulkan (code) | Nice-to-have | Not a defect — no change planned |

---

## Phase 3 Step 3.1 Implementation (2026-09-06)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 3.1's remaining task (SIMD affine matrix math) in the previously-empty `tre-math` crate. Full detail in `planning/archive/PLAN_PHASE3_STEP3_1.md`/`LOG_PHASE3_STEP3_1.md`; this is the summary for the documentation's own record.

Status: **Complete.** Worth recording plainly: this is the first step in the project with no real bug found by actually running the code -- every prior GPU-facing step (Phases 0-2) surfaced at least one genuine runtime issue via the Vulkan validation layer or a deliberate test. `tre-math`'s pure-CPU, `forbid(unsafe_code)`, no-FFI surface area gave the compiler and unit tests a much better chance of catching problems before "running" was even a separate step -- the closest thing to a finding here is three `clippy::pedantic` false positives (a `similar_names` flag on `tx`/`ty`-derived bindings that are this codebase's own established field names, a `doc_markdown` flag on a LaTeX-style doc comment now rewritten in plain backticked code style, and a `float_cmp` flag on tests whose inputs involve no rounding at all) -- caught and fixed at compile time, not given numbered findings, consistent with this project's standing practice of reserving REVIEW.md findings for issues an actual run (or a real review pass) surfaces.

All 11 unit tests passed on the first run, including the SIMD-vs-scalar-reference comparison across every remainder length relative to the 8-wide chunk size (`0, 1, 7, 8, 9, 16, 17`). `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace.

---

## Phase 3 Step 3.2 Implementation (2026-09-06)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 3.2 (analytical SDF rounded rectangles) -- a new `sdf_rounded_rect.{vert,frag}` shader pair, `RenderingCanvas::draw_rounded_rect`'s real `radius` parameter, and the vertex-attribute wiring both need. Full detail in `planning/archive/PLAN_PHASE3_STEP3_2.md`/`LOG_PHASE3_STEP3_2.md`; this is the summary for the documentation's own record.

Status: **Complete.**

### 84. [Should-fix] `UiVertex::params` had existed since Phase 0 but was never wired as a shader-readable vertex attribute
`VulkanDevice::create_pipeline`'s vertex attribute descriptions had only ever declared `position`/`uv`/`color` (locations 0-2) across every pipeline built so far -- `params` has been present in every vertex buffer uploaded since Phase 0's walking skeleton, but no shader could ever read it, silently. This step's shader is the first to actually need it. Found by direct code inspection while scoping this step's blast radius, not by a failure at runtime (no prior shader declared a `location = 3` input, so nothing was ever wrong for existing code -- the gap was latent, not a live bug).

**Change:** added location 3 (`R32G32B32_SFLOAT`, offset 20) to the one universal pipeline layout every pipeline gets, matching the existing precedent for the bindless descriptor set and push-constant range (declared everywhere; unused by shaders that don't reference it). Re-verified all 7 pre-existing examples under `VK_LAYER_KHRONOS_validation` after the change -- zero errors, only the expected benign `pVertexInputState` performance warning that older shaders don't consume the new input.

### 85. [Nice-to-have] A perfectly pixel-aligned flat edge produces no fractional-coverage AA sample -- a real property of this technique, not a bug
Discovered while writing this step's own verification demo: an initial version scanned pixels along the rect's flat left edge (placed at an exact integer canvas coordinate) looking for a genuinely blended pixel, and found none -- every pixel was exactly foreground or exactly background, with a hard transition between adjacent pixels. Root cause, confirmed by hand-computing the SDF at the relevant pixel centers: `fwidth(d)` on a flat, axis-aligned edge is exactly 1 pixel, so the entire analytical AA ramp (`d` in `[-0.5, 0.5]`) falls exactly between two pixel centers (at the standard half-integer sample offsets) whenever the true edge sits on an integer coordinate -- both bracketing samples land exactly on the ramp's clamp boundaries, so neither one ever samples the ramp's interior. This is an inherent property of evaluating a 1-pixel analytical AA band at pixel centers, not a defect in the shader's math or in `fwidth`'s hardware derivative.

**Disposition:** not a defect -- no code change. The demo's AA-band assertion was rewritten to scan pixels around a rounded corner's arc instead, whose non-axis-aligned gradient has no such alignment and reliably produces several genuinely partial-alpha pixels; this is also the more representative check anyway, since proving the rounding itself (not a flat edge, which the old flat-color shader already rendered correctly) is this step's actual goal. Recorded here so a future reader investigating an apparently "AA not working" report on a flat edge finds this explanation rather than re-diagnosing it from scratch.

## Summary table (Phase 3 Step 3.2)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 84 | `UiVertex::params` never wired as a shader-readable vertex attribute since Phase 0 | tre-rhi-vulkan (code) | Should-fix | Fixed — added location 3 to the universal pipeline layout |
| 85 | A pixel-aligned flat edge samples no fractional AA coverage -- a real property of this technique | tre-rhi-vulkan (demo/docs) | Nice-to-have | Not a defect — demo's AA check moved to the rounded corner instead |

All 7 pre-existing Vulkan examples (`walking_skeleton`, `multi_window`, `headless`, `input_demo`, `memory_pools_demo`, `bindless_textures_demo`, `gc_demo`) re-run manually under `VK_LAYER_KHRONOS_validation`, zero errors. New `sdf_rounded_rect_demo` verified end to end: exact foreground at the interior, exact background outside the rounding arc, and a genuine partial-alpha blend confirmed near the rounded corner. `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace, including 3 new `tre-engine` unit tests.

---

## Phase 3 Step 3.3.1 Implementation (2026-09-06)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 3.3.1 (SVG ingestion via `usvg` + ear-clipping tessellation of simple polygons), the first of the sub-steps IMPLEMENTATION.md Step 3.3 was split into (see IMPLEMENTATION.md's own "Scope decision" note under Step 3.3, and `planning/archive/PLAN_PHASE3_STEP3_3_1.md`). Full detail in `planning/archive/LOG_PHASE3_STEP3_3_1.md`; this is the summary for the documentation's own record.

Status: **Complete.** Both findings below were caught by the same mechanism: writing a real, non-convex verification demo (a five-pointed star) rather than trusting unit tests built only against convex or already-passing shapes. Both are also a real, useful lesson in test design, recorded in each finding's own disposition.

### 86. [Critical] Ear-clipping returned triangle indices valid against an internal working copy, not the caller's own point array
`triangulate`'s first implementation deduplicated and (when the input polygon's original winding needed correcting) reversed its working copy of the polygon's points in place, then emitted triangle indices directly from that working copy's own numbering. Every caller -- including `to_ui_vertices`, which builds the actual GPU vertex/index buffers -- expects indices valid against the ORIGINAL `polygon.points` array the function was called with. Whenever reversal happened (any polygon whose original point order came out negative/"clockwise" under the shoelace formula), the returned indices silently named the wrong physical points, producing a corrupted mesh with no error, warning, or panic anywhere.

Not caught by the square/L-shape unit tests written first, because neither of their point orderings happened to trigger reversal. Confirmed real by direct trace (temporary `eprintln!`s) cross-checked against an independent Python ray-casting point-in-polygon reference implementation.

**Change:** track a parallel `original_index: Vec<u32>` array through the exact same deduplication and reversal operations as the working `points` copy, and translate every emitted triangle through it before returning.

### 87. [Critical] Ear-validity check needed BOTH "no vertex inside" and "no edge crosses", not either alone
After fixing #86, the star still rendered wrong -- one of its concave notches was incorrectly filled by a triangle. The specific accepted "ear" covered a real, remaining polygon vertex (the concave-notch point) whose own two edges each terminated exactly at one of the ear triangle's own corners -- meaning neither edge ever registered as a "proper crossing" of the ear's diagonal (a segment sharing an endpoint with another segment it also happens to run through cannot satisfy a strict-inequality proper-intersection test), even though the vertex itself was genuinely, strictly inside the triangle.

This is the exact mirror image of the L-shape bug this step's *first* ear-clipping attempt (using only a vertex-inside-triangle check) had already found and fixed by switching to an edge-crossing check instead of replacing it: that earlier case was a vertex sitting exactly on an edge (not caught by "inside" alone) while an edge through it still crossed the boundary; this case is a vertex fully inside a triangle while its own edges never "properly cross" anything (not caught by edge-crossing alone). Each check has a real, independent blind spot.

**Change:** both checks now run together -- an ear is valid only if no remaining vertex is strictly inside the candidate triangle AND no remaining edge properly crosses its diagonal.

## Summary table (Phase 3 Step 3.3.1)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 86 | Ear-clipping returned indices valid against an internal (possibly-reversed) working copy, not the caller's array | tre-svg (code) | Critical | Fixed — explicit `original_index` remapping threaded through dedup/reversal |
| 87 | Ear-validity check needs both "no vertex inside" and "no edge crosses" -- either alone has a real blind spot | tre-svg (code) | Critical | Fixed — both checks now required together |

Verified by `svg_tessellation_demo`'s pixel readback (star interior filled, a concave notch not) and by strengthening `tre-svg`'s own five-pointed-star unit test to check total area against the true shoelace-formula area AND that a known concave-notch point is covered by no triangle -- a regression test for exactly this bug class, since the test's prior, weaker form (triangle count only) did not catch it. All 7 pre-existing Vulkan examples re-run manually under `VK_LAYER_KHRONOS_validation`, zero errors. `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace, including 15 new `tre-svg` unit tests.

---

## Phase 3 Step 3.3.2 Implementation (2026-09-06)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 3.3.2 (SIMD path-morphing interpolation), the second of Step 3.3's sub-steps. Full detail in `planning/archive/LOG_PHASE3_STEP3_3_2.md`; this is the summary for the documentation's own record.

Status: **Complete.** A notably smoother implementation than Step 3.3.1 -- the SIMD batch-lerp math itself was correct on the first real test run; the only issue found was in a unit test's own assumption, not the implementation, closer in kind to Step 3.1's "nothing broke" experience than Step 3.3.1's two real algorithm bugs.

### 88. [Nice-to-have] Unit test assumed bit-exact round-tripping through FMA at `t=1.0`
`lerp_points_batch_at_t_zero_and_one_returns_the_endpoints_exactly` originally used `assert_eq!` for exact equality between the SIMD output at `t=1.0` and the raw keyframe values. Failed immediately on real (not hand-picked-to-be-exact) sample data: `-1.4` round-tripped through `(to - from).mul_add(1.0, from)` as `-1.4000001`, and `0.0` came back as `-0.0`. `(b - a).mul_add(1.0, a)` is mathematically `b`, but the `b - a` subtraction rounds once, separately, before the fused multiply-add's own single rounding runs -- two composed roundings don't always cancel back to the original bit pattern, the same category of issue `compose_batch`'s own test suite already documented for FMA vs. separate-operation composition.

**Change:** rewrote the test to compare within `EPSILON`, matching every other float-comparison test in this crate, instead of asserting exact equality on a value produced by computation rather than typed as a literal.

## Summary table (Phase 3 Step 3.3.2)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 88 | Unit test assumed bit-exact FMA round-tripping at t=1.0, contradicting this project's own established FMA-precision lesson | tre-math (test code) | Nice-to-have | Fixed — epsilon comparison, matching every other float test in the crate |

Verified by `svg_morph_demo`'s pixel readback at `t = 0.0, 0.5, 1.0` using two probe points (one inside the "from" keyframe but outside "to"; one outside BOTH keyframes but inside their exact vertex-wise midpoint shape) that pairwise distinguish all three renders -- independently verified against a Python ray-casting point-in-polygon reference before any Rust code was written, and the actual GPU render matched that prediction exactly on the first run. All 8 pre-existing Vulkan examples re-run manually under `VK_LAYER_KHRONOS_validation`, zero errors. `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace, including 3 new `tre-math` unit tests and 3 new `tre-svg` unit tests.

## Phase 3 Step 3.3.3 Implementation (2026-09-06)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 3.3.3 (stencil-and-cover fallback rendering for self-intersecting paths), the third and final of Step 3.3's sub-steps -- the first to extend the shared RHI surface (`begin_frame`/`create_pipeline`) rather than stay confined to `tre-svg`/a demo. Full detail in `LOG.md` (to be archived as `planning/archive/LOG_PHASE3_STEP3_3_3.md`); this is the summary for the documentation's own record.

Status: **Complete.** Both `NonZero` and `EvenOdd` fill rules built. Two real bugs found and fixed, both caught before or by this step's own verification demo rather than surviving into a separate review pass -- closer in kind to Step 3.3.1's experience (real algorithm/API-usage bugs on first attempt) than Step 3.3.2's (a test-only issue).

### 89. [Critical] Stencil-only image aspect mask is invalid Vulkan without `separateDepthStencilLayouts` explicitly enabled
The plan's design -- a stencil-only image view/aspect-mask/layout (`STENCIL_ATTACHMENT_OPTIMAL`) on the combined depth+stencil format every physical device is guaranteed to support one of, since this project never reads or writes depth -- is only valid with `VK_KHR_separate_depth_stencil_layouts` (core in Vulkan 1.2, the API version already targeted) explicitly enabled at device creation. Re-running all 7 pre-existing headless examples immediately after wiring the stencil attachment into `begin_frame` (before writing any new demo code) failed every one of them with `VUID-VkImageMemoryBarrier-image-03320`: with the feature disabled, an aspect mask on a combined depth+stencil image must include both `DEPTH` and `STENCIL` bits, not `STENCIL` alone. An easy gap to leave open, since the feature struct is a separate opt-in never implied by targeting API version 1.2 alone.

**Change:** added `vk::PhysicalDeviceSeparateDepthStencilLayoutsFeatures` (`separate_depth_stencil_layouts(true)`) to `VulkanDevice::new`'s feature chain, alongside the existing `dynamic_rendering`/`descriptor_indexing` feature structs. All 7 examples passed immediately after, with no other change needed.

### 90. [Critical] `triangulate`'s ear-validity checks only guard against the boundary remaining *during* clipping, not the *original* polygon's full edge set
This step's own verification demo assumed a classic self-intersecting pentagram would be rejected by `tre_svg::triangulate` with `SvgError::NotSimplePolygon` -- instead it returned `Ok(...)` with a plausible-looking but wrong triangulation. Root cause: Step 3.3.1's ear-validity checks ("no vertex inside the candidate triangle," "no remaining edge crosses the candidate diagonal") are evaluated against whichever boundary happens to still remain at each step of clipping, never against the complete original polygon -- so a self-intersecting polygon can clip cleanly to completion if the specific ears chosen never happen to produce a diagonal that conflicts with what's left. Confirmed by hand computation that this exact pentagram's non-adjacent edges 0-1 and 2-3 genuinely cross at approximately (120.8, 109.8). A silent wrong-answer for input the function is specifically documented to either handle or reject, not a rendering artifact.

**Change:** added `has_self_intersection`, an explicit, global check over every pair of non-adjacent original edges (independent of, and run once before, the clipping process itself), wired into `triangulate` immediately after the existing `points.len() < 3` guard. Locked in as a permanent `tre-svg` unit test regression (`rejects_a_classic_self_intersecting_pentagram`).

## Summary table (Phase 3 Step 3.3.3)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 89 | Stencil-only aspect mask on a combined depth+stencil image is invalid without `separateDepthStencilLayouts` enabled | tre-rhi-vulkan | Critical | Fixed — feature explicitly enabled at device creation; all 7 pre-existing examples re-verified |
| 90 | `triangulate`'s ear-validity checks don't guarantee catching every self-intersecting original polygon | tre-svg | Critical | Fixed — explicit global `has_self_intersection` pre-check added, with a permanent regression test |

Verified via a standalone Python winding-number/ray-casting script computing expected pixel outcomes at the pentagram's center and outer tip *before* any Rust demo code was written, then matched exactly by the real GPU render under both fill rules (`stencil_and_cover_demo`, output PNG visually inspected). All 10 pre-existing examples (7 headless + 3 windowed) re-run manually after the shared RHI surface changes to `begin_frame`/`create_pipeline`, zero validation errors -- the elevated verification bar this step's own plan called for. `cargo fmt`/`clippy -D warnings`/`test` clean across the workspace, including 1 new `tre-engine` usage and `tre-svg`'s suite growing to 24 tests.

## Phase 4 Step 4.1 Implementation (2026-09-06)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 4.1 (HarfBuzz & FreeType Integration), built as an all-pure-Rust font stack per the project owner's direction (`rustybuzz` + `skrifa` in place of the literal HarfBuzz/FreeType C libraries named in IMPLEMENTATION.md -- see `planning/archive/PLAN_PHASE4_STEP4_1.md`). Full detail in `planning/archive/LOG_PHASE4_STEP4_1.md`; this is the summary for the documentation's own record.

Status: **Complete.** A new `tre-text` crate: real bidi + script run segmentation and shaping, a real `fontconfig`-driven fallback cascade, and real glyph outline extraction, all verified against real installed fonts and a real font's real glyph rendered through the pre-existing tessellation + Vulkan pipeline. One finding, a test-assumption bug caught by the test itself before it could ship, not a defect in the shipped fallback logic.

### 91. [Nice-to-have] A fallback-cascade unit test assumed a plain-text font lacks emoji coverage, which was empirically false for the real installed font
The test proving `resolve_font_index` actually falls through the cascade originally used U+1F600 (the classic grinning-face emoji) as "a codepoint DejaVu Sans surely lacks." It failed immediately: `fc-query`'s own charset dump confirmed DejaVu Sans's real, installed build genuinely maps the entire classic "Emoticons" Unicode block (U+1F600-U+1F61F) to real glyph entries (monochrome, not color, but a real `cmap` hit) -- a long-standing feature of DejaVu's unusually broad Unicode coverage. `covers`/`resolve_font_index` themselves were correct throughout; only the test's unverified assumption about a real font's contents was wrong.

**Change:** re-verified via the same `fc-query` charset-dump technique before picking a replacement, and switched to U+1F9E0 (the "brain" emoji, a newer Unicode block independently confirmed absent from both DejaVu Sans's and Noto Sans's charset dumps and present in Noto Color Emoji's) -- the same codepoint reused in `text_shaping_demo`'s own live fallback-resolution proof.

## Summary table (Phase 4 Step 4.1)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 91 | A fallback-cascade unit test assumed emoji coverage a real installed font actually has, contradicting the test's own premise | tre-text (test code) | Nice-to-have | Fixed — switched to an independently-verified-absent codepoint before asserting |

Verified by `cargo fmt`/`clippy -D warnings`/`test` clean across the workspace (11 new `tre-text` unit tests, all against real installed fonts rather than synthetic data) and `text_shaping_demo`'s live proofs: bidi+script shaping of a real mixed Latin/Hebrew string (Hebrew run's glyphs confirmed in visually-reversed cluster order), real fallback resolution of an emoji codepoint absent from the primary font, a real glyph's ('L') outline extracted, flattened via `tre-svg`'s now-`pub` flatten functions, and rendered through the unmodified pre-existing pipeline, and (added after the project owner specifically asked to see stronger evidence of correctness, given how much text rendering matters) the real word `"TEXT"` shaped and rendered with every letter positioned purely by `rustybuzz`'s own advances -- seven probes (each letter's bounding-box center, plus every adjacent inter-letter gap checked against every letter's outline, not just its neighbors) all matched an independently-computed point-in-polygon check before the GPU render was trusted. All 11 pre-existing examples re-run manually, zero validation errors, confirming the new dependency graph disturbed nothing already built.

## Phase 4 Step 4.2.1 Implementation (2026-09-06)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 4.2.1 (Guillotine atlas bin-packing), the first of Step 4.2's four sub-steps (split the same way Step 3.3 was, per the project owner's direction). Full detail in `planning/archive/LOG_PHASE4_STEP4_2_1.md`; this is the summary for the documentation's own record.

Status: **Complete.** The packer itself worked correctly on the first real run (all 5 unit tests and all 12 demo placements passed immediately). This sub-step's own demo, however, was the first thing in this project's history to render genuinely distinct non-white colors -- and in doing so surfaced one real, previously-invisible engine-wide finding plus one self-authored demo bug, neither in the packer itself.

### 92. [Should-fix] `walking_skeleton.frag`/`sdf_rounded_rect.frag` never perform the sRGB-to-linear conversion `UiVertex::color`'s own doc comment promises
The headless swapchain's color format is `vk::Format::B8G8R8A8_SRGB`, so the GPU automatically sRGB-encodes whatever the fragment shader outputs on store. Both shaders pass the vertex color straight through unchanged (`out_color = frag_color;`), and the `color` vertex attribute itself is fetched as plain `R8G8B8A8_UNORM` (no gamma awareness either). Net effect: an sRGB-authored color is treated as already-linear and then sRGB-encoded once more on store -- correct only for values that are fixed points of the gamma curve (pure 0 or 255 per channel), wrong for everything else (a mid-tone gray `150` round-trips to `202`).
>
> **Correction (Phase 1-4 review, 2026-09-06):** the claim above -- and the identical claim this finding's own Step 4.2.1 status note repeated -- that this went undetected because "every demo before this one rendered exclusively pure white/black" is itself wrong, and self-contradicted by this very finding's own title: `walking_skeleton.rs` (line 73) draws `rgba8(0xE0, 0xA0, 0x40, 0xFF)`, a genuinely non-invariant amber (R=224, G=160, B=64, none of which are 0 or 255) -- exactly the mid-tone color class this bug corrupts -- and predates `atlas_packing_demo` (where this finding was actually raised) by four phases. The real reason this went unnoticed for `walking_skeleton` specifically is that its own verification was a qualitative screenshot check on a live windowed swapchain, not a programmatic exact-pixel-value assertion (those only started with `atlas_packing_demo`'s headless PNG readback) -- so this double-sRGB-encoding bug has been visibly wrong on real GPU output since this project's very first rendered pixel (Phase 0), not merely a latent risk since Step 4.2.1 as originally stated here.

Not a regression: `walking_skeleton.frag`'s own header already calls it a "Phase 0 placeholder," and TECHNICAL.md Section 6's canonical sRGB<->linear formula is explicitly referenced as IMPLEMENTATION.md Step 7.1's ("Linear sRGB Conversions & HDR") job, not yet built. Real and worth tracking as a finding regardless -- every non-invariant color rendered by this engine today is photometrically wrong until Step 7.1 lands, which matters for upcoming MSDF text rendering (Step 4.2.3) and any future colored icon/decal work in the meantime.

**Change (originally, Step 4.2.1):** not fixed here -- implementing real linear-space color management now would preempt Step 7.1's own, more complete scope (HDR, tone mapping) with a partial shader patch. `atlas_packing_demo`'s own palette instead uses only the 7 "pure" (each channel 0 or 255) colors, which round-trip correctly both today and after Step 7.1's real fix, so this demo needs no revisiting later.

> **Update (Phase 7 Step 7.1, 2026-09-08): Fixed for real.** A canonical `srgb_to_linear(vec3)` GLSL helper (TECHNICAL.md Section 6.2's exact formula) is now applied to `frag_color.rgb` before any coverage/blend math in all 4 real fragment shaders affected -- not just the 2 this finding's own title names: `walking_skeleton.frag`, `sdf_rounded_rect.frag`, `bindless_textured.frag`'s no-texture-bound fallback branch, and `msdf.frag` (the last two found by direct inspection during Step 7.1's own implementation, not anticipated by this finding's original text). Verified by a new demo, `linear_color_demo.rs` (`demo/phase7_step7_1/`): a real, opaque, non-fixed-point `rgb(150, 100, 200)` rect now round-trips through the real GPU **exactly** -- `[150, 100, 200]` back out, matching this finding's own `150`->`202` worked example precisely (the demo independently computes the old broken value too: `[202, 168, 229]`, confirming the fix's real, measurable effect, not just "didn't crash"). All 24 Vulkan demos re-run manually, zero regressions, exactly as this finding's own resolution below predicted (every pre-existing demo already used only gamma-invariant colors). See IMPLEMENTATION.md's own Step 7.1 write-up for the full account.

### 93. [Nice-to-have] The demo's own `pixel_at` helper compared real BGRA memory bytes against an RGBA-ordered expectation
`HeadlessSwapchain::read_pixels_bgra8` is correctly named and returns genuine BGRA memory byte order. This demo's first draft of `pixel_at`, copied from the identical pattern every prior demo already used, returned those bytes unswapped and compared them against a `[R,G,B,A]`-ordered expected color -- invisible in every prior demo since white/black are invariant under a channel swap (R=G=B). A real red rectangle rendered back as `[0,0,255,255]` (blue) under the wrong comparison.

**Change:** `pixel_at` now explicitly swaps indices 0 and 2 before returning, matching `PALETTE`'s `[R,G,B]` convention. Not an engine defect -- `read_pixels_bgra8` behaves exactly as documented; this was this demo's own first attempt at being the first caller to care about channel order at all.

## Summary table (Phase 4 Step 4.2.1)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 92 | `walking_skeleton.frag`/`sdf_rounded_rect.frag`/`bindless_textured.frag`/`msdf.frag` never perform the sRGB-to-linear conversion `UiVertex::color` documents | tre-rhi-vulkan (shaders) | Should-fix | Fixed at Phase 7 Step 7.1 -- `srgb_to_linear` applied in all 4 shaders; verified via `linear_color_demo.rs`, exact round-trip on a real non-fixed-point color, all 24 demos re-run, zero regressions |
| 93 | Demo's `pixel_at` compared real BGRA bytes against an RGBA-ordered expectation | tre-rhi-vulkan (example code) | Nice-to-have | Fixed — explicit channel swap added, matching `read_pixels_bgra8`'s documented behavior |

Verified by `cargo fmt`/`clippy -D warnings`/`test` clean across the workspace (5 new `tre-atlas` unit tests, including a hand-worked-out 100x100/90x10 split-heuristic example) and `atlas_packing_demo`'s real GPU render: 12 varied rectangles packed with zero overlaps, every placement's own center pixel-verified as its own color, and one unpacked probe confirmed still background. All 12 pre-existing examples re-run manually, zero validation errors.

## Phase 4 Step 4.2.2 Implementation (2026-09-06)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 4.2.2 (MSDF glyph generation via `fdsm`), the second of Step 4.2's four sub-steps. Full detail in `planning/archive/LOG_PHASE4_STEP4_2_2.md`; this is the summary for the documentation's own record.

Status: **Complete.** The core pipeline (contour conversion, fit transform, edge coloring, MSDF generation, sign correction) worked correctly on the first real run against a genuine hole-having glyph (`'O'`) -- both new unit tests and the demo's independent scanline check passed immediately. Two small, non-architectural issues surfaced while wiring up the demo and re-touching this crate's manifest, neither in the MSDF math itself.

### 94. [Nice-to-have] The demo's first draft called `image::ImageBuffer::save()`, which needs a codec feature this crate deliberately doesn't enable
`tre-text` depends on `image` with `default-features = false` (matching `fdsm`'s own choice -- only the plain buffer types are needed, not bundled format codecs). `.save(path)` compiled but failed at runtime (`Unsupported(... Format(Name("Png")))`) since no PNG codec is linked in without `image`'s own `png` feature.

**Change:** switched both PNG writes to the same standalone `png` crate + `png::Encoder` pattern every other demo in this project already uses, extracting raw bytes via `ImageBuffer::as_raw()`. Not a real defect -- `default-features = false` was the right call for the library; the demo just needed to encode PNGs the way every other demo here already does.

### 95. [Nice-to-have] `tre-svg` was a real dependency of `tre-text` but was never actually used by it
Step 4.1 added `tre-svg` to `tre-text`'s `[dependencies]` intending to reuse its flatten functions, but that reuse only ever happened in a completely different crate's example (`tre-rhi-vulkan/examples/text_shaping_demo.rs`, which already separately declares its own `tre-svg` dev-dependency). `tre-text`'s own library code never imported `tre_svg`, and this step's MSDF generation doesn't need curve flattening either (`fdsm` operates on real Bezier curves directly).

**Change:** removed the unused `tre-svg` entry from `tre-text`'s `[dependencies]` entirely. Confirmed `tre-rhi-vulkan`'s `text_shaping_demo` still builds and runs correctly via its own independent dependency, unaffected.

## Summary table (Phase 4 Step 4.2.2)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 94 | Demo used `image::save()`, which needs a codec feature this crate doesn't enable | tre-text (example code) | Nice-to-have | Fixed — switched to the standalone `png` crate, matching every other demo |
| 95 | `tre-svg` was declared as a `tre-text` dependency but never used by it | tre-text (Cargo.toml) | Nice-to-have | Fixed — removed; the real consumer already has its own independent dependency |

Verified by `cargo fmt`/`clippy -D warnings`/`test` clean across the workspace (3 new `tre-text` unit tests: contour-closing logic, and a hand-built shape's interior/exterior median check) and `msdf_generation_demo`'s real run: a genuine `'O'` glyph extracted as 2 contours, its MSDF's center scanline showing exactly 2 outside-to-inside transitions (the signature only a true ring can produce), and both output PNGs visually inspected -- a sharp, correctly-hollow ring in the rendered preview and a consistent raw-channel dump at the true 32x32 resolution. All 13 pre-existing examples re-run manually, zero validation errors, confirming this sub-step's `Cargo.toml` changes disturbed nothing already built.

## Phase 4 Step 4.2.3 Implementation (2026-09-06)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 4.2.3 (the MSDF evaluation shader and a real anti-aliased glyph render), the third of Step 4.2's four sub-steps -- the sub-step that actually resolves the jagged-'X' observation drawn from Step 4.1's demo. Full detail in `planning/archive/LOG_PHASE4_STEP4_2_3.md`; this is the summary for the documentation's own record.

Status: **Complete, no findings.** Worth recording plainly, matching Phase 3 Step 3.1's own precedent for this: this step's new surface area (one new `TextureFormat` variant, one new fragment shader reusing an already-proven vertex shader and an already-proven bindless-texture pipeline) was small and well-isolated enough that the real GPU render was correct on the very first run -- no shader bug, no pipeline-creation issue, no Vulkan validation error, nothing to fix. The groundwork from Steps 2.1 (bindless textures), 4.2.1 (the packer), and 4.2.2 (the MSDF generator) did the real work of de-risking this step in advance.

Verified by `cargo fmt`/`clippy -D warnings`/`test` clean across the workspace and `msdf_rendering_demo`'s real GPU render: the same `'O'` glyph from Step 4.2.2, its MSDF uploaded as a real `Rgba8Unorm` texture and rendered at roughly 7x on-screen magnification through the new `msdf.frag`. A center-row pixel scan (computed against the actual rendered pixels, not predicted, since bilinear texture filtering makes the exact transition point a function of GPU sampling) found real stretches of both fill color and background, plus genuinely intermediate pixels at each ring-wall crossing -- the concrete, measurable signature of real sub-pixel anti-aliasing. Output PNG visually inspected and shows a smoothly anti-aliased hollow ring with no visible jaggedness at this magnification. All 13 pre-existing examples re-run manually, zero validation errors.

## Phase 4 Step 4.2.4 Implementation (2026-09-06)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 4.2.4 (multi-window atlas concurrency), the closing sub-step of the whole Step 4.2 arc -- and, with it, all of Phase 4. Full detail in `planning/archive/LOG_PHASE4_STEP4_2_4.md`; this is the summary for the documentation's own record.

Status: **Complete.** The real concurrency primitives (`MpscRingBuffer`, `SwmrSlotTable`, `AtlasOwner`) all worked correctly on the first attempt, including their own real multi-threaded stress tests. This step's own capstone demo -- the first thing in this project to drive several genuinely independent OS threads through the *whole* pipeline at once (concurrent request, real packing, real MSDF generation, real publish, real GPU render) -- surfaced two real, minor findings, both in the demo's own code, neither in the concurrency primitives themselves.

### 96. [Nice-to-have] `thread::yield_now()` is only a scheduler hint, not a real wait -- a tight polling loop built on it can starve indefinitely
The demo's first draft polled for each letter's atlas placement using `thread::yield_now()` between attempts (100,000 attempts). Every lookup failed. Confirmed by hand: switching the same loop to a real `thread::sleep` between attempts resolved every letter almost immediately, proving the concurrency mechanism itself was correct throughout -- `yield_now()`'s own documentation states plainly that the OS scheduler is free to ignore it entirely, and this environment's scheduler apparently did, letting a 100,000-iteration retry budget exhaust in pure CPU time without ever switching to the atlas owner's background thread.

**Change:** both the producer's own request-retry loop and the result-polling loop now use a real `sleep`-based backoff.

### 97. [Nice-to-have] Checking only a glyph's bounding-box center to verify it rendered repeats a lesson Step 4.1 already taught with `'L'`
The demo's first draft verified each letter actually rendered by checking only its on-screen bounding-box center pixel. `'G'` failed -- its open counter places real background exactly at its own bbox center, the identical property that made Step 4.1's own `'L'` demo pick a hand-verified interior point instead of the bbox center. A *generic*, per-letter verification loop can't hand-pick a good interior point for six arbitrary real letters the way a single-letter demo can.

**Change:** rewrote the check to scan each letter's entire on-screen quad for any pixel differing from the background, correct regardless of which specific letters are being verified.

## Summary table (Phase 4 Step 4.2.4)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 96 | Demo's polling loops used `thread::yield_now()`, which is only a scheduler hint and can starve | tre-rhi-vulkan (example code) | Nice-to-have | Fixed — switched to a real `sleep`-based backoff |
| 97 | Demo verified rendering via bbox-center pixels only, which can land on background for open-counter glyphs | tre-rhi-vulkan (example code) | Nice-to-have | Fixed — scans the whole on-screen quad for any non-background pixel instead |

Verified by `cargo fmt`/`clippy -D warnings`/`test` clean across the workspace (6 new `tre-memory` unit tests for `MpscRingBuffer`, including a real 8-thread/80,000-item stress test; 11 new tests across `tre-atlas` for `SwmrSlotTable`, `AtlasKey` packing, and `AtlasOwner`'s own real multi-producer-thread round trip) and `atlas_concurrency_demo`'s real run: 3 producer threads concurrently requesting MSDF space for the 6 letters of "GLYPHS," every placement non-overlapping and byte-identical to an independently regenerated MSDF, the finished shared atlas uploaded as one real GPU texture and every letter rendered correctly in a single draw call via the unmodified `msdf.frag` pipeline. Output PNG visually inspected and clearly reads "GLYPHS." All 15 pre-existing examples re-run manually, zero validation errors.

## Phase 1-4 Comprehensive Review (2026-09-06)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions. Requested explicitly by the project owner as a complete, twice-checked review of everything built so far (Phases 1-4, all steps) once Phase 4 closed: "look for code optimizations, cross-phase implementation optimizations and possible bugs/errors, performance optimizations, code formatting, code audit and security issues, proper and optimized functionality... Rendering performances and optimizations, frame rates are optimal, typography is clean, optimized, and error free... Everything should be checked twice to ensure nothing is missed and all phases work together as designed," followed by "produce a report with all findings and fixes implemented."

Scope and method: six independent dimensions (concurrency & data-race correctness; performance/allocation/rendering-hot-path efficiency; security/unsafe-code/trust-boundary audit; code quality & cross-crate consistency; typography pipeline correctness end to end; cross-phase integration & documentation accuracy), each run as its own "Find" agent reading the real code (not this document's own claims) against DESIGN/TECHNICAL/ARCHITECTURE/IMPLEMENTATION.md, followed by an independent "Verify" agent per dimension instructed to adversarially try to refute each finding by re-reading the exact cited lines rather than taking the Find agent's word for it ("checked twice," per the request). All 19 findings across all 6 dimensions were independently confirmed by their verifier; none were rejected. Every confirmed finding was then triaged for this pass: a contained, low-risk code fix was applied directly wherever one existed; a finding whose real fix is itself substantial new feature work (a full LRU eviction system, real swapchain recreation, MSDF-aware mipmap generation, a from-scratch reflex-vertex-set ear-clipping rewrite) was deliberately left as a clearly disclosed, documented gap rather than built opportunistically inside an unrelated review/fix pass — consistent with this project's own precedent (finding #92's Step 7.1 deferral).

### Concurrency & data-race correctness

### 98. [Critical] `upload_command_pool`'s `Mutex` guard was dropped before the command-pool operations it exists to protect, reopening finding #72
`VulkanTexture::from_pixels` did `let upload_pool = *device.upload_command_pool.lock().expect(...)` -- the temporary `MutexGuard` returned by `.lock()` is dropped at the end of that `let` statement (only the `Copy` handle value survives), so the lock was released *before* the subsequent `allocate_command_buffers` call and long before `free_command_buffers` -- both fully outside any lock, despite the struct's own doc comment claiming "concurrent `create_texture` calls from multiple threads serialize safely." Per the Vulkan spec, `vkAllocateCommandBuffers`/`vkFreeCommandBuffers` require the command pool to be externally synchronized; two threads calling `create_texture` concurrently -- the whole point of this dedicated pool, and exactly what ARCHITECTURE.md's planned per-window worker threads would eventually trigger -- would race on the same `VkCommandPool` with no synchronization at all. No current call site exercises concurrent `create_texture` (confirmed via grep), so this was a latent, not actively-triggered, defect.

**Change:** the guard is now bound to a named local (`upload_pool_guard`) and kept alive across the entire allocate -> record -> submit -> free sequence, dropped explicitly right after the final `free_command_buffers` call. This is the actual fix for #72, which is now genuinely closed rather than only appearing closed.

### 99. [Should-fix] `AtlasKey::from_glyph(u32::MAX, u32::MAX)` packs to `u64::MAX`, colliding with `SwmrSlotTable`'s reserved `EMPTY_KEY` sentinel
The only guard against this was a `debug_assert_ne!`, compiled out in release builds (the configuration this workspace ships). When a real key's `u64` representation equals the sentinel, `SwmrSlotTable::insert`'s first probed slot (freshly initialized to the sentinel) matches on `existing == key_u64` before ever reaching the "claim this empty slot" branch, so `keys[index]` is never actually written even though `values[index]` now holds real data. A later `insert` for a *different* key probing through that same index sees the sentinel and silently claims/overwrites it, corrupting an unrelated key's atlas placement with no panic, crash, or log in release builds. The practical trigger (`font_id`/`glyph_id` both simultaneously at `u32::MAX`) is extremely unlikely with real font/glyph ranges, but the code path was real and unguarded.

**Change:** `SwmrSlotTable::insert`/`get` now reject the sentinel value unconditionally (a real runtime check, not `debug_assert`) rather than letting it miscompare against a freshly-initialized slot -- a general fix at the table layer protecting against *any* `K: Into<u64>` that ever produces this value, not just this one caller. A new unit test confirms the sentinel is rejected and doesn't corrupt a real key hashing to the same first-probed slot.

### 100. [Nice-to-have] `AtlasOwnerHandle::request_insert` could report success for a request that raced `AtlasOwner::join`'s shutdown and would never be processed
`request_insert` reported success purely from `queue.push`'s own result, with no way to know the owner thread had already popped `Shutdown` and exited its loop. Since `AtlasOwnerHandle` is `Clone` and independent of the `AtlasOwner` that `join()` consumes, a live handle clone used concurrently with (or after) another caller's `join()` could push a real request that lands after the owner thread stopped polling -- `request_insert` still returns `true`, and `lookup` for that key returns `None` forever, indistinguishable from "still pending." Not exercised by any current caller (`join()` is only ever called once at teardown after producers have already joined), but nothing in the API ruled it out.

**Change:** added a shared `Arc<AtomicBool>` "closed" flag, set (`Release`) immediately before `join()` pushes the `Shutdown` message; `request_insert` now checks it (`Acquire`) first and reports failure once set, closing the largest practical window for this race (a concurrent push racing the flag-set/shutdown-push pair itself is inherent to any two-sided handshake without a full drain protocol, and is not what this fix targets).

### Performance, allocation, and rendering-hot-path efficiency

### 101. [Should-fix] `compose_batch`/`lerp_points_batch`'s per-lane `gather` is a scalar extraction loop, not a real hardware gather, and its overhead may erase the SIMD win entirely for AoS-shaped real data
`gather()` builds an `f32x8` via a per-lane scalar write into a stack array, not a hardware gather instruction. `compose_batch` calls it 12 times per 8-wide chunk (6 `Affine2` fields x 2 operands) to feed just 6 vector FMA/mul instructions of actual arithmetic, then does an equivalent scalar scatter to reassemble `out`; `lerp_points_batch` is the same shape with 4 gathers. Because the real arithmetic these functions accelerate is cheap to begin with, the O(n) gather/scatter overhead rides along with the O(n) vector work it's supposed to speed up, and for AoS-shaped real data (not genuinely separate SoA arrays) it is not obvious this "SIMD" path beats 8 independent scalar calls over the same slices. No criterion benchmark exists in this workspace to settle this either way -- a known, separately-tracked CI gap.

**Change:** not restructured here -- a genuine fix means either switching real hot-path storage to true SoA layout, or adding a criterion benchmark to measure against a plain scalar loop, before more call sites are built on this primitive. Both are real future work, not something to attempt opportunistically alongside an unrelated review/fix pass; documented in detail directly on `gather`'s own doc comment (`crates/tre-math/src/lib.rs`) so a future contributor sees the open question before building more on top of it.

### 102. [Should-fix] Ear-clipping's per-candidate validity checks are each O(m), making `triangulate()` worst-case O(n^3) for a single polygon, while the crate's own point-count cap only bounds the whole document's total, not any one polygon's shape
The `while remaining.len() > 3` loop tries up to `m` candidates per removal, and each candidate's `no_vertex_inside`/`no_edge_crosses` checks are themselves O(m) -- textbook naive ear-clipping, not the reflex-vertex-set-optimized O(n^2) variant. `parse_svg`'s `max_points` threads one shared budget across the whole document tree (`collect_polygons`), not per-path, so a single adversarially-shaped path (a dense comb/star of reflex vertices) can still consume the entire budget while triggering the cubic behavior -- bounding peak memory but not worst-case CPU time. For realistic icon/glyph-sized polygons (tens of points) this is a complete non-issue; it matters only for large, adversarially-shaped single paths.

**Change:** not rewritten here -- a proper fix needs a genuine reflex-vertex-set-maintained ear-clipping algorithm, which risks the exact class of subtle correctness regression the pentagram/L-shape/star-polygon fixes already hard-won into this exact function (see #86-90 above) were fixing, and shouldn't be attempted opportunistically alongside unrelated fixes. Documented explicitly on `triangulate`'s own doc comment and on `parse_svg`'s `max_points` doc comment, so the real worst-case complexity isn't mistaken for a CPU-time bound by a future reader.

### 103. [Should-fix] Every texture (MSDF glyph atlases included) is created with exactly one mip level, so `msdf.frag`'s `fwidth`-based opacity formula -- verified only at ~7x magnification -- has no prefiltered data once a glyph is minified below its fixed 32x32 texel resolution
`VulkanTexture::from_pixels` sets `.mip_levels(1)` unconditionally, making the shared bindless sampler's `mipmap_mode(LINEAR)` dead configuration. Naive minification of an MSDF texture doesn't just blur it -- because the median-of-3-channel decode is nonlinear, it can locally corrupt the encoded distance near corners/thin strokes, and `fwidth`-based anti-aliasing has no way to correct for an already-corrupted sample. Any on-screen glyph smaller than roughly 32 device pixels tall (ordinary UI body text at typical DPI) is affected; the expected symptom is shimmering/incorrect coverage as text scales or moves sub-pixel amounts.

**Change:** not fixed here -- a real fix needs MSDF-aware mip generation (a naive box filter also corrupts the median encoding), which is genuine new rendering-pipeline work, not a bug-fix-sized change. Documented as a known, currently-untested limitation directly in IMPLEMENTATION.md's Step 4.2.3 section, so it isn't mistaken for something that step's magnification-only verification already covered.

### 104. [Should-fix] `tre_svg::morph` heap-allocated a fresh `Vec` on every call before handing it to the zero-alloc `lerp_points_batch` primitive, defeating that primitive's whole purpose on its own real call path
`morph()` did `let mut points = vec![[0.0f32; 2]; from.points.len()];` before every call, immediately defeating `lerp_points_batch`'s own documented purpose ("since an eventual per-frame animation-morphing caller cannot allocate on that path, DESIGN.md Section 2.1's zero-allocation steady state"). The only current real caller (`svg_morph_demo`) calls it 3 times for fixed `t` values in a one-shot loop, not yet inside a real per-frame animation loop, so today's runtime impact is limited -- but the architectural defect is real and would bite the moment a genuine per-frame caller is built on top of it.

**Change:** added `morph_into(from, to, t, out: &mut Vec<[f32; 2]>)`, writing into a caller-supplied buffer (`out.resize`'s no-op-when-already-correctly-sized behavior means a steady-state per-frame loop allocates nothing after its first call). `morph()` is now a thin wrapper over `morph_into` with a fresh `Vec`, kept for one-shot callers like the existing demo. A new unit test confirms `morph_into` never grows its buffer's capacity across repeated same-size calls.

### Security, unsafe-code, and trust-boundary audit

### 105. [Critical] `pack_slot_value` panicked on any atlas rect whose width or height reached exactly 4096 -- the documented production atlas size, not a contrived edge case
`COORD_BITS = 12` gave a mask of 4095, one short of the documented `4096x4096` production atlas (IMPLEMENTATION.md). `AtlasPacker::insert` imposes no upper bound tied to this encoding, so a single glyph/icon request sized to the full atlas on one or both axes (e.g. the first request into a fresh, otherwise-empty packer) succeeds in `insert()` and then panics inside `pack_slot_value` on the atlas owner's background thread -- inconsistent with the sibling full-packer/full-slot-table cases in the exact same function, which are deliberately designed to silently drop rather than panic. `AtlasOwner::join`'s `.expect("atlas owner thread panicked")` then propagates the panic to whichever thread joins it.

**Change:** widened `COORD_BITS` from 12 to 13 (covers 0-8191, inclusive of a full-atlas-sized rect), added a real assert on `generation` (no longer occupies its type's exact full range now that 12, not 16, bits remain for it), and added `fits_packed_range` as a second, defense-in-depth check in `process_insert` so an atlas whose dimensions ever exceed even the widened range is still dropped gracefully like the sibling full-packer/full-slot-table cases, rather than reaching `pack_slot_value`'s own assert at all. New unit tests cover a full-width/height 4096 rect round-tripping correctly and a too-large generation being rejected.

### 106. [Should-fix] `generate_msdf` panicked on a glyph with zero contours (e.g. U+0020 SPACE) or one whose points were all coincident/non-finite, both producible by ordinary or malformed font data, not just caller error
The doc comment claimed "a real glyph always has real extent; an empty input here is a caller error," which is false for the single most common non-ink character in any real text (space) and for a corrupted/malicious font (this crate's own stated threat model) that yields NaN/coincident outline points. Neither case is guarded anywhere upstream (`glyph_outline` legitimately returns an empty `Vec` for space), and none of the three example call sites checked for it before calling `generate_msdf`.

**Change:** `generate_msdf` now returns `Option<MsdfBitmap>` instead of panicking -- `None` for a genuinely empty or degenerate (near-zero-extent) bounding box, so callers can render nothing or substitute DESIGN.md Section 2.6's placeholder-glyph fallback. `bounding_box` now rejects a near-zero (not just non-finite) extent directly, closing a related, previously-undetected gap: the old code's own doc comment claimed it would panic on a "zero-area bounding box," but `bounding_box` never actually checked for that case at all -- it would have silently produced an astronomically-scaled, meaningless transform instead (see also finding #110 below, the typography-dimension's independent report of this same root cause). All 4 real call sites (3 examples plus this crate's own test) updated to `.expect(...)` where the input is known-non-degenerate (a real glyph), and 2 new unit tests cover the empty and coincident-point cases directly.

### 107. [Should-fix] The `width * height * bytes_per_pixel` staging-buffer-length check (added for finding #66) used unchecked `u64` multiplication, which can silently wrap in this workspace's overflow-checks-disabled release profile
`u64::from(width) * u64::from(height) * bytes_per_pixel(format)` can overflow `u64` for extreme `width`/`height` (e.g. both `u32::MAX`, times up to 8 bytes/pixel for `Rgba16Float`), silently wrapping in a release build to a small value a small, real `pixels` buffer could then pass against -- defeating the exact OOB-read protection this check exists to provide. Real GPU `maxImageDimension2D` limits currently stand between this and an actual GPU-side OOB read (a multi-exabyte image allocation would fail first), so this is not immediately exploitable on real hardware, but the validation math itself was unsound, and this is exactly the boundary this crate's own comments identify as protecting a future, less-trusted caller (the eventual Python C-ABI).

**Change:** `expected_len` is now computed in `u128` (ample headroom for the worst case) and compared against `pixels.len() as u128`, rather than relying on GPU-side extent limits as an incidental backstop.

### Code quality, cross-crate consistency, and API design

### 108. [Should-fix] The `pixel_at` BGRA->RGBA channel-swap fix from finding #93 was applied to only 3 of 8 example demos, leaving two visually identical closure signatures with opposite channel semantics
5 demos (`sdf_rounded_rect_demo`, `svg_tessellation_demo`, `svg_morph_demo`, `stencil_and_cover_demo`, `text_shaping_demo`) still returned raw, unswapped `read_pixels_bgra8` bytes from their own `pixel_at` closures, invisible only because every comparison in those files happened to be against a channel-swap-invariant color (white/black/self-referential background) -- exactly the precondition finding #93 itself identified as having hidden the original bug. A future contributor extending any of these demos to check a real non-gray color would have no signal which convention to copy from a sibling file.

**Change:** extracted one shared `pixel_helpers::bgra_pixel_at(bgra, width, x, y) -> [u8; 4]` (`crates/tre-rhi-vulkan/examples/support/pixel_helpers.rs`, included via `#[path = ...]` since examples are separate compilation units with no shared library crate for this) and rewired all 8 examples that read back pixels -- the 5 previously-raw ones plus the 3 already-correct ones -- to delegate to it, so there is exactly one place this channel swap can be gotten wrong going forward. `bindless_textures_demo` was deliberately left alone: its own raw form is self-consistent (its own comment documents BGRA-ordered expected constants), a third, different-but-not-buggy convention that would need its expected-color constants changed too, out of scope for this fix. All 8 rewired examples re-run for real against the GPU with zero validation errors and all pixel assertions still passing.

### 109. [Should-fix] `create_pipeline` and the `build_pass` closure inside `create_stencil_and_cover_pipelines` duplicate the entire shader-module-create -> pipeline-create -> destroy-modules -> wrap sequence almost verbatim
Confirmed: ~50 lines of pipeline-construction plumbing (shader module creation, the `PipelineRenderingCreateInfo` push_next pattern, the `create_graphics_pipelines` call and its SAFETY comment, shader-module destruction, the `VulkanPipelineState` wrap) are repeated near-verbatim between `create_pipeline` and `build_pass` (itself called twice), differing only in which `depth_stencil`/`color_blend` state structs are plugged in -- effectively three near-identical copies of the same tail sequence. `create_universal_pipeline_layout` was already extracted once specifically to avoid duplicating a smaller piece of this same construction; the same reasoning applies here but was not applied when Step 3.3.3 added the stencil-and-cover variant.

**Change:** not refactored here -- extracting a shared `build_graphics_pipeline` helper touches unsafe, correctness-sensitive Vulkan pipeline-creation code across two working, already-verified call sites, and the risk of introducing a subtle initialization-order regression outweighs the benefit of removing pure duplication with no correctness impact of its own. Documented here as a confirmed, real finding for a future dedicated refactor pass, deliberately not attempted opportunistically inside this review/fix pass.

### Typography pipeline correctness end to end

### 110. [Should-fix] `apply_fit_transform`'s `largest.max(f64::EPSILON)` guard prevented a literal divide-by-zero but not the actual corruption it existed to stop, for a glyph contour collapsed to a single coincident point
When a non-empty contour's points are all coincident (a malformed/degenerate glyph), `bbox_width`/`bbox_height` are both exactly `0.0`; `largest.max(f64::EPSILON)` yields `f64::EPSILON` rather than `0`, avoiding NaN/Infinity but producing an astronomically large (though finite) scale that gets baked into the affine matrix and applied to real geometry before `fdsm`'s edge-coloring ever runs -- with no validation step to catch it and substitute a placeholder. This is the same root cause finding #106 (security dimension) independently reported from the "does `generate_msdf` panic correctly" angle; both are closed by the same fix.

**Change:** `bounding_box` (see #106) now rejects a near-zero-extent bounding box directly, so `apply_fit_transform` never receives a degenerate box in the first place; its own `.max(f64::EPSILON)` guard remains as a defensive backstop, no longer load-bearing for correctness.

### 111. [Should-fix] `FontCascade::discover`'s duplicate check compared raw `PathBuf` values, not canonicalized paths, so a symlink-aliased duplicate could silently waste one of only 3 cascade slots
`fontconfig` returns whatever path is stored in a matched pattern's `FC_FILE`, which can differ across a symlinked alias vs. the real file for the same underlying font -- a real, if uncommon, fontconfig configuration pattern on some distros. `CASCADE_FAMILIES` has only 3 entries, so a false-negative duplicate check consumes one of only three fallback slots.

**Change:** the dedup check now compares `std::fs::canonicalize`d paths (falling back to the raw path if canonicalization fails, so a transient filesystem issue doesn't drop an otherwise-valid entry), recognizing two different-looking paths to the same real file as the same font.

### 112. [Nice-to-have] `to_fdsm_contour`'s correctness for an un-`Close`d contour relied entirely on an unenforced upstream invariant (skrifa always emitting `Close`), with no local fallback
Verified today's actual invariant holds (skrifa's `PendingState::finish` unconditionally calls `pen.close()` for every glyf contour), but that guarantee lives entirely in skrifa's internals, not in anything this crate asserts or falls back on -- a future skrifa version or a different `Contour` producer could violate it silently, leaving `fdsm` with an open shape and no error raised anywhere.

**Change:** the closing check now also runs unconditionally after the segment loop (not only inside the `Close` match arm), so a contour lacking an explicit `Close` segment at all is still force-closed if its last point doesn't already equal its start -- a free, zero-cost-on-the-already-correct-path strengthening.

### 113. [Should-fix] The mandated fixed 32x32px MSDF resolution is a genuine, previously-undiscussed quality ceiling on glyph *complexity*, not just glyph *scale*
MSDF's whole benefit is resolution-independent magnification; it cannot invent detail a fixed 32x32 source grid never captured. A genuinely complex glyph (a dense CJK ideograph, a heavily-serifed/ligature glyph) will have distinct nearby strokes blur/merge in the distance field itself, regardless of shader correctness. Every glyph this pipeline has actually tested (`'O'`, `'L'`, "GLYPHS") is Latin and comparatively simple, so this gap is real but has not yet been empirically exercised.

**Change:** not changed -- accepted as a deliberate Latin/simple-script-first limitation. Documented directly in IMPLEMENTATION.md's Step 4.2.2 section so a future contributor building non-Latin/dense-script support knows this ceiling exists before relying on the current fixed resolution.

### Cross-phase integration and documentation accuracy

### 114. [Should-fix] Atlas LRU eviction (IMPLEMENTATION.md Step 4.2 task 4's own explicit scope) was never built, and Step 4.2.4's status text declared Phase 4 complete without disclosing the omission
`owner.rs`'s `process_insert` silently and *permanently* drops a request the packer can't currently fit -- no eviction/reclamation exists anywhere in `tre-atlas`. Step 4.2.1's own status text honestly disclosed "LRU reclamation is separate... future work," but Step 4.2.4 -- the sub-step whose own task list explicitly includes "the `LastFrameUsed` map driving LRU eviction" -- never mentioned this gap and declared "This closes Step 4.2... and, with it, all of Phase 4." DESIGN.md Section 2.6's placeholder-fallback story implies eventual resolution once eviction frees space; without eviction, a full atlas makes that fallback permanent, not "for this frame," for any new key from that point on.

**Change:** not built here -- real LRU eviction is a genuine, substantial feature (a `LastFrameUsed` map, an eviction policy, generation-counter bookkeeping the packed format already reserves room for), not a bug-fix-sized change, and building it opportunistically alongside an unrelated review/fix pass would risk under-designing it. Disclosed explicitly in IMPLEMENTATION.md's Step 4.2.4 section, matching the honesty of Step 4.2.1's own original disclosure.

### 115. [Should-fix] Finding #92's own explanation for why the sRGB gap went unnoticed ("every demo before this one rendered exclusively pure white/black") was self-contradicted by its own title, which names `walking_skeleton.frag` -- a demo that draws a genuinely non-invariant amber color
`walking_skeleton.rs` draws `rgba8(0xE0, 0xA0, 0x40, 0xFF)` -- a real mid-tone amber, not white/black -- and predates `atlas_packing_demo` (where #92 was actually raised) by four phases. The real reason it went unnoticed there specifically is that its own verification was a qualitative screenshot check, not a programmatic exact-pixel assertion (those only started with `atlas_packing_demo`'s headless PNG readback) -- a different, more accurate explanation than "the colors were gamma-invariant." This means the double-sRGB-encoding bug has been visibly wrong on real GPU output since this project's very first rendered pixel (Phase 0), not merely a latent risk since Step 4.2.1 as both documents previously stated.

**Change:** corrected both REVIEW.md finding #92's own body text and IMPLEMENTATION.md's Step 4.2.1 status text to state the accurate explanation, without changing #92's own disposition (still correctly deferred to Step 7.1).

### 116. [Should-fix] No swapchain-recreation/resize-recovery code exists anywhere in the codebase, despite IMPLEMENTATION.md Step 1.1 requiring DPI-triggered instantaneous resizing and declaring status "Linux complete" without disclosing this gap
Confirmed via grep: `VulkanSwapchain` has only a constructor, no recreate/resize method; `tre-platform`'s `scale_factor()` has no caller outside its own dispatch wrapper; `InputEvent::Resized` is drained by `input_demo` purely to log it; `acquire_next_image`/`present` correctly surface `EngineError::SwapchainOutOfDate` but nothing anywhere recovers from it. Step 1.1's own status text lists several other real caveats/deviations honestly but never mentions this one -- the only disclosure anywhere in the docs is a buried, unrelated aside four steps later (Step 3.3.3's stencil-image discussion). Any real application that lets a user resize a window -- the overwhelmingly common case for desktop UI -- would hit this on the very first resize with no recovery path.

**Change:** not built here -- real swapchain recreation across all three eventual platform backends is substantial, genuinely scoped future work (likely its own future Step), not a fix to retrofit alongside an unrelated review pass. Disclosed explicitly in IMPLEMENTATION.md's Step 1.1 status section, matching the honesty of that same section's Windows/macOS deferral disclosure.

## Summary table (Phase 1-4 Comprehensive Review)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 98 | `upload_command_pool`'s `Mutex` guard dropped before the operations it protects, reopening #72 | tre-rhi-vulkan (src) | Critical | Fixed — guard held across the whole allocate/record/submit/free sequence |
| 99 | `AtlasKey::from_glyph(MAX, MAX)` collides with `SwmrSlotTable`'s `EMPTY_KEY` sentinel | tre-memory, tre-atlas | Should-fix | Fixed — sentinel rejected unconditionally at the table layer |
| 100 | `request_insert` could report success for a request racing `join()`'s shutdown | tre-atlas | Nice-to-have | Fixed — shared closed flag checked before every push |
| 101 | `gather`'s scalar per-lane loop may erase `compose_batch`/`lerp_points_batch`'s SIMD win for AoS data | tre-math | Should-fix | Documented as an open question on `gather`'s own doc comment; not restructured |
| 102 | Ear-clipping is worst-case O(n^3) per polygon; the point cap bounds memory, not CPU time | tre-svg | Should-fix | Documented on `triangulate`/`parse_svg`'s doc comments; not rewritten |
| 103 | Single-mip-level textures give MSDF no prefiltered data once minified below 32px | tre-rhi-vulkan, shaders | Should-fix | Documented as a known limitation in IMPLEMENTATION.md Step 4.2.3; not fixed |
| 104 | `morph()` allocated a fresh `Vec` every call, defeating `lerp_points_batch`'s zero-alloc design | tre-svg | Should-fix | Fixed — added `morph_into` with a caller-supplied buffer |
| 105 | `pack_slot_value` panicked on a rect reaching the documented 4096 production atlas size | tre-atlas | Critical | Fixed — widened to 13 bits/coordinate plus a graceful drop for any still-over-range rect |
| 106 | `generate_msdf` panicked on an empty (e.g. space) or degenerate glyph | tre-text | Should-fix | Fixed — now returns `Option<MsdfBitmap>`, `None` for no-real-ink input |
| 107 | `expected_len`'s `u64` multiplication could silently wrap in a release build | tre-rhi-vulkan | Should-fix | Fixed — computed in `u128` instead |
| 108 | `pixel_at`'s BGRA/RGBA channel-swap fix (#93) was applied to only 3 of 8 demos | tre-rhi-vulkan (examples) | Should-fix | Fixed — extracted one shared `pixel_helpers::bgra_pixel_at`, used by all 8 |
| 109 | `create_pipeline`/`build_pass` duplicate ~50 lines of pipeline-construction plumbing | tre-rhi-vulkan | Should-fix | Documented; not refactored (too risky to touch working unsafe Vulkan code opportunistically) |
| 110 | `apply_fit_transform`'s epsilon guard prevented divide-by-zero but not astronomical-scale corruption | tre-text | Should-fix | Fixed — same root-cause fix as #106 (`bounding_box` now rejects near-zero extent) |
| 111 | `FontCascade::discover`'s dedup check missed symlink-aliased duplicate font paths | tre-text | Should-fix | Fixed — compares canonicalized paths |
| 112 | `to_fdsm_contour` relied on an unenforced upstream "always closes" invariant | tre-text | Nice-to-have | Fixed — unconditional post-loop close-if-needed check added |
| 113 | Fixed 32x32px MSDF resolution is an undiscussed ceiling on glyph complexity, not just scale | documentation | Should-fix | Documented as an accepted Latin/simple-script-first limitation; not changed |
| 114 | Atlas LRU eviction (Step 4.2 task 4) was never built; Step 4.2.4 didn't disclose it | tre-atlas, documentation | Should-fix | Documented — gap disclosed in IMPLEMENTATION.md Step 4.2.4, matching #4.2.1's own honesty |
| 115 | Finding #92's own explanation for going unnoticed was self-contradicted by its own title | documentation | Should-fix | Fixed — corrected REVIEW.md #92 and IMPLEMENTATION.md Step 4.2.1's text |
| 116 | No swapchain recreation/resize-recovery exists despite Step 1.1 claiming "Linux complete" | tre-rhi-vulkan, tre-platform, documentation | Should-fix | Documented — gap disclosed in IMPLEMENTATION.md Step 1.1's status section |

Verified by `cargo fmt --all -- --check`/`clippy --workspace --all-targets -- -D warnings`/`test --workspace` clean across the whole workspace (zero failures across every crate, including 4 new `tre-memory`/`tre-atlas` unit tests for the sentinel-rejection and full-atlas-rect-packing fixes, 2 new `tre-text` unit tests for the empty/degenerate-glyph `None` cases, and 1 new `tre-svg` unit test for `morph_into`'s buffer-reuse behavior) and all 15 pre-existing GPU examples plus `msdf_generation_demo` (the CPU-only CI step) re-run for real against actual Vulkan hardware with validation layers enabled: zero validation errors, every pixel assertion in every demo still passing after the `pixel_at` unification and the mutex/overflow/panic fixes above.

## Phase 4 Step 4.3.1 Implementation (2026-09-06)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 4.3.1 (`SwmrSlotTable` tombstone deletion & recency tracking), the first of Step 4.3's three sub-steps -- closing the Phase 1-4 review's finding #114 (atlas LRU eviction was never built). Full detail in `planning/archive/LOG_PHASE4_STEP4_3_1.md`; this is the summary for the documentation's own record.

Status: **Complete.** This sub-step is a genuine concurrent-data-structure change, not a policy tweak: `SwmrSlotTable`'s own module doc comment had explicitly stated its lock-free-read guarantee depended on entries being "add-only -- never removed... a table that removed entries in place would need tombstones instead." Real eviction needs exactly that, so this sub-step builds it in isolation (classic tombstone deletion) before any eviction policy touches `tre-atlas` at all.

### 117. [Nice-to-have] `SwmrSlotTable::insert`'s first tombstone-reuse draft only claimed a remembered tombstone when the probe also reached a genuine `EMPTY_KEY`
The initial implementation tracked `first_available` (the earliest tombstoned-or-empty slot seen while probing) but only actually claimed it inside the `existing == EMPTY_KEY` branch. Once a table has been through enough remove/insert cycles that every slot is either a live key or a tombstone -- zero `EMPTY_KEY` slots left at all -- the probe loop runs its full `capacity` iterations without ever entering that branch, and `insert` incorrectly falls through to reporting the table full, even though a tombstoned slot was genuinely available and should have been reused. This is not a contrived edge case: it's the steady state any long-running atlas reaches once eviction has cycled through its slots even once, making it the single most likely condition a real caller would have hit first.

**Change:** restructured `insert` to check the remembered tombstone-or-empty slot once, after the probe loop ends -- whether the loop ended via an early stop at a genuine `EMPTY_KEY`, or by exhausting the full probe sequence without ever finding one. Caught immediately by a dedicated unit test (`a_slot_freed_by_remove_does_not_permanently_shrink_capacity`) built specifically for the zero-`EMPTY_KEY`-slots condition, before this code ever had a real caller.

## Summary table (Phase 4 Step 4.3.1)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 117 | `insert`'s tombstone reuse only worked when a genuine `EMPTY_KEY` was also reached, failing once a table had zero empty slots left | tre-memory | Nice-to-have | Fixed — tombstone claim now checked once after the full probe, regardless of how it ended |

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace: 12 unit tests in `tre-memory`'s `swmr` module (up from 7 before this sub-step), including a new 6-reader-thread/20,000-round-per-reader concurrent stress test confirming no reader ever observes a torn or corrupted value while a `remove` races them, matching this module's own pre-existing testing discipline. No RHI/example surface touched this sub-step -- all 15 pre-existing examples remain unaffected, since nothing in `tre-atlas`'s own code calls the new `remove`/`get_and_touch`/`scan_older_than` methods yet (Step 4.3.3's job).

## Phase 4 Step 4.3.2 Implementation (2026-09-06)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 4.3.2 (`AtlasPacker` free-rectangle reclamation), the second of Step 4.3's three sub-steps. Full detail in `planning/archive/LOG_PHASE4_STEP4_3_2.md`; this is the summary for the documentation's own record.

Status: **Complete, no findings.** Worth recording plainly, matching Phase 3 Step 3.1's and Phase 4 Step 4.2.3's own precedent for this: every design decision locked into the plan (unmerged reclamation extending Step 4.2.1's own original simplification, a `total_area` cached at construction rather than recomputed, `saturating_sub` guarding a hypothetical caller-bug underflow) held up exactly as reasoned through, with nothing surprising turning up during implementation -- a real change of pace from 4.3.1's own `insert` tombstone-reuse bug one commit earlier.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace: 16 unit tests in `tre-atlas` (up from 11 before this sub-step), including a worked exact-fill/free/refill example mirroring Step 4.2.1's own original testing style and an explicit test confirming two adjacent freed rectangles do not silently merge -- the accepted no-merge limitation as a tested property, not an unstated gap. No new example this sub-step, matching 4.3.1's own precedent (`AtlasPacker`'s own capability is proven via unit tests alone; the real end-to-end proof lands in Step 4.3.3). `atlas_packing_demo` and `atlas_concurrency_demo` re-run for real against actual Vulkan hardware as a regression check: zero validation errors, every existing assertion still passes.

## Phase 4 Step 4.3.3 Implementation (2026-09-06)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 4.3.3 (atlas LRU eviction policy & wiring), the third and closing sub-step of Step 4.3 -- wiring 4.3.1's `SwmrSlotTable` and 4.3.2's `AtlasPacker` into `AtlasOwner`'s real eviction policy, finally closing the Phase 1-4 review's finding #114. Full detail in `planning/archive/LOG_PHASE4_STEP4_3_3.md`; this is the summary for the documentation's own record.

Status: **Complete, no numbered findings.** A cold-start hazard -- a freshly-inserted entry's recency defaults to `0`, making it look maximally stale the instant a *later* insert crosses the 85% capacity threshold, evicting brand-new content before it's ever used -- was identified and designed around during this sub-step's own planning, before any code was written, rather than discovered as a defect afterward: `process_insert` stamps a fresh entry's recency to its own creation frame immediately after insertion, treating creation as an access the same way a real LRU cache does. Every design decision (the atlas/frame-number geometry chosen for the demo and unit tests, the paired `slots.remove`/`packer.remove` eviction logic) held up on the first real run with no bugs -- a real change of pace from 4.3.1's own tombstone-reuse bug, matching 4.3.2's clean run instead.

Also reconsidered during this sub-step: REVIEW.md's own finding #114 disposition had speculated "Step 4.3.3 is where a real eviction actually bumps [the packed generation counter]." On reflection, nothing in this codebase reads or depends on `generation` changing, and building the persistent per-key bookkeeping a meaningful bump would require is complexity with no current consumer -- `generation` stays `0` from every real caller, deferred indefinitely rather than merely to this step, per this project's own "don't build what the task doesn't need" discipline. Noted here since it corrects an expectation this document itself set in an earlier entry.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace: 19 unit tests in `tre-atlas` (up from 16), including three new tests precisely engineered around the real `0.85`/`600`-frame thresholds. The new capstone example, `atlas_eviction_demo` (a real 64x64 atlas exactly holding four 32x32 MSDF glyphs, a real eviction triggered by a fifth request 700 frames later, then a real GPU render of the two survivors through the unmodified `msdf.frag` pipeline), passed on its first real run with zero Vulkan validation errors. All 16 examples (the 15 pre-existing plus the new one) re-run manually end to end, zero regressions from the `request_insert`/`lookup` signature change all real callers were updated for. Added to the `vulkan-validation` CI job. This closes Step 4.3 (4.3.1-4.3.3) in full, and with it, the Phase 1-4 review's finding #114.

## Phase 5 Step 5.1.1 Implementation (2026-09-07)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 5.1.1 (Canvas Drawing-Context state stack), the first of Step 5.1's three sub-steps -- opening Phase 5. Full detail in `planning/archive/LOG_PHASE5_STEP5_1_1.md`; this is the summary for the documentation's own record.

Status: **Complete.** `RenderingCanvas` was not started from scratch -- Phase 0 already built a real stub (`draw_rounded_rect`, `push_layer`/`pop_layer`, the exact `UiDrawCommand`/`CommandType` structs); this sub-step gave it real `save`/`restore` (transform + alpha) and a separate `push_clip`/`pop_clip` scissor stack, per DESIGN.md's own architecture diagram naming them as two distinct mechanisms, wired into `draw_rounded_rect`.

### 118. [Should-fix] `draw_rounded_rect`'s alpha scaling reduced only the vertex color's alpha byte, silently making every `Canvas::set_alpha()` call invisible rather than merely imprecise
`sdf_rounded_rect.frag`'s own output formula, `vec4(frag_color.rgb * coverage, frag_color.a * coverage)`, only ever multiplies `frag_color.rgb` by the SDF's own anti-aliasing coverage term -- never by `frag_color.a`. `UiVertex::color` must therefore already be premultiplied by whatever effective alpha a caller wants before it's written, not merely carry a reduced alpha channel with RGB left at full brightness; an alpha-only reduction produces a genuinely over-bright premultiplied color once GPU blending folds in the reduced alpha, which the hardware silently clamps back to fully opaque. Caught directly by running the new `canvas_state_stack_demo`: a 50%-alpha square rendered pixel-identical to a fully opaque one.

**Change:** the scaling helper (renamed `premultiply_alpha`, from `scale_alpha`) now scales all four channels -- R, G, B, and A -- together. Documented directly on `UiVertex`'s own canonical definition in ARCHITECTURE.md Section 3.1 as a real, previously-undocumented convention any future primitive writing `color` (`draw_text`, `draw_path`, `draw_svg` -- Step 5.1.2 onward) must also follow, not something specific to this one function.

### 119. [Nice-to-have] `canvas_state_stack_demo`'s first draft checked a point that fell inside a different rect's own footprint, making the check pass regardless of whether the feature under test actually worked
The demo's transform check read back pixel `(25, 25)` to confirm Rect A's raw, untransformed local position was empty -- but `(25, 25)` sits inside Rect B's own separately-drawn footprint `(10,10)-(60,60)`, so the assertion would have found *something* there even if `Canvas::transform()` were completely broken.

**Change:** moved the check point to `(5, 5)`, confirmed clear of every other shape in the scene.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace: 20 unit tests in `tre-engine` (up from 12). `canvas_state_stack_demo` proves what actually reaches the GPU with real pixels (a transformed world-space placement, a genuine visible partial-alpha blend) and checks `push_clip`/`pop_clip` at the IR level only, since nothing in the render pipeline consumes `clip_bounds` yet (Step 5.1.3/Phase 6's job) -- not a GPU scissor test that doesn't exist yet. `sdf_rounded_rect_demo` re-run manually, confirming the rewired logic is a genuine no-op at the default identity/full-alpha/no-clip state. All 17 examples (16 pre-existing plus the new one) re-run manually end to end, zero validation errors. Added to the `vulkan-validation` CI job.

## Summary table (Phase 5 Step 5.1.1)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 118 | Alpha-only vertex color scaling made `set_alpha()` invisible due to `sdf_rounded_rect.frag`'s premultiplied-alpha output formula | tre-engine | Should-fix | Fixed — all four channels now scaled together (`premultiply_alpha`); convention documented on `UiVertex` |
| 119 | Demo's own transform-check point fell inside a different rect's footprint, making the check vacuous | tre-rhi-vulkan (example code) | Nice-to-have | Fixed — check point moved to a genuinely clear location |

## Phase 5 Step 5.1.2 Implementation (2026-09-07)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 5.1.2 (`Canvas::draw_text`), the second of Step 5.1's three sub-steps -- `tre-engine`'s first real wiring into `tre-text`/`tre-atlas`. Full detail in `planning/archive/LOG_PHASE5_STEP5_1_2.md`; this is the summary for the documentation's own record.

Status: **Complete, no numbered findings.** Every design decision locked into `PLAN.md` (borrowed, never owned, atlas/font resources; whitespace filtered by outline emptiness rather than a source-text lookup; cache miss firing a real `request_insert` and rendering nothing that frame; fixed `px_size`-square glyph quads, the same simplification `atlas_concurrency_demo` already uses) held up unchanged through implementation, matching Phase 4 Step 4.3.2's own clean-run precedent -- the whole feature compiled, passed clippy pedantic, and passed every new unit test and the new GPU demo on the first real run.

One refinement made during implementation, not anticipated in the plan's own sketched signature: `draw_text`'s cache-hit path needs the shared atlas's own pixel dimensions to normalize a `PackedRect` into UV coordinates, which `PLAN.md`'s flat parameter list omitted. Rather than adding a fifth loose parameter, the four atlas-related values (`atlas`, `texture_handle`, `dimensions`, `current_frame`) were bundled into one new `GlyphAtlasContext<'a>` struct -- the same kind of grouping the plan's own "exact grouping left TBD" note had flagged as a live-but-undecided question, resolved once the real signature was in front of the actual call sites.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace: 4 new unit tests in `tre-engine` (24 total, up from 20), each against a real system font and a real `AtlasOwner` background thread rather than a synthetic stub. The new capstone example, `canvas_draw_text_demo`, proves both halves of `draw_text`'s cache contract with a real word ("TEXT") end to end -- a first call is a genuine cache miss for every glyph (zero commands), a second call after real background resolution is a genuine cache hit for every glyph (one real textured command each), rendered through the existing, unmodified `bindless_textured.vert`/`msdf.frag` pipeline and read back as real, non-background GPU pixels per glyph. `atlas_concurrency_demo`/`atlas_eviction_demo` (the two demos touched by promoting `GlyphRasterSource` out of their own duplicated local copies into `tre_text::GlyphRasterSource`) re-run manually end to end, zero regressions, their own existing assertions all still passing. Added to the `vulkan-validation` CI job.

## Summary table (Phase 5 Step 5.1.2)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| -- | No numbered findings this sub-step; one plan-vs-implementation refinement (atlas dimensions needed for UV math, not in the original signature sketch) | tre-engine | -- | Resolved by bundling atlas context into `GlyphAtlasContext` |

## Phase 5 Step 5.1.3 Implementation (2026-09-07)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 5.1.3 (real sort key, `begin_overlay`/`end_overlay`, real batch flattening), the third and closing sub-step of Step 5.1 -- the capstone. Full detail in `planning/archive/LOG_PHASE5_STEP5_1_3.md`; this is the summary for the documentation's own record.

Status: **Complete.** Every core design decision locked into `PLAN.md` (markers as hard sort/merge barriers, a single global monotonic `next_depth_id`, `OverlayLayerPriority` as a concrete absolute offset, index-buffer rewriting rather than command reordering alone) held up unchanged through implementation -- the sort key, overlay routing, and flattening logic itself compiled and passed every new unit test on the first real run. The two findings below are not bugs in the new logic; they are pre-existing files whose own test assumptions the new, correct merging behavior genuinely invalidated -- exactly the class of regression `PLAN.md`'s own "audit existing tests/demos" task anticipated, one instance of which it named specifically in advance.

### 120. [Nice-to-have] `canvas_state_stack_demo.rs`'s `RECT_C_COMMAND_INDEX` assumed Rect A and Rect B could never merge into one command
Rect A and Rect B (both drawn before any `push_clip`, both default Layer/Pipeline/Texture) share identical Layer+Pipeline+Texture+`clip_bounds` with nothing between their two `draw_rounded_rect` calls -- real batch flattening now merges them into one `DrawGeometry` command, shifting every later command's index down by one. `PLAN.md` itself predicted this exact case in advance (Task 8), so this was a planned fix applied during implementation, not a surprise discovered afterward.

**Change:** `RECT_C_COMMAND_INDEX` updated from `3` to `2`, comment rewritten to describe the new command sequence.

### 121. [Nice-to-have] `canvas_draw_text_demo.rs`'s frame-2 assertion assumed one command per glyph, the same class of gap `PLAN.md` anticipated generically but did not name specifically
All 4 glyphs of "TEXT" share Layer 0, `PIPELINE_MSDF_TEXT`, the same atlas `texture_handle`, and the same full-window `clip_bounds` -- real batch flattening merges all 4 into a single command (`element_count == 24`). Caught by re-running every pre-existing example during this step's own verification pass, per `PLAN.md`'s "audit existing tests/demos for the new merging behavior" task.

**Change:** the assertion now expects exactly 1 merged command with `element_count` equal to `glyphs.len() * 6`; the doc comment and `eprintln!` wording updated to describe merging rather than a one-per-glyph mapping.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace: 36 unit tests in `tre-engine` (up from 24), including a direct reproduction of DESIGN.md Section 8's own worked example and a white-box test of `flatten_run`'s `clip_bounds` check (unreachable via the public API today, kept as a forward-looking regression guard). The new capstone example, `canvas_batch_flattening_demo`, passed on its first real run: exactly 3 batches at the IR level, and a real GPU render (the first in this codebase to record more than one `draw_indexed` call and switch pipelines within a single frame) confirming all 4 logical shapes still render at their own correct, distinct positions. Every one of the (now 19) pre-existing examples re-run manually end to end -- only the two named above needed a real code change, zero further regressions. Added to the `vulkan-validation` CI job. This closes Step 5.1 (5.1.1-5.1.3) in full.

## Summary table (Phase 5 Step 5.1.3)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 120 | `canvas_state_stack_demo.rs`'s `RECT_C_COMMAND_INDEX` assumed no merging between Rect A and Rect B | tre-rhi-vulkan (example code) | Nice-to-have | Fixed — constant updated `3` -> `2`, comment rewritten |
| 121 | `canvas_draw_text_demo.rs`'s frame-2 assertion assumed one command per glyph | tre-rhi-vulkan (example code) | Nice-to-have | Fixed — assertion now expects exactly 1 merged command |

## Phase 5 Step 5.2.1 Implementation (2026-09-07)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 5.2.1 (`SubCanvas` and thread-local recording), the first of Step 5.2's three sub-steps -- opening the engine's real multi-threaded recording work. Full detail in `planning/archive/LOG_PHASE5_STEP5_2_1.md`; this is the summary for the documentation's own record.

Status: **Complete, no numbered findings.** Every design decision locked into `PLAN.md` (`SubCanvas` as a thin `Deref`/`DerefMut` wrapper rather than a duplicated API; only Depth ID promoted to a shared `Arc<AtomicU32>`, Layer ID staying thread-local; the concurrency cap enforced via a compare-exchange loop plus a `Drop`-based release exact even under a caught panic; a `#[cfg(test)]`-only cap override for deterministic testing) held up unchanged through implementation -- the whole feature compiled, passed clippy pedantic, and passed every new unit test, including the real 4-thread concurrent stress test, on the first run.

One small, disclosed simplification carried over from the design phase into the actual code comment: `next_sort_key`'s own `# Panics` section previously warned about `next_depth_id` overflowing a raw `u32` (4 billion calls); switching to `AtomicU32::fetch_add` drops that check entirely, since atomic addition wraps rather than panics on overflow -- correctly reasoned as harmless in `PLAN.md`, since the real, far lower 20-bit Depth ID threshold `compute_sort_key` already checks was always the meaningful bound, not the raw counter's own wrap point four orders of magnitude higher.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace: 41 unit tests in `tre-engine` (up from 36). The centerpiece is a real concurrency test, not a simulated one: 4 real `std::thread::spawn` threads, each holding its own `SubCanvas`, each recording 50 `draw_rounded_rect` calls, with every one of the 200 resulting Depth IDs collected back on the main thread and confirmed pairwise distinct -- the same "real threads under genuine stress, not a single-threaded stand-in" rigor `tre-memory`'s own `MpscRingBuffer` test and `atlas_concurrency_demo` already established. No new demo this sub-step (a `SubCanvas` has no way to be rendered without Step 5.2.2's merge primitive), matching Steps 4.3.1/4.3.2's own precedent of unit-tests-only sub-steps ahead of a capstone. All 19 pre-existing examples re-run manually end to end, zero regressions, confirming `next_sort_key`'s internal `fetch_add` change is behaviorally identical to the old plain increment for every existing single-threaded caller.

## Summary table (Phase 5 Step 5.2.1)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| -- | No numbered findings this sub-step | tre-engine | -- | -- |

## Phase 5 Step 5.2.2 Implementation (2026-09-07)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 5.2.2 (the real lock-free stitching primitive and wiring it into a multi-source `flatten`), the second of Step 5.2's three sub-steps. Full detail in `planning/archive/LOG_PHASE5_STEP5_2_2.md`; this is the summary for the documentation's own record.

Status: **Complete, no numbered findings.** Every design decision locked into `PLAN.md` (workers stitch themselves rather than a sequential coordinator-only merge; `ScatterArena<T>` following `MpscRingBuffer`'s own `UnsafeCell<MaybeUninit<T>>` pattern rather than a new one; copy-and-rebase instead of a literal `copy_from_slice`; `std::mem::take` to extract a `Drop`-implementing `SubCanvas`'s inner data; reusing Step 5.1.3's sort/merge logic via a shared `segment_and_flatten` function) held up unchanged through implementation -- the whole feature, including the two real concurrency stress tests, compiled and passed clippy pedantic on the first run. Two small, purely mechanical clippy fixes surfaced (a `needless_pass_by_value` on `segment_and_flatten`'s `indices` parameter once it became borrowed rather than owned, and a stray `drop()` call on a type with no `Drop` impl in a `ScatterArena` test) -- neither reflects a design issue, both were one-line fixes.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace: `tre-memory` gained 5 new tests (30 total, up from 25), including a real 8-thread/4,000-item concurrent stress test for `ScatterArena` confirming zero data corruption or overlap across all reserved ranges. `tre-engine` gained 3 new tests (44 total, up from 41): an exact-value rebasing check (two sequential sub-canvases' rects stitched into one arena, hand-verifying the second source's indices shifted by precisely the first source's vertex count -- `[0,1,2,0,2,3,4,5,6,4,6,7]`), an arena-overflow check (`stitch_into` reports `false` rather than corrupting anything when the destination is too small), and the centerpiece: a real 4-worker-thread test in which each thread stitches its own `SubCanvas` into one shared `FrameArena` as its own last action before exiting, with the resulting frame's 16 vertices and single merged 24-index command both confirmed correct regardless of which thread's reservation happened to land first. No new demo this sub-step, matching 5.2.1's own precedent -- the real end-to-end GPU proof (real worker threads, real merged render, real pixel readback) is Step 5.2.3's capstone job. All 19 pre-existing examples re-run manually end to end, zero regressions from the `flatten()` refactor.

## Summary table (Phase 5 Step 5.2.2)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| -- | No numbered findings this sub-step | tre-memory, tre-engine | -- | -- |

## Phase 5 Step 5.2.3 Implementation (2026-09-07)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 5.2.3 (the capstone: real concurrent recording, rendered), the third and closing sub-step of Step 5.2. Full detail in `planning/archive/LOG_PHASE5_STEP5_2_3.md`; this is the summary for the documentation's own record.

Status: **Complete.** Two real issues surfaced during the demo's own first-draft development, both caught before any commit -- exactly the kind of thing this capstone's real-stress approach exists to catch, unlike a demo that only exercises the happy path once.

### 122. [Should-fix] The demo's own first draft used a second, independent `RenderingCanvas` for the root's rect, whose separate Depth ID counter collided with a worker's
`root_canvas.draw_rounded_rect(...)` was called on a fresh `RenderingCanvas::new()`, distinct from the `root` instance `create_sub_canvas()` was called on -- meaning the root's own rect drew its Depth ID from a completely different counter than every worker's. Two commands (the root's rect and whichever worker happened to also land on depth `0` from its own counter) then shared an identical sort key, which `sort_unstable_by_key` has no defined tie-breaking behavior for -- observed directly as a real GPU render with 4 batches instead of the expected 3, one of them a plain rect that should have merged with the others but didn't.

**Change:** draw the root's own rect directly on `root` (the same instance every `create_sub_canvas()` call already borrows and shares a Depth ID counter with), and defer `root.stitch_into(&arena)` (which consumes it by value) until after every `create_sub_canvas()` borrow is done.

### 123. [Nice-to-have] The demo's first verification draft asserted an exact batch count that concurrent stitching cannot actually guarantee
`canvas_batch_flattening_demo` (Step 5.1.3, one single-threaded call sequence) always collapses to exactly 3 batches, and the first draft of this demo asserted the same. But the overlay worker's `begin_overlay`/`end_overlay` markers are hard run-segmentation barriers (Step 5.1.3's own design), and *which* other threads' plain rects land before vs. after that marker pair in the concurrently-stitched command array depends on real thread scheduling -- observed directly: repeated runs on the same 24-core development machine produced anywhere from 1 to 2 separate batches for the same 4 plain rects. This is ARCHITECTURE.md Section 4.2's own documented "soft target, not a guarantee" caveat, genuinely exercised for the first time by a demo (every earlier demo's recording was single-threaded, where this variability cannot occur).

**Change:** the assertion now checks what real batch flattening actually guarantees regardless of scheduling -- every plain rect's content (its own 6 indices) is accounted for *somewhere*, however many pieces it was split into, and the overlay rect and the text glyph each always remain their own, never-merging batch -- rather than a fixed total batch count.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace, plus 15+ real runs of the new demo on real hardware confirming the batch-count variability directly (not merely reasoned about) while every pixel and content assertion held on every single run. All 18 pre-existing examples re-run manually end to end, zero regressions. Added to the `vulkan-validation` CI job. **This closes Step 5.2 (5.2.1-5.2.3) in full.**

## Summary table (Phase 5 Step 5.2.3)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 122 | Demo's own second, independent root canvas had a Depth ID counter that collided with a worker's | tre-rhi-vulkan (example code) | Should-fix | Fixed — root's rect now drawn on and stitched from the same shared-counter canvas |
| 123 | Demo asserted an exact batch count concurrent stitching cannot actually guarantee | tre-rhi-vulkan (example code) | Nice-to-have | Fixed — assertion now checks content/hard-guarantees instead of a fixed count |

## Phase 5 Step 5.3.1 Implementation (2026-09-07)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 5.3.1 (`Canvas::tag_accessibility_node` and its IR-adjacent data), the first of Step 5.3's three sub-steps. Full detail in `planning/archive/LOG_PHASE5_STEP5_3_1.md`; this is the summary for the documentation's own record.

Status: **Complete, no numbered findings.** Every design decision locked into `PLAN.md` (four transformed corners reduced to a real axis-aligned bounding box rather than `draw_rounded_rect`'s vertex-only corner transform; `f32` bounds with no rounding to an OS-native integer convention yet; no clip-stack intersection; a small 4-variant starter `AccessibilityRole` rather than AT-SPI2's full taxonomy; a flat per-frame node list with no engine-built tree; mandatory `SubCanvas`/`FrameArena`/`stitch_into` integration this sub-step, not deferred) held up unchanged through implementation. The only code changes needed beyond the new feature itself were three pre-existing test call sites and one demo call site updated for `FrameArena::with_capacity`'s new fourth parameter -- mechanical, not a design issue.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace: `tre-engine` gained 4 new tests (48 total, up from 44) -- a pure-translation sanity check, a rotation-correctness test proving the real axis-aligned bounding box of four rotated corners rather than the naive untransformed rect, a `SubCanvas` + `stitch_into` carry-through check, and a real 4-worker-thread test extending Step 5.2.3's own capstone pattern with per-thread tagging. No new demo this sub-step, matching 5.2.1/5.2.2's own precedent -- tagged data has nowhere real to go until 5.3.2's OS bridge exists; the real end-to-end proof is deferred to the 5.3.3 capstone. All 4 examples touching the shared `flatten`/`stitch_into`/`FrameArena` code path re-run manually end to end, zero regressions.

## Summary table (Phase 5 Step 5.3.1)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| -- | No numbered findings this sub-step | tre-engine | -- | -- |

## Phase 5 Step 5.3.2 Implementation (2026-09-08)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 5.3.2 (the real Linux AT-SPI2 bridge), the second of Step 5.3's three sub-steps. Full detail in `planning/archive/LOG_PHASE5_STEP5_3_2.md`; this is the summary for the documentation's own record.

Status: **Complete, no numbered findings.** `PLAN.md`'s core approach (a new `tre-a11y` crate built on `accesskit`/`accesskit_unix` rather than hand-rolled `zbus`) held up completely. One deliberate, disclosed design correction was made *during* the plan's own Task 1 research step, not discovered as a defect afterward: reading `accesskit_unix::Adapter`'s real source revealed it already owns a dedicated background thread and an unbounded internal channel, making the plan's own speculative "hand-roll a second background thread with a bounded channel" unnecessary complexity -- removed before any code was written against it, not built and then reverted. Every other real API detail the plan deliberately left open (the `accesskit::Rect` coordinate convention, `Tree`'s `app_name`/`toolkit_name`/`toolkit_version` fields, `Node::set_bounds`/`set_children`, the `/org/a11y/atspi/accessible/<adapter>/<node_id>` object path scheme) was confirmed against `accesskit`/`accesskit_unix`/`accesskit_atspi_common`'s real source before writing the conversion code, and matched what the real, live AT-SPI2 stack on this development machine actually returned on the first genuine round-trip test.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace: the new `tre-a11y` crate ships 6 tests, including one real integration test that discovers the published application in the actual system AT-SPI2 registry (not a mock) and confirms `Component.GetExtents` matches exactly what was published. CI's `test` job was extended to install `dbus-user-session`/`at-spi2-core` and wrap `cargo test --workspace` in `dbus-run-session`, relying on `org.a11y.Bus`'s own standard D-Bus service-activation mechanism -- genuinely unconfirmed on the actual hosted runner as of this writing (this machine's own already-running desktop accessibility stack is what the local proof used), disclosed rather than assumed to work; the round-trip test itself is designed to skip gracefully, not fail, if CI's bus turns out unreachable. No new `tre-rhi-vulkan` demo this sub-step, matching 5.2.1/5.2.2/5.3.1's own precedent and DESIGN.md Section 5's "(Decoupled from Rendering)" framing -- the real end-to-end capstone wiring this bridge into a rendered scene is Step 5.3.3's job.

## Summary table (Phase 5 Step 5.3.2)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| -- | No numbered findings this sub-step | tre-a11y, .github/workflows/ci.yml | -- | -- |

## Phase 5 Step 5.3.3 Implementation (2026-09-08)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 5.3.3 (the capstone -- a real rendered scene, verified live), the third and closing sub-step of Step 5.3. Full detail in `planning/archive/LOG_PHASE5_STEP5_3_3.md`; this is the summary for the documentation's own record.

Status: **Complete.** Two real issues surfaced while building the new demo's own first draft, both caught before any commit -- exactly the kind of thing a real, combined end-to-end capstone exists to catch, unlike three isolated tests that each only ever exercised their own layer.

### 124. [Should-fix] `tre-a11y`'s `AccessibilityRole::Generic -> accesskit::Role::GenericContainer` mapping (Step 5.3.2) made every `Generic`-tagged node invisible to real assistive technology
`accesskit_consumer::common_filter` (the filter `accesskit_atspi_common` uses to decide what actually reaches the platform accessibility tree) hard-codes `Role::GenericContainer`/`Role::TextRun` as always excluded -- the real accesskit equivalent of ARIA's `role="none"`/`"presentation"`, a "hide this from assistive technology" signal, not "a generic taggable element." Step 5.3.2's own tests never caught this because its round-trip test tagged a `Button`, never a `Generic` node, against a live bus. This capstone's own first-draft scene tags one `Generic` rect among three, and its real AT-SPI2 `Registry`-side `GetChildren` on the synthesized root returned 2 children instead of 3 -- observed directly, not reasoned about in the abstract.

**Change:** `tre-a11y`'s `map_role` now maps `AccessibilityRole::Generic -> Role::Unknown` (confirmed via `accesskit_consumer::common_filter`'s own real source to be the correct, unfiltered choice -- `Unknown` is this enum's own `#[default]` variant and is not special-cased anywhere in the filter). `tre-a11y`'s own existing unit test updated to match.

### 125. [Nice-to-have] The demo's own first-draft verification hung forever, instead of failing, once an assertion inside a `thread::scope` actually failed
While the regression above was still unfixed, the demo's own `thread::scope` block (main thread verifying while a background thread steadily republishes) panicked on the failing node-count assertion -- but `thread::scope`'s own contract requires every spawned thread to be joined before the scope returns, even while unwinding, and the spawned "keep publishing" thread's stop flag (a plain `AtomicBool`) was only ever cleared on the success path at the very end of the closure. The whole process hung indefinitely instead of reporting the real failure, discovered only by manually observing the process was still running via `ps`/`/proc/<pid>/wchan` well past a 120-second command timeout.

**Change:** an RAII guard (`StopOnDrop`) now clears the flag when the scope's closure exits for any reason, panic included, so a real assertion failure now reports promptly instead of hanging the session.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace, plus 3 consecutive real runs of the new demo against this development machine's own live AT-SPI2 stack, all passing: every rect's real `Component.GetExtents` matches its own real IR bounds exactly, including the rotated rect's real axis-aligned bounding box, and all 3 rects' distinct roles survive as 3 pairwise-distinct real AT-SPI2 roles. `demo/phase5_step5_3_3/` added. `vulkan-validation`'s CI job gained `dbus-user-session`/`at-spi2-core`, with only the one new demo's run line wrapped in `dbus-run-session -- xvfb-run -a ...`. All pre-existing examples re-run manually, zero regressions from `tre-rhi-vulkan`'s new `tre-a11y`/`zbus` dev-dependencies. **This closes Step 5.3 (5.3.1-5.3.3) in full.**

## Summary table (Phase 5 Step 5.3.3)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 124 | `AccessibilityRole::Generic` mapped onto an `accesskit::Role` that is always filtered out of the real platform tree | tre-a11y | Should-fix | Fixed — remapped to `Role::Unknown`, confirmed unfiltered via the real filter source |
| 125 | Demo's own verification hung forever instead of failing when an assertion panicked inside a `thread::scope` | tre-rhi-vulkan (example code) | Nice-to-have | Fixed — an RAII guard now always clears the publisher thread's stop flag on scope exit |

## Phase 5 Step 5.3.3 CI Verification (2026-09-08)

Discovered while checking the actual GitHub-hosted `vulkan-validation` run for the first time after pushing Step 5.3.3 (the standing "check CI via `gh run watch` after every push" process, applied here to a job whose own docs had explicitly flagged its D-Bus wiring as "genuinely unconfirmed on the actual hosted runner"). Real evidence, not speculation: `org.a11y.Bus`/`org.a11y.atspi.Registry` both D-Bus-activated successfully on the runner (the core wiring from Step 5.3.2 works), but `canvas_accessibility_demo` panicked with "our app never appeared in the real AT-SPI2 registry within 10s." Comparing timestamps against the same run's `test` job showed its own round-trip test had passed, but only by finding the app at the very edge of its own 10-second deadline (10.05s of a 10s budget) -- not a comfortable margin.

### 126. [Should-fix] A freshly D-Bus-activated `at-spi2-registryd` on a hosted CI runner starts with `org.a11y.Status.IsEnabled` false, racing every 10-second discovery timeout
Unlike a developer's own real desktop (where a running session already sets `IsEnabled` true well before any test runs), nothing on a fresh, headless `dbus-run-session` ever flips it -- `accesskit_unix`'s adapter only activates (embeds into the registry) once it observes that property become true. The `test` job's round-trip test happened to still pass because activation apparently completes just under 10 seconds even from cold, but `canvas_accessibility_demo` -- running later in a job under heavier load (many prior Vulkan examples, `xvfb-run` overhead) -- missed the same race outright.

**Change (original, since disproved by direct evidence -- see Correction below):** both the `test` and `vulkan-validation` CI jobs explicitly set `org.a11y.Status.IsEnabled` to `true` via a real `dbus-send org.freedesktop.DBus.Properties.Set` call, inside the same `dbus-run-session` invocation, before running anything that depends on it.

**Correction (same day, after the next real push):** the `IsEnabled`-setting fix had zero measurable effect. A second real CI run, with the fix in place, showed `test`'s own round-trip test still taking exactly ~10.06s -- statistically identical to the *first* run's ~10.05s, measured *without* the fix. Directly comparing the two real runs' own timestamps proves `IsEnabled` was never the actual bottleneck; something else on a freshly-activated `at-spi2-registryd` consistently takes ~10 seconds regardless of that property's value, and the true root cause was not conclusively identified (candidate causes considered: `accesskit_unix`'s own `receive_is_enabled_changed()` stream possibly waiting for a genuine change signal rather than an initial value; some other fixed daemon-startup delay). Local reproduction attempts were inconclusive for a different reason: this development machine's own real, already-running desktop session (with ~29 real accessible applications already registered) makes it an unreliable proxy for a clean CI container's D-Bus environment either way. **Interim fix:** widen `tre-a11y`'s own round-trip test's and `canvas_accessibility_demo`'s discovery timeout from 10s to 30s. The now-proven-ineffective `dbus-send`/`IsEnabled` CI step was removed rather than left in place as misleading dead weight. The demo's own `a11y_bus()` call was also reordered to run before `A11yBridge::connect` (matching `tre-a11y`'s own already-working test's exact call order) as a harmless, reasonable defensive measure, though it was directly ruled out as the actual cause on this machine (the failure persisted even with `a11y_bus()` moved to the very first line of `main`, before any Vulkan/window setup at all).

**Second correction (same day, after a third real push):** the 30s timeout was still not enough -- `canvas_accessibility_demo` in `vulkan-validation` timed out completely (a real, full 30s timeout, not a near-miss), while `test`'s own round-trip test (same 30s bound, unrelated job) again succeeded at ~10.05s. This rules out "needs a slightly longer timeout" entirely: the discovery either completes in ~10s or, in `vulkan-validation` specifically, never completes at all. The one real, load-bearing difference between the two jobs: `vulkan-validation` runs 20 other CPU-bound, software-rendered (lavapipe) Vulkan demos before ever reaching this one, on a shared, resource-constrained hosted runner, while `test` runs nothing else CPU-heavy first. **Interim fix:** moved `canvas_accessibility_demo` out of `vulkan-validation` entirely into its own new, minimal `accessibility-validation` job -- installing the same Vulkan/D-Bus packages but running only this one command, mirroring the exact "lightweight, uncontended job" shape that has succeeded reliably in `test`. `vulkan-validation` itself reverts to installing only what its own remaining 20 demos need (no D-Bus packages).

**Third correction (same day, after a fourth real push):** the isolated job *still* timed out completely at 30s with nothing else running first, disproving CPU contention too. **Fourth correction (fifth push):** decoupling `xvfb-run`'s own wrapper script from `dbus-run-session` (starting Xvfb directly instead) made no difference either, disproving wrapper nesting. **Fifth correction (sixth push):** reordering the demo so accessibility publish/verify ran *before* any Vulkan/window setup at all also made no difference -- the registry stayed completely empty for the full 30s regardless, disproving call ordering and runtime Vulkan/X11 usage as the cause.

**A sixth correction (seventh push):** `tre-a11y`'s own already-reliable round-trip test (zero Vulkan/X11 code of its own) was added as an extra diagnostic step inside `accessibility-validation`'s own job -- same packages installed, same `DISPLAY` set, completely unrelated binary. That identical test took **30.06 seconds** there, versus its usual ~10.05s in the plain `test` job. This LOOKED like the answer -- "the environment is just slower with Xvfb/Vulkan installed" -- so both discovery timeouts were widened from 30s to 60s on this evidence.

**The real, final root cause (eighth push):** the 60s-timeout fix was itself disproven immediately -- in the very same workflow run where the diagnostic `tre-a11y` test had just succeeded at 30.06s, `canvas_accessibility_demo`'s own next step, in the identical job, timed out completely at a full 60s (0 registry entries the entire time). Since both ran back to back in the same job, "the environment is uniformly slower" cannot be the explanation -- `tre-a11y`'s test can succeed there while the demo never can, no matter the timeout. The real, load-bearing difference is the binary itself: `canvas_accessibility_demo` links real Vulkan/X11 shared libraries (`ash`/`x11rb`, for its own real GPU rendering); `tre-a11y`'s test does not. `accesskit_unix`'s own background thread -- which performs the actual AT-SPI2 registration -- simply cannot complete that registration inside a process that also has Vulkan/X11 linked, regardless of how long it's given.

**Real, final fix:** a genuine two-process split, not another timeout adjustment. `canvas_accessibility_demo` now only renders and tags -- it writes its tagged `AccessibilityNode`s to a small plain-text handoff file and exits, never touching `tre_a11y`/AT-SPI2 at all. A new `canvas_accessibility_verify` binary -- confirmed via `ldd` to link zero Vulkan/X11/Wayland libraries -- reads that file, performs the real `A11yBridge::connect`/`publish` (the actual AT-SPI2 registration now happens in a clean process), and verifies via a second, independent D-Bus connection, mirroring `tre-a11y`'s own proven-reliable round-trip test exactly. This also happens to match real AT-SPI2 practice more closely than the original self-verifying design: a real screen reader is always a separate process from the application it inspects. `demo/phase5_step5_3_3/run_canvas_accessibility_demo.sh` and `.github/workflows/ci.yml`'s `accessibility-validation` job both updated to run the two binaries in sequence, each job/step only installing what it actually needs (Xvfb/Vulkan for the render step, D-Bus/AT-SPI2 for the verify step).

**A seventh correction (ninth push):** the two-process split above was itself disproven -- `canvas_accessibility_verify`, confirmed via `ldd` to link zero Vulkan/X11/Wayland libraries, failed in CI identically to the original single-process demo (registry completely empty for the full 30s). This directly falsifies the "Vulkan/X11 linking breaks the same process's own registration" theory: a genuinely Vulkan-free process has the exact same problem. At the user's own direction, real upstream research (reading `at-spi2-core`'s actual source, not another blind guess) resolved it: `at-spi-bus-launcher.c`'s `on_event_listener_registered` function shows `org.a11y.Status.IsEnabled` flips true *only* when `at-spi2-registryd` emits a real `EventListenerRegistered` D-Bus signal, which only happens when some real client calls `org.a11y.atspi.Registry.RegisterEvent`. Neither `accesskit_unix` nor `accesskit_atspi_common` ever call this themselves (confirmed by grepping their real source for it -- no matches). A real desktop session already has some component that has done this at some point in its life; a fresh CI container has nothing that ever does, so `IsEnabled` never flips and `accesskit_unix`'s own adapter -- which only activates upon *observing* that transition -- waits forever, regardless of which process it runs in or how long it's given.

**Real fix:** both `tre-a11y`'s round-trip test and `canvas_accessibility_verify` now call `Registry.RegisterEvent` themselves and confirm `IsEnabled` reads true before proceeding -- exactly what a real assistive technology does on startup, so this is not a workaround but the verifier honestly playing the AT role it already occupies by querying the tree at all. Disclosed honestly rather than overclaimed: local verification on this specific development machine remains inconclusive for an apparently *separate*, unexplained reason (a connection-level failure at the initial bus connect itself, a different symptom than CI's empty-registry timeout, most plausibly residue from this same session's own extensive earlier manual D-Bus experimentation on this machine) -- `tre-a11y`'s own test continues to succeed reliably throughout, but a from-scratch binary in `tre-rhi-vulkan` does not, despite identical connection code and zero linked-library differences confirmed via `ldd`. The real, decisive confirmation is the next actual CI run, not this local machine.

**An eighth correction (tenth push):** the "real fix" above was itself disproven -- calling `RegisterEvent` once, then a single 10s/30s wait, was replaced with retrying the real call itself every 200ms for up to 30s (reasoning: `EventListenerRegistered` is a fire-and-forget signal, so a single call made before `at-spi-bus-launcher`'s own subscription is active would simply be lost forever). The very next real CI run left `IsEnabled` false for the entire 30s despite roughly 150 real, distinct `RegisterEvent` calls.

**A ninth correction (eleventh push, user-approved diagnostic-only step):** rather than guess again, a CI step that attempts no fix at all was added at the user's explicit direction, to capture real evidence during a real failing run: the live D-Bus wire traffic (`dbus-monitor --address`), the real process list (`ps aux`), and the real set of connected clients on the a11y bus (`busctl list`). This proved the `RegisterEvent` → `EventListenerRegistered` mechanism genuinely works exactly as the upstream source predicts: `at-spi2-registryd` (`:1.2`) responds to every one of our ~150 real calls with a real broadcast `EventListenerRegistered` signal. But `busctl list` showed only two connected clients on the a11y bus the entire time -- `:1.2` (`at-spi2-registryd`) and `:1.3` (`busctl`, our own query) -- `at-spi-bus-launcher` itself, confirmed alive via `ps aux` (PID 5819, having correctly spawned the a11y bus's own `dbus-daemon` at PID 5825 with the right config file and address), never once appears as a connected client on the very bus it launched. No amount of retrying `RegisterEvent` from our side can matter if the one component that would act on it is never listening at all.

**The real mechanism (read directly from `at-spi-bus-launcher.c`'s own real upstream source, not another guess):** exactly one function, `ensure_a11y_bus()`, ever installs the `EventListenerRegistered` subscription that lets `IsEnabled` flip -- `g_dbus_connection_signal_subscribe` on a *second*, self-made `GDBusConnection` that the launcher opens to the very a11y bus it just spawned. It runs at most once per launcher process, guarded by its own `a11y_bus_pid` field, and is reached one of two ways: eagerly from `on_name_acquired`, but *only* when the process was started with the `--launch-immediately` command-line flag -- which D-Bus's own service-activation mechanism (the path that starts `at-spi-bus-launcher` in every job here) never passes; or lazily from `handle_method_call`'s `GetAddress` handler, the only call site that actually fires in this environment, triggered by the very first `GetAddress` query and run synchronously to completion *before that method call even replies*. Inside it, after a blocking read of the freshly spawned `dbus-daemon`'s own `--print-address` pipe confirms it is genuinely listening, `ensure_a11y_bus` makes its own `g_dbus_connection_new_for_address_sync` call to that address -- and if that call fails for any reason, the only consequence is one `g_critical` log line: no retry, no crash. The launcher keeps running exactly as `ps aux` showed it, permanently unsubscribed. This also directly rules out the `gsettings-desktop-schemas` theory an earlier web search raised: a failed schema lookup calls `g_error()`, which aborts the whole process immediately -- incompatible with `ps aux` showing `at-spi-bus-launcher` alive throughout every diagnostic run, so schema lookup is not the failure.

One real gap this doesn't yet resolve: `tre-a11y`'s own round-trip test exercises this identical code path (same `GetAddress` call, same lazy `ensure_a11y_bus()` trigger) inside the separate `test` job and has passed in every CI run so far -- so whatever makes the self-connect fail is not universal to this runner image, only to how/where it's triggered in `accessibility-validation`'s own job. The one piece of evidence still missing is `at-spi-bus-launcher`'s own stderr at the moment of that self-connect attempt: a service reached purely through D-Bus's own activation mechanism does not have its stdio captured by anything in the diagnostic step run so far, so a real or silent failure there looks identical from the outside. **Next diagnostic prepared, not yet pushed:** launch `at-spi-bus-launcher --launch-immediately` directly ourselves, with `G_MESSAGES_DEBUG=all` and its own stdout/stderr redirected to a file we control, so its real log output around this exact call finally becomes visible.

**A tenth correction (twelfth push):** the direct-stderr capture above ran (`G_MESSAGES_DEBUG=all`, own connection redirected to a file). Real findings: `at-spi-bus-launcher` (PID 3077, this run) logs cleanly through its entire startup -- `_g_io_module_get_default: Found default implementation dconf (DConfSettingsBackend) for "gsettings-backend"`, both GSettings watches established, `Launched a11y bus, child is 3084`, `a11y bus address: unix:path=/run/user/1001/at-spi/bus,guid=...`. This **definitively rules out the `gsettings-desktop-schemas` theory**, not just by inference this time but directly: the schema backend was found and used without issue. Then: nothing. No further line at all -- not `ensure_a11y_bus`'s `g_critical` failure message (which prints unconditionally, not gated by `G_MESSAGES_DEBUG`), not any indication of success either (there is no log statement on the success path in the real source). `busctl list` at the end again shows `at-spi-bus-launcher` never became a connected client -- this time in total isolation, with `at-spi2-registryd` never even activated (nothing in this script calls `RegisterEvent`), so the finding can't be attributed to any interaction with it.

Disclosed honestly rather than overclaimed: this diagnostic has a real gap in its own coverage. `ps -p "$LAUNCHER_PID"` was checked exactly once, right after the initial startup sleep and *before* the self-connect would even run -- never again immediately before the final `busctl list`. The process did stay responsive long enough to answer our own separate `GetAddress` query afterward, which argues against a long hang inside the synchronous connect call, but does not rule out the process dying (e.g. a hard crash bypassing GLib's own `g_critical` logging entirely) at some point after reading the bus address. Silent failure, silent success invisible to `busctl` for some other reason, and silent death after the fact remain three distinct, currently indistinguishable explanations. **Next diagnostic, not yet built:** re-check `ps -p "$LAUNCHER_PID"` (and capture `wait`'s real exit status, currently discarded) immediately before the final `busctl list`, so a late crash becomes distinguishable from the other two.

**An eleventh correction (thirteenth push): the crash theory is ruled out too.** The re-check above ran. `ps -p "$LAUNCHER_PID" -o pid,stat,cmd` immediately before the final `busctl list` showed `3253 Sl /usr/libexec/at-spi-bus-launcher --launch-immediately` -- state `Sl`, a normal healthy sleep, not a zombie or stuck state. We then killed it ourselves; `wait` reported exit code `0`, not a signal-death code -- consistent with the real source's own `init_sigterm_handling`/`sigterm_handler` self-pipe SIGTERM handler shutting it down cleanly in response to *our* kill, not evidence of any earlier crash. Combined with the previous run's clean startup log and this run's continued responsiveness to our own `GetAddress` query, this closes out all three remaining candidates from the ninth/tenth corrections: not a crash (confirmed alive and healthy right up to the check), not a long hang (answered `GetAddress` within about a second both times), not the GSettings schema issue (confirmed twice now).

What's left is a genuinely tighter puzzle than before: the process is alive, responsive, and never logs `ensure_a11y_bus`'s `g_critical` failure line -- yet `busctl list` still shows no connection from it. Nothing in `at-spi-bus-launcher.c`'s own source explains a silent, errorless, non-fatal path that ends without a visible bus connection; the remaining unknown is specifically what happens *inside* `g_dbus_connection_new_for_address_sync`'s own authentication handshake for this connection, which the process's own debug log flags as a named, version-gated risk: `"Using cross-namespace EXTERNAL authentication (this will deadlock if server is GDBus < 2.73.3)"`. `G_MESSAGES_DEBUG=all` (used so far) only controls GLib's own log-message verbosity and says nothing about this. `G_DBUS_DEBUG=all` is the separate, GDBus-specific environment variable that traces the actual wire-level authentication and connection lifecycle from inside `GDBusConnection` itself -- the right tool for the one thing still unexplained. **Next diagnostic, not yet built:** re-run this same capture with `G_DBUS_DEBUG=all` added, to see the real authentication handshake for this specific connection attempt.

**A twelfth correction (fourteenth push): the entire mechanism from the ninth/tenth/eleventh corrections was never actually running in CI.** The `G_DBUS_DEBUG=authentication,connection,call,address` trace shows exactly one real authentication sequence -- explicitly `"In g_dbus_address_get_for_bus_sync() for bus type 'session'"`, targeting `$DBUS_SESSION_BUS_ADDRESS` -- appearing twice only because the script `cat`s the same log file twice. The a11y bus address itself (`unix:path=/run/user/1001/at-spi/bus,...`) appears only in plain `g_debug` lines, never once inside a traced `Address`/`Auth` block. With `G_DBUS_DEBUG` covering every `GDBusConnection` the process creates, this means `g_dbus_connection_new_for_address_sync` for the a11y bus was never called at all -- not hung, not failed, simply never reached, directly contradicting what `main` branch's source shows as unconditional right after the address is obtained.

The reason: every correction from the seventh onward read `at-spi2-core`'s upstream `main` branch, silently assuming it matched what `apt-get install at-spi2-core` actually installs on `ubuntu-latest`. It doesn't. Ubuntu 24.04 (noble) ships **at-spi2-core 2.52.0** (confirmed via packages.ubuntu.com), and diffing that exact tag's real source (`AT_SPI2_CORE_2_52_0`) against `main` settles it: 2.52.0's `ensure_a11y_bus()` ends at `return TRUE;` right after the X11-property-setting block -- no `g_dbus_connection_new_for_address_sync`, no `g_dbus_connection_signal_subscribe`, no `EventListenerRegistered` anywhere in the file. That entire mechanism is a `main`-branch-only addition, not yet in any released version this environment can install. Every fact gathered while chasing it (`RegisterEvent`/`EventListenerRegistered` genuinely working on the wire, `at-spi-bus-launcher` staying alive and healthy throughout, the gsettings-schema theory being wrong) was real and correctly observed -- the mechanism connecting those facts to `IsEnabled` was simply the wrong one.

2.52.0's real mechanism, confirmed directly in its own source: `IsEnabled` is a plain, boring `readwrite` D-Bus property backed by `app->a11y_enabled`. Unless launched with an explicit `--a11y=0|1` flag (nothing in this CI setup ever passes one -- D-Bus service activation can't, and our own `--launch-immediately` diagnostic didn't either), `main()` initializes it, once, at the very top of startup, directly from `g_settings_get_boolean(interface_schema, "toolkit-accessibility")` -- the real `org.gnome.desktop.interface` GSettings key -- before any bus connection, any callback, anything else runs. It's also watched live afterward (`"changed::toolkit-accessibility"`) and directly settable via a plain `org.freedesktop.DBus.Properties.Set` call -- the exact mechanism this investigation's very first fix attempt used, back before `canvas_accessibility_demo`'s total failure was even identified as a distinct problem, and which was dropped as "proven ineffective" based on unchanged *timing* in a run where the property was very possibly already reading true either way.

This reopens a real question the whole RegisterEvent theory never actually resolved either: why does `tre-a11y`'s own round-trip test, in the separate `test` job, pass reliably around 10s using this exact same GSettings-backed property, while `accessibility-validation`'s never does? Not yet answered by direct evidence -- both are plausible on the schema's real default (works if `toolkit-accessibility` simply defaults true, and `test`'s ~10s is some other, still-unexplained accesskit-registration delay unrelated to `IsEnabled`) or on something specific to this job reading it false. **Next diagnostic, not yet built:** directly query the real current values of both `org.a11y.Bus`'s `IsEnabled` property (`Properties.Get`) and the underlying `gsettings get org.gnome.desktop.interface toolkit-accessibility`, in this exact job, before proposing a fix based on inference again.

**A thirteenth correction (eighteenth push): the real fix, tried directly, still failed -- but usefully.** `dconf write /org/gnome/desktop/interface/toolkit-accessibility true` ran in its own `dbus-run-session`, and an independent `dconf read` immediately afterward, in that same session, confirmed the write really took: `true`. The very next step -- a separate `dbus-run-session` running `canvas_accessibility_verify` for real -- still panicked with the exact same message as every prior attempt: `"org.a11y.Status.IsEnabled never became true within 30s of repeated RegisterEvent calls"`.

This narrows to three live explanations, not yet disambiguated by direct evidence: (1) the write genuinely doesn't persist to whatever a *fresh* session's `at-spi-bus-launcher` reads, despite both being the same user/`$HOME` on the same runner, which would mean something about dconf's own persistence model in this environment differs from the reasonable assumption that its on-disk database is session-independent; (2) it persists fine and `at-spi-bus-launcher` reads it correctly, but something else now blocks `IsEnabled` from reflecting it; or (3) the bug is in *our own* code -- `ensure_accessibility_enabled`'s `status.get_property::<bool>("IsEnabled").unwrap_or(false)` silently collapses a real D-Bus error (wrong interface, timing, anything) to the same `false` as a legitimate read, so a genuine error could be indistinguishable from the property actually being false. **Next diagnostic, not yet built:** inside the *same* fresh `dbus-run-session` the verify step itself will run in, read `toolkit-accessibility` via `dconf` and `IsEnabled` via a plain `dbus-send Properties.Get` -- both independent of our own Rust code -- before running the real verify, to see directly which of the three it is rather than guessing again.

## STOPPING POINT (nineteenth push, 2026-09-08): the real, precise bug is found and understood. Not fixed. Investigation paused here at the user's explicit instruction.

The disambiguating diagnostic settled it completely. In a fresh session, shaped exactly like `canvas_accessibility_verify`'s own: `dconf read toolkit-accessibility` → `true` (the write **does** persist across sessions -- explanation 1 above is wrong, ruled out directly). Then, in that same fresh session, a plain `dbus-send --session --print-reply --dest=org.a11y.Bus /org/a11y/bus org.freedesktop.DBus.Properties.Get string:org.a11y.Status string:IsEnabled` returned `variant boolean true` -- **`IsEnabled` genuinely is true**, read directly, independent of our own code entirely (explanation 2 is wrong too). Yet the very next step, `canvas_accessibility_verify` itself, still panicked with the identical message it always has.

That leaves explanation 3, and it's not just plausible -- it's the actual, precise, confirmed bug: **`ensure_accessibility_enabled` queries `IsEnabled` on the wrong D-Bus connection.** `at-spi-bus-launcher.c`'s own real source (confirmed directly, line 991) calls `g_bus_own_name(G_BUS_TYPE_SESSION, "org.a11y.Bus", ...)` -- `org.a11y.Bus`, `/org/a11y/bus`, and its `IsEnabled` property under interface `org.a11y.Status` are owned on the **session bus**. But in both `canvas_accessibility_verify.rs` and `tre-a11y/tests/round_trip.rs`, `ensure_accessibility_enabled(bus: &Connection)` is called with `bus` being the return value of `a11y_bus()` -- a connection built via `ConnectionBuilder::address(...)` to the **a11y bus** address obtained from `GetAddress`, not the session bus. `Proxy::new(bus, "org.a11y.Bus", "/org/a11y/bus", "org.a11y.Status")` is therefore built on the wrong bus: there is no such destination there at all, so every `status.get_property::<bool>("IsEnabled")` call fails outright -- and `.unwrap_or(false)` silently turns that failure into the same `false` a real "not enabled yet" reading would produce, indistinguishable from the outside. The loop dutifully retries for the full 30s (with `RegisterEvent`, correctly sent to `registry`, which *is* rightly built on the a11y bus, since that's where `at-spi2-registryd` actually lives -- explaining why that part always looked completely healthy) and then panics.

This single bug fully and cleanly explains every symptom gathered across this entire investigation's nineteen pushes: why nothing done to the actual `IsEnabled` mechanism itself -- the GSettings key, `Properties.Set`, `RegisterEvent` retries -- ever had any effect from our own code's point of view (we were never actually asking the right bus the question), while `RegisterEvent`/`EventListenerRegistered` always looked completely healthy on the wire (that part of the code was never wrong).

**The fix this points to, not yet built or tested:** `ensure_accessibility_enabled` needs the *session* bus connection for its `status` proxy, not the a11y bus connection it currently receives. The cleanest shape: pass both connections in (session and a11y), or restructure so the function itself opens `a11y_bus()`'s own session-bus step and reuses that same `Connection` for the `org.a11y.Status` proxy, only using the separate a11y-bus connection for `registry` and everything downstream (the actual tagged-node querying).

**One important, real loose end, deliberately left unconfirmed rather than overclaimed:** `tre-a11y/tests/round_trip.rs` has this identical bug in this identical shape, yet that test has been reported passing reliably around ~10s in the `test` job throughout this entire investigation -- never directly re-examined this session against this specific new finding. Given the bug means `ensure_accessibility_enabled` should behave identically there (loop the full 30s, then return `false` rather than panic, since that test's version returns `bool` and skips gracefully rather than panicking), a real possibility worth checking before trusting that test's "pass" at face value: it may have been silently skipping the entire time rather than genuinely verifying anything, and whatever produces its consistent ~10s timing may be unrelated to this function entirely. This was not verified this session -- flagged here specifically so it isn't lost.

**Current state, explicitly:** `accessibility-validation` still fails for real (the gate is genuine, not `continue-on-error`). The bug is understood and precisely located; the fix is not implemented. Per explicit user instruction, this investigation stops here rather than attempting and pushing that fix now. Resuming it later should start by implementing the session-bus fix above in both `canvas_accessibility_verify.rs` and `tre-a11y/tests/round_trip.rs`, and directly checking whether the `test` job's round-trip test was ever really passing or silently skipping.

## Summary table (Phase 5 Step 5.3.3 CI Verification)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 126 | `ensure_accessibility_enabled` (`canvas_accessibility_verify.rs` and `tre-a11y/tests/round_trip.rs`) queries `org.a11y.Status`'s `IsEnabled` property on the **a11y bus** connection, but `at-spi-bus-launcher` owns `org.a11y.Bus`/`IsEnabled` on the **session bus** (`g_bus_own_name(G_BUS_TYPE_SESSION, ...)`, confirmed directly in its real source) -- every `get_property` call therefore fails outright (wrong destination bus), silently collapsed to `false` by `.unwrap_or(false)`, indistinguishable from a real "not enabled" reading. Confirmed via a plain `dbus-send --session ... Properties.Get` returning `true` in a fresh session where our own code still saw `IsEnabled` as false | tre-a11y/tests/round_trip.rs, tre-rhi-vulkan/examples/canvas_accessibility_verify.rs | Should-fix | **STOPPED, unfixed, per explicit user instruction** -- root cause fully understood and precisely located (wrong-bus query, not an environment/mechanism issue); fix identified (build the `status` proxy on the session bus, not the a11y bus) but not implemented or pushed. `accessibility-validation` still fails for real. Also flagged, not confirmed: `test` job's round-trip test may have been silently skipping via this same bug rather than genuinely passing -- not checked before stopping |

## Phase 6 Step 6.1 Implementation (2026-09-08)

### 127. [Should-fix] `draw_rounded_rect` and the two demos consuming its commands each independently invented a different "no texture bound" sentinel

`tre-engine`'s `draw_rounded_rect` emitted `UiDrawCommand::texture_handle: 0` for "no texture sampled." `canvas_batch_flattening_demo.rs` and `canvas_sub_canvas_demo.rs` -- the only two demos that loop over `FlattenedFrame::commands` at all -- each independently defined their own `const NO_TEXTURE: u32 = u32::MAX` for the RHI-binding side, reconciled with the IR's own `0` only by each demo's hardcoded `if pipeline_state_id == PIPELINE_MSDF_TEXT { bind_texture(0, command.texture_handle) } else { bind_texture(0, NO_TEXTURE) }` branch -- never by reading `command.texture_handle` on the non-textured path at all. Found while building `PipelineRegistry` (Phase 6 Step 6.1), specifically because that registry exists to replace this exact branch with a data-driven lookup, which cannot work correctly while the IR's own "no texture" value (`0`) collides with a legitimate real bindless index a future textured pipeline could validly use.

**Fix:** promoted `NO_TEXTURE: u32 = u32::MAX` (the demos' own value) into a single real, shared `tre-engine` constant; `draw_rounded_rect` -- the one real `DrawGeometry`-emitting call site that needed it -- now emits `texture_handle: NO_TEXTURE`. The `PushScissor`/`PopScissor`/`PushLayer`/`PopLayer` marker commands, which also literally read `texture_handle: 0` in the source, were deliberately left unchanged: each has `element_count: 0` and is never consumed as a real draw call (a future executor only ever `continue`s past them), so the field carries no meaning there and changing it would only be cosmetic. Verified: `tre-engine`'s own `draw_rounded_rect` command-field test extended to assert `NO_TEXTURE`; all 5 demos touching `draw_rounded_rect`/`flatten()` re-run manually, zero regressions -- confirming this was a real, latent inconsistency, not yet a manifested bug, since nothing today reads `command.texture_handle` on the SDF-rect branch.

## Summary table (Phase 6 Step 6.1)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 127 | `draw_rounded_rect` emitted `texture_handle: 0` for "no texture," while `canvas_batch_flattening_demo.rs`/`canvas_sub_canvas_demo.rs` each independently defined their own `NO_TEXTURE = u32::MAX` for RHI-side binding -- two sentinels for one concept, reconciled only by a hardcoded per-demo branch; `0` is unsafe to keep since it collides with a legitimate real bindless index | tre-engine (`draw_rounded_rect`, new `NO_TEXTURE`/`PipelineKind`/`PipelineRegistry`) | Should-fix | Fixed -- `NO_TEXTURE` promoted to one real, shared `tre-engine` constant; `draw_rounded_rect` emits it; verified via an extended unit test and all 5 affected demos re-run manually, zero regressions |

## Phase 6 Step 6.4.1 Implementation (2026-09-08)

### 128. [Should-fix] `resume_swapchain_rendering`'s first draft never restored the viewport, so a draw recorded after resuming silently rendered through a stale, smaller viewport

Vulkan's dynamic viewport/scissor state is a persistent property of the command buffer, not scoped to one `cmd_begin_rendering` instance -- ending one rendering scope and beginning another does not reset it. `begin_render_to_texture` correctly sets its own viewport to match the offscreen target's own (typically smaller) dimensions before rendering into it, but the first draft of `resume_swapchain_rendering` only re-began rendering into the swapchain's own attachments -- it never re-issued `cmd_set_viewport`/`cmd_set_scissor`, leaving both stuck at whatever `begin_render_to_texture` last set for the layer.

Found by `render_to_texture_demo`'s own first real run, not designed around in the abstract: the demo's real pixel assertions failed outright (a swapchain pixel expected to show the composited layer's own real content instead showed exactly the background clear color), even though the Vulkan validation layer raised nothing at all -- a viewport smaller than its actual render target is not itself invalid, just semantically wrong for what the caller wanted. Only checking the real, composited pixel caught it.

**Fix:** `resume_swapchain_rendering` now explicitly restores both viewport and scissor to the real swapchain extent (`self.swapchain_width`/`self.swapchain_height`) as part of its own real work, rather than leaving this to a caller to remember -- the demo's own earlier workaround (explicitly re-calling `set_scissor` itself after resuming) was removed once the RHI method itself became self-sufficient. Verified: `render_to_texture_demo` re-run after the fix -- both real pixel assertions (composited interior shows real foreground, composited transparent area shows real background) pass; the fix is scoped to a genuinely new code path (`resume_swapchain_rendering` didn't exist before this step), so no other demo could have been affected either way -- confirmed anyway by re-running all 21 pre-existing Vulkan demos, zero regressions.

## Summary table (Phase 6 Step 6.4.1)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 128 | `resume_swapchain_rendering`'s first draft never restored the viewport (a persistent, command-buffer-wide piece of Vulkan state `begin_render_to_texture` had already changed for the layer's own smaller size) -- a draw recorded after resuming silently rendered through the wrong viewport, with no validation-layer warning at all | tre-rhi-vulkan (`VulkanCommandBuffer::resume_swapchain_rendering`) | Should-fix | Fixed -- both viewport and scissor now explicitly restored to the real swapchain extent inside `resume_swapchain_rendering` itself; verified via `render_to_texture_demo`'s own real pixel assertions and all 21 pre-existing Vulkan demos re-run, zero regressions |

## Phase 6 Step 6.4.2 Implementation (2026-09-08)

### 129. [Should-fix] `segment_and_flatten` left a geometry-carrying marker command's `vertex_offset` unrebased, pointing at the wrong index buffer entirely

`segment_and_flatten`'s own doc comment stated "every marker passes through unchanged" -- true until this step, since no non-`DrawGeometry` command had ever carried real geometry of its own. Step 6.4.2's `pop_layer` broke that assumption: it now bakes a real composite-quad's vertices/indices and records them on the `PopLayer` command itself (`element_count: 6`, `vertex_offset` pointing into the canvas's own raw, pre-flatten `indices`). `segment_and_flatten` only rewrites `vertex_offset`/copies indices for commands inside a `DrawGeometry` run (via `flatten_run`); every non-`DrawGeometry` command, `PopLayer` included, was pushed into `out_commands` completely unchanged. The returned `FlattenedFrame::indices` is a freshly built buffer (`out_indices`) containing only the indices `flatten_run` copied into it -- `PopLayer`'s own composite-quad indices were never among them, so its `vertex_offset` would have pointed at a position with no correspondence in the buffer `execute_frame`'s `draw_indexed` actually reads from (uploaded from `FlattenedFrame::indices`, not the canvas's own raw `indices`).

Found by re-reading `segment_and_flatten`'s own real implementation and doc comment while implementing `pop_layer`'s geometry-baking, before writing or running any test against it -- the doc comment's own "every marker passes through unchanged" claim was the tell, once `pop_layer` was about to make it false.

**Fix:** `segment_and_flatten` now rebases any boundary (non-`DrawGeometry`) command whose `element_count > 0` the same way `flatten_run` already rebases `DrawGeometry` commands: its own indices are copied into `out_indices` and its `vertex_offset` rewritten to the new position, before being pushed into `out_commands`. A command with `element_count == 0` (every marker before this step, and `PushLayer`/`PushScissor`/`PopScissor` still today) is unaffected -- the copy is skipped entirely, so this is a strict extension, not a behavior change, for every pre-existing command kind. Verified: a new unit test (`pop_layer_bakes_composite_quad_vertices_at_the_descs_own_screen_position`) asserts the flattened frame's real `indices` buffer, sliced at the `PopLayer` command's own (rebased) `vertex_offset`, contains the expected `[0, 1, 2, 0, 2, 3]` -- would have failed outright (wrong slice, likely out of bounds or reading unrelated data) without this fix; all 55 pre-existing `tre-engine` tests still pass unchanged, confirming no regression for any command kind that predates this step.

## Summary table (Phase 6 Step 6.4.2)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 129 | `segment_and_flatten` pushed every non-`DrawGeometry` command through unchanged, including a `PopLayer` command's own new real `vertex_offset`/`element_count` (Step 6.4.2's composite-quad geometry) -- left pointing at the canvas's own raw, pre-flatten index buffer rather than the flattened `out_indices` buffer the returned frame (and the RHI index buffer built from it) actually contains | tre-engine (`segment_and_flatten`) | Should-fix | Fixed -- any boundary command with `element_count > 0` is now rebased into `out_indices` exactly as `flatten_run` already does for `DrawGeometry` commands; caught by code inspection before any test ran, verified by a new unit test asserting the rebased indices and a full 55-test regression pass |

## Phase 6 Step 6.5 Implementation (2026-09-08)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 6.5 (the combining capstone), closing Phase 6 in full. Retroactively added here during the Phase 1-8 Comprehensive Review below (finding #147) -- this section was never written at the time, an indexing gap in this document, not a gap in the step's own real verification.

Status: **Complete, no findings.** A pure integration proof adding no new `tre-engine`/RHI surface: one `Canvas` scene, one `execute_frame` call, exercising every command kind (`DrawGeometry`/`PushScissor`/`PopScissor`/`PushLayer`/`PopLayer`) together for the first time. IMPLEMENTATION.md's own Step 6.5 write-up states "No bugs found — passed on its first real run," matching Step 4.3.2's own precedent for a clean pass worth recording plainly.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace and the new `canvas_combined_scene_demo` (`demo/phase6_step6_5/`) re-run for real against Vulkan hardware with zero validation errors, every real pixel assertion passing.

## Phase 7 Step 7.2.1 Investigation (2026-09-08)

### 130. [Should-fix, Fixed] Sampling a bindless texture while the current render target is an offscreen texture (not the swapchain) produced all-zero output -- real root cause found and fixed (2026-09-08)

`dual_kawase_blur_demo.rs`'s own real Dual-Kawase chain (`crates/tre-rhi-vulkan/examples/dual_kawase_blur_demo.rs`) draws a real, opaque square into a transient target (L0), then attempts to downsample it into a second transient target (L1) via a shader that samples L0 through the bindless texture array while L1 is the active render target. The real GPU readback of the composited result is **exactly** the background clear color everywhere -- both where the square's own blurred content should appear and where it never did -- indicating the downsample pass's own `texture(sampler2D(bindless_textures[nonuniformEXT(pc.texture_index)], bindless_sampler), frag_uv)` call reads all-zero (fully transparent) data regardless of the real, correctly-registered texture it should be reading.

**Real root cause not found, despite an extensive, systematic elimination.** Each of the following was independently tested and ruled out as the cause:
- **The original double-`cmd_end_rendering` bug** (chaining `end_render_to_texture`/`begin_render_to_texture` directly double-calls `cmd_end_rendering` for one active scope) -- real, found, and fixed by adding `RhiCommandBuffer::begin_render_to_texture_no_end` (paired with a plain `end_render_to_texture` immediately before it). This fix is real and correct on its own terms, but did not resolve the sampling failure.
- **`RhiDevice::register_bindless` called while a render pass was active** (an initial combined `chain_render_to_texture` design, since reverted, called it mid-pass) -- fixed by the same `begin_render_to_texture_no_end` split, calling `register_bindless` with no render pass active, matching Step 6.4.1's own proven call order exactly. Did not resolve the failure.
- **`nonuniformEXT` decoration lost through an intermediate local variable or function parameter** -- both `kawase_downsample.frag`/`kawase_upsample.frag` were rewritten to fully inline the bindless lookup at every sample, matching `bindless_textured.frag`/`msdf.frag`'s own established pattern exactly (byte-for-byte identical descriptor layout declarations, confirmed via direct diff). Did not resolve the failure.
- **The 5-tap/8-tap averaging math itself** -- reduced to a single, unweighted `texture(...)` call with no offsets; still failed.
- **Descriptor-index reuse within one frame** (deregistering L0's index and immediately re-registering L1 might reuse the same slot, and since descriptor writes are host-side/immediate while GPU execution is deferred until submission, an index reused within one frame's recording could resolve to the *wrong* texture at actual execution time) -- tested directly by keeping L0's index alive and confirming L1 received a genuinely distinct index (0 vs. 1); still failed.
- **Render-target format** (`Rgba16Float` vs. the swapchain's own `Bgra8Srgb`) -- tested both; both failed identically.
- **Render-target size mismatch** (L1 at half L0's size vs. exactly matching) -- tested both; both failed identically.
- **Shader source file** (`kawase_downsample.frag` vs. the already-proven-working, unmodified `bindless_textured.frag`) -- swapped in the real, working shader unchanged; it *also* failed when its own render target was an offscreen texture instead of the swapchain, ruling out anything specific to the new shader files.
- **Whether `frag_uv`/`pc.texture_index` actually reach the shader correctly** -- confirmed directly by outputting them as color (`vec4(frag_uv, texture_index / 4.0, 1.0)`); both carried real, correct, non-degenerate values.
- **Descriptor-indexing device features** -- confirmed `shader_sampled_image_array_non_uniform_indexing`/`descriptor_binding_partially_bound`/`runtime_descriptor_array` are all enabled at device creation (`VulkanDevice::new`).
- **Pipeline/render-pass invocation itself** -- confirmed working via a hardcoded, texture-independent `out_color = vec4(1.0, 0.0, 0.0, 1.0)` output, which correctly appeared across the entire composited result, proving the pipeline, viewport/scissor, blend state, and composite chain are all functioning; only the *texture read itself* returns wrong data.

**The one variable that reliably reproduces the failure across every test:** sampling a bindless texture while the *currently active render target* is anything other than the swapchain (i.e., anything reached via `begin_render_to_texture`/`begin_render_to_texture_no_end` rather than `resume_swapchain_rendering`). No prior code in this project has ever exercised this combination -- every existing demo that samples a bindless texture does so only while rendering into the swapchain (`render_to_texture_demo.rs`'s own composite step included); every existing demo that renders into an offscreen target via `begin_render_to_texture` never simultaneously samples a *different* bindless texture while doing so.

**Change:** not fixed. `RhiCommandBuffer::begin_render_to_texture_no_end` (a real, independently-correct fix for the double-`cmd_end_rendering` bug, and used together with a plain `end_render_to_texture` call before it, matching Step 6.4.1's proven call order for `register_bindless`) is kept -- it is real, tested-compiling, and does not affect any existing demo (all 24 pre-existing Vulkan demos re-run manually, zero regressions). `dual_kawase_blur_demo.rs`, `kawase_downsample.frag`, and `kawase_upsample.frag` are kept as a real, precisely-documented reproduction case for future debugging (e.g., with a real GPU frame-capture tool such as RenderDoc, beyond what CLI-based investigation can resolve) -- not added to `ci.yml`, since it currently fails its own real pixel assertions. TECHNICAL.md Section 5.5's canonical Dual-Kawase formula remains correct and unaffected regardless of this blocker. Step 7.2.1 stays open; Step 7.2.2 (wiring to `push_layer`/`pop_layer`) cannot proceed until this is resolved.

**Second investigation session (2026-09-08): an independent external research report reviewed, four additional root-cause mechanisms checked against real code, one genuinely new diagnostic technique found and proven to work -- root cause still not found.** The project owner commissioned an independent AI deep-research review of this exact bug, in parallel with the Phase 1-8 Comprehensive Review above; its full report is kept at `documentation/DUAL-KAWASE_BLUR_RESEARCH.md`/`.pdf`. Its own twelve tested hypotheses map 1:1 onto the twelve already listed above (expected, since it was given this investigation's own notes) -- no new ground there. Its real contribution was Section 3, four theoretical root-cause mechanisms not explicitly checked before. Each was checked against the real, current code, not assumed:

- **Missing/incorrect pipeline barrier stage or access masks** (as opposed to just the layout transition, which was already confirmed correct) -- checked by actually running the demo with Vulkan **synchronization validation** enabled (`VK_LAYER_VALIDATE_SYNC=1`, the current setting name; the report's own suggested `VK_VALIDATION_FEATURE_ENABLE_SYNCHRONIZATION_VALIDATION_EXT` is deprecated and was rejected by the loader with a warning naming the replacement). This is a real, previously-untried diagnostic: it performs the deeper cross-command-buffer hazard tracking the *standard* validation layer (already run on every CI job) does not. **Proven genuinely active in this environment via a real positive-control test**: deliberately commenting out `end_render_to_texture`'s own real barrier immediately produced a real, specific `WRITE_AFTER_WRITE hazard detected` error naming the exact missing synchronization -- confirming the tool works and would catch a real barrier defect here. Reverting that deliberate break and re-running the actual, unmodified demo produced **zero** synchronization warnings of any kind. This rules out missing/incorrect barriers as the root cause with substantially more confidence than the original investigation's own code-reading-only conclusion.
- **Descriptor set invalidated by an incompatible pipeline layout switch, never rebound** -- ruled out by reading `VulkanCommandBuffer::set_pipeline`'s real implementation (`crates/tre-rhi-vulkan/src/lib.rs:2454-2486`): it unconditionally calls `cmd_bind_descriptor_sets` with the bindless set and the just-bound pipeline's own layout on *every* `set_pipeline` call, and `create_pipeline` always builds every pipeline (the kawase downsample/upsample ones included, confirmed via `dual_kawase_blur_demo.rs`'s own `create_pipeline` call sites) against the identical, compatible layout. There is no way for the descriptor set to go stale across a pipeline switch in this codebase.
- **Descriptor set layout missing `UPDATE_AFTER_BIND`/`PARTIALLY_BOUND` flags** -- ruled out; already correctly present (`crates/tre-rhi-vulkan/src/lib.rs:654-659`, confirmed by direct read).
- **Transient target missing `VK_IMAGE_USAGE_SAMPLED_BIT`, or a zeroing component swizzle** -- ruled out; `VulkanTexture::new` (the real transient-target constructor) already requests `COLOR_ATTACHMENT | SAMPLED` (`crates/tre-rhi-vulkan/src/lib.rs:3320`), and no code anywhere sets an explicit component mapping, so the Vulkan default (`IDENTITY`, not `ZERO`) applies.

**Net result of the report-driven checks alone: all four newly-proposed mechanisms are ruled out, joining the original twelve (thirteen total, now including a real synchronization-validation pass).** The synchronization-validation technique itself (`VK_LAYER_VALIDATE_SYNC=1`) is a genuinely new, durable diagnostic asset from this session, beyond what the original twelve hypotheses' own CLI-based methods could check.

**Third investigation step (2026-09-08), prompted by the project owner asking how macOS/Windows/game engines actually implement this effect: the bindless texture array itself is now real, reproducibly implicated as the root cause.** Real-time backdrop-blur implementations elsewhere (Windows Acrylic/Mica, macOS's own backdrop materials, and in-app game-engine post-process chains) essentially never route a just-rendered, same-frame offscreen target through a large, persistent, `UPDATE_AFTER_BIND` bindless array shared with every other texture in the engine -- they bind it through a small, plain, dedicated, conventionally-bound sampler for that one pass. This engine had never tried that. A new isolated experiment, `dual_kawase_nonbindless_experiment.rs`, was built to test it directly: a hand-rolled, non-bindless descriptor set layout (one plain `COMBINED_IMAGE_SAMPLER` binding, no `UPDATE_AFTER_BIND`/`PARTIALLY_BOUND` flags at all) and a matching pipeline layout, used for exactly the same real render-to-texture lifecycle (`begin_render_to_texture`/`end_render_to_texture`/`begin_render_to_texture_no_end`/`resume_swapchain_rendering`, all real, unmodified RHI calls) as `dual_kawase_blur_demo.rs` -- draw a white square into a full-size offscreen target (L0), downsample it into a half-size offscreen target (L1) by sampling L0 through the new plain descriptor instead of `bindless_textures[]`, then composite L1 onto the swapchain the same way. Real hand-rolled Vulkan throughout (`RhiCommandBuffer::raw_handle()` recovers the real `vk::CommandBuffer` so the experiment's own pipeline/descriptor-set binds can bypass `set_pipeline`'s unconditional bindless rebind).

**Result: the composited square's own center read real, correct content -- exactly the pure-white foreground color, not the background clear color -- reproduced consistently across 4 separate real runs.** This is the exact opposite of every bindless-based attempt, which reads all-zero/background in this identical position under this identical render-to-texture lifecycle. Nothing else changed between the two demos except the one variable under test (bindless array vs. a plain, conventional descriptor). This is real, reproducible evidence that the bindless texture array -- specifically its interaction with sampling a texture that was itself this same frame's own render target moments earlier -- is the actual root cause, not a coincidental correlation.

**Change:** `dual_kawase_nonbindless_experiment.rs` (and its two new shaders, `kawase_downsample_nonbindless.frag`/`passthrough_nonbindless.frag`) are kept as a real, passing, cleanly-resource-managed proof -- unlike `dual_kawase_blur_demo.rs`, this one is added to `ci.yml`'s `vulkan-validation` job, since it genuinely passes. `dual_kawase_blur_demo.rs` itself is left unmodified for now (still bindless, still failing, still excluded from CI) -- converting the *real* Dual-Kawase implementation to this non-bindless approach is real follow-on work, not done as part of this diagnostic step. Step 7.2.1 stays open, but for the first time with a real, demonstrated, working path forward rather than only ruled-out hypotheses.

**Fourth investigation step (2026-09-08): the bindless array was never actually the cause. Real root cause found, fixed, and verified -- finding closes.** Converting `dual_kawase_blur_demo.rs`'s real 5-hop chain to the third step's proven non-bindless approach built cleanly but *still failed* past the second hop, with the identical background-only symptom -- unexpected, since the third step's own experiment only ever exercised one hop. Bisecting hop by hop (a 2-hop chain, matching the experiment's own shape, passed; a 3-hop chain, reusing the downsample pipeline a second time, failed) and then substituting one variable at a time -- sampling L0 (bindless-produced) a second time into a second offscreen target: **passed**; sampling L1 (non-bindless-produced) into the swapchain: **passed** (already known, from the 2-hop case); sampling L1 into a second offscreen target, with the pipeline object swapped for a different, never-yet-used one: **still failed** -- isolated the failure to reading a texture that was itself produced by a non-bindless pass, specifically when the destination was a second offscreen target, independent of which pipeline object did the reading.

Printing the acquired texture's own real `dimensions()` (rather than assuming they matched the requested size) found the actual defect: `RhiDevice::acquire_transient_target`'s own documented "oversized borrow" fallback (`crates/tre-rhi-vulkan/src/lib.rs:1578-1602` -- hands back a *larger* already-freed texture when no free bucket of the exact requested size exists yet) returned L0's just-released 256x128 texture when L2's 64x32 bucket was requested for the first time in this process. This alone would have been survivable -- except `RhiCommandBuffer::draw_indexed` (`crates/tre-rhi-vulkan/src/lib.rs:2574-2606`) unconditionally performs its *own*, second `cmd_push_constants` call using `self.width`/`self.height` (set from the render target's own real dimensions by `begin_render_to_texture`/`begin_render_to_texture_no_end`), silently overwriting the correct, already-pushed per-hop `screen_size` a caller had just pushed manually for one of these custom, non-bindless pipelines -- right before the draw call executes. With the wrong (oversized) `screen_size` in effect, the vertex shader's NDC mapping (`bindless_textured.vert`: `ndc = (in_position / pc.screen_size) * 2.0 - 1.0`) confined the actual drawn quad to a small corner of the real, oversized backing image, leaving the texture's real center/interior untouched at the `[0,0,0,0]` clear color -- which is exactly what every later hop and the final composite kept sampling, reproducing the "exact background" symptom under three different framings (bindless array, missing barrier, "second render-to-texture round trip") across four prior investigation sessions, none of which were the real cause. Confirmed directly: `eprintln!("{:?}", l2.dimensions())` printed `(256, 128)` for a texture requested as `(64, 32)`.

**Fix:** every non-bindless downsample/upsample/composite pass in `dual_kawase_blur_demo.rs` now issues its draw via a raw `cmd_draw_indexed` call instead of `RhiCommandBuffer::draw_indexed`, so the wrapper's own redundant, potentially-stale push-constant call never executes for these passes -- the correct, manually-pushed `screen_size` is the only one ever in effect. The non-bindless conversion itself (a plain `COMBINED_IMAGE_SAMPLER` per hop instead of the bindless array) is kept, since it remains a real, independently-reasonable design matching how other engines implement this exact kind of same-frame offscreen backdrop blur -- it just was never what was actually broken. Neither `acquire_transient_target`'s oversized-borrow fallback nor the render-to-texture lifecycle needed any change; both are working exactly as documented. **Verified:** the full, real 5-hop chain's own pixel assertions (interior stays foreground; edge shows genuine partial blend, not pure background) pass consistently across 5 separate real runs against actual GPU hardware, with clean cleanup (zero leaked Vulkan objects) every time. `dual_kawase_blur_demo.rs`'s own header doc comment has the complete technical account and is added to `ci.yml`'s `vulkan-validation` job. `dual_kawase_nonbindless_experiment.rs`'s header is updated to note it carries the exact same latent `draw_indexed` hazard, just never triggered by its own single-hop shape (it never releases a larger bucket before first requesting a smaller one). **Step 7.2.1 is closed.** Step 7.2.2 (wiring this real capability to `push_layer`/`pop_layer`) is real, separate future work, not attempted here.

**Fifth investigation step (2026-09-08, during Step 7.2.2's own pre-work): the bindless array genuinely still has a real, separate, unexplained defect -- disclosed as a known gap, not chased further.** Before designing Step 7.2.2, a scratch test checked whether REVIEW.md finding #152's own general fix (`begin_render_to_texture`'s new `logical_width`/`logical_height` parameters, closing the *general* case of this same root mechanism) also happened to resolve this finding's original bindless-sampling symptom. It did not: a plain two-hop chain (draw a square into L0, downsample L0 -> L1 via the *original*, bindless `kawase_downsample.frag`, entirely through standard `RhiCommandBuffer`/`RhiDevice` trait calls -- `register_bindless`/`set_pipeline`/`bind_texture`/`draw_indexed`, exactly `PopLayer`'s own existing, already-proven pattern -- with the real, correct `logical_width`/`logical_height` passed throughout) still read back the destination's own center as background, not foreground. This means the original symptom this finding chased was never *fully* explained by the fourth step's own fix alone: there remains a second, genuinely unexplained defect specific to sampling a bindless texture while rendering into a *different* offscreen target (not the swapchain) -- independent of the `draw_indexed`/oversized-borrow interaction, since that was controlled for directly in this test. Not investigated further here -- real, separate future work; `dual_kawase_blur_demo.rs`'s own already-proven non-bindless mechanism sidesteps it entirely (its own final composite step samples while rendering into the *swapchain*, not a second offscreen target, the one condition already proven to work), so it has no bearing on Step 7.2.2's own real capability.

## Summary table (Phase 7 Step 7.2.1)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 130 | Sampling a bindless texture while the active render target is an offscreen texture (not the swapchain) produced all-zero output; 13 hypotheses on the bindless path itself were ruled out -- the bindless array turned out not to be the cause at all | tre-rhi-vulkan (`dual_kawase_blur_demo.rs`, `dual_kawase_nonbindless_experiment.rs`, `lib.rs`'s `RhiCommandBuffer::draw_indexed`) | Should-fix, **Fixed** | **Real root cause found and fixed**: `RhiCommandBuffer::draw_indexed`'s own unconditional, second `cmd_push_constants` call (using the render target's real dimensions) silently clobbered a manually-pushed `screen_size` for these custom pipelines, whenever `acquire_transient_target`'s documented "oversized borrow" fallback returned a texture larger than requested -- confining the actual draw to a small corner of the oversized image and leaving the real interior at the clear color. Fixed by issuing every non-bindless pass's draw via a raw `cmd_draw_indexed` call instead of the wrapper method. Verified: the full real 5-hop chain's own pixel assertions pass consistently across 5 real runs against actual GPU hardware, zero leaked objects; added to `ci.yml`. Step 7.2.1 is closed. **Fifth step (during Step 7.2.2 pre-work):** a real, separate, still-unexplained defect confirmed genuinely remains in the bindless path itself (sampling a bindless texture while rendering into a *different* offscreen target still fails, controlling for this finding's own fix) -- disclosed as a known gap; `dual_kawase_blur_demo.rs`'s own non-bindless mechanism, reused by Step 7.2.2, sidesteps it entirely |

## Phase 6 Step 6.4.2 Regression -- REVIEW.md Finding #152 (2026-09-08)

### 152. [Should-fix, Fixed] The real, shipped `PushLayer`/`PopLayer` compositing silently drops a layer's own content when `acquire_transient_target`'s oversized-borrow fallback fires

Found while checking whether Step 7.2.2 (wiring Dual-Kawase blur into
`push_layer`/`pop_layer`) would be building on solid ground, immediately
after closing finding #130 above. Same underlying mechanism as #130's
real root cause, reached through the standard, *production*
`PushLayer`/`PopLayer` path instead of a hand-rolled demo -- meaning
this bug has been live in Step 6.4.2's own shipped compositing capability
since it was built, not something this session introduced.

**Confirmed via a real, GPU-backed repro before being treated as fact.**
Push+pop a first, larger layer (200x150, a fresh transient-pool
allocation), letting it release back to the pool at frame end; push+pop
a second, smaller, never-before-requested layer (50x40) in a later
frame. `acquire_transient_target`'s own documented "oversized borrow"
fallback (any free bucket at least as large as requested is usable) has
no exact 50x40 bucket yet, so it hands back the freed 200x150 texture
instead. The second layer's own content -- a rect meant to nearly fill
its own local 50x40 bounds -- read back as exactly the background clear
color at its own center: silently missing, no error, no validation
warning.

**Root cause, precisely:** `RhiCommandBuffer::begin_render_to_texture`/
`begin_render_to_texture_no_end` (`tre-rhi-vulkan/src/lib.rs`) set
`self.width`/`self.height` -- `draw_indexed`'s own NDC-mapping push
constant source -- from `texture.dimensions()`, the render target's
*real* physical size. `PushLayer`'s own inner `DrawGeometry` commands
have their vertex positions baked at `Canvas` record time against the
`LayerDesc`'s own requested, *logical* size, before the real texture is
ever acquired. When the two diverge (oversized borrow), the NDC mapping
uses the wrong, larger size, confining the actual draw to a small corner
of the oversized image.

**The fix, worked out in full before writing any code, not just
patched by trial and error:** viewport/scissor/render area can stay
driven by the texture's real size unchanged -- NDC always spans -1..1
across whatever the *current* viewport actually is, so a real target
larger than intended just means the content draws proportionally
"stretched" to fill it. That stretch is exactly undone later:
`PopLayer`'s own composite quad samples the texture across its full,
un-scaled `(0,0)`-`(1,1)` UV range (unchanged) and redraws it at the
`LayerDesc`'s own real, requested on-screen size (unchanged) -- an
encode-with-intended-size / decode-via-normalized-UV round trip that is
a mathematical identity regardless of the intermediate real texture's
own physical size or aspect ratio. The only thing that actually needed
to change is what feeds `self.width`/`self.height`: `begin_render_to_
texture`/`begin_render_to_texture_no_end` now take explicit
`logical_width`/`logical_height` parameters -- the caller's own
intended size, always already known (exactly what it passed to
`acquire_transient_target`) -- used only for `self.width`/`self.height`.
No UV rescaling, no dynamic vertex-buffer rewriting, no viewport/scissor
change needed anywhere.

**Change:** `RhiCommandBuffer::begin_render_to_texture`/`begin_render_
to_texture_no_end` gained the two new parameters (a real trait
signature change); `execute_frame`'s `PushLayer` handling now passes
`command.clip_bounds.width`/`.height` (the requested size) instead of
letting the callee infer it from the acquired texture. Every other real
call site updated mechanically, using each caller's own already-known
intended size: `render_to_texture_demo.rs`, `dual_kawase_nonbindless_
experiment.rs`, and `dual_kawase_blur_demo.rs` (5 call sites --
`dual_kawase_blur_demo.rs`'s own REVIEW.md #130 raw-`cmd_draw_indexed`
workaround is left unchanged, now technically redundant for new code
but not reverted, since undoing an already-shipped, already-verified
step to prove that point is no part of what this finding needed).
`RhiDevice::acquire_transient_target`'s own oversized-borrow logic is
untouched -- it is deliberate and correctly documented (DESIGN.md
Section 2.6's "no dynamic RHI allocation inside the render tick"); the
bug was always in how a caller's NDC math reacted to it, never in the
fallback itself.

**Verified at two levels.** A new `tre-engine` unit test
(`execute_frame_push_layer_passes_the_requested_size_not_an_oversized_
borrowed_textures_own`) extends `FakeDevice` with an `oversized_borrow`
override returning a texture larger than requested, proving
`execute_frame` passes the *requested* size to `begin_render_to_texture`
regardless -- all 63 `tre-engine` tests pass. A new, permanent,
real-GPU demo (`layer_oversize_regression_demo.rs`, added to `ci.yml`'s
`vulkan-validation` job) reproduces the exact two-frame triggering
sequence against the real `VulkanDevice`/`acquire_transient_target`
oversized-borrow path and asserts the second layer's own content
composites correctly -- confirmed passing across 3 consecutive real
runs. Full regression sweep: every pre-existing Vulkan demo re-run
manually, zero regressions, `cargo fmt`/`clippy -D warnings`/`build`/
`test` clean across the workspace.

## Summary table (Phase 6 Step 6.4.2 Regression)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 152 | `execute_frame`'s real `PushLayer`/`PopLayer` compositing silently drops a layer's own content whenever `acquire_transient_target`'s documented oversized-borrow fallback hands back a texture larger than requested -- same underlying mechanism as finding #130, reached through the production path instead of a demo | tre-engine (`RhiCommandBuffer::begin_render_to_texture`/`begin_render_to_texture_no_end`, `execute_frame`), tre-rhi-vulkan (`lib.rs`) | Should-fix, **Fixed** | **Fixed**: `begin_render_to_texture`/`begin_render_to_texture_no_end` gained explicit `logical_width`/`logical_height` parameters, used only for the NDC-mapping push-constant source, not viewport/scissor/render area -- provably equivalent to sizing the texture exactly right, since the resulting "stretch" is exactly undone by `PopLayer`'s own normalized-UV composite read. Verified by a new `tre-engine` unit test (oversized `FakeTexture`) and a new permanent real-GPU demo (`layer_oversize_regression_demo.rs`, in `ci.yml`), plus a full zero-regression sweep |

## Phase 7 Step 7.2.2 Implementation (2026-09-08)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 7.2.2 (wiring the Dual-Kawase blur chain to `push_layer`/`pop_layer` via a new `RhiCommandBuffer::apply_layer_blur`).

### 153. [Should-fix, Fixed] `apply_layer_blur`'s own internal hops left the wrong vertex/index buffers bound for the composite draw immediately following

`RhiCommandBuffer::apply_layer_blur`'s own internal downsample/upsample chain rebinds the command buffer's vertex/index buffers to its own small, cached unit quad for each of its 4 hops (`cmd_bind_vertex_buffers`/`cmd_bind_index_buffer`, raw calls inside `draw_hop`). Nothing rebound the frame's *real*, shared vertex/index buffers afterward -- REVIEW.md finding #135's own "bound once, at the top of `execute_frame`" invariant assumes those bindings persist unchanged for the rest of the frame, which every call site until this one was true for (neither `set_pipeline` nor any `begin_render_to_texture`/`end_render_to_texture` variant touches them). `execute_frame`'s own `PopLayer` handling, immediately after calling `apply_layer_blur`, proceeds straight to the composite draw (`cmd_buffer.draw_indexed(command.element_count, command.vertex_offset, 0)`) assuming the frame's real index buffer is still bound -- but it was still `apply_layer_blur`'s own tiny, 6-index (24-byte) unit quad buffer.

**Found by an actual GPU run, not caught by design review.** The first real run of `layer_blur_demo.rs` hit a genuine Vulkan validation error: `vkCmdDrawIndexed(): index size (4) * (firstIndex (6) + indexCount (6)) + binding offset (0) = an ending offset of 48 bytes, which is greater than the index buffer size (24)` -- `firstIndex` here is the composite quad's own real `vertex_offset` into the frame's shared index buffer (6, since it follows the square's own 6 indices), read against `apply_layer_blur`'s own leftover 24-byte buffer instead.

**Fix:** `execute_frame`'s own `PopLayer` handling re-binds the frame's real vertex/index buffers (`cmd_buffer.bind_vertex_buffer`/`bind_index_buffer`, the same `vertex_buffer`/`index_buffer` parameters already bound once at the top) immediately after the `apply_layer_blur` call returns, before proceeding to the composite draw -- the same "restore whatever this call disturbed" responsibility `resume_swapchain_rendering`'s own scissor-restore already established (Step 6.4.1/REVIEW.md finding #128). Scoped to the `blur: true` branch only, since the `blur: false` path never touches these bindings at all. Verified: `layer_blur_demo.rs`'s own real pixel assertions pass consistently across 4 separate real runs after the fix, with zero validation errors of any kind.

## Summary table (Phase 7 Step 7.2.2)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 153 | `apply_layer_blur`'s own internal hops rebind the command buffer's vertex/index buffers to its own unit quad, but nothing restored the frame's real shared buffers before the composite draw immediately following -- an out-of-bounds index read a real GPU run caught via validation | tre-engine (`execute_frame`'s `PopLayer` handling) | Should-fix, **Fixed** | **Fixed**: `execute_frame` re-binds the frame's real vertex/index buffers right after `apply_layer_blur` returns, only on the `blur: true` path. Verified across 4 real runs, zero validation errors |

## Phase 8 Step 8.1.1 Implementation (2026-09-08)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 8.1.1 (real `FrameClock` + `spring_decay` primitives). Retroactively added here during the Phase 1-8 Comprehensive Review below (finding #147) -- this section was never written at the time, an indexing gap in this document, not a gap in the step's own real verification.

Status: **Complete, no numbered findings.** Two new, independent, zero-consumer primitives, matching this project's own "build and prove the primitive before its exact consumer exists" precedent (`Affine2::compose_batch`, `tone_map`): `FrameClock` (`tre-engine`) wraps `std::time::Instant`; `spring_decay` (`tre-math`) implements the exact exponential-decay formula IMPLEMENTATION.md's own Step 8.1 task 2 names. Neither primitive was wired to any real consumer in this step, deliberately -- Step 8.1.2 (below) is their first real consumer.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace: real unit tests for both, including a `std::thread::sleep`-based real hardware-timing check for `FrameClock` (not a mocked clock).

## Phase 8 Step 8.1.2 Implementation (2026-09-08)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: implementing IMPLEMENTATION.md Step 8.1.2 (the full 8-stage continuous main loop), closing Phase 8. Retroactively added here during the Phase 1-8 Comprehensive Review below (finding #147) -- this section was never written at the time, an indexing gap in this document, not a gap in the step's own real verification.

Status: **Complete.** Real investigation before writing any code found two genuine, previously-undiscovered gaps beyond simply wiring already-proven mechanisms together:

- `execute_frame` hardcoded a `0` byte offset at its `bind_vertex_buffer`/`bind_index_buffer` calls, even though `RhiCommandBuffer::bind_vertex_buffer`/`bind_index_buffer` already accepted a real `offset: u32` parameter -- meaning it could never bind a real per-frame `RhiDynamicRingBuffer`-backed segment, only a one-shot-uploaded whole-frame buffer. Fixed by adding a new `BufferBinding<'a> { buffer: &'a dyn RhiBuffer, offset: u32 }` type (bundling buffer + offset, matching `GlyphAtlasContext`'s own precedent, and needed anyway once `clippy::too_many_arguments` fired at the resulting 9-parameter signature) and updating all 12 pre-existing call sites.
- `tre-atlas`'s `AtlasOwner` has no way to read its own background thread's current atlas pixels without stopping that thread entirely (`AtlasOwner::join`, terminal) -- a real, previously-undiscussed limit on ever growing the atlas live, mid-run. Scoped around, not silently ignored: the new demo's atlas is fully pre-seeded before its loop starts, matching every prior text-drawing demo's own proven pattern; live growth is named explicitly as future work.

New demo, `main_loop_demo.rs` (`demo/phase8_step8_1_2/`): the first demo to combine all 8 of Step 8.1's own named pipeline stages inside one real, continuous, windowed loop. Step 8.1.1's `FrameClock`/`spring_decay` get their first real consumer here, animating a rect toward a target every frame. Verified by independently replaying `spring_decay` over the loop's own recorded per-frame `dt` sequence (not a pixel read -- `VulkanSwapchain` has no readback path the way `HeadlessSwapchain` does): 90 real frames presented on a live GPU and display, zero Vulkan validation-layer errors, animation verified end to end.

No numbered findings recorded at the time; the Phase 1-8 Comprehensive Review below found several real issues in this step's own code after the fact (findings #134-135, #138, #150-151) -- see that section for full disposition of each.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace; all 5 demos whose `execute_frame` call site changed shape re-run manually, zero regressions. Added to `ci.yml`'s `vulkan-validation` job.

## Phase 1-8 Comprehensive Review (2026-09-08)

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions. Requested explicitly by the project owner as a second full review, following the exact same requirements as the "Phase 1-4 Comprehensive Review" above, now covering everything built since (Phases 5-8: multi-threaded canvas recording, RHI execution/compositing, color management, and the full continuous main loop), run while an independent external review of the Step 7.2.1 Dual-Kawase blocker (finding #130) proceeded in parallel via a separate channel.

Scope and method: identical to the Phase 1-4 review -- six independent dimensions (concurrency & data-race correctness; performance/allocation/rendering-hot-path efficiency; security/unsafe-code/trust-boundary audit; code quality & cross-crate consistency; typography pipeline correctness end to end; cross-phase integration & documentation accuracy), each run as its own "Find" agent reading the real code across all of Phases 1-8 against DESIGN/TECHNICAL/ARCHITECTURE/IMPLEMENTATION.md and this document's own existing findings (so as not to re-report anything already fixed), followed by an independent "Verify" agent per dimension instructed to adversarially re-read the exact cited lines and try to refute each finding. All 21 findings across all 6 dimensions were independently confirmed by their verifier -- several strengthened rather than merely rubber-stamped: the concurrency verifier found the SwmrSlotTable race's real reachability is *stronger* than first claimed (near-full occupancy makes long probe chains more likely to land on a just-vacated slot); the typography verifier traced the exact reachable code path into the pinned `skrifa` dependency's own real source, confirming an ordinary (non-malicious) single-point font contour reaches the bug, not only adversarial/fuzzed input; the security verifier traced finding #139's exact history and identified it as a genuine regression of already-fixed finding #106, reintroduced one day later by Step 5.1.2's `GlyphRasterSource`; the cross-phase verifier found Phase 6 Step 6.5 had the identical "no REVIEW.md section" gap finding #147 flagged for Phase 8, an earlier instance of the same lapse, not a counter-example; two verifiers each caught small arithmetic overstatements in their own Find agent's supporting numbers (corrected in the write-ups below) without weakening either finding's core claim; one verifier caught a single fabricated supporting quote (struck below) inside an otherwise-correct finding. Two independent dimensions (security and typography) converged on the identical root-cause bug (finding #139) from different angles, then were each independently re-confirmed by their own verifier -- four independent passes on the same bug, all agreeing.

Every confirmed finding was triaged the same way the Phase 1-4 review triaged its own: a contained, low-risk fix was applied directly wherever one existed; a finding whose real fix is substantial new architecture (deferred, fence-gated resource release; a cross-frame reuse API touching multiple crates' public surface; a data-structure change to the atlas eviction scan; a `Result`-return-type change touching every `execute_frame` call site; extracting ~500-750 lines of duplicated example boilerplate across 21-25 already-verified working files) was deliberately left as a clearly disclosed, documented gap rather than built opportunistically inside this pass -- consistent with this project's own precedent (findings #101-103/#109/#113-114/#116 above).

### Concurrency & data-race correctness

### 131. [Critical] `SwmrSlotTable::get`/`get_and_touch` could return a completely different key's value once the writer's real eviction-then-reuse sequence claimed the same physical slot mid-lookup
`probe_get` (finding the slot) and the subsequent value load are two separate atomic steps, not one. Between them, the atlas owner's own real eviction policy (Step 4.3.3) does exactly `remove(stale_key)` immediately followed by `insert(new_key, ...)` -- and `insert`'s own documented tombstone-reuse logic will happily claim that exact freshly-vacated slot for the new, unrelated key. A reader whose `probe_get` already found the slot before this race, then read `values[index]` after it, would read the *new* key's value while believing it answered its own, different key's lookup -- silent cross-key data corruption, not a crash, with no existing test (`concurrent_readers_never_see_a_torn_value_while_a_remove_races_them` only ever removes, never reinserts a *different* key into the freed slot) exercising this exact sequence. Independent verification found the reachability is if anything *stronger* than initially framed: eviction only runs once the table is already near-full (`EVICTION_CAPACITY_THRESHOLD`), so a real `insert`'s probe chain at that occupancy is long and more likely, not less, to revisit a just-vacated slot.

**Change:** the first fix attempt (re-loading `keys[index]` once, after the value read, rejecting a mismatch) was itself caught incomplete by the very stress test written to prove it: a real ABA case survives a single key recheck, since the writer's own real eviction cycle can move a slot key A -> key B -> key A again entirely within one reader's read window, letting the key match *again* by the time of the recheck while the value actually read came from the B-occupied instant in between. Fixed properly with a per-slot `epoch: Box<[AtomicU64]>` seqlock counter, bumped by the single writer as the last step of every `insert`/`remove` that touches a slot; `get`/`get_and_touch` now read `epoch` before and after their value/key reads and reject the result if either the key or the epoch changed -- catching a slot changing hands *any* number of times during the read, not just once. A real concurrency stress test (`a_reader_never_observes_a_different_keys_value_when_the_writer_evicts_and_immediately_reuses_its_slot`, `crates/tre-memory/src/swmr.rs`) forces sustained collisions via a capacity-1 table and 50,000 real remove/insert rounds racing a reader thread -- this test caught the first fix attempt's own real gap (6 cross-key observations in one run) before it shipped, and passes cleanly and repeatedly (20 consecutive real runs, no failures) against the epoch-based fix.

### 132. [Should-fix] `ScatterArena::reserve`'s unconditional `fetch_add` let one thread's overflow permanently poison the arena for every other thread's later, correctly-sized reservation
`len` was advanced by `count` *before* the capacity check ran, and never rolled back on failure. Since `len` only ever increases, one overflowing `reserve` call left it stuck above `capacity` for the arena's whole remaining lifetime -- every later call from any thread, however small, failed too, even one that would have easily fit in the space the failed caller's own advance "consumed" without ever using. `FrameArena`'s own real multi-worker `stitch_into` consumer (Step 5.2.2/5.2.3) means one oversized `SubCanvas` could silently wipe out an arbitrary, scheduling-dependent subset of otherwise-correctly-sized sibling canvases' geometry for that entire frame -- a substantially larger, non-deterministic blast radius than `stitch_into`'s own doc comment (which only describes the *failing* canvas's own loss) discloses.

**Change:** `reserve` now uses `fetch_update` instead of a bare `fetch_add`, committing the advance only when the reservation actually fits -- a failed reservation leaves `len` untouched. A new unit test (`one_overflowing_reserve_does_not_permanently_poison_capacity_for_a_later_smaller_one`) proves a smaller reservation still succeeds, and is correctly visible in `into_vec`'s output, after an earlier overflowing one.

### 133. [Nice-to-have] `A11yBridge::publish`'s two lock acquisitions are not atomic together
`self.state.nodes` and the live AT-SPI2 tree (`self.adapter`) are updated via two separate `Mutex` acquisitions, not one atomic step -- concurrent `publish` calls with different node sets could leave the two out of sync with each other. Every real caller today publishes from exactly one thread, so this is latent, not exercised; unlike this crate's other cross-thread-shared types (`SwmrSlotTable`, `MpscRingBuffer`), nothing previously documented this as a single-writer-only contract.

**Change:** documented directly on `publish`'s own doc comment (`crates/tre-a11y/src/lib.rs`) as a single-writer-only requirement, matching this project's own documentation discipline for its other concurrency-sensitive types. Not restructured (real fix -- one combined lock, or an explicit serialization contract -- has no current real motivating caller).

### Performance, allocation, and rendering-hot-path efficiency

### 134. [Should-fix] The engine's one real continuous-loop consumer allocates on the heap roughly 20+ times per frame, squarely inside DESIGN.md Section 2.1's own named zero-allocation boundary
`main_loop_demo.rs` (Step 8.1.2) constructs a fresh `Arc<FrameArena>` (4 `ScatterArena::with_capacity` allocations inside it) and up to 3 fresh `RenderingCanvas`/`SubCanvas` instances (each allocating its own `state_stack` at construction, plus 3 more on each canvas's first draw call) *inside* its own `while` loop, every frame; `FrameArena::flatten`/`segment_and_flatten` allocate several more `Vec`s consuming and rebuilding the arena's contents. Neither `ScatterArena`, `FrameArena`, nor `RenderingCanvas` has a `reset()`/reuse method anywhere -- architecturally, no caller can avoid rebuilding all three from scratch every frame. Every other windowed demo (`walking_skeleton`, `multi_window`, `input_demo`) sidesteps this by building its `RenderingCanvas` once, before its loop, and re-issuing the same static draw commands forever -- `main_loop_demo` is the first and only demo to record genuinely new per-frame content, and the first to expose this gap.

**Change:** not fixed here -- a real fix (an in-place `reset()`/`clear()` API retaining allocated capacity across frames, on all three types) touches public surface in both `tre-memory` and `tre-engine`, genuine new API design rather than a bug-fix-sized change. Disclosed directly in IMPLEMENTATION.md's Step 8.1.2 "Explicitly out of scope" list and `demo/phase8_step8_1_2/README.md`.

### 135. [Should-fix] `execute_frame` rebound the loop-invariant vertex/index buffer on every `DrawGeometry`/`PopLayer` command instead of once per frame
`vertex_buffer`/`index_buffer` are `execute_frame`'s own parameters, constant for its entire call -- yet `bind_vertex_buffer`/`bind_index_buffer` (each a real, unconditional `vkCmdBindVertexBuffers`/`vkCmdBindIndexBuffer` call, confirmed via the Vulkan implementation, not a cached no-op) were issued inside the per-command loop, once per `DrawGeometry` and once per `PopLayer` -- 2N redundant driver calls for N such commands, where 2 total would suffice, unlike `set_pipeline`/`bind_texture`, which legitimately vary between adjacent output commands even after batch-merging.

**Change:** hoisted both bind calls to run once, before the per-command loop begins -- a vertex/index buffer binding is command-buffer state that persists across `PopLayer`'s own render-target switch (`begin_render_to_texture`/`resume_swapchain_rendering` never touch it, unlike viewport/scissor, finding #128), so this is behavior-preserving. All 4 affected unit tests updated to assert the new, correct call order; re-verified against real GPU hardware via `main_loop_demo` and all 5 other affected demos, zero regressions.

### 136. [Should-fix] `AtlasOwner`'s per-insert eviction scan is a full `O(slot_capacity)` linear scan, repeated on *every* request once the atlas crosses 85% full, not amortized to once per threshold-crossing
`process_insert` calls `maybe_evict_stale_entries` unconditionally at the top of every real insert request; once at/above `EVICTION_CAPACITY_THRESHOLD`, `SwmrSlotTable::scan_older_than`'s full table scan (plus a growing heap `Vec` of matches) repeats on every subsequent request while occupancy stays at/above that threshold -- real, unamortized background-thread cost under exactly the sustained-near-capacity load the eviction feature exists to run in (DESIGN.md Section 10.2). Pre-existing Step 4.3.3 code, never previously reported; no current real caller exercises it repeatedly (every demo pre-seeds its atlas once and stops the owner thread before any real per-frame loop).

**Change:** not fixed here -- a real fix (re-scanning only periodically, or once the previous pass's freed budget is exhausted, rather than unconditionally) is a real data-structure/policy change to safety-relevant eviction logic, not attempted opportunistically. Documented directly on `maybe_evict_stale_entries`'s own doc comment (`crates/tre-atlas/src/owner.rs`).

### 137. [Nice-to-have] `Canvas::draw_text`'s cache-miss path re-parses the glyph outline and re-fires `request_insert` every single frame a glyph stays unresolved
No "already requested" tracking exists anywhere in the stack (`AtlasOwnerHandle::lookup`'s own doc comment states pending-vs-never-requested is deliberately indistinguishable) -- a glyph that takes more than one frame to resolve would repeat real outline extraction and a queue-push attempt every frame until resolution, on the caller's own recording thread. Latent today (every demo pre-seeds its atlas, so nothing here ever stays unresolved for more than one frame); would fire immediately for a real live-text-under-load consumer.

**Change:** not fixed here -- tracking in-flight-requested keys, or extending `lookup`'s own contract to distinguish "pending" from "never requested," is real future work. Documented directly on `draw_text`'s cache-miss branch (`crates/tre-engine/src/lib.rs`).

### Security, unsafe-code, and trust-boundary audit

### 138. [Critical] Sibling `PushLayer`/`PopLayer` pairs in one frame can alias the same bindless descriptor slot and the same physical transient texture before the GPU ever executes the earlier draw
`execute_frame` records an entire frame into one command buffer, submitted once at the end; `active_layer.is_none()` only rejects *nested* `PushLayer` (this project's own documented scope), so two or more sequential, sibling layers -- DESIGN.md Section 6.2's own named common case (multiple blurred/glassmorphism panels) -- are explicitly allowed. `register_bindless`/`deregister_bindless`/`release_transient_target` all run immediately at *recording* time (a single, non-frame-multiplexed `vk::DescriptorSet`; the transient pool's free list is a plain, immediately-reusable `Vec`), not GPU execution time. A second layer's `PopLayer` can free and immediately reuse the exact slot/texture a first layer's already-recorded (not yet executed) composite draw still references -- by submission time, the first layer's draw samples the second layer's own content instead of its own, with no Vulkan validation-layer warning at all (valid API usage, wrong data referenced -- the same class as finding #128). No test exercises two sibling layers in one frame; `main_loop_demo.rs` never calls `push_layer` at all, so this has not yet manifested in any real demo.

**Change:** not fixed here -- the real fix (deferring `deregister_bindless`/`release_transient_target` until the GPU has actually finished with the resource, fence-gated, mirroring this project's own generational-GC deferred-release pattern, TECHNICAL.md Section 3.3) is genuine new cross-cutting architecture touching frame-lifecycle plumbing, not attempted opportunistically. Disclosed directly in IMPLEMENTATION.md's Step 6.4.2 section (where the affected code was introduced) and cross-referenced from Step 8.1.2's own out-of-scope list.

### Typography pipeline correctness end to end

### 139. [Critical] `Canvas::draw_text`'s "no real ink -> skip" guard checked a narrower condition than `generate_msdf`'s own real degeneracy check, reopening finding #106's panic one layer up
`draw_text` gated `GlyphRasterSource` construction on `!outline.is_empty()` alone -- the literal empty-contour-list case (whitespace). But a *non-empty* `Vec<Contour>` whose points are all coincident or non-finite (a lone `MoveTo`+`Close` pair with no drawing segment between them) still fails `bounding_box`'s own real degeneracy check, so `generate_msdf` legitimately returns `None` for it -- and `GlyphRasterSource::rasterize()`'s `.expect(...)` on that `None` panics, unguarded, on the atlas owner's *shared background thread* (no `catch_unwind` anywhere in the crate), silently killing glyph resolution for the rest of the session and re-panicking whichever thread later calls `AtlasOwner::join()`. Independently found and confirmed by two separate dimensions of this review (typography and security), then independently re-verified by both dimensions' own verifiers -- one of which traced the exact reachable path into the pinned `skrifa` dependency's own real `to_path` source, confirming a real single-point TrueType contour (a legitimate, non-malicious artifact some real-world fonts contain -- editor-tool leftovers, cursive-attachment anchor points -- not only fuzzed/corrupted data) reaches this. This is a genuine regression of already-fixed finding #106: `generate_msdf`'s own contract changed from "panics" to "returns `None`" during the Phase 1-4 review, but `GlyphRasterSource` (added one step later, Step 5.1.2, on the *production* `Canvas::draw_text` path this time, not an example/test) reintroduced an `.expect()` on that exact `None`, and `draw_text`'s own guard was never widened to match.

**Change:** added a new public `tre_text::has_real_ink(contours: &[Contour]) -> bool` (a thin, correct wrapper over the same real `bounding_box` check `generate_msdf` itself uses), and changed `draw_text`'s guard from `!outline.is_empty()` to `tre_text::has_real_ink(&outline)`. Both doc comments (`draw_text`'s own, and `GlyphRasterSource::rasterize`'s implicit contract) updated to name the real condition. New unit tests in `tre-text` (`has_real_ink_is_false_for_a_non_empty_but_single_point_contour`, plus agreement/regression tests against the existing empty-contour and `generate_msdf` behavior) cover exactly the gap the old guard missed. Re-verified against real GPU hardware: all 6 text-drawing demos re-run, zero regressions.

### Code quality, API design, and cross-crate consistency

### 140. [Should-fix] ~21-25 of this project's 30 example files duplicate verbatim device-bootstrap, shader-loading, and PNG-output boilerplate
The device-bootstrap block (`PlatformConnection::new` -> probe window -> `VulkanDevice::new` -> `destroy_surface`) is byte-for-byte identical, differing only in a window-title string, across at least 21 files; shader-loading boilerplate across 25; PNG-write boilerplate across 21. This project already solved "share code between examples with no shared library crate for this" once, for finding #108's `pixel_helpers::bgra_pixel_at` -- the same mechanism was never applied to these two much larger sources of duplication in the same directory, and no doc comment anywhere states self-containment as a deliberate choice.

**Change:** not fixed here -- extracting shared `support/bootstrap.rs`/`support/shader_loader.rs`/`support/png_writer.rs` helpers touches ~21-25 already-verified working files at once, matching this project's own explicit precedent for finding #109 ("the risk of introducing a subtle... regression outweighs the benefit of removing pure duplication... deliberately not attempted opportunistically inside this review/fix pass"). Documented here as a confirmed, real finding for a future dedicated refactor pass.

### 141. [Should-fix] `execute_frame` panics on exactly the failures `EngineError`/DESIGN.md Section 2.6 classify as recoverable, and its own doc comment cited a documentation disclosure that did not exist
`acquire_transient_target`/`register_bindless` both return `Result<_, EngineError>` -- `TransientPoolBudgetExceeded`'s own doc comment calls that specific failure "recoverable: a caller can release outstanding textures, wait for the GC thread to catch up, and retry" -- yet `execute_frame` (no `Result` return type) `.expect()`s both. Its own `# Panics` section claimed this was "an honest, documented limit... see IMPLEMENTATION.md Step 6.4.2's own write-up for why," but a direct grep and full read of that section found no such disclosure anywhere in the document -- the citation was simply false.

**Change:** the false citation is corrected -- `execute_frame`'s own doc comment now states plainly that this is a real, undisclosed gap (not a previously-documented limit) and names the real fix (a `Result<(), EngineError>` return type, propagating both `Err`s, updating every real call site). The signature change itself is not made here -- genuine, call-site-touching API surface work, not a doc-comment-sized fix -- and remains open, now honestly disclosed rather than falsely claimed as already covered.

### 142. [Nice-to-have] `RhiDynamicRingBuffer::write` returns `Option<u32>`, not `Result<u32, EngineError>`, narrower than DESIGN.md Section 2.6's own blanket policy
Section 2.6's governing sentence requires every fallible operation to return `Result<T, EngineError>`, and its own "ring buffer starvation" bullet describes graceful degradation (dropping lowest-priority pending draws), not a bare `None`. `main_loop_demo.rs`, the first real per-frame caller, currently `.expect()`s this -- starvation crashes the process today, not graceful degradation.

**Change:** flagged directly on `write`'s own doc comment as a narrower contract than policy, so a future reader doesn't assume it already matches DESIGN.md Section 2.6. Not restructured -- implementing the real policy belongs with the overlay-priority/depth-sorting machinery, real future work.

### 143. [Nice-to-have] `main_loop_demo.rs`'s `Renderer` struct omitted the "field order matters" comment its only sibling (`walking_skeleton.rs`) carries, despite the identical footgun shape
Both structs hold an RHI-resource field (`atlas_texture`/none, respectively) that must drop before `device` for the reason finding #43's real historical SIGSEGV established -- `walking_skeleton.rs` documents this; `main_loop_demo.rs`'s field order is actually correct, but carried no comment warning a future field addition about it.

**Change:** added the equivalent comment, citing both finding #43 (the original bug) and this finding.

### 144. [Nice-to-have] `spring_decay` omitted the `# Panics` doc section every other public function in `tre-math` carries
`compose_batch`, `lerp_points_batch`, and `tone_map` all document `# Panics`; `spring_decay` (Step 8.1.1) -- the same shape of pure, non-panicking function -- did not.

**Change:** added a `# Panics: Never` section matching the file's own established convention.

### Cross-phase integration and documentation accuracy

### 145. [Should-fix] REVIEW.md's own top status preamble was stale, silently omitting the two most consequential unresolved items in the entire document
The preamble paragraph named sections in order only through "Phase 5 Step 5.1.1 Implementation," then stopped -- 14 more headed sections follow it, including the still-open Step 5.3.3 CI investigation (ending "STOPPING POINT... Not fixed") and the still-open Step 7.2.1 Dual-Kawase blocker (finding #130, "STOPPED, unfixed"). A reader who trusted the preamble as this document's own index would have no idea either genuinely open bug exists.

**Change:** extended with a new paragraph indexing every section from Phase 5 Step 5.1.2 through this section itself, matching the original paragraph's own density and explicitly naming both open blockers' real status.

### 146. [Critical] `demo/phase5_step5_3_3/README.md` presented a since-disproven diagnosis as fact, and omitted that Step 5.3 is still open
The README stated a "seven-real-push CI investigation... found the actual reason" this demo failed in CI (a Vulkan/X11-linking theory) -- but REVIEW.md's own later investigation directly disproved this exact claim (a genuinely Vulkan-free process failed identically), and IMPLEMENTATION.md's own Step 5.3.3 section was already corrected to say so explicitly ("An earlier version of this section claimed a 'real root cause'... and declared Step 5.3 closed on that basis -- both wrong, corrected here"). The demo README, stating the identical now-disproven claim, was never updated to match, and never stated the current, real status ("CI's own `accessibility-validation` gate not yet passing -- Step 5.3 NOT yet closed").

**Change:** rewrote both the status framing (added at the top) and the disproven-diagnosis paragraph to match IMPLEMENTATION.md's own corrected account exactly, including an explicit note that the two-process architecture itself remains correct, just not for the reason originally given.

### 147. [Should-fix] Phase 6 Step 6.5 and Phase 8 (Steps 8.1.1/8.1.2) never got their own REVIEW.md sections, breaking this project's own "every step gets indexed, even a clean one" convention
Every other step -- including genuinely zero-finding ones (4.2.3, 4.3.2) -- has its own REVIEW.md section explicitly recording its status. Grep confirmed no `## Phase 6 Step 6.5` or `## Phase 8 Step 8.1.1`/`8.1.2` heading existed anywhere, even though IMPLEMENTATION.md's own Step 8.1.2 write-up describes two real, previously-undiscovered gaps in exactly the style every numbered finding uses.

**Change:** added `## Phase 6 Step 6.5 Implementation`, `## Phase 8 Step 8.1.1 Implementation`, and `## Phase 8 Step 8.1.2 Implementation` sections above, retroactively, matching this document's own established format and cross-referencing the real gaps each step's own IMPLEMENTATION.md write-up already describes.

### 148. [Nice-to-have] ARCHITECTURE.md's canonical RHI trait sketch never got a "Phase 7 Step 7.2.1" annotation for `begin_render_to_texture_no_end`
Every other real trait addition since this section's original sketch (`begin_render_to_texture`, `end_render_to_texture`, `resume_swapchain_rendering`, `register_bindless`/`deregister_bindless`, Step 6.4.1) is named via its own dated annotation; `begin_render_to_texture_no_end` (added one step later, real and used by `dual_kawase_blur_demo.rs`) was never added, even though IMPLEMENTATION.md documents it correctly.

**Change:** added the missing annotation, matching the section's own established format exactly.

### 149. [Should-fix] DESIGN.md presented Dual-Kawase blur and blend modes as current, unconditional, working capabilities
Blur is completely non-functional (finding #130, "STOPPED... Not fixed"); blend modes (Multiply/Screen/Overlay/Soft Light/Color Dodge) have zero implementation anywhere in the codebase -- confirmed via a repo-wide grep, and independently confirmed by this project's own archived plan (`planning/archive/PLAN_PHASE7_STEP7_2_1.md`) explicitly naming them "out of scope... never part of IMPLEMENTATION.md's own real Step 7.2 task list." DESIGN.md's own "Implementation status" annotation convention (used 3 times, all for Phase 5 accessibility) was never applied to this section despite two real phases (6, 7.2.1) landing directly on its content.

**Change:** added an "Implementation status (Phase 7 Step 7.2.1)" annotation stating both real statuses plainly, matching the existing convention's own format.

### 150. [Nice-to-have] `main_loop_demo.rs` never exercises `push_layer`/`pop_layer` across frames, and this was not listed among its disclosed out-of-scope items
No demo has ever run the render-to-texture/layer-compositing path (Steps 6.4.1/6.4.2/6.5) across more than one isolated, single-frame `execute_frame` call -- a real cross-phase seam (Phase 6 x Phase 8) neither phase's own individual review could have seen alone. IMPLEMENTATION.md's own "Explicitly out of scope (Step 8.1.2)" list named several other deliberate exclusions but not this one.

**Change:** added to that list and to `demo/phase8_step8_1_2/README.md`, cross-referencing finding #138 (which this gap in coverage means has never actually manifested in a real demo).

### 151. [Should-fix] The disclosed swapchain-resize gap (finding #116) becomes a concrete, guaranteed-panic risk at Phase 8.1.2, uncross-referenced there
Finding #116 disclosed that `acquire_next_image`/`present` correctly surface `EngineError::SwapchainOutOfDate` but nothing anywhere recovers from it. `main_loop_demo.rs`'s own `.expect("begin_frame failed")`/`.expect("submit_and_present failed")` calls mean resizing its window mid-run panics the whole process -- the first place this long-disclosed gap becomes the concrete behavior of this project's own reference "how you write the main loop" implementation, not just a latent risk in a short-lived demo.

**Change:** added a cross-reference to finding #116 in IMPLEMENTATION.md's Step 8.1.2 out-of-scope list and `demo/phase8_step8_1_2/README.md`, stating the concrete resize-panics-the-process behavior plainly.

## Summary table (Phase 1-8 Comprehensive Review)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 131 | `SwmrSlotTable::get`/`get_and_touch` could return a different key's value across a real evict-then-reuse race (a genuine ABA hazard, not caught by a plain key recheck) | tre-memory | Critical | Fixed -- per-slot `epoch` seqlock counter, bumped on every mutating `insert`/`remove`; new 50,000-round stress test caught the first, incomplete fix attempt before it shipped |
| 132 | `ScatterArena::reserve`'s `fetch_add` permanently poisoned capacity after one overflow | tre-memory | Should-fix | Fixed -- switched to `fetch_update`, only commits when it fits; new regression test |
| 133 | `A11yBridge::publish`'s two lock acquisitions are not atomic together | tre-a11y | Nice-to-have | Documented as single-writer-only on `publish`'s own doc comment; not restructured |
| 134 | `main_loop_demo`'s continuous loop allocates ~20+ times per frame, inside DESIGN.md's own zero-allocation boundary | tre-engine, tre-memory, tre-rhi-vulkan (examples) | Should-fix | Documented in IMPLEMENTATION.md Step 8.1.2 and the demo README; not fixed (needs a cross-crate reset/reuse API) |
| 135 | `execute_frame` rebound the vertex/index buffer on every command instead of once per frame | tre-engine | Should-fix | Fixed -- hoisted above the per-command loop; 4 tests updated, re-verified on real GPU hardware |
| 136 | `AtlasOwner`'s eviction scan is `O(capacity)` and unamortized once ≥85% full | tre-atlas | Should-fix | Documented on `maybe_evict_stale_entries`'s own doc comment; not fixed |
| 137 | `draw_text`'s cache-miss path re-fires `request_insert` every frame a glyph stays unresolved | tre-engine | Nice-to-have | Documented on `draw_text`'s own miss branch; not fixed |
| 138 | Sibling `PushLayer`/`PopLayer` pairs in one frame can alias the same bindless slot/texture before GPU execution | tre-engine, tre-rhi-vulkan | Critical | Documented in IMPLEMENTATION.md Step 6.4.2 and Step 8.1.2's out-of-scope list; not fixed (needs fence-gated deferred release) |
| 139 | `Canvas::draw_text`'s ink-detection guard was narrower than `generate_msdf`'s real degeneracy check, reopening finding #106 one layer up | tre-engine, tre-text | Critical | Fixed -- new `tre_text::has_real_ink` used as the real guard; new unit tests; independently found and confirmed by 2 dimensions, 4 total passes |
| 140 | ~21-25 of 30 example files duplicate device-bootstrap/shader-load/PNG-write boilerplate verbatim | tre-rhi-vulkan (examples) | Should-fix | Documented; not refactored (touches too many working files opportunistically, matching finding #109's own precedent) |
| 141 | `execute_frame` panics on recoverable `EngineError` failures; its own doc comment cited a documentation disclosure that did not exist | tre-engine, documentation | Should-fix | Fixed the false citation; the underlying `Result`-return-type change itself remains open, now honestly disclosed |
| 142 | `RhiDynamicRingBuffer::write` returns `Option`, not `Result`, narrower than DESIGN.md's own blanket policy | tre-engine | Nice-to-have | Documented on `write`'s own doc comment; not restructured |
| 143 | `main_loop_demo`'s `Renderer` struct omitted the field-order-matters comment its sibling carries | tre-rhi-vulkan (examples) | Nice-to-have | Fixed -- comment added |
| 144 | `spring_decay` omitted the `# Panics` section every other function in its module carries | tre-math | Nice-to-have | Fixed -- section added |
| 145 | REVIEW.md's own top status preamble was stale, omitting two open blockers | documentation | Should-fix | Fixed -- preamble extended through this section |
| 146 | `demo/phase5_step5_3_3/README.md` presented a since-disproven diagnosis as fact, omitted that Step 5.3 is open | documentation | Critical | Fixed -- rewritten to match IMPLEMENTATION.md's own corrected account |
| 147 | Phase 6 Step 6.5 and Phase 8 (8.1.1/8.1.2) never got their own REVIEW.md sections | documentation | Should-fix | Fixed -- all 3 sections added retroactively |
| 148 | ARCHITECTURE.md's RHI trait sketch is missing `begin_render_to_texture_no_end` | documentation | Nice-to-have | Fixed -- annotation added |
| 149 | DESIGN.md presents Dual-Kawase blur and blend modes as working, unconditional capabilities | documentation | Should-fix | Fixed -- implementation-status annotation added, stating both are non-functional/never built |
| 150 | `main_loop_demo` never exercises `push_layer`/`pop_layer` across frames, undisclosed as out of scope | documentation | Nice-to-have | Fixed -- added to the out-of-scope list, cross-referencing finding #138 |
| 151 | The disclosed swapchain-resize gap (#116) becomes a guaranteed-panic risk in Step 8.1.2, uncross-referenced there | documentation | Should-fix | Fixed -- cross-reference added to IMPLEMENTATION.md and the demo README |

Verified by `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` clean across the whole workspace (zero failures across every crate, including 2 new `tre-memory` tests for findings #131/#132, 3 new `tre-text` tests for finding #139) and real GPU re-runs: `main_loop_demo` (90 frames, zero validation errors, animation re-verified) and all 6 other demos whose call sites changed (`canvas_layer_composite_demo`, `canvas_sub_canvas_demo`, `canvas_batch_flattening_demo`, `canvas_state_stack_demo`, `canvas_combined_scene_demo`, `canvas_draw_text_demo`) re-run manually end to end against real Vulkan hardware, zero regressions.

## Phase 9 Step 9.1 Implementation (2026-09-09)

Real pre-work investigation, done before writing any test per this
step's own task list, found two of that task list's own premises no
longer matched the real codebase -- both real documentation/code
discrepancies, not test-authoring mistakes, and both surfaced to the
project owner via `AskUserQuestion` before any code was written, since
each was a genuine scope fork.

### 154. [Should-fix] TECHNICAL.md Section 4 and ARCHITECTURE.md Section 4.1 documented a 4-pass radix sort for the draw-command sort key that had never actually been built
Both sections, and this document's own Architectural Decision Matrix
above ("Command Sorting: 4-Pass Radix Sort ($\mathcal{O}(N)$)... Guarantees
deterministic sub-millisecond sorting times even when UI trees contain
over 10,000 active nodes"), described a real algorithm; `UiDrawCommand::
sort_key`'s own doc comment already called it a "64-bit Radix Sort Key."
The real `flatten_run`, unchanged since Step 5.1.3 first built it, has
always called `std::sort_unstable_by_key` -- a comparison sort -- instead.
Output was never incorrect (both algorithms produce the same total
order), but the documented $\mathcal{O}(N)$ determinism guarantee at
large node counts was never real.

**Change:** confirmed with the project owner via AskUserQuestion
("Build the real radix sort now") rather than only correcting the
documentation. Built `radix_sort_by_key` (`tre-engine`), a genuine
4-pass, 16-bit-digit LSD radix sort with no per-call heap allocation
(ping-pongs caller-provided `items`/`scratch` buffers), wired through
`segment_and_flatten`/`flatten_run`/`RenderingCanvas::flatten()`/
`FrameArena::flatten()`. Adversarially tested (all-identical keys,
reverse-sorted input, field-boundary-clustered keys, maximum Depth ID
values, empty/single-element runs, a mismatched-scratch-length panic
guard, 200 rounds of randomized inputs checked against `sort_unstable_
by_key` as an independent oracle) and further proven correct end to end
by a real GPU pixel-diff (`batching_equivalence_demo.rs`, finding
#154's own real consumer) showing the new sort produces byte-for-byte
identical rendered output to the old comparison sort. See IMPLEMENTATION.md
Step 9.1 for the full account.

### 155. [Should-fix] DESIGN.md Section 2.6 documented an atlas-exhaustion placeholder-glyph fallback that had never been built; the real code silently drops the request instead
Section 2.6 described falling back to "a lower-fidelity placeholder
(e.g., a bounding-box glyph or solid-color swatch)" when eviction
cannot free enough atlas space. The real `AtlasOwner::process_insert`
has always silently dropped the request instead (`let Some(rect) =
packer.insert(..) else { return; }`) -- a prior session's own code
comment had already independently reasoned this still satisfies the
section's "report, don't block" contract (no panic, no corrupted atlas
state, no blocked frame), just not via the specific mechanism the
documentation promised.

**Change:** confirmed with the project owner via AskUserQuestion
("Correct the docs, test the real drop behavior") -- rewrote DESIGN.md
Section 2.6 to describe the real silent-drop behavior instead of
building the never-implemented placeholder-glyph path opportunistically
inside a testing step. Added a dedicated regression test in `tre-atlas`
(`a_request_that_can_never_fit_even_after_a_full_eviction_pass_is_
silently_dropped`) proving a permanently-oversized request is dropped
every time across 1000 real polling iterations, without ever wedging
the atlas's background thread or corrupting a subsequent normal
insert. See IMPLEMENTATION.md Step 9.1 for the full account.

Also fixed during this step, precisely characterizing (via two real
test-failure iterations against the new fragmentation/eviction test,
not assumption) two non-obvious real boundary conditions in the
Guillotine atlas's existing eviction logic -- not new findings, since
no doc or shipped code ever asserted the wrong behavior; only a
same-session test draft's own mental model was wrong before being
corrected: `maybe_evict_stale_entries`'s `EVICTION_CAPACITY_THRESHOLD`
check reads **pre-insert** usage, not accounting for the incoming
request's own space; `SwmrSlotTable::scan_older_than`'s staleness
comparison is **strict** (`last_used < cutoff_frame`), so an entry idle
for *exactly* `EVICTION_MIN_IDLE_FRAMES` survives. Both are now
permanent, passing regression coverage.

Verified by `cargo fmt --all -- --check`/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo build --workspace --all-targets`/
`cargo test --workspace` clean across the whole workspace (`tre-engine`
72 tests, up from 64; `tre-atlas` 22 tests, up from 18; `tre-svg` 28
tests, up from 25, including 3 new `proptest`-based adversarial-input
properties substituting for `cargo-fuzz`, disclosed as unavailable in
this environment -- no nightly Rust toolchain installed). A full manual
regression sweep of all 31 real Vulkan demos re-run after the sort-
algorithm swap under `flatten()`/`execute_frame`'s entire draw
pipeline: 30 passed; `canvas_accessibility_verify` failed only on the
same pre-existing, already-documented environmental limitation as
finding #126 (no AT-SPI registry daemon in this sandbox), unrelated to
any change this step made. New demo `batching_equivalence_demo` added
to `ci.yml`'s `vulkan-validation` job.

## Phase 9 Step 9.2 Implementation (2026-09-09)

Real pre-work investigation, before writing any code, found this
step's own task list rested on primitives that had never actually been
built: TECHNICAL.md Section 3.4's zero-allocation debug guard (no
`global_allocator`/`GlobalAlloc`/`thread_local` anywhere in the
codebase) and Section 9.2's own `criterion`-based performance suite (no
`criterion` dependency, no `benches/` directory anywhere) -- both
confirmed with the project owner via `AskUserQuestion` before
proceeding, alongside the real, disclosed risk that wiring the guard to
the real continuous main loop would immediately fail on finding #134's
own already-disclosed ~20+ allocations/frame. All three were confirmed:
build the real guard, fix finding #134 first, and build the minimal
criterion bench too.

### 156. [Should-fix] `VulkanDevice::begin_frame` allocates a fresh `Box<dyn RhiCommandBuffer>` every frame, even though the underlying Vulkan command buffer handle it wraps is already reused
Surfaced only once the real zero-allocation debug guard existed to
catch it -- `begin_frame`'s own comment already states the Vulkan-level
`vk::CommandBuffer` handle is "the one persistent command buffer
(allocated once in `new`)," but the Rust-level `VulkanCommandBuffer`
wrapper struct holding it is still `Box::new`'d fresh every single call,
purely to satisfy `RhiDevice::begin_frame`'s own `Box<dyn
RhiCommandBuffer>`-by-value return type.

**Change:** not fixed here -- a real fix means redesigning `RhiDevice::
begin_frame`/`submit_and_present`'s ownership model away from consuming
`Box<dyn RhiCommandBuffer>` by value, which would ripple through all 31
demo call sites that construct/consume one -- genuine trait-boundary
redesign, not a bug-fix-sized change, the same shape finding #134 itself
has and was legitimately deferred with that exact reasoning. Disclosed
directly in `main_loop_demo.rs`'s own header comment as the reason RHI
submission stays outside this step's new `RenderTickGuard` coverage.

### 157. [Critical] `radix_sort_by_key`'s own `counts` histogram buffer was allocated fresh on every call, not just once per frame -- a real bug the new zero-allocation guard caught on its very first real run against `main_loop_demo`
Step 9.1's own "allocate once, reuse across the frame" discipline was
applied to `radix_sort_by_key`'s `scratch` parameter but missed the
function's *own* internal `counts` buffer (`vec![0u32; RADIX_BUCKETS]`,
65,536 elements = 256 KiB), which was constructed fresh inside the
function on every single call -- once per marker-free run within every
`flatten_run` call, every frame. Real, previously undetected: Step
9.1's own randomized/adversarial tests never checked for allocation
behavior, only sort correctness.

**Change:** `counts` is now a caller-provided, reused `&mut Vec<u32>`
parameter threaded through `radix_sort_by_key` -> `flatten_run` ->
`sort_and_batch_into`, with a matching new persistent field on
`FrameArena` (alongside `raw_commands`/`raw_indices`/`sort_scratch`) for
the real, reused `flatten_into` path; `segment_and_flatten`'s own
consuming path allocates a fresh one per call, matching `scratch`'s own
existing behavior there. Since `RADIX_BUCKETS` is a fixed compile-time
constant, a persistent `counts` reaches its final length on its first
call and never resizes again. New regression test
(`radix_sort_reuses_the_same_counts_buffer_across_calls_without_
reallocating`) asserts the buffer's own pointer is stable across two
independent sort calls.

### 158. [Nice-to-have] `std::thread::scope` allocates an `Arc<ScopeData>` bookkeeping value on every call -- a real, unavoidable cost of this project's own "fresh OS thread every frame" design, only visible once a zero-allocation guard actually wrapped it
Found during this step's own development: wrapping the *entire*
per-frame span (including the `std::thread::scope` call itself) in
`RenderTickGuard` panicked on `std::thread::scope`'s own internal `Arc`
allocation, not on any TRE-side code. This is real standard-library
behavior, not a bug -- `std::thread::scope` must allocate shared
bookkeeping so its spawned threads can synchronize back to the caller
before it returns.

**Change:** not fixed here -- eliminating it requires a persistent,
reused worker-thread pool, already named as real, separate future work
by Step 8.1.2's own "Explicitly out of scope" list, unchanged and
re-confirmed by this step's own `PLAN.md`. `main_loop_demo.rs`'s
`RenderTickGuard` coverage is split into two spans (root recording, and
stitch/sort-batch/ring-buffer-write) bracketing the unguarded
`std::thread::scope` call, disclosed directly in the demo's own header
comment; each worker's own guard, started inside its spawned closure,
still covers that worker's real per-frame work in full.

### 159. [Should-fix] Real, measured frame-processing performance (~0.80ms at the Architectural Decision Matrix's own 10,000-node scale) exceeds the documented $\le 0.50\text{ ms}$ CPU budget (TECHNICAL.md Section 9.2)
The new `record_and_flatten_10k_nodes` criterion benchmark
(`crates/tre-engine/benches/frame_processing.rs`) is the first time this
budget has ever been measured against real code -- no benchmark of any
kind existed before this step. Real result: ~796µs mean, roughly 1.6x
the documented budget.

**Change:** not fixed here -- confirmed with the project owner:
optimizing the sort/flatten hot path to actually meet the budget is
real, separate performance-tuning work (TECHNICAL.md Section 9.2's own
scope, not this correctness/CI-gating step's), and the new `ci.yml`
`test` job step that runs this bench and checks its result against the
budget is deliberately wired to fail honestly on this real, pre-existing
gap rather than silently passing or having its threshold quietly
loosened to match current reality. This CI job is expected to be red
until real optimization work lands as a future step.

Verified by `cargo fmt --all -- --check`/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo build --workspace --all-targets`/
`cargo test --workspace` clean across the whole workspace (`tre-memory`
41 tests, up from 32, including 6 new tests for the real zero-allocation
debug guard; `tre-engine` 78 tests, up from 72). `main_loop_demo.rs`,
rebuilt to reuse every per-frame structure via the new `reset()`/
non-consuming `stitch_into`/`flatten_into` APIs and wrapped in the real
`#[global_allocator]` guard, re-run live against real GPU hardware
multiple times: 90 real frames each run, zero allocations detected
inside any guarded span, animation independently re-verified. A full
manual regression sweep of all 31 pre-existing Vulkan demos re-run after
`stitch_into`'s signature change (consuming -> borrowing) and
`radix_sort_by_key`'s new `counts` parameter: zero regressions beyond
the same pre-existing, already-documented `canvas_accessibility_verify`
environmental limitation (finding #126).

## Follow-up: Python Bindings via Direct PyO3, Bypassing `tre-ffi` (2026-09-09)

Status: **Documented, not yet implemented** (Phase 10 Steps 10.2/10.3
are still both planned, not built) -- this is a subsequent project
decision revising Phase 10's own planned architecture before any of it
ships, not a finding from a review of shipped code, recorded here as a
follow-up entry matching finding #18's own precedent (the original
Rust/Python language decision) rather than folded into the numbered
findings above.

### 160. [Decision] Python UI framework bindings will use PyO3 directly against `tre-engine`'s native Rust API, not `tre-ffi`'s C-ABI

Every document previously specified one cross-language boundary --
`tre-ffi`'s `#[repr(C)]`/`extern "C"` API -- shared by every language
binding, Python included. DESIGN.md Section 2.7 stated this explicitly
and repeatedly: Python binds "through this same C-ABI boundary,"
receiving "no engine access that a UI framework written in C++, C#, or
any other language could not also obtain through the same bindings."
IMPLEMENTATION.md's original Step 10.3 (Python bindings) task list
required "wrapping `tre-ffi`'s C-ABI -- not calling into `tre-engine`
internals directly -- so the Python bindings exercise the identical
boundary any other language would use."

**Change, per explicit project-owner direction:** `tre-ffi` (Phase 10
Step 10.2) remains real and necessary -- the stable C-ABI boundary for
every language *other* than Python (C, C++, any future non-Python
dynamic-language binding). The project's own Python UI framework
(Phase 10 Step 10.3) now binds directly to `tre-engine`'s native Rust
API via a new, dedicated `tre-python` crate using PyO3's `#[pyclass]`/
`#[pymethods]` machinery, depending on `tre-engine` directly and never
depending on `tre-ffi` at all. **Real, stated rationale: efficiency and
performance** -- a `tre-ffi`-routed binding pays a double marshalling
cost on every call (native Rust type -> C-compatible shadow
representation -> PyO3 conversion back to a Python object) and opaque-
handle-plus-getter/setter indirection for high-frequency calls (e.g., a
Phase 10 Step 10.1 shape's own per-frame property mutation) that a
direct binding has no reason to pay. PyO3's own built-in mechanisms
replace what `tre-ffi`'s hand-written C-ABI machinery would otherwise
need to provide for Python specifically: `Drop`-integrated ownership
(no `tre_*_free` calls), a native panic-to-exception boundary (no
hand-written `catch_unwind` wrapper), and direct `Result<T, EngineError>`
-to-`PyErr` conversion (no integer result code round trip).

**This is a genuine, disclosed departure from this project's own
previously-stated "one boundary, no privileged access" principle, not a
silent one.** Python -- as the project's own first-party UI framework,
not a third-party integration -- now receives real, privileged access
(direct native-Rust-type binding) that a UI framework written in any
other language does not. DESIGN.md Section 2.7 is corrected to state
this plainly under a new "Cross-Language Boundary: Two Real Paths"
heading rather than leave the superseded "equal footing" claim standing
uncorrected -- the same discipline finding #145 (a stale REVIEW.md
preamble) and finding #146 (a since-disproven demo README claim)
already established for this document set: correct a stated claim
openly, with a dated note, rather than silently edit around it.

**Per-document changes:**

- **DESIGN.md:** Section 2.7 rewritten with a new "Cross-Language
  Boundary: Two Real Paths" subsection stating the real architecture
  and the performance rationale; its own FFI-safety paragraph and the
  Executive Summary's/Section 3's own Python-related claims corrected
  to match.
- **TECHNICAL.md:** Section 9.4 split into 9.4.1 (`tre-ffi`, for C/C++/
  other non-Python bindings, otherwise unchanged) and 9.4.2 (`tre-python`,
  the new direct PyO3 binding mechanism, ownership, concurrency, panic
  safety, and testing story). Section 9.1's `panic = "unwind"`
  requirement extended to cover `tre-python`'s own build for the same
  underlying reason. Section 9.2's Build System bullet updated to name
  the new `tre-python` crate as a second, independent build output.
- **IMPLEMENTATION.md:** Step 10.2 (`tre-ffi`) re-scoped to "for C, C++,
  and other non-Python bindings," with a new task building `tre-ffi`'s
  own dedicated non-Python test harness (Python's test suite no longer
  covers it). Step 10.3 (Python bindings) rewritten in full: a new
  `tre-python` crate, direct `#[pyclass]`/`#[pymethods]` wrapping, and
  the corrected technical rationale. Step 10.1's own cross-reference to
  "Python via ... `tre-ffi`" corrected.
- **ARCHITECTURE.md:** Section 7 (the planned shape-primitive system,
  written earlier this same session) had three passages describing
  Python reaching `ShapeId` through `tre-ffi`'s opaque-handle pattern --
  all three corrected to describe the real, now-two-path design: `tre-ffi`
  opaque handles for non-Python languages, direct PyO3 `#[pyclass]`
  wrapping for Python.
- **`PLAN.md`** (Phase 10 Step 10.1's own working plan, not yet
  archived): its own three references to Python reaching this layer
  via `tre-ffi` corrected to match.

**Note for future reviewers:** Like finding #18, this is a language/
architecture-boundary decision record, not a re-review -- the shape-
primitive data model itself (ARCHITECTURE.md Section 7's structs/enums)
is unaffected; only *which mechanism* Python uses to reach it changed.
No code exists yet for either `tre-ffi` or `tre-python` (Phase 10 is
still entirely planned, not implemented), so there is no shipped
behavior this decision contradicts -- only prior *documentation* of a
planned architecture, corrected before it was ever built the old way.

## Phase 10 Step 10.1 Implementation (2026-09-09)

### 161. [Should-fix] IMPLEMENTATION.md's own Step 10.1 task 4 claimed `Path` shapes could be filled via `tre-svg`'s existing tessellator; real implementation found this needs new, not-yet-built geometry work
Task 4's original wording named `tre-svg`'s existing ear-clipping
tessellator (Phase 3 Step 3.3.1) as the real rendering path for filled
`Path` shapes -- implying "wire an existing function," a bug-fix-sized
addition. Investigating what that would actually take, before writing
the flattening pass, found this is not accurate: `tre-svg`'s
tessellator consumes an already-flattened polygon point list, and the
new `Path::commands: Vec<PathCommand>` (`MoveTo`/`LineTo`/quadratic/
cubic Bezier segments) has no existing flattening step anywhere in this
codebase to convert it into that point-list form -- SVG's own path
parsing (Step 3.3.1) does this internally via the `usvg` dependency,
not through any function `tre-engine`'s new `shapes` module could call
directly. Real, separate geometry work (Bezier subdivision/flattening
at some tolerance), not a bug-fix-sized addition.

**Change:** `ShapeRegistry::flatten_into` panics loudly
(`unimplemented!`) on any `Path` shape, fill or stroke both, rather than
half-implementing fill support the task list's own wording assumed was
trivial. IMPLEMENTATION.md Step 10.1's "Explicitly out of scope" list
corrected to name `Path` rendering as entirely out of scope (was
previously only naming stroking), matching the same "confirmed via
investigation, corrected rather than silently built around or silently
expanded" discipline every prior step's own real discrepancies (radix
sort, atlas placeholder fallback, Section 9.4's own Python binding
mechanism) received.

Verified by `cargo fmt --all -- --check`/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo build --workspace --all-targets`/
`cargo test --workspace` clean across the whole workspace (`tre-engine`
90 tests, up from 78). A new real GPU demo (`shape_registry_demo.rs`,
`demo/phase10_step10_1/`) proves the one real-rendering-supported case
(a `Rectangle` with uniform `CornerRadii`, `FillStyle::Solid`, no
border, no smoothing) produces byte-for-byte identical pixels whether
drawn directly via today's `draw_rounded_rect` or via `ShapeRegistry::
insert` + `flatten_into`, confirmed stable across 3 real runs. A full
manual regression sweep of all 32 Vulkan demos (31 pre-existing plus
this step's own new one): 31 passed; `canvas_accessibility_verify`
failed only on the same pre-existing, already-documented environmental
limitation as finding #126, unrelated to this step -- this step touches
no existing public API signature, so zero regressions were expected and
confirmed.

## Phase 10 Step 10.2 Implementation (2026-09-09)

### 162. [Fixed] A bit-cast style-buffer word index produced a subnormal `f32` real GPU hardware silently flushed to zero, reading the wrong style record entirely
Step 10.2 needed a way to reference a per-shape `GpuRectStyle`/
`GpuEllipseStyle` record (in the new bindless binding-1 storage buffer)
from a vertex, using one of `UiVertex.params`'s three existing float
slots (no new vertex attribute, to avoid growing the 32-byte layout).
The first implementation encoded the record's word index by bit-casting
it into the float slot (`f32::from_bits(word_index)`, reversed in the
shader with `floatBitsToUint`) -- a reasonable-looking approach, but
wrong in practice: a small integer like `64` bit-cast to `f32` has an
all-zero exponent field, making it a *subnormal* (denormal) number, and
real GPU hardware (confirmed by actually running the new
`shape_full_rendering_demo` against it, not assumed from documentation)
silently flushed that value to exactly `0.0` somewhere between the
vertex and fragment stage -- a well-documented, common ALU/interpolation
optimization ("flush-to-zero") most GPUs apply to subnormals by default.
The fragment shader therefore always read `style_index == 0`, silently
substituting whichever shape's style happened to occupy word 0 of the
buffer for every OTHER shape's own real style data.

**How it was found:** not by code review -- by running the real demo
and observing wrong pixels. The circle in `shape_full_rendering_demo`
rendered with a visible radius of ~10px against a requested 50px; a
horizontal pixel scanline across it (added temporarily, removed once the
bug was understood) showed a clean, correctly-anti-aliased disc at that
wrong radius, not noise -- which pointed at "reading a real but wrong
style record" rather than "the SDF math is broken." Working the numbers
backward (`radius - border_thickness = visible_radius`, i.e. `50 - 40 =
10`) matched exactly what would happen if the circle's fragment shader
were reading the *rectangle's* own `GpuRectStyle` record (written first,
at word 0) as if it were a `GpuEllipseStyle` -- `border_thickness`
landing on one of the rectangle's own `corner_radii` fields (40.0, a
real bit pattern that round-trips through `uintBitsToFloat` without
being subnormal) confirmed `style_index` was reading as `0`.

**Fix:** carry the word index numerically (`(byte_offset / 4) as f32` on
the Rust side, `uint(frag_params.x)` in both shaders), not by bit-cast.
Every real word index this codebase produces is a small integer, exactly
representable as a *normal* `f32` (`f32` represents every integer up to
`2^24` exactly) -- no denormal is ever in play, so there is nothing for
flush-to-zero to corrupt. `crates/tre-engine/src/gpu_style.rs`'s own
`style_index_param` doc comment has the full account for future readers.

Verified by re-running `shape_full_rendering_demo` (all 8 pixel-sample
assertions pass, confirmed stable across 3 repeated runs) and the full
`tre-engine`/`tre-math` unit test suites (including a dedicated
`style_index_param_numerically_encodes_the_word_index_not_the_byte_offset`
regression test).

### 163. [Fixed same-day, 2026-09-09] `Path` fill rendering remains unimplemented -- the real triangulator this engine would reuse (`tre_svg::triangulate`) is unreachable from `tre-engine` without a circular crate dependency
`tre-svg` already depends on `tre-engine` (for `tre_engine::UiVertex`,
reused by `tre_svg::to_ui_vertices`) -- so `tre-engine` cannot depend
back on `tre-svg` to reuse its existing, real, tested ear-clipping
triangulator (`tre_svg::triangulate`, Phase 3 Step 3.3.1) for `Path`
fill without creating a dependency cycle. This is a real architectural
constraint, confirmed by reading both crates' actual `Cargo.toml`
dependency declarations, not assumed. `Polygon` fill sidesteps this
entirely (a regular/star polygon's own procedural generation is
star-shaped with respect to its center by construction, so a simple
from-center triangle fan this crate wrote itself -- `fan_from_center` --
suffices; ear-clipping was never needed for it), but an arbitrary
Bezier-flattened `Path` boundary has no such guarantee and genuinely
needs a general triangulator.

**Not fixed this step:** duplicating a full ear-clipping triangulator a
second time (as this step already did, deliberately, for the much
smaller curve-flattening math -- finding-worthy in its own right, see
IMPLEMENTATION.md Step 10.2's own "Explicitly out of scope" list) was
judged too large and too risky to redo correctly for a substantially
more complex algorithm within this step's own scope. The real, clean fix
-- extracting `tre_svg`'s curve-flattening and triangulation primitives
into a new, lower-level shared crate both `tre-engine` and `tre-svg` can
depend on -- is real, separate, not-yet-scheduled future work.
`shapes::flatten_path`'s own Bezier-flattening output is real and tested
today regardless (used by `ShapeRegistry::hit_test`'s `Path` case, which
needs no triangulation at all); only the *rendering* path is blocked.

**Closed the same day (2026-09-09), by a different fix than the one
proposed above.** The user directed this fixed for real rather than
left disclosed, and, mid-implementation, pointed at
[`lyon`](https://github.com/nical/lyon) as an alternative to the
shared-bridge-crate plan this finding named -- then explicitly chose
"full replacement": migrate `tre-svg`'s own tessellation pipeline to
`lyon` too, retiring the hand-rolled ear-clipper (`tre_svg::triangulate`)
and the stencil-and-cover technique (Step 3.3.3) entirely. The real fix
this circular dependency needed was not a shared internal crate at all:
once `tre-engine` depends directly on the same external `lyon` crate
`tre-svg` now also uses, neither crate needs to reach into the other's
tessellation code, so the cycle this finding described simply doesn't
need routing around. See finding #165 and IMPLEMENTATION.md's "Step
10.2 Follow-up" write-up for the full account of everything this
decision retired.

### 164. [Fixed, 2026-09-09] `walking_skeleton.frag` (now `PipelineKind::FlatColor`, `Polygon`/`Path` fill's real shader) does not premultiply its own output by alpha, unlike every other pipeline in this engine
Every other real fragment shader in this codebase (`sdf_rounded_rect.
frag`, `sdf_rect_styled.frag`, `sdf_ellipse.frag`, `msdf.frag`) outputs
premultiplied color (`vec4(linear_color * alpha, frag_color.a * alpha)`)
to match ARCHITECTURE.md Section 6.1's documented premultiplied-alpha
blend state. `walking_skeleton.frag` -- a Phase 0 placeholder shader,
unmodified since Phase 0, and (until this step) never given a real
`Canvas` caller -- outputs `vec4(srgb_to_linear(frag_color.rgb),
frag_color.a)`: RGB is NOT multiplied by alpha. For fully-opaque colors
(`alpha == 1.0`) the two are bit-identical, so this was invisible in
every prior Phase-0-era demo; Step 10.2's own new `Polygon`/`Path` fill
caller (`RenderingCanvas::draw_flat_polygon`) only ever draws fully
opaque test geometry, so this step's own demo/tests do not exercise the
gap either. A translucent (`alpha < 1.0`) flat-filled polygon would
render visibly too bright/washed out against this blend state.

**Not fixed this step:** `walking_skeleton.frag` is long-lived, shared,
Phase-0-era shader code this step did not otherwise need to touch;
patching its blend math carries real, if probably small, risk of
changing behavior for whatever (if anything) else might come to depend
on its exact current output, and is out of this step's own stated scope
(shape rendering, not a Phase-0-shader audit). Disclosed here so a
future translucent-fill caller does not discover it the hard way.

**Fixed the same day (2026-09-09), immediately after the lyon migration
follow-up gave `Polygon`/`Path` a real stroke -- a second, independent
reason a translucent flat fill could now reach this shader for real.**
`main()` now premultiplies: `vec4(linear_color * frag_color.a,
frag_color.a)`, the same pattern every other real shader in this
codebase already used. **Verified with a real, dedicated GPU demo**
(`translucent_flat_fill_demo.rs`, `demo/phase10_step10_2_finding_164/`),
not just re-running the existing (all fully-opaque) demos: draws a
genuinely translucent flat fill and compares the real GPU readback
against an independent Rust reference of the correct premultiplied
blend AND against what the old, unfixed math would have produced --
confirmed to match the correct reference almost exactly (`[169, 77,
139]` expected vs. measured) and to be measurably different from the
broken one (`[233, 100, 187]`). The demo's own regression-catching power
was itself verified, not assumed: temporarily reverting the shader to
its old, buggy form during development made this exact demo fail with
`got 233, expected 169`, proving it would genuinely have caught the
original bug. Every other real consumer of this shader
(`walking_skeleton`, `svg_morph_demo`, `svg_tessellation_demo`,
`text_shaping_demo`, `self_intersecting_fill_demo`,
`path_and_polygon_demo`, `atlas_packing_demo`, `headless`) re-run and
confirmed bit-for-bit unchanged, since all of them draw only
fully-opaque geometry. `cargo fmt`/`clippy -D warnings`/`build`/`test`
clean across the whole workspace.

## Phase 10 Step 10.2 Follow-up: `lyon` Migration (2026-09-09)

### 165. [Decision] Full replacement of this project's hand-rolled tessellation (ear-clipping triangulator, stencil-and-cover fallback) with `lyon`, a real architecture pivot -- what it retired and why
Closing finding #163 (`Path` fill blocked by a real, one-directional
circular-dependency constraint between `tre-svg` and `tre-engine`) was
originally planned as a new, lower-level shared bridge crate exposing
`tre-svg`'s existing curve-flattening/triangulation primitives to both
sides. Mid-implementation (only an empty `Cargo.toml` stub had been
created, no source files -- confirmed and cleaned up before proceeding),
the user pointed at [`lyon`](https://github.com/nical/lyon), the current
Rust ecosystem's de facto standard 2D GPU tessellation library, as an
alternative. Given new information mid-task, this was treated as a
genuine architecture decision needing the user's own judgment call
rather than a default continuation of the original plan: presented via
`AskUserQuestion` as three real options ("new work only" -- fix `Path`
fill with `lyon`, leave `tre-svg`'s own existing, shipped, tested
tessellation code untouched; "full replacement" -- migrate `tre-svg` to
`lyon` too, retiring its hand-rolled work entirely; "keep hand-rolled,
just relocate" -- the original bridge-crate plan). **The user chose full
replacement.**

**What this retired, named explicitly, not silently dropped:**
- `tre-svg::triangulate` (Phase 3 Step 3.3.1's ear-clipping
  triangulator, `crates/tre-svg/src/triangulate.rs`, ~463 lines) --
  including the documented record of three separately hard-won
  correctness bugs found only via real GPU demo pixel readbacks, never
  its own unit tests. Deleted outright, not deprecated.
- The stencil-and-cover GPU fallback (Phase 3 Step 3.3.3):
  `VulkanDevice::create_stencil_and_cover_pipelines`
  (`crates/tre-rhi-vulkan/src/lib.rs`, ~204 lines) and `tre-svg`'s own
  `stencil.rs` (`fan_triangles`/`bounding_box`). Deleted outright.
- `tre_engine::FillRule` (`crates/tre-engine/src/lib.rs`) -- existed
  solely to parameterize the now-deleted stencil-and-cover pipelines.
  Deleted (confirmed `tre-text`'s own unrelated `FillRule` usage is a
  distinct third-party `fdsm::bezier::scanline::FillRule`, not this
  one, before removing it).
- `SvgError::NotSimplePolygon` (the old ear-clipper's own rejection for
  every case it structurally couldn't handle -- self-intersection, true
  holes, multiple contours). Replaced by `SvgError::TessellationFailed`,
  now reached only on `lyon`'s own genuine tessellation failure, not as
  the routine "this shape is too complex for this algorithm" case it
  used to be -- because `lyon`'s real sweep-line fill tessellator
  handles every one of those cases directly.

**Why this counts as a real architecture pivot, not scope creep.** 2D
path tessellation -- fill with self-intersection/hole/winding-rule
resolution, plus stroke joins/caps/miter limits (a capability this
workspace never had at all until this follow-up) -- is a deep,
well-solved problem domain with a mature, actively-maintained,
ecosystem-standard answer, matching the same category this project
already treats `usvg` (SVG DOM/parsing) and `wide` (SIMD) as being in,
rather than a core-identity primitive this project deliberately hand-
rolls (sort, atlas, arena). `lyon`'s real behavior was verified directly
against its published documentation and source (via `docs.rs` and
`gh api`/raw GitHub fetches, not assumed from training data) before any
integration code was written, including two non-obvious, easy-to-get-
wrong facts: `Flattened`'s iterator contract exactly matches the old
hand-rolled flattening functions' own contract (starts after the current
point, ends exactly at the segment's endpoint), and `StrokeVertex::
position()` -- not `position_on_path()`, the pre-offset centerline point
-- is the correct final tessellated stroke position to consume.

**A real, self-found correctness bug beyond what was asked, disclosed
here.** While rewriting `tre-engine`'s own `flatten_path`, the original
function discarded per-subpath closedness (whether a subpath ended via
`PathCommand::Close`) entirely. An explicitly-closed subpath needs a
continuous stroke loop with no end caps; an open subpath needs real caps
at both ends (per `stroke_line_cap`). Fixed by splitting `flatten_path`
(public, unchanged signature) into a thin wrapper over a new private
`flatten_path_with_closed`, which now threads closedness through to
`tessellate_stroke`.

**Verified.** Every real consumer of the retired API was found and
updated, including one (`text_shaping_demo.rs`) missed in the initial
sweep and caught only via a full `cargo build -p tre-rhi-vulkan
--all-targets`. `self_intersecting_fill_demo.rs` (renamed from
`stencil_and_cover_demo.rs`) re-proves the exact same textbook pentagram
fill-rule disagreement this project used to prove the old technique with,
now via `tessellate_fill` directly -- confirmed via a real run and visual
inspection of the output PNG. A new demo, `path_and_polygon_demo.rs`
(`demo/phase10_step10_2_followup/`), proves the original motivating goal
end to end: a "donut" `Path` with a real subtractive hole, and a bordered
`Polygon`, both rendered through `ShapeRegistry`, with real pixel
assertions and visual confirmation. `cargo fmt`/`clippy -D warnings`/
`build`/`test` clean across the whole workspace (126 `tre-engine` tests,
up from 119).

## Summary table (Phase 10 Step 10.2 Follow-up)

| # | Severity | Status | One-line summary |
|---|----------|--------|-------------------|
| 165 | Decision | Resolved | Adopted `lyon` as the one tessellation backend for `tre-svg` and `tre-engine`, retiring the hand-rolled ear-clipper and stencil-and-cover technique entirely (user-directed "full replacement"), closing finding #163 by a different fix than originally proposed |
| 164 | Should-fix | Fixed | `walking_skeleton.frag` (`PipelineKind::FlatColor`) now premultiplies its own output by alpha; verified with a new, dedicated translucent-fill GPU demo that would have caught the original bug |

## Phase 10 Step 10.2 Completion Roadmap (2026-09-09)

### 166. [Decision] Full Shape Rendering Support's six remaining disclosed gaps are now planned as sequenced Steps 10.2.1-10.2.6, including a real correction to this project's own prior blend-mode assumption
The user asked what remained open in "Full Shape Rendering Support" after
finding #163 (`Path` fill) and #164 (premultiplied alpha) both closed the
same day. The honest answer, cross-checked against the real, current code
(not just prior documentation) rather than assumed from memory: six real
gaps remain --

1. `FillStyle::Gradient`/`Texture` -- real enum variants since Step 10.1,
   still `unimplemented!()` at all four `flatten_*` call sites.
2. Non-`Normal` `BlendMode` -- a real, already-threaded field, read by
   nothing.
3. `sd_ellipse`'s disclosed scaled-circle approximation and
   `corner_smoothing`'s unverified squircle match.
4. No rounded stroke caps on a partial-arc `Circle`/`Ellipse`.
5. The shape system's zero-allocation claim is architecturally sound but
   not proven live the way `main_loop_demo`'s own claim is.

The user directed these be finished, in full, as sequenced Steps
10.2.1-10.2.6 (`PLAN.md`, full investigation/scope/task breakdown per
sub-step). **One real, useful correction surfaced while researching the
blend-mode sub-step before writing the plan, not during implementation:**
Step 10.2's own original disclosure claimed non-`Normal` blend modes
"need new RHI framebuffer-read capability." Direct research into
`VK_EXT_blend_operation_advanced` (not relied on from memory) shows this
extension maps `Multiply`/`Screen`/`Overlay`/`SoftLight`/`ColorDodge`
directly onto hardware `VkBlendOp` values with no framebuffer read at
all, if the real device supports it (a capability query, not an
assumption) -- a materially better, simpler technical path than this
project's own prior finding assumed, caught only by verifying the real
extension's real behavior before committing the plan to an approach.
Documented in `PLAN.md`'s own Step 10.2.3 section and ARCHITECTURE.md
Section 7.5.

**Not yet built as of this entry** -- this is a planning decision, not an
implementation report. Each sub-step gets its own REVIEW.md finding(s) as
real issues surface during its own implementation, and the TRE Build
Tracker artifact's six new rows flip from PLANNED to DONE only as each
sub-step's own real, tested, demoed work lands.

## Summary table (Phase 10 Step 10.2 Completion Roadmap)

| # | Severity | Status | One-line summary |
|---|----------|--------|-------------------|
| 166 | Decision | Planned | Sequenced Steps 10.2.1-10.2.6 close every remaining disclosed Full Shape Rendering Support gap; research surfaced a real, better blend-mode technical path (`VK_EXT_blend_operation_advanced`) than originally assumed |

## Phase 10 Step 10.2.1 Implementation (2026-09-09)

### 167. [Fixed same-day] A `GradientDef`'s points were authored in `Rectangle`/`Circle`'s public top-left-relative local space, but `frag_uv` is center-relative -- the demo's own first real run caught the mismatch directly
`sdf_rect_styled.frag`/`sdf_ellipse.frag`'s own `frag_uv` is CENTER-
relative (`draw_styled_rectangle`/`draw_ellipse`'s own `uv` construction,
an internal shader convention chosen for symmetric SDF math). But
`GradientDef`'s own points -- `GradientKind::Linear { start, end }` /
`Radial { center, radius }` -- were originally written straight into the
`GpuGradientStyle` record with no conversion, while every other
`Rectangle`/`Circle` field (`corner_radius`, `border_thickness`, and
`flatten_circle`'s own explicit doc comment) is authored in the shape's
PUBLIC local space: bounding-box top-left at the origin.

Caught by `gradient_fill_demo.rs`'s own first real run, not by
inspection: a rectangle gradient defined `[0,0] -> [200,0]` (its own
public top-left-relative space) with a probe at local `(30, 50)`
(expected `t = 0.15`, a blended red-purple tone) instead rendered pure,
unmixed red -- the shader was evaluating `t` against `frag_uv`'s own
center-relative origin, not the origin the gradient was actually
authored against, so every real point landed far outside `[0, 1]` and
clamped to one end.

**Fixed same day:** `build_gpu_gradient_style`/`write_gradient_style`
gained a `local_origin_offset: Vec2` parameter -- `[half_width,
half_height]` for `Rectangle`, `radius` for `Circle` (exactly the
center `flatten_circle`'s own doc comment already names), `[0, 0]` for
`Polygon`/`Path` (whose own local space is already center-relative by
construction, or -- for `Path` -- has no fixed convention at all, so a
gradient on one is defined in those same raw coordinates directly) --
subtracted from every point/center before the record is written. Gradient
authors keep thinking in the same top-left-relative coordinates as every
other shape property; the center-relative conversion is now an internal
implementation detail, never surfaced.

**Verified.** `gradient_fill_demo.rs` re-run after the fix: all three
shapes' gradients (rectangle linear, circle radial, hexagon linear via
the separate `GradientFill` pipeline) match an independent Rust
reference of the exact gradient math within a small disclosed tolerance.
A new unit test (`flatten_into_renders_a_rectangles_gradient_fill_via_
the_styled_path`) asserts the real, offset-corrected `GpuGradientStyle`
point values directly, so a regression here would be caught without
needing a GPU run.

## Summary table (Phase 10 Step 10.2.1 Implementation)

| # | Severity | Status | One-line summary |
|---|----------|--------|-------------------|
| 167 | Should-fix | Fixed | Gradient points authored in public top-left-relative local space now correctly offset into `frag_uv`'s own center-relative space for `Rectangle`/`Circle`; caught by the new demo's own first real run |

## Phase 10 Step 10.2.2 Implementation (2026-09-09)

### 168. [Decision] Polygon/Path texture fill reuses the existing TexturedQuad pipeline directly, a better path than PLAN.md's own original suggestion
`PLAN.md`'s own Step 10.2.2 scope decision proposed extending
`PipelineKind::GradientFill`'s shader with a texture branch for
`Polygon`/`Path`, avoiding "a fourth pipeline" and the combinatorial
pipeline growth that would otherwise result. Implementation found a
better option once the real shape of the problem was in front of it:
`PipelineKind::TexturedQuad`/`bindless_textured.frag` -- a real, already
-built, already-registered-in-other-demos pipeline -- already does
exactly what Polygon/Path texture fill needs (sample a bindless texture
at a real per-vertex UV, with the exact `0xFFFFFFFF`-sentinel-for-
"no texture" fallback convention this workspace already established).
Gradient evaluation was genuinely new math needing a new shader
(`gradient_fill.frag`); texture sampling is not -- reusing the existing
pipeline needed zero new GLSL, zero new pipeline objects, and zero new
descriptor bindings, only a new `RenderingCanvas::draw_textured_polygon`
method carrying real UVs instead of `draw_flat_polygon`'s zeroed ones.
Disclosed here as a real, deliberate departure from the written plan,
not a silent scope change -- `PLAN.md`'s own text is left as the
historical record of the original, reasonable-at-the-time proposal.

### 169. [Decision] draw_styled_rectangle/draw_ellipse's fill-selection parameters bundled into one StyleFill value before they grew a third time
Step 10.2.1 added two trailing `u32` parameters (`fill_kind`,
`gradient_word_index`) to `draw_styled_rectangle`/`draw_ellipse`. Step
10.2.2 was about to add a third (`texture_index`) to the same two
methods -- a real, growing "one more `u32` every time a fill kind
ships" pattern that would only get worse at Step 10.2.3 (blend modes)
and beyond. Consolidated all three into one new `StyleFill` struct
(`fill_kind`/`gradient_word_index`/`texture_index`, plus a `StyleFill::
SOLID` constant for the common case) instead of a fourth trailing
parameter, updating both real callers (`flatten_rectangle`/`flatten_
circle`) and all 5 existing test call sites. A real, disclosed
mid-course correction to `PLAN.md`'s own original phrasing ("`GpuRectStyle`/
`GpuEllipseStyle` gain one more trailing `u32`"), made for the Rust API
surface specifically -- the GPU-side word layout itself is unaffected,
still three separate `u32` words in the style buffer.

**Verified.** All 5 pre-existing `draw_styled_rectangle`/`draw_ellipse`
tests updated and passing; no behavioral change, a pure API-surface
consolidation.

## Summary table (Phase 10 Step 10.2.2 Implementation)

| # | Severity | Status | One-line summary |
|---|----------|--------|-------------------|
| 168 | Decision | Resolved | Polygon/Path texture fill reuses the existing TexturedQuad/bindless_textured.frag pipeline directly instead of extending GradientFill, avoiding all new shader/pipeline/descriptor work for this shape-kind pair |
| 169 | Decision | Resolved | Bundled fill_kind/gradient_word_index/texture_index into one new StyleFill value for draw_styled_rectangle/draw_ellipse, avoiding a third trailing u32 parameter and future growth |

## Phase 10 Step 10.2.3 Implementation (2026-09-09)

### 170. [Decision] `VK_EXT_blend_operation_advanced`, this project's own written primary path for non-`Normal` `BlendMode` rendering, is not implemented by RADV -- pivoted to the real `VK_KHR_dynamic_rendering_local_read` alternative, at the user's explicit direction
Finding #166 (Phase 10 Step 10.2 Completion Roadmap) had already recorded
a "correction" to this project's own original blend-mode assumption,
concluding `VK_EXT_blend_operation_advanced` needed no framebuffer read
at all -- a real device-capability query would confirm it, not assumed.
That query, run for real at the start of this step, returned `false`:
direct `vulkaninfo` inspection of this project's own real dev GPU (AMD
Radeon 890M, Mesa 26.2.2-arch3.2, RADV driver) shows the extension
simply absent from the device's advertised extension list.
Independently corroborated via Mesa's own release notes (not a one-off
local misconfiguration) before treating it as settled. `PLAN.md`'s
entire primary path for this step was therefore invalidated before any
implementation code was written -- a direct conflict with this
project's standing "real code, real GPU demos as the correctness
oracle" discipline, since that path could never be exercised by a real
GPU demo on this project's own hardware.

Surfaced to the user as a genuine three-way fork via `AskUserQuestion`
(build the real `VK_KHR_dynamic_rendering_local_read` framebuffer-read
path; ship only the capability-gated `Normal`-blending fallback; build
both). The user's first response was a genuine clarifying question, not
a selection ("is there an external package that would save time?") --
researched honestly rather than guessed at: no relevant blend-mode
shader crate exists in the Rust ecosystem, and `vk-sync-fork` (a real
candidate for Vulkan barrier ergonomics) predates the new
`VK_IMAGE_LAYOUT_RENDERING_LOCAL_READ_KHR` layout and would be
inconsistent with this codebase's own 100%-hand-rolled-via-`ash`
barrier convention. Re-presented the same three options after reporting
that research; the user's final, explicit direction: "use the
alternative, it sounds like the designed way to do it,
VK_KHR_dynamic_rendering_local_read" -- build ONLY the real
framebuffer-read path, no dual-path, no fallback-only shortcut.

`VK_KHR_dynamic_rendering_local_read` -- confirmed present on this same
real GPU via `vulkaninfo` -- is real, portable, and fully implemented in
this step (see IMPLEMENTATION.md's own Step 10.2.3 write-up for the
complete technical account: the new descriptor set/pipeline layout,
`RENDERING_LOCAL_READ_KHR` layout, by-region barrier, and
`flat_color_blend.frag`'s own W3C blend-formula math). `PLAN.md`'s own
text, and finding #166's now-superseded conclusion, are left as the
historical record of the original, reasonable-at-the-time plan.

### 171. [Fixed same-day] A real windowed swapchain's presentable surface is not spec-guaranteed to support `INPUT_ATTACHMENT` usage the way a manually allocated headless image's always safely can
`HeadlessSwapchain`'s color image is allocated directly via
`vkCreateImage` (a `VulkanDevice`-owned, general-purpose image), so
declaring `VK_IMAGE_USAGE_INPUT_ATTACHMENT_BIT` on it is always valid --
a core Vulkan 1.0 flag with no capability query needed. `VulkanSwapchain`'s
images instead come from `vkCreateSwapchainKHR` against a real
presentable surface, whose `imageUsage` must be a subset of that
surface's own `VkSurfaceCapabilitiesKHR::supportedUsageFlags` --
`INPUT_ATTACHMENT` support there is real, driver/platform-defined
behavior, not a spec guarantee, unlike `HeadlessSwapchain`'s case.

Caught by this step's own real GPU demo-regression sweep, not reasoned
out in advance: `local_read_supported` (the device-level capability
flag `begin_frame` originally gated `RENDERING_LOCAL_READ_KHR` on) is
`true` on this project's own real dev GPU/driver regardless of which
swapchain is in use -- meaning every windowed demo (`walking_skeleton`,
`multi_window`, `input_demo`, `main_loop_demo`) would have silently
started requesting a layout the windowed swapchain's own images were
never created to support, a real correctness risk (validation error, or
driver-defined behavior) that had nothing to do with those demos'
own actual behavior. Fixed by querying `capabilities.
supported_usage_flags` for real in `VulkanSwapchain::new` (the same
"query, don't assume" discipline `local_read_supported` itself already
established) and exposing the result via a new `RhiSwapchain::
supports_local_read_input_attachment` trait method; `begin_frame`/
`submit_and_present` now require BOTH the device-level AND this
swapchain-level query before choosing `RENDERING_LOCAL_READ_KHR`,
failing closed to ordinary `COLOR_ATTACHMENT_OPTIMAL` (blend modes then
unavailable on that specific window, exactly as if the device itself
lacked the capability) rather than risking a validation error.

**Verified.** All four windowed demos re-run directly against this
machine's real X11 session (`xvfb-run` unavailable locally) and confirmed
passing, both before this fix was needed to be checked for (they never
actually failed, since `VulkanSwapchain::new`'s query happens to return
`true` on this project's own real surface) and after, proving the new
code path is at minimum inert on real hardware that does support the
flag.

### 172. [Fixed same-day] `resume_swapchain_rendering` hardcoded `COLOR_ATTACHMENT_OPTIMAL`, but `begin_frame` had already moved the swapchain image to `RENDERING_LOCAL_READ_KHR`
A second real regression the same demo-regression sweep caught, distinct
from #171: every demo exercising a `PushLayer`/`PopLayer` redirect
(`render_to_texture_demo`, `canvas_layer_composite_demo`,
`layer_oversize_regression_demo`, `layer_blur_demo`,
`canvas_combined_scene_demo`, `dual_kawase_nonbindless_experiment`,
`dual_kawase_blur_demo`) crashed outright with a real Vulkan validation
error and a core dump:

```
vkCmdBeginRenderingKHR(): pRenderingInfo->pColorAttachments[0] ... is
expected to have layout VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL but
previous known layout is VK_IMAGE_LAYOUT_RENDERING_LOCAL_READ.
```

`VulkanCommandBuffer::resume_swapchain_rendering` (re-begins rendering
into the swapchain after one or more `begin_render_to_texture`/
`end_render_to_texture` pairs redirected rendering to a layer's own
texture) hardcoded `vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL` in its own
`RenderingAttachmentInfo`, written before this step under the
correct-at-the-time assumption that `begin_frame` always left the
swapchain image in exactly that layout. This step's own `begin_frame`
change (using `RENDERING_LOCAL_READ_KHR` instead, when supported) broke
that assumption, and nothing in between (`begin_render_to_texture`/
`end_render_to_texture` only ever touch a *texture's* own image, never
the swapchain's) transitions the swapchain image back. Fixed by stashing
the real layout `begin_frame` actually chose on a new `VulkanCommandBuffer`
field (`swapchain_color_layout`) and having `resume_swapchain_rendering`
declare that same layout instead of a hardcoded constant.

**Verified.** All seven previously-crashing demos re-run and confirmed
passing after the fix, with no change to any other demo's output.

## Summary table (Phase 10 Step 10.2.3 Implementation)

| # | Severity | Status | One-line summary |
|---|----------|--------|-------------------|
| 170 | Decision | Resolved | VK_EXT_blend_operation_advanced is not implemented by RADV (this project's own real dev GPU); pivoted to the real VK_KHR_dynamic_rendering_local_read framebuffer-read path at the user's explicit direction |
| 171 | Fixed | Resolved | A windowed swapchain's surface isn't spec-guaranteed to support INPUT_ATTACHMENT usage the way a manually allocated headless image's always safely can -- added a real per-swapchain capability query (RhiSwapchain::supports_local_read_input_attachment) alongside the device-level one |
| 172 | Fixed | Resolved | resume_swapchain_rendering hardcoded COLOR_ATTACHMENT_OPTIMAL after begin_frame had moved the swapchain image to RENDERING_LOCAL_READ_KHR, crashing every PushLayer/PopLayer demo with a real validation error -- fixed by stashing and reusing the real layout |

## Phase 10 Step 10.2.4 Implementation (2026-09-09)

### 173. [Fixed, real finding along the way] The ellipse SDF's "scaled circle" approximation was replaced with Inigo Quilez's real, verified-exact Newton-Raphson formula -- and its actual real-world defect turned out to be border-thickness magnitude, not the fill boundary PLAN.md's own task language expected
`sdf_ellipse.frag`'s `sd_ellipse` shipped in Step 10.2 as a standard
"scaled circle" approximation (`k1*(k1-1)/k2`), disclosed as exact only
when `radius.x == radius.y`. Real research (fetching Inigo Quilez's own
published article, iquilezles.org/articles/ellipsedist, directly rather
than trusting memory) confirmed the exact point-to-ellipse distance has
no simpler closed form than a Newton-Raphson refinement on the
ellipse's implicit parametrization (a direct quartic solve is, per IQ's
own article, "both expensive and not very stable") -- implemented the
real, rotation-based variant (5 iterations, IQ's own published default),
replacing the old approximation entirely.

A real, independent-reference investigation (`tre-engine`'s new
`sdf_ellipse_fidelity` test module: the old formula, the new formula,
and a fully independent brute-force ground truth via dense parametric-
boundary sampling, sharing no math with either) found something
`PLAN.md`'s own task language did not anticipate: the old approximation's
fill/no-fill BOUNDARY was always exactly correct everywhere, not just
on-axis -- `k1 = length(p/r)` is exactly `1.0` on the true ellipse
boundary by construction (regardless of angle), so the approximation's
own zero-crossing always coincided with the real one. Its actual,
measured, practical defect was in the SDF's MAGNITUDE away from the
boundary (10.67px and 1.31px real error at two representative off-axis
points on a 3.5:1-eccentricity ellipse) -- exactly the value
`border_thickness` rendering depends on (`inner_d = d + border_thickness`),
meaning a bordered, eccentric ellipse's border thickness would have
visibly varied around its own perimeter under the old formula even
though its outer silhouette was always drawn correctly. `PLAN.md`'s own
task 2 expected the old formula to show real error at reference points
"along the major/minor axis" -- direct derivation and this same test
module confirmed it is, in fact, exact there too (for an exterior
point); a genuine ellipse-geometry subtlety was found and disclosed
along the way instead: an INTERIOR major-axis point can have its true
nearest boundary point be a pair of symmetric, off-axis points rather
than the vertex, whenever it falls inside the ellipse's own evolute
cusp (at `x = (r.x^2 - r.y^2) / r.x` from center) -- a real property of
ellipse geometry itself, unrelated to either formula tested.

**Verified.** 3 new tests (analytic axis-point exactness for an exterior
point; the new formula's real, sub-0.05px agreement with the
independent brute-force reference off-axis; the old formula's real,
measured error against that same reference, over 1px at both tested
off-axis points). A new real GPU demo, `ellipse_sdf_fidelity_demo.rs`
(`demo/phase10_step10_2_4/`) -- the first ever to render a genuinely
non-circular ellipse -- confirms the real border/fill transition on
actual GPU-rendered pixels matches a second, independent CPU
transcription of the exact formula at four angles. Every pre-existing
GPU demo re-run and confirmed passing, including the three real
consumers of the ellipse pipeline at their own circular cases (where
old and new are both exact, so no regression is possible or found).

### 174. [Decision] corner_smoothing's disclosed "not verified against any reference" gap resolved by real research into Figma's actual squircle construction -- quantified, not fixed, since no simple closed form of it exists
`sdf_rect_styled.frag`'s `corner_norm` (a superellipse-exponent blend,
`smoothing` raising the corner falloff's norm from 2 toward 5) shipped
in Step 10.2 disclosed as "not a byte-for-byte match of any specific
reference implementation's own squircle algorithm (e.g. Figma's)."
`PLAN.md` Step 10.2.4's own "Scope decisions" authorized researching a
real reference and either matching it (if a simple closed form exists)
or formally verifying and documenting the real deviation instead.

Real research (Figma's own blog post, figma.com/blog/desperately-
seeking-squircles, plus the real, independently-verified, widely-cited
open-source transcription of their actual algorithm at
github.com/tienphaw/figma-squircle -- fetched and read directly, not
assumed) found that Figma's construction is a real SVG path per corner
(two curvature-continuous cubic Bezier ramps plus a circular arc), not
an implicit distance field at all. This settles the scope call the plan
itself left open: there is no simple closed form of Figma's own
construction to drop into an SDF shader -- computing an exact per-pixel
distance to an arbitrary Bezier curve is a substantially harder,
more expensive real-time problem than this one bounded step's scope, so
no rewrite was attempted. `corner_norm` is kept exactly as it shipped
(still a real, legitimate, monotonic smoothing control), with its real
deviation from Figma's own construction now measured and disclosed
precisely instead of left as an open question: a new `tre-engine` test
module (`corner_smoothing_fidelity`, an independent Rust transcription
of Figma's real corner-path-parameter algorithm, a real SVG-arc
endpoint-to-center solver, and a cubic Bezier evaluator) found the two
constructions produce the identical corner at `smoothing == 0` (both
reduce to the same plain circular-arc rounded corner) but diverge up to
~71% of the corner radius at `smoothing == 1.0` (confirmed to scale
linearly with radius by testing at two radii) -- a real, visually
significant difference, not a rounding-scale one. Anyone wanting a
byte-for-byte Figma match at high `corner_smoothing` values should treat
this engine's own control as a distinct, engine-native smoothing curve.

**Verified.** 2 new tests (exact match at `smoothing == 0.0`; the real,
quantified divergence bounded within a measured range at `smoothing ==
0.5`/`1.0`, re-checked against the real reference implementation rather
than an assumed number).

## Summary table (Phase 10 Step 10.2.4 Implementation)

| # | Severity | Status | One-line summary |
|---|----------|--------|-------------------|
| 173 | Fixed | Resolved | Replaced the ellipse SDF's "scaled circle" approximation with Inigo Quilez's real, verified-exact Newton-Raphson formula; the real, measured defect turned out to be border-thickness magnitude away from the boundary, not the fill boundary itself (which was always exact) |
| 174 | Decision | Resolved | Researched Figma's real squircle construction (a Bezier-path per corner, not an implicit distance field) -- no simple closed form exists to swap into corner_norm, so its real deviation from Figma's construction was measured and disclosed instead (up to ~71% of corner radius at max smoothing) |

## Phase 10 Step 10.2.5 Implementation (2026-09-09)

### 175. [Fixed] A partial-arc Circle/Ellipse's bordered cut edges were hard, flat cutoffs even with a real border_thickness set -- now real, analytic, bounded rounded stroke caps
`sdf_ellipse.frag`'s sector-cutoff branch (`if (relative > arc_sweep_
angle) { d = max(d, 0.001); }`) gave every partial-arc `Circle`/
`Ellipse` a hard, flat cut at both ends of its sweep, regardless of
`border_thickness` -- a progress-ring-style component's two visible
ends never rounded off, even though `stroke_line_cap` (a real field,
borrowed from `Path`'s own convention) implied they should. Fixed with
a new `cap_sdf` function: a real circle of radius `border_thickness /
2`, centered on the border band's own centerline at each of the arc's
two cut angles (the cap center's radial distance to the true boundary
uses the ellipse's own polar equation, `1/sqrt((cos(angle)/r.x)^2 +
(sin(angle)/r.y)^2)` -- a genuinely exact closed form, distinct from
`sd_ellipse`'s own Newton refinement, which only Step 10.2.4 needed for
nearest-point distance from an arbitrary point). The final distance is
`min(sector_clipped_d, cap_start_d, cap_end_d)` -- a plain SDF union
that provably only ever pulls the distance more "inside" near the two
cut points, by construction, leaving the plain swept interior and the
rest of the excluded wedge untouched.

One disclosed approximation carries over, narrowed from Step 10.2.4's
own broader one: the cap center sits along the RADIAL direction at each
cut angle (exact for a `Circle`; a real, consistent approximation for a
true, non-uniform-radius `Ellipse`, whose local outward normal
generally differs from radial there -- the same distinction finding
#173 surfaced while researching the exact ellipse SDF itself).

**Verified.** No CPU-side reference test applies here -- there is no
independent formula for "the correct antialiased 2D render of a rounded
cap" the way there was for `sd_ellipse`'s own distance value; the real
per-pixel shader behavior IS the thing under test. A new real GPU demo
(`arc_rounded_cap_demo.rs`) draws a bordered quarter-circle arc and
samples real pixels a few degrees past each of the two cut angles, at
the border band's own centerline radius: 4° past each cut (within the
cap's own bounded angular footprint at that radius) is confirmed
border-colored, proving the real rounding; 25° past each cut is
confirmed still-excluded background, proving the rounding is genuinely
BOUNDED, not simply "the cutoff stopped working." Every pre-existing
GPU demo re-run and confirmed passing, including `shape_full_rendering_
demo`'s own pre-existing 270°-sweep bordered circle -- its own
arc-wedge-exclusion assertion still passes unchanged, and the render
now visibly shows real rounded caps where it previously had flat ones.

## Summary table (Phase 10 Step 10.2.5 Implementation)

| # | Severity | Status | One-line summary |
|---|----------|--------|-------------------|
| 175 | Fixed | Resolved | Added real, analytic, bounded rounded stroke caps at a partial arc's two cut angles (a circle-SDF union of radius border_thickness/2 at each cut, via min()) -- proven on real GPU pixels a few degrees past each cut, both rounded (near) and still-excluded (far) |

## Phase 10 Step 10.2.6 Implementation (2026-09-09)

### 176. [Fixed] Two real, previously-undetected per-frame allocations found by this step's own new zero-allocation guard -- `generate_polygon_points`/`fan_from_center` and `bounding_box_uvs` each returned a freshly heap-allocated `Vec` on every call
No demo had ever wrapped a `ShapeRegistry::flatten_into`-driven scene
in `tre_memory::RenderTickGuard`/`DebugAllocGuard` (Step 9.2's own real,
self-checking zero-allocation enforcement) before this step's own new
`shape_registry_zero_alloc_demo.rs` -- ARCHITECTURE.md Section 7.5 had
disclosed this as "architecturally sound but not yet proven" since Step
9.2 itself. The very first real GPU render under the guard's own
mutating, mixed scene (`Rectangle`, gradient `Circle`, texture
`Polygon`, blend-mode `Polygon`, bordered arc `Circle`) panicked
immediately, with a real backtrace pointing at two distinct, genuine
per-call allocations neither `radix_sort_by_key`'s own Step 9.2 fix nor
any prior GPU demo had ever exercised under allocation pressure:

- `generate_polygon_points` (`crates/tre-engine/src/shapes.rs`), called
  by `flatten_polygon` for every `Polygon` shape, built its output via
  `.collect()` into a fresh `Vec<Vec2>` every call; `flatten_polygon`
  then built a SECOND fresh `Vec` to prepend the fill's own fan pivot,
  and `fan_from_center` returned a THIRD fresh `Vec<[u32; 3]>` for the
  triangle indices -- three real allocations per `Polygon` flatten, not
  one.
- `bounding_box_uvs` (Step 10.2.2's own texture-fill UV helper, shared
  by `Polygon`'s and `Path`'s `FillStyle::Texture` dispatch) did the
  same for its own output.

Both fixed the same way: new `_into` siblings (`generate_polygon_
points_into`, `fan_from_center_into`, `bounding_box_uvs_into`) that
clear and refill a caller-supplied `&mut Vec` instead of returning a
freshly allocated one, backed by three new persistent scratch fields on
`ShapeRegistry` itself (`polygon_points_scratch`/`polygon_triangles_
scratch`/`polygon_uv_scratch`) -- the same "grow once during warm-up,
reuse after" discipline Step 9.2 already established for `FrameArena`'s
own scratch buffers. `flatten_polygon`'s own prepend-the-center-pivot
step became a plain `Vec::insert(0, ..)` on the already-capacitized
scratch buffer (an O(n) shift, not a reallocation) instead of building
a second `Vec`. `generate_polygon_points` itself (the original, owned-
`Vec`-returning form) is kept, since `hit_test_polygon` still calls it
and that is not a per-frame guarded path; `fan_from_center` had no
other real caller left once `flatten_polygon` moved to the reuse-
friendly sibling, so it was removed outright (its own two tests
rewritten against `fan_from_center_into` instead of kept as coverage
for now-dead code).

**A real, deeper gap found and deliberately NOT fixed, disclosed
instead**: `lyon`-backed tessellation (`tessellate_fill`/`tessellate_
stroke`, used by any `Path`'s own fill/stroke and any BORDERED
`Polygon`) still constructs a fresh `lyon::path::Path`, a fresh
`FillTessellator`/`StrokeTessellator`, and a fresh `VertexBuffers` on
every single call -- a substantially larger reuse redesign (persistent,
reusable `lyon` tessellator/buffer state threaded through `ShapeRegistry`)
than this finding's own two fixes. This is the exact same category of
deferred gap `main_loop_demo.rs`'s own Step 9.2 already disclosed for
RHI submission (`Box<dyn RhiCommandBuffer>`, allocated fresh every
frame in `VulkanDevice::begin_frame`) and `std::thread::scope`
(`Arc<ScopeData>`) -- a real, unavoidable-this-pass cost, named
explicitly rather than silently worked around. `shape_registry_zero_
alloc_demo.rs`'s own scene deliberately uses only borderless shapes and
no `Path` specifically to avoid exercising this gap, disclosed in that
demo's own header comment, not hidden by omission.

**A small, real, new public API added to make the demo's own required
"mutates... gradient stops" scenario possible at all**: `ShapeRegistry::
gradient_mut(&mut self, id: GradientId) -> Option<&mut GradientDef>`.
No prior method let a caller update an already-registered gradient's
own stops in place -- only `create_gradient` (push-only) existed, and
calling that fresh every frame would grow `self.gradients` without
bound (`GradientId`'s own doc comment already discloses this table has
no generational reuse/removal), defeating any zero-allocation steady
state. Mutating an existing entry's data in place is a different,
orthogonal concern from that already-disclosed lifecycle constraint, so
this method doesn't reopen it.

**Verified.** 4 new tests (155 total, up from 151): `gradient_mut`'s
own real mutation-is-visible and out-of-range-returns-`None` cases, plus
a "clears prior contents before refilling" case for both new `_into`
siblings that didn't already have one. The new demo runs 120 real
frames of the real, mutating, mixed scene with zero heap allocations
after warm-up. Every pre-existing GPU demo re-run and confirmed passing,
including `path_and_polygon_demo` (a bordered `Polygon`/`Path` scene,
confirming these fixes don't change real behavior for the lyon-
tessellated path they deliberately don't touch) and `texture_fill_demo`
(all four shape kinds' own UV mapping, confirming `bounding_box_uvs_
into`'s real output is unchanged from the original `bounding_box_uvs`).

## Summary table (Phase 10 Step 10.2.6 Implementation)

| # | Severity | Status | One-line summary |
|---|----------|--------|-------------------|
| 176 | Fixed | Resolved | generate_polygon_points/fan_from_center and bounding_box_uvs each returned a freshly heap-allocated Vec on every Polygon/texture-fill call, found by this step's own new zero-allocation guard (the first real check of these code paths under allocation pressure) -- fixed via reuse-friendly _into siblings and new ShapeRegistry-owned scratch buffers; lyon-backed Path/bordered-Polygon tessellation remains a real, disclosed, unfixed gap in the same category as main_loop_demo's own Step 9.2 exclusions |

## Phase 11 Step 11.1 Implementation (2026-09-10)

### 177. [Fixed, incidental] Wayland's `xdg_toplevel::set_app_id` was hardcoded to the literal string `"tre-walking-skeleton"` regardless of a window's real title

A Phase-0-walking-skeleton leftover in the hand-rolled Wayland backend (`wayland.rs`'s `create_window`, since deleted): every window created via the Wayland backend, for any real application, reported the same fixed `app_id` to the compositor rather than one derived from its actual title or a caller-supplied application identifier. Found while investigating whether `tre-platform` exposed full window-chrome control (title/minimize/maximize/icon), not by a dedicated audit of this specific call.

**Resolution:** disappeared as a side effect of Phase 11 Step 11.1's migration to `winit` -- `WindowAttributes` has no equivalent hardcoded-string field, and this crate's Wayland/X11 backends were deleted outright, not patched. Recorded here as an incidental fix, not a claimed deliberate one, since no dedicated `app_id`-parameterization work was done or is in scope.

### 178. [Fixed same-day] `PlatformConnection::scale_factor`'s `i32` return type throws away real per-window fractional-DPI precision `winit` now supplies

The previous backends' `scale_factor` was either a single connection-wide, last-seen-`wl_output`-scale value (Wayland, explicitly disclosed as "single-monitor dev setups only") or a hardcoded `1` (X11, `Xft.dpi`/RandR detection explicitly out of scope). `winit::window::Window::scale_factor() -> f64` gives a real per-window value (Wayland `wp-fractional-scale` falling back to integer scale; X11 `Xft.dpi`/RandR) -- a strict improvement in the underlying data. Phase 11 Step 11.1's own scope decision (preserve `PlatformConnection`'s public API exactly, to avoid touching all 40 dependent demo call sites) kept the method's return type at `i32`, so the new value was rounded to the nearest integer before returning, throwing the newly available fractional precision back away.

**Resolution:** fixed same-day, at the project owner's explicit direction. `PlatformConnection::scale_factor`/`WinitConnection::scale_factor` both widened `i32` -> `f64`, returning winit's own value directly (`1.0` default for an unknown/already-closed window, replacing the old `1`). Confirmed via a workspace-wide grep before making the change that the only real caller anywhere was `tre-platform/examples/smoke_test.rs`'s own diagnostic `eprintln!` -- none of the 40 `tre-rhi-vulkan` demos call this method at all, so the "future API-breaking step" this finding originally anticipated turned out to touch a single real call site, not 40. `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace; `smoke_test.rs` re-run for real, printing the new `f64` value correctly.

### 179. [Decision] Migrating to `winit` materially increases `tre-platform`'s dependency footprint, even with default features disabled

`cargo tree -p tre-platform` after the migration shows real new transitive dependencies beyond the previous `wayland-client`/`wayland-protocols`/`x11rb`/`as-raw-xcb-connection` combination: `smithay-client-toolkit`, `calloop`/`calloop-wayland-source`, `wayland-backend`, `wayland-protocols-wlr`/`wayland-protocols-plasma`, `xkbcommon-dl`, `x11-dl`, `ahash`, `tracing`, `memmap2`, among others. `winit`'s `default-features = false` with an explicit `["rwh_06", "x11", "wayland", "wayland-dlopen"]` feature list was used deliberately to exclude `wayland-csd-adwaita` (which would have additionally pulled in `sctk-adwaita` and `ab_glyph` font rendering, needed only for drawing winit's own client-side-decoration title bars -- irrelevant here since this project talks to real compositors that supply their own decorations).

**Resolution:** accepted, not fixed -- this is the direct, expected cost of the project owner's own explicit "proven library over hand-rolled protocol integration" decision (this session), not a surprise. Recorded here per this project's standing disclosure discipline rather than left unstated.

## Summary table (Phase 11 Step 11.1 Implementation)

| # | Severity | Status | One-line summary |
|---|----------|--------|-------------------|
| 177 | Nice-to-have | Fixed (incidental) | Wayland's `app_id` was hardcoded to `"tre-walking-skeleton"`; disappeared when the hand-rolled backend was deleted and replaced by `winit`, not from a targeted fix |
| 178 | Nice-to-have | Fixed (same-day) | `scale_factor`'s preserved `i32` return type rounded away the real per-window `f64` precision `winit` now supplies; widened to `f64` at the project owner's explicit direction -- the only real caller anywhere was a diagnostic print |
| 179 | Nice-to-have | Documented | `winit` materially increases `tre-platform`'s dependency tree even with a trimmed feature set; accepted as the direct, expected cost of the project owner's own decision to move off a hand-rolled backend |

## Full Workspace API Audit, for `API.md` (2026-09-10)

Requested as part of writing `API.md` (a complete, LLM-facing API reference) and an interactive human-facing artifact: "Make sure it is complete and correct. If API functionality is missing then fix it." Four parallel `Explore` agents extracted the complete public API surface of every real crate (`tre-engine`, `tre-platform`, `tre-memory`, `tre-math`, `tre-a11y`, `tre-svg`, `tre-text`, `tre-atlas`, `tre-rhi-vulkan`; `tre-ffi`/`tre-rhi-dx12`/`tre-rhi-metal` confirmed still genuinely empty placeholders, matching their known IMPLEMENTATION.md status), each also flagged undocumented panics, doc/code mismatches, and real API asymmetries. Findings actually fixed:

### 180. [Fixed] `tre-platform` still had no post-creation title/minimize/maximize/icon control

The exact gap identified in this session's earlier window-interface investigation, now closed: `PlatformConnection::set_title`/`set_minimized`/`is_minimized`/`set_maximized`/`is_maximized`/`set_icon` added, backed by `winit::window::Window`'s own real methods. Real testing (not assumed) found and disclosed a genuine, protocol-level behavior: `is_maximized()`/`is_minimized()` lag their own just-sent `set_*` request by a real compositor round trip -- `smoke_test.rs`'s own demo initially showed `is_maximized() == false` immediately after `set_maximized(true)` before a `poll_events()`-based fix confirmed the settled value updates correctly once the compositor's own confirmation is processed. `set_icon` (`WindowIcon`, a new `rgba`/`width`/`height` struct) confirmed working on X11, confirmed a real, disclosed no-op on Wayland (the protocol has no client-side icon mechanism at all). New `PlatformError::UnknownWindow` variant for all six new fallible methods.

### 181. [Fixed] Three places in `tre-engine/src/shapes.rs` falsely claimed `FillStyle::Texture` still panics via `unimplemented!()`

Stale since Step 10.2.2 shipped real texture-fill support for all four shape kinds -- the module's own top-level doc comment, `ShapeRegistry::flatten_into`'s `# Panics` section, and a doc block misattached to `bounding_box_uvs_into` (see #182) all still described a gap that closed weeks earlier. Corrected in all three places to state plainly that every `FillStyle` variant is real for every shape kind; the only remaining real panic is a `FillStyle::Gradient` naming a `GradientId` this registry never issued.

### 182. [Fixed] Two doc comments in `shapes.rs` were misattached to the wrong function, per Rust's own "a doc comment attaches to the next item" rule

A block describing both `ShapeRegistry::flatten_into` and `create_gradient` sat entirely above `create_gradient` (the actual `flatten_into`, far below, had only its own short `# Panics`-only block) -- so the `flatten_into` prose was orphaned onto the wrong function. The same pattern recurred: a block describing `draw_polygon_fill`'s fill dispatch sat above the unrelated private `bounding_box_uvs_into` instead of `draw_polygon_fill` itself. Both split apart and moved to sit directly above the function each actually describes.

### 183. [Fixed] `Rectangle::new()` existed; `Circle`/`Polygon`/`Path` had no equivalent convenience constructor

A real, evidenced asymmetry (`Rectangle` itself established the precedent). Added `Circle::new(radius, color)`, `Polygon::new(sides, radius, color)`, `Path::new(commands, color)`, each mirroring `Rectangle::new`'s own "minimal fields, sane defaults, set the rest directly" shape. Also fixed stale "No rendering support exists for this shape at all yet" claims on all three structs' own doc comments (predating Step 10.2's real rendering work).

### 184. [Fixed] `ShapeRegistry::gradient_mut()` had no read-only counterpart

Added `gradient(&self, id) -> Option<&GradientDef>`, matching the `get`/`get_mut` pattern the registry already uses for shapes themselves.

### 185. [Fixed] Four comments in `tre-rhi-vulkan/src/lib.rs` still named `create_stencil_and_cover_pipelines`, a function deleted 2026-09-09 (REVIEW.md finding #165) when the stencil-and-cover fill technique was retired for `lyon`

Updated to describe current reality (the helper functions these comments actually document are now shared by `create_pipeline`/`create_blend_mode_pipeline` instead).

### 186. [Fixed] Several real, reachable panics lacked `# Panics` documentation

`SpscRingBuffer::with_capacity`/`MpscRingBuffer::with_capacity` (both panic on `capacity == 0`, inconsistent with `SwmrSlotTable::with_capacity`'s own correctly-documented identical check) and `A11yBridge::publish` (poisoned-mutex panic via `.lock().unwrap()`, previously not even acknowledged as a possibility) all fixed.

### 187. [Fixed] Two small, real, low-risk API completeness gaps

`MpscRingBuffer::is_empty()` (its sibling `SpscRingBuffer` already had one) and `Affine2::to_array`/`from_array` (the type is `#[repr(C)]`, implying GPU/FFI-buffer intent, but had no raw-array conversion at all).

### 188. [Documented, not fixed] A substantial number of individual struct fields, enum variants, and trivial accessor methods across every crate lack their own doc comment

Every crate's *type-level* doc comments are thorough without exception; the gap is specifically at the field/variant/single-method granularity (e.g. most fields of `ScissorRect`, `UiDrawCommand`, `AccessibilityNode`, `PackedRect`, `ShapedGlyph`, and dozens more). Not fixed in this pass -- the volume (dozens of individually low-value one-line additions across the whole workspace) was judged out of proportion to this task's real scope (auditing for *missing functionality* and *incorrect* documentation, not a blanket doc-comment completeness pass). `API.md` itself supplies field-level descriptions for the reference regardless of what the source comments say.

### 189. [Documented, not fixed] Two RHI trait-impl methods swallow a real, recoverable `Err` into an undocumented panic

`RhiCommandBuffer::apply_layer_blur` (`tre-rhi-vulkan`) calls `.expect(...)` on `acquire_transient_target`'s `Result`, which can legitimately return `Err(EngineError::TransientPoolBudgetExceeded)` under real VRAM pressure; `RhiDevice::create_dynamic_ring_buffer` does the same on its own allocation. Both would require widening the `RhiCommandBuffer`/`RhiDevice` trait signatures themselves (in `tre-engine`, affecting every RHI backend) to fix properly -- a real, disclosed, deliberately out-of-scope architectural change for this pass, not silently worked around.

**Verified.** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean. `smoke_test.rs` re-run on both Wayland and forced X11/XWayland, confirming every new method's real, live behavior (including the settled-state round trip). `shape_full_rendering_demo`, `shape_registry_demo`, `main_loop_demo`, and `texture_fill_demo` re-run with their own real pixel-level assertions, confirming the `shapes.rs` doc/comment restructuring introduced no behavioral change.

## Phase 10 Step 10.4 Implementation (2026-09-10)

The first real slice of direct PyO3 Python bindings, in a new `tre-python` crate. See IMPLEMENTATION.md's own "Implementation status (Phase 10 Step 10.4, 2026-09-10)" write-up for the full scope account (the `pySilver` investigation, the task-by-task disposition against the step's original four-task plan, and the deliberate solid-fill-only/no-zero-copy/no-CI-job boundaries). This section records the real defects found and fixed while building and actually running the binding, not just compiling it.

### 190. [Fixed] `tre-rhi-vulkan/src/lib.rs`: a real, release-build-only `unused_mut` warning, previously undetected all session

`let mut enabled_layers: Vec<*const c_char> = Vec::new();` in `VulkanDevice::new` is only mutated inside a `#[cfg(debug_assertions)]`-gated block (the validation-layer setup just below it) -- a release build compiles that block out entirely, leaving the `mut` genuinely unused, but only in that profile. This session's own `cargo build`/`cargo clippy` verification had run exclusively in debug mode up to this point; `maturin develop --release`'s own underlying `cargo build --release` call was the first thing all session to actually build this crate in release, surfacing a warning that had been latent the whole time. Fixed with a `#[allow(unused_mut, reason = "...")]` stating the cfg-dependent asymmetry explicitly, rather than restructuring the code -- the asymmetry itself is legitimate, not a bug.

### 191. [Fixed] `EngineError` was the only error type in the workspace with no `Display`/`Error` impl

`SvgError`/`TextError`/`GradientError` and others all implement `std::fmt::Display` + `std::error::Error`; `EngineError` did not, discovered because `tre-python`'s own `TreError` mapping (`error.rs`) needs `EngineError::to_string()` to produce a real, descriptive Python exception message rather than a `Debug`-formatted one. Added both impls, covering all six variants with real, descriptive text.

### 192. [Fixed] `RhiPipelineState` lacked `Send + Sync`, blocking GIL release entirely

`PyHeadlessRenderer`'s `render()` needs to call `Python::detach` (the real requirement behind IMPLEMENTATION.md Step 10.4 task 3, "release the GIL around any engine call that can block on a GPU fence") around the blocking GPU round trip. `Python::detach`'s own closure bound requires everything it captures to be `Send`; `PipelineRegistry` holds `Box<dyn RhiPipelineState>`, and without a `Send + Sync` supertrait bound on that trait, the `Box<dyn _>` itself is neither -- which in turn blocked `PyHeadlessRenderer`'s own `#[pyclass]` derive (needs `Send`) as well as the `detach` call, three separate compiler errors all sharing this one root cause. Investigated whether `VulkanDevice`'s existing Phase 2.3 GC-thread architecture had already proven the underlying Vulkan types thread-safe (it hadn't -- the GC thread only ever holds `Arc`-cloned sub-state, never the whole device) before concluding the trait bound itself was the real, necessary fix. Fixed by adding `Send + Sync` to `RhiPipelineState`'s own trait definition in `tre-engine` -- not by marking `PyHeadlessRenderer` `#[pyclass(unsendable)]`, which would have compiled but silently defeated task 3's own requirement instead of satisfying it.

### 193. [Fixed] A real segfault at Python interpreter shutdown -- struct field drop order was backwards

`PyHeadlessRenderer` originally declared its fields as `device, swapchain, pipelines, width, height`. Rust drops struct fields in *declaration* order (top to bottom) -- the opposite of local variables, which drop in *reverse* declaration order. Every existing RHI demo declares `device`, then `swapchain`, then its pipeline(s) as local variables in `main()`, so their end-of-scope drop order is correct (pipelines and swapchain torn down before the device they were built from) without anyone having to think about it. `PyHeadlessRenderer` is a struct, not a sequence of locals, so it got the *opposite*, wrong order for free instead: `device` was destroyed first, while `HeadlessSwapchain` and `PipelineRegistry` still held live Vulkan handles built against it -- a real use-after-free, reproducing as a real `SIGSEGV` (exit code 139) at Python interpreter shutdown, after the demo script's own `print("... PASSED")` had already run successfully. Fixed by reordering the struct's fields to `pipelines, swapchain, device, width, height`, so `device` now drops last, matching the safe order every example gets implicitly. Re-verified: the demo now exits 0 with no crash.

### 194. [Fixed] `PyCircle`'s `x`/`y` semantics were undocumented, and the step's own first demo draft got them wrong

`tre_engine::Circle`'s position field is the shape's bounding-box top-left, matching `Rectangle`'s own convention (`flatten_circle`'s own doc comment in `tre-engine/src/shapes.rs`: "matches `Rectangle`'s own [convention] ... rather than centering on it, so a UI framework author positions every shape kind the same way regardless of which one it is") -- *not* the circle's center, despite "a circle's `x, y`" reading naturally as its center to a new caller. `PyCircle`'s own doc comment carried no mention of this either way. The demo script's first draft assumed "center," inserted a circle at the wrong position, and rendered the wrong pixel color at the coordinate the demo expected to be the circle's center -- caught only because the demo asserts an *exact* expected pixel value rather than merely "differs from background," which a center/top-left mixup would have silently passed. Fixed by correcting the demo's own math (`x = center_x - radius`, etc.) and adding the convention explicitly to `PyCircle`'s own doc comment, so this doesn't have to be rediscovered the same way by the next caller.

### 195. [Documented, not fixed] This pass's deliberate scope boundaries

Three real, disclosed boundaries, not oversights: (1) shapes expose `FillStyle::Solid` only -- `Gradient`/`Texture` fill are real in `tre-engine` but not yet bound to Python, matching this project's own precedent of shipping one fill kind at a time (Step 10.2.1/10.2.2). (2) `HeadlessRenderer::render()` returns a real `bytes` object (one `Vec<u8>` -> `PyBytes` copy) rather than a custom zero-copy buffer-protocol `#[pyclass]`, since the latter needs `unsafe` `__getbuffer__`/`__releasebuffer__` FFI code that would have expanded TECHNICAL.md Section 9.1's `unsafe`-permitted closed set beyond the one call this step already needed to add it for (`PyHeadlessRenderer::new`'s probe-surface teardown). (3) No CI job wired for the Python-binding test suite yet (IMPLEMENTATION.md Step 10.4 task 4) -- `demo/phase10_step10_4/demo.py`, run manually against real GPU hardware with real, exact pixel assertions, is this pass's own correctness-oracle proof instead.

**Verified.** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean, in both debug and `--release` profiles. `demo/phase10_step10_4/demo.py`, run via the project's own `.venv` (`maturin develop --release`), builds a `ShapeRegistry` with a red `Rectangle`, a green `Circle`, and a blue hexagonal `Polygon`, renders via `HeadlessRenderer` against a real Vulkan device, and asserts exact expected BGRA8 bytes at each shape's own center pixel plus a non-background check -- exits 0, no crash, after the field-reordering fix for finding #193.

## `/review-project` Pass on `tre-python` (2026-09-10)

At the user's explicit instruction ("When done with tre-python do a /review-project"), ran a four-lens multi-agent review (Performance, Architecture, Security, Modernizer) against the just-completed `tre-python` crate. Found and fixed four real, verified defects; one architectural observation left as a disclosed follow-up.

### 196. [Fixed] `HeadlessRenderer::render()` produced an empty frame -- and crashed -- on any call after the first

The single most serious finding. `PyHeadlessRenderer::render()` built a brand-new `RenderingCanvas::new()` on every call, then called `ShapeRegistry::flatten_into`, which -- by design (ARCHITECTURE.md's own incremental-flatten contract) -- skips any shape that is not `layout_dirty` and has no active animation, clearing `layout_dirty` as a side effect of the shapes it *does* flatten. The first `render()` call correctly flattens every shape (all freshly inserted, so all dirty) and clears every one of their dirty flags. Since `tre-python` exposes no way to mutate an already-inserted shape from Python at all (`insert_rectangle`/etc. convert-and-copy into the registry; the original Python object is fully decoupled afterward) and no Python-visible dirty-flag protocol, every `render()` call after the first flattened *nothing* into its fresh, empty canvas -- an empty `frame.vertices`/`frame.indices`, which `device.upload_buffer` then passed to `vkCreateBuffer` with `size = 0`, a real Vulkan spec violation (`VUID-VkBufferCreateInfo-size-00912`). Reproduced directly: a debug build (validation layers on) surfaced `[Vulkan ERROR VALIDATION] vkCreateBuffer(): pCreateInfo->size is zero`; the same call in the shipped release build (no validation layer) **segfaulted** instead of erroring cleanly. This was never caught by the demo built alongside the original Step 10.4 work because that demo only ever called `render()` once.

Root cause was purely a `tre-python`-side usage gap, not a `tre-engine` defect: `flatten_into`'s incremental semantics are correct for the multi-frame, persistent-canvas pattern every Rust demo uses (`root.reset(); registry.flatten_into(&mut root, ...)` where every mutated shape's `layout_dirty` gets re-set by the caller each frame) -- `tre-python`'s single-shot "render whatever the registry currently holds, however many times asked" contract just doesn't fit that model. Fixed by adding `ShapeRegistry::mark_all_dirty(&mut self)` to `tre-engine` (a small, real, generically useful primitive: force a full re-flatten regardless of prior state) and calling it in `render()` immediately before `flatten_into`. Re-verified via a live repro script (`HeadlessRenderer.render()` called twice on an unmutated registry) and a new regression assertion added to `demo/phase10_step10_4/demo.py` (`frame2 == frame`) -- both now pass cleanly with exit code 0.

### 197. [Fixed] `HeadlessRenderer(width, height)` accepted zero or unbounded dimensions with no validation before reaching raw Vulkan calls

`width`/`height` flowed unchecked from Python into `vk::Extent3D` for `HeadlessSwapchain::new`'s `create_image` call. `0` violates `VUID-VkImageCreateInfo-extent-00944` (undefined behavior on a release build with no validation layer, not a guaranteed clean failure); an unbounded value (e.g. `16384x16384`) allows an unauthenticated-by-nothing Python caller to trigger multi-gigabyte GPU allocations with no application-level cap. Fixed with a `validate_dimensions` check in `PyHeadlessRenderer::new` raising `ValueError` for `0` or `> MAX_DIMENSION` (8192, a conservative, documented cap). Verified: `HeadlessRenderer(0, 100)` and `HeadlessRenderer(999999, 100)` each now raise a clean `ValueError`; normal construction is unaffected.

### 198. [Fixed] `Polygon.sides`/`star_points` had no upper bound before reaching `tre_engine::generate_polygon_points`

That function computes `star_points * 2` with no overflow guard (a real, reachable overflow panic for `star_points > u32::MAX/2`) and allocates one `Vec2` per resulting vertex with no cap -- a real DoS/panic surface for a value crossing straight from untrusted Python input. Fixed by validating both fields against a new `MAX_POLYGON_SIDES` (4096) constant in `PyShapeRegistry::insert_polygon`, which now returns `PyResult<PyShapeId>` and raises `ValueError` on violation. Verified: `sides=999999999` now raises a clean `ValueError` instead of reaching the engine at all.

### 199. [Fixed] `HeadlessRenderer::render` took `&self` over single-buffered, non-thread-safe GPU state

The device/swapchain/pipelines this method drives are real, single-frame-in-flight Vulkan state (one reused command buffer, one fence, one swapchain image, per `tre-rhi-vulkan`'s own documented model) -- calling `render()` concurrently from two Python threads on the same `HeadlessRenderer` (real GIL is released for the whole GPU round trip via `Python::detach`, so this is genuinely reachable) would race on that shared state. Fixed by changing the receiver to `&mut self`: PyO3's own runtime borrow check on a `#[pyclass]` then enforces exclusive access automatically, turning a would-be GPU command-buffer race into a clean `PyBorrowMutError` instead.

Two small, safe cleanups from the Modernizer lens, also applied: four `#[pyo3(signature = (...))]` attributes on `PyRectangle`/`PyCircle`/`PyPolygon`/`PyPath::new` each just restated the constructor's own required-positional parameter list verbatim (PyO3 requires no explicit signature for a plain required-arg list) -- removed as dead boilerplate.

**Left as a disclosed follow-up, not fixed this pass** (real, but a broader refactor than a review-driven autonomous fix should take on): the Architecture lens found `PyHeadlessRenderer::render`'s own upload-execute-submit-readback sequence is the 16th near-identical copy of that same sequence across the workspace's RHI examples plus this crate -- a real, pre-existing, growing duplication at the `tre-rhi-vulkan` level (not introduced by this step) that a shared `render_and_read_frame` helper could address; out of scope for a bounded review pass on one new crate.

**Verified.** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` clean in both debug and `--release`. `demo/phase10_step10_4/demo.py` re-run end to end against real GPU hardware after every fix: all original pixel assertions still pass, plus the new double-render regression check, plus a standalone repro script confirming the zero/oversized-dimension and oversized-polygon `ValueError`s raise cleanly without reaching Vulkan.

## Workspace-Wide `submit_frame` Migration (2026-09-10)

At the user's explicit instruction ("fix Shared GPU submission helper across the workspace" -> "use option b" -> "Full migration, all ~40 files"), closed out the one item the `/review-project` pass above left as disclosed-not-fixed.

### 200. [Fixed] Real scope was ~41 files, not ~15 -- and roughly half never called `execute_frame` at all

The prior estimate (~15 files) was informal. A real grep found 41 files calling `submit_and_present`; roughly half issue hand-written `set_pipeline`/`bind_vertex_buffer`/`draw_indexed` calls directly rather than going through the shared `execute_frame` free function, so a helper wrapping `execute_frame` specifically would not have covered them. Surfaced this honestly via `AskUserQuestion` before committing to a scope, rather than silently either overcommitting or narrowing the effort described in the review artifact.

Designed and built a more general primitive instead: `tre_engine::submit_frame(device: &dyn RhiDevice, swapchain: &dyn RhiSwapchain, record: impl FnOnce(&mut dyn RhiCommandBuffer)) -> Result<(), EngineError>`, wrapping only the part that never varies across any caller -- `begin_frame` (fence-wait/image-acquire) and `submit_and_present` (submit/present), both already generic over `&dyn RhiSwapchain` -- while leaving the per-frame recording itself (an `execute_frame` call, hand-written draw calls, or a multi-pass render-to-texture sequence) to the caller's closure. This single function covers every real caller in the workspace, headless and windowed alike, with no branching.

Migrated: all 41 `tre-rhi-vulkan` examples plus `tre-python`'s own `PyHeadlessRenderer::render` (REVIEW.md #196-199's own fix). The bulk of the mechanical transform (extracting the `begin_frame`/middle/`submit_and_present` block into a `submit_frame` closure call) was done via a purpose-written, dry-run-verified script given how regular the pattern was across files with identical `let (mut cmd_buffer, image) = ...`/`cmd_buffer`/`image` naming; four real, script-missed cases were found and fixed by hand:

- `multi_window.rs`/`input_demo.rs`: `device` was already `&VulkanDevice` at the call site (a function parameter), so the script's blanket "always prepend `&`" rule produced `&&VulkanDevice`, which doesn't coerce to `&dyn RhiDevice` -- a real compiler error (`E0277`), not a silent bug, caught by the very next `cargo build`.
- `render_to_texture_demo.rs`: `bindless_index` was computed inside the moved recording block but referenced again afterward (`device.deregister_bindless(bindless_index)`, which must run once the frame is submitted) -- moving code into a closure trapped its declaration where the later use couldn't see it, a real `E0425` "cannot find value" compile error. Fixed by hoisting the registration call above `submit_frame` (it only depends on `device`/the texture, not on `cmd_buffer`, so it never needed to be inside the closure at all).
- `dual_kawase_blur_demo.rs`/`dual_kawase_nonbindless_experiment.rs`: the same pattern for `u0`/`l1`, this file's own final-hop transient texture, released after composite. Fixed by moving the `release_transient_target` call to just before the closure's own closing brace, matching how this same file already releases every earlier hop (`l0`/`l1`/`l2`/`u1`) inline during recording.
- `text_shaping_demo.rs`'s second begin/submit block (a "word render" pass using `word_cmd_buffer`/`word_image`, not `cmd_buffer`/`image`) was correctly *not* matched by the script (different variable names) and migrated by hand separately.

### 201. [Documented, not a defect] `l0`/`l1`/`l2`/`u1`/`u0` naming inside `dual_kawase_blur_demo.rs`'s closure

Not a finding against this migration -- noted here only because it's exactly the kind of thing that would have silently miscompiled if wrong: the `draw_nonbindless_pass` local closure this file defines mid-recording (originally between `begin_frame` and `submit_and_present`, now inside `submit_frame`'s own closure) captures `cmd_buffer` from its enclosing scope. Since its definition site was already inside the moved region, nesting one more closure level changes nothing about what it can see -- verified by a clean build and, more importantly, by this file's own real GPU pixel assertions (bounded-interior/edge-bleed checks) still passing after the move.

**Verified.** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean, in both debug and `--release` profiles. A full real-GPU regression sweep of all 41 migrated examples (debug build, `VK_LAYER_KHRONOS_validation` enabled) -- every demo exits 0 with zero validation errors/warnings-as-errors and, for the ~30 that carry their own internal pixel/value assertions, every one of those assertions passes for real (spot-checked directly: `dual_kawase_blur_demo`'s bounded-interior/edge-bleed checks, `render_to_texture_demo`'s composite/transparency checks including the hoisted `bindless_index` deregistration path, `text_shaping_demo`'s hand-migrated second block reporting "7 probes matched", `multi_window`/`input_demo` both creating real windows and exiting cleanly). `tre-python`'s own `demo/phase10_step10_4/demo.py`, rebuilt via `maturin develop --release` against the new `submit_frame`-based `renderer.rs`, re-run clean end to end including its own double-render regression check.

## Full-Project `/review-project` Pass (2026-09-10)

At the user's explicit instruction ("Perform a full project wide /review-project"), ran the same four-lens (Performance/Architecture/Security/Modernizer) multi-agent review across the ENTIRE workspace this time (~25,000 lines: `tre-engine`, `tre-rhi-vulkan`, `tre-atlas`, `tre-memory`, `tre-svg`, `tre-text`, `tre-math`, `tre-platform`, `tre-python` -- `tre-ffi`/`tre-rhi-dx12`/`tre-rhi-metal` skipped as empty stubs), not just the one new crate the prior pass covered. Each agent was instructed to check REVIEW.md first and not re-report anything already disclosed there.

**Result: the codebase held up well under a fresh, full-scope look.** The Modernizer lens found nothing at all -- dependencies are all at the newest version this workspace's MSRV (`rust-version = "1.75"`) permits (verified against several packages' own upstream `Cargo.toml` MSRV bumps), no deprecated API usage anywhere, no idiom inconsistencies. The Security lens confirmed the `unsafe` closed-set invariant (TECHNICAL.md Section 9.1) holds with zero leaks workspace-wide, and spot-checked several `SAFETY` comments in `tre-memory`'s lock-free primitives and `tre-rhi-vulkan/src/headless.rs` against the code they annotate -- all sound. One real Security finding did surface, fixed below (#202).

### 202. [Fixed] `tre-python`'s shape fields accepted `NaN`/`+-inf`/negative values with zero validation

`PyRectangle`/`PyCircle`/`PyPolygon`/`PyPath` accepted arbitrary `f32` coordinates and sizes with no finiteness or range check anywhere between Python and `tre_engine`'s flatten path -- the same class of gap REVIEW.md #197-198 already closed for `HeadlessRenderer` dimensions and `Polygon.sides`, just not yet generalized to every other numeric field. Two real, distinct risks: `Path` command coordinates (`move_to`/`line_to`/`quad_to`/`cubic_to`) flow unchecked into `lyon`'s tessellator, which documents finite input as a caller obligation -- a non-finite point there is a real panic/hang surface, not just a wrong render (the same category of bug REVIEW.md #106 already fixed once for font-outline geometry, never generalized to this newer Python-facing path); and `corner_smoothing` flows unclamped into the GPU style buffer despite a documented `0.0..=1.0` contract (`gpu_style.rs`) that production code never enforced.

Fixed with two small helpers (`validate_finite`, `validate_non_negative`) in `tre-python/src/shapes.rs`, applied at every real entry point: `PyShapeRegistry::insert_rectangle`/`insert_circle`/`insert_polygon`/`insert_path` (each now returns `PyResult`, raising `ValueError` on a non-finite or negative field) and `PyPath::move_to`/`line_to`/`quad_to`/`cubic_to` (validated at the call that introduces the bad coordinate, not deferred to insert time, since `Path` builds incrementally). `corner_smoothing` is clamped to `0.0..=1.0` rather than rejected -- a cosmetic parameter an animation can briefly overshoot, unlike a geometry-defining size or a path coordinate.

**Two further findings, real but left as disclosed, not fixed this pass** -- both are genuine design decisions with real trade-offs, not "clear, unambiguous" fixes, so left for a deliberate follow-up rather than taken on inside an autonomous review pass:

### 203. [Documented, not fixed] `tre-python`'s `render()` forces a full re-flatten and two fresh GPU buffer allocations on every call

The Performance lens found that `PyHeadlessRenderer::render()`'s own `mark_all_dirty()` (REVIEW.md #196's fix) forces a complete CPU re-tessellation of the whole registry every call, and `device.upload_buffer()` performs a fresh `vkCreateBuffer`+`vkAllocateMemory` twice per call (vertex + index) -- never pooled, unlike the `TransientPool`-backed paths used elsewhere in `tre-rhi-vulkan`. For a Python caller driving an animation loop (the documented "construct once, call `render()` as many times as needed" contract), this is real, compounding per-frame overhead the Rust-side render path was specifically engineered to avoid via dirty-flag skipping and scratch-buffer reuse. Real fix directions exist (a growable, reused vertex/index buffer pair; or adopting the already-proven `RhiDynamicRingBuffer` pattern `main_loop_demo.rs` uses) but both involve real design trade-offs (buffer growth policy; ring-buffer capacity planning for an unbounded Python-driven scene size) -- left for the project owner to direct rather than decided unilaterally.

### 204. [Documented, not fixed] `tre-python`'s `renderer.rs` bypasses the `RhiDevice` trait abstraction

The Architecture lens found `renderer.rs` calls `self.device.upload_buffer(bytes, vk::BufferUsageFlags::VERTEX_BUFFER)` -- an inherent `VulkanDevice` method (`tre-rhi-vulkan/src/lib.rs:1431`), not a `RhiDevice` trait method, requiring `tre-python` to depend on `ash` directly and import `vk::BufferUsageFlags` -- a real, Vulkan-specific type leak into what ARCHITECTURE.md Section 6 describes as a deliberately backend-agnostic trait boundary. Not just a style concern: `tre-python` cannot currently be retargeted to a future DX12/Metal backend without rewriting `renderer.rs`'s buffer-upload code from scratch. A real fix (adding an `upload_buffer`-shaped method to `RhiDevice`) needs a backend-agnostic `BufferUsage` enum first (mirroring the existing `TextureFormat` pattern that already abstracts `vk::Format`), since a trait method can't take a raw `ash`-specific parameter without leaking the same coupling one level up -- real API design work, left for a deliberate follow-up. Directly related to finding #203 above: fixing #203 via the `RhiDynamicRingBuffer` route (already a real `RhiDevice` trait method) would resolve this coupling at the same time, for free.

### 205. [Documented, not fixed] `tre-engine/src/lib.rs` is a 6,834-line file holding four separable concerns

The Architecture lens found `lib.rs` still holds the input event queue/`FrameClock`, the `RenderingCanvas`/IR-recording engine, `FrameArena` (sort/batch/radix-sort), and the entire RHI trait surface (`RhiDevice`/`RhiCommandBuffer`/`RhiSwapchain`/`PipelineRegistry`/`execute_frame`/`submit_frame`) all inline, at a size where sibling modules the crate already split out (`shapes.rs`, 3,932 lines; `gpu_style.rs`, 420 lines) demonstrate the project's own preferred pattern. Not a correctness issue -- every piece is individually well-documented and the public API surface wouldn't need to change (a module split keeps the same `tre_engine::X` re-export paths via `pub use`) -- but a real navigability gap in the crate's single largest file. Left undone this pass: a 6,834-line internal reorganization has real blast radius (re-export paths, `#[cfg(test)]` module placement, potential for a silent visibility regression) that deserves the project owner's explicit go-ahead before undertaking, matching how this session's own `submit_frame` migration scope was confirmed via `AskUserQuestion` before proceeding rather than assumed.

**Verified.** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean, in both debug and `--release` profiles. `demo/phase10_step10_4/demo.py` re-run clean after the validation fix (all prior assertions plus the double-render check still pass). A standalone script confirmed the new validation: `NaN`/negative `Rectangle.width`, `+-inf` `Circle.radius`, and `NaN` `Path.move_to` coordinates all now raise a clean `ValueError`; `corner_smoothing = 5.0` clamps silently to `1.0` rather than erroring; ordinary shape construction is unaffected.

## Finding #203 Fixed: `tre-python` Adopts `RhiDynamicRingBuffer` (2026-09-10)

At the user's explicit instruction ("Fix render() re-tessellates and re-allocates GPU buffers on every call with Option B"), closed finding #203 by adopting the project's own established ring-buffer pattern (`main_loop_demo.rs`/`shape_registry_zero_alloc_demo.rs`) -- which, as REVIEW.md #204 itself predicted, resolved the `RhiDevice`-trait-bypass finding at the same time, for free: `create_dynamic_ring_buffer`/`RhiDynamicRingBuffer::write` are real trait methods, so `PyHeadlessRenderer` no longer needs a direct `ash` dependency or Vulkan-specific `vk::BufferUsageFlags` at all -- `use ash::vk;` is gone from `renderer.rs` entirely.

`PyHeadlessRenderer` gained one persistent `ring_buffer: Box<dyn RhiDynamicRingBuffer>` field (created once in `new()`, `RING_BUFFER_CAPACITY = 512 * 1024` bytes shared between vertex and index writes -- generous headroom above the `64 * 1024` two existing real demos already prove sufficient for a real mixed multi-shape scene, since a Python-driven scene isn't bounded the way a fixed demo scene is). `render()`'s two `upload_buffer` calls (a fresh `vkCreateBuffer`+`vkAllocateMemory` every single call) are replaced with two `ring_buffer.write()` calls (a bump-allocation into an already-live, already-mapped buffer). Field declaration order required the same care as findings #193/#199's own struct-ordering discipline: `ring_buffer` joins `pipelines`/`swapchain` ahead of `device` in the struct, since it too holds a live buffer built from `device`.

A real, necessary upstream fix, surfacing the same category of gap findings #192/#199 already closed once: `RhiBuffer` (the trait `RhiDynamicRingBuffer` extends) lacked `Send + Sync`, blocking the pyclass holding it from compiling at all. Unlike `RhiPipelineState`'s earlier fix, adding the bound to `RhiDynamicRingBuffer` alone was not sufficient -- `BufferBinding.buffer: &dyn RhiBuffer`'s own coercion needed the bound at the *base* trait, since a trait object's auto-trait properties belong to its own static type, not to whatever concrete type was coerced from. Two test-only fallout fixes followed directly from this: `tre-engine`'s two `FakeStyleBuffer` test doubles (`lib.rs` and `shapes.rs`, near-identical) used `RefCell<Vec<u8>>` internally, which isn't `Sync` -- switched to `Mutex<Vec<u8>>`, the compiler's own suggested fix, since neither fake is actually touched from more than one thread in practice.

`RhiDynamicRingBuffer::write`'s own documented contract is `Option<u32>`, `None` on starvation (deliberately narrower than `EngineError`'s blanket `Result` policy per that trait's own doc comment, REVIEW.md #142 -- real graceful-degradation policy work still pending). Rather than `.expect()`-crashing on `None` the way `main_loop_demo.rs` currently does, `tre-python` gets a real, local `RenderError` enum (`Engine(EngineError)` / `RingBufferStarved`) converting starvation into a clean `TreError` with an actionable message, rather than widening `tre_engine::EngineError` itself for one caller's own choice of degradation policy.

**Verified.** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean, in both debug and `--release` profiles. `demo/phase10_step10_4/demo.py` re-run clean. Directly proved the fix's own point: 200 consecutive `render()` calls on the same renderer (simulating a real animation loop) all succeed with no crash, averaging ~1.1ms/call; a deliberately oversized scene (5,000 32-sided polygons) correctly raises a clean `TreError` naming the real capacity instead of crashing, and the renderer remains fully usable for a normal-sized scene immediately afterward -- no corrupted or poisoned state left behind by the starvation path.

## Finding #205 Fixed: `tre-engine/src/lib.rs` Split Into Modules (2026-09-10)

At the user's explicit instruction ("Split lib.rs into modules"), closed the last open item from the full-project review. `lib.rs` went from 6,862 lines to 3,490 -- and of those 3,490, roughly 3,074 are the pre-existing test module left in place (see below for why); the crate's *real* non-test surface shrank from ~3,788 lines to ~416, with the rest now living in three new, purpose-named files:

- `input.rs` (192 lines): `WindowId`, `MouseButton`, `ElementState`, `InputEvent`, `InputEventQueue`, `FrameClock`. Fully self-contained -- confirmed via grep before extraction that nothing else in the crate's other new modules depends on it.
- `canvas.rs` (2,361 lines): `RenderingCanvas`/`SubCanvas`/`GlyphAtlasContext` (the Drawing Context IR-recording engine) *and* `FrameArena`/the sort-batch pipeline (`segment_and_flatten`, `radix_sort_by_key`, `compute_sort_key`, `premultiply_alpha`, `flatten_run`, ...), merged into one module rather than the two the original finding suggested -- a real, evidenced decision, not a shortcut: `RenderingCanvas::flatten()`/`flatten_unbatched()` call directly into the sort/batch helpers (confirmed via grep before deciding), and `FrameArena` exists specifically to serve multi-threaded canvas recording, so splitting them would have meant `pub(crate)`-widening half a dozen more private functions across a module boundary that both halves would still need to cross constantly. One cohesive module matches the real coupling; two would have been a boundary drawn for its own sake.
- `rhi.rs` (870 lines): the entire RHI trait surface (`RhiDevice`/`RhiCommandBuffer`/`RhiSwapchain`/`RhiBuffer`/`RhiTexture`/`RhiPipelineState`/`RhiDynamicRingBuffer`), `PipelineRegistry`, `execute_frame`, `submit_frame`. Confirmed fully independent of `canvas`/`input` before extraction (zero real references, one doc-comment mention).

Every extracted item's real, external `tre_engine::X` path is preserved exactly via `pub use` re-exports at the crate root -- `cargo build --workspace` confirmed this directly: every downstream crate (`tre-platform`, `tre-rhi-vulkan` and all 41 examples, `tre-a11y`, `tre-svg`, `tre-ffi`, `tre-python`) compiled with zero changes needed anywhere outside `tre-engine` itself.

**The pre-existing, ~3,074-line test module was deliberately left in `lib.rs`, not split to match.** A real, disclosed scope boundary, not an oversight: triaging which of that module's many tests belong to which of the three new modules, and verifying each move doesn't silently lose coverage, is a materially different (and materially riskier) task than the code-motion done here -- REVIEW.md's own original finding already flagged "a 6,834-line internal reorganization has real blast radius," and splitting a large, long-lived, extensively cross-referenced test suite is exactly the kind of risk that warning was about. This did require two real categories of fallout fixes, found only by actually attempting the split, not assumed safe in advance:

- Several free functions/consts the surviving test module references by bare name (`compute_sort_key`, `flatten_run`, `premultiply_alpha`, `radix_sort_by_key`, `DEPTH_ID_MASK`) were private to the old single-file `lib.rs`; moving their defining code into `canvas.rs` made them invisible to `lib.rs`'s own test module, now a *sibling* of `canvas` rather than a descendant of the same file. Widened to `pub(crate)` in `canvas.rs`, re-exported via a `#[cfg(test)]`-gated `pub(crate) use canvas::{...};` at the crate root (gated since these exist purely to serve the surviving tests -- a plain `--lib` build has no need of them and would otherwise warn).
- Two `RenderingCanvas` fields (`vertices`, `commands`) and two of its own methods (`new_with_sub_canvas_cap`, `next_sort_key`) were private and directly accessed/called by several existing tests -- the same "now a sibling, not a descendant" visibility loss. Widened to `pub(crate)` directly on the struct/impl; no crate-root re-export needed for these, since the type itself was already fully `pub`.

A real, self-caught false start along the way: the initial draft used `use crate::*;` (a glob import) in both new modules to sidestep manually enumerating every foundational-type dependency -- caught by `clippy::wildcard_imports` (part of this workspace's `-D warnings` gate) before it could ship, and replaced with the exact explicit import list clippy's own diagnostic suggested, pruned further once `unused_imports` flagged several of clippy's suggested names as only ever appearing inside doc-comment prose, never real code (`OVERLAY_LAYER_BASE`'s and `segment_and_flatten`'s own supposed test references turned out to be the same thing -- plain-text mentions inside an assertion message and a doc comment, not real usage, so neither needed the `pub(crate)` widening or re-export originally planned for them).

**Verified.** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean, in both debug and `--release` profiles, with zero changes needed in any crate other than `tre-engine`. All 155 `tre-engine` tests pass unchanged. A real-GPU spot-check across 8 representative examples (`headless`, `shape_registry_demo`, `main_loop_demo`, `canvas_layer_composite_demo`, `text_shaping_demo`, `dual_kawase_blur_demo`, `multi_window`, `shape_registry_zero_alloc_demo`) -- chosen to cover headless/windowed, single-shot/continuous-loop, and the most RHI-surface-heavy demo in the workspace -- all pass with their own real pixel/equivalence/zero-allocation assertions intact, not just a clean exit code. Not a full 41-example sweep this time (unlike the `submit_frame` migration): this change touched zero example files, only `tre-engine`'s own internal organization, so the real risk surface was `tre-engine` itself, which got the full local test suite plus this representative spot-check. `tre-python`'s own demo, including its double-render regression check, re-verified clean after rebuilding against the reorganized crate.
