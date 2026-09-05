//! Core engine crate: the `Canvas` API, intermediate representation,
//! sort/batch pipeline, dynamic texture atlas, and SVG/MSDF tessellation.
//!
//! Pure safe Rust -- see TECHNICAL.md Section 9.1 for the workspace's
//! `unsafe` policy. Raw graphics-API FFI lives in the `tre-rhi-*` crates,
//! and zero-allocation buffer/arena/atlas-concurrency primitives live in
//! `tre-memory`; this crate depends on both but contains no `unsafe` itself.
#![forbid(unsafe_code)]
