//! `TreHeadlessRenderer` -- a real, working headless renderer built
//! directly against `tre-engine`'s `RhiDevice` trait and the
//! `tre-rhi-vulkan` backend, with no PyO3 involved at all
//! (IMPLEMENTATION.md Phase 10 Step 10.3). Mirrors `tre-python`'s own
//! `HeadlessRenderer` construction sequence exactly (verified against
//! `crates/tre-python/src/renderer.rs` before writing this) -- every call
//! it makes below `tre-python`'s own PyO3 wrapper layer is already plain,
//! reusable, non-PyO3 API.
//!
//! Solid-fill-only, no `Text` shapes (this crate's own bounded first
//! slice -- see the plan this step shipped against): `TextFlattenContext`
//! stays `None` at `flatten_into`, matching every one of the ~40
//! pre-existing Rust examples that already do exactly this, so no
//! `tre-python`-style `TextAtlas`/`AtlasOwner` machinery is needed here.

use std::os::raw::c_void;

use raw_window_handle::HasDisplayHandle;
use tre_engine::{
    execute_frame, submit_frame, BufferBinding, PipelineRegistry, RenderingCanvas, RhiDevice,
    RhiDynamicRingBuffer, ScissorRect, ShapeRegistry,
};
use tre_platform::PlatformConnection;
use tre_rhi_vulkan::{register_shape_pipelines, HeadlessSwapchain, VulkanDevice, HEADLESS_FORMAT};

use crate::error::TreErrorCode;
use crate::ffi_guard;
use crate::frame_buffer::TreFrameBuffer;
use crate::handle;

/// Shared by vertex and index writes every `render()` call -- matches
/// `tre-python`'s own `RING_BUFFER_CAPACITY` exactly (512 KiB), the
/// same real, already-proven-sufficient budget for this bounded first
/// slice's own solid-fill-only shape set.
const RING_BUFFER_CAPACITY: usize = 512 * 1024;

/// Field declaration order is real teardown order here (Rust drops
/// struct fields top-to-bottom, the OPPOSITE of local variables' own
/// reverse-declaration-order drop). `pipelines`/`swapchain`/
/// `ring_buffer` each hold live Vulkan handles built from `device`, so
/// `device` must be declared, and therefore dropped, LAST -- exactly the
/// real bug `tre-python`'s own `PyHeadlessRenderer` hit (a use-after-free
/// that segfaulted at Python interpreter shutdown when `device` was
/// declared first) before being fixed there. Copied here deliberately
/// rather than rediscovering the same bug independently.
struct Renderer {
    ring_buffer: Box<dyn RhiDynamicRingBuffer>,
    pipelines: PipelineRegistry,
    swapchain: HeadlessSwapchain,
    device: VulkanDevice,
    width: u32,
    height: u32,
}

#[repr(transparent)]
pub struct TreHeadlessRenderer(*mut c_void);

impl TreHeadlessRenderer {
    pub(crate) const fn null() -> Self {
        Self(std::ptr::null_mut())
    }
}

fn build_renderer(width: u32, height: u32) -> Result<Renderer, TreErrorCode> {
    // The real, never-shown 1x1 probe window -- `VulkanDevice::new` needs
    // a live display-server connection and a real window handle to pick
    // a physical device / present-capable queue family, even though
    // nothing built from it is ever shown. Mapped to `DeviceLost`: every
    // failure at this stage means rendering setup itself could not reach
    // a real device, the closest honest `TreErrorCode` match (`tre_engine
    // ::EngineError` has no "platform connection failed" variant of its
    // own to shadow instead).
    let mut probe = PlatformConnection::new().map_err(|_| TreErrorCode::DeviceLost)?;
    let probe_window = probe
        .create_window("tre-ffi headless probe (never shown)", 1, 1)
        .map_err(|_| TreErrorCode::DeviceLost)?;
    let display_handle = probe
        .display_handle()
        .map_err(|_| TreErrorCode::DeviceLost)?
        .as_raw();
    let window_handle = probe
        .window_handle(probe_window)
        .map_err(|_| TreErrorCode::DeviceLost)?
        .as_raw();

    let (device, surface_loader, surface) =
        VulkanDevice::new(display_handle, window_handle).map_err(TreErrorCode::from)?;
    // SAFETY: `surface` was just created by `VulkanDevice::new` against
    // `surface_loader`, both still valid here; nothing else references
    // `surface` -- this renderer is headless and never presents through
    // a real swapchain built on it, matching every other real call
    // site's own identical probe-surface teardown (e.g.
    // `tre-python`'s own `PyHeadlessRenderer::new`).
    unsafe {
        surface_loader.destroy_surface(surface, None);
    }

    let swapchain = HeadlessSwapchain::new(&device, width, height).map_err(TreErrorCode::from)?;
    let mut pipelines = PipelineRegistry::new();
    register_shape_pipelines(&device, &mut pipelines, HEADLESS_FORMAT)
        .map_err(TreErrorCode::from)?;
    let ring_buffer = device.create_dynamic_ring_buffer(RING_BUFFER_CAPACITY);

    Ok(Renderer {
        ring_buffer,
        pipelines,
        swapchain,
        device,
        width,
        height,
    })
}

/// Creates a headless (zero-window) renderer of `width x height`
/// pixels. Writes the new renderer's handle to `*out`.
///
/// # Errors
/// Returns a real `TreErrorCode` (never panics across the boundary) if
/// the display-server connection, the probe window, or the underlying
/// Vulkan device/swapchain/pipeline setup fails -- see
/// `docs/getting-started.md`'s own disclosed caveat, inherited unchanged
/// here: this cannot render even headlessly in a true no-display
/// environment (a bare container with no compositor at all needs a
/// software Vulkan implementation plus a virtual display, the same way
/// this project's own CI does it).
///
/// # Safety
/// `out` must be valid for one write of a [`TreHeadlessRenderer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tre_headless_renderer_new(
    width: u32,
    height: u32,
    out: *mut TreHeadlessRenderer,
) -> TreErrorCode {
    ffi_guard(TreErrorCode::PanicCaught, move || {
        if out.is_null() {
            return TreErrorCode::InvalidArgument;
        }
        match build_renderer(width, height) {
            Ok(renderer) => {
                // SAFETY: caller's contract guarantees `out` is valid for
                // one write of a `TreHeadlessRenderer`.
                unsafe { out.write(TreHeadlessRenderer(handle::into_raw(renderer))) };
                TreErrorCode::Success
            }
            Err(code) => {
                // SAFETY: same as above -- a caller that unconditionally
                // reads `*out` regardless of the returned code still
                // gets a well-defined null handle, never uninitialized
                // memory.
                unsafe { out.write(TreHeadlessRenderer::null()) };
                code
            }
        }
    })
}

/// Destroys a renderer created by [`tre_headless_renderer_new`]. Safe to
/// call with a null/already-freed handle (a no-op).
///
/// # Safety
/// `renderer` must be null, or a still-live value
/// [`tre_headless_renderer_new`] wrote that has not already been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tre_headless_renderer_free(renderer: TreHeadlessRenderer) {
    // SAFETY: forwarded from this function's own `# Safety` contract.
    unsafe { handle::from_raw::<Renderer>(renderer.0) }
}

/// Renders every shape currently in `registry` and writes the resulting
/// raw BGRA8 pixel buffer (`width * height * 4` bytes) to `*out`. Always
/// flattens the entire registry (`ShapeRegistry::mark_all_dirty` runs
/// internally on every call) since there is no cross-language dirty-flag
/// protocol a caller on this side of the boundary could observe --
/// matching `tre-python`'s own identical, already-fixed-once convention
/// (REVIEW.md finding #196).
///
/// # Errors
/// Returns a real `TreErrorCode` on ring-buffer starvation or a GPU
/// submission/device-loss failure. `*out` is left at
/// [`TreFrameBuffer::empty`] in that case, safe to pass to
/// [`tre_frame_buffer_free`](crate::tre_frame_buffer_free) unconditionally.
///
/// # Safety
/// `renderer` and `registry` must each be a still-live value their
/// respective constructors produced. `out` must be valid for one write
/// of a [`TreFrameBuffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tre_headless_renderer_render(
    renderer: TreHeadlessRenderer,
    registry: crate::registry::TreShapeRegistry,
    out: *mut TreFrameBuffer,
) -> TreErrorCode {
    ffi_guard(TreErrorCode::PanicCaught, move || {
        if out.is_null() {
            return TreErrorCode::InvalidArgument;
        }
        let result = render(renderer, registry);
        let (code, buffer) = match result {
            Ok(bytes) => (TreErrorCode::Success, TreFrameBuffer::from_vec(bytes)),
            Err(code) => (code, TreFrameBuffer::empty()),
        };
        // SAFETY: caller's contract guarantees `out` is valid for one
        // write of a `TreFrameBuffer`.
        unsafe { out.write(buffer) };
        code
    })
}

fn render(
    renderer: TreHeadlessRenderer,
    registry: crate::registry::TreShapeRegistry,
) -> Result<Vec<u8>, TreErrorCode> {
    // SAFETY: both handles come from this function's own caller, which
    // documents the same still-live-value contract this relies on.
    let renderer =
        unsafe { handle::as_mut::<Renderer>(renderer.0) }.ok_or(TreErrorCode::InvalidArgument)?;
    let registry = unsafe { handle::as_mut::<ShapeRegistry>(registry.0) }
        .ok_or(TreErrorCode::InvalidArgument)?;

    registry.mark_all_dirty();
    let mut canvas = RenderingCanvas::new();
    registry.flatten_into(&mut canvas, &renderer.device, None);
    let frame = canvas.flatten();

    let vertex_bytes: &[u8] = bytemuck::cast_slice(&frame.vertices);
    let index_bytes: &[u8] = bytemuck::cast_slice(&frame.indices);
    let vertex_offset = renderer
        .ring_buffer
        .write(vertex_bytes)
        .ok_or(TreErrorCode::TransientPoolBudgetExceeded)?;
    let index_offset = renderer
        .ring_buffer
        .write(index_bytes)
        .ok_or(TreErrorCode::TransientPoolBudgetExceeded)?;
    let full_window = ScissorRect {
        x: 0,
        y: 0,
        width: renderer.width,
        height: renderer.height,
    };

    submit_frame(&renderer.device, &renderer.swapchain, |cmd_buffer| {
        execute_frame(
            &frame,
            &renderer.pipelines,
            BufferBinding {
                buffer: &*renderer.ring_buffer,
                offset: vertex_offset,
            },
            BufferBinding {
                buffer: &*renderer.ring_buffer,
                offset: index_offset,
            },
            &full_window,
            &renderer.device,
            cmd_buffer,
        );
    })
    .map_err(TreErrorCode::from)?;

    renderer
        .swapchain
        .read_pixels_bgra8()
        .map_err(TreErrorCode::from)
}
