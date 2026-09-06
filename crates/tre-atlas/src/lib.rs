//! 2D Guillotine bin-packing (IMPLEMENTATION.md Step 4.2 task 1) -- a
//! free-rectangle list, Best-Area-Fit selection, and a guillotine split
//! that always partitions the leftover "L-shaped" region into exactly two
//! non-overlapping rectangles (never the overlapping-candidate-rectangle
//! set a MaxRects packer would keep). Shared by MSDF glyph atlas entries
//! and plain-color icon/vector-decal atlas entries alike (ARCHITECTURE.md
//! Section 2.3, DESIGN.md Section 10.2) -- not a text-specific concern,
//! hence its own crate rather than living in `tre-text`.
//!
//! Also (Step 4.2.4) the real multi-window atlas concurrency model built
//! on top of the packer: [`AtlasOwner`] is a dedicated background thread
//! draining [`AtlasInsertRequest`]s (via `tre_memory::MpscRingBuffer`)
//! from any number of producer threads, performing the real packing and
//! rasterization, and publishing results into a `tre_memory::SwmrSlotTable`
//! any reader can consult without ever blocking. No `unsafe` code lives
//! in this crate itself -- the concurrency primitives that need it live
//! in `tre-memory` (TECHNICAL.md Section 9.1's `unsafe` policy groups
//! them there, alongside the pre-existing input-event ring buffer).
#![forbid(unsafe_code)]

mod key;
mod owner;
mod raster;

pub use key::{pack_slot_value, unpack_slot_value, AtlasKey};
pub use owner::{AtlasOwner, AtlasOwnerHandle};
pub use raster::{AtlasInsertRequest, RasterSource};

/// A placed or free rectangle within an atlas, in atlas pixel coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackedRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl PackedRect {
    fn area(self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    /// True if `self` and `other` share any pixel -- used only by this
    /// crate's own tests to assert the packer's core invariant; the
    /// packer itself never needs to check placed rectangles against each
    /// other, since a correct guillotine split never hands out the same
    /// free space twice.
    #[must_use]
    pub fn overlaps(&self, other: &PackedRect) -> bool {
        self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }
}

/// A single atlas's free-space bookkeeping. Plain, single-threaded
/// `&mut self` state -- IMPLEMENTATION.md Step 4.2.4's atlas owner is the
/// only code ever allowed to touch one of these once multi-window
/// concurrency is built; this crate itself makes no threading claims at
/// all.
#[derive(Debug)]
pub struct AtlasPacker {
    free_rects: Vec<PackedRect>,
}

impl AtlasPacker {
    /// A new packer over a `width x height` atlas, entirely free.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            free_rects: vec![PackedRect {
                x: 0,
                y: 0,
                width,
                height,
            }],
        }
    }

    /// Finds space for a `width x height` rectangle and returns its
    /// placement, or `None` if no free rectangle is large enough. Never
    /// panics, never wraps -- a request this atlas cannot currently
    /// satisfy is a normal, expected outcome (DESIGN.md Section 2.6's
    /// placeholder-glyph fallback is a later step's response to it, not
    /// this method's).
    pub fn insert(&mut self, width: u32, height: u32) -> Option<PackedRect> {
        if width == 0 || height == 0 {
            return None;
        }

        // Best Area Fit: among every free rectangle big enough to hold
        // the request, use whichever has the smallest area -- the
        // tightest fit, minimizing space wasted on this one insertion.
        let mut best_index = None;
        let mut best_area = u64::MAX;
        for (index, free) in self.free_rects.iter().enumerate() {
            if free.width >= width && free.height >= height && free.area() < best_area {
                best_area = free.area();
                best_index = Some(index);
            }
        }
        let index = best_index?;
        let free = self.free_rects.swap_remove(index);
        let placed = PackedRect {
            x: free.x,
            y: free.y,
            width,
            height,
        };

        let (first, second) = split_leftover(free, width, height);
        self.free_rects.extend(first);
        self.free_rects.extend(second);
        Some(placed)
    }
}

/// Partitions whatever's left of `free` after placing a `placed_width x
/// placed_height` rectangle in its top-left corner into exactly two
/// non-overlapping rectangles (either may be absent if there's no
/// leftover along that side) -- a real guillotine cut, not the
/// overlapping free-space set a MaxRects packer keeps instead.
///
/// The leftover "L-shaped" region can be cut either way (a full-width
/// horizontal cut below the placed rectangle, or a full-height vertical
/// cut beside it); this always picks whichever cut leaves the single
/// larger resulting rectangle, computed by actually building both
/// candidate splits and comparing their larger piece's area -- not
/// approximated from the raw leftover dimensions alone, since (as a
/// worked example in this module's own tests shows) the split with the
/// smaller *leftover width* does not always produce the larger resulting
/// piece.
fn split_leftover(
    free: PackedRect,
    placed_width: u32,
    placed_height: u32,
) -> (Option<PackedRect>, Option<PackedRect>) {
    let leftover_width = free.width - placed_width;
    let leftover_height = free.height - placed_height;

    // Cut A: a full-width rectangle below the placed piece, and a sliver
    // to its right sized to the placed piece's own height.
    let a_right = (leftover_width > 0).then(|| PackedRect {
        x: free.x + placed_width,
        y: free.y,
        width: leftover_width,
        height: placed_height,
    });
    let a_below = (leftover_height > 0).then(|| PackedRect {
        x: free.x,
        y: free.y + placed_height,
        width: free.width,
        height: leftover_height,
    });

    // Cut B: a full-height rectangle to the right of the placed piece,
    // and a sliver below it sized to the placed piece's own width.
    let b_right = (leftover_width > 0).then(|| PackedRect {
        x: free.x + placed_width,
        y: free.y,
        width: leftover_width,
        height: free.height,
    });
    let b_below = (leftover_height > 0).then(|| PackedRect {
        x: free.x,
        y: free.y + placed_height,
        width: placed_width,
        height: leftover_height,
    });

    let larger_piece = |first: Option<PackedRect>, second: Option<PackedRect>| -> u64 {
        first
            .map(PackedRect::area)
            .unwrap_or(0)
            .max(second.map(PackedRect::area).unwrap_or(0))
    };

    if larger_piece(a_right, a_below) >= larger_piece(b_right, b_below) {
        (a_right, a_below)
    } else {
        (b_right, b_below)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_into_a_fresh_packer_places_at_the_origin() {
        let mut packer = AtlasPacker::new(64, 64);
        let placed = packer.insert(10, 10).expect("must fit in a fresh atlas");
        assert_eq!(
            placed,
            PackedRect {
                x: 0,
                y: 0,
                width: 10,
                height: 10
            }
        );
    }

    #[test]
    fn insert_returns_none_when_nothing_fits() {
        let mut packer = AtlasPacker::new(64, 64);
        assert!(packer.insert(65, 1).is_none());
        assert!(packer.insert(1, 65).is_none());
        assert!(packer.insert(0, 10).is_none());
    }

    #[test]
    fn a_varied_insertion_sequence_never_produces_overlapping_placements() {
        let mut packer = AtlasPacker::new(256, 256);
        // Deliberately varied sizes (not all-identical, which would hide
        // axis-choice bugs the split-heuristic comparison exists to
        // catch) mimicking a realistic mix of small glyph-sized and
        // larger icon-sized requests.
        let requests = [
            (32, 32),
            (16, 8),
            (64, 40),
            (8, 8),
            (100, 20),
            (20, 100),
            (12, 12),
            (50, 50),
            (5, 30),
            (30, 5),
        ];
        let mut placed = Vec::new();
        for (width, height) in requests {
            if let Some(rect) = packer.insert(width, height) {
                assert_eq!((rect.width, rect.height), (width, height));
                assert!(
                    rect.x + rect.width <= 256 && rect.y + rect.height <= 256,
                    "placement {rect:?} escapes the 256x256 atlas bounds"
                );
                placed.push(rect);
            }
        }
        assert!(
            placed.len() >= 8,
            "expected most of this modest request sequence to fit in a 256x256 atlas, only {} did",
            placed.len()
        );
        for i in 0..placed.len() {
            for j in (i + 1)..placed.len() {
                assert!(
                    !placed[i].overlaps(&placed[j]),
                    "placements {:?} and {:?} overlap",
                    placed[i],
                    placed[j]
                );
            }
        }
    }

    #[test]
    fn two_rectangles_can_exactly_tile_a_small_atlas() {
        // A hand-verifiable, exactly-fills-the-atlas case: a 20x10 atlas,
        // two 10x10 squares side by side, filling it with zero leftover
        // space -- a third insertion of any size must then fail.
        let mut packer = AtlasPacker::new(20, 10);
        let first = packer.insert(10, 10).unwrap();
        let second = packer.insert(10, 10).unwrap();
        assert!(!first.overlaps(&second));
        assert_eq!(first.area() + second.area(), 200);
        assert!(packer.insert(1, 1).is_none());
    }

    #[test]
    fn split_leftover_picks_the_cut_that_leaves_the_larger_single_piece() {
        // Worked example from this module's own doc comment: a 100x100
        // free rectangle with a 90x10 piece placed in its corner. The
        // *smaller leftover width* (10, vs. leftover height 90) does NOT
        // correspond to the split that leaves the larger single piece --
        // cutting the other way (a 90x90 rectangle below, area 8100)
        // beats a 100x90 rectangle from the other cut (area 9000) is
        // false; hand-computed: cut A (a_below = 100x90 = 9000, a_right =
        // 10x10 = 100) beats cut B (b_right = 10x100 = 1000, b_below =
        // 90x90 = 8100), so cut A's 9000 must win despite its own
        // leftover *width* (10) being the smaller of the two raw leftover
        // dimensions -- confirming the split is chosen by actually
        // comparing resulting areas, not by comparing leftover_width vs.
        // leftover_height directly.
        let free = PackedRect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let (first, second) = split_leftover(free, 90, 10);
        let areas: Vec<u64> = [first, second]
            .into_iter()
            .flatten()
            .map(PackedRect::area)
            .collect();
        assert!(
            areas.contains(&9000),
            "expected the 100x90 piece (area 9000) to survive this split: {areas:?}"
        );
    }
}
