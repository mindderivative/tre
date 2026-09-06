//! Zero-allocation triple-buffered ring arenas, the transient render-target
//! pool, generational GC/deferred-release bookkeeping (TECHNICAL.md
//! Section 3), and the dynamic atlas's lock-free MPSC request queue plus
//! single-writer/multi-reader publish table (TECHNICAL.md Section 8,
//! ARCHITECTURE.md Section 2.3).
//!
//! One of the three crates permitted to contain `unsafe`
//! (TECHNICAL.md Section 9.1) -- every `unsafe` block requires an adjacent
//! `// SAFETY:` comment stating the invariant being upheld.

mod mpsc;
mod spsc;
mod swmr;

pub use mpsc::MpscRingBuffer;
pub use spsc::SpscRingBuffer;
pub use swmr::SwmrSlotTable;
