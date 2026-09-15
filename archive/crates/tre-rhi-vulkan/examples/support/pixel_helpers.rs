//! Shared pixel-readback helper for headless-swapchain demos.
//!
//! Quality-review finding (Phase 1-4 review, 2026-09-06): REVIEW.md
//! finding #93 fixed one demo's `pixel_at` closure, which had returned
//! `read_pixels_bgra8`'s real BGRA memory bytes unswapped while comparing
//! them against an RGBA-ordered expected color. That fix was applied
//! locally to a handful of files instead of becoming a shared helper, so
//! several other demos still carried the same raw, unswapped closure --
//! invisible only because they happened to compare exclusively
//! channel-swap-invariant colors (white/black/self-referential). Every
//! demo that needs pixel readback now includes this module (via
//! `#[path = "pixel_helpers.rs"] mod pixel_helpers;` -- examples are
//! separate compilation units with no shared library crate to put this in
//! otherwise) instead of hand-rolling its own closure, so there is
//! exactly one place this channel swap can be gotten wrong.

/// Reads the pixel at `(x, y)` out of a `read_pixels_bgra8`-returned
/// buffer (real BGRA memory byte order, `width` pixels per row, 4 bytes
/// per pixel) and returns it as true `[R, G, B, A]` -- the swap any
/// comparison against an RGBA-ordered expected color needs.
#[allow(
    dead_code,
    reason = "not every example that includes this module calls every helper it defines"
)]
pub fn bgra_pixel_at(bgra: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let idx = ((y * width + x) * 4) as usize;
    [bgra[idx + 2], bgra[idx + 1], bgra[idx], bgra[idx + 3]]
}
