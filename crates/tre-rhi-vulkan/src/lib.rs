//! Vulkan 1.2+ RHI backend (`RhiDevice`/`RhiCommandBuffer` impls,
//! ARCHITECTURE.md Section 6), built on the `ash` raw-bindings crate
//! (IMPLEMENTATION.md Step 2.1). Cross-platform wherever Vulkan is
//! available -- unlike the DX12/Metal backends, not target-gated to one OS.
//!
//! One of the three crates permitted to contain `unsafe`
//! (TECHNICAL.md Section 9.1), for raw Vulkan FFI.
#![deny(unsafe_op_in_unsafe_fn)]
