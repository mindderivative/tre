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
//!
//! REVIEW.md finding #235, Option 2, ported here from `tre-perf-suite`'s
//! own `resize_test.rs` after the project owner directly compared it
//! against Option 3 and chose it ("Option 2 is the way I want to go"):
//! `submit_frame_to_window`'s proactive resize (finding #234) targets a
//! coarse, `DRAG_COARSE_STEP`-rounded size while `coarse_target_for`'s
//! own settle countdown is still running, and the exact real size once
//! it has expired -- see that function's own doc comment for the full
//! account.
//!
//! **Revised after a live test showed real, visible squash/stretch
//! during a drag** (the project owner: "the shapes begin to squash,
//! then jump suddenly to the right size") -- researching how pyCopper's
//! own `LESSONS_LEARNED.md`-cited fix actually avoids this (its own
//! source, `pycopper/src/pycopper/runtime/engine.py`, not just the
//! summary) found the real mechanism: content is projected using the
//! REAL window size even while the swapchain buffer itself is held at a
//! coarser size, so Wayland's own compositor-side scale-to-fit (the
//! same mechanism finding #234 fixed as a bug when unintentional)
//! exactly cancels the resulting pre-stretch. `submit_frame_to_window`
//! now does the identical thing via the new
//! [`tre_engine::submit_frame_with_logical_size`] -- see that
//! function's own doc comment for the geometry argument in full. The
//! settle policy (`coarse_target_for`) also now matches pyCopper's own
//! validated state machine exactly (a `SETTLE_FRAMES`-frame countdown
//! after the last real size change, not a wall-clock timer), and
//! `DRAG_COARSE_STEP` was raised from an untested 64px guess to
//! pyCopper's own production value (256px) -- safe to be far coarser
//! now that the projection fix makes the coarse/exact mismatch
//! invisible rather than merely "smaller."

use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use raw_window_handle::HasDisplayHandle;
use tre_engine::{
    execute_frame, submit_frame_with_logical_size, BufferBinding, EngineError, FlattenedFrame,
    FrameArena, PipelineRegistry, RenderingCanvas, RhiDevice, RhiDynamicRingBuffer, RhiSwapchain,
    ScissorRect, WindowId,
};
use tre_platform::{CursorIcon, PlatformConnection, WindowIcon};
use tre_rhi_vulkan::{register_shape_pipelines, VulkanDevice, VulkanSwapchain};

use crate::canvas::PyCanvas;
use crate::error::engine_err;
use crate::input::{PyInputEvent, PyWindowId};
use crate::renderer::{
    flatten_registry_into, render_canvas_shared, render_err, render_parallel_shared,
    render_single_registry, setup_err, validate_dimensions, RenderError,
    RENDER_ARENA_ACCESSIBILITY_CAPACITY, RENDER_ARENA_COMMAND_CAPACITY,
    RENDER_ARENA_INDEX_CAPACITY, RENDER_ARENA_VERTEX_CAPACITY, RING_BUFFER_CAPACITY,
};
use crate::shapes::PyShapeRegistry;
use crate::text_atlas::TextAtlas;
use crate::texture::{PyTexture, PyTextureFormat};

fn unknown_window_err(id: WindowId) -> PyErr {
    PyValueError::new_err(format!(
        "window {} does not belong to this WindowedRenderer (already closed, or never created \
         by it)",
        id.0
    ))
}

struct WindowSlot {
    // REVIEW.md finding #232: back to a concrete `VulkanSwapchain`
    // (finding #216 had generalized this to `Box<dyn RhiSwapchain>`,
    // reasoning "every real use of it already went through `&dyn
    // RhiSwapchain`") -- that premise no longer holds now that resize
    // uses `VulkanSwapchain::recreate` directly, a Vulkan-specific
    // capability with no `RhiSwapchain` trait equivalent (and none
    // planned: it would need the same low-level `ash` handles
    // `RhiDevice`'s own higher-level trait deliberately doesn't expose).
    // The same "ongoing concrete-device need" reasoning this struct's
    // own `device` field docs already state applies identically here.
    swapchain: VulkanSwapchain,
    pipelines: PipelineRegistry,
    width: u32,
    height: u32,
    /// REVIEW.md finding #235, Option 2: `width`/`height` as of the
    /// previous `submit_frame_to_window` call, and the settle countdown
    /// -- `coarse_target_for`'s own two state variables, persisted here
    /// per-window between calls (mirroring pyCopper's own `Engine.
    /// _last_size`/`_settle` fields).
    previous_size: (u32, u32),
    settle: u8,
}

/// REVIEW.md finding #235, Option 2: while a drag is active, the
/// swapchain targets `width`/`height` each rounded up to this many
/// pixels instead of the exact live size, so a burst of same-bucket
/// resize events needs no swapchain recreation at all. 256, matching
/// pyCopper's own validated production value (`Settings.resize_bucket`)
/// exactly, not this project's own earlier, untested 64px guess -- safe
/// to be this coarse because `coarse_target_for`'s caller now also
/// applies pyCopper's other real half of the fix (`submit_frame_with_
/// logical_size`), which makes the coarse/exact mismatch invisible
/// rather than merely small.
const DRAG_COARSE_STEP: u32 = 256;
/// REVIEW.md finding #235, Option 2: frames the swapchain stays pinned
/// to a coarse size after the last real size change -- matching
/// pyCopper's own `SETTLE_FRAMES` exactly, and for the identical reason
/// stated in its own doc comment: a resize draws synchronously per
/// compositor configure, so this is naturally a handful of frames after
/// a drag stops, not a wall-clock delay tied to any particular frame
/// rate.
const SETTLE_FRAMES: u8 = 3;

/// The size to configure the swapchain at, and the new settle countdown
/// -- a direct Rust port of pyCopper's own `surface_size_for`
/// (`pycopper/src/pycopper/runtime/engine.py`), factored out exactly as
/// it is there so the policy is a plain, testable decision about
/// integers with no window/GPU state of its own.
fn coarse_target_for(
    size: (u32, u32),
    previous: (u32, u32),
    settle: u8,
    bucket: u32,
) -> ((u32, u32), u8) {
    let settle = if size != previous {
        SETTLE_FRAMES
    } else {
        settle.saturating_sub(1)
    };
    if settle > 0 {
        (
            (
                size.0.div_ceil(bucket) * bucket,
                size.1.div_ceil(bucket) * bucket,
            ),
            settle,
        )
    } else {
        (size, settle)
    }
}

impl WindowSlot {
    // `device` stays a concrete `&VulkanDevice`, not `&dyn RhiDevice`
    // (unlike `PyHeadlessRenderer`'s fully generalized field) -- real,
    // disclosed boundary: `register_shape_pipelines` (below) needs a
    // concrete backend device to compile this window's own shape
    // pipelines against, and unlike `PyHeadlessRenderer` (which does
    // this exactly once, before its own device field is boxed), this
    // renderer calls it again every time a new window is created
    // (`create_window`) -- an ongoing, not one-time, concrete-device
    // need. See `PyWindowedRenderer::device`'s own field doc comment.
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
            previous_size: (width, height),
            settle: 0,
        })
    }

    /// Resizes in place: recreates only `self.swapchain`'s own
    /// `VkSwapchainKHR` and dependent resources (via `VulkanSwapchain::
    /// recreate`, reusing the same `VkSurfaceKHR` this slot was created
    /// with) and updates `width`/`height` -- `self.pipelines` is
    /// deliberately untouched, since a swapchain's color format is a
    /// property of the surface, which this never touches. Replaces the
    /// previous "drop this whole `WindowSlot`, build a brand-new one
    /// with a brand-new surface" resize path (REVIEW.md finding #230),
    /// which is what caused that finding's own real Wayland
    /// `wp_fifo_manager_v1` crash in the first place.
    fn resize(
        &mut self,
        device: &VulkanDevice,
        width: u32,
        height: u32,
    ) -> Result<(), EngineError> {
        self.swapchain.recreate(device, width, height)?;
        self.width = width;
        self.height = height;
        Ok(())
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
    // `text_atlas` (Phase 12 Step 12.3) joins `ring_buffer`/`windows` for
    // the identical reason: its own GPU texture is built from `device`.
    ring_buffer: Box<dyn RhiDynamicRingBuffer>,
    text_atlas: TextAtlas,
    windows: HashMap<WindowId, WindowSlot>,
    connection: PlatformConnection,
    // Stays a concrete `VulkanDevice`, not `Box<dyn RhiDevice>` (unlike
    // `PyHeadlessRenderer::device`, Architecture review: RHI trait-object
    // generalization, REVIEW.md finding #216) -- a real, disclosed
    // boundary, not an oversight. Every *rendering* use of this field
    // already goes through `&dyn RhiDevice` (`submit_frame`/
    // `execute_frame`, `flatten_registry_into`, `create_dynamic_ring_
    // buffer`, `TextAtlas::new`), but `create_window`/resize recovery
    // repeatedly call `device.create_surface(..)` (raw platform-handle
    // surface construction) and `register_shape_pipelines` (compiling
    // this crate's own built-in shape pipelines) throughout this
    // renderer's whole lifetime, not just once at construction --
    // neither is part of the `RhiDevice` trait, and neither reasonably
    // belongs there: both are backend-selection/window-surface
    // bootstrapping, the same category `VulkanDevice::new` itself
    // already occupies, not steady-state per-frame rendering. Only
    // `WindowSlot::swapchain` (per-window, built once, used many times
    // purely through the trait) generalizes cleanly here.
    device: VulkanDevice,
    main_window: WindowId,
    /// `render`'s own persistent scratch canvas, and `render`/
    /// `render_canvas`/`render_parallel`'s shared persistent stitch
    /// target/output buffer (the Performance review's "render()
    /// allocates a fresh canvas every frame" finding) -- see
    /// `PyHeadlessRenderer`'s identical fields for the full rationale.
    /// Hold no GPU handles of their own, so their position in this
    /// struct has no teardown-order implication.
    scratch_canvas: RenderingCanvas,
    frame_arena: FrameArena,
    flattened: FlattenedFrame,
    /// REVIEW.md finding #222: `execute_frame`'s own clip-stack scratch is
    /// now caller-owned -- reused every `submit_frame_to_window` call,
    /// cleared rather than reallocated, matching `scratch_canvas`/
    /// `frame_arena`/`flattened`'s own zero-allocation-reuse convention
    /// just above. Shared across every window rather than per-`WindowSlot`
    /// since only one window submits at a time through this method.
    clip_stack: Vec<ScissorRect>,
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
        let text_atlas = TextAtlas::new(&device)?;

        let mut windows = HashMap::new();
        windows.insert(
            main_window,
            WindowSlot {
                swapchain,
                pipelines,
                width,
                height,
                previous_size: (width, height),
                settle: 0,
            },
        );

        Ok(Self {
            ring_buffer,
            text_atlas,
            windows,
            connection,
            device,
            main_window,
            scratch_canvas: RenderingCanvas::new(),
            frame_arena: FrameArena::with_capacity(
                RENDER_ARENA_VERTEX_CAPACITY,
                RENDER_ARENA_INDEX_CAPACITY,
                RENDER_ARENA_COMMAND_CAPACITY,
                RENDER_ARENA_ACCESSIBILITY_CAPACITY,
            ),
            flattened: FlattenedFrame::default(),
            clip_stack: Vec::new(),
        })
    }

    #[getter]
    fn main_window(&self) -> PyWindowId {
        self.main_window.into()
    }

    /// Uploads `pixels` (tightly packed, `width * height` pixels in
    /// `format`) as a new, real GPU texture and registers it in the
    /// bindless array, returning a `Texture` usable as any shape's
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
            } = event
            {
                let (window, width, height) = (*window, *width, *height);
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
        let mut reg = registry.borrow_mut();
        render_single_registry(
            &self.device,
            &mut self.text_atlas,
            &mut self.scratch_canvas,
            &mut self.frame_arena,
            &mut self.flattened,
            &mut reg,
        )?;
        drop(reg);
        self.submit_frame_to_window(py, window.0)
    }

    /// Flattens `registry`'s current shapes into `canvas` -- the real
    /// seam (Phase 12 Step 12.5) letting more than one registry, with
    /// `canvas.clip(...)`/`canvas.layer(...)` scopes interleaved between
    /// them, share one canvas before a single
    /// [`render_canvas`](Self::render_canvas) call submits it to a real
    /// window. `render`'s own single-registry convenience wrapper calls
    /// this internally with a fresh, throwaway canvas.
    ///
    /// # Errors
    /// Raises `TreError` if a due text-atlas texture refresh fails.
    fn flatten_into(
        &mut self,
        canvas: &Bound<'_, PyCanvas>,
        registry: &Bound<'_, PyShapeRegistry>,
    ) -> PyResult<()> {
        let mut reg = registry.borrow_mut();
        let mut canvas = canvas.borrow_mut();
        flatten_registry_into(
            &self.device,
            &mut self.text_atlas,
            &mut canvas.inner,
            &mut reg,
        )
    }

    /// Renders an already-assembled [`PyCanvas`] to `window`'s own
    /// swapchain -- the real "submit what I built" counterpart to
    /// `render`'s own single-registry convenience wrapper. Consumes
    /// `canvas`'s own recorded content (leaving it freshly empty, like a
    /// new `Canvas`) rather than the `Canvas` object itself, so the same
    /// Python `Canvas` can be reused next frame.
    ///
    /// # Errors
    /// Raises `ValueError` if `window` is not a window this renderer
    /// created (or has already been closed), `TreError` on any other
    /// real, recoverable engine failure.
    fn render_canvas(
        &mut self,
        py: Python<'_>,
        window: PyWindowId,
        canvas: &Bound<'_, PyCanvas>,
    ) -> PyResult<()> {
        {
            let mut canvas = canvas.borrow_mut();
            render_canvas_shared(
                &mut canvas.inner,
                &mut self.frame_arena,
                &mut self.flattened,
            )?;
        }
        self.submit_frame_to_window(py, window.0)
    }

    /// Renders `registries` in real, genuine parallel to `window` (Phase
    /// 12 Step 12.6) -- see `PyHeadlessRenderer::render_parallel`'s own
    /// doc comment for the full real concurrency-safety account (lock-
    /// free `stitch_into`, a mutex-protected GPU style buffer, a
    /// lock-free shared text atlas); the only difference here is the
    /// final step submits to `window`'s own swapchain (with the same
    /// real resize-recovery retry every other `render`/`render_canvas`
    /// call already gets) instead of returning readback bytes.
    ///
    /// # Errors
    /// Raises `ValueError` if `window` is unknown to this renderer or
    /// `registries.len()` exceeds this machine's own concurrency cap,
    /// `TreError` on any other real, recoverable engine failure.
    fn render_parallel(
        &mut self,
        py: Python<'_>,
        window: PyWindowId,
        registries: Vec<Py<PyShapeRegistry>>,
    ) -> PyResult<()> {
        render_parallel_shared(
            py,
            &self.device,
            &mut self.text_atlas,
            &mut self.frame_arena,
            &mut self.flattened,
            registries,
        )?;
        self.submit_frame_to_window(py, window.0)
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

    /// Sets `window`'s mouse cursor appearance (Phase 12 Step 12.7).
    ///
    /// # Errors
    /// Raises `ValueError` if `window` is unknown to this renderer.
    fn set_cursor(&self, window: PyWindowId, icon: PyCursorIcon) -> PyResult<()> {
        self.connection
            .set_cursor(window.0, icon.into())
            .map_err(setup_err)
    }

    /// Enables or disables real IME composition for `window` -- required
    /// before `poll_events()` will ever return `InputEvent.ImeEnabled`/
    /// `ImePreedit`/`ImeCommit`/`ImeDisabled` for it (a real platform
    /// requirement, not a tre choice). A real text-input caller enables
    /// this only while an editable field has focus, and disables it again
    /// when focus leaves -- see `tre_platform::PlatformConnection::
    /// set_ime_allowed`'s own doc comment for why.
    ///
    /// # Errors
    /// Raises `ValueError` if `window` is unknown to this renderer.
    fn set_ime_allowed(&self, window: PyWindowId, allowed: bool) -> PyResult<()> {
        self.connection
            .set_ime_allowed(window.0, allowed)
            .map_err(setup_err)
    }
}

/// `tre_platform::CursorIcon`, bound directly.
#[pyclass(name = "CursorIcon", eq)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PyCursorIcon {
    Default,
    ContextMenu,
    Help,
    Pointer,
    Progress,
    Wait,
    Cell,
    Crosshair,
    Text,
    VerticalText,
    Alias,
    Copy,
    Move,
    NoDrop,
    NotAllowed,
    Grab,
    Grabbing,
    EResize,
    NResize,
    NeResize,
    NwResize,
    SResize,
    SeResize,
    SwResize,
    WResize,
    EwResize,
    NsResize,
    NeswResize,
    NwseResize,
    ColResize,
    RowResize,
    AllScroll,
    ZoomIn,
    ZoomOut,
}

impl From<PyCursorIcon> for CursorIcon {
    fn from(icon: PyCursorIcon) -> Self {
        match icon {
            PyCursorIcon::Default => Self::Default,
            PyCursorIcon::ContextMenu => Self::ContextMenu,
            PyCursorIcon::Help => Self::Help,
            PyCursorIcon::Pointer => Self::Pointer,
            PyCursorIcon::Progress => Self::Progress,
            PyCursorIcon::Wait => Self::Wait,
            PyCursorIcon::Cell => Self::Cell,
            PyCursorIcon::Crosshair => Self::Crosshair,
            PyCursorIcon::Text => Self::Text,
            PyCursorIcon::VerticalText => Self::VerticalText,
            PyCursorIcon::Alias => Self::Alias,
            PyCursorIcon::Copy => Self::Copy,
            PyCursorIcon::Move => Self::Move,
            PyCursorIcon::NoDrop => Self::NoDrop,
            PyCursorIcon::NotAllowed => Self::NotAllowed,
            PyCursorIcon::Grab => Self::Grab,
            PyCursorIcon::Grabbing => Self::Grabbing,
            PyCursorIcon::EResize => Self::EResize,
            PyCursorIcon::NResize => Self::NResize,
            PyCursorIcon::NeResize => Self::NeResize,
            PyCursorIcon::NwResize => Self::NwResize,
            PyCursorIcon::SResize => Self::SResize,
            PyCursorIcon::SeResize => Self::SeResize,
            PyCursorIcon::SwResize => Self::SwResize,
            PyCursorIcon::WResize => Self::WResize,
            PyCursorIcon::EwResize => Self::EwResize,
            PyCursorIcon::NsResize => Self::NsResize,
            PyCursorIcon::NeswResize => Self::NeswResize,
            PyCursorIcon::NwseResize => Self::NwseResize,
            PyCursorIcon::ColResize => Self::ColResize,
            PyCursorIcon::RowResize => Self::RowResize,
            PyCursorIcon::AllScroll => Self::AllScroll,
            PyCursorIcon::ZoomIn => Self::ZoomIn,
            PyCursorIcon::ZoomOut => Self::ZoomOut,
        }
    }
}

impl PyWindowedRenderer {
    /// Submits `self.flattened` to `window`'s own swapchain, with the
    /// same real resize-recovery retry (this module's own doc comment)
    /// every submission path shares. Factored out so `render_canvas`/
    /// `render_parallel` differ only in how `self.flattened` itself gets
    /// built before calling this. Reads `self.flattened` directly rather
    /// than taking it as a parameter -- see `PyHeadlessRenderer::
    /// submit_and_read_bgra`'s identical doc-comment note for why.
    fn submit_frame_to_window(&mut self, py: Python<'_>, window: WindowId) -> PyResult<()> {
        if !self.windows.contains_key(&window) {
            return Err(unknown_window_err(window));
        }

        // REVIEW.md finding #234: resize the swapchain proactively, here,
        // BEFORE ever attempting to submit a frame against it -- `poll_
        // events` already updates `slot.width`/`slot.height` the instant
        // a real `Resized` event arrives, but until this check existed,
        // nothing made the *swapchain itself* catch up until a real
        // `SwapchainOutOfDate` error eventually surfaced from `vkAcquire
        // NextImage`/`vkQueuePresent` -- purely reactive, matching this
        // module's own original doc comment ("a subsequent render() call
        // recreates that window's swapchain automatically if needed").
        // Wayland does not report that error promptly: instead of
        // erroring, it silently stretches the still-stale-sized
        // swapchain image to fit the window's already-resized real
        // surface bounds, which is real, visible squash/stretch during a
        // live resize -- not a transient one-frame glitch, and not
        // something the existing pixel-space coordinate math (finding
        // #233) has any way to prevent, since the GPU image being
        // presented is a genuinely wrong size the whole time. Checking
        // and resizing here closes that window entirely: by the time
        // `submit_frame` below ever runs, the swapchain's own real
        // extent already matches this check's own target size.
        //
        // REVIEW.md finding #235, Option 2: that target is `slot.width`/
        // `slot.height` exactly once `coarse_target_for`'s own settle
        // countdown (pyCopper's `SETTLE_FRAMES` state machine, ported
        // verbatim) has expired -- while it's still running, each is
        // rounded up to the nearest `DRAG_COARSE_STEP` pixels instead, so
        // a burst of same-bucket resize events during one continuous
        // drag needs no swapchain recreation at all (ported from
        // `tre-perf-suite/src/resize_test.rs` after the project owner
        // directly compared this against a bounded-timeout-acquire
        // alternative and chose this one). Content is still placed
        // pixel-exactly either way -- see `submit_frame_with_logical_
        // size`'s own doc comment below for why a coarse swapchain no
        // longer means visible squash/stretch, unlike this function's
        // own first version of this fix.
        let (target_width, target_height) = if let Some(slot) = self.windows.get_mut(&window) {
            let current_size = (slot.width, slot.height);
            let (target, settle) = coarse_target_for(
                current_size,
                slot.previous_size,
                slot.settle,
                DRAG_COARSE_STEP,
            );
            slot.previous_size = current_size;
            slot.settle = settle;
            target
        } else {
            unreachable!("checked present above")
        };
        if let Some(slot) = self.windows.get(&window) {
            let (actual_width, actual_height) = slot.swapchain.extent();
            if actual_width != target_width || actual_height != target_height {
                self.windows
                    .get_mut(&window)
                    .expect("checked present above")
                    .resize(&self.device, target_width, target_height)
                    .map_err(engine_err)?;
            }
        }

        let frame = &self.flattened;
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
            let clip_stack = &mut self.clip_stack;
            // The swapchain's own real (possibly coarse, mid-drag)
            // extent -- the scissor must always match whatever the
            // swapchain was actually just resized to above, never the
            // window's exact logical size (REVIEW.md finding #235).
            let (window_width, window_height) = swapchain.extent();
            let full_window = ScissorRect {
                x: 0,
                y: 0,
                width: window_width,
                height: window_height,
            };
            // REVIEW.md finding #235, Option 2's real fix: content is
            // still projected using the window's true logical size
            // (`slot.width`/`slot.height`) even when the swapchain
            // buffer above is a coarser size -- pyCopper's own
            // mechanism (`engine.py`'s `_upload`/`_pin_surface`) for why
            // this produces pixel-exact geometry with no visible
            // squash/stretch: the compositor's own scale-to-fit of the
            // oversized buffer down to the real window exactly cancels
            // the pre-stretch this causes. See
            // `tre_engine::submit_frame_with_logical_size`'s own doc
            // comment for the full mechanism.
            let logical_size = (slot.width, slot.height);
            let outcome: Result<(), RenderError> = py.detach(move || {
                let vertex_offset = ring_buffer
                    .write(vertex_bytes)
                    .ok_or(RenderError::RingBufferStarved)?;
                let index_offset = ring_buffer
                    .write(index_bytes)
                    .ok_or(RenderError::RingBufferStarved)?;
                submit_frame_with_logical_size(device, swapchain, logical_size, |cmd_buffer| {
                    execute_frame(
                        frame,
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
                        clip_stack,
                    );
                })?;
                Ok(())
            });

            match outcome {
                Ok(()) => return Ok(()),
                Err(RenderError::Engine(EngineError::SwapchainOutOfDate)) if attempt == 0 => {
                    let (width, height) = (slot.width, slot.height);
                    // REVIEW.md finding #232: resize in place via
                    // `WindowSlot::resize` (which only recreates the
                    // swapchain, reusing the same `VkSurfaceKHR`)
                    // instead of tearing down and rebuilding the whole
                    // `WindowSlot` with a brand-new surface -- closes
                    // finding #230's own Wayland `wp_fifo_manager_v1`
                    // crash at its real root (the surface is never
                    // recreated at all) rather than working around it,
                    // and skips the redundant shape-pipeline
                    // re-registration finding #232 also found this path
                    // was doing on every single resize for no reason (a
                    // swapchain's color format never changes across a
                    // resize of the same surface).
                    self.windows
                        .get_mut(&window)
                        .expect(
                            "checked present above; only removed by close_window, not called \
                             here",
                        )
                        .resize(&self.device, width, height)
                        .map_err(engine_err)?;
                }
                Err(e) => return Err(render_err(e)),
            }
        }
        Err(render_err(RenderError::Engine(
            EngineError::SwapchainOutOfDate,
        )))
    }
}
