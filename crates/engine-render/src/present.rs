//! 0.5.4 (#101): which `wgpu` present mode a window's swapchain uses.
//!
//! `Surface::get_default_config` takes the first mode the driver lists. On
//! Linux with Mesa that list is `[Mailbox, Fifo, Immediate]`, and `Mailbox`
//! never waits for the display: a window with one animating box then renders
//! thousands of frames a second and uses a whole CPU core (measured: 97.5% of
//! a core, against 8.2% with vsync). So the engine chooses the mode itself.

/// 0.5.4 (#128): the surface format to render into, out of those it supports.
///
/// The renderer writes colours already encoded for display (an app's
/// `(103, 80, 164)` is stored as those bytes), so the target must not be an
/// sRGB format: the hardware would encode them a second time on write, and
/// every colour on screen came out lighter than asked (measured: that fill
/// showed as `(170, 152, 210)`). `get_default_config` takes the driver's first
/// choice, which on Linux and most desktops is an sRGB format. Takes the first
/// format that is not, or `None` if every one is (the caller keeps its default).
pub fn linear_surface_format(supported: &[wgpu::TextureFormat]) -> Option<wgpu::TextureFormat> {
    supported.iter().copied().find(|format| !format.is_srgb())
}

/// What an app asks of the swapchain.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PresentChoice {
    /// One frame per display refresh, never tearing, and the loop waits for
    /// the display between frames. The default.
    #[default]
    Vsync,
    /// The newest frame replaces any waiting one (`Mailbox`), so a frame is
    /// never a refresh old; the loop is not paced, so it runs as fast as it
    /// can while something animates. Falls back to [`Vsync`](Self::Vsync)
    /// where the surface has no such mode.
    LowLatency,
}

impl PresentChoice {
    /// The name Python uses.
    pub fn name(self) -> &'static str {
        match self {
            Self::Vsync => "vsync",
            Self::LowLatency => "low_latency",
        }
    }

    /// The choice a Python name stands for.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "vsync" => Some(Self::Vsync),
            "low_latency" => Some(Self::LowLatency),
            _ => None,
        }
    }

    /// The present mode to configure a surface with, given the modes it
    /// supports. `AutoVsync` is always valid (`wgpu` picks `FifoRelaxed` or
    /// `Fifo`), so it is the answer for [`Vsync`](Self::Vsync) and the
    /// fallback for [`LowLatency`](Self::LowLatency).
    pub fn mode(self, supported: &[wgpu::PresentMode]) -> wgpu::PresentMode {
        match self {
            Self::LowLatency if supported.contains(&wgpu::PresentMode::Mailbox) => {
                wgpu::PresentMode::Mailbox
            }
            _ => wgpu::PresentMode::AutoVsync,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::PresentMode::{AutoVsync, Fifo, Immediate, Mailbox};

    /// What Mesa on Wayland reports, in its order: the order `wgpu` would
    /// have taken the first of.
    const MESA: &[wgpu::PresentMode] = &[Mailbox, Fifo, Immediate];

    #[test]
    fn vsync_is_the_default_and_never_picks_a_mode_that_does_not_wait() {
        assert_eq!(PresentChoice::default(), PresentChoice::Vsync);
        assert_eq!(PresentChoice::Vsync.mode(MESA), AutoVsync);
        assert_eq!(
            PresentChoice::Vsync.mode(&[Immediate, Mailbox, Fifo]),
            AutoVsync
        );
        assert_eq!(PresentChoice::Vsync.mode(&[]), AutoVsync);
    }

    #[test]
    fn low_latency_is_mailbox_where_there_is_one() {
        assert_eq!(PresentChoice::LowLatency.mode(MESA), Mailbox);
    }

    #[test]
    fn low_latency_falls_back_to_vsync_and_never_to_tearing() {
        assert_eq!(PresentChoice::LowLatency.mode(&[Fifo]), AutoVsync);
        assert_eq!(
            PresentChoice::LowLatency.mode(&[Fifo, Immediate]),
            AutoVsync
        );
    }

    #[test]
    fn the_surface_format_is_never_srgb_where_a_choice_exists() {
        use wgpu::TextureFormat::{Bgra8Unorm, Bgra8UnormSrgb, Rgba8Unorm, Rgba8UnormSrgb};
        // What Mesa reports on X11 and on Wayland, in order.
        assert_eq!(
            linear_surface_format(&[Bgra8UnormSrgb, Bgra8Unorm]),
            Some(Bgra8Unorm)
        );
        assert_eq!(
            linear_surface_format(&[Rgba8UnormSrgb, Bgra8UnormSrgb, Rgba8Unorm, Bgra8Unorm]),
            Some(Rgba8Unorm)
        );
        assert_eq!(linear_surface_format(&[Bgra8UnormSrgb]), None);
        assert_eq!(linear_surface_format(&[]), None);
    }

    #[test]
    fn the_names_round_trip() {
        for choice in [PresentChoice::Vsync, PresentChoice::LowLatency] {
            assert_eq!(PresentChoice::from_name(choice.name()), Some(choice));
        }
        assert_eq!(PresentChoice::from_name("mailbox"), None);
        assert_eq!(PresentChoice::from_name(""), None);
    }
}
