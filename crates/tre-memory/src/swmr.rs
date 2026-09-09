//! A fixed-capacity, open-addressed Single-Writer/Multi-Reader publish
//! table (TECHNICAL.md Section 8): "the atlas owner is the only writer
//! (`Ordering::Release` store into a slot), and any window's rendering
//! thread reads (`Ordering::Acquire` load) without ever taking a lock or
//! performing a CAS."
//!
//! Phase 4 Step 4.3.1: entries can now be [`SwmrSlotTable::remove`]d --
//! this module's own doc comment used to say entries were "add-only,
//! never removed... which is exactly what makes the 'hit an empty slot
//! during probing means the key was never inserted' early-exit sound; a
//! table that removed entries in place would need tombstones instead."
//! That's exactly what changed: a second reserved sentinel,
//! `TOMBSTONE_KEY`, marks a removed slot. Probing now stops (concludes
//! "never inserted") only at a genuine `EMPTY_KEY`; a tombstone means
//! "keep looking; this exact slot just isn't it anymore." Textbook
//! linear-probing-with-tombstones, not a novel scheme -- see `insert`'s
//! and the private `probe_get`'s own doc comments for the exact
//! algorithm.
//!
//! Recency tracking ([`SwmrSlotTable::get_and_touch`]) is a second,
//! independent addition: a parallel `last_used` array any reader thread
//! can write to via [`AtomicU64::fetch_max`], since "mark this entry as
//! still in use" is inherently a write every reader needs to make --
//! a different concurrency shape than the single-writer key/value
//! payload above, so it gets its own mechanism rather than being folded
//! into that one.
//!
//! Needs no `unsafe` at all: every per-slot field (key, value, recency
//! timestamp) is a plain `AtomicU64`, so "is this slot occupied, and by
//! which key" is answered by an ordinary atomic load, not a raw-pointer
//! read into possibly-uninitialized memory the way `MpscRingBuffer`'s
//! per-slot values are.

use std::sync::atomic::{AtomicU64, Ordering};

/// Reserved key value meaning "this slot has never been claimed." Callers
/// map their own key type to `u64` via [`Into<u64>`]; that mapping must
/// never produce this exact value for a real key.
const EMPTY_KEY: u64 = u64::MAX;

/// Reserved key value meaning "this slot held a real entry that
/// [`SwmrSlotTable::remove`] took back." Distinct from `EMPTY_KEY` so a
/// lookup can tell the two apart: a tombstoned slot proves nothing about
/// whether the key being searched for exists elsewhere along the same
/// probe sequence (so probing must continue past it), while a genuine
/// `EMPTY_KEY` proves the key was never inserted (so probing may safely
/// stop there) -- see this module's own top-level doc comment.
const TOMBSTONE_KEY: u64 = u64::MAX - 1;

/// True for either reserved sentinel. A real key must never equal either
/// value; both `insert` and `remove` reject them unconditionally (a real,
/// non-debug-only check -- not merely a `debug_assert`, since silently
/// miscomparing against a sentinel would corrupt an unrelated key's
/// entry in a release build with no diagnostic at all).
fn is_reserved(key_u64: u64) -> bool {
    key_u64 == EMPTY_KEY || key_u64 == TOMBSTONE_KEY
}

/// A fixed-capacity table mapping `K` to a `u64` payload, safe for one
/// writer ([`SwmrSlotTable::insert`], [`SwmrSlotTable::remove`]) and any
/// number of concurrent readers ([`SwmrSlotTable::get`],
/// [`SwmrSlotTable::get_and_touch`]).
pub struct SwmrSlotTable<K> {
    keys: Box<[AtomicU64]>,
    values: Box<[AtomicU64]>,
    /// Per-slot recency stamp for [`SwmrSlotTable::get_and_touch`] --
    /// deliberately separate from `values` above: any number of reader
    /// threads write here (via `fetch_max`), never just the single
    /// writer that owns `keys`/`values`.
    last_used: Box<[AtomicU64]>,
    /// Per-slot seqlock counter (REVIEW.md finding #131), bumped by the
    /// single writer as the last step of every `insert`/`remove` that
    /// touches a slot. `get`/`get_and_touch` read this before and after
    /// their own key/value reads and reject the result if it changed --
    /// re-checking `keys[index]` alone is *not* sufficient on its own:
    /// a slot can cycle key A -> key B -> key A again entirely within a
    /// reader's own read window (exactly what real sustained eviction-
    /// then-reuse traffic produces), so a plain "does the key still
    /// match" check can pass while the *value* actually read came from
    /// the B-occupied instant in between -- classic ABA. The epoch
    /// changing at all, even back to a value it held before, proves a
    /// writer mutation overlapped this read.
    epoch: Box<[AtomicU64]>,
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
            last_used: (0..capacity).map(|_| AtomicU64::new(0)).collect(),
            epoch: (0..capacity).map(|_| AtomicU64::new(0)).collect(),
            capacity,
            _key: std::marker::PhantomData,
        }
    }

    /// Publishes `value` under `key`. Must only ever be called from a
    /// single writer thread -- concurrent callers of `insert`/`remove`
    /// themselves are not supported (only `insert`/`remove` vs. `get`/
    /// `get_and_touch` is safe to run concurrently); the atlas owner is
    /// this table's one writer.
    ///
    /// The value is stored *before* the key is published (`Ordering::Release`
    /// only on the key store for a new slot), so any reader that observes
    /// the key via `get`'s `Ordering::Acquire` load is guaranteed to also
    /// observe this exact value, never a stale or default one -- the
    /// standard "publish the payload, then publish its availability"
    /// idiom.
    ///
    /// Probes for an existing occurrence of `key` first (updating it in
    /// place if found), remembering the first tombstoned-or-empty slot
    /// seen along the way. Probing stops as soon as it reaches a genuine
    /// `EMPTY_KEY` (proof `key` isn't present further along), but a
    /// tombstoned slot alone does not stop it -- `key` might still be
    /// present later in the same probe sequence, so a tombstone only
    /// becomes the claimed slot once the whole sequence has been
    /// exhausted (every remaining slot examined) without finding `key`
    /// or a genuine `EMPTY_KEY`. This is what guarantees a stale
    /// duplicate can never be left behind further along the same key's
    /// probe chain, and what lets a tombstoned slot be reused even in a
    /// table with no `EMPTY_KEY` slots left at all (every slot either
    /// holds a live key or a tombstone) -- the common case once a table
    /// has been through several remove/insert cycles.
    ///
    /// Returns `false` (without panicking -- DESIGN.md Section 2.6:
    /// capacity overflow is reported, not grown) if the table is full and
    /// `key` was not already present.
    #[must_use]
    pub fn insert(&self, key: K, value: u64) -> bool {
        let key_u64 = key.into();
        if is_reserved(key_u64) {
            // See `is_reserved`'s own doc comment: a real key colliding
            // with a sentinel must be rejected outright, not miscompared
            // against a slot that only looks empty/tombstoned because
            // it's the reserved value itself.
            return false;
        }
        let start = usize_index(mix(key_u64), self.capacity);
        let mut first_available: Option<usize> = None;
        for probe in 0..self.capacity {
            let index = (start + probe) % self.capacity;
            let existing = self.keys[index].load(Ordering::Relaxed);
            if existing == key_u64 {
                self.values[index].store(value, Ordering::Release);
                self.epoch[index].fetch_add(1, Ordering::Release);
                return true;
            }
            if existing == EMPTY_KEY {
                first_available.get_or_insert(index);
                break;
            }
            if existing == TOMBSTONE_KEY {
                first_available.get_or_insert(index);
            }
        }
        let Some(claim) = first_available else {
            return false;
        };
        self.values[claim].store(value, Ordering::Relaxed);
        // A reused tombstoned slot's old recency stamp belongs to
        // whatever key used to occupy it -- reset it so a freshly
        // (re)inserted entry doesn't look stale from the moment it's
        // published.
        self.last_used[claim].store(0, Ordering::Relaxed);
        self.keys[claim].store(key_u64, Ordering::Release);
        // Bumped last, after the key/value are both fully published --
        // see `epoch`'s own doc comment (REVIEW.md finding #131).
        self.epoch[claim].fetch_add(1, Ordering::Release);
        true
    }

    /// Removes `key`'s entry, if present, so a future `get`/`get_and_touch`
    /// for it returns `None` and a future `insert` for it (or for any
    /// other key) may reuse its slot. Must only ever be called from the
    /// single writer thread, same as `insert`.
    ///
    /// Stores the [`TOMBSTONE_KEY`] sentinel into the found slot
    /// (`Ordering::Release`, matching `insert`'s own publish discipline)
    /// rather than reverting it to `EMPTY_KEY` -- reverting to `EMPTY_KEY`
    /// would incorrectly signal "nothing was ever inserted past this
    /// point" to a concurrent `get` for a *different* key whose probe
    /// sequence happens to pass through this same slot, causing it to
    /// stop early and report a false miss for an entry that's still
    /// genuinely present further along.
    ///
    /// Returns `false` if `key` was not present.
    pub fn remove(&self, key: K) -> bool {
        let key_u64 = key.into();
        if is_reserved(key_u64) {
            return false;
        }
        let start = usize_index(mix(key_u64), self.capacity);
        for probe in 0..self.capacity {
            let index = (start + probe) % self.capacity;
            let existing = self.keys[index].load(Ordering::Relaxed);
            if existing == key_u64 {
                self.keys[index].store(TOMBSTONE_KEY, Ordering::Release);
                self.epoch[index].fetch_add(1, Ordering::Release);
                return true;
            }
            if existing == EMPTY_KEY {
                return false;
            }
        }
        false
    }

    /// Looks up `key`, or `None` if it was never inserted or has since
    /// been [`SwmrSlotTable::remove`]d. Safe to call concurrently from
    /// any number of reader threads, and concurrently with the single
    /// writer's own `insert`/`remove` calls. Does not affect recency
    /// tracking -- use [`SwmrSlotTable::get_and_touch`] for a lookup that
    /// also marks the entry as still in use.
    ///
    /// # Concurrency note (REVIEW.md finding #131)
    /// `probe_get` (finding the slot) and the subsequent value load are
    /// two separate atomic steps, not one -- between them, the single
    /// writer could `remove(key)` (tombstoning the slot) and then
    /// `insert` a *different* key that reuses that exact slot, which is
    /// exactly what the eviction policy's evict-then-immediately-reuse
    /// sequence does under real load. A first fix attempt (re-loading
    /// `keys[index]` once, after the value read) was not sufficient on
    /// its own: a real stress test found the slot can cycle key A -> key
    /// B -> key A again entirely within one reader's read window, so the
    /// key can match *again* by the time of the recheck while the value
    /// actually read came from the B-occupied instant in between --
    /// classic ABA, not caught by a plain equality recheck. This is why
    /// `epoch` exists: a real seqlock read (epoch before, value, key,
    /// epoch after -- all four checked together) catches a slot changing
    /// hands *any* number of times during the read, not just once,
    /// because any writer mutation overlapping the read bumps `epoch` at
    /// least once, regardless of what key ends up there by the time this
    /// call finishes reading. A mismatch reports the same outcome a
    /// lookup that lost the race entirely would have (`None`), which
    /// this API already treats as indistinguishable from "pending" or
    /// "evicted."
    #[must_use]
    pub fn get(&self, key: K) -> Option<u64> {
        let key_u64 = key.into();
        if is_reserved(key_u64) {
            return None;
        }
        let index = self.probe_get(key_u64)?;
        let epoch_before = self.epoch[index].load(Ordering::Acquire);
        let value = self.values[index].load(Ordering::Acquire);
        let key_after = self.keys[index].load(Ordering::Acquire);
        let epoch_after = self.epoch[index].load(Ordering::Acquire);
        if key_after != key_u64 || epoch_before != epoch_after {
            return None;
        }
        Some(value)
    }

    /// Same lookup as [`SwmrSlotTable::get`], additionally recording
    /// `frame` as this entry's most recent use (via
    /// [`AtomicU64::fetch_max`], so a lower `frame` than what's already
    /// recorded never regresses it -- the point of tracking a *maximum*
    /// observed frame number across any number of racing reader threads,
    /// not a plain timestamp). A miss touches nothing. Safe to call
    /// concurrently from any number of reader threads.
    ///
    /// Subject to the same slot-reuse race [`SwmrSlotTable::get`]'s own
    /// doc comment describes (REVIEW.md finding #131); guarded the same
    /// seqlock way. The recency touch only happens once the read is
    /// confirmed consistent, so (unlike an earlier draft of this method)
    /// a slot that turned out to have been reused by a different key
    /// never gets a stray touch credited to it.
    #[must_use]
    pub fn get_and_touch(&self, key: K, frame: u64) -> Option<u64> {
        let key_u64 = key.into();
        if is_reserved(key_u64) {
            return None;
        }
        let index = self.probe_get(key_u64)?;
        let epoch_before = self.epoch[index].load(Ordering::Acquire);
        let value = self.values[index].load(Ordering::Acquire);
        let key_after = self.keys[index].load(Ordering::Acquire);
        let epoch_after = self.epoch[index].load(Ordering::Acquire);
        if key_after != key_u64 || epoch_before != epoch_after {
            return None;
        }
        self.last_used[index].fetch_max(frame, Ordering::Relaxed);
        Some(value)
    }

    /// The shared probe loop behind both `get` and `get_and_touch`:
    /// returns the physical slot index currently holding `key_u64`, or
    /// `None` if it's absent. Continues past a `TOMBSTONE_KEY` (proves
    /// nothing about whether `key_u64` exists further along) and stops
    /// only at a genuine `EMPTY_KEY` (proves `key_u64` was never
    /// inserted, since `insert` always continues probing past every
    /// occupied-or-tombstoned slot in this exact same order before ever
    /// stopping at one).
    fn probe_get(&self, key_u64: u64) -> Option<usize> {
        let start = usize_index(mix(key_u64), self.capacity);
        for probe in 0..self.capacity {
            let index = (start + probe) % self.capacity;
            let existing = self.keys[index].load(Ordering::Acquire);
            if existing == key_u64 {
                return Some(index);
            }
            if existing == EMPTY_KEY {
                return None;
            }
        }
        None
    }

    /// Visits every currently-occupied slot whose recency stamp is below
    /// `cutoff_frame`, calling `visit(raw_key, value)` for each -- the
    /// enumeration a future eviction policy needs ("which entries haven't
    /// been touched in the last N frames") without polling every possible
    /// key individually. Returns the raw `u64` key rather than `K`,
    /// deliberately avoiding a `u64`-back-to-`K` trait bound this module
    /// otherwise has no use for; the caller reconstructs its own key type
    /// from it.
    ///
    /// Must only ever be called from the single writer thread -- like
    /// `insert`/`remove`, this is not a `get`-style operation any number
    /// of readers can share, since a future policy built on this is
    /// expected to also call `remove` based on what it finds, and only
    /// the writer may do that.
    pub fn scan_older_than(&self, cutoff_frame: u64, mut visit: impl FnMut(u64, u64)) {
        for index in 0..self.capacity {
            let key_u64 = self.keys[index].load(Ordering::Relaxed);
            if is_reserved(key_u64) {
                continue;
            }
            let last_used = self.last_used[index].load(Ordering::Relaxed);
            if last_used < cutoff_frame {
                visit(key_u64, self.values[index].load(Ordering::Relaxed));
            }
        }
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
    fn a_key_equal_to_the_reserved_sentinel_is_rejected_not_miscompared() {
        let table: SwmrSlotTable<Key> = SwmrSlotTable::with_capacity(4);
        assert!(!table.insert(Key(u64::MAX), 1));
        assert_eq!(table.get(Key(u64::MAX)), None);
        // A real key hashing to the same first-probed slot still inserts
        // and reads back correctly -- the sentinel key was never actually
        // written into that slot.
        assert!(table.insert(Key(1), 42));
        assert_eq!(table.get(Key(1)), Some(42));
    }

    #[test]
    fn a_key_equal_to_the_tombstone_sentinel_is_also_rejected() {
        // u64::MAX - 1, TOMBSTONE_KEY's own value -- not directly
        // constructible from outside this module, but a caller's own key
        // mapping could still produce it, and it must be rejected exactly
        // like EMPTY_KEY is.
        let table: SwmrSlotTable<Key> = SwmrSlotTable::with_capacity(4);
        assert!(!table.insert(Key(u64::MAX - 1), 1));
        assert!(!table.remove(Key(u64::MAX - 1)));
        assert_eq!(table.get(Key(u64::MAX - 1)), None);
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
    fn removed_key_is_gone_but_other_keys_still_round_trip() {
        let table: SwmrSlotTable<Key> = SwmrSlotTable::with_capacity(16);
        for i in 0..8u64 {
            assert!(table.insert(Key(i), i * 100));
        }
        assert!(table.remove(Key(3)));
        assert_eq!(table.get(Key(3)), None);
        for i in 0..8u64 {
            if i != 3 {
                assert_eq!(
                    table.get(Key(i)),
                    Some(i * 100),
                    "key {i} should be unaffected"
                );
            }
        }
        // Removing an absent key (never inserted, or already removed)
        // reports failure rather than panicking.
        assert!(!table.remove(Key(3)));
        assert!(!table.remove(Key(999)));
    }

    #[test]
    fn a_slot_freed_by_remove_does_not_permanently_shrink_capacity() {
        // A table sized for exactly 2 live keys must still accept a
        // 3rd insert once one of the first 2 is removed -- proving the
        // tombstoned slot is genuinely reused, not permanently wasted.
        let table: SwmrSlotTable<Key> = SwmrSlotTable::with_capacity(2);
        assert!(table.insert(Key(1), 1));
        assert!(table.insert(Key(2), 2));
        assert!(
            !table.insert(Key(3), 3),
            "table should be genuinely full here"
        );

        assert!(table.remove(Key(1)));
        assert!(
            table.insert(Key(3), 3),
            "the slot Key(1) vacated must be reusable by a different key"
        );
        assert_eq!(table.get(Key(1)), None);
        assert_eq!(table.get(Key(2)), Some(2));
        assert_eq!(table.get(Key(3)), Some(3));
    }

    #[test]
    fn re_inserting_a_removed_key_round_trips_its_new_value() {
        let table: SwmrSlotTable<Key> = SwmrSlotTable::with_capacity(4);
        assert!(table.insert(Key(5), 50));
        assert!(table.remove(Key(5)));
        assert_eq!(table.get(Key(5)), None);
        assert!(table.insert(Key(5), 500));
        assert_eq!(table.get(Key(5)), Some(500));
    }

    #[test]
    fn a_key_whose_probe_sequence_passes_through_a_tombstone_still_resolves() {
        // Fill a small table completely, remove one entry (leaving a
        // tombstone mid-chain for anything that collides through it),
        // then confirm every other key -- including ones that must probe
        // past the tombstone to reach their own slot -- still resolves
        // correctly, and the removed key stays genuinely absent.
        let table: SwmrSlotTable<Key> = SwmrSlotTable::with_capacity(8);
        for i in 0..8u64 {
            assert!(table.insert(Key(i), i));
        }
        assert!(table.remove(Key(2)));
        for i in 0..8u64 {
            if i == 2 {
                assert_eq!(table.get(Key(i)), None);
            } else {
                assert_eq!(table.get(Key(i)), Some(i));
            }
        }
    }

    #[test]
    fn get_and_touch_records_a_monotonically_increasing_recency_stamp() {
        let table: SwmrSlotTable<Key> = SwmrSlotTable::with_capacity(4);
        assert!(table.insert(Key(1), 111));

        assert_eq!(table.get_and_touch(Key(1), 10), Some(111));
        let mut stale = Vec::new();
        table.scan_older_than(10, |k, v| stale.push((k, v)));
        assert!(stale.is_empty(), "last_used=10 is not older than cutoff=10");

        // A lower frame number than what's already recorded must not
        // regress the stamp -- fetch_max, not a plain store.
        assert_eq!(table.get_and_touch(Key(1), 3), Some(111));
        let mut stale = Vec::new();
        table.scan_older_than(10, |k, v| stale.push((k, v)));
        assert!(
            stale.is_empty(),
            "an earlier touch with a lower frame must not have regressed last_used below 10"
        );

        let mut stale = Vec::new();
        table.scan_older_than(11, |k, v| stale.push((k, v)));
        assert_eq!(stale, vec![(1, 111)]);
    }

    #[test]
    fn scan_older_than_excludes_tombstoned_and_never_inserted_slots() {
        let table: SwmrSlotTable<Key> = SwmrSlotTable::with_capacity(8);
        assert!(table.insert(Key(1), 10));
        assert!(table.insert(Key(2), 20));
        assert!(table.insert(Key(3), 30));
        assert_eq!(table.get_and_touch(Key(1), 5), Some(10));
        assert_eq!(table.get_and_touch(Key(2), 50), Some(20));
        // Key(3) is never touched -- last_used stays at its post-insert 0.
        assert!(table.remove(Key(2)));

        let mut found: Vec<(u64, u64)> = Vec::new();
        table.scan_older_than(6, |k, v| found.push((k, v)));
        found.sort_unstable();
        // Key(2) is gone (tombstoned, excluded regardless of its former
        // recency); Key(1)'s last_used=5 is older than cutoff=6; Key(3)'s
        // last_used=0 is also older than cutoff=6.
        assert_eq!(found, vec![(1, 10), (3, 30)]);
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

    #[test]
    fn concurrent_readers_never_see_a_torn_value_while_a_remove_races_them() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{Arc, Barrier};
        use std::thread;

        const READERS: usize = 6;
        const ROUNDS: usize = 20_000;

        let table = Arc::new(SwmrSlotTable::<Key>::with_capacity(8));
        assert!(table.insert(Key(1), 111));
        assert!(table.insert(Key(2), 222));
        let ready = Arc::new(Barrier::new(READERS + 1));
        let saw_bad_value = Arc::new(AtomicBool::new(false));

        let readers: Vec<_> = (0..READERS)
            .map(|i| {
                let reader_table = table.clone();
                let reader_ready = ready.clone();
                let reader_bad = saw_bad_value.clone();
                thread::spawn(move || {
                    reader_ready.wait();
                    let key = if i % 2 == 0 { Key(1) } else { Key(2) };
                    let expected = if i % 2 == 0 { 111 } else { 222 };
                    for frame in 0..ROUNDS as u64 {
                        // Key(1) is the one being concurrently removed
                        // below; either it's still present (must read
                        // back exactly 111, never a torn/corrupted
                        // value) or it's genuinely absent -- both are
                        // valid outcomes of the race, unlike a value
                        // that's neither.
                        match reader_table.get_and_touch(key, frame) {
                            Some(value) if value != expected => {
                                reader_bad.store(true, Ordering::SeqCst);
                            }
                            _ => {}
                        }
                    }
                })
            })
            .collect();

        ready.wait();
        assert!(table.remove(Key(1)));
        for reader in readers {
            reader.join().unwrap();
        }

        assert!(
            !saw_bad_value.load(Ordering::SeqCst),
            "a reader observed a value that matched neither the expected payload nor a clean miss"
        );
        assert_eq!(table.get(Key(1)), None);
        assert_eq!(table.get(Key(2)), Some(222));
    }

    #[test]
    fn a_reader_never_observes_a_different_keys_value_when_the_writer_evicts_and_immediately_reuses_its_slot(
    ) {
        // REVIEW.md finding #131: a real reproduction of the atlas
        // owner's own eviction pattern (remove(key), then immediately
        // insert a *different* key) -- the exact sequence the
        // pre-existing `concurrent_readers_never_see_a_torn_value_while_
        // a_remove_races_them` test above never exercises (it only
        // removes, never reinserts a different key into the freed slot
        // while readers are in flight). Capacity 1 forces both keys to
        // collide on the same physical slot every time, guaranteeing
        // real reuse rather than hoping for a hash collision.
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        use std::sync::{Arc, Barrier};
        use std::thread;

        const ROUNDS: usize = 50_000;
        const KEY_A: Key = Key(1);
        const VALUE_A: u64 = 0xAAAA_AAAA;
        const KEY_B: Key = Key(2);
        const VALUE_B: u64 = 0xBBBB_BBBB;

        let table = Arc::new(SwmrSlotTable::<Key>::with_capacity(1));
        assert!(table.insert(KEY_A, VALUE_A));
        let ready = Arc::new(Barrier::new(2));
        let stop = Arc::new(AtomicBool::new(false));
        let saw_cross_key_value = Arc::new(AtomicBool::new(false));
        let reads_of_b = Arc::new(AtomicUsize::new(0));

        let reader_table = table.clone();
        let reader_ready = ready.clone();
        let reader_stop = stop.clone();
        let reader_bad = saw_cross_key_value.clone();
        let reader_b_count = reads_of_b.clone();
        let reader = thread::spawn(move || {
            reader_ready.wait();
            while !reader_stop.load(Ordering::Relaxed) {
                // Always asking about KEY_A: the only legitimate answers
                // are VALUE_A (still present) or None (evicted/not yet
                // reinserted). VALUE_B would mean this call read KEY_B's
                // value while believing it answered a KEY_A lookup --
                // exactly the bug #131 describes.
                match reader_table.get_and_touch(KEY_A, 0) {
                    Some(VALUE_A) | None => {}
                    Some(v) => {
                        reader_bad.store(true, Ordering::SeqCst);
                        if v == VALUE_B {
                            reader_b_count.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }
        });

        ready.wait();
        for _ in 0..ROUNDS {
            assert!(table.remove(KEY_A));
            assert!(table.insert(KEY_B, VALUE_B));
            assert!(table.remove(KEY_B));
            assert!(table.insert(KEY_A, VALUE_A));
        }
        stop.store(true, Ordering::Relaxed);
        reader.join().unwrap();

        assert!(
            !saw_cross_key_value.load(Ordering::SeqCst),
            "a reader asking about KEY_A observed a value other than VALUE_A or None -- \
             {} of those were KEY_B's own value, meaning the reader read across a slot the \
             writer had evicted-and-reused mid-lookup",
            reads_of_b.load(Ordering::Relaxed)
        );
    }
}
