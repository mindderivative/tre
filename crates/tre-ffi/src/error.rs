//! `TreErrorCode` -- the `#[repr(C)]` shadow of `tre_engine::EngineError`
//! every fallible `extern "C"` function returns (TECHNICAL.md Section
//! 9.4.1's "error propagation" rule: an integer result code, never a
//! Rust `Result`, with the success value carried through an
//! out-parameter instead).
//!
//! Two variants exist here that `EngineError` itself has no need for:
//! `InvalidArgument` (mirrors `tre-python`'s own established
//! validation-at-insert-time convention -- a Rust caller there gets a
//! real `ValueError`, which has no `EngineError` equivalent to shadow)
//! and `PanicCaught` (what [`crate::ffi_guard`] converts an unwinding
//! panic into -- a Rust caller never needs this variant since a panic
//! there just panics).

use tre_engine::EngineError;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreErrorCode {
    Success = 0,
    DeviceLost,
    SwapchainOutOfDate,
    PipelineCreationFailed,
    InvalidTextureData,
    BindlessArrayExhausted,
    TransientPoolBudgetExceeded,
    ShaderCompilationFailed,
    /// FFI-only: a shape-registry insert call was given a non-finite or
    /// negative dimension. `tre_engine::shapes::ShapeRegistry::insert`
    /// itself never fails (it always succeeds), so this crate's own
    /// `tre_shape_registry_insert_*` wrappers validate before calling it
    /// -- see `crate::registry`.
    InvalidArgument,
    /// FFI-only: [`crate::ffi_guard`] caught an unwinding Rust panic
    /// before it could cross the C-ABI boundary (TECHNICAL.md Section
    /// 9.4.1's "FFI safety" rule).
    PanicCaught,
}

impl From<EngineError> for TreErrorCode {
    fn from(error: EngineError) -> Self {
        match error {
            EngineError::DeviceLost => Self::DeviceLost,
            EngineError::SwapchainOutOfDate => Self::SwapchainOutOfDate,
            EngineError::PipelineCreationFailed => Self::PipelineCreationFailed,
            EngineError::InvalidTextureData => Self::InvalidTextureData,
            EngineError::BindlessArrayExhausted => Self::BindlessArrayExhausted,
            EngineError::TransientPoolBudgetExceeded => Self::TransientPoolBudgetExceeded,
            EngineError::ShaderCompilationFailed(_) => Self::ShaderCompilationFailed,
        }
    }
}
