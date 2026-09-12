# Rust API

This section documents `tre`'s own native Rust engine -- the workspace's fifteen crates, their public types, functions, and traits -- as opposed to the [Python API](../python-api/index.md), which documents `tre-python`'s PyO3 binding layer on top of it.

**Read this section if** you're embedding `tre-engine` directly from Rust, writing a new RHI backend (DirectX 12, Metal, or something else entirely), contributing to the engine itself, or you just want to know exactly what a `tre-python` call does underneath. **Read the [Python API](../python-api/index.md) instead if** you're building a UI on top of `tre` from Python, which is how almost everyone should approach this project (see [Getting Started](../getting-started.md)) -- nearly every type here has a thin, close-to-1:1 Python wrapper documented there.

This section is a **reference**: exact signatures, real documented bugs and their fixes, disclosed scope limits, and the "why" behind non-obvious design choices, all pulled directly from the engine's own source and doc comments. For how the pieces fit together conceptually -- the pipeline overview, the intermediate representation's design rationale, the 8-stage frame loop -- see [Architecture](../architecture.md), which this section complements rather than duplicates.

## Crate map

| Crate | Covers | Documented on |
|---|---|---|
| `tre-math` | 2D affine transforms, SIMD-batched compose/lerp, HDR tone-mapping, exponential decay | [Math & Memory](math-and-memory.md) |
| `tre-memory` | Lock-free ring buffers (SPSC/MPSC), the scatter arena, the SWMR publish table, the debug zero-allocation guard | [Math & Memory](math-and-memory.md) |
| `tre-engine` | The Canvas API, intermediate representation, sort/batch pipeline, shape registry, RHI trait definitions, input events, focus, accessibility tagging | [Canvas & IR](canvas-and-ir.md), [Shapes](shapes.md), [RHI Trait & Backends](rhi-and-backends.md), [Input & Focus](input-and-focus.md), [Accessibility](accessibility.md), [Errors & FFI](errors-and-ffi.md) |
| `tre-rhi-vulkan` | The one real, complete RHI backend | [RHI Trait & Backends](rhi-and-backends.md) |
| `tre-rhi-dx12` | Windows RHI backend | Placeholder -- see [RHI Trait & Backends](rhi-and-backends.md) |
| `tre-rhi-metal` | macOS RHI backend | Placeholder -- see [RHI Trait & Backends](rhi-and-backends.md) |
| `tre-platform` | Native windowing (`winit`), clipboard, file dialogs, system tray | [Platform](platform.md) |
| `tre-a11y` | Publishing tagged accessibility nodes to the real Linux AT-SPI2 bus | [Accessibility](accessibility.md) |
| `tre-text` | Bidi/script shaping, glyph outlines, MSDF generation, line-breaking, caret placement, font fallback | [Text, SVG & Atlas](text-svg-atlas.md) |
| `tre-svg` | SVG parsing, curve flattening, fill tessellation, keyframe morphing, SMIL animation parsing | [Text, SVG & Atlas](text-svg-atlas.md) |
| `tre-atlas` | The shared dynamic texture atlas: bin-packing plus real multi-thread concurrency | [Text, SVG & Atlas](text-svg-atlas.md) |
| `tre-tween` | Easing curves, generic interpolation, a real damped spring | [Animation](animation.md) |
| `tre-animation` | The real animation sequencer (`Timeline`) built on `tre-tween` | [Animation](animation.md) |
| `tre-ffi` | The engine's planned C-ABI surface | Placeholder -- see [Errors & FFI](errors-and-ffi.md) |
| `tre-python` | PyO3 bindings over everything above | [Python API](../python-api/index.md) (its own section) |

## Conventions that hold across the whole engine

**`#![forbid(unsafe_code)]` almost everywhere.** Of the fifteen crates, only four are permitted to contain `unsafe` at all: `tre-memory` (lock-free ring buffers/arenas need it), `tre-ffi`/`tre-rhi-dx12`/`tre-rhi-metal` (raw C-ABI/DX12/Metal FFI), and `tre-rhi-vulkan` (raw Vulkan FFI via `ash`). Every other crate -- including `tre-engine` itself, the core of the whole system -- carries `#![forbid(unsafe_code)]` and means it literally: the attribute makes the crate fail to compile if it ever contains an `unsafe` block.

**Recoverable failures are `Result`, programmer errors are panics.** Every crate follows the same split: a condition a real caller can trigger through normal use (invalid pixel data, a full ring buffer, a stale handle passed to a lookup) returns a real `Err`/`None`; a condition that can only happen from a genuine programmer mistake (an unbalanced `save`/`restore`, referencing a `GradientId`/`FontId` this exact registry never issued, requesting a zero-capacity buffer) panics, usually with a message naming the exact invariant violated.

**Zero-allocation steady state, deliberately not absolute.** Per-frame hot paths (`ScatterArena`, the ring buffers, `FrameArena`'s scratch buffers) are built to allocate once and never again once warmed up -- verified in debug builds by a real custom `#[global_allocator]` (`tre_memory::DebugAllocGuard`) that panics on any allocation observed while a `RenderTickGuard` is active. Complex external subsystems (SVG's own `Vec<PathCommand>`, a `Path` shape's command list) are explicitly exempted -- the rule targets the per-frame render loop, not every allocation anywhere in the engine.

**Stable handles with generation counters, not raw indices.** `ShapeId`, and the atlas's own slot table, all pair an index with a generation counter so a stale handle (its slot was removed and reused) is detectable rather than silently resolving to a different, unrelated object.

**Real, cited external standards over invented rules.** Where a real specification already exists -- HTML's `tabindex` convention for tab order, UAX #14 for line-breaking, UAX #29 for word boundaries, the Vyukov MPMC ring-buffer design -- the engine implements that, cited by name in the relevant doc comment, rather than a bespoke scheme nobody outside the codebase would recognize.

**Documented bugs, not silent fixes.** A striking number of doc comments across this engine describe a real bug that was found (often via a real GPU, a real concurrent stress test, or a real fuzzer), how it was diagnosed, and why the current code is the fix -- not just what the code does now. Several of the most load-bearing examples are called out directly on the pages in this section (a GPU denormal-flush-to-zero bug in the GPU style-buffer encoding, an ABA slot-reuse race in the atlas's publish table, a caret hit-testing off-by-one in vertical line lookup, among others).
