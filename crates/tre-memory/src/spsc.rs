//! The canonical Single-Producer Single-Consumer lock-free ring buffer
//! (TECHNICAL.md Section 8): "the engine has exactly one consumer (the UI
//! framework's logic tick draining the queue)." Backing storage is
//! allocated exactly once, at construction -- `push`/`pop` never allocate,
//! matching DESIGN.md Section 2.1's zero-allocation steady state. Phase 1
//! Step 2 is this ring buffer's first real use (OS input events,
//! IMPLEMENTATION.md Step 1.2), driven from a single thread for now; it is
//! built genuinely lock-free/atomic-based so it needs no redesign whenever
//! a real second thread is introduced.

use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct SpscRingBuffer<T> {
    buffer: Box<[UnsafeCell<MaybeUninit<T>>]>,
    capacity: usize,
    head: AtomicUsize, // next write index -- producer-owned
    tail: AtomicUsize, // next read index -- consumer-owned
}

// SAFETY: `SpscRingBuffer<T>` moves ownership of `T` values between
// exactly one producer thread and one consumer thread via the atomic
// head/tail handshake below; no two threads ever access the same slot
// concurrently, so `Send` is sound whenever `T: Send`. It is not `Sync`
// beyond what a shared `&SpscRingBuffer` needs for the SPSC push/pop
// calls themselves, which is exactly what the derive would give it
// anyway once `T: Send`; declared explicitly since `UnsafeCell` opts out
// of the auto-trait by default.
unsafe impl<T: Send> Send for SpscRingBuffer<T> {}
unsafe impl<T: Send> Sync for SpscRingBuffer<T> {}

impl<T> SpscRingBuffer<T> {
    /// `capacity` is the number of items the buffer can hold before
    /// `push` starts reporting it as full (DESIGN.md Section 2.6:
    /// overflow is reported, never grown dynamically mid-frame). One
    /// extra internal slot is allocated to distinguish "full" from
    /// "empty" without a separate length counter.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(capacity > 0, "SpscRingBuffer capacity must be non-zero");
        let internal_capacity = capacity + 1;
        let buffer: Box<[UnsafeCell<MaybeUninit<T>>]> = (0..internal_capacity)
            .map(|_| UnsafeCell::new(MaybeUninit::uninit()))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            buffer,
            capacity: internal_capacity,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// Pushes an item onto the queue. Returns the item back as `Err` if
    /// the buffer is currently full.
    pub fn push(&self, item: T) -> Result<(), T> {
        let head = self.head.load(Ordering::Relaxed);
        let next_head = (head + 1) % self.capacity;
        if next_head == self.tail.load(Ordering::Acquire) {
            return Err(item);
        }
        // SAFETY: only the single producer ever writes to a slot at or
        // ahead of `head`, and the consumer never reads a slot until this
        // Release store publishes it as part of `[tail, head)`.
        unsafe {
            (*self.buffer[head].get()).write(item);
        }
        self.head.store(next_head, Ordering::Release);
        Ok(())
    }

    /// Pops the oldest queued item, or `None` if the buffer is empty.
    pub fn pop(&self) -> Option<T> {
        let tail = self.tail.load(Ordering::Relaxed);
        if tail == self.head.load(Ordering::Acquire) {
            return None;
        }
        // SAFETY: this slot was written and published (Release store to
        // `head`) by the producer before the Acquire load above could
        // observe it as part of `[tail, head)`; the single consumer is
        // the only reader, and this is the one and only read of this slot
        // before the producer is allowed to reuse it (gated by `tail`
        // advancing below).
        let item = unsafe { (*self.buffer[tail].get()).assume_init_read() };
        self.tail
            .store((tail + 1) % self.capacity, Ordering::Release);
        Some(item)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tail.load(Ordering::Acquire) == self.head.load(Ordering::Acquire)
    }

    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity - 1
    }
}

impl<T> Drop for SpscRingBuffer<T> {
    fn drop(&mut self) {
        while self.pop().is_some() {}
    }
}

#[cfg(test)]
mod tests {
    use super::SpscRingBuffer;

    #[test]
    fn push_pop_preserves_fifo_order() {
        let q = SpscRingBuffer::with_capacity(4);
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
        let q = SpscRingBuffer::with_capacity(2);
        assert!(q.push(1).is_ok());
        assert!(q.push(2).is_ok());
        assert_eq!(q.push(3), Err(3));
    }

    #[test]
    fn wraps_around_correctly_after_interleaved_use() {
        let q = SpscRingBuffer::with_capacity(2);
        q.push(1).unwrap();
        assert_eq!(q.pop(), Some(1));
        q.push(2).unwrap();
        q.push(3).unwrap();
        assert_eq!(q.pop(), Some(2));
        assert_eq!(q.pop(), Some(3));
        assert!(q.is_empty());
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
        let q = SpscRingBuffer::with_capacity(4);
        q.push(DropCounter(count.clone())).unwrap();
        q.push(DropCounter(count.clone())).unwrap();
        drop(q);
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn genuinely_concurrent_producer_and_consumer_never_lose_or_duplicate_items() {
        use std::sync::Arc;
        use std::thread;

        let q = Arc::new(SpscRingBuffer::with_capacity(16));
        let producer_q = q.clone();
        const N: usize = 100_000;

        let producer = thread::spawn(move || {
            for i in 0..N {
                while producer_q.push(i).is_err() {
                    std::hint::spin_loop();
                }
            }
        });

        let mut received = Vec::with_capacity(N);
        while received.len() < N {
            if let Some(item) = q.pop() {
                received.push(item);
            }
        }
        producer.join().unwrap();

        assert_eq!(received, (0..N).collect::<Vec<_>>());
    }
}
