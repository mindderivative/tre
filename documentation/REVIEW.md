# Documentation Review — September 2026

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: `DESIGN.md`, `TECHNICAL.md`, `ARCHITECTURE.md`, `IMPLEMENTATION.md`, reviewed in that order. All findings below have been implemented directly in those four files; this document is the record of what was found and what changed.

Status: **All findings implemented.** See "Follow-up: Rust/Python Language Migration," "Review of Rust-Specific Additions," "Full Documentation Review," "Engineering Decisions: Suggested Improvements Actioned," "Phase 0 Implementation" (2026-09-04), "Phase 1 Step 1 Implementation," "Pre-Phase-1-Step-2 Doc Check," "Phase 1 Step 2 Implementation," "Phase 1 Review," "Phase 2 Step 1 Implementation," "Phase 2 Step 2 Implementation," "Phase 2 Step 2.1 Implementation," "Phase 2 Code Review" (2026-09-05), "Phase 2 Step 2.3 Implementation," and "Phase 2 Step 2.3 Code Review" (2026-09-06) below for subsequent, out-of-band work not part of this original review. "Phase 1 Review"'s finding #51 has since been fixed; #52-53 remain deliberately deferred to Phase 2 (not yet revisited) — see that section for disposition. "Phase 2 Code Review"'s findings #66-70/#72/#73/#75 have since been fixed and re-verified (fmt/clippy/build/test clean, all six examples re-run with zero validation errors, #66 additionally proven via a deliberate-bug run); #71 fixed (VulkanSwapchain's matching #56 remains separately open); #74 documented as deliberate rather than changed; #76 left unfixed (no safe way to determine correct CI package pins from this environment) — see that section for full disposition. All of Phase 2 (Steps 2.1, 2.2, 2.4, and now 2.3) is complete as of 2026-09-06. "Phase 2 Step 2.3 Code Review"'s findings #78-82 have since been fixed and re-verified (fmt/clippy/build/test clean, all seven examples re-run with zero validation errors, `gc_demo` re-run three more times confirming consistent behavior under the new admission cap); #83 is not a defect — see that section for full disposition. "Phase 3 Step 3.1 Implementation", "Phase 3 Step 3.2 Implementation", "Phase 3 Step 3.3.1 Implementation", and "Phase 3 Step 3.3.2 Implementation" (2026-09-06) are also complete, below. Step 3.2's finding #84 has been fixed and re-verified (all 7 pre-existing examples re-run with zero validation errors, plus the new `sdf_rounded_rect_demo`); #85 is not a defect — see that section for full disposition. Step 3.3.1's findings #86-87 have both been fixed and re-verified (fmt/clippy/build/test clean, all 7 pre-existing examples re-run with zero validation errors, plus the new `svg_tessellation_demo`) — see that section for full disposition. Step 3.3.2's finding #88 (a unit-test-only issue) has been fixed and re-verified (all 8 pre-existing examples re-run with zero validation errors, plus the new `svg_morph_demo`) — see that section for full disposition. "Phase 3 Step 3.3.3 Implementation" (2026-09-06) is also complete, below, and with it all of IMPLEMENTATION.md Step 3.3 (3.3.1-3.3.3). Findings #89-90 have both been fixed and re-verified (fmt/clippy/build/test clean, all 10 pre-existing examples re-run with zero validation errors, plus the new `stencil_and_cover_demo`) — see that section for full disposition. "Phase 4 Step 4.1 Implementation" (2026-09-06) is also complete, below. Finding #91 (a unit-test-only issue) has been fixed and re-verified (all 11 pre-existing examples re-run with zero validation errors, plus the new `text_shaping_demo`) — see that section for full disposition. "Phase 4 Step 4.2.1 Implementation" (2026-09-06) is also complete, below (the first of Step 4.2's four sub-steps). Finding #92 (a real, engine-wide sRGB gamma gap, already independently scheduled as Step 7.1) is documented and deliberately not fixed here; #93 (a self-authored demo bug) has been fixed and re-verified (all 12 pre-existing examples re-run with zero validation errors, plus the new `atlas_packing_demo`) — see that section for full disposition. "Phase 4 Step 4.2.2 Implementation" (2026-09-06) is also complete, below (the second of Step 4.2's four sub-steps). Findings #94-95 (both minor, non-architectural) have been fixed and re-verified (all 13 pre-existing examples re-run with zero validation errors, plus the new `msdf_generation_demo`) — see that section for full disposition. "Phase 4 Step 4.2.3 Implementation" (2026-09-06) is also complete, below (the third of Step 4.2's four sub-steps, and the one that resolves the jagged-'X' observation from Step 4.1) — no findings this step; see that section for full disposition. "Phase 4 Step 4.2.4 Implementation" (2026-09-06) is also complete, below (the fourth and closing sub-step of Step 4.2, and with it all of Phase 4). Findings #96-97 (both minor, non-architectural, both in demo code) have been fixed and re-verified (all 15 pre-existing examples re-run with zero validation errors, plus the new `atlas_concurrency_demo`) — see that section for full disposition. "Phase 1-4 Comprehensive Review" (2026-09-06) is also complete, below — a 6-dimension, twice-checked (independent Find + adversarial Verify) audit of everything built across Phases 1-4, requested once Phase 4 closed. All 19 findings (#98-116) were independently confirmed by their verifier; 12 were fixed directly (#98-100, #104-108, #110-112, #115) and re-verified (fmt/clippy/test clean across the workspace, all 15 pre-existing GPU examples plus `msdf_generation_demo` re-run for real with zero validation errors); 7 whose real fix is substantial new feature work rather than a bug fix (#101-103, #109, #113-114, #116) were deliberately left as clearly disclosed, documented gaps instead of being built opportunistically inside this pass — see that section for full disposition of each. "Phase 4 Step 4.3.1 Implementation" (2026-09-06) is also complete, below — the first of Step 4.3's three sub-steps, closing finding #114 above. Finding #117 (a real bug in the new tombstone-reuse logic, caught by a dedicated unit test before this code had any real caller) has been fixed and re-verified — see that section for full disposition. "Phase 4 Step 4.3.2 Implementation" (2026-09-06) is also complete, below — the second of Step 4.3's three sub-steps, no findings this time. "Phase 4 Step 4.3.3 Implementation" (2026-09-06) is also complete, below — the third and closing sub-step of Step 4.3, wiring the previous two sub-steps' primitives into a real eviction policy in `AtlasOwner` and finally closing finding #114. No numbered findings this sub-step; a cold-start recency hazard was identified and designed around during planning itself rather than discovered afterward — see that section for full detail. "Phase 5 Step 5.1.1 Implementation" (2026-09-07) is also complete, below — the first of Step 5.1's three sub-steps, opening Phase 5. Findings #118 (a real bug: alpha-only vertex scaling made `set_alpha()` invisible) and #119 (a demo test-design bug) have both been fixed and re-verified — see that section for full disposition.

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
TECHNICAL.md Section 9.1 stated the `cdylib`/`staticlib` build targets set `panic = "abort"`, while that same paragraph — plus DESIGN.md Section 2.7 and IMPLEMENTATION.md Phase 10 Step 10.1 — relies on `std::panic::catch_unwind` at every FFI entry point to convert a panic into a recoverable `EngineError`. These are mutually exclusive: `panic = "abort"` terminates the process the instant a panic fires, before any stack unwinding occurs, so `catch_unwind` can never trigger and silently becomes dead code. As written, any panic anywhere in the engine crashes the host Python process outright — precisely the outcome Section 2.7 says the `catch_unwind` wrapper exists to prevent.

**Change:** TECHNICAL.md Section 9.1 corrected: the `tre-ffi` crate and its full dependency graph must build with the default `panic = "unwind"` strategy, and `panic = "abort"` is now explicitly called out as prohibited on any profile used to build the shipped `cdylib`/`staticlib`, with the reasoning (it makes `catch_unwind` a no-op) stated inline so a future contributor optimizing binary size doesn't reintroduce it.

### 20. [Should-fix] "Only crate compiled into the cdylib" is imprecise to the point of being misleading
TECHNICAL.md Section 9.2 and IMPLEMENTATION.md Phase 10 Step 10.1 both stated that `tre-ffi` is "the only crate compiled into the shipped `cdylib`/`staticlib`." Taken literally this is false: `tre-engine` and the RHI backend crates' code must be statically linked into that same binary for the engine to function at all. The intended meaning — that `tre-ffi` is the only crate whose items are exported as public `extern "C"` symbols — was never actually stated.

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

**Change:** not fixed here -- implementing real linear-space color management now would preempt Step 7.1's own, more complete scope (HDR, tone mapping) with a partial shader patch. `atlas_packing_demo`'s own palette instead uses only the 7 "pure" (each channel 0 or 255) colors, which round-trip correctly both today and after Step 7.1's real fix, so this demo needs no revisiting later.

### 93. [Nice-to-have] The demo's own `pixel_at` helper compared real BGRA memory bytes against an RGBA-ordered expectation
`HeadlessSwapchain::read_pixels_bgra8` is correctly named and returns genuine BGRA memory byte order. This demo's first draft of `pixel_at`, copied from the identical pattern every prior demo already used, returned those bytes unswapped and compared them against a `[R,G,B,A]`-ordered expected color -- invisible in every prior demo since white/black are invariant under a channel swap (R=G=B). A real red rectangle rendered back as `[0,0,255,255]` (blue) under the wrong comparison.

**Change:** `pixel_at` now explicitly swaps indices 0 and 2 before returning, matching `PALETTE`'s `[R,G,B]` convention. Not an engine defect -- `read_pixels_bgra8` behaves exactly as documented; this was this demo's own first attempt at being the first caller to care about channel order at all.

## Summary table (Phase 4 Step 4.2.1)

| # | Finding | Doc(s)/Code | Severity | Resolution |
|---|---|---|---|---|
| 92 | `walking_skeleton.frag`/`sdf_rounded_rect.frag` never perform the sRGB-to-linear conversion `UiVertex::color` documents | tre-rhi-vulkan (shaders) | Should-fix | Documented, deferred to Step 7.1 (already independently scheduled there); this step's own demo works around it with a gamma-invariant palette |
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
