//! 0.5.7 (#219): how much memory `wgpu` may reserve per GPU device.
//!
//! `wgpu`'s default (`MemoryHints::Performance`) makes device memory blocks of at least 128 MiB
//! (host-visible ones 64 MiB) the moment a device exists, whatever is then put in them; tre makes
//! a device per window, so a 200 x 100 window held about 210 MiB of video memory to draw one box
//! (`docs/design/vram-per-window.md`). `MemoryHints::MemoryUsage` uses blocks of 8 to 64 MiB
//! instead (4 to 32 MiB host-visible): the same window is 31 MiB, large resources still get what
//! they ask for, and the frame time did not change in the measurements.
//!
//! `TRE_GPU_MEMORY=performance` brings the old reservation back; anything else, or nothing, is
//! the small blocks.

/// The memory hints for a new device: small blocks, unless `TRE_GPU_MEMORY` says `performance`.
pub(crate) fn memory_hints() -> wgpu::MemoryHints {
    let setting = std::env::var("TRE_GPU_MEMORY").ok();
    if let Some(value) = setting.as_deref()
        && !matches!(value, "small" | "performance")
    {
        tracing::warn!(
            value,
            "TRE_GPU_MEMORY is \"small\" or \"performance\"; using \"small\""
        );
    }
    hints_for(setting.as_deref())
}

fn hints_for(setting: Option<&str>) -> wgpu::MemoryHints {
    match setting {
        Some("performance") => wgpu::MemoryHints::Performance,
        _ => wgpu::MemoryHints::MemoryUsage,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_blocks_are_the_default_and_performance_is_opt_in() {
        assert!(matches!(hints_for(None), wgpu::MemoryHints::MemoryUsage));
        assert!(matches!(
            hints_for(Some("small")),
            wgpu::MemoryHints::MemoryUsage
        ));
        assert!(matches!(
            hints_for(Some("performance")),
            wgpu::MemoryHints::Performance
        ));
        // A mistake keeps the default rather than the large reservation.
        assert!(matches!(
            hints_for(Some("Performance")),
            wgpu::MemoryHints::MemoryUsage
        ));
        assert!(matches!(
            hints_for(Some("")),
            wgpu::MemoryHints::MemoryUsage
        ));
    }
}
