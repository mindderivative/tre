# TRE (Tesserae Render Engine) — Archived v0.1.0 Alpha

This folder holds the complete first iteration of TRE: a low-overhead,
hardware-accelerated 2D rendering engine targeting Linux + Vulkan 1.2+,
with Python bindings and a C ABI, built to eventually back GUI
frameworks. It is archived here, unchanged from its last working state,
as reference material while a second iteration is designed from scratch.

**Git tag `archived-2026-09-14`** points at the exact commit this
archive was taken from — the full commit history below that tag is the
complete, real development record: 21 implementation phases, 261
numbered review findings, and every CI failure and fix along the way.
Nothing here is a snapshot with history stripped; `git log`, `git
blame`, and `git show` all work normally against this archived tree,
since it was moved with `git mv`, not copied out of the repo.

## What's in here

- **`crates/`** — the 16-crate Cargo workspace: `tre-engine` (core
  canvas/shape/RHI-trait layer), `tre-rhi-vulkan` (the one real backend),
  stub `tre-rhi-dx12`/`tre-rhi-metal`, `tre-platform` (winit-backed
  windowing), `tre-text`/`tre-atlas`/`tre-svg`/`tre-a11y`/`tre-tween`/
  `tre-animation`/`tre-memory`/`tre-math`, `tre-ffi` (C ABI),
  `tre-python` (PyO3 bindings), `tre-perf-suite`.
- **`documentation/REVIEW.md`** — the single most valuable file in this
  archive: an append-only ledger of 261 numbered findings across
  performance, architecture, security, and modernization passes, each
  with what was found, why, and how it was verified. Start here for
  **why** any specific design choice was made.
- **`documentation/{DESIGN,TECHNICAL,ARCHITECTURE,IMPLEMENTATION}.md`** —
  the standing design documents, kept current throughout development.
- **`README.md`** (this folder's, one level down from this index) — the
  project's own public-facing README, badges and all, as it shipped.
- **`demo/`** — ~30 phase-numbered Python scripts, each a real,
  self-asserting demonstration of one feature as it was built.
- **`docs/`** — the published MkDocs site source (API reference).
- **`planning/archive/`** — per-step plan/log files from the project's
  own phase-by-phase workflow.
- **`.github/workflows/`** — the CI pipeline as it stood: eight jobs
  (fmt, clippy, msrv, build, test, python, vulkan-validation,
  accessibility-validation), all green at archive time.
- **`LOG.md`** / **`PLAN.md`** — the most recent phase's plan and log
  (release prep for v0.1.0).

## Where to start

1. **`LESSONS_LEARNED.md`** (this folder) — a distilled retrospective
   written for the next iteration, not a restatement of REVIEW.md.
2. **`documentation/ARCHITECTURE.md`** — the system as it ended up.
3. **`documentation/REVIEW.md`**'s own top preamble — a running summary
   of what every finding pass found and fixed, updated after each pass.
4. **`CHANGELOG.md`** — the v0.1.0 release notes, organized by subsystem.

## Status at archive time

Working tree clean, all CI jobs green, 387 Rust tests passing, 42
additional Vulkan integration targets passing on both a real GPU and the
software (lavapipe) driver, and 29 of 30 Python demos passing end-to-end
against a freshly built wheel (the 30th intentionally skipped — it needs
a live desktop portal). Nothing was mid-flight or broken; this is a
clean stopping point, not an abandoned one.
