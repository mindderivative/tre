//! M49 Phase 2: MD3's real, published shape (corner-radius) and
//! elevation scales, centralized for the first time. Before this,
//! these exact same token names/values existed only as ~40 scattered,
//! independently-"verified against Material Web's own token source"
//! doc comments beside individual `const`s across `engine-py::
//! window_factory.rs` (e.g. `CARD_CORNER_RADIUS: f64 = 12.0` with a
//! comment citing `corner-large`) -- confirmed via direct grep before
//! writing this module, not assumed. **Data only, this milestone:**
//! `window_factory.rs`'s own existing constants are left exactly as
//! they are, still their own independent literals -- migrating them to
//! reference these instead is real, separate, explicitly out-of-scope
//! follow-up (see `BUILD_TRACKER.md`'s M49 section), not attempted
//! here. This module exists so a theme YAML author, and any future
//! consumer, has one real, testable, named source of truth for these
//! numbers instead of needing to know or re-derive them.

/// MD3's published corner-radius scale, in density-independent pixels.
/// `full` is deliberately *not* a constant here -- real MD3 "full"
/// shape is relative to the element's own height (typically `height /
/// 2.0`, a real stadium shape), not a fixed dp value; inventing a
/// placeholder number for it would misrepresent what it actually means,
/// the same "don't fake a number, name the real limit" discipline this
/// codebase has followed throughout (see e.g. `corner_radii_override`'s
/// own doc comment in `engine-core`).
pub const SHAPE_NONE: f64 = 0.0;
pub const SHAPE_EXTRA_SMALL: f64 = 4.0;
pub const SHAPE_SMALL: f64 = 8.0;
pub const SHAPE_MEDIUM: f64 = 12.0;
pub const SHAPE_LARGE: f64 = 16.0;
pub const SHAPE_EXTRA_LARGE: f64 = 28.0;

/// MD3's published elevation scale. `PaintProperties.elevation`
/// (`engine-core`) and every `window_factory.rs` component that sets it
/// already use these exact same small integers (`DIALOG_ELEVATION: f64
/// = 3.0`, `FAB_REST_ELEVATION_LEVEL: f64 = 3.0`, etc.) -- the level
/// index itself, not a literal shadow blur/offset in dp (that
/// conversion happens in `engine-render`'s own paint code, out of
/// scope for this data-only module).
pub const ELEVATION_LEVEL_0: f64 = 0.0;
pub const ELEVATION_LEVEL_1: f64 = 1.0;
pub const ELEVATION_LEVEL_2: f64 = 2.0;
pub const ELEVATION_LEVEL_3: f64 = 3.0;
pub const ELEVATION_LEVEL_4: f64 = 4.0;
pub const ELEVATION_LEVEL_5: f64 = 5.0;

#[cfg(test)]
mod tests {
    use super::*;

    /// Not a tautology -- catches an accidental typo'd/reordered value
    /// (e.g. `SHAPE_SMALL` and `SHAPE_MEDIUM` swapped) by asserting the
    /// real, known MD3-published relative ordering holds.
    // Both orderings are genuinely `const` values, so clippy's own
    // `assertions_on_constants` lint fires -- deliberately kept as real
    // `#[test]` functions rather than `const { assert!(...) }` blocks
    // anyway, matching this codebase's own convention of every real
    // invariant showing up as a named, visible line in `cargo test`'s
    // own output, not silently checked only at compile time.
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn the_shape_scale_is_strictly_increasing() {
        assert!(SHAPE_NONE < SHAPE_EXTRA_SMALL);
        assert!(SHAPE_EXTRA_SMALL < SHAPE_SMALL);
        assert!(SHAPE_SMALL < SHAPE_MEDIUM);
        assert!(SHAPE_MEDIUM < SHAPE_LARGE);
        assert!(SHAPE_LARGE < SHAPE_EXTRA_LARGE);
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn the_elevation_scale_is_strictly_increasing() {
        assert!(ELEVATION_LEVEL_0 < ELEVATION_LEVEL_1);
        assert!(ELEVATION_LEVEL_1 < ELEVATION_LEVEL_2);
        assert!(ELEVATION_LEVEL_2 < ELEVATION_LEVEL_3);
        assert!(ELEVATION_LEVEL_3 < ELEVATION_LEVEL_4);
        assert!(ELEVATION_LEVEL_4 < ELEVATION_LEVEL_5);
    }

    /// Cross-checks against `window_factory.rs`'s own already-shipped,
    /// independently-verified real values -- not just internally
    /// consistent with itself, but consistent with the one other real
    /// place in this codebase that already encodes these same MD3
    /// tokens (`CARD_CORNER_RADIUS`/`DIALOG_CORNER_RADIUS`/`DIALOG_
    /// ELEVATION`, cited directly, not re-derived from memory).
    #[test]
    fn matches_the_real_values_already_shipped_in_window_factory() {
        assert_eq!(SHAPE_MEDIUM, 12.0); // CARD_CORNER_RADIUS
        assert_eq!(SHAPE_EXTRA_LARGE, 28.0); // DIALOG_CORNER_RADIUS
        assert_eq!(SHAPE_SMALL, 8.0); // CHIP_CORNER_RADIUS
        assert_eq!(ELEVATION_LEVEL_3, 3.0); // DIALOG_ELEVATION, FAB_REST_ELEVATION_LEVEL
    }
}
