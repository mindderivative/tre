//! A fixed-capacity, open-addressed Single-Writer/Multi-Reader publish
//! table (TECHNICAL.md Section 8): "the atlas owner is the only writer
//! (`Ordering::Release` store into a slot), and any window's rendering
//! thread reads (`Ordering::Acquire` load) without ever taking a lock or
//! performing a CAS." Entries are add-only -- never removed or updated to
//! a different key in place, matching the atlas's own real access
//! pattern (a resident glyph's slot is only ever revisited to update its
//! *value*, e.g. a new generation after eviction/reuse, never to change
//! *which* key occupies it) -- which is exactly what makes the "hit an
//! empty slot during probing means the key was never inserted" early-exit
//! below sound; a table that removed entries in place would need
//! tombstones instead.
//!
//! Needs no `unsafe` at all: both the key and the value at each slot are
//! plain `AtomicU64`s, so "is this slot occupied, and by which key" is
//! answered by an ordinary atomic load, not a raw-pointer read into
//! possibly-uninitialized memory the way `MpscRingBuffer`'s per-slot
//! values are.

use std::sync::atomic::{AtomicU64, Ordering};

/// Reserved key value meaning "this slot has never been claimed." Callers
/// map their own key type to `u64` via [`Into<u64>`]; that mapping must
/// never produce this exact value for a real key.
const EMPTY_KEY: u64 = u64::MAX;

/// A fixed-capacity table mapping `K` to a `u64` payload, safe for one
/// writer ([`SwmrSlotTable::insert`]) and any number of concurrent
/// readers ([`SwmrSlotTable::get`]).
pub struct SwmrSlotTable<K> {
    keys: Box<[AtomicU64]>,
    values: Box<[AtomicU64]>,
    capacity: usize,
    _key: std::marker::PhantomData<K>,
}

/// Spreads a `u64` key across the table's slots -- SplitMix64's own
/// finalizer mixing step, a well-known, fast integer hash (not a
/// from-scratch invention), reused here purely to pick a good starting
/// probe index, not for any cryptographic property.
fn mix(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

impl<K: Copy + Eq + Into<u64>> SwmrSlotTable<K> {
    /// # Panics
    ///
    /// Panics if `capacity` is zero.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(capacity > 0, "SwmrSlotTable capacity must be non-zero");
        Self {
            keys: (0..capacity).map(|_| AtomicU64::new(EMPTY_KEY)).collect(),
            values: (0..capacity).map(|_| AtomicU64::new(0)).collect(),
            capacity,
            _key: std::marker::PhantomData,
        }
    }

    /// Publishes `value` under `key`. Must only ever be called from a
    /// single writer thread -- concurrent callers of `insert` itself are
    /// not supported (only `insert` vs. `get` is safe to run
    /// concurrently); the atlas owner is this table's one writer.
    ///
    /// The value is stored *before* the key is published (`Ordering::Release`
    /// only on the key store for a new slot), so any reader that observes
    /// the key via `get`'s `Ordering::Acquire` load is guaranteed to also
    /// observe this exact value, never a stale or default one -- the
    /// standard "publish the payload, then publish its availability"
    /// idiom.
    ///
    /// Returns `false` (without panicking -- DESIGN.md Section 2.6:
    /// capacity overflow is reported, not grown) if the table is full and
    /// `key` was not already present.
    #[must_use]
    pub fn insert(&self, key: K, value: u64) -> bool {
        let key_u64 = key.into();
        debug_assert_ne!(
            key_u64, EMPTY_KEY,
            "a real key must never equal the reserved EMPTY_KEY sentinel"
        );
        let start = usize_index(mix(key_u64), self.capacity);
        for probe in 0..self.capacity {
            let index = (start + probe) % self.capacity;
            let existing = self.keys[index].load(Ordering::Relaxed);
            if existing == key_u64 {
                self.values[index].store(value, Ordering::Release);
                return true;
            }
            if existing == EMPTY_KEY {
                self.values[index].store(value, Ordering::Relaxed);
                self.keys[index].store(key_u64, Ordering::Release);
                return true;
            }
        }
        false
    }

    /// Looks up `key`, or `None` if it was never inserted. Safe to call
    /// concurrently from any number of reader threads, and concurrently
    /// with the single writer's own `insert` calls.
    #[must_use]
    pub fn get(&self, key: K) -> Option<u64> {
        let key_u64 = key.into();
        let start = usize_index(mix(key_u64), self.capacity);
        for probe in 0..self.capacity {
            let index = (start + probe) % self.capacity;
            let existing = self.keys[index].load(Ordering::Acquire);
            if existing == key_u64 {
                return Some(self.values[index].load(Ordering::Acquire));
            }
            if existing == EMPTY_KEY {
                // Sound only because entries are never removed: a
                // genuinely empty slot encountered while probing proves
                // `key` was never inserted, since insertion would have
                // continued probing past every occupied slot in exactly
                // this same order.
                return None;
            }
        }
        None
    }

    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

fn usize_index(mixed: u64, capacity: usize) -> usize {
    #[allow(
        clippy::cast_possible_truncation,
        reason = "reduced modulo `capacity` (a usize) immediately after; the intermediate \
                   truncation on a 32-bit target does not change the result modulo capacity, \
                   which is what actually matters"
    )]
    let mixed = mixed as usize;
    mixed % capacity
}

#[cfg(test)]
mod tests {
    use super::SwmrSlotTable;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Key(u64);
    impl From<Key> for u64 {
        fn from(key: Key) -> u64 {
            key.0
        }
    }

    #[test]
    fn get_before_any_insert_is_none() {
        let table: SwmrSlotTable<Key> = SwmrSlotTable::with_capacity(8);
        assert_eq!(table.get(Key(42)), None);
    }

    #[test]
    fn insert_then_get_round_trips() {
        let table: SwmrSlotTable<Key> = SwmrSlotTable::with_capacity(8);
        assert!(table.insert(Key(7), 12345));
        assert_eq!(table.get(Key(7)), Some(12345));
        assert_eq!(table.get(Key(8)), None);
    }

    #[test]
    fn re_inserting_the_same_key_updates_its_value() {
        let table: SwmrSlotTable<Key> = SwmrSlotTable::with_capacity(8);
        assert!(table.insert(Key(1), 100));
        assert!(table.insert(Key(1), 200));
        assert_eq!(table.get(Key(1)), Some(200));
    }

    #[test]
    fn a_full_table_reports_failure_rather_than_panicking() {
        let table: SwmrSlotTable<Key> = SwmrSlotTable::with_capacity(2);
        assert!(table.insert(Key(1), 1));
        assert!(table.insert(Key(2), 2));
        assert!(!table.insert(Key(3), 3));
        // The two entries that did fit are still both intact.
        assert_eq!(table.get(Key(1)), Some(1));
        assert_eq!(table.get(Key(2)), Some(2));
    }

    #[test]
    fn many_distinct_keys_all_round_trip_despite_hash_collisions() {
        let table: SwmrSlotTable<Key> = SwmrSlotTable::with_capacity(64);
        for i in 0..50u64 {
            assert!(table.insert(Key(i), i * 10));
        }
        for i in 0..50u64 {
            assert_eq!(table.get(Key(i)), Some(i * 10));
        }
    }

    #[test]
    fn concurrent_readers_see_a_fully_published_value_never_a_default() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{Arc, Barrier};
        use std::thread;

        let table = Arc::new(SwmrSlotTable::<Key>::with_capacity(4));
        let ready = Arc::new(Barrier::new(2));
        let saw_bad_value = Arc::new(AtomicBool::new(false));

        let reader_table = table.clone();
        let reader_ready = ready.clone();
        let reader_bad = saw_bad_value.clone();
        let reader = thread::spawn(move || {
            reader_ready.wait();
            loop {
                if let Some(value) = reader_table.get(Key(99)) {
                    if value != 0xDEAD_BEEF {
                        reader_bad.store(true, Ordering::SeqCst);
                    }
                    break;
                }
                std::hint::spin_loop();
            }
        });

        ready.wait();
        assert!(table.insert(Key(99), 0xDEAD_BEEF));
        reader.join().unwrap();

        assert!(
            !saw_bad_value.load(Ordering::SeqCst),
            "a reader observed the key before its real value was fully published"
        );
    }
}
