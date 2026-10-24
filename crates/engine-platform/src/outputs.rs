//! 0.5.7 (#202): the screens (outputs) the compositor has, as plain data.
//!
//! Positions and sizes are *logical* pixels in the desktop's layout (what a layer surface
//! and a pointer position use); `scale_factor` is the ratio of physical to logical pixels
//! (1.5 on a 1.5x output), not the integer `wl_output` scale. The Wayland side that fills
//! this in is `layer`; this part has no Wayland in it, so its conversions are tested
//! everywhere.

/// One output, as of the moment it was read.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputInfo {
    /// The connector name (`"DP-1"`, `"HDMI-A-1"`): stable across runs, unlike the
    /// compositor's own numbering.
    pub name: String,
    pub description: String,
    /// The output's top-left in the desktop's logical layout.
    pub x: i32,
    pub y: i32,
    /// The output's logical size.
    pub width: u32,
    pub height: u32,
    /// Physical over logical pixels.
    pub scale_factor: f64,
    /// `"normal"`, `"90"`, `"180"`, `"270"`, or one of those with `"flipped_"` before it.
    pub transform: String,
}

/// The effective scale: the current mode's physical size over the logical size, rounded to
/// 1/120 (the unit the fractional-scale protocol speaks), so 1.4999 reads as 1.5. A rotated
/// output (`"90"`, `"270"`, and their flipped forms) swaps the mode's axes first. Falls back
/// to `integer_scale` (`wl_output`'s own) when either size is unknown or zero.
pub fn effective_scale(
    mode: Option<(i32, i32)>,
    logical: Option<(i32, i32)>,
    transform: &str,
    integer_scale: i32,
) -> f64 {
    let fallback = f64::from(integer_scale.max(1));
    let (Some((mw, mh)), Some((lw, lh))) = (mode, logical) else {
        return fallback;
    };
    if mw <= 0 || mh <= 0 || lw <= 0 || lh <= 0 {
        return fallback;
    }
    let rotated = transform.ends_with("90") || transform.ends_with("270");
    let (mode_width, mode_height) = if rotated { (mh, mw) } else { (mw, mh) };
    let by_width = f64::from(mode_width) / f64::from(lw);
    let by_height = f64::from(mode_height) / f64::from(lh);
    // The two ratios agree for a real output; a compositor that rounds a logical size
    // can make them differ in the last digit, so take the one nearer a whole 1/120.
    let ratio = (by_width + by_height) / 2.0;
    (ratio * 120.0).round() / 120.0
}

/// The names the Python layer shows for a Wayland transform, by its protocol value
/// (`wl_output.transform`: 0 normal, 1 to 3 rotations, 4 flipped, 5 to 7 flipped and
/// rotated).
pub fn transform_name(value: u32) -> &'static str {
    match value {
        0 => "normal",
        1 => "90",
        2 => "180",
        3 => "270",
        4 => "flipped_normal",
        5 => "flipped_90",
        6 => "flipped_180",
        7 => "flipped_270",
        _ => "normal",
    }
}

/// What changed in the set of outputs.
#[derive(Clone, Debug, PartialEq)]
pub enum OutputEvent {
    Added(OutputInfo),
    /// Moved, resized, rescaled or rotated.
    Updated(OutputInfo),
    Removed(OutputInfo),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_is_the_mode_over_the_logical_size() {
        assert_eq!(
            effective_scale(Some((1920, 1080)), Some((1920, 1080)), "normal", 1),
            1.0
        );
        assert_eq!(
            effective_scale(Some((3840, 2160)), Some((1920, 1080)), "normal", 2),
            2.0
        );
        assert_eq!(
            effective_scale(Some((2880, 1800)), Some((1920, 1200)), "normal", 2),
            1.5
        );
        // A logical size the compositor rounded still reads as 1.5.
        assert_eq!(
            effective_scale(Some((2560, 1440)), Some((1707, 960)), "normal", 2),
            1.5
        );
    }

    #[test]
    fn a_rotated_output_swaps_the_modes_axes() {
        assert_eq!(
            effective_scale(Some((1920, 1080)), Some((1080, 1920)), "90", 1),
            1.0
        );
        assert_eq!(
            effective_scale(Some((3840, 2160)), Some((1080, 1920)), "flipped_270", 2),
            2.0
        );
    }

    #[test]
    fn without_sizes_it_falls_back_to_the_integer_scale() {
        assert_eq!(effective_scale(None, Some((1920, 1080)), "normal", 2), 2.0);
        assert_eq!(effective_scale(Some((1920, 1080)), None, "normal", 1), 1.0);
        assert_eq!(
            effective_scale(Some((0, 0)), Some((0, 0)), "normal", 0),
            1.0
        );
    }

    #[test]
    fn transforms_have_names() {
        assert_eq!(transform_name(0), "normal");
        assert_eq!(transform_name(3), "270");
        assert_eq!(transform_name(5), "flipped_90");
        assert_eq!(transform_name(99), "normal");
    }
}
