# Errors & FFI

## `EngineError`

`tre-engine`'s one shared error type for recoverable engine failures. Every fallible engine operation returns `Result<T, EngineError>`; panics are reserved for programmer errors (a stale handle, an unbalanced `save`/`restore`, an unregistered pipeline id), never for these expected failure modes.

```rust
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EngineError {
    #[error("GPU device lost (removal, driver TDR, or a stale swapchain)")]
    DeviceLost,
    #[error("swapchain is out of date and must be recreated")]
    SwapchainOutOfDate,
    #[error("graphics pipeline creation failed")]
    PipelineCreationFailed,
    #[error("texture pixel data doesn't match width/height/format")]
    InvalidTextureData,
    #[error("bindless texture array has no free slots left")]
    BindlessArrayExhausted,
    #[error("transient render target pool's VRAM budget exceeded")]
    TransientPoolBudgetExceeded,
    #[error("shader compilation failed: {0}")]
    ShaderCompilationFailed(String),
    #[error("this swapchain has no CPU-visible readback path (not a headless swapchain)")]
    PixelReadbackUnsupported,
    #[error("timed out waiting for the next swapchain image")]
    AcquireTimedOut,
}
```

`Display`/`std::error::Error` come from [`thiserror`](https://docs.rs/thiserror) via the per-variant `#[error("...")]` messages above, rather than a hand-written impl.

| Variant | Raised by | Meaning |
|---|---|---|
| `DeviceLost` | `RhiDevice::begin_frame`/`submit_and_present` | GPU device removal, driver TDR, or a stale swapchain. |
| `SwapchainOutOfDate` | `RhiDevice::begin_frame`/`submit_and_present`, `RhiSwapchain::acquire_next_image`/`present` | The swapchain no longer matches the window (e.g. after a resize) and must be recreated. |
| `PipelineCreationFailed` | pipeline construction | A graphics pipeline failed to create. |
| `InvalidTextureData` | `RhiDevice::create_texture` | `pixels.len()` doesn't match `width * height * bytes_per_pixel(format)`, or `width`/`height` is zero. Caught before any GPU call -- no out-of-bounds read into `pixels` or its staging buffer can occur. |
| `BindlessArrayExhausted` | `RhiDevice::create_texture`/`register_bindless` | The persistent bindless texture array has no free slots left. Recoverable in principle (a caller can release textures and retry), though no eviction policy exists yet. |
| `TransientPoolBudgetExceeded` | `RhiDevice::acquire_transient_target` | A genuinely novel size would need cold-allocating while the transient pool's idle free bytes are already at or past the dynamic-VRAM budget. A reuse of an already-pooled size never fails this way. |
| `ShaderCompilationFailed(String)` | custom shader compilation | Real GLSL fragment-shader source failed to compile to SPIR-V via `shaderc` -- carries `shaderc`'s own real compiler diagnostic (line numbers, the exact GLSL error), since a caller authoring their own shader source genuinely needs to see *why* it failed. |
| `PixelReadbackUnsupported` | `RhiSwapchain::read_pixels_bgra8` | Called on a swapchain with no CPU-visible staging buffer -- a real windowed swapchain presents straight to the surface and was never given one, unlike a headless swapchain's own manually allocated staging buffer (Architecture review: RHI trait-object generalization, REVIEW.md finding #216). |
| `AcquireTimedOut` | `RhiSwapchain::acquire_next_image_with_timeout`/`RhiDevice::begin_frame_with_options` | The bounded wait for the next swapchain image elapsed before one became available (REVIEW.md finding #235). Recoverable by simply skipping this tick's render and retrying next iteration; never returned by the plain, unbounded `acquire_next_image`/`begin_frame`. |

`InvalidTextureData` and `BindlessArrayExhausted` both replaced what used to be unconditional panics -- both are real caller-triggerable conditions (malformed pixel data, a genuinely exhausted bindless array), not programmer errors.

## `ScissorRect`

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct ScissorRect { pub x: i32, pub y: i32, pub width: u32, pub height: u32 }
```

A clip rectangle in the coordinate space `RenderingCanvas::push_clip`/scissor operations use -- referenced but never concretely defined by the architecture sketch; defined here. "No clip, full window" is represented internally by a sentinel `{ x: 0, y: 0, width: u32::MAX, height: u32::MAX }`, reused across every call site that needs it rather than each constructing an equivalent literal.

## `tre-ffi`

The engine's public C-ABI surface for every language *other* than Python: `#[repr(C)]` opaque handles and `extern "C"` functions -- the **only** crate in the workspace whose items are exported as public symbols in the shipped `cdylib`/`staticlib`; every other crate is linked in but exports nothing of its own. One of the four crates permitted to contain `unsafe` (alongside `tre-rhi-vulkan`/`tre-rhi-dx12`/`tre-rhi-metal`, see [RHI Trait & Backends](rhi-and-backends.md)), used here for raw handle/pointer conversion and manual buffer-ownership transfer across the C-ABI boundary.

```rust
#![deny(unsafe_op_in_unsafe_fn)]
```

`tre-python`'s PyO3 bindings remain the engine's privileged, first-party binding (see the [Python API](../python-api/index.md), and [Platform](platform.md) for why Python bypasses this boundary entirely) -- `tre-ffi` is the correct, real integration point for C, C++, or any other language that calls a C ABI.

### A real, deliberately bounded first slice

Mirrors `tre-python`'s own historical first slice exactly: headless rendering plus `Rectangle`/`Circle`/`Polygon`/`Path`, **solid fill only** -- no `Text`, no gradients/textures, no windowed rendering yet. Every opaque handle is a boxed Rust value behind a `#[repr(transparent)]` pointer wrapper; every fallible function returns a `TreErrorCode` result code with the success value carried through an out-parameter, never a Rust `Result`/`Option`/`enum`-with-data crossing the boundary directly.

```rust
#[repr(C)]
pub enum TreErrorCode {
    Success = 0,
    DeviceLost, SwapchainOutOfDate, PipelineCreationFailed,
    InvalidTextureData, BindlessArrayExhausted, TransientPoolBudgetExceeded,
    ShaderCompilationFailed,
    PixelReadbackUnsupported,
    InvalidArgument,  // FFI-only: this crate's own insert-time validation failed
    PanicCaught,      // FFI-only: an unwinding panic was caught at the boundary
    AcquireTimedOut,  // appended last, after the two FFI-only variants, so it doesn't shift their discriminants
}
```

A 1:1 shadow of `EngineError` (see above) plus two genuinely FFI-only additions: `InvalidArgument` (`tre_engine::ShapeRegistry::insert` itself never fails, so this crate validates dimensions before calling it -- mirroring `tre-python`'s own established validation-at-insert-time convention) and `PanicCaught` (what every exported function's shared `ffi_guard` helper converts an unwinding Rust panic into, per TECHNICAL.md Section 9.4.1's "no panic may cross the boundary" rule). `AcquireTimedOut` is appended last (not alongside the other `EngineError`-mirrored variants) specifically so adding it didn't shift the two FFI-only variants' numeric discriminants.

The real, C-facing surface this slice exposes (as a caller including `tre_ffi.h` sees it -- opaque `void*`-shaped handles, plain C declarations):

```c
typedef void *TreShapeRegistry;
typedef void *TreShapeId;
typedef void *TreHeadlessRenderer;

TreShapeRegistry tre_shape_registry_new(void);
void             tre_shape_registry_free(TreShapeRegistry);
size_t           tre_shape_registry_len(TreShapeRegistry);
TreErrorCode     tre_shape_registry_insert_rectangle(TreShapeRegistry, float x, float y, float w, float h, uint32_t rgba, TreShapeId *out_id);
TreErrorCode     tre_shape_registry_insert_circle(TreShapeRegistry, float x, float y, float rx, float ry, uint32_t rgba, TreShapeId *out_id);
TreErrorCode     tre_shape_registry_insert_polygon(TreShapeRegistry, float x, float y, uint32_t sides, float radius, uint32_t rgba, TreShapeId *out_id);
TreErrorCode     tre_shape_registry_insert_path(TreShapeRegistry, float x, float y, const TrePathCommand *commands, size_t count, uint32_t rgba, TreShapeId *out_id);
bool             tre_shape_registry_remove(TreShapeRegistry, TreShapeId);  // consumes + frees the id
void             tre_shape_id_free(TreShapeId);  // for an id never removed

TreErrorCode     tre_headless_renderer_new(uint32_t width, uint32_t height, TreHeadlessRenderer *out);
void             tre_headless_renderer_free(TreHeadlessRenderer);
TreErrorCode     tre_headless_renderer_render(TreHeadlessRenderer, TreShapeRegistry, TreFrameBuffer *out);

uint32_t         tre_rgba8(uint8_t r, uint8_t g, uint8_t b, uint8_t a);
```

`TreShapeId` is deliberately boxed (heap-allocated) like every other handle, even though the real `ShapeId` is a tiny `Copy` value -- IMPLEMENTATION.md's own framing is explicit: "an opaque handle plus getter/setter functions, never raw struct-layout access across the boundary," and `ShapeId`'s fields are private in `tre-engine` with no public accessor to mirror honestly anyway. `TreHeadlessRenderer` is a real renderer built directly against `tre-rhi-vulkan`/`tre-platform` with no PyO3 involved -- it mirrors `tre-python`'s own `HeadlessRenderer` construction sequence (a never-shown probe window, `VulkanDevice::new`, `HeadlessSwapchain`, `register_shape_pipelines`, a dynamic ring buffer) exactly, including copying its real struct-field-order Drop-safety fix (`device` must be declared, and therefore dropped, last).

```rust
#[repr(C)]
pub struct TreFrameBuffer { pub data: *const u8, pub len: usize, pub capacity: usize }
```

```c
void tre_frame_buffer_free(TreFrameBuffer buffer);
```

The pointer+length shadow of the raw BGRA8 pixel `Vec<u8>` a render call hands back -- freed by `tre_frame_buffer_free`, never by the caller's own allocator. `capacity` is carried alongside `len` (distinct whenever the two differ) purely so the free function can reconstruct the exact `Vec<u8>` that was leaked; a C caller has no use for it beyond passing the struct back unchanged.

`TrePathCommand` is the one `#[repr(C)]` tagged-union shadow this slice needs (`tre_engine::shapes::PathCommand` is a Rust `enum`-with-data, which never crosses the boundary directly): a `kind` tag plus up to three `(x, y)` point pairs, unused fields ignored per variant.

### The generated header

`build.rs` runs [`cbindgen`](https://github.com/mozilla/cbindgen) against this crate's own `extern "C"` items on every build, writing `include/tre_ffi.h` -- checked into the repository (so a C/C++ consumer has it without running a Rust build first) but always regenerated, never hand-edited, so it can never drift from the real signatures.

### A real, dedicated non-Python test harness

`tests/c/harness.c` is a real C program: it builds a registry, inserts a red `Rectangle`, deliberately triggers `TRE_ERROR_CODE_INVALID_ARGUMENT` with a negative width, renders headlessly, and asserts *exact* BGRA8 pixel bytes at the rectangle's own center and at a background pixel outside it -- not just "the call didn't crash." `tests/c_harness.rs` compiles and links it against this crate's own just-built `cdylib`, then runs it and checks its exit code, wiring the whole thing into `cargo test` directly (TECHNICAL.md Section 9.4.1's literal "`cargo test`'s FFI test target" requirement).

Two real, found-and-fixed issues from actually building this, not assumed:

- The [`cc`](https://docs.rs/cc) crate's `Build`/`get_compiler()` API only works inside a `build.rs` -- it reads Cargo-injected environment variables (`OPT_LEVEL`, `TARGET`, `HOST`, ...) that don't exist in a plain `cargo test` binary. The harness invokes the system C compiler directly via `std::process::Command` instead (respecting `$CC`).
- Linking the harness against `libtre_ffi.a` (the `staticlib`) left dozens of undefined GLib/GDK/GIO symbols unresolved -- `tre-platform`'s real system-library dependencies (`tray-icon`/`rfd`) are only linked in via `cargo:rustc-link-lib` directives Cargo applies when producing a *final* linked artifact, not when a foreign `cc` invocation links the raw archive directly. The harness links against `libtre_ffi.so` (the `cdylib`) instead, which is already fully self-contained; both build targets are still asserted to exist.
- Locating the crate's own build output from inside a plain `#[test]` (Cargo gives a `build.rs` its own `OUT_DIR`, but nothing equivalent exists for an ordinary test binary) needed more than one strategy: a `current_exe()`-ancestor-walking trick that worked locally produced the wrong directory on GitHub Actions' own runner (a real difference found only via an actual CI round-trip) -- fixed by trying the workspace's own fixed, `CARGO_MANIFEST_DIR`-relative location first, falling back to a bounded upward walk that verifies the real artifact is present before accepting a candidate directory.
