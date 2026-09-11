//! A headless (zero-window) renderer (IMPLEMENTATION.md Phase 10 Step
//! 10.4 tasks 1-3): the real GPU round trip -- `ShapeRegistry::
//! flatten_into` -> vertex/index upload -> `execute_frame` -> readback --
//! wrapped as one Python method returning real pixel bytes.
//!
//! Headless, not windowed, deliberately: this project's own standing
//! correctness-oracle convention (every prior real render feature is
//! proven via `HeadlessSwapchain` before anything windowed) applies here
//! too, and a windowed Python renderer needs real event-loop integration
//! (polling `tre_platform::PlatformConnection` from Python's own run
//! loop) that is real, separate, disclosed follow-up work, not this
//! step's scope.
//!
//! Real, disclosed caveat inherited from `tre-rhi-vulkan` itself, not
//! introduced here: `VulkanDevice::new` requires a real display-server
//! connection to select a physical device and build a probe surface,
//! even though nothing it returns is ever shown -- there is no way to
//! render even headlessly without *some* live Wayland/X11 session
//! reachable, so this binding cannot be used in a true no-display
//! environment (a bare CI container with no compositor at all, for
//! example).

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use raw_window_handle::HasDisplayHandle;
use tre_engine::{
    execute_frame, submit_frame, BufferBinding, EngineError, FlattenedFrame, FrameArena,
    PipelineRegistry, RenderingCanvas, RhiDevice, RhiDynamicRingBuffer, ScissorRect,
    TextFlattenContext,
};
use tre_rhi_vulkan::{register_shape_pipelines, HeadlessSwapchain, VulkanDevice, HEADLESS_FORMAT};

use crate::canvas::PyCanvas;
use crate::error::engine_err;
use crate::shapes::PyShapeRegistry;
use crate::text_atlas::TextAtlas;
use crate::texture::{PyTexture, PyTextureFormat};

pub(crate) fn setup_err<E: std::fmt::Display>(e: E) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

/// Shared by vertex and index writes every `render()` call (REVIEW.md
/// #203/#204's own fix): comfortably above what `shape_registry_zero_
/// alloc_demo.rs`/`main_loop_demo.rs` already prove sufficient for a
/// real, mixed multi-shape scene (`64 * 1024`) -- widened here since a
/// real Python UI framework's scene is not bounded the way a fixed demo
/// scene is. Not tunable yet (a real, disclosed scope boundary): ring-
/// buffer starvation raises a clean `TreError` rather than growing
/// dynamically mid-frame (`RhiDynamicRingBuffer::write`'s own contract),
/// so a caller with a genuinely larger scene has no way to raise this
/// short of a future constructor parameter.
pub(crate) const RING_BUFFER_CAPACITY: usize = 512 * 1024;

/// `render_parallel`'s own `FrameArena` element capacities (Phase 12
/// Step 12.6) -- comfortably above `RING_BUFFER_CAPACITY`'s own real
/// per-frame scale (that constant is a *byte* budget for vertices+
/// indices combined; these are per-kind *element* counts, since
/// `FrameArena`'s three `ScatterArena`s are typed, not raw bytes). Not
/// tunable yet, the same real, disclosed scope boundary
/// `RING_BUFFER_CAPACITY`'s own doc comment already establishes:
/// `SubCanvas::stitch_into` reports `false` (mapped to a clean
/// `TreError`) rather than growing mid-frame, so a caller whose combined
/// parallel scene is genuinely larger has no way to raise this short of
/// a future constructor parameter.
pub(crate) const PARALLEL_ARENA_VERTEX_CAPACITY: usize = 65536;
pub(crate) const PARALLEL_ARENA_INDEX_CAPACITY: usize = 131072;
pub(crate) const PARALLEL_ARENA_COMMAND_CAPACITY: usize = 8192;
pub(crate) const PARALLEL_ARENA_ACCESSIBILITY_CAPACITY: usize = 1024;

/// The `py.detach`'d render closure's own error type -- kept local to
/// this module rather than widening `tre_engine::EngineError` itself,
/// since `RhiDynamicRingBuffer::write`'s `Option<u32>` starvation signal
/// is deliberately not yet part of that enum (its own doc comment:
/// real graceful-degradation policy is future work, REVIEW.md #142).
pub(crate) enum RenderError {
    Engine(EngineError),
    RingBufferStarved,
}

impl From<EngineError> for RenderError {
    fn from(e: EngineError) -> Self {
        Self::Engine(e)
    }
}

pub(crate) fn render_err(e: RenderError) -> PyErr {
    match e {
        RenderError::Engine(e) => engine_err(e),
        RenderError::RingBufferStarved => crate::error::TreError::new_err(format!(
            "scene too large for this frame's ring-buffer capacity ({RING_BUFFER_CAPACITY} \
             bytes, shared between vertex and index data) -- reduce the scene's shape count, \
             or split it across multiple render() calls"
        )),
    }
}

/// Real Vulkan hardware limits on the smallest reachable machine still
/// rejects `width`/`height == 0` (`VUID-VkImageCreateInfo-extent-00944`)
/// -- undefined behavior on a release build with no validation layer,
/// not a clean error -- and a caller-supplied dimension has no other
/// bound before reaching a real GPU image allocation. `MAX_DIMENSION` is
/// a conservative cap (found via this project's own review process,
/// REVIEW.md #196-198): comfortably above any real UI use case, safely
/// below the `maxImageDimension2D` every target GPU class supports, and
/// small enough that a caller who mistypes zeros doesn't get to request
/// a multi-gigabyte allocation before this constructor ever calls into
/// Vulkan.
const MAX_DIMENSION: u32 = 8192;

pub(crate) fn validate_dimensions(width: u32, height: u32) -> PyResult<()> {
    if width == 0 || height == 0 {
        return Err(PyValueError::new_err(format!(
            "HeadlessRenderer width/height must be non-zero, got {width}x{height}"
        )));
    }
    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(PyValueError::new_err(format!(
            "HeadlessRenderer width/height must be <= {MAX_DIMENSION}, got {width}x{height}"
        )));
    }
    Ok(())
}

/// Renders a [`crate::shapes::PyShapeRegistry`] scene headlessly and
/// returns the finished frame as real pixel bytes. Construct once, call
/// [`PyHeadlessRenderer::render`] as many times as needed.
#[pyclass(name = "HeadlessRenderer")]
pub struct PyHeadlessRenderer {
    // Field declaration order is real teardown order here (Rust drops
    // struct fields top-to-bottom, the OPPOSITE of local variables' own
    // reverse-declaration-order drop -- a real, easy-to-miss footgun
    // every RHI *example*'s own local-variable ordering sidesteps for
    // free). `pipelines` and `swapchain` each hold live Vulkan handles
    // built from `device`, so `device` must be declared, and therefore
    // dropped, LAST -- declaring it first (as this struct originally
    // did) destroyed the logical device while `swapchain`/`pipelines`
    // still held handles against it, a real use-after-free that
    // segfaulted at Python interpreter shutdown. `ring_buffer` joins
    // `pipelines`/`swapchain` here for the identical reason (REVIEW.md
    // #203/#204): it too holds a live buffer built from `device`.
    // `text_atlas` (Phase 12 Step 12.3) joins them for the identical
    // reason again: its own GPU texture is built from `device` too.
    ring_buffer: Box<dyn RhiDynamicRingBuffer>,
    text_atlas: TextAtlas,
    pipelines: PipelineRegistry,
    swapchain: HeadlessSwapchain,
    device: VulkanDevice,
    width: u32,
    height: u32,
}

#[pymethods]
impl PyHeadlessRenderer {
    /// # Errors
    /// Raises `ValueError` if `width`/`height` are zero or exceed
    /// [`MAX_DIMENSION`], `TreError` (a real `EngineError`), or
    /// `RuntimeError` (a display-server/window-setup failure -- see
    /// this module's own doc comment for why one is needed at all for a
    /// headless renderer).
    #[new]
    fn new(width: u32, height: u32) -> PyResult<Self> {
        validate_dimensions(width, height)?;
        let mut probe = tre_platform::PlatformConnection::new().map_err(setup_err)?;
        let probe_window = probe
            .create_window("tre-python headless probe (never shown)", 1, 1)
            .map_err(setup_err)?;
        let display_handle = probe.display_handle().map_err(setup_err)?.as_raw();
        let window_handle = probe
            .window_handle(probe_window)
            .map_err(setup_err)?
            .as_raw();

        let (device, surface_loader, surface) =
            VulkanDevice::new(display_handle, window_handle).map_err(engine_err)?;
        // SAFETY: `surface` was just created by `VulkanDevice::new` against
        // `surface_loader`, both still valid here; nothing else references
        // `surface` (this renderer is headless -- it never presents through
        // a real swapchain built on it), matching every other real
        // call site's own identical probe-surface teardown.
        unsafe {
            surface_loader.destroy_surface(surface, None);
        }

        let swapchain = HeadlessSwapchain::new(&device, width, height).map_err(engine_err)?;
        let mut pipelines = PipelineRegistry::new();
        register_shape_pipelines(&device, &mut pipelines, HEADLESS_FORMAT).map_err(engine_err)?;
        let ring_buffer = device.create_dynamic_ring_buffer(RING_BUFFER_CAPACITY);
        let text_atlas = TextAtlas::new(&device)?;

        Ok(Self {
            device,
            swapchain,
            pipelines,
            ring_buffer,
            text_atlas,
            width,
            height,
        })
    }

    #[getter]
    fn width(&self) -> u32 {
        self.width
    }

    #[getter]
    fn height(&self) -> u32 {
        self.height
    }

    /// Uploads `pixels` (tightly packed, `width * height` pixels in
    /// `format`) as a new, real GPU texture and registers it in the
    /// bindless array, returning a [`PyTexture`] usable as any shape's
    /// `fill_color` (Phase 12 Step 12.4). Mirrors `RhiDevice::
    /// create_texture` directly.
    ///
    /// # Errors
    /// Raises `TreError` if `pixels`' length doesn't match `width *
    /// height * bytes_per_pixel(format)`, `width`/`height` is zero, or
    /// the bindless array is exhausted.
    fn create_texture(
        &self,
        width: u32,
        height: u32,
        format: PyTextureFormat,
        pixels: Vec<u8>,
    ) -> PyResult<PyTexture> {
        let texture = self
            .device
            .create_texture(width, height, format.into(), &pixels)
            .map_err(engine_err)?;
        PyTexture::new(texture)
    }

    /// Flattens `registry`'s current shapes, renders them, and returns
    /// the finished frame as tightly-packed `BGRA8` bytes -- a real
    /// `bytes` object, itself a real buffer-protocol type Python (or
    /// `numpy.frombuffer`) can wrap with no further copy from here.
    ///
    /// Releases the GIL for the real GPU round trip (upload, submit,
    /// present, readback) -- IMPLEMENTATION.md Step 10.4 task 3 -- so
    /// other Python threads keep running while this one blocks on the
    /// GPU fence.
    ///
    /// Takes `&mut self`, not `&self` (found via this project's own
    /// review process, REVIEW.md #196-198): the device/swapchain/
    /// pipelines this method drives are single-buffered, single-frame-
    /// in-flight Vulkan state (one reused command buffer, one fence, one
    /// swapchain image) -- calling `render()` on the same renderer from
    /// two threads at once would race on that shared state. `&mut self`
    /// makes PyO3's own runtime borrow check enforce exclusive access:
    /// a second concurrent call raises a clean `PyBorrowMutError`
    /// instead of corrupting GPU command state.
    ///
    /// # Errors
    /// Raises `TreError` on any real, recoverable engine failure.
    fn render(
        &mut self,
        py: Python<'_>,
        registry: &Bound<'_, PyShapeRegistry>,
    ) -> PyResult<Py<PyBytes>> {
        let canvas = Bound::new(
            py,
            PyCanvas {
                inner: RenderingCanvas::new(),
            },
        )?;
        self.flatten_into(&canvas, registry)?;
        let frame = {
            let mut canvas = canvas.borrow_mut();
            std::mem::replace(&mut canvas.inner, RenderingCanvas::new()).flatten()
        };
        self.submit_and_read_bgra(py, &frame)
    }

    /// Flattens `registry`'s current shapes into `canvas` -- the real
    /// seam (Phase 12 Step 12.5) letting more than one registry, with
    /// `canvas.clip(...)`/`canvas.layer(...)` scopes interleaved between
    /// them, share one canvas before a single [`render_canvas`]
    /// (`PyHeadlessRenderer::render_canvas`) call submits it. `render`'s
    /// own single-registry convenience wrapper calls this internally
    /// with a fresh, throwaway canvas.
    ///
    /// Like `render`, always flattens `registry`'s *full* current state
    /// (`mark_all_dirty`), not an incremental delta -- see `render`'s own
    /// doc comment for why.
    ///
    /// # Errors
    /// Raises `TreError` if a due text-atlas texture refresh fails (see
    /// `crate::text_atlas::TextAtlas`'s own doc comment).
    fn flatten_into(
        &mut self,
        canvas: &Bound<'_, PyCanvas>,
        registry: &Bound<'_, PyShapeRegistry>,
    ) -> PyResult<()> {
        let mut reg = registry.borrow_mut();
        let mut canvas = canvas.borrow_mut();
        reg.inner.mark_all_dirty();
        let atlas_context = self.text_atlas.context(&self.device)?;
        let (inner, fonts) = reg.inner_and_fonts();
        let text_context = TextFlattenContext {
            fonts,
            atlas: &atlas_context,
        };
        inner.flatten_into(&mut canvas.inner, &self.device, Some(&text_context));
        Ok(())
    }

    /// Renders an already-assembled [`PyCanvas`] (built via one or more
    /// [`flatten_into`](Self::flatten_into) calls, optionally interleaved
    /// with `canvas.clip(...)`/`canvas.layer(...)` scopes) and returns
    /// the finished frame as `BGRA8` bytes -- the real "submit what I
    /// built" counterpart to `render`'s own single-registry convenience
    /// wrapper. Consumes `canvas`'s own recorded content (leaving it
    /// freshly empty, like a new `Canvas`) rather than the `Canvas`
    /// object itself, so the same Python `Canvas` can be reused next
    /// frame.
    ///
    /// # Errors
    /// Raises `TreError` on any real, recoverable engine failure.
    fn render_canvas(
        &mut self,
        py: Python<'_>,
        canvas: &Bound<'_, PyCanvas>,
    ) -> PyResult<Py<PyBytes>> {
        let frame = {
            let mut canvas = canvas.borrow_mut();
            std::mem::replace(&mut canvas.inner, RenderingCanvas::new()).flatten()
        };
        self.submit_and_read_bgra(py, &frame)
    }

    /// Renders `registries` in real, genuine parallel (Phase 12 Step
    /// 12.6) -- Python only supplies the work; every threading decision
    /// (how many real OS threads, when they run, how their output is
    /// merged) is made here, in Rust. Each registry is flattened into
    /// its own real `SubCanvas` on its own real OS thread (the GIL
    /// released for the whole span, so this is true parallelism, not
    /// GIL-serialized cooperative scheduling), then every `SubCanvas` is
    /// stitched into one shared `FrameArena` and submitted as a single
    /// frame.
    ///
    /// The real concurrency-safety story, verified before building this
    /// (not assumed): `SubCanvas::stitch_into`'s own `ScatterArena`
    /// reservations are lock-free; the real per-shape GPU style-buffer
    /// writes (`RhiDevice::shape_style_buffer`) are mutex-protected in
    /// the real Vulkan backend (`VulkanRingBuffer::write`); and the
    /// shared text atlas's own request/lookup path is lock-free by
    /// design (Phase 4) -- none of `ShapeRegistry::flatten_into`'s own
    /// real work needs any *new* synchronization to run concurrently
    /// across registries. The one genuinely serialized step is the text
    /// atlas's own texture-refresh check (`TextAtlas::context`), done
    /// once up front, before any worker thread starts -- every thread
    /// shares that one already-resolved `GlyphAtlasContext` read-only.
    ///
    /// Each registry's own current state is always flattened in full
    /// (`mark_all_dirty`), matching `render`'s own single-registry
    /// convenience wrapper.
    ///
    /// # Errors
    /// Raises `ValueError` if `registries.len()` exceeds this machine's
    /// own concurrency cap (`available_parallelism() - 1`), `TreError`
    /// if the combined scene exceeds `render_parallel`'s own fixed
    /// `FrameArena` capacity (see [`PARALLEL_ARENA_VERTEX_CAPACITY`]'s
    /// own doc comment) or any other real, recoverable engine failure.
    fn render_parallel(
        &mut self,
        py: Python<'_>,
        registries: Vec<Py<PyShapeRegistry>>,
    ) -> PyResult<Py<PyBytes>> {
        let root = RenderingCanvas::new();
        if registries.len() > root.max_sub_canvases() {
            return Err(PyValueError::new_err(format!(
                "render_parallel got {} registries, more than this machine's own concurrency cap \
                 of {} (available_parallelism() - 1)",
                registries.len(),
                root.max_sub_canvases()
            )));
        }

        let atlas_context = self.text_atlas.context(&self.device)?;

        // `try_borrow_mut`, not `borrow_mut` -- the latter panics on a
        // bad borrow (e.g. the same registry object passed twice in
        // `registries`, or already borrowed elsewhere), a real caller
        // mistake this should report cleanly, not crash on.
        let mut guards: Vec<PyRefMut<'_, PyShapeRegistry>> = registries
            .iter()
            .map(|r| r.bind(py).try_borrow_mut().map_err(PyErr::from))
            .collect::<PyResult<_>>()?;
        // `PyRefMut` itself is `!Send` (it carries a `Python<'py>` GIL
        // token internally), so it cannot cross into the `py.detach`
        // closure below at all -- not even just captured, unused. Each
        // guard stays right here, keeping its own borrow-flag set (and
        // therefore keeping any *other* concurrent Python-side access to
        // the same registry correctly rejected) for this whole method's
        // duration; only a plain `&mut PyShapeRegistry` re-borrowed out
        // of it -- ordinary Rust data with no GIL ties -- actually moves
        // into a worker thread.
        let mut registry_refs: Vec<&mut PyShapeRegistry> =
            guards.iter_mut().map(|g| &mut **g).collect();
        let mut sub_canvases: Vec<_> = registry_refs
            .iter()
            .map(|_| root.create_sub_canvas())
            .collect();

        let device = &self.device;
        let atlas_context = &atlas_context;
        py.detach(|| {
            std::thread::scope(|scope| {
                for (registry, sub_canvas) in registry_refs.iter_mut().zip(sub_canvases.iter_mut())
                {
                    let registry: &mut PyShapeRegistry = registry;
                    scope.spawn(move || {
                        registry.inner.mark_all_dirty();
                        let (inner, fonts) = registry.inner_and_fonts();
                        let text_context = TextFlattenContext {
                            fonts,
                            atlas: atlas_context,
                        };
                        inner.flatten_into(sub_canvas, device, Some(&text_context));
                    });
                }
            });
        });

        let mut arena = FrameArena::with_capacity(
            PARALLEL_ARENA_VERTEX_CAPACITY,
            PARALLEL_ARENA_INDEX_CAPACITY,
            PARALLEL_ARENA_COMMAND_CAPACITY,
            PARALLEL_ARENA_ACCESSIBILITY_CAPACITY,
        );
        for sub_canvas in &sub_canvases {
            if !sub_canvas.stitch_into(&arena) {
                return Err(crate::error::TreError::new_err(
                    "render_parallel's combined scene exceeded its own fixed FrameArena capacity \
                     -- reduce the total shape count across all registries, or split it across \
                     multiple render_parallel() calls",
                ));
            }
        }
        let mut frame = FlattenedFrame::default();
        arena.flatten_into(&mut frame);

        self.submit_and_read_bgra(py, &frame)
    }
}

impl PyHeadlessRenderer {
    /// Releases the GIL for the real GPU round trip (upload, submit,
    /// present, readback) -- IMPLEMENTATION.md Step 10.4 task 3 -- so
    /// other Python threads keep running while this one blocks on the
    /// GPU fence. Shared by `render`/`render_canvas`, the only two real
    /// differences between them being how `frame` itself gets built.
    fn submit_and_read_bgra(
        &mut self,
        py: Python<'_>,
        frame: &tre_engine::FlattenedFrame,
    ) -> PyResult<Py<PyBytes>> {
        let bgra: Result<Vec<u8>, RenderError> = py.detach(|| {
            let vertex_bytes: &[u8] = bytemuck::cast_slice(&frame.vertices);
            let index_bytes: &[u8] = bytemuck::cast_slice(&frame.indices);
            // `&*self.ring_buffer` (not `&self.ring_buffer`) -- `write`
            // is a `RhiDynamicRingBuffer` trait method, and `ring_buffer`
            // is a `Box<dyn RhiDynamicRingBuffer>`; `BufferBinding.buffer`
            // needs the same `&dyn RhiBuffer` either write returns from.
            let vertex_offset = self
                .ring_buffer
                .write(vertex_bytes)
                .ok_or(RenderError::RingBufferStarved)?;
            let index_offset = self
                .ring_buffer
                .write(index_bytes)
                .ok_or(RenderError::RingBufferStarved)?;
            let full_window = ScissorRect {
                x: 0,
                y: 0,
                width: self.width,
                height: self.height,
            };
            submit_frame(&self.device, &self.swapchain, |cmd_buffer| {
                execute_frame(
                    frame,
                    &self.pipelines,
                    BufferBinding {
                        buffer: &*self.ring_buffer,
                        offset: vertex_offset,
                    },
                    BufferBinding {
                        buffer: &*self.ring_buffer,
                        offset: index_offset,
                    },
                    &full_window,
                    &self.device,
                    cmd_buffer,
                );
            })?;
            Ok(self.swapchain.read_pixels_bgra8()?)
        });

        let bgra = bgra.map_err(render_err)?;
        Ok(PyBytes::new(py, &bgra).unbind())
    }
}
