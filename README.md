# Tesserae Render Engine (TRE)

A low-overhead, hardware-accelerated 2D rendering engine designed as a bridge between high-level UI frameworks and low-level graphics APIs (Vulkan, DirectX 12, Metal, WebGPU).

## Status

This repository currently contains the engine's design and planning documentation only — no source code yet.

## Overview

- Implemented in **Rust**, exposed to UI frameworks through a stable, language-agnostic C-ABI.
- The project's own high-level UI framework is being built in **Python** as the reference integration — any other language capable of calling a C ABI can bind to the engine on equal footing.
- Targets ultra-low-latency desktop UI rendering: up to 240 Hz frame delivery, a zero-allocation steady state, vector path rendering, dynamic typography (MSDF), animated SVG, HDR/wide-gamut color, multi-window support, and headless/CI rendering.

## Documentation

| Document | Contents |
|---|---|
| [DESIGN.md](documentation/DESIGN.md) | Design philosophy, core principles, failure modes, target applications, subsystem overviews |
| [TECHNICAL.md](documentation/TECHNICAL.md) | Performance budgets, hardware/platform requirements, toolchain, FFI/Python binding spec |
| [ARCHITECTURE.md](documentation/ARCHITECTURE.md) | Subsystem decomposition, core data structures, the 64-bit sort key, RHI trait interfaces |
| [IMPLEMENTATION.md](documentation/IMPLEMENTATION.md) | Phased implementation plan, from a walking skeleton through testing and Python bindings |
| [REVIEW.md](documentation/REVIEW.md) | Running record of documentation reviews, findings, and engineering decisions |

Start with [DESIGN.md](documentation/DESIGN.md) for the *why*, [ARCHITECTURE.md](documentation/ARCHITECTURE.md) for the *how*, and [IMPLEMENTATION.md](documentation/IMPLEMENTATION.md) for the build plan.

## Key characteristics

- **Zero-allocation steady state** during the render loop, mechanically enforced in CI.
- **Single-digit draw calls per frame** via 64-bit sort-key batching and dynamic index stitching.
- **Rust core, Python UI framework, language-agnostic C-ABI boundary.**
- **Vulkan 1.2+, DirectX 12, Metal 2.4+** backends compiled from one shared HLSL shader source.
- **MSDF typography**, analytical SDF rounded rectangles, and native animated SVG rendering.
- **Explicit, tested failure modes** for device loss, atlas exhaustion, malformed input, and resource starvation — no undocumented happy-path-only assumptions.
