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

use ash::vk;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use raw_window_handle::HasDisplayHandle;
use tre_engine::{
    execute_frame, BufferBinding, EngineError, PipelineRegistry, RenderingCanvas, RhiDevice,
    ScissorRect,
};
use tre_rhi_vulkan::{register_shape_pipelines, HeadlessSwapchain, VulkanDevice, HEADLESS_FORMAT};

use crate::error::engine_err;
use crate::shapes::PyShapeRegistry;

fn setup_err<E: std::fmt::Display>(e: E) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
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
    // segfaulted at Python interpreter shutdown.
    pipelines: PipelineRegistry,
    swapchain: HeadlessSwapchain,
    device: VulkanDevice,
    width: u32,
    height: u32,
}

#[pymethods]
impl PyHeadlessRenderer {
    /// # Errors
    /// Raises `TreError` (a real `EngineError`) or `RuntimeError` (a
    /// display-server/window-setup failure -- see this module's own doc
    /// comment for why one is needed at all for a headless renderer).
    #[new]
    fn new(width: u32, height: u32) -> PyResult<Self> {
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

        Ok(Self {
            device,
            swapchain,
            pipelines,
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
    /// # Errors
    /// Raises `TreError` on any real, recoverable engine failure.
    fn render(
        &self,
        py: Python<'_>,
        registry: &Bound<'_, PyShapeRegistry>,
    ) -> PyResult<Py<PyBytes>> {
        let frame = {
            let mut reg = registry.borrow_mut();
            let mut canvas = RenderingCanvas::new();
            reg.inner.flatten_into(&mut canvas, &self.device);
            canvas.flatten()
        };

        let bgra: Result<Vec<u8>, EngineError> = py.detach(|| {
            let vertex_buffer = self.device.upload_buffer(
                bytemuck::cast_slice(&frame.vertices),
                vk::BufferUsageFlags::VERTEX_BUFFER,
            )?;
            let index_buffer = self.device.upload_buffer(
                bytemuck::cast_slice(&frame.indices),
                vk::BufferUsageFlags::INDEX_BUFFER,
            )?;
            let full_window = ScissorRect {
                x: 0,
                y: 0,
                width: self.width,
                height: self.height,
            };
            let (mut cmd_buffer, image) = self.device.begin_frame(&self.swapchain)?;
            execute_frame(
                &frame,
                &self.pipelines,
                BufferBinding {
                    buffer: &vertex_buffer,
                    offset: 0,
                },
                BufferBinding {
                    buffer: &index_buffer,
                    offset: 0,
                },
                &full_window,
                &self.device,
                &mut *cmd_buffer,
            );
            self.device
                .submit_and_present(cmd_buffer, &self.swapchain, image)?;
            self.swapchain.read_pixels_bgra8()
        });

        let bgra = bgra.map_err(engine_err)?;
        Ok(PyBytes::new(py, &bgra).unbind())
    }
}
