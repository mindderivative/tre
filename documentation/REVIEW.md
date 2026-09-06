# Documentation Review — September 2026

Reviewer: Claude (Cowork), acting as Principal Engineer / Lead Tech Architect, per project standing instructions.
Scope: `DESIGN.md`, `TECHNICAL.md`, `ARCHITECTURE.md`, `IMPLEMENTATION.md`, reviewed in that order. All findings below have been implemented directly in those four files; this document is the record of what was found and what changed.

Status: **All findings implemented.** See "Follow-up: Rust/Python Language Migration," "Review of Rust-Specific Additions," "Full Documentation Review," "Engineering Decisions: Suggested Improvements Actioned," "Phase 0 Implementation" (2026-09-04), "Phase 1 Step 1 Implementation," "Pre-Phase-1-Step-2 Doc Check," "Phase 1 Step 2 Implementation," "Phase 1 Review," "Phase 2 Step 1 Implementation," "Phase 2 Step 2 Implementation," "Phase 2 Step 2.1 Implementation," "Phase 2 Code Review" (2026-09-05), "Phase 2 Step 2.3 Implementation," and "Phase 2 Step 2.3 Code Review" (2026-09-06) below for subsequent, out-of-band work not part of this original review. "Phase 1 Review"'s finding #51 has since been fixed; #52-53 remain deliberately deferred to Phase 2 (not yet revisited) — see that section for disposition. "Phase 2 Code Review"'s findings #66-70/#72/#73/#75 have since been fixed and re-verified (fmt/clippy/build/test clean, all six examples re-run with zero validation errors, #66 additionally proven via a deliberate-bug run); #71 fixed (VulkanSwapchain's matching #56 remains separately open); #74 documented as deliberate rather than changed; #76 left unfixed (no safe way to determine correct CI package pins from this environment) — see that section for full disposition. All of Phase 2 (Steps 2.1, 2.2, 2.4, and now 2.3) is complete as of 2026-09-06. "Phase 2 Step 2.3 Code Review"'s findings #78-82 have since been fixed and re-verified (fmt/clippy/build/test clean, all seven examples re-run with zero validation errors, `gc_demo` re-run three more times confirming consistent behavior under the new admission cap); #83 is not a defect — see that section for full disposition. "Phase 3 Step 3.1 Implementation", "Phase 3 Step 3.2 Implementation", "Phase 3 Step 3.3.1 Implementation", and "Phase 3 Step 3.3.2 Implementation" (2026-09-06) are also complete, below. Step 3.2's finding #84 has been fixed and re-verified (all 7 pre-existing examples re-run with zero validation errors, plus the new `sdf_rounded_rect_demo`); #85 is not a defect — see that section for full disposition. Step 3.3.1's findings #86-87 have both been fixed and re-verified (fmt/clippy/build/test clean, all 7 pre-existing examples re-run with zero validation errors, plus the new `svg_tessellation_demo`) — see that section for full disposition. Step 3.3.2's finding #88 (a unit-test-only issue) has been fixed and re-verified (all 8 pre-existing examples re-run with zero validation errors, plus the new `svg_morph_demo`) — see that section for full disposition. IMPLEMENTATION.md Step 3.3's remaining sub-step (3.3.3 stencil-and-cover fallback) is not yet started.

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
