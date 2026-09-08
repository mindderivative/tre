//! A fixed-capacity, lock-free "scatter" arena (Step 5.2.2,
//! ARCHITECTURE.md Section 2.2/TECHNICAL.md Section 8): any number of
//! threads can concurrently reserve disjoint, contiguous output ranges
//! via a single atomic `fetch_add`, then write into them without ever
//! taking a lock. Unlike [`crate::MpscRingBuffer`], nothing here is
//! ever popped or reused mid-frame -- every reservation is permanently
//! exclusive until the whole arena is consumed once, at frame's end,
//! via [`ScatterArena::into_vec`]. First real use: `tre-engine`'s
//! `FrameArena`, merging a `SubCanvas`'s locally-recorded
//! `vertices`/`indices`/`commands` into one shared destination as that
//! worker thread's own last action before it exits
//! (IMPLEMENTATION.md Step 5.2's own task list: "Worker threads
//! bulk-copy... to the global arena at their acquired offset").

use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct ScatterArena<T> {
    slots: Box<[UnsafeCell<MaybeUninit<T>>]>,
    len: AtomicUsize,
}

// SAFETY: every reservation's range `[start, start + count)` is granted
// by exactly one `fetch_add` call and is never granted again -- `len`
// only ever increases, so no two `reserve` calls (concurrent or not)
// can ever return overlapping ranges. Each `ScatterSlice` therefore has
// genuinely exclusive access to its own slots for as long as it lives,
// and `into_vec` only ever runs after every `ScatterSlice` that could
// have written into the arena has already been dropped (it takes
// `self` by value), so there is no remaining writer left to race
// against by the time a read happens.
unsafe impl<T: Send> Send for ScatterArena<T> {}
unsafe impl<T: Send> Sync for ScatterArena<T> {}

impl<T: Copy> ScatterArena<T> {
    /// `capacity` is the arena's fixed total size, decided by the
    /// caller up front (DESIGN.md Section 2.1's zero-allocation-
    /// during-a-frame steady state) -- `reserve` reports overflow
    /// rather than growing it.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        let slots = (0..capacity)
            .map(|_| UnsafeCell::new(MaybeUninit::uninit()))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            slots,
            len: AtomicUsize::new(0),
        }
    }

    #[must_use]
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// Reserves `count` contiguous slots for the caller's exclusive
    /// use, returning `None` (report, don't grow -- DESIGN.md Section
    /// 2.6) if doing so would exceed capacity. Safe to call
    /// concurrently from any number of threads; never blocks.
    #[must_use]
    pub fn reserve(&self, count: usize) -> Option<ScatterSlice<'_, T>> {
        let start = self.len.fetch_add(count, Ordering::AcqRel);
        if start + count > self.slots.len() {
            return None;
        }
        // SAFETY: `start..start+count` was reserved by this call's own
        // `fetch_add` and, since `len` only ever increases, is disjoint
        // from every range any other `reserve` call has been or will
        // be granted -- so treating it as an exclusive `&mut [T]`
        // aliases no other live reference. The memory may still be
        // uninitialized, but `T: Copy` guarantees `T` has no `Drop`
        // impl, so a plain assignment into this slice (`slice[i] =
        // value`) never runs a destructor on whatever (possibly
        // garbage) bytes were there before -- unlike a `&mut [T]` over
        // uninitialized memory for a `T` with real drop glue, this is
        // sound. `UnsafeCell<MaybeUninit<T>>` and `T` share layout, so
        // the cast below is valid; the bounds check above confirms the
        // whole range lies within `self.slots`.
        let slice = unsafe {
            let ptr = self.slots.as_ptr().add(start).cast::<T>().cast_mut();
            std::slice::from_raw_parts_mut(ptr, count)
        };
        Some(ScatterSlice { slice, start })
    }

    /// Consumes the arena and returns the actually-written prefix as a
    /// plain `Vec<T>` (any never-reserved tail capacity is discarded).
    /// Callers must not call this until every [`ScatterSlice`] this
    /// arena ever granted has finished writing and been dropped --
    /// typically "after every worker thread has been joined."
    #[must_use]
    pub fn into_vec(self) -> Vec<T> {
        let len = self.len.load(Ordering::Acquire).min(self.slots.len());
        let mut out = Vec::with_capacity(len);
        for slot in &self.slots[..len] {
            // SAFETY: every slot in `0..len` was written by some
            // `ScatterSlice` before `len` was advanced past it -- the
            // `fetch_add` that claimed a given slot happens-before any
            // caller could observe `len` large enough to include it --
            // and this method takes `self` by value, so no
            // `ScatterSlice` can still be outstanding to race this read.
            out.push(unsafe { (*slot.get()).assume_init_read() });
        }
        out
    }
}

/// One thread's exclusive write access to `[start, start + slice.len())`
/// within a [`ScatterArena`] (`ScatterArena::reserve`'s own return
/// value). `Deref`/`DerefMut` to `[T]` for plain indexed writes;
/// [`ScatterSlice::start_index`] tells the caller where this
/// reservation actually landed, since that depends on however many
/// other threads' reservations were granted first -- needed to rebase
/// any data (like index-buffer values, or a sibling arena's own
/// offsets) that refers to positions relative to this reservation's
/// own start.
pub struct ScatterSlice<'a, T> {
    slice: &'a mut [T],
    start: usize,
}

impl<T> ScatterSlice<'_, T> {
    #[must_use]
    pub fn start_index(&self) -> usize {
        self.start
    }
}

impl<T> std::ops::Deref for ScatterSlice<'_, T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        self.slice
    }
}

impl<T> std::ops::DerefMut for ScatterSlice<'_, T> {
    fn deref_mut(&mut self) -> &mut [T] {
        self.slice
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserve_hands_out_the_expected_start_index_for_sequential_calls() {
        let arena: ScatterArena<u32> = ScatterArena::with_capacity(10);
        let first = arena.reserve(3).unwrap();
        assert_eq!(first.start_index(), 0);
        let second = arena.reserve(4).unwrap();
        assert_eq!(second.start_index(), 3);
    }

    #[test]
    fn writes_through_a_reserved_slice_are_visible_in_into_vec() {
        let arena: ScatterArena<u32> = ScatterArena::with_capacity(4);
        let mut slice = arena.reserve(4).unwrap();
        for (i, slot) in slice.iter_mut().enumerate() {
            *slot = u32::try_from(i).unwrap() * 10;
        }
        assert_eq!(arena.into_vec(), vec![0, 10, 20, 30]);
    }

    #[test]
    fn reserve_reports_none_rather_than_growing_past_capacity() {
        let arena: ScatterArena<u32> = ScatterArena::with_capacity(4);
        assert!(arena.reserve(3).is_some());
        assert!(
            arena.reserve(2).is_none(),
            "3 + 2 exceeds the arena's capacity of 4"
        );
    }

    #[test]
    fn into_vec_truncates_to_the_actually_reserved_prefix() {
        let arena: ScatterArena<u32> = ScatterArena::with_capacity(10);
        let mut slice = arena.reserve(3).unwrap();
        slice[0] = 1;
        slice[1] = 2;
        slice[2] = 3;
        assert_eq!(arena.into_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn many_real_threads_reserving_and_writing_concurrently_never_corrupt_or_overlap() {
        use std::sync::Arc;
        use std::thread;

        const THREADS: usize = 8;
        const PER_THREAD: usize = 500;

        let arena = Arc::new(ScatterArena::<(usize, usize)>::with_capacity(
            THREADS * PER_THREAD,
        ));
        let handles: Vec<_> = (0..THREADS)
            .map(|thread_id| {
                let arena = Arc::clone(&arena);
                thread::spawn(move || {
                    let mut slice = arena.reserve(PER_THREAD).expect("capacity is exact");
                    for (i, slot) in slice.iter_mut().enumerate() {
                        *slot = (thread_id, i);
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().expect("worker thread panicked");
        }

        let arena = Arc::try_unwrap(arena).unwrap_or_else(|_| panic!("all threads have joined"));
        let result = arena.into_vec();
        assert_eq!(result.len(), THREADS * PER_THREAD);

        let mut seen = vec![false; THREADS * PER_THREAD];
        for (thread_id, i) in result {
            assert!(thread_id < THREADS && i < PER_THREAD);
            let flat_index = thread_id * PER_THREAD + i;
            assert!(
                !seen[flat_index],
                "({thread_id}, {i}) landed in the result more than once"
            );
            seen[flat_index] = true;
        }
        assert!(
            seen.iter().all(|&s| s),
            "every (thread_id, i) pair must appear exactly once"
        );
    }
}
