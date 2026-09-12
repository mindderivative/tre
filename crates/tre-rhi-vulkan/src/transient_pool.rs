//! `VulkanDevice`'s transient render target pool machinery
//! (TECHNICAL.md Section 3.2) and its background GC thread
//! (IMPLEMENTATION.md Step 2.3): `TransientPool`/`TransientPoolStats`,
//! the deferred-release queue (`DeferredRelease`, drained on the main
//! thread in `VulkanDevice::begin_frame`), `gc_thread_loop` itself, and
//! the separate but similarly-shaped `BindlessRegistry` free-list
//! allocator. Split out of `lib.rs` as one of its ten separable concerns
//! (Architecture review finding).

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tre_engine::TextureFormat;

use crate::{
    FrameSync, VulkanTexture, GC_EVICTION_AGE_FRAMES, GC_MAX_EVICTIONS_PER_SCAN, GC_SCAN_INTERVAL,
    GC_TRIGGER_THRESHOLD_BYTES,
};

/// Diagnostic counters for `VulkanDevice`'s transient render target pool
/// (TECHNICAL.md Section 3.2), exposed so demos/tests can prove
/// steady-state pool reuse (a `hits`-only-growing, `misses`-flat pattern
/// after warmup) rather than asserting on internal state.
#[derive(Debug, Default, Clone, Copy)]
pub struct TransientPoolStats {
    /// Requests satisfied by reusing an already-allocated pool entry.
    pub hits: u64,
    /// Requests that required a cold allocation (no matching free entry).
    pub misses: u64,
    /// Entries the background GC thread has moved from the free list into
    /// the deferred-release queue (IMPLEMENTATION.md Step 2.3) -- not yet
    /// physically destroyed, just no longer available for reuse. Distinct
    /// from `destroyed` so a demo/test can observe both phases of the
    /// deferred-release design, not just an aggregate.
    pub evictions: u64,
    /// Entries the main thread has actually destroyed after their 3-frame
    /// grace period elapsed (IMPLEMENTATION.md Step 2.3).
    pub destroyed: u64,
}

/// An evicted transient texture awaiting `DEFERRED_RELEASE_GRACE_FRAMES`
/// before the main thread physically destroys it (IMPLEMENTATION.md
/// Step 2.3). Holds the real `VulkanTexture` -- moving it here (rather
/// than, say, just its raw handles) means its own `Drop` does the correct
/// image/view/memory teardown once this struct is finally dropped for
/// real, with nothing further to reconstruct.
pub(crate) struct DeferredRelease {
    // Never read by field name -- its only purpose is to be dropped, at
    // the right moment, running `VulkanTexture`'s own real teardown.
    // `dead_code` can't see a field-access-free drop as a "use," so this
    // is a deliberate `allow`, not an oversight.
    #[allow(dead_code)]
    texture: VulkanTexture,
    /// `FrameSync::total_frame_count` at the moment the GC thread evicted
    /// this entry -- what the main thread's grace-period check compares
    /// against the current frame count.
    pub(crate) evicted_at_frame: u64,
}

/// The background GC thread's entire loop body (IMPLEMENTATION.md
/// Step 2.3) -- the engine's first genuine OS thread. Deliberately never
/// calls a single Vulkan function: it only locks `transient_pool` (plain
/// Rust `HashMap`/`Vec` manipulation) and moves evicted `VulkanTexture`
/// values into `deferred_release`. The actual `vkDestroy*` calls happen
/// later, on the main thread, in `VulkanDevice::begin_frame`'s deferred-
/// release drain -- see PLAN_PHASE2_STEP2_3.md's "why this is safe despite
/// being genuinely concurrent" for the full reasoning. Exits when
/// `running` is set to `false` by `Drop for VulkanDevice`.
pub(crate) fn gc_thread_loop(
    transient_pool: Arc<Mutex<TransientPool>>,
    frame_sync: Arc<FrameSync>,
    deferred_release: Arc<Mutex<VecDeque<DeferredRelease>>>,
    running: Arc<AtomicBool>,
) {
    while running.load(Ordering::Acquire) {
        std::thread::sleep(GC_SCAN_INTERVAL);
        // Re-check immediately after waking: `Drop for VulkanDevice` may
        // have requested shutdown while this thread was asleep, and
        // `transient_pool` may already be mid-teardown by the time a
        // sleep started before shutdown finishes.
        if !running.load(Ordering::Acquire) {
            break;
        }

        let mut pool = transient_pool.lock().expect("transient pool poisoned");
        if pool.total_free_bytes < GC_TRIGGER_THRESHOLD_BYTES {
            continue;
        }

        // IMPLEMENTATION.md Step 2.3 task 3: evict free-list entries older
        // than `GC_EVICTION_AGE_FRAMES`, once triggered -- not stopping as
        // soon as the pool is back under budget, matching the task's
        // literal wording ("identify resources older than N = 600
        // frames"). Capped at `GC_MAX_EVICTIONS_PER_SCAN` per wake-up
        // (Phase 2 Step 2.3 Code Review finding #81), a THROUGHPUT limit,
        // not a "stop once under budget" one -- a backlog beyond the cap
        // still gets every eligible entry eventually, just spread across
        // more `GC_SCAN_INTERVAL`-spaced scans instead of one unboundedly
        // long critical section.
        let current_frame = frame_sync.total_frame_count.load(Ordering::Acquire);
        let mut evicted = Vec::new();
        'scan: for textures in pool.free.values_mut() {
            let mut i = 0;
            while i < textures.len() {
                let age = current_frame.saturating_sub(textures[i].last_used_frame);
                if age > GC_EVICTION_AGE_FRAMES {
                    // `swap_remove`, not `remove`: free-list order is
                    // irrelevant, and this avoids an O(n) shift per
                    // eviction. Does NOT increment `i` -- the element
                    // swapped into this position hasn't been checked yet.
                    // (Byte accounting updated after this loop, once
                    // `pool.free`'s borrow above has ended -- mutating
                    // `pool.total_free_bytes` here too would borrow `pool`
                    // mutably a second time while `values_mut()`'s
                    // iterator is still live.)
                    evicted.push(textures.swap_remove(i));
                    if evicted.len() >= GC_MAX_EVICTIONS_PER_SCAN {
                        break 'scan;
                    }
                } else {
                    i += 1;
                }
            }
        }
        // `saturating_sub` (Phase 2 Step 2.3 Code Review finding #79): see
        // `acquire_transient_target`'s matching comment -- this runs on
        // the GC thread, where an underflow panic would poison
        // `transient_pool`'s mutex and cascade into every future
        // main-thread caller sharing it.
        pool.total_free_bytes = pool
            .total_free_bytes
            .saturating_sub(evicted.iter().map(|t| t.size_bytes).sum::<u64>());
        if evicted.is_empty() {
            continue;
        }
        // Phase 2 Step 2.3 Code Review finding #82: a bucket emptied
        // entirely by eviction would otherwise stay in `pool.free` forever
        // as a live key with an empty `Vec` -- harmless at today's small,
        // bounded key space, but unbounded in principle if this pool is
        // later shared with a wider key space (e.g. the atlas/SVG cache
        // this step's literal wording targets).
        pool.free.retain(|_, textures| !textures.is_empty());
        pool.stats.evictions += evicted.len() as u64;
        // Released before taking `deferred_release`'s lock: nothing below
        // needs `transient_pool` any further this iteration, and the main
        // thread should never have to wait on the GC thread's scan just to
        // acquire/release a transient target.
        drop(pool);

        let mut queue = deferred_release
            .lock()
            .expect("deferred release queue poisoned");
        for texture in evicted {
            queue.push_back(DeferredRelease {
                texture,
                evicted_at_frame: current_frame,
            });
        }
    }
}

/// Free-list + bump allocator for `VulkanDevice::bindless_descriptor_set`'s
/// array slots (IMPLEMENTATION.md Step 2.1). Same shape as `TransientPool`'s
/// checked-in/checked-out bookkeeping, for the same reason: a small,
/// `Mutex`-guarded piece of device state that many textures' lifetimes touch
/// independently.
pub(crate) struct BindlessRegistry {
    /// Indices below this bound have been assigned at least once.
    next: u32,
    /// The real ceiling -- `VulkanDevice::new`'s runtime-clamped
    /// `bindless_capacity`, not `BINDLESS_TEXTURE_CAPACITY_TARGET`
    /// unconditionally, since a real device (a software rasterizer, most
    /// plausibly) may support fewer update-after-bind sampled images than
    /// the sort key's 4,096-slot target.
    capacity: u32,
    /// Indices released by a dropped `VulkanTexture`, available for reuse
    /// before bumping `next`.
    free: Vec<u32>,
}

impl BindlessRegistry {
    pub(crate) fn new(capacity: u32) -> Self {
        Self {
            next: 0,
            capacity,
            free: Vec::new(),
        }
    }

    /// Returns `None` if every slot up to `capacity` is currently live --
    /// exhausting the bindless array is a real, reportable condition, not
    /// something to paper over with an unbounded `Vec`.
    pub(crate) fn allocate(&mut self) -> Option<u32> {
        if let Some(index) = self.free.pop() {
            return Some(index);
        }
        if self.next < self.capacity {
            let index = self.next;
            self.next += 1;
            return Some(index);
        }
        None
    }

    pub(crate) fn release(&mut self, index: u32) {
        self.free.push(index);
    }
}

#[derive(Default)]
pub(crate) struct TransientPool {
    /// Checked-in (available) textures, bucketed by power-of-two
    /// `(width, height)` plus format.
    pub(crate) free: HashMap<(u32, u32, TextureFormat), Vec<VulkanTexture>>,
    /// Exact buckets a miss needs grown at the start of the next frame
    /// (deduplicated -- see the `contains` check at the push site).
    pub(crate) pending_growth: Vec<(u32, u32, TextureFormat)>,
    pub(crate) stats: TransientPoolStats,
    /// Sum of `size_bytes` across every entry currently in `free`
    /// (IMPLEMENTATION.md Step 2.3) -- what the GC thread compares against
    /// `GC_TRIGGER_THRESHOLD_BYTES`. Maintained incrementally (added to on
    /// check-in, subtracted on check-out/eviction) rather than recomputed
    /// by summing `free` on every scan, since the GC thread's scan
    /// interval is independent of how often the pool itself changes.
    pub(crate) total_free_bytes: u64,
}
