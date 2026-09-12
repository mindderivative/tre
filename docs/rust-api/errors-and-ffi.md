# Errors & FFI

## `EngineError`

`tre-engine`'s one shared error type for recoverable engine failures. Every fallible engine operation returns `Result<T, EngineError>`; panics are reserved for programmer errors (a stale handle, an unbalanced `save`/`restore`, an unregistered pipeline id), never for these expected failure modes.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    DeviceLost,
    SwapchainOutOfDate,
    PipelineCreationFailed,
    InvalidTextureData,
    BindlessArrayExhausted,
    TransientPoolBudgetExceeded,
    ShaderCompilationFailed(String),
}
// impl std::fmt::Display, impl std::error::Error
```

| Variant | Raised by | Meaning |
|---|---|---|
| `DeviceLost` | `RhiDevice::begin_frame`/`submit_and_present` | GPU device removal, driver TDR, or a stale swapchain. |
| `SwapchainOutOfDate` | `RhiDevice::begin_frame`/`submit_and_present`, `RhiSwapchain::acquire_next_image`/`present` | The swapchain no longer matches the window (e.g. after a resize) and must be recreated. |
| `PipelineCreationFailed` | pipeline construction | A graphics pipeline failed to create. |
| `InvalidTextureData` | `RhiDevice::create_texture` | `pixels.len()` doesn't match `width * height * bytes_per_pixel(format)`, or `width`/`height` is zero. Caught before any GPU call -- no out-of-bounds read into `pixels` or its staging buffer can occur. |
| `BindlessArrayExhausted` | `RhiDevice::create_texture`/`register_bindless` | The persistent bindless texture array has no free slots left. Recoverable in principle (a caller can release textures and retry), though no eviction policy exists yet. |
| `TransientPoolBudgetExceeded` | `RhiDevice::acquire_transient_target` | A genuinely novel size would need cold-allocating while the transient pool's idle free bytes are already at or past the dynamic-VRAM budget. A reuse of an already-pooled size never fails this way. |
| `ShaderCompilationFailed(String)` | custom shader compilation | Real GLSL fragment-shader source failed to compile to SPIR-V via `shaderc` -- carries `shaderc`'s own real compiler diagnostic (line numbers, the exact GLSL error), since a caller authoring their own shader source genuinely needs to see *why* it failed. |

`InvalidTextureData` and `BindlessArrayExhausted` both replaced what used to be unconditional panics -- both are real caller-triggerable conditions (malformed pixel data, a genuinely exhausted bindless array), not programmer errors.

## `ScissorRect`

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct ScissorRect { pub x: i32, pub y: i32, pub width: u32, pub height: u32 }
```

A clip rectangle in the coordinate space `RenderingCanvas::push_clip`/scissor operations use -- referenced but never concretely defined by the architecture sketch; defined here. "No clip, full window" is represented internally by a sentinel `{ x: 0, y: 0, width: u32::MAX, height: u32::MAX }`, reused across every call site that needs it rather than each constructing an equivalent literal.

## `tre-ffi`

The engine's planned entire public C-ABI surface: `#[repr(C)]` opaque handles and `extern "C"` functions -- the **only** crate in the workspace whose items would be exported as public symbols in a shipped `cdylib`/`staticlib`; every other crate is linked in but exports nothing of its own.

```rust
#![deny(unsafe_op_in_unsafe_fn)]
```

**Currently a placeholder** -- the crate exists (13 lines, module doc only) but has no `extern "C"` functions yet. It's one of the three crates permitted to contain `unsafe` (alongside `tre-rhi-dx12`/`tre-rhi-metal`, see [RHI Trait & Backends](rhi-and-backends.md)), reserved for raw handle/pointer conversion and manual buffer-ownership transfer across the C-ABI boundary once built. Its own doc comment already states the load-bearing safety rule for when it is: every exported `extern "C"` function must wrap its body in `std::panic::catch_unwind` -- panics must never unwind past an `extern "C"` boundary, which is undefined behavior.

Today, `tre-python`'s PyO3 bindings are the engine's only real external-language boundary (see the [Python API](../python-api/index.md)) -- `tre-ffi` is where a plain C ABI would live if/when one is built, entirely independent of the Python bindings.
