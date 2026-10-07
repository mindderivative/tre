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
    use wgpu::TextureFormat::{Bgra8Unorm, Rgba8Unorm};
    // The 8-bit formats the renderer draws in come first: a ten-bit-colour
    // format (`Rgb10a2Unorm`, which Mesa lists ahead of them on Wayland) has
    // two bits of alpha, which a see-through window shows as four-step
    // blending (0.5.4, #137).
    [Bgra8Unorm, Rgba8Unorm]
        .into_iter()
        .find(|format| supported.contains(format))
        .or_else(|| supported.iter().copied().find(|format| !format.is_srgb()))
}

/// 0.5.4 (#137): the alpha mode for a see-through window, out of those the
/// surface supports, or `None` where it can't blend with the desktop at all.
/// The renderer writes premultiplied alpha, so `PreMultiplied` is exact;
/// `Inherit` leaves the choice to the window system, which expects
/// premultiplied; `PostMultiplied` (straight alpha) would draw wrong edges
/// and is never chosen.
pub fn transparent_alpha_mode(
    supported: &[wgpu::CompositeAlphaMode],
) -> Option<wgpu::CompositeAlphaMode> {
    use wgpu::CompositeAlphaMode::{Inherit, PreMultiplied};
    [PreMultiplied, Inherit]
        .into_iter()
        .find(|mode| supported.contains(mode))
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

/// How long after the last resize a window keeps its resize pacing: long
/// enough that a sweep with brief pauses is one resize, short enough that an
/// animation after it is paced to the display again at once.
pub const RESIZE_HOLD: std::time::Duration = std::time::Duration::from_millis(400);

/// 0.5.5 (#155): whether a window is being resized, and so presents without
/// vsync for the moment.
///
/// A vsync present on Wayland commits with a `wp_fifo_v1` barrier, and a
/// resize builds a new swapchain, so a new fifo object, every frame. KDE's
/// compositor then waited 84 to 565 ms, in the middle of a resize, to send the
/// next configure after a barriered commit (traced: 2,048 commits, five waits
/// over 50 ms; none of ~6,300 commits without the barrier waited over 152 ms).
/// The window trailed the pointer and went on moving after the button was
/// released. While resizing, the window uses the mode `low_latency` selects,
/// which carries no barrier; a short while after the last resize it goes back
/// to the app's choice. Pure state, so it is tested without a window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResizePacing {
    /// When the window last needed a new size, while that is recent.
    last: Option<std::time::Instant>,
}

impl ResizePacing {
    /// The window is being given a new size at `now`.
    pub fn resized(&mut self, now: std::time::Instant) {
        self.last = Some(now);
    }

    /// Whether resize pacing is in effect (a resize is in progress, or ended
    /// less than [`RESIZE_HOLD`] ago). The loop keeps running while it is, so
    /// it notices when the hold is over.
    pub fn active(&self) -> bool {
        self.last.is_some()
    }

    /// Ends resize pacing once `now` is [`RESIZE_HOLD`] past the last resize.
    /// `true` exactly once, when it ends: the caller then restores the mode.
    pub fn settle(&mut self, now: std::time::Instant) -> bool {
        match self.last {
            Some(last) if now.saturating_duration_since(last) >= RESIZE_HOLD => {
                self.last = None;
                true
            }
            _ => false,
        }
    }

    /// What to present with, given the app's `steady` choice: low latency
    /// while resizing, the app's choice otherwise.
    pub fn choice(&self, steady: PresentChoice) -> PresentChoice {
        if self.active() {
            PresentChoice::LowLatency
        } else {
            steady
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
            Some(Bgra8Unorm)
        );
        // Never a ten-bit-colour format while an 8-bit one is there.
        assert_eq!(
            linear_surface_format(&[
                Rgba8UnormSrgb,
                Bgra8UnormSrgb,
                wgpu::TextureFormat::Rgb10a2Unorm,
                Rgba8Unorm,
                Bgra8Unorm
            ]),
            Some(Bgra8Unorm)
        );
        assert_eq!(
            linear_surface_format(&[wgpu::TextureFormat::Rgb10a2Unorm]),
            Some(wgpu::TextureFormat::Rgb10a2Unorm),
            "better than nothing"
        );
        assert_eq!(linear_surface_format(&[Bgra8UnormSrgb]), None);
        assert_eq!(linear_surface_format(&[]), None);
    }

    #[test]
    fn a_transparent_window_wants_premultiplied_alpha_and_never_straight() {
        use wgpu::CompositeAlphaMode::{Auto, Inherit, Opaque, PostMultiplied, PreMultiplied};
        assert_eq!(
            transparent_alpha_mode(&[Opaque, PreMultiplied, Inherit]),
            Some(PreMultiplied)
        );
        assert_eq!(transparent_alpha_mode(&[Opaque, Inherit]), Some(Inherit));
        assert_eq!(transparent_alpha_mode(&[Opaque, PostMultiplied]), None);
        assert_eq!(transparent_alpha_mode(&[Auto, Opaque]), None);
        assert_eq!(transparent_alpha_mode(&[]), None);
    }

    #[test]
    fn the_names_round_trip() {
        for choice in [PresentChoice::Vsync, PresentChoice::LowLatency] {
            assert_eq!(PresentChoice::from_name(choice.name()), Some(choice));
        }
        assert_eq!(PresentChoice::from_name("mailbox"), None);
        assert_eq!(PresentChoice::from_name(""), None);
    }

    #[test]
    fn a_window_is_paced_without_vsync_only_while_it_is_being_resized() {
        use std::time::{Duration, Instant};
        let t0 = Instant::now();
        let ms = Duration::from_millis;
        let mut pacing = ResizePacing::default();
        assert!(!pacing.active(), "a window at rest keeps the app's choice");
        assert_eq!(pacing.choice(PresentChoice::Vsync), PresentChoice::Vsync);
        pacing.resized(t0);
        assert!(pacing.active());
        assert_eq!(
            pacing.choice(PresentChoice::Vsync),
            PresentChoice::LowLatency
        );
        // Not yet over, however often it is asked.
        assert!(!pacing.settle(t0 + ms(399)));
        assert!(pacing.active());
        // A later resize starts the hold again.
        pacing.resized(t0 + ms(300));
        assert!(!pacing.settle(t0 + ms(699)));
        // Over, once, and then at rest again.
        assert!(pacing.settle(t0 + ms(700)));
        assert!(!pacing.active());
        assert!(!pacing.settle(t0 + ms(5_000)), "it ends only once");
        assert_eq!(pacing.choice(PresentChoice::Vsync), PresentChoice::Vsync);
    }

    #[test]
    fn resizing_never_makes_a_low_latency_window_wait_for_the_display() {
        use std::time::Instant;
        let mut pacing = ResizePacing::default();
        pacing.resized(Instant::now());
        // Where the surface has Mailbox the resize uses it, with no vsync barrier;
        // where it has none the fallback is vsync, never a tearing mode.
        assert_eq!(pacing.choice(PresentChoice::Vsync).mode(MESA), Mailbox);
        assert_eq!(pacing.choice(PresentChoice::Vsync).mode(&[Fifo]), AutoVsync);
        assert_eq!(pacing.choice(PresentChoice::LowLatency).mode(MESA), Mailbox);
        // An app that already asked for low latency sees no change at all.
        let steady = PresentChoice::LowLatency;
        assert_eq!(PresentChoice::default().mode(MESA), AutoVsync);
        assert_eq!(ResizePacing::default().choice(steady), steady);
    }
}
