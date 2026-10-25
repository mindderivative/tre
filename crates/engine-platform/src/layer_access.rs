//! 0.5.7 (#200): a layer surface's accessibility tree, through `accesskit_unix`.
//!
//! `accesskit_winit` needs a `winit` window, and a layer surface has none, so a bar was
//! invisible to a screen reader. The Linux backend underneath, `accesskit_unix`, does not need
//! one: its `Adapter` takes three handlers and shares the process's one AT-SPI application
//! object with `winit`'s adapters, so a bar is another top-level window of the same app.
//!
//! The handlers run on another thread, and the tree is built on the loop thread (it reads the
//! Python-owned nodes), so they only post a `PlatformEvent` through the loop's proxy and the loop
//! answers: `request_initial_tree` returns `None` and the loop thread then calls
//! `update_if_active` with a full tree, as `accesskit_winit` does for a window.

use accesskit::{ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, TreeUpdate};
use accesskit_unix::Adapter;
use winit::event_loop::EventLoopProxy;
use winit::window::WindowId;

use crate::PlatformEvent;

/// What an assistive technology asked of a layer surface.
pub(crate) enum LayerAccessEvent {
    /// A screen reader connected and wants the tree.
    InitialTree,
    /// It asked the surface to do something (click a button, focus a field).
    Action(ActionRequest),
}

#[derive(Clone)]
struct Handler {
    proxy: EventLoopProxy<PlatformEvent>,
    window: WindowId,
}

impl Handler {
    fn send(&self, event: LayerAccessEvent) {
        let _ = self
            .proxy
            .send_event(PlatformEvent::LayerAccess(self.window, event));
    }
}

impl ActivationHandler for Handler {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        // Built on the loop thread, which owns the nodes; see the module doc.
        self.send(LayerAccessEvent::InitialTree);
        None
    }
}

impl ActionHandler for Handler {
    fn do_action(&mut self, request: ActionRequest) {
        self.send(LayerAccessEvent::Action(request));
    }
}

impl DeactivationHandler for Handler {
    fn deactivate_accessibility(&mut self) {
        // `update_if_active` already does nothing once the adapter is inactive.
    }
}

/// The adapter for the layer surface whose loop-side id is `window`.
pub(crate) fn adapter(proxy: EventLoopProxy<PlatformEvent>, window: WindowId) -> Adapter {
    let handler = Handler { proxy, window };
    Adapter::new(handler.clone(), handler.clone(), handler)
}
