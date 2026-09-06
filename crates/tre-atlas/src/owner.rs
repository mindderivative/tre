//! The atlas owner: a real, dedicated background thread (the same
//! precedent Phase 2 Step 2.3's generational GC thread already
//! established), the *only* code ever allowed to touch the
//! [`AtlasPacker`]'s free-rectangle list (TECHNICAL.md Section 8: "the
//! atlas owner drains the MPSC queue, performs the Guillotine insertion
//! and MSDF rasterization, and is the only code in the engine ever
//! allowed to touch the free-rectangle list").

use std::sync::Arc;
use std::thread::{self, JoinHandle, Thread};
use std::time::Duration;

use tre_memory::{MpscRingBuffer, SwmrSlotTable};

use crate::key::pack_slot_value;
use crate::raster::AtlasInsertRequest;
use crate::{AtlasKey, AtlasPacker, PackedRect};

/// How long the owner thread's `park_timeout` waits before checking the
/// queue again on its own, in case an `unpark` call from a producer is
/// ever missed (e.g. a producer's `push`/`unpark` pair is interrupted by
/// the OS between the two calls) -- a correctness backstop, not the
/// normal wakeup path, which is the `unpark` call itself.
const PARK_TIMEOUT: Duration = Duration::from_millis(5);

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
}

impl AtlasOwnerHandle {
    /// Requests atlas space for `key`, to be rasterized via
    /// `raster_source` once the owner thread gets to it. Never blocks --
    /// returns `false` (without touching the queue's own contents) if the
    /// bounded request queue is currently full, matching DESIGN.md
    /// Section 2.6's "report, don't block" contract.
    #[must_use]
    pub fn request_insert(
        &self,
        key: AtlasKey,
        raster_source: Box<dyn crate::RasterSource>,
    ) -> bool {
        let ok = self
            .queue
            .push(OwnerMessage::Insert(AtlasInsertRequest {
                key,
                raster_source,
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
    /// been requested yet, or has been requested but not yet processed --
    /// deliberately indistinguishable from a reader's perspective
    /// (DESIGN.md Section 2.6's placeholder-glyph fallback responds to
    /// both the same way: use a placeholder this frame, re-check later).
    #[must_use]
    pub fn lookup(&self, key: AtlasKey) -> Option<(PackedRect, u16)> {
        self.slots.get(key).map(crate::key::unpack_slot_value)
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
            },
            join,
        }
    }

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
    let (width, height) = request.raster_source.size();
    // A request the packer can't currently fit is silently dropped --
    // this step builds no eviction/reclamation at all (Step 4.2.1's own
    // deferred future work), so there is nothing more this step's owner
    // could correctly do about it yet.
    let Some(rect) = packer.insert(width, height) else {
        return;
    };
    let pixels = request.raster_source.rasterize();
    copy_into_atlas(buffer, atlas_width, rect, &pixels);
    let packed = pack_slot_value(rect, 0);
    // A full slot table is reported the same way a full packer is above
    // -- silently, since this step has nothing more correct to do about
    // it yet (no eviction). The rect this glyph just took from the
    // packer is not reclaimed either way; sizing `slot_capacity`
    // generously relative to the real number of distinct keys a caller
    // expects is that caller's own responsibility.
    let _ = slots.insert(request.key, packed);
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

    fn wait_for(handle: &AtlasOwnerHandle, key: AtlasKey) -> (PackedRect, u16) {
        for _ in 0..10_000 {
            if let Some(result) = handle.lookup(key) {
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
        ));

        let (rect, generation) = wait_for(&handle, key);
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
                let (rect, _generation) = wait_for(&handle, key);
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
}
