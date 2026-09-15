//! DirectX 12 RHI backend (`RhiDevice`/`RhiCommandBuffer` impls,
//! ARCHITECTURE.md Section 6), built on the `windows` crate's
//! `Win32::Graphics::Direct3D12` bindings (IMPLEMENTATION.md Step 2.1).
//! Windows-only -- the `windows` dependency itself is target-gated in
//! Cargo.toml, so this crate builds (as a no-op) on other host platforms.
//!
//! One of the three crates permitted to contain `unsafe`
//! (TECHNICAL.md Section 9.1), for raw DX12 FFI.
#![deny(unsafe_op_in_unsafe_fn)]
