//! The engine's entire public surface: `#[repr(C)]` opaque handles and
//! `extern "C"` functions (TECHNICAL.md Section 9.4). The only crate in this
//! workspace whose items are exported as public symbols in the shipped
//! `cdylib`/`staticlib` -- every other crate is linked in but exports
//! nothing of its own (TECHNICAL.md Section 9.2).
//!
//! One of the three crates permitted to contain `unsafe`
//! (TECHNICAL.md Section 9.1), for raw handle/pointer conversion and manual
//! buffer-ownership transfer across the C-ABI boundary. Every exported
//! `extern "C"` function must wrap its body in `std::panic::catch_unwind`
//! (DESIGN.md Section 2.7, TECHNICAL.md Section 9.1) -- panics must never
//! unwind past this boundary.
#![deny(unsafe_op_in_unsafe_fn)]
