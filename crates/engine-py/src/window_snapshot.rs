//! 0.5.4 (#108): `Window.snapshot` -- what a window draws, as pixels.

use std::cell::RefCell;

use engine_core::InputEvent;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use taffy::prelude::{AvailableSpace, Size};

use crate::window::PyWindow;

thread_local! {
    /// The device snapshots render on, made on first use and kept: creating
    /// one takes far longer than a small frame. It is no window's device, so
    /// a snapshot works before `App.run()` and with no display at all.
    static DEVICE: RefCell<Option<(wgpu::Device, wgpu::Queue)>> = const { RefCell::new(None) };
}

/// A device on the default adapter, or on a software one where there is no
/// GPU (a headless CI machine).
fn headless_device() -> Result<(wgpu::Device, wgpu::Queue), String> {
    let instance = wgpu::Instance::default();
    let adapter = [false, true]
        .into_iter()
        .find_map(|fallback| {
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: fallback,
                compatible_surface: None,
                apply_limit_buckets: false,
            }))
            .ok()
        })
        .ok_or("no GPU adapter available to render a snapshot")?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("engine-py snapshot device"),
        required_features: wgpu::Features::empty(),
        ..Default::default()
    }))
    .map_err(|err| format!("couldn't create a GPU device for the snapshot: {err}"))
}

#[pymethods]
impl PyWindow {
    /// What the window draws, as `(rgba, width, height)`: straight-alpha
    /// RGBA8 bytes, `width * height * 4` of them, top row first.
    ///
    /// Rendered offscreen from the window's tree, so it works before
    /// `App.run()`, with no display, and while a window is open. `width` and
    /// `height` are logical pixels and default to the window's own; `scale`
    /// (default the window's, `1.0` before it opens or with `dpi_scaling` off)
    /// multiplies them into the pixels returned. `time` is the clock, in
    /// seconds, an animated shader sees as `frame.time`. Animations are drawn
    /// at their current values, not advanced.
    #[pyo3(signature = (width=None, height=None, scale=None, time=0.0))]
    fn snapshot(
        &mut self,
        py: Python<'_>,
        width: Option<f64>,
        height: Option<f64>,
        scale: Option<f64>,
        time: f32,
    ) -> PyResult<(Py<PyBytes>, u32, u32)> {
        let (own_w, own_h) = self.handles.logical_size();
        let (w, h) = (width.unwrap_or(own_w), height.unwrap_or(own_h));
        let scale = scale.unwrap_or_else(|| self.handles.scale.get());
        for (name, value) in [("width", w), ("height", h), ("scale", scale)] {
            if !value.is_finite() || value <= 0.0 {
                return Err(PyValueError::new_err(format!(
                    "snapshot `{name}` must be a number greater than 0"
                )));
            }
        }

        let (tree, root, handlers) = (
            self.handles.tree.clone(),
            self.handles.root,
            self.handles.handlers.clone(),
        );
        // A different size lays the tree out at that size, then puts it back.
        let resized = (w - own_w).abs() > f64::EPSILON || (h - own_h).abs() > f64::EPSILON;
        let now = crate::clock::now(&tree);
        if resized {
            tree.borrow_mut().dispatch(
                root,
                InputEvent::Resized {
                    width: w as f32,
                    height: h as f32,
                },
                now,
            );
        }
        crate::node_callbacks::layout(
            &tree,
            root,
            Size {
                width: AvailableSpace::Definite(w as f32),
                height: AvailableSpace::Definite(h as f32),
            },
            &handlers,
            py,
        );

        let (pixel_w, pixel_h) = (
            crate::scale::to_physical(w, scale),
            crate::scale::to_physical(h, scale),
        );
        let result = DEVICE.with(|cell| {
            let mut slot = cell.borrow_mut();
            if slot.is_none() {
                *slot = Some(headless_device()?);
            }
            let (device, queue) = slot.as_ref().expect("just made");
            engine_render::snapshot(
                device,
                queue,
                &tree.borrow(),
                root,
                pixel_w,
                pixel_h,
                scale,
                time,
            )
        });

        if resized {
            tree.borrow_mut().dispatch(
                root,
                InputEvent::Resized {
                    width: own_w as f32,
                    height: own_h as f32,
                },
                now,
            );
        }
        // Laid out at another size, or its rows materialized for one: the
        // window's next frame lays it out again.
        tree.borrow_mut().mark_dirty();

        let shot = result.map_err(PyRuntimeError::new_err)?;
        Ok((
            PyBytes::new(py, &shot.rgba).unbind(),
            shot.width,
            shot.height,
        ))
    }
}
