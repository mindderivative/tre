# Plan: Phase 10 Step 10.4 — Direct PyO3 Python Bindings (`tre-python`), First Slice

**Status: Complete (2026-09-10).** See `documentation/IMPLEMENTATION.md`'s
own "Implementation status (Phase 10 Step 10.4, 2026-09-10)" write-up for
the full technical account, `documentation/REVIEW.md`'s "Phase 10 Step
10.4 Implementation" section (findings #190-195), and
`demo/phase10_step10_4/README.md` for the verification summary. This file
records the plan as it was actually executed (this step ran via direct
instruction, not `EnterPlanMode`/`ExitPlanMode`), archived here now that
the work is done, per this project's own "one active plan, archived once
real work begins" convention (see `documentation/PLAN.md`'s own git
history for prior steps' plans, each superseded in place the same way).

## User request (verbatim)

> Start 10.4 we will work on 10.3 after.

Mid-turn: > Work with pySilver project for this if you need

Scope confirmed via `AskUserQuestion`: **"Generic PyO3 wrapper (original
plan)"** — bind `tre-engine`'s existing `Canvas`/`ShapeRegistry` API
directly, not an attempt to match the separate `pySilver` project's own
incompatible single-instanced-draw-call rendering contract.

## Context

IMPLEMENTATION.md's own pre-existing Step 10.4 plan (written 2026-09-09,
before this session) already specified: a new `tre-python` crate binding
`tre-engine` directly via PyO3 (not through `tre-ffi`'s C-ABI), four
tasks (shape/registry bindings, Pythonic ergonomics, GIL release around
blocking GPU calls, a CI job), and the rationale for bypassing `tre-ffi`
(avoiding a double marshalling round trip for a first-party, high-frequency
boundary). This step executes that existing plan's first real slice.

Before writing any binding code, investigated the project's own real
Python UI framework, `pySilver` (`/home/phil/pyDev/projects/pysilver`), per
the user's mid-turn direction. Found its actual rendering contract (one
instanced draw call per frame over a flat 144-byte-per-instance `numpy`
array, 7 primitive kinds, its own WGSL SDF shader) is architecturally
incompatible with `tre-engine`'s multi-pipeline `Canvas`/`ShapeRegistry`
design, and that `pySilver`'s own `ARCHITECTURE.md` has zero TRE-integration
detail despite its README's stated migration intent. Surfaced this via
`AskUserQuestion` rather than guessing; the project owner's answer (above)
is this step's definitive scope.

## Tasks (against IMPLEMENTATION.md's own pre-existing four-task plan)

1. Shape/registry bindings: `Rectangle`/`Circle`/`Polygon`/`Path` +
   `ShapeRegistry`, `#[pyclass]`/`#[pymethods]` directly over `tre-engine`
   types. **Done for solid fill only** — `Gradient`/`Texture` fill deferred.
2. Pythonic ergonomics: `TreError` exception mapping, `Canvas.save()`/
   `restore()` context manager, buffer-protocol-adjacent frame readback.
   **Partially done** — exceptions and `Canvas` save/restore are real;
   zero-copy buffer protocol deliberately not built (would need `unsafe`
   FFI code expanding TECHNICAL.md Section 9.1's closed set beyond the one
   call this step already needed to add `tre-python` to it for); a real
   `bytes` copy is returned instead.
3. Release the GIL around blocking GPU calls. **Done** — required a real,
   necessary upstream fix: `RhiPipelineState: Send + Sync` in `tre-engine`
   (REVIEW.md #192), without which `Python::detach` (PyO3 0.27's
   `allow_threads` replacement) could not be used at all.
4. Wire a CI job for the Python-binding test suite. **Not done** — a real
   local demo (`demo/phase10_step10_4/demo.py`) is this pass's own
   correctness-oracle proof instead, matching every prior phase's own
   "real demo before CI automation" sequencing.

## Real defects found and fixed while building and actually running this
## (not merely compiling it) — full detail in REVIEW.md #190-194

- A real, release-build-only `unused_mut` warning in `tre-rhi-vulkan`,
  previously undetected because this session's own verification had only
  ever run in debug mode until `maturin develop --release` forced a real
  release build for the first time.
- `EngineError` was the only error type in the workspace missing
  `Display`/`Error` impls, needed for `TreError`'s own message text.
- `RhiPipelineState` lacked `Send + Sync` (see task 3 above).
- A real segfault at Python interpreter shutdown: `PyHeadlessRenderer`'s
  struct fields were declared in the wrong order (Rust drops struct fields
  in *declaration* order, the opposite of local variables, which every RHI
  *example*'s own `main()` gets the safe order from for free) — `device`
  was destroyed before `swapchain`/`pipelines` still held live handles
  against it. Fixed by reordering fields so `device` drops last.
- `Circle`'s `x`/`y` mean bounding-box top-left (matching `Rectangle`),
  not center — undocumented on `PyCircle`, and this step's own demo script
  got it wrong on first use before being corrected and documented.

## Verification plan — executed exactly as planned

`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D
warnings`, `cargo build --workspace --all-targets`, `cargo test --workspace`
all clean, in both debug and `--release` profiles. `demo/phase10_step10_4/
demo.py`, run against real GPU hardware via `maturin develop --release`,
builds a `ShapeRegistry` with a red `Rectangle`, a green `Circle`, and a
blue `Polygon`, renders via `HeadlessRenderer`, and asserts exact expected
BGRA8 bytes at each shape's own center pixel — exits 0, no crash, after the
struct-field-ordering fix.

## Deferred (explicit user instruction: "we will work on 10.3 after")

Phase 10 Step 10.3 (the `tre-ffi` C-ABI crate) — not started this step.
