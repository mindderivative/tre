//! `AtlasKey` and the packed `(rect, generation)` slot-value encoding
//! (ARCHITECTURE.md Section 2.3's `AtlasSlot` comment: "Packed (u,v,w,h)
//! rect + generation counter"). The generic open-addressing/hashing
//! mechanics live in `tre_memory::SwmrSlotTable`; this module supplies
//! only what's genuinely atlas-specific on top of it.

use crate::PackedRect;

/// Identifies one atlas-resident entry -- ARCHITECTURE.md's own comment:
/// "(font_id, glyph_id) or icon identifier." Deliberately opaque (a
/// single `u64`) rather than a `(font_id, glyph_id)` struct field pair,
/// so this crate stays content-agnostic (Step 4.2.1's own precedent):
/// `tre-atlas` has no idea a glyph key and an icon key are packed
/// differently, only that each `AtlasKey` is a distinct `u64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtlasKey(u64);

impl AtlasKey {
    /// A convenience constructor for the glyph case named in
    /// ARCHITECTURE.md's own comment -- `font_id`/`glyph_id` packed into
    /// one `u64` with no collision risk, since both halves keep their
    /// own full 32 bits.
    #[must_use]
    pub fn from_glyph(font_id: u32, glyph_id: u32) -> Self {
        Self((u64::from(font_id) << 32) | u64::from(glyph_id))
    }
}

impl From<AtlasKey> for u64 {
    fn from(key: AtlasKey) -> u64 {
        key.0
    }
}

// 12 bits per coordinate covers the stated production atlas size
// (DESIGN.md/IMPLEMENTATION.md: 4096x4096, and 4095 is the largest value
// 12 bits can hold) with room to spare for this step's own much smaller
// demo atlas, leaving the remaining 16 of the 64 bits for the generation
// counter ARCHITECTURE.md's own `AtlasSlot` comment calls for.
const COORD_BITS: u32 = 12;
const COORD_MASK: u64 = (1 << COORD_BITS) - 1;

/// Packs `rect` and `generation` into the raw `u64` payload
/// [`tre_memory::SwmrSlotTable`] stores. `generation` is included for
/// format forward-compatibility with eventual LRU eviction/reuse
/// (DESIGN.md Section 10.2) -- nothing in this step actually reuses a
/// slot yet, so every real caller this step passes `0`.
///
/// # Panics
///
/// Panics if any of `rect`'s fields, or `generation`, exceeds what its
/// allotted bits can hold.
#[must_use]
pub fn pack_slot_value(rect: PackedRect, generation: u16) -> u64 {
    for (name, value) in [
        ("x", rect.x),
        ("y", rect.y),
        ("width", rect.width),
        ("height", rect.height),
    ] {
        assert!(
            u64::from(value) <= COORD_MASK,
            "atlas rect field {name} ({value}) exceeds the {COORD_BITS}-bit packed range"
        );
    }
    u64::from(rect.x)
        | (u64::from(rect.y) << COORD_BITS)
        | (u64::from(rect.width) << (COORD_BITS * 2))
        | (u64::from(rect.height) << (COORD_BITS * 3))
        | (u64::from(generation) << (COORD_BITS * 4))
}

/// The inverse of [`pack_slot_value`].
#[must_use]
pub fn unpack_slot_value(value: u64) -> (PackedRect, u16) {
    #[allow(
        clippy::cast_possible_truncation,
        reason = "each field is masked down to COORD_BITS (12) bits first, well within u32"
    )]
    let rect = PackedRect {
        x: (value & COORD_MASK) as u32,
        y: ((value >> COORD_BITS) & COORD_MASK) as u32,
        width: ((value >> (COORD_BITS * 2)) & COORD_MASK) as u32,
        height: ((value >> (COORD_BITS * 3)) & COORD_MASK) as u32,
    };
    #[allow(
        clippy::cast_possible_truncation,
        reason = "the generation counter occupies exactly the remaining 16 bits"
    )]
    let generation = (value >> (COORD_BITS * 4)) as u16;
    (rect, generation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_and_unpack_round_trips() {
        let rect = PackedRect {
            x: 100,
            y: 200,
            width: 32,
            height: 48,
        };
        let packed = pack_slot_value(rect, 7);
        assert_eq!(unpack_slot_value(packed), (rect, 7));
    }

    #[test]
    fn glyph_keys_from_distinct_font_glyph_pairs_never_collide() {
        let a = AtlasKey::from_glyph(1, 100);
        let b = AtlasKey::from_glyph(1, 101);
        let c = AtlasKey::from_glyph(2, 100);
        assert_ne!(u64::from(a), u64::from(b));
        assert_ne!(u64::from(a), u64::from(c));
        assert_ne!(u64::from(b), u64::from(c));
    }

    #[test]
    #[should_panic(expected = "exceeds the 12-bit packed range")]
    fn pack_slot_value_rejects_a_coordinate_too_large_to_fit() {
        let _ = pack_slot_value(
            PackedRect {
                x: 5000,
                y: 0,
                width: 1,
                height: 1,
            },
            0,
        );
    }
}
