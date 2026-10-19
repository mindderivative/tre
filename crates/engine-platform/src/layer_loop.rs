//! 0.5.7 (#198): layer surfaces inside the `winit` event loop.
//!
//! The layer thread (`layer`) reports through a callback that posts
//! `PlatformEvent::Layer`; the loop handles those here, on its own thread, where the
//! host's callbacks (which touch Python) may run. A layer surface has no `winit`
//! `WindowId`, so each is given one minted from the high-bit range of the `u64`s
//! `WindowId` converts from (the ids `winit` makes on Wayland, X11, Windows and macOS
//! are pointers or small numbers and never set the top bit): the callbacks the loop
//! already has keep their signature, and a layer surface is one more window to them.

use std::sync::Arc;

use engine_core::InputEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
use winit::window::{Window, WindowId};

use crate::layer::{
    CloseReason, LayerController, LayerEvent, LayerHost, LayerNotice, LayerRequest,
    LayerSurfaceInfo, RawPtr,
};
use crate::{MultiWindowApp, PlatformEvent, WindowLifecycle};

const LAYER_ID_TAG: u64 = 1 << 63;

/// The `WindowId` the loop uses for layer surface number `n`.
pub(crate) fn window_id(n: u64) -> WindowId {
    WindowId::from(LAYER_ID_TAG | n)
}

/// A layer surface's state in the loop.
pub(crate) struct PerLayer {
    token: u64,
    /// Its number (the low bits of its `WindowId`).
    id: u64,
    controller: LayerController,
    surface: Option<RawPtr>,
    /// Whether the host has built its GPU side (after the first configure).
    opened: bool,
    needs_frame: bool,
    animating: bool,
    frame: u32,
    max_frames: Option<u32>,
}

/// A logical size at `scale`, in physical pixels.
fn physical(logical: u32, scale: f64) -> u32 {
    (f64::from(logical) * scale).round().max(1.0) as u32
}

impl<C, F, A, N, X, L> MultiWindowApp<C, F, A, N, X, L>
where
    C: FnMut(WindowId, u64, Arc<Window>) -> bool,
    F: FnMut(WindowId, u32, bool) -> bool,
    A: FnMut(WindowId) -> accesskit::TreeUpdate,
    N: FnMut(WindowId, InputEvent),
    X: FnMut(WindowId, accesskit::ActionRequest),
    L: FnMut(WindowId, WindowLifecycle) -> bool,
{
    fn notice(&mut self, id: WindowId, token: u64, notice: LayerNotice) {
        if let Some(hooks) = &mut self.layer_hooks {
            (hooks.on_notice)(id, token, notice);
        }
    }

    /// Starts the loop's exit when nothing is left open.
    fn exit_if_empty(&self, event_loop: &ActiveEventLoop) {
        if self.windows.is_empty() && self.layers.is_empty() {
            event_loop.exit();
        }
    }

    /// `PlatformEvent::OpenLayer`: starts the layer thread if this is the first, and
    /// asks it for the surface.
    pub(crate) fn open_layer(&mut self, event_loop: &ActiveEventLoop, request: LayerRequest) {
        let n = self.next_layer;
        self.next_layer += 1;
        let id = window_id(n);
        if self.layer_host.is_none() {
            let display = match event_loop.display_handle().map(|h| h.as_raw()) {
                Ok(RawDisplayHandle::Wayland(handle)) => handle.display,
                _ => {
                    self.notice(
                        id,
                        request.token,
                        LayerNotice::Failed("layer surfaces need a Wayland session".into()),
                    );
                    return;
                }
            };
            let proxy = self.proxy.clone();
            match LayerHost::start(display, move |event| {
                proxy.send_event(PlatformEvent::Layer(event)).is_ok()
            }) {
                Ok(host) => self.layer_host = Some(host),
                Err(error) => {
                    self.notice(id, request.token, LayerNotice::Failed(error.to_string()));
                    return;
                }
            }
        }
        let controller = self
            .layer_host
            .as_ref()
            .expect("the host was just started")
            .controller();
        controller.create(n, request.config);
        self.layers.insert(
            id,
            PerLayer {
                token: request.token,
                id: n,
                controller,
                surface: None,
                opened: false,
                needs_frame: false,
                animating: false,
                frame: 0,
                max_frames: request.max_frames,
            },
        );
    }

    /// `PlatformEvent::Layer`: one thing the layer thread reports.
    pub(crate) fn layer_event(&mut self, event_loop: &ActiveEventLoop, event: LayerEvent) {
        match event {
            LayerEvent::Created { id, surface, .. } => {
                if let Some(layer) = self.layers.get_mut(&window_id(id)) {
                    layer.surface = Some(surface);
                }
            }
            LayerEvent::Configure {
                id,
                width,
                height,
                scale,
            }
            | LayerEvent::Rescale {
                id,
                width,
                height,
                scale,
            } => {
                let wid = window_id(id);
                let (pw, ph) = (physical(width, scale), physical(height, scale));
                let Some(layer) = self.layers.get_mut(&wid) else {
                    return;
                };
                if layer.opened {
                    layer.needs_frame = true;
                    (self.on_input)(
                        wid,
                        InputEvent::ScaleFactorChanged {
                            scale_factor: scale,
                        },
                    );
                    (self.on_input)(
                        wid,
                        InputEvent::Resized {
                            width: pw as f32,
                            height: ph as f32,
                        },
                    );
                    return;
                }
                let (Some(surface), token) = (layer.surface, layer.token) else {
                    return;
                };
                let controller = layer.controller.clone();
                let display = self
                    .layer_host
                    .as_ref()
                    .map(LayerHost::display)
                    .expect("a surface implies a host");
                let info = LayerSurfaceInfo {
                    id,
                    display,
                    surface,
                    width: pw,
                    height: ph,
                    scale,
                    controller: controller.clone(),
                };
                let accepted = self
                    .layer_hooks
                    .as_mut()
                    .is_some_and(|hooks| (hooks.on_created)(wid, token, info));
                if accepted {
                    if let Some(layer) = self.layers.get_mut(&wid) {
                        layer.opened = true;
                        layer.needs_frame = true;
                        layer.animating = true;
                    }
                } else {
                    controller.destroy(id);
                    self.layers.remove(&wid);
                    self.exit_if_empty(event_loop);
                }
            }
            LayerEvent::Closed { id, reason } => {
                let wid = window_id(id);
                if let Some(layer) = self.layers.remove(&wid) {
                    self.notice(wid, layer.token, LayerNotice::Closed(reason));
                    if layer.opened {
                        (self.on_lifecycle)(wid, WindowLifecycle::Closed);
                    }
                    self.exit_if_empty(event_loop);
                }
            }
            LayerEvent::Failed { id, message } => {
                let wid = window_id(id);
                if let Some(layer) = self.layers.remove(&wid) {
                    self.notice(wid, layer.token, LayerNotice::Failed(message));
                    self.exit_if_empty(event_loop);
                }
            }
            LayerEvent::Input { id, event } => {
                let wid = window_id(id);
                if self.layers.get(&wid).is_some_and(|l| l.opened) {
                    (self.on_input)(wid, event);
                    if let Some(layer) = self.layers.get_mut(&wid) {
                        layer.needs_frame = true;
                    }
                }
            }
            LayerEvent::OutputChanged { id, name } => {
                let wid = window_id(id);
                if let Some(token) = self.layers.get(&wid).map(|l| l.token) {
                    self.notice(wid, token, LayerNotice::OutputChanged(name));
                }
            }
        }
    }

    /// Draws every layer surface that needs a frame, and sets the loop's pace for all
    /// surfaces together.
    pub(crate) fn run_layer_frames(&mut self, event_loop: &ActiveEventLoop) {
        if self.layers.is_empty() {
            return;
        }
        let due: Vec<WindowId> = self
            .layers
            .iter()
            .filter(|(_, l)| l.opened && l.needs_frame)
            .map(|(id, _)| *id)
            .collect();
        for wid in due {
            let frame = match self.layers.get_mut(&wid) {
                Some(layer) => {
                    layer.needs_frame = false;
                    layer.frame
                }
                None => continue,
            };
            let still = (self.on_frame)(wid, frame, true);
            let Some(layer) = self.layers.get_mut(&wid) else {
                continue;
            };
            layer.frame += 1;
            layer.animating = still || layer.max_frames.is_some();
            layer.needs_frame = layer.animating;
            if layer.max_frames.is_some_and(|max| layer.frame >= max) {
                let (id, controller) = (layer.id, layer.controller.clone());
                self.layers.remove(&wid);
                (self.on_lifecycle)(wid, WindowLifecycle::Closed);
                controller.destroy(id);
            }
        }
        let any = self.windows.values().any(|w| w.animating)
            || self.layers.values().any(|l| l.animating || l.needs_frame);
        event_loop.set_control_flow(if any {
            ControlFlow::Poll
        } else {
            ControlFlow::Wait
        });
        self.exit_if_empty(event_loop);
    }

    /// The app closes a layer surface (`close()`): `close_requested` first, which may
    /// keep it open.
    pub(crate) fn close_layer(&mut self, event_loop: &ActiveEventLoop, wid: WindowId) {
        if !self.layers.contains_key(&wid) {
            return;
        }
        if (self.on_lifecycle)(wid, WindowLifecycle::CloseRequested)
            && let Some(layer) = self.layers.remove(&wid)
        {
            self.notice(wid, layer.token, LayerNotice::Closed(CloseReason::App));
            (self.on_lifecycle)(wid, WindowLifecycle::Closed);
            layer.controller.destroy(layer.id);
            self.exit_if_empty(event_loop);
        }
    }

    /// Every layer surface wants a frame (a wake from another thread).
    pub(crate) fn wake_layers(&mut self) {
        for layer in self.layers.values_mut() {
            layer.needs_frame = true;
        }
    }

    /// Gives every open layer surface an event (an OS preference changed).
    pub(crate) fn tell_layers(&mut self, event: InputEvent) {
        let ids: Vec<WindowId> = self
            .layers
            .iter()
            .filter(|(_, l)| l.opened)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            (self.on_input)(id, event.clone());
            if let Some(layer) = self.layers.get_mut(&id) {
                layer.needs_frame = true;
            }
        }
    }
}
