# Tesserae Render Engine (TRE)

[![CI](https://github.com/mindderivative/tre/actions/workflows/ci.yml/badge.svg)](https://github.com/mindderivative/tre/actions/workflows/ci.yml)

A low-overhead, hardware-accelerated 2D rendering engine designed as a bridge between high-level UI frameworks and low-level graphics APIs (Vulkan, DirectX 12, Metal, WebGPU).

## Status

**19 phases shipped and merged to `main`** (`documentation/IMPLEMENTATION.md`'s own phased plan, Phase 0's walking skeleton through Phase 19's focus/keyboard-navigation system), plus Phase 10 Step 10.3's real first slice of the `tre-ffi` C-ABI crate. A real Vulkan 1.2+ backend renders headless and windowed, multi-window, multi-threaded scenes with real vector shapes, dynamic MSDF typography, animated SVG, gradients/textures, non-`Normal` blend modes, real shadows (both Dual-Kawase-blur- and SDF-based), a full multi-line text editor with clipboard/IME, native window chrome/clipboard/file-dialogs/system tray, and a real Linux AT-SPI2 accessibility bridge with in-app widget focus/Tab order — all exercised by `tre-python`, the project's real, privileged Python binding, and now also by `tre-ffi`'s own real (if intentionally narrower) C-ABI first slice for every other language. DirectX 12/Metal backends and Windows/macOS windowing remain real, disclosed placeholders.

See `documentation/IMPLEMENTATION.md` for the full phase-by-phase build history, `documentation/REVIEW.md` for every real bug found and fixed along the way, and the [MkDocs site](docs/) (`mkdocs serve` from the repo root) for the complete Python and Rust API reference.

## Overview

- Implemented in **Rust**. The project's own high-level UI framework is built in **Python**, and binds directly to `tre-engine`'s native Rust API via PyO3 (`tre-python`) rather than through a C-ABI — a deliberate, disclosed performance choice (see `documentation/DESIGN.md` Section 2.7's "Cross-Language Boundary: Two Real Paths").
- Every other language reaches the engine through `tre-ffi`, a stable `#[repr(C)]`/`extern "C"` boundary — real as of Phase 10 Step 10.3's first slice (headless rendering, `Rectangle`/`Circle`/`Polygon`/`Path`), with gradients/textures/text/windowing as disclosed follow-up work.
- Targets ultra-low-latency desktop UI rendering: up to 240 Hz frame delivery, a zero-allocation steady state (mechanically enforced in CI), vector path rendering, dynamic typography (MSDF), animated SVG, HDR/wide-gamut color, multi-window support, and headless/CI rendering.

## Documentation

| Document | Contents |
|---|---|
| [DESIGN.md](documentation/DESIGN.md) | Design philosophy, core principles, failure modes, target applications, subsystem overviews |
| [TECHNICAL.md](documentation/TECHNICAL.md) | Performance budgets, hardware/platform requirements, toolchain, FFI/Python binding spec |
| [ARCHITECTURE.md](documentation/ARCHITECTURE.md) | Subsystem decomposition, core data structures, the 64-bit sort key, RHI trait interfaces |
| [IMPLEMENTATION.md](documentation/IMPLEMENTATION.md) | Phased implementation plan and its full, real, phase-by-phase build history |
| [REVIEW.md](documentation/REVIEW.md) | Running record of documentation reviews, findings, and engineering decisions |

Start with [DESIGN.md](documentation/DESIGN.md) for the *why*, [ARCHITECTURE.md](documentation/ARCHITECTURE.md) for the *how*, and [IMPLEMENTATION.md](documentation/IMPLEMENTATION.md) for the real build history. For using the engine rather than building it, the [MkDocs site](docs/) (`mkdocs serve`) has a Getting Started guide and the complete Python and Rust API reference.

## Building

Requires the Rust toolchain pinned in [`rust-toolchain.toml`](rust-toolchain.toml) (installed automatically by `rustup` on first use).

```bash
cargo build --workspace
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
```

### Crate layout

| Crate | Role | `unsafe` |
|---|---|---|
| [`tre-engine`](crates/tre-engine) | `Canvas` API, IR, sort/batch pipeline, shape registry, RHI trait definitions, input/focus/accessibility tagging | forbidden |
| [`tre-math`](crates/tre-math) | Vector/matrix math, SIMD via the `wide` crate | forbidden |
| [`tre-memory`](crates/tre-memory) | Ring arenas, transient pool, atlas lock-free concurrency primitives | permitted |
| [`tre-atlas`](crates/tre-atlas) | Guillotine bin-packing plus the dynamic atlas's multi-thread concurrency | forbidden |
| [`tre-text`](crates/tre-text) | Bidi/script shaping, glyph outlines, MSDF generation, line-breaking, caret placement, font fallback | forbidden |
| [`tre-svg`](crates/tre-svg) | SVG parsing, curve flattening, fill tessellation, keyframe morphing, SMIL parsing | forbidden |
| [`tre-tween`](crates/tre-tween) | Easing curves, generic interpolation, a real damped spring | forbidden |
| [`tre-animation`](crates/tre-animation) | The animation sequencer (`Timeline`) built on `tre-tween` | forbidden |
| [`tre-a11y`](crates/tre-a11y) | Publishes tagged accessibility nodes to the real Linux AT-SPI2 bus | forbidden |
| [`tre-platform`](crates/tre-platform) | Native windowing (`winit`), clipboard, file dialogs, system tray; Linux only for now | forbidden |
| [`tre-rhi-vulkan`](crates/tre-rhi-vulkan) | Vulkan 1.2+ backend (via `ash`) — the one real, complete RHI backend | permitted |
| [`tre-rhi-dx12`](crates/tre-rhi-dx12) | DirectX 12 backend (via `windows`), Windows-only — placeholder | permitted |
| [`tre-rhi-metal`](crates/tre-rhi-metal) | Metal 2.4+ backend (via `objc2-metal`), macOS-only — placeholder | permitted |
| [`tre-ffi`](crates/tre-ffi) | The engine's C-ABI surface (`cdylib`/`staticlib`) for every language other than Python — real first slice as of Phase 10 Step 10.3 | permitted |
| [`tre-python`](crates/tre-python) | Direct PyO3 bindings over `tre-engine`'s native Rust API — the project's real, privileged binding | permitted (one call) |

See TECHNICAL.md Section 9.1 for the full `unsafe` policy this table reflects.

## Key characteristics

- **Zero-allocation steady state** during the render loop, mechanically enforced in CI.
- **Single-digit draw calls per frame** via 64-bit sort-key batching and dynamic index stitching.
- **Rust core, a privileged direct-PyO3 Python binding, and a separate real C-ABI boundary (`tre-ffi`) for every other language.**
- **Vulkan 1.2+ is the one real, complete RHI backend** (real GLSL shaders compiled to SPIR-V via `shaderc`/`glslc`); DirectX 12 and Metal 2.4+ remain real, disclosed placeholders, not yet implemented.
- **MSDF typography**, analytical SDF rounded rectangles, and native animated SVG rendering.
- **Explicit, tested failure modes** for device loss, atlas exhaustion, malformed input, and resource starvation — no undocumented happy-path-only assumptions.
