//! The engine's entire public surface: `#[repr(C)]` opaque handles and
//! `extern "C"` functions (TECHNICAL.md Section 9.4). The only crate in the
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
//!
//! IMPLEMENTATION.md Phase 10 Step 10.3's own real, bounded first slice
//! (mirroring `tre-python`'s identical Step 10.4 precedent): headless
//! rendering plus `Rectangle`/`Circle`/`Polygon`/`Path`, solid fill only
//! -- no `Text`, no gradients/textures, no windowed rendering yet. See
//! `crate::registry`/`crate::renderer` for the real surface this ships.
#![deny(unsafe_op_in_unsafe_fn)]

mod error;
mod frame_buffer;
mod handle;
mod registry;
mod renderer;

pub use error::TreErrorCode;
pub use frame_buffer::{tre_frame_buffer_free, TreFrameBuffer};
pub use registry::{
    tre_shape_id_free, tre_shape_registry_free, tre_shape_registry_insert_circle,
    tre_shape_registry_insert_path, tre_shape_registry_insert_polygon,
    tre_shape_registry_insert_rectangle, tre_shape_registry_len, tre_shape_registry_new,
    tre_shape_registry_remove, TrePathCommand, TrePathCommandKind, TreShapeId, TreShapeRegistry,
};
pub use renderer::{
    tre_headless_renderer_free, tre_headless_renderer_new, tre_headless_renderer_render,
    TreHeadlessRenderer,
};

/// Wraps the body of every exported `extern "C"` function in this crate
/// in `std::panic::catch_unwind`, converting an unwinding panic into
/// `default` instead of letting it cross the C-ABI boundary
/// (TECHNICAL.md Section 9.4.1's "FFI safety" rule -- undefined behavior
/// otherwise, per the Rust reference). `default` is usually
/// `TreErrorCode::PanicCaught` for a fallible function, or a null/empty
/// handle for one whose normal return type has no error-code slot of its
/// own (e.g. `tre_shape_registry_new`).
pub(crate) fn ffi_guard<T>(default: T, f: impl FnOnce() -> T + std::panic::UnwindSafe) -> T {
    std::panic::catch_unwind(f).unwrap_or(default)
}

/// Packs 8-bit RGBA channels into the `u32` this crate's shape-insert
/// functions expect for a fill color -- the exact same byte order
/// `tre_engine::rgba8` produces (re-implemented here rather than
/// re-exported, since `tre_engine::rgba8` is a `const fn` returning a
/// plain `u32`, already trivially `extern "C"`-compatible, but not
/// itself marked `extern "C"`/`#[unsafe(no_mangle)]` -- this crate is the
/// one place that boundary-facing wrapping belongs, per its own module
/// doc comment above).
#[unsafe(no_mangle)]
pub extern "C" fn tre_rgba8(r: u8, g: u8, b: u8, a: u8) -> u32 {
    tre_engine::rgba8(r, g, b, a)
}
