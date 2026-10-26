//! 0.5.9 (#220): one `wgpu` instance, adapter and device for every window of the process.
//!
//! Each window used to make its own (`GpuState::new`): a `wgpu::Instance`, an adapter, a device
//! and a queue. After the small memory blocks (#219) a further window still cost about 13 MiB of
//! video memory and 11 MiB of mappable memory, and the time to create an instance and a device.
//! Now the first window makes them and later windows take clones of its handles (`Instance`,
//! `Adapter`, `Device` and `Queue` are reference-counted), so a further window holds only its own
//! surface, render target and renderer.
//!
//! The shared handles live as long as some window uses them: the registry here holds a `Weak`, so
//! when the last window closes the device goes with it and the next window starts afresh. A device
//! reported lost is never handed out again. A window whose surface the shared adapter cannot
//! present to (a second GPU, say) gets a device of its own, as before.
//!
//! `TRE_GPU_DEVICES=per-window` restores one device per window (`shared`, or nothing, is the
//! default). With one device, `frame_stats()["memory"]` is the whole device's: every window
//! reads the same totals.

use std::sync::{Arc, Mutex, Weak};

use engine_render::GpuHub;

/// The GPU handles windows share.
pub(crate) struct SharedGpu {
    pub(crate) instance: wgpu::Instance,
    pub(crate) adapter: wgpu::Adapter,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    /// The device's health: one handler pair, one watch per window.
    pub(crate) hub: GpuHub,
}

static CURRENT: Mutex<Weak<SharedGpu>> = Mutex::new(Weak::new());

/// Whether windows share a device: yes, unless `TRE_GPU_DEVICES` says `per-window`.
pub(crate) fn sharing_enabled() -> bool {
    let setting = std::env::var("TRE_GPU_DEVICES").ok();
    if let Some(value) = setting.as_deref()
        && !matches!(value, "shared" | "per-window")
    {
        tracing::warn!(
            value,
            "TRE_GPU_DEVICES is \"shared\" or \"per-window\"; using \"shared\""
        );
    }
    sharing_for(setting.as_deref())
}

fn sharing_for(setting: Option<&str>) -> bool {
    setting != Some("per-window")
}

/// The device the process's windows share right now, if there is one that can still be used.
pub(crate) fn current() -> Option<Arc<SharedGpu>> {
    let shared = CURRENT.lock().unwrap().upgrade()?;
    (!shared.hub.is_lost()).then_some(shared)
}

/// Makes `shared` the device later windows take.
pub(crate) fn publish(shared: &Arc<SharedGpu>) {
    *CURRENT.lock().unwrap() = Arc::downgrade(shared);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devices_are_shared_unless_a_window_each_is_asked_for() {
        assert!(sharing_for(None));
        assert!(sharing_for(Some("shared")));
        assert!(!sharing_for(Some("per-window")));
        // A mistake keeps the default, which is the cheaper one.
        assert!(sharing_for(Some("per_window")));
        assert!(sharing_for(Some("")));
    }
}
