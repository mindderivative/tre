//! A real, on-screen renderer -- the piece that makes `tre-python` a GUI
//! framework's backend rather than "just a pretty renderer" (the project
//! owner's own framing). Wraps `tre_platform::PlatformConnection` +
//! `tre_rhi_vulkan::VulkanSwapchain`, following `multi_window.rs`'s real,
//! proven window/surface/swapchain sequence -- multiple windows sharing
//! one `VulkanDevice` from day one, since the underlying engine already
//! supports this for free.
//!
//! Real resize recovery (found necessary here, not previously fixed
//! anywhere in the workspace -- REVIEW.md finding #116 disclosed that
//! `EngineError::SwapchainOutOfDate` had no real recovery path anywhere,
//! and every existing Rust demo's own `.expect()` on `begin_frame`/
//! `submit_and_present` panics on a mid-run resize): `render()` catches
//! `SwapchainOutOfDate`, recreates that window's swapchain at its last-
//! known size, and retries once, rather than propagating a fatal error a
//! real GUI framework cannot recover a whole running application from.

use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use raw_window_handle::HasDisplayHandle;
use tre_engine::{
    execute_frame, submit_frame, BufferBinding, EngineError, PipelineRegistry, RenderingCanvas,
    RhiDevice, RhiDynamicRingBuffer, ScissorRect, WindowId,
};
use tre_platform::{PlatformConnection, WindowIcon};
use tre_rhi_vulkan::{register_shape_pipelines, VulkanDevice, VulkanSwapchain};

use crate::error::engine_err;
use crate::input::{PyInputEvent, PyWindowId};
use crate::renderer::{
    render_err, setup_err, validate_dimensions, RenderError, RING_BUFFER_CAPACITY,
};
use crate::shapes::PyShapeRegistry;

fn unknown_window_err(id: WindowId) -> PyErr {
    PyValueError::new_err(format!(
        "window {} does not belong to this WindowedRenderer (already closed, or never created \
         by it)",
        id.0
    ))
}

struct WindowSlot {
    swapchain: VulkanSwapchain,
    pipelines: PipelineRegistry,
    width: u32,
    height: u32,
}

impl WindowSlot {
    fn create(
        device: &VulkanDevice,
        connection: &PlatformConnection,
        window: WindowId,
        width: u32,
        height: u32,
    ) -> PyResult<Self> {
        let display_handle = connection.display_handle().map_err(setup_err)?.as_raw();
        let window_handle = connection
            .window_handle(window)
            .map_err(setup_err)?
            .as_raw();
        let (surface_loader, surface) = device
            .create_surface(display_handle, window_handle)
            .map_err(engine_err)?;
        let swapchain = VulkanSwapchain::new(device, surface_loader, surface, width, height)
            .map_err(engine_err)?;
        let mut pipelines = PipelineRegistry::new();
        register_shape_pipelines(device, &mut pipelines, swapchain.format()).map_err(engine_err)?;
        Ok(Self {
            swapchain,
            pipelines,
            width,
            height,
        })
    }
}

/// A window-backed renderer: create one or more real on-screen windows
/// sharing one GPU device, poll real input events, and render directly
/// to a window's own swapchain -- no byte readback (that stays
/// [`crate::renderer::PyHeadlessRenderer`]'s own job).
///
/// Construct once; [`main_window`](PyWindowedRenderer::main_window) is
/// the [`crate::input::PyWindowId`] of the window created by the
/// constructor itself. Call
/// [`create_window`](PyWindowedRenderer::create_window) for any
/// additional windows.
///
/// `unsendable` (found necessary via a real build, not assumed): this
/// struct owns a real `tre_platform::PlatformConnection`, which on
/// Linux wraps winit's X11/Wayland backends -- raw `Rc`/`RefCell`/FFI
/// handles (an X11 IME pointer, a Wayland event-loop `Rc`, ...) that
/// are not `Send`, because the underlying platform APIs themselves are
/// only ever safe to touch from the thread that created the connection.
/// `unsendable` tells PyO3 to enforce that same single-thread rule at
/// the Python boundary instead of requiring (falsely) that this type is
/// thread-safe.
#[pyclass(name = "WindowedRenderer", unsendable)]
pub struct PyWindowedRenderer {
    // See `PyHeadlessRenderer`'s own field-order comment (REVIEW.md
    // #193/#199): `ring_buffer`/`windows` hold live handles built from
    // `device`, so both must be declared -- and therefore dropped --
    // before it. `windows`' own swapchains additionally hold surfaces
    // built against `connection`'s real OS windows, so `connection`
    // must outlive them too (declared after `windows`, before `device`).
    ring_buffer: Box<dyn RhiDynamicRingBuffer>,
    windows: HashMap<WindowId, WindowSlot>,
    connection: PlatformConnection,
    device: VulkanDevice,
    main_window: WindowId,
}

#[pymethods]
impl PyWindowedRenderer {
    /// # Errors
    /// Raises `ValueError` if `width`/`height` are zero or unreasonably
    /// large, `TreError` on a real engine failure, or `RuntimeError` on
    /// a display-server/window-setup failure.
    #[new]
    fn new(title: &str, width: u32, height: u32) -> PyResult<Self> {
        validate_dimensions(width, height)?;
        let mut connection = PlatformConnection::new().map_err(setup_err)?;
        let main_window = connection
            .create_window(title, width, height)
            .map_err(setup_err)?;
        let display_handle = connection.display_handle().map_err(setup_err)?.as_raw();
        let window_handle = connection
            .window_handle(main_window)
            .map_err(setup_err)?
            .as_raw();
        let (device, surface_loader, surface) =
            VulkanDevice::new(display_handle, window_handle).map_err(engine_err)?;
        let swapchain = VulkanSwapchain::new(&device, surface_loader, surface, width, height)
            .map_err(engine_err)?;
        let mut pipelines = PipelineRegistry::new();
        register_shape_pipelines(&device, &mut pipelines, swapchain.format())
            .map_err(engine_err)?;
        let ring_buffer = device.create_dynamic_ring_buffer(RING_BUFFER_CAPACITY);

        let mut windows = HashMap::new();
        windows.insert(
            main_window,
            WindowSlot {
                swapchain,
                pipelines,
                width,
                height,
            },
        );

        Ok(Self {
            ring_buffer,
            windows,
            connection,
            device,
            main_window,
        })
    }

    #[getter]
    fn main_window(&self) -> PyWindowId {
        self.main_window.into()
    }

    /// Creates an additional top-level window sharing this renderer's
    /// own `VulkanDevice`/ring buffer, following the identical real
    /// sequence `multi_window.rs` proves works.
    ///
    /// # Errors
    /// Raises `ValueError`/`TreError`/`RuntimeError` under the same
    /// conditions as the constructor.
    fn create_window(&mut self, title: &str, width: u32, height: u32) -> PyResult<PyWindowId> {
        validate_dimensions(width, height)?;
        let window = self
            .connection
            .create_window(title, width, height)
            .map_err(setup_err)?;
        let slot = WindowSlot::create(&self.device, &self.connection, window, width, height)?;
        self.windows.insert(window, slot);
        Ok(window.into())
    }

    /// Drains pending input/lifecycle events for every window on this
    /// renderer. Call once per frame; never blocks. A `Resized` event's
    /// new size is also recorded internally, so a subsequent `render()`
    /// call recreates that window's swapchain automatically if needed.
    fn poll_events(&mut self) -> Vec<PyInputEvent> {
        let events = self.connection.poll_events();
        for event in &events {
            if let tre_engine::InputEvent::Resized {
                window,
                width,
                height,
            } = *event
            {
                if let Some(slot) = self.windows.get_mut(&window) {
                    slot.width = width;
                    slot.height = height;
                }
            }
        }
        events.into_iter().map(PyInputEvent::from).collect()
    }

    /// Removes `window` from this renderer, tearing down its swapchain.
    /// A no-op if `window` is already unknown (e.g. already closed).
    fn close_window(&mut self, window: PyWindowId) {
        self.windows.remove(&window.0);
    }

    #[getter]
    fn open_window_count(&self) -> usize {
        self.windows.len()
    }

    /// Flattens `registry`'s current shapes and renders them directly
    /// to `window`'s own swapchain -- no byte readback.
    ///
    /// Releases the GIL for the real GPU round trip, exactly like
    /// [`crate::renderer::PyHeadlessRenderer::render`], and for the
    /// identical reason takes `&mut self`.
    ///
    /// The GIL-released closure itself only ever borrows `device`/
    /// `ring_buffer`/the target window's `swapchain`+`pipelines` --
    /// never `self.connection` -- deliberately: `PlatformConnection`
    /// is not `Send` (this class's own `unsendable` doc comment), so
    /// `Python::detach`'s `Ungil` bound would reject a closure that
    /// touched it, found via a real build. Swapchain recreation on a
    /// stale-swapchain retry (which does need `connection`) therefore
    /// happens back on the GIL-holding thread, between `detach` calls,
    /// not inside one.
    ///
    /// # Errors
    /// Raises `ValueError` if `window` is not a window this renderer
    /// created (or has already been closed), `TreError` on any other
    /// real, recoverable engine failure.
    fn render(
        &mut self,
        py: Python<'_>,
        window: PyWindowId,
        registry: &Bound<'_, PyShapeRegistry>,
    ) -> PyResult<()> {
        let window = window.0;
        if !self.windows.contains_key(&window) {
            return Err(unknown_window_err(window));
        }

        let frame = {
            let mut reg = registry.borrow_mut();
            let mut canvas = RenderingCanvas::new();
            reg.inner.mark_all_dirty();
            reg.inner.flatten_into(&mut canvas, &self.device);
            canvas.flatten()
        };
        let vertex_bytes: &[u8] = bytemuck::cast_slice(&frame.vertices);
        let index_bytes: &[u8] = bytemuck::cast_slice(&frame.indices);

        // Real resize recovery (this module's own doc comment): retry
        // exactly once against a freshly recreated swapchain if the
        // current one is stale.
        for attempt in 0..2 {
            let device = &self.device;
            let ring_buffer = &*self.ring_buffer;
            let slot = self
                .windows
                .get(&window)
                .expect("checked present above; only removed by close_window, not called here");
            let pipelines = &slot.pipelines;
            let swapchain = &slot.swapchain;
            let full_window = ScissorRect {
                x: 0,
                y: 0,
                width: slot.width,
                height: slot.height,
            };
            let frame_ref = &frame;

            let outcome: Result<(), RenderError> = py.detach(move || {
                let vertex_offset = ring_buffer
                    .write(vertex_bytes)
                    .ok_or(RenderError::RingBufferStarved)?;
                let index_offset = ring_buffer
                    .write(index_bytes)
                    .ok_or(RenderError::RingBufferStarved)?;
                submit_frame(device, swapchain, |cmd_buffer| {
                    execute_frame(
                        frame_ref,
                        pipelines,
                        BufferBinding {
                            buffer: ring_buffer,
                            offset: vertex_offset,
                        },
                        BufferBinding {
                            buffer: ring_buffer,
                            offset: index_offset,
                        },
                        &full_window,
                        device,
                        cmd_buffer,
                    );
                })?;
                Ok(())
            });

            match outcome {
                Ok(()) => return Ok(()),
                Err(RenderError::Engine(EngineError::SwapchainOutOfDate)) if attempt == 0 => {
                    let (width, height) = (slot.width, slot.height);
                    let fresh =
                        WindowSlot::create(&self.device, &self.connection, window, width, height)?;
                    self.windows.insert(window, fresh);
                }
                Err(e) => return Err(render_err(e)),
            }
        }
        Err(render_err(RenderError::Engine(
            EngineError::SwapchainOutOfDate,
        )))
    }

    /// # Errors
    /// Raises `ValueError` if `window` is unknown to this renderer.
    fn set_title(&self, window: PyWindowId, title: &str) -> PyResult<()> {
        self.connection
            .set_title(window.0, title)
            .map_err(setup_err)
    }

    /// See `tre_platform::PlatformConnection::set_minimized`'s own doc
    /// comment for the real round-trip/platform caveats this passes
    /// through unchanged.
    ///
    /// # Errors
    /// Raises `ValueError` if `window` is unknown to this renderer.
    fn set_minimized(&self, window: PyWindowId, minimized: bool) -> PyResult<()> {
        self.connection
            .set_minimized(window.0, minimized)
            .map_err(setup_err)
    }

    fn is_minimized(&self, window: PyWindowId) -> Option<bool> {
        self.connection.is_minimized(window.0)
    }

    /// # Errors
    /// Raises `ValueError` if `window` is unknown to this renderer.
    fn set_maximized(&self, window: PyWindowId, maximized: bool) -> PyResult<()> {
        self.connection
            .set_maximized(window.0, maximized)
            .map_err(setup_err)
    }

    fn is_maximized(&self, window: PyWindowId) -> bool {
        self.connection.is_maximized(window.0)
    }

    /// Sets or clears (`rgba=None`) `window`'s icon. `rgba`, if given,
    /// must be exactly `width * height * 4` bytes (tightly packed RGBA8).
    /// A real no-op on Wayland (the protocol has no client-side icon
    /// mechanism) -- see `tre_platform::PlatformConnection::set_icon`'s
    /// own doc comment.
    ///
    /// # Errors
    /// Raises `ValueError` if `window` is unknown to this renderer, or
    /// if `rgba`'s length doesn't match `width * height * 4`.
    #[pyo3(signature = (window, rgba, width, height))]
    fn set_icon(
        &self,
        window: PyWindowId,
        rgba: Option<Vec<u8>>,
        width: u32,
        height: u32,
    ) -> PyResult<()> {
        let icon = rgba.map(|rgba| WindowIcon {
            rgba,
            width,
            height,
        });
        self.connection.set_icon(window.0, icon).map_err(setup_err)
    }
}
