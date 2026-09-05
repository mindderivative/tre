//! Vector/matrix math, affine transform batching, and SIMD-accelerated path
//! interpolation, built entirely on the `wide` crate's safe portable-SIMD
//! API (TECHNICAL.md Sections 2.2, 5.4, 7.2).
//!
//! Because `wide`'s public surface is safe Rust, this crate needs no
//! `unsafe` of its own and is not on TECHNICAL.md Section 9.1's
//! `unsafe`-permitted list. Per DESIGN.md Section 12.4, this is a stateless
//! evaluation library -- the UI framework's widget tree owns animation
//! state, not this crate.
#![forbid(unsafe_code)]
