//! Shared opaque-handle plumbing every `#[repr(transparent)]` handle
//! type in this crate (`TreShapeRegistry`, `TreShapeId`,
//! `TreHeadlessRenderer`) is built from -- a boxed value behind a raw
//! pointer (TECHNICAL.md Section 9.4.1's "opaque handle types as
//! `#[repr(transparent)]` pointer wrappers" rule), so the pattern lives
//! in one place instead of being hand-repeated per handle type.

use std::os::raw::c_void;

/// Boxes `value` and leaks it into a raw pointer suitable for a
/// `#[repr(transparent)]` handle's own inner field. Paired with
/// [`from_raw`] (consuming, in a `tre_*_free`) or [`as_ref`]/[`as_mut`]
/// (borrowing, for every other call taking this handle).
pub(crate) fn into_raw<T>(value: T) -> *mut c_void {
    Box::into_raw(Box::new(value)).cast()
}

/// Reclaims and drops the value [`into_raw`] boxed, or does nothing if
/// `ptr` is null (a caller freeing an already-null/never-created
/// handle is harmless, matching every `tre_*_free` function's own
/// null-safe contract in this crate).
///
/// # Safety
/// `ptr` must be null, or a value `into_raw::<T>` produced that has not
/// already been passed to `from_raw` -- the same one-free-per-value
/// contract every boxed handle in this crate carries.
pub(crate) unsafe fn from_raw<T>(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: caller's contract above.
    drop(unsafe { Box::from_raw(ptr.cast::<T>()) });
}

/// Borrows the value behind `ptr` immutably, or `None` if `ptr` is null.
///
/// # Safety
/// `ptr` must be null, or a still-live value `into_raw::<T>` produced
/// (not yet passed to [`from_raw`]), and no `&mut T` to the same value
/// may be outstanding for the duration of the returned borrow.
pub(crate) unsafe fn as_ref<'a, T>(ptr: *mut c_void) -> Option<&'a T> {
    // SAFETY: caller's contract above.
    unsafe { ptr.cast::<T>().as_ref() }
}

/// Borrows the value behind `ptr` mutably, or `None` if `ptr` is null.
///
/// # Safety
/// `ptr` must be null, or a still-live value `into_raw::<T>` produced
/// (not yet passed to [`from_raw`]), and no other reference to the same
/// value may be outstanding for the duration of the returned borrow.
pub(crate) unsafe fn as_mut<'a, T>(ptr: *mut c_void) -> Option<&'a mut T> {
    // SAFETY: caller's contract above.
    unsafe { ptr.cast::<T>().as_mut() }
}
