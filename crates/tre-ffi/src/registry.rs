//! `TreShapeRegistry`/`TreShapeId` -- the opaque-handle boundary over
//! `tre_engine::shapes::ShapeRegistry`/`ShapeId` (IMPLEMENTATION.md Phase
//! 10 Step 10.3 task 1). Solid-fill-only, `Rectangle`/`Circle`/`Polygon`/
//! `Path` -- this crate's own real, bounded first slice, matching
//! `tre-python`'s own identical Step 10.4 first slice exactly.

use std::os::raw::c_void;

use tre_engine::{
    Circle, Path, PathCommand, Polygon, Rectangle, ShapeId, ShapePrimitive, ShapeRegistry,
};

use crate::error::TreErrorCode;
use crate::ffi_guard;
use crate::handle;

#[repr(transparent)]
pub struct TreShapeRegistry(pub(crate) *mut c_void);

impl TreShapeRegistry {
    pub(crate) const fn null() -> Self {
        Self(std::ptr::null_mut())
    }
}

#[repr(transparent)]
pub struct TreShapeId(*mut c_void);

impl TreShapeId {
    pub(crate) const fn null() -> Self {
        Self(std::ptr::null_mut())
    }
}

/// One segment of a `Path`'s command list -- the `#[repr(C)]` tagged-
/// union shadow of `tre_engine::shapes::PathCommand` (TECHNICAL.md
/// Section 9.4.1: no Rust `enum`-with-data crosses the boundary
/// directly). Unused fields per `kind` are ignored on the Rust side and
/// should be zeroed by a well-behaved caller, but are never read for a
/// `kind` that doesn't name them.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TrePathCommand {
    pub kind: TrePathCommandKind,
    /// `MoveTo`/`LineTo`: the target point. `QuadraticTo`: the control
    /// point. `CubicTo`: the first control point. `Close`: unused.
    pub x0: f32,
    pub y0: f32,
    /// `QuadraticTo`: the target point. `CubicTo`: the second control
    /// point. Otherwise unused.
    pub x1: f32,
    pub y1: f32,
    /// `CubicTo`: the target point. Otherwise unused.
    pub x2: f32,
    pub y2: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrePathCommandKind {
    MoveTo = 0,
    LineTo = 1,
    QuadraticTo = 2,
    CubicTo = 3,
    Close = 4,
}

fn to_path_command(command: &TrePathCommand) -> PathCommand {
    match command.kind {
        TrePathCommandKind::MoveTo => PathCommand::MoveTo([command.x0, command.y0]),
        TrePathCommandKind::LineTo => PathCommand::LineTo([command.x0, command.y0]),
        TrePathCommandKind::QuadraticTo => PathCommand::QuadraticTo {
            control: [command.x0, command.y0],
            to: [command.x1, command.y1],
        },
        TrePathCommandKind::CubicTo => PathCommand::CubicTo {
            control1: [command.x0, command.y0],
            control2: [command.x1, command.y1],
            to: [command.x2, command.y2],
        },
        TrePathCommandKind::Close => PathCommand::Close,
    }
}

/// `false` for any non-finite or negative dimension -- `tre_engine`'s own
/// `ShapeRegistry::insert` never validates (it always succeeds), so this
/// crate's own insert wrappers check first, mirroring `tre-python`'s
/// established validation-at-insert-time convention
/// (`docs/python-api/index.md`).
fn dimension_is_valid(value: f32) -> bool {
    value.is_finite() && value >= 0.0
}

/// Leaves `*out_id` at [`TreShapeId::null`] on a validation failure, so a
/// caller that reads `*out_id` unconditionally (ignoring the returned
/// [`TreErrorCode`]) still gets a well-defined null handle rather than
/// uninitialized memory -- the same defensive convention
/// [`crate::frame_buffer::TreFrameBuffer::empty`] and
/// [`crate::renderer::TreHeadlessRenderer::null`] already establish for
/// their own out-parameters.
fn write_null_id(out_id: *mut TreShapeId) {
    if !out_id.is_null() {
        // SAFETY: a non-null `out_id` is valid for one write of a
        // `TreShapeId` per every caller's own `# Safety` contract.
        unsafe { out_id.write(TreShapeId::null()) };
    }
}

/// Creates a new, empty shape registry. Infallible.
#[unsafe(no_mangle)]
pub extern "C" fn tre_shape_registry_new() -> TreShapeRegistry {
    ffi_guard(TreShapeRegistry::null(), || {
        TreShapeRegistry(handle::into_raw(ShapeRegistry::new()))
    })
}

/// Destroys a registry created by [`tre_shape_registry_new`]. Safe to
/// call with a null/already-freed handle (a no-op).
///
/// # Safety
/// `registry` must be null, or a still-live value [`tre_shape_registry_new`]
/// returned that has not already been freed. Any [`TreShapeId`] this
/// registry issued and not yet passed to [`tre_shape_registry_remove`]
/// or [`tre_shape_id_free`] is leaked, not use-after-freed -- `TreShapeId`
/// owns its own boxed `ShapeId` value independently of the registry.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tre_shape_registry_free(registry: TreShapeRegistry) {
    // SAFETY: forwarded from this function's own `# Safety` contract.
    unsafe { handle::from_raw::<ShapeRegistry>(registry.0) }
}

/// The number of shapes currently live in `registry`, or `0` if
/// `registry` is null.
///
/// # Safety
/// `registry` must be null, or a still-live value [`tre_shape_registry_new`]
/// returned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tre_shape_registry_len(registry: TreShapeRegistry) -> usize {
    // SAFETY: forwarded from this function's own `# Safety` contract.
    unsafe { handle::as_ref::<ShapeRegistry>(registry.0) }.map_or(0, ShapeRegistry::len)
}

fn insert(registry: *mut c_void, shape: ShapePrimitive, out_id: *mut TreShapeId) -> TreErrorCode {
    ffi_guard(TreErrorCode::PanicCaught, move || {
        // SAFETY: `registry`/`out_id` come from this function's own
        // callers below, each of which documents the same `# Safety`
        // contract this relies on (a still-live registry, a valid
        // out-pointer).
        let Some(registry) = (unsafe { handle::as_mut::<ShapeRegistry>(registry) }) else {
            write_null_id(out_id);
            return TreErrorCode::InvalidArgument;
        };
        let id = registry.insert(shape);
        if !out_id.is_null() {
            // SAFETY: caller's contract guarantees `out_id` is valid for
            // one write of a `TreShapeId`.
            unsafe { out_id.write(TreShapeId(handle::into_raw(id))) };
        }
        TreErrorCode::Success
    })
}

/// Inserts a solid-filled, borderless, corner-radius-free rectangle at
/// `(x, y)` (its own top-left corner) sized `width x height`, filled
/// `rgba` (see [`tre_rgba8`](crate::tre_rgba8)). Writes the new shape's
/// id to `*out_id`.
///
/// # Errors
/// Returns [`TreErrorCode::InvalidArgument`] if `width`/`height` is
/// non-finite or negative, or `registry` is null.
///
/// # Safety
/// `registry` must be a still-live value [`tre_shape_registry_new`]
/// returned. `out_id` must be valid for one write of a [`TreShapeId`],
/// or null (the id is discarded).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tre_shape_registry_insert_rectangle(
    registry: TreShapeRegistry,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    rgba: u32,
    out_id: *mut TreShapeId,
) -> TreErrorCode {
    if !dimension_is_valid(width) || !dimension_is_valid(height) {
        write_null_id(out_id);
        return TreErrorCode::InvalidArgument;
    }
    let mut rectangle = Rectangle::new([width, height], rgba);
    rectangle.common.transform.position = [x, y];
    insert(registry.0, ShapePrimitive::Rectangle(rectangle), out_id)
}

/// Inserts a solid-filled, borderless, full (non-partial-arc) circle or
/// ellipse centered at `(x + radius_x, y + radius_y)` (`(x, y)` is its
/// own bounding-box top-left, matching [`tre_shape_registry_insert_rectangle`]'s
/// convention), filled `rgba`. Writes the new shape's id to `*out_id`.
///
/// `radius_x == radius_y` is a circle; otherwise an ellipse.
///
/// # Errors
/// Returns [`TreErrorCode::InvalidArgument`] if `radius_x`/`radius_y` is
/// non-finite or negative, or `registry` is null.
///
/// # Safety
/// Same contract as [`tre_shape_registry_insert_rectangle`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tre_shape_registry_insert_circle(
    registry: TreShapeRegistry,
    x: f32,
    y: f32,
    radius_x: f32,
    radius_y: f32,
    rgba: u32,
    out_id: *mut TreShapeId,
) -> TreErrorCode {
    if !dimension_is_valid(radius_x) || !dimension_is_valid(radius_y) {
        write_null_id(out_id);
        return TreErrorCode::InvalidArgument;
    }
    let mut circle = Circle::new([radius_x, radius_y], rgba);
    circle.common.transform.position = [x, y];
    insert(registry.0, ShapePrimitive::Circle(circle), out_id)
}

/// Inserts a solid-filled, borderless regular `sides`-gon (not a star)
/// of `radius` centered at `(x, y)`, filled `rgba`. Writes the new
/// shape's id to `*out_id`.
///
/// # Errors
/// Returns [`TreErrorCode::InvalidArgument`] if `radius` is non-finite
/// or negative, `sides` is less than 3, or `registry` is null.
///
/// # Safety
/// Same contract as [`tre_shape_registry_insert_rectangle`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tre_shape_registry_insert_polygon(
    registry: TreShapeRegistry,
    x: f32,
    y: f32,
    sides: u32,
    radius: f32,
    rgba: u32,
    out_id: *mut TreShapeId,
) -> TreErrorCode {
    if !dimension_is_valid(radius) || sides < 3 {
        write_null_id(out_id);
        return TreErrorCode::InvalidArgument;
    }
    let mut polygon = Polygon::new(sides, radius, rgba);
    polygon.common.transform.position = [x, y];
    insert(registry.0, ShapePrimitive::Polygon(polygon), out_id)
}

/// Inserts a solid-filled, borderless path built from `commands`
/// (`count` entries), offset by `(x, y)`, filled `rgba`. Writes the new
/// shape's id to `*out_id`.
///
/// # Errors
/// Returns [`TreErrorCode::InvalidArgument`] if `commands` is null while
/// `count` is non-zero, or `registry` is null.
///
/// # Safety
/// Same contract as [`tre_shape_registry_insert_rectangle`], plus:
/// `commands` must be valid for reads of `count` consecutive
/// [`TrePathCommand`] values, or null if `count` is `0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tre_shape_registry_insert_path(
    registry: TreShapeRegistry,
    x: f32,
    y: f32,
    commands: *const TrePathCommand,
    count: usize,
    rgba: u32,
    out_id: *mut TreShapeId,
) -> TreErrorCode {
    if commands.is_null() && count > 0 {
        write_null_id(out_id);
        return TreErrorCode::InvalidArgument;
    }
    // SAFETY: caller's contract guarantees `commands` is valid for
    // `count` reads whenever it's non-null; `count == 0` makes the slice
    // empty regardless of `commands`' own value.
    let commands = if count == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(commands, count) }
    };
    let path_commands: Vec<PathCommand> = commands.iter().map(to_path_command).collect();
    let mut path = Path::new(path_commands, rgba);
    path.common.transform.position = [x, y];
    insert(registry.0, ShapePrimitive::Path(path), out_id)
}

/// Removes `id`'s shape from `registry`, freeing `id` itself in the same
/// call regardless of whether it was actually found (matching
/// `ShapeRegistry::remove`'s own "report, don't panic" contract for a
/// stale id) -- `id` must not be used again after this call.
///
/// # Safety
/// `registry` must be a still-live value [`tre_shape_registry_new`]
/// returned. `id` must be null, or a still-live value this registry (or
/// any registry -- a foreign id is simply reported not-found, per
/// `ShapeRegistry::remove`'s own contract) issued, that has not already
/// been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tre_shape_registry_remove(
    registry: TreShapeRegistry,
    id: TreShapeId,
) -> bool {
    ffi_guard(false, move || {
        // SAFETY: forwarded from this function's own `# Safety` contract.
        let removed = match unsafe { handle::as_ref::<ShapeId>(id.0) } {
            Some(&id) => unsafe { handle::as_mut::<ShapeRegistry>(registry.0) }
                .is_some_and(|registry| registry.remove(id)),
            None => false,
        };
        // SAFETY: `id` is consumed by this call regardless of outcome,
        // matching this function's own documented contract.
        unsafe { handle::from_raw::<ShapeId>(id.0) };
        removed
    })
}

/// Frees a [`TreShapeId`] without removing its shape from any registry
/// -- for a caller that created a shape and, for whatever reason, never
/// wants to remove it before the registry itself is destroyed (avoiding
/// a leak of the id's own tiny boxed allocation; the shape itself is
/// still reclaimed normally when the registry is freed).
///
/// # Safety
/// `id` must be null, or a still-live value a `tre_shape_registry_insert_*`
/// call produced that has not already been passed to this function or to
/// [`tre_shape_registry_remove`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tre_shape_id_free(id: TreShapeId) {
    // SAFETY: forwarded from this function's own `# Safety` contract.
    unsafe { handle::from_raw::<ShapeId>(id.0) }
}
