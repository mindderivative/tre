//! 0.5.8 (#201): tre's own `zwlr_layer_shell_v1` binding, at version 5.
//!
//! `smithay-client-toolkit` 0.19 (the copy `winit` shares) binds the layer shell only up to
//! version 4, and `set_exclusive_edge` (a corner-anchored surface's zone is for one of its
//! two edges) is version 5. The layer thread owns every layer surface already, so this is
//! the toolkit's wrapper done here: bind the global, make the surface, acknowledge each
//! `configure` before the app is told, destroy on drop. One stack, no second toolkit copy.

use smithay_client_toolkit::reexports::client::globals::{BindError, GlobalList};
use smithay_client_toolkit::reexports::client::protocol::{wl_output, wl_surface};
use smithay_client_toolkit::reexports::client::{Dispatch, Proxy, QueueHandle};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::{self, ZwlrLayerShellV1};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    self, ZwlrLayerSurfaceV1,
};

pub(crate) use zwlr_layer_shell_v1::Layer;
pub(crate) use zwlr_layer_surface_v1::{Anchor, KeyboardInteractivity};

/// The bound global, at the highest version the compositor offers (1 to 5).
pub(crate) struct LayerShell {
    shell: ZwlrLayerShellV1,
}

/// What each surface's events are dispatched with: which `wl_surface` it is.
pub(crate) struct LayerData {
    pub(crate) surface: wl_surface::WlSurface,
}

impl LayerShell {
    pub(crate) fn bind<D>(globals: &GlobalList, qh: &QueueHandle<D>) -> Result<Self, BindError>
    where
        D: Dispatch<ZwlrLayerShellV1, ()> + 'static,
    {
        Ok(Self {
            shell: globals.bind(qh, 1..=5, ())?,
        })
    }

    pub(crate) fn create_layer_surface<D>(
        &self,
        qh: &QueueHandle<D>,
        surface: &wl_surface::WlSurface,
        layer: Layer,
        namespace: &str,
        output: Option<&wl_output::WlOutput>,
    ) -> LayerSurface
    where
        D: Dispatch<ZwlrLayerSurfaceV1, LayerData> + 'static,
    {
        let proxy = self.shell.get_layer_surface(
            surface,
            output,
            layer,
            namespace.to_string(),
            qh,
            LayerData {
                surface: surface.clone(),
            },
        );
        LayerSurface {
            proxy,
            surface: surface.clone(),
        }
    }
}

/// A layer surface. Every setting is double buffered: it takes effect at the next commit.
pub(crate) struct LayerSurface {
    proxy: ZwlrLayerSurfaceV1,
    surface: wl_surface::WlSurface,
}

impl LayerSurface {
    pub(crate) fn set_layer(&self, layer: Layer) {
        self.proxy.set_layer(layer);
    }

    pub(crate) fn set_anchor(&self, anchor: Anchor) {
        self.proxy.set_anchor(anchor);
    }

    pub(crate) fn set_size(&self, width: u32, height: u32) {
        self.proxy.set_size(width, height);
    }

    pub(crate) fn set_exclusive_zone(&self, zone: i32) {
        self.proxy.set_exclusive_zone(zone);
    }

    pub(crate) fn set_margin(&self, top: i32, right: i32, bottom: i32, left: i32) {
        self.proxy.set_margin(top, right, bottom, left);
    }

    pub(crate) fn set_keyboard_interactivity(&self, mode: KeyboardInteractivity) {
        self.proxy.set_keyboard_interactivity(mode);
    }

    /// The edge a corner-anchored surface's exclusive zone is for (one `Anchor` bit). Needs
    /// version 5; returns whether the request was sent, so a caller can tell the app when
    /// the compositor is older.
    pub(crate) fn set_exclusive_edge(&self, edge: Anchor) -> bool {
        if self.proxy.version() < 5 {
            return false;
        }
        self.proxy.set_exclusive_edge(edge);
        true
    }

    /// Parents an `xdg_popup` (made with a null xdg parent) to this surface. Before its first commit.
    pub(crate) fn get_popup(
        &self,
        popup: &smithay_client_toolkit::reexports::protocols::xdg::shell::client::xdg_popup::XdgPopup,
    ) {
        self.proxy.get_popup(popup);
    }

    pub(crate) fn commit(&self) {
        self.surface.commit();
    }
}

impl Drop for LayerSurface {
    fn drop(&mut self) {
        self.proxy.destroy();
    }
}
