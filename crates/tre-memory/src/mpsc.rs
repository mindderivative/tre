//! A bounded, pre-allocated Multi-Producer Single-Consumer lock-free ring
//! buffer (TECHNICAL.md Section 8): "generalizes the SPSC ring buffer
//! already used for OS input events... to the multi-producer case, since
//! here multiple window threads genuinely are independent producers."
//! First real use: IMPLEMENTATION.md Step 4.2.4's `AtlasInsertRequest`
//! channel, carrying requests from any number of producer threads
//! (eventually per-window worker threads) to the single atlas owner.
//!
//! [`SpscRingBuffer`](crate::SpscRingBuffer)'s own `push` is sound only
//! because exactly one producer ever reads/advances `head` -- with
//! multiple producers racing to claim the same slot, that same sequence
//! (load `head`, write the slot, store the new `head`) would let two
//! producers both write into the same slot before either publishes it.
//! This implements Dmitry Vyukov's well-known bounded MPMC ring buffer
//! design (each slot carries its own atomic sequence number, resolving
//! the producer-side race without a global lock), simplified for the
//! single-consumer case: the consumer side needs no CAS at all, since
//! only one thread is ever popping.

use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Slot<T> {
    /// Vyukov's per-slot sequence number. A slot at ring index `i` is
    /// ready to accept a write for enqueue position `pos` (where `pos %
    /// capacity == i`) exactly when `sequence == pos`, and ready to be
    /// read for that same `pos` exactly when `sequence == pos + 1`. This
    /// is what lets multiple producers safely race for "the next slot"
    /// without a global lock: a producer only commits its claim (the
    /// `enqueue_pos` CAS below) after confirming *this specific slot* is
    /// the one it expects, not just "some slot was free."
    sequence: AtomicUsize,
    value: UnsafeCell<MaybeUninit<T>>,
}

pub struct MpscRingBuffer<T> {
    buffer: Box<[Slot<T>]>,
    capacity: usize,
    enqueue_pos: AtomicUsize,
    dequeue_pos: AtomicUsize,
}

// SAFETY: a `T` value written into a slot by one producer thread is only
// ever read once, by the single consumer thread, after that producer's
// `Release` store to the slot's `sequence` is observed via the
// consumer's own `Acquire` load -- the same happens-before relationship
// `SpscRingBuffer` relies on, just with the additional producer-side CAS
// below ensuring no two producers ever write the same slot concurrently.
unsafe impl<T: Send> Send for MpscRingBuffer<T> {}
unsafe impl<T: Send> Sync for MpscRingBuffer<T> {}

impl<T> MpscRingBuffer<T> {
    /// `capacity` is the number of items the buffer can hold before
    /// `push` starts reporting it as full (DESIGN.md Section 2.6:
    /// overflow is reported, never grown dynamically mid-frame).
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(capacity > 0, "MpscRingBuffer capacity must be non-zero");
        let buffer: Box<[Slot<T>]> = (0..capacity)
            .map(|i| Slot {
                sequence: AtomicUsize::new(i),
                value: UnsafeCell::new(MaybeUninit::uninit()),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            buffer,
            capacity,
            enqueue_pos: AtomicUsize::new(0),
            dequeue_pos: AtomicUsize::new(0),
        }
    }

    /// Pushes an item onto the queue. Safe to call concurrently from any
    /// number of producer threads. Returns the item back as `Err` if the
    /// buffer is currently full.
    pub fn push(&self, item: T) -> Result<(), T> {
        let mut pos = self.enqueue_pos.load(Ordering::Relaxed);
        loop {
            let slot = &self.buffer[pos % self.capacity];
            let seq = slot.sequence.load(Ordering::Acquire);
            #[allow(
                clippy::cast_possible_wrap,
                reason = "both operands are ring positions that only ever grow by 1 per \
                          successful push/pop, far below isize::MAX for any real run"
            )]
            let diff = seq as isize - pos as isize;
            match diff.cmp(&0) {
                std::cmp::Ordering::Equal => {
                    // This slot is free for exactly this position -- try
                    // to claim it. A failed CAS means another producer
                    // claimed `pos` first; reload and retry against
                    // whatever `enqueue_pos` actually is now.
                    match self.enqueue_pos.compare_exchange_weak(
                        pos,
                        pos + 1,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => break self.write_claimed_slot(slot, pos, item),
                        Err(current) => pos = current,
                    }
                }
                std::cmp::Ordering::Less => return Err(item),
                std::cmp::Ordering::Greater => {
                    // Another producer already advanced past this
                    // position; reload the current enqueue position and
                    // try again.
                    pos = self.enqueue_pos.load(Ordering::Relaxed);
                }
            }
        }
    }

    fn write_claimed_slot(&self, slot: &Slot<T>, pos: usize, item: T) -> Result<(), T> {
        // SAFETY: this producer's `compare_exchange_weak` above is the
        // only way any thread could have claimed enqueue position `pos`
        // -- the queue enforces that at most one producer ever wins that
        // CAS for a given `pos`, so no other thread writes this slot
        // concurrently. The consumer never reads this slot until the
        // `Release` store to `sequence` below publishes it.
        unsafe {
            (*slot.value.get()).write(item);
        }
        slot.sequence.store(pos + 1, Ordering::Release);
        Ok(())
    }

    /// Pops the oldest queued item, or `None` if the buffer is empty.
    /// Must only ever be called from a single consumer thread -- this is
    /// the one place this type's contract is genuinely MPSC, not MPMC.
    pub fn pop(&self) -> Option<T> {
        let pos = self.dequeue_pos.load(Ordering::Relaxed);
        let slot = &self.buffer[pos % self.capacity];
        let seq = slot.sequence.load(Ordering::Acquire);
        #[allow(
            clippy::cast_possible_wrap,
            reason = "both operands are ring positions far below isize::MAX for any real run"
        )]
        let diff = seq as isize - (pos as isize + 1);
        if diff != 0 {
            // Either genuinely empty (no producer has published this
            // position yet), or -- since there is only one consumer --
            // this can never be "already claimed by another consumer".
            return None;
        }
        // SAFETY: `seq == pos + 1` was published by a producer's
        // `Release` store after it finished writing this slot's value,
        // observed here via the `Acquire` load above; this is the single
        // consumer's one and only read of this slot before it is handed
        // back to producers (via the `sequence` store below).
        let item = unsafe { (*slot.value.get()).assume_init_read() };
        // Free this slot for the producer that will claim position `pos
        // + capacity` (the next full lap around the ring).
        slot.sequence.store(pos + self.capacity, Ordering::Release);
        self.dequeue_pos.store(pos + 1, Ordering::Relaxed);
        Some(item)
    }

    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

impl<T> Drop for MpscRingBuffer<T> {
    fn drop(&mut self) {
        while self.pop().is_some() {}
    }
}

#[cfg(test)]
mod tests {
    use super::MpscRingBuffer;

    #[test]
    fn push_pop_preserves_fifo_order_for_a_single_producer() {
        let q = MpscRingBuffer::with_capacity(4);
        q.push(1).unwrap();
        q.push(2).unwrap();
        q.push(3).unwrap();
        assert_eq!(q.pop(), Some(1));
        assert_eq!(q.pop(), Some(2));
        assert_eq!(q.pop(), Some(3));
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn reports_full_rather_than_growing() {
        let q = MpscRingBuffer::with_capacity(2);
        assert!(q.push(1).is_ok());
        assert!(q.push(2).is_ok());
        assert_eq!(q.push(3), Err(3));
    }

    #[test]
    fn wraps_around_correctly_after_interleaved_use() {
        let q = MpscRingBuffer::with_capacity(2);
        q.push(1).unwrap();
        assert_eq!(q.pop(), Some(1));
        q.push(2).unwrap();
        q.push(3).unwrap();
        assert_eq!(q.pop(), Some(2));
        assert_eq!(q.pop(), Some(3));
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn drop_cleans_up_still_queued_items() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        #[derive(Debug)]
        struct DropCounter(Arc<AtomicUsize>);
        impl Drop for DropCounter {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        let count = Arc::new(AtomicUsize::new(0));
        let q = MpscRingBuffer::with_capacity(4);
        q.push(DropCounter(count.clone())).unwrap();
        q.push(DropCounter(count.clone())).unwrap();
        drop(q);
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn many_real_producer_threads_never_lose_or_duplicate_items() {
        use std::collections::HashSet;
        use std::sync::Arc;
        use std::thread;

        const PRODUCERS: usize = 8;
        const PER_PRODUCER: usize = 10_000;

        let q = Arc::new(MpscRingBuffer::with_capacity(64));
        let producers: Vec<_> = (0..PRODUCERS)
            .map(|producer_id| {
                let q = q.clone();
                thread::spawn(move || {
                    for i in 0..PER_PRODUCER {
                        let item = (producer_id, i);
                        while q.push(item).is_err() {
                            std::hint::spin_loop();
                        }
                    }
                })
            })
            .collect();

        let mut received = HashSet::with_capacity(PRODUCERS * PER_PRODUCER);
        while received.len() < PRODUCERS * PER_PRODUCER {
            if let Some(item) = q.pop() {
                assert!(
                    received.insert(item),
                    "item {item:?} was received more than once"
                );
            }
        }
        for producer in producers {
            producer.join().unwrap();
        }

        for producer_id in 0..PRODUCERS {
            for i in 0..PER_PRODUCER {
                assert!(
                    received.contains(&(producer_id, i)),
                    "item ({producer_id}, {i}) was never received"
                );
            }
        }
    }
}
