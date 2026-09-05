//! Metal 2.4+ RHI backend (`RhiDevice`/`RhiCommandBuffer` impls,
//! ARCHITECTURE.md Section 6), built on the `objc2-metal` crate
//! (IMPLEMENTATION.md Step 2.1). macOS-only -- the `objc2-metal` dependency
//! itself is target-gated in Cargo.toml, so this crate builds (as a no-op)
//! on other host platforms.
//!
//! One of the three crates permitted to contain `unsafe`
//! (TECHNICAL.md Section 9.1), for raw Metal/Objective-C FFI.
#![deny(unsafe_op_in_unsafe_fn)]
