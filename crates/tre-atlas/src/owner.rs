//! The atlas owner: a real, dedicated background thread (the same
//! precedent Phase 2 Step 2.3's generational GC thread already
//! established), the *only* code ever allowed to touch the
//! [`AtlasPacker`]'s free-rectangle list (TECHNICAL.md Section 8: "the
//! atlas owner drains the MPSC queue, performs the Guillotine insertion
//! and MSDF rasterization, and is the only code in the engine ever
//! allowed to touch the free-rectangle list").

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle, Thread};
use std::time::Duration;

use tre_memory::{MpscRingBuffer, SwmrSlotTable};

use crate::key::{pack_slot_value, unpack_slot_value};
use crate::raster::AtlasInsertRequest;
use crate::{AtlasKey, AtlasPacker, PackedRect};

/// How long the owner thread's `park_timeout` waits before checking the
/// queue again on its own, in case an `unpark` call from a producer is
/// ever missed (e.g. a producer's `push`/`unpark` pair is interrupted by
/// the OS between the two calls) -- a correctness backstop, not the
/// normal wakeup path, which is the `unpark` call itself.
const PARK_TIMEOUT: Duration = Duration::from_millis(5);

/// DESIGN.md Section 10.2: "when an atlas space capacity exceeds 85%,
/// the engine runs an asynchronous LRU garbage collection pass."
const EVICTION_CAPACITY_THRESHOLD: f64 = 0.85;

/// DESIGN.md Section 10.2: "evicting stale glyphs... that have not been
/// rendered within the last N frames (e.g., N >= 600 frames)."
const EVICTION_MIN_IDLE_FRAMES: u64 = 600;

enum OwnerMessage {
    Insert(AtlasInsertRequest),
    Shutdown,
}

/// The `Clone`-able, producer-facing side -- safe to share across any
/// number of threads requesting atlas space or reading back results.
#[derive(Clone)]
pub struct AtlasOwnerHandle {
    queue: Arc<MpscRingBuffer<OwnerMessage>>,
    slots: Arc<SwmrSlotTable<AtlasKey>>,
    owner_thread: Thread,
    closed: Arc<AtomicBool>,
}

impl AtlasOwnerHandle {
    /// Requests atlas space for `key`, to be rasterized via
    /// `raster_source` once the owner thread gets to it. Never blocks --
    /// returns `false` (without touching the queue's own contents) if the
    /// bounded request queue is currently full, matching DESIGN.md
    /// Section 2.6's "report, don't block" contract. Also returns `false`
    /// once [`AtlasOwner::join`] has been called on *any* clone of this
    /// handle's owner: without this check, a request that raced a
    /// concurrent shutdown could be pushed successfully yet never
    /// processed (the owner thread has already stopped polling the
    /// queue), silently leaving `lookup` returning `None` forever with no
    /// way for the caller to distinguish that from "still pending".
    ///
    /// `current_frame` (Step 4.3.3) is the caller's own notion of "what
    /// frame is it right now" -- the only way the owner's background
    /// thread learns what frame number to weigh its own LRU eviction
    /// check (DESIGN.md Section 10.2) against before attempting to pack
    /// this request.
    #[must_use]
    pub fn request_insert(
        &self,
        key: AtlasKey,
        raster_source: Box<dyn crate::RasterSource>,
        current_frame: u64,
    ) -> bool {
        if self.closed.load(Ordering::Acquire) {
            return false;
        }
        let ok = self
            .queue
            .push(OwnerMessage::Insert(AtlasInsertRequest {
                key,
                raster_source,
                current_frame,
            }))
            .is_ok();
        if ok {
            // Wakes the owner thread immediately if it's currently
            // parked waiting for work, rather than leaving it to
            // discover this request only after `PARK_TIMEOUT` elapses.
            self.owner_thread.unpark();
        }
        ok
    }

    /// Looks up `key`'s current atlas placement, or `None` if it hasn't
    /// been requested yet, has been requested but not yet processed, or
    /// has since been evicted (Step 4.3.3) -- deliberately
    /// indistinguishable from a reader's perspective (DESIGN.md Section
    /// 2.6's placeholder-glyph fallback responds to all three the same
    /// way: use a placeholder this frame, re-check/re-request later).
    ///
    /// `current_frame` records this read as evidence `key` is still
    /// genuinely in use as of this frame (`SwmrSlotTable::get_and_touch`,
    /// Step 4.3.1) -- a resident glyph that keeps getting looked up every
    /// frame it's actually rendered never goes stale enough to be
    /// evicted, which is the entire point of tracking recency at all.
    #[must_use]
    pub fn lookup(&self, key: AtlasKey, current_frame: u64) -> Option<(PackedRect, u16)> {
        self.slots
            .get_and_touch(key, current_frame)
            .map(crate::key::unpack_slot_value)
    }
}

/// Owns the background thread's [`JoinHandle`]; [`AtlasOwner::join`]
/// signals shutdown and returns the finished shared atlas pixel buffer
/// (RGBA8, `width * height * 4` bytes) for the caller to do with as it
/// pleases -- e.g. a one-time GPU texture upload (Step 4.2.3's
/// `TextureFormat::Rgba8Unorm`).
pub struct AtlasOwner {
    handle: AtlasOwnerHandle,
    join: JoinHandle<Vec<u8>>,
}

impl AtlasOwner {
    /// Spawns the real background thread. `request_capacity` bounds the
    /// pending-request queue; `slot_capacity` bounds how many distinct
    /// [`AtlasKey`]s the published-results table can ever hold -- both
    /// fixed at construction, matching DESIGN.md Section 2.1's
    /// zero-allocation steady state.
    #[must_use]
    pub fn spawn(
        atlas_width: u32,
        atlas_height: u32,
        request_capacity: usize,
        slot_capacity: usize,
    ) -> Self {
        let queue = Arc::new(MpscRingBuffer::with_capacity(request_capacity));
        let slots = Arc::new(SwmrSlotTable::with_capacity(slot_capacity));
        let closed = Arc::new(AtomicBool::new(false));

        let thread_queue = queue.clone();
        let thread_slots = slots.clone();
        let join = thread::spawn(move || {
            run_owner_loop(atlas_width, atlas_height, &thread_queue, &thread_slots)
        });
        let owner_thread = join.thread().clone();

        Self {
            handle: AtlasOwnerHandle {
                queue,
                slots,
                owner_thread,
                closed,
            },
            join,
        }
    }

    /// A new, `Clone`-able handle to this same atlas owner -- give one to
    /// every producer thread that needs to request insertions or read
    /// back placements.
    #[must_use]
    pub fn handle(&self) -> AtlasOwnerHandle {
        self.handle.clone()
    }

    /// Signals the background thread to stop and waits for it, returning
    /// the finished shared atlas pixel buffer.
    ///
    /// # Panics
    ///
    /// Panics if the background thread itself panicked.
    #[must_use]
    pub fn join(self) -> Vec<u8> {
        // Set before the Shutdown message is even pushed, so any
        // `request_insert` call any handle clone makes from this point
        // on reports failure instead of pushing a request the owner
        // thread may never poll for again.
        self.handle.closed.store(true, Ordering::Release);
        // Retries a few times in the (here, essentially theoretical)
        // case the queue happens to be transiently full of real
        // requests right as shutdown is requested -- a real
        // implementation would drain first; this demo-grade version
        // just waits briefly and retries rather than blocking forever.
        while self.handle.queue.push(OwnerMessage::Shutdown).is_err() {
            thread::sleep(Duration::from_millis(1));
        }
        self.handle.owner_thread.unpark();
        self.join.join().expect("atlas owner thread panicked")
    }
}

fn run_owner_loop(
    atlas_width: u32,
    atlas_height: u32,
    queue: &MpscRingBuffer<OwnerMessage>,
    slots: &SwmrSlotTable<AtlasKey>,
) -> Vec<u8> {
    let mut packer = AtlasPacker::new(atlas_width, atlas_height);
    #[allow(
        clippy::cast_possible_truncation,
        reason = "atlas dimensions are far below usize::MAX on any real target"
    )]
    let mut buffer = vec![0u8; (atlas_width as usize) * (atlas_height as usize) * 4];

    loop {
        match queue.pop() {
            Some(OwnerMessage::Shutdown) => break,
            Some(OwnerMessage::Insert(request)) => {
                process_insert(&mut packer, &mut buffer, atlas_width, slots, request);
            }
            None => thread::park_timeout(PARK_TIMEOUT),
        }
    }
    buffer
}

fn process_insert(
    packer: &mut AtlasPacker,
    buffer: &mut [u8],
    atlas_width: u32,
    slots: &SwmrSlotTable<AtlasKey>,
    request: AtlasInsertRequest,
) {
    maybe_evict_stale_entries(packer, slots, request.current_frame);

    let (width, height) = request.raster_source.size();
    // A request the packer still can't fit even after the eviction pass
    // above (e.g. the atlas is genuinely undersized for its real workload,
    // not merely fragmented with stale entries) is silently dropped --
    // matching DESIGN.md Section 2.6's "report, don't block" contract;
    // there is nothing more this owner could correctly do about it.
    let Some(rect) = packer.insert(width, height) else {
        return;
    };
    // `pack_slot_value` covers the full documented production atlas size
    // (4096x4096) but `AtlasPacker::insert` itself enforces no upper
    // bound tied to that encoding -- a caller that ever spins up an
    // atlas larger than the packed format can address must be dropped
    // the same way a full packer/slot-table is, not allowed through to
    // `pack_slot_value`'s own panic-on-overflow assert (security-review
    // finding: caller-supplied geometry should never be able to panic
    // this background thread).
    if !crate::key::fits_packed_range(rect) {
        return;
    }
    let pixels = request.raster_source.rasterize();
    copy_into_atlas(buffer, atlas_width, rect, &pixels);
    let packed = pack_slot_value(rect, 0);
    // A full slot table is reported the same way a full packer is above
    // -- silently; the caller is responsible for sizing `slot_capacity`
    // generously relative to the real number of distinct keys expected to
    // be resident at once (eviction reclaims *stale* slots, not capacity
    // itself).
    if slots.insert(request.key, packed) {
        // Stamps this brand-new entry as used as of right now (Step
        // 4.3.3's cold-start fix): `SwmrSlotTable::insert` resets a
        // freshly claimed slot's recency to 0, which would otherwise make
        // it look maximally stale the instant any *later* insert crosses
        // `EVICTION_CAPACITY_THRESHOLD` -- evicting content before it's
        // ever had a chance to be read. Treating insertion itself as an
        // access is standard LRU-cache practice, not a special case.
        let _ = slots.get_and_touch(request.key, request.current_frame);
    }
}

/// DESIGN.md Section 10.2's LRU eviction policy: once the atlas is over
/// `EVICTION_CAPACITY_THRESHOLD` full, every entry not read (via
/// `AtlasOwnerHandle::lookup`) or inserted within the last
/// `EVICTION_MIN_IDLE_FRAMES` frames is evicted in one pass -- its table
/// slot freed via `SwmrSlotTable::remove` and its atlas space freed via
/// `AtlasPacker::remove`, paired per entry so neither a leaked rect nor a
/// resurrected key with no backing space can result from only doing one
/// half. A no-op below the capacity threshold, regardless of how stale
/// any individual entry is -- capacity, not age alone, gates eviction.
///
/// # Known limitation (REVIEW.md finding #136, documented, not fixed)
/// This runs unconditionally at the top of every real `process_insert`
/// call, and once at/above the threshold, `SwmrSlotTable::scan_older_
/// than`'s full `O(capacity)` linear scan (plus a growing `Vec`
/// collecting matches) repeats on *every subsequent* insert while
/// occupancy stays at/above it -- not once per threshold-crossing. A
/// production atlas kept busy near its own capacity ceiling (exactly
/// the steady state this eviction policy exists to run in, per
/// DESIGN.md Section 10.2) pays this unamortized cost on the atlas
/// owner's own background thread on every miss resolution while under
/// that pressure. No current real caller exercises this repeatedly
/// (every demo pre-seeds its atlas once and stops the owner thread
/// before entering any real per-frame loop). Real fix: only re-scan
/// periodically, or once the previous pass's freed budget is exhausted,
/// rather than unconditionally.
fn maybe_evict_stale_entries(
    packer: &mut AtlasPacker,
    slots: &SwmrSlotTable<AtlasKey>,
    current_frame: u64,
) {
    if packer.used_fraction() < EVICTION_CAPACITY_THRESHOLD {
        return;
    }
    let cutoff_frame = current_frame.saturating_sub(EVICTION_MIN_IDLE_FRAMES);
    let mut stale: Vec<(u64, u64)> = Vec::new();
    slots.scan_older_than(cutoff_frame, |raw_key, packed_value| {
        stale.push((raw_key, packed_value));
    });
    for (raw_key, packed_value) in stale {
        let key = AtlasKey::from(raw_key);
        if slots.remove(key) {
            let (rect, _generation) = unpack_slot_value(packed_value);
            packer.remove(rect);
        }
    }
}

/// Copies a `rect.width x rect.height` RGBA8 block from `pixels`
/// (tightly packed, row-major) into `buffer` (the full atlas, also
/// row-major RGBA8, `atlas_width` wide) at `rect`'s own offset.
fn copy_into_atlas(buffer: &mut [u8], atlas_width: u32, rect: PackedRect, pixels: &[u8]) {
    let bytes_per_row = (rect.width as usize) * 4;
    for row in 0..rect.height {
        let src_start = (row as usize) * bytes_per_row;
        let dest_x = rect.x as usize;
        let dest_y = (rect.y + row) as usize;
        let dest_start = (dest_y * (atlas_width as usize) + dest_x) * 4;
        buffer[dest_start..dest_start + bytes_per_row]
            .copy_from_slice(&pixels[src_start..src_start + bytes_per_row]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RasterSource;

    /// A trivial `RasterSource` for tests: a solid `width x height` block
    /// of one repeated RGBA8 color, so a test can both request a
    /// specific size and independently verify the exact bytes the owner
    /// wrote into the shared atlas buffer.
    struct SolidColor {
        width: u32,
        height: u32,
        color: [u8; 4],
    }

    impl RasterSource for SolidColor {
        fn size(&self) -> (u32, u32) {
            (self.width, self.height)
        }

        fn rasterize(&self) -> Vec<u8> {
            self.color
                .iter()
                .copied()
                .cycle()
                .take((self.width * self.height * 4) as usize)
                .collect()
        }
    }

    fn wait_for(handle: &AtlasOwnerHandle, key: AtlasKey, current_frame: u64) -> (PackedRect, u16) {
        for _ in 0..10_000 {
            if let Some(result) = handle.lookup(key, current_frame) {
                return result;
            }
            thread::yield_now();
        }
        panic!("key {key:?} never resolved");
    }

    #[test]
    fn a_single_request_round_trips_with_correct_pixels() {
        let owner = AtlasOwner::spawn(64, 64, 16, 16);
        let handle = owner.handle();
        let key = AtlasKey::from_glyph(1, 1);
        assert!(handle.request_insert(
            key,
            Box::new(SolidColor {
                width: 8,
                height: 8,
                color: [255, 0, 0, 255],
            }),
            0,
        ));

        let (rect, generation) = wait_for(&handle, key, 0);
        assert_eq!((rect.width, rect.height), (8, 8));
        assert_eq!(generation, 0);

        let buffer = owner.join();
        let bytes_per_row = 8 * 4;
        let dest_start = ((rect.y as usize) * 64 + rect.x as usize) * 4;
        assert_eq!(
            &buffer[dest_start..dest_start + bytes_per_row],
            [255u8, 0, 0, 255].repeat(8).as_slice()
        );
    }

    #[test]
    fn many_real_producer_threads_concurrently_requesting_distinct_glyphs_all_resolve_correctly_and_without_overlap(
    ) {
        const PRODUCERS: usize = 6;
        const PER_PRODUCER: usize = 5;

        let owner = AtlasOwner::spawn(256, 256, 64, 64);
        let producers: Vec<_> = (0..PRODUCERS)
            .map(|producer_id| {
                let handle = owner.handle();
                thread::spawn(move || {
                    for i in 0..PER_PRODUCER {
                        #[allow(
                            clippy::cast_possible_truncation,
                            reason = "PRODUCERS/PER_PRODUCER are small test constants"
                        )]
                        let key = AtlasKey::from_glyph(producer_id as u32, i as u32);
                        let width = 10 + (i as u32) * 2;
                        let height = 12 + (producer_id as u32);
                        #[allow(
                            clippy::cast_possible_truncation,
                            reason = "producer_id/i are small test constants, well within u8"
                        )]
                        let color = [producer_id as u8 * 20, i as u8 * 20, 100, 255];
                        while !handle.request_insert(
                            key,
                            Box::new(SolidColor {
                                width,
                                height,
                                color,
                            }),
                            0,
                        ) {
                            thread::yield_now();
                        }
                    }
                })
            })
            .collect();
        for producer in producers {
            producer.join().unwrap();
        }

        let handle = owner.handle();
        let mut rects = Vec::new();
        for producer_id in 0..PRODUCERS {
            for i in 0..PER_PRODUCER {
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "PRODUCERS/PER_PRODUCER are small test constants"
                )]
                let key = AtlasKey::from_glyph(producer_id as u32, i as u32);
                let (rect, _generation) = wait_for(&handle, key, 0);
                let width = 10 + (i as u32) * 2;
                let height = 12 + (producer_id as u32);
                assert_eq!((rect.width, rect.height), (width, height));
                rects.push(rect);
            }
        }

        for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                let (a, b) = (rects[i], rects[j]);
                let overlaps = a.x < b.x + b.width
                    && b.x < a.x + a.width
                    && a.y < b.y + b.height
                    && b.y < a.y + a.height;
                assert!(!overlaps, "placements {a:?} and {b:?} overlap");
            }
        }

        let _ = owner.join();
    }

    /// Requests a solid-color `width x height` insertion and waits for it
    /// to resolve, both stamped with `current_frame` -- reduces the
    /// eviction tests below to their actual point (capacity/recency
    /// bookkeeping) instead of repeating this same request/wait pair.
    fn insert_and_wait(
        handle: &AtlasOwnerHandle,
        key: AtlasKey,
        width: u32,
        height: u32,
        current_frame: u64,
    ) -> (PackedRect, u16) {
        assert!(handle.request_insert(
            key,
            Box::new(SolidColor {
                width,
                height,
                color: [10, 20, 30, 255],
            }),
            current_frame,
        ));
        wait_for(handle, key, current_frame)
    }

    #[test]
    fn eviction_does_not_run_below_the_capacity_threshold_no_matter_how_stale() {
        // A spacious 64x64 (4096px^2) atlas: two tiny 4x4 (16px^2 each)
        // placements use a negligible fraction of it. Requesting a third
        // insertion at a frame far past EVICTION_MIN_IDLE_FRAMES (600)
        // must not evict the first two -- capacity, not age alone, gates
        // eviction, and this atlas never comes close to 85% full.
        let owner = AtlasOwner::spawn(64, 64, 16, 16);
        let handle = owner.handle();
        let a = AtlasKey::from_glyph(1, 1);
        let b = AtlasKey::from_glyph(1, 2);
        insert_and_wait(&handle, a, 4, 4, 0);
        insert_and_wait(&handle, b, 4, 4, 0);

        let c = AtlasKey::from_glyph(1, 3);
        insert_and_wait(&handle, c, 4, 4, 10_000);

        assert!(
            handle.lookup(a, 10_000).is_some(),
            "far-below-capacity atlas must not evict anything, however stale"
        );
        assert!(handle.lookup(b, 10_000).is_some());

        let _ = owner.join();
    }

    #[test]
    fn a_freshly_inserted_entry_is_not_evicted_by_the_very_next_insert_that_crosses_capacity() {
        // The cold-start hazard this step's own plan called out: a
        // 10x10 (100px^2) atlas, `a` placed at 9x10 (90px^2, 0.90 used --
        // already over the 0.85 threshold) at frame 700, immediately
        // followed by a request for `b` (1x1) also at frame 700. Without
        // stamping `a`'s recency at insertion time, `a`'s last_used would
        // still read its post-insert default of 0, which IS older than
        // this eviction pass's cutoff (700 - 600 = 100) -- incorrectly
        // evicting content the instant after it was created. With the
        // fix, `a`'s recency is 700, well above the cutoff, so it must
        // survive.
        let owner = AtlasOwner::spawn(10, 10, 8, 8);
        let handle = owner.handle();
        let a = AtlasKey::from_glyph(9, 10);
        insert_and_wait(&handle, a, 9, 10, 700);

        let b = AtlasKey::from_glyph(1, 1);
        insert_and_wait(&handle, b, 1, 1, 700);

        assert!(
            handle.lookup(a, 700).is_some(),
            "a brand-new entry must not be evicted by the very next insert"
        );

        let _ = owner.join();
    }

    #[test]
    fn a_real_eviction_frees_space_for_a_new_insertion_while_sparing_a_touched_entry() {
        // A 10x10 (100px^2) atlas: `a` (5x9, 45px^2) and `b` (5x9, 45px^2)
        // both placed at frame 0 (used=0.90, already over the 0.85
        // threshold for whatever comes next). `a` is explicitly touched
        // at frame 700 (kept fresh); `b` is left untouched. Requesting
        // `c` (1x1) at frame 700 must trigger eviction (cutoff = 100):
        // `b`'s last_used (0, from its own frame-0 insertion) is stale
        // and it is evicted, freeing its 45px^2 back to the packer; `a`'s
        // last_used (700, from the explicit touch) survives; `c` succeeds
        // by reusing real, reclaimed atlas space.
        let owner = AtlasOwner::spawn(10, 10, 8, 8);
        let handle = owner.handle();
        let a = AtlasKey::from_glyph(5, 9);
        let (rect_a, _) = insert_and_wait(&handle, a, 5, 9, 0);
        let b = AtlasKey::from_glyph(5, 91);
        insert_and_wait(&handle, b, 5, 9, 0);

        // Keep `a` fresh; leave `b` untouched.
        assert!(handle.lookup(a, 700).is_some());

        let c = AtlasKey::from_glyph(1, 1);
        let (rect_c, _) = insert_and_wait(&handle, c, 1, 1, 700);

        assert!(
            handle.lookup(b, 700).is_none(),
            "the stale, untouched entry must have been evicted"
        );
        assert!(
            handle.lookup(a, 700).is_some(),
            "the touched, fresh entry must have survived eviction"
        );
        assert!(
            !rect_a.overlaps(&rect_c),
            "the new insertion must not overlap the surviving entry"
        );

        let _ = owner.join();
    }

    #[test]
    fn eviction_boundary_is_correct_across_several_entries_with_mixed_staleness_at_once() {
        // Phase 9 Step 9.1: extends the single-stale-entry case above to
        // several entries at once, spanning the exact
        // EVICTION_MIN_IDLE_FRAMES (600) cutoff from both sides -- real
        // adversarial coverage for an off-by-one in the age comparison,
        // not just "eviction happens at all." A 10x10 atlas (100px^2):
        // three 10x3 entries (30px^2 each, 90px^2 total = 0.90 used --
        // `maybe_evict_stale_entries` checks *pre*-insert usage, so this
        // must already be at/above the 0.85 threshold on its own,
        // unlike a fraction that only crosses it once the trigger
        // insert's own space is added) inserted at frame 0. Requesting
        // a fourth 1x1 entry at frame 700 crosses the threshold; idle =
        // 700 - last_used for each of the first three.
        let owner = AtlasOwner::spawn(10, 10, 8, 8);
        let handle = owner.handle();

        // `maybe_evict_stale_entries` computes `cutoff_frame =
        // current_frame - EVICTION_MIN_IDLE_FRAMES` (700 - 600 = 100
        // here) and evicts only entries with `last_used < cutoff_frame`
        // (`SwmrSlotTable::scan_older_than`'s own real, strict
        // comparison) -- so `last_used == cutoff_frame` exactly
        // *survives* (idle == 600 exactly is not evicted), and only
        // `last_used < cutoff_frame` (idle > 600) is.
        let stale = AtlasKey::from_glyph(1, 1);
        insert_and_wait(&handle, stale, 10, 3, 0);
        // Touched at frame 100 -- last_used == cutoff_frame exactly
        // (idle 600 at the trigger frame): must survive, the strict
        // `<` comparison excludes it.
        let at_cutoff_survives = AtlasKey::from_glyph(1, 2);
        insert_and_wait(&handle, at_cutoff_survives, 10, 3, 0);
        assert!(handle.lookup(at_cutoff_survives, 100).is_some());
        // Touched at frame 99 -- last_used is one frame *under*
        // cutoff_frame (idle 601 at the trigger frame): must be
        // evicted.
        let past_cutoff_evicted = AtlasKey::from_glyph(1, 3);
        insert_and_wait(&handle, past_cutoff_evicted, 10, 3, 0);
        assert!(handle.lookup(past_cutoff_evicted, 99).is_some());

        let trigger = AtlasKey::from_glyph(1, 4);
        insert_and_wait(&handle, trigger, 1, 1, 700);

        assert!(
            handle.lookup(stale, 700).is_none(),
            "never touched since frame 0 -- idle 700, well past the cutoff"
        );
        assert!(
            handle.lookup(past_cutoff_evicted, 700).is_none(),
            "idle 601 (700 - 99), one frame past the cutoff, must be evicted"
        );
        assert!(
            handle.lookup(at_cutoff_survives, 700).is_some(),
            "idle exactly 600 (700 - 100) at the trigger frame must survive -- the strict `<` \
             comparison in scan_older_than excludes it"
        );

        let _ = owner.join();
    }

    #[test]
    fn sustained_insert_evict_churn_stays_correct_across_many_cycles() {
        // Phase 9 Step 9.1's own "fragmentation behavior... under
        // sustained insert/evict churn" -- a single eviction event
        // proves the mechanism works once; real production use cycles
        // through many rounds of insert-some/evict-some/insert-again,
        // repeatedly fragmenting and reclaiming the same atlas space at
        // varying sizes. 40 rounds: each round inserts one small, one
        // medium, and one large entry (varying dimensions round to
        // round, so the packer never sees the exact same request twice
        // in a row), touches the *previous* round's own entries to keep
        // them alive, and lets the round-before-that's entries go
        // stale. Every round's own newly-inserted entries must resolve
        // correctly; the atlas must never wedge (every insert either
        // succeeds or is a genuine capacity miss, never a hang).
        let owner = AtlasOwner::spawn(64, 64, 256, 256);
        let handle = owner.handle();

        let mut previous_round_keys: Vec<AtlasKey> = Vec::new();
        let mut round_before_keys: Vec<AtlasKey> = Vec::new();

        for round in 0..40u32 {
            let frame = u64::from(round) * 700;
            let sizes = [(3, 3), (5, 7), (2, 9)];
            let mut this_round_keys = Vec::new();
            for (i, &(w, h)) in sizes.iter().enumerate() {
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "round/i are small test-loop counters"
                )]
                let key = AtlasKey::from_glyph(round, (i as u32) + 1);
                // Vary the exact size slightly round to round so the
                // packer must genuinely reclaim fragmented space of
                // differing shapes, not just the identical rect repeatedly.
                let width = w + (round % 3);
                let height = h + (round % 2);
                let (rect, _) = insert_and_wait(&handle, key, width, height, frame);
                assert_eq!(
                    (rect.width, rect.height),
                    (width, height),
                    "round {round}'s own entry {i} must resolve at its real requested size"
                );
                this_round_keys.push(key);
            }

            // Keep the previous round's own entries alive by touching
            // them; let the round-before-that's entries go stale and
            // become real eviction candidates on some later round once
            // capacity is crossed.
            for &key in &previous_round_keys {
                let _ = handle.lookup(key, frame);
            }

            round_before_keys = std::mem::replace(&mut previous_round_keys, this_round_keys);
        }
        let _ = round_before_keys;

        let _ = owner.join();
    }

    #[test]
    fn a_request_that_can_never_fit_even_after_a_full_eviction_pass_is_silently_dropped() {
        // REVIEW.md/DESIGN.md Section 2.6 (corrected, Phase 9 Step
        // 9.1): a request larger than the atlas itself can never be
        // satisfied no matter how much eviction frees -- `process_
        // insert`'s own real behavior is to silently drop it (`let
        // Some(rect) = packer.insert(..) else { return; }`), not to
        // render any kind of placeholder. The request must simply never
        // resolve, and -- critically -- the owner thread itself must
        // keep working correctly afterward, proving the drop path
        // doesn't corrupt any shared state (the packer, the slot
        // table, or the owner's own request loop).
        let owner = AtlasOwner::spawn(10, 10, 8, 8);
        let handle = owner.handle();

        let too_big = AtlasKey::from_glyph(9, 9);
        assert!(handle.request_insert(
            too_big,
            Box::new(SolidColor {
                width: 20,
                height: 20,
                color: [1, 2, 3, 255],
            }),
            0,
        ));
        // Give the owner thread real time to actually process (and
        // drop) the request before asserting it never resolves --
        // `wait_for`'s own retry loop would be the wrong tool here,
        // since it exists to prove a request *does* eventually resolve.
        for _ in 0..1_000 {
            assert!(
                handle.lookup(too_big, 0).is_none(),
                "a request bigger than the whole atlas must never resolve"
            );
            thread::yield_now();
        }

        // The owner must still be alive and correct: a real, fittable
        // request submitted right after the dropped one must resolve
        // normally.
        let fits = AtlasKey::from_glyph(1, 1);
        let (rect, _) = insert_and_wait(&handle, fits, 4, 4, 0);
        assert_eq!((rect.width, rect.height), (4, 4));

        let _ = owner.join();
    }
}
