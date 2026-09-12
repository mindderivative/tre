# Math & Memory

Two small, foundational crates every other crate in the workspace either depends on directly or benefits from indirectly: `tre-math` (pure, stateless 2D transform math) and `tre-memory` (the lock-free ring buffers, arenas, and publish tables that give the engine its zero-allocation steady state). Neither crate depends on `tre-engine` -- both are usable standalone, and both are cited by name from doc comments elsewhere in the engine as "the real intended caller" for specific functions, even though neither actually imports its consumer.

## `tre-math`

```python
# crates/tre-math -- forbids unsafe entirely
```

Vector/matrix math, affine transform batching, and SIMD-accelerated path interpolation, built on the [`wide`](https://docs.rs/wide) crate's safe portable-SIMD API. Because `wide`'s public surface is safe Rust, `tre-math` needs no `unsafe` of its own -- the crate carries `#![forbid(unsafe_code)]`. It is deliberately a **stateless evaluation library**: the UI framework's own widget tree owns animation state, not this crate.

### `Affine2`

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Affine2 {
    pub a: f32,
    pub b: f32,
    pub tx: f32,
    pub c: f32,
    pub d: f32,
    pub ty: f32,
}
```

A 2D affine transform, stored as the six meaningful values of the canonical `[[a, b, tx], [c, d, ty], [0, 0, 1]]` matrix -- the bottom row is always `[0, 0, 1]` for any genuine affine transform, so storing a full dense 3x3 (9 floats) would waste memory and SIMD lanes for no benefit. Every `Affine2` this API can construct is guaranteed affine; it can never represent a general projective transform.

`Affine2::IDENTITY` is the identity transform (`a=1, d=1`, everything else `0`).

| Method | Signature | Notes |
|---|---|---|
| `to_array` | `pub const fn to_array(self) -> [f32; 6]` | `[a, b, tx, c, d, ty]`, for GPU upload or an FFI boundary. |
| `from_array` | `pub const fn from_array(v: [f32; 6]) -> Self` | Inverse of `to_array`. |
| `from_translation` | `pub const fn from_translation(tx: f32, ty: f32) -> Self` | Pure translation. |
| `from_rotation` | `pub fn from_rotation(theta: f32) -> Self` | Counterclockwise, radians. Not `const` (uses `sin_cos`). |
| `from_scale` | `pub const fn from_scale(sx: f32, sy: f32) -> Self` | A negative component is a legitimate flip, not an error. |
| `from_translation_rotation_scale` | `pub fn from_translation_rotation_scale(translation: [f32; 2], rotation: f32, scale: [f32; 2]) -> Self` | One UI node's local transform in a single call. |
| `compose` | `pub fn compose(&self, child: &Self) -> Self` | `self` is the parent. `self.compose(&child).transform_point(p) == self.transform_point(child.transform_point(p))`. Not commutative. |
| `transform_point` | `pub fn transform_point(&self, point: [f32; 2]) -> [f32; 2]` | Applies the transform to a point. |
| `invert` | `pub fn invert(&self) -> Option<Self>` | `None` if `self` is degenerate (determinant magnitude at or below `f32::EPSILON`). |

`invert`'s doc comment names `tre-engine`'s `ShapeRegistry::hit_test` directly as its real intended first caller -- mapping a world-space hit-test point back into a shape's own local space, the exact inverse of what `flatten_into`/`to_affine2` already does forward.

### Batch functions

```rust
pub fn compose_batch(parents: &[Affine2], children: &[Affine2], out: &mut [Affine2])
pub fn lerp_points_batch(from: &[[f32; 2]], to: &[[f32; 2]], t: f32, out: &mut [[f32; 2]])
```

SIMD-batched versions of `Affine2::compose` and point-lerp, processing 8 items at a time via `wide::f32x8` with a scalar fallback for the `% 8` remainder. Both write into a caller-supplied `out` slice rather than allocating, since a per-frame flattening/morphing caller cannot allocate on that path. Both **panic** (not `Result`) if their input/output slices don't all share the same length -- a length mismatch is a programmer error, not a recoverable runtime condition.

`lerp_points_batch`'s doc comment names `tre-svg::morph` as the expected caller, with an explicit contract split: this function assumes already-equal-length inputs, and it is `tre-svg::morph`'s own job to validate topological equivalence between two independently-parsed keyframe shapes *before* ever calling it.

!!! note "An open, disclosed performance question"
    A large doc comment on the private `gather` helper (`compose_batch`'s per-lane extraction loop) discloses that `gather` is a scalar extraction loop, not a real hardware gather instruction -- `compose_batch` calls it 12 times per 8-wide chunk to feed only 6 vector FMA/mul instructions. The comment states plainly that "for AoS-shaped real data ... it is not obvious this 'SIMD' path beats 8 independent scalar `Affine2::compose` calls," and that no criterion benchmark exists in the workspace to settle it. This is a real, tracked, unresolved question -- not a claim that the SIMD path is faster.

### Other free functions

```rust
pub fn tone_map(linear: f32, headroom: f32) -> f32
pub fn spring_decay(current: f32, target: f32, lambda: f32, dt: f32) -> f32
```

- **`tone_map`** -- the engine's HDR-to-SDR tone-mapping curve: identity at or below standard white (`linear <= 1.0`), Reinhard-style compression above it, continuous and monotonic across the boundary. `headroom` is the display's real reported HDR headroom in SDR-white multiples (e.g. `3.0`). A pure primitive only -- its doc comment discloses that `tre-rhi-vulkan`'s `VulkanSwapchain::new` never actually selects a genuine HDR-capable surface on any hardware available to this project today, so there is no real trigger to call this yet.
- **`spring_decay`** -- frame-rate-independent exponential decay of `current` toward `target`: `x(t+dt) = target + (x(t) - target) * e^(-lambda*dt)`. Pure exponential smoothing, **not** a mass-spring-damper ODE -- monotonic, never overshoots `target`. This is the deliberate contrast with `tre-tween::Spring` (see [Animation](animation.md)), which *can* overshoot and oscillate. Its doc comment names `FrameClock::tick` as the expected source of `dt`.

None of `tre-math`'s public functions panic except `compose_batch`/`lerp_points_batch`'s length-mismatch asserts -- every other function is pure arithmetic with no assertions.

## `tre-memory`

```python
# crates/tre-memory -- one of the workspace's `unsafe`-permitted crates
```

Zero-allocation triple-buffered ring arenas, the transient render-target pool's bookkeeping primitives, and the dynamic atlas's lock-free MPSC request queue plus single-writer/multi-reader publish table. Every internal module (`alloc_guard`, `mpsc`, `scatter`, `spsc`, `swmr`) is **private** -- everything is re-exported flat at the crate root, so a caller only ever sees `tre_memory::{DebugAllocGuard, RenderTickGuard, MpscRingBuffer, ScatterArena, ScatterSlice, SpscRingBuffer, SwmrSlotTable}`, never a `tre_memory::mpsc::...` path.

### `DebugAllocGuard` / `RenderTickGuard`

A debug-build-only enforcement pair for TECHNICAL.md's "zero allocations per frame" budget. Lives in `tre-memory` rather than `tre-engine` specifically because implementing `GlobalAlloc` requires `unsafe`, and `tre-engine` carries `#![forbid(unsafe_code)]`.

```rust
pub struct DebugAllocGuard; // implements GlobalAlloc, wraps std::alloc::System

pub struct RenderTickGuard { /* private */ }
impl RenderTickGuard {
    pub fn begin() -> Self;
}
```

- **`DebugAllocGuard`** is installed by a *binary* that opts in (e.g. `main_loop_demo.rs`) via `#[global_allocator]` -- never by the library itself, since that attribute is whole-binary-scoped. Every allocation it intercepts checks a thread-local flag; if a `RenderTickGuard` is active on that thread, it panics: `"heap allocation observed while a RenderTickGuard was active (TECHNICAL.md Section 3.4) -- the 0 bytes/frame zero-allocation budget was violated"`. In release builds (`cfg(not(debug_assertions))`) every check is a no-op.
- **`RenderTickGuard::begin()`** is an RAII scope guard: construct one at the start of a frame's budgeted CPU work, drop it at the end. The flag is **thread-local by design** -- a multi-threaded recording scheme (`SubCanvas` workers) needs its own guard started on each thread whose allocations should be checked. Guards do not nest: calling `begin()` while one is already active on the same thread panics.

Both use a "disarm before panic" pattern -- the thread-local flag is cleared *before* the panic message is raised, specifically to avoid a double-panic if the panic machinery's own formatting allocates.

### `MpscRingBuffer<T>`

A bounded, pre-allocated lock-free Multi-Producer Single-Consumer ring buffer, implementing a simplified form of Dmitry Vyukov's bounded MPMC design (each slot carries its own atomic sequence number, resolving the producer-side race without a global lock; simplified here since only one thread is ever popping). First real use: the dynamic atlas's `AtlasInsertRequest` channel, carrying requests from any number of producer threads to the single atlas owner (see [Text, SVG & Atlas](text-svg-atlas.md)).

```rust
impl<T> MpscRingBuffer<T> {
    pub fn with_capacity(capacity: usize) -> Self;   // panics if capacity == 0
    pub fn push(&self, item: T) -> Result<(), T>;    // Err returns the item back if full
    pub fn pop(&self) -> Option<T>;                  // single consumer only
    pub fn is_empty(&self) -> bool;
    pub fn capacity(&self) -> usize;
}
```

`pop` **must only ever be called from a single consumer thread** -- this is the one place the type's contract is genuinely MPSC, not MPMC. `is_empty` is a best-effort snapshot under concurrent producers, not an authoritative fact. `Drop` drains any still-queued items via `pop`.

### `ScatterArena<T>` / `ScatterSlice<'a, T>`

*(`T: Copy` throughout.)* A fixed-capacity, lock-free arena where any number of threads reserve disjoint, contiguous output ranges via a single atomic `fetch_add`-equivalent, then write into them without ever taking a lock. Unlike `MpscRingBuffer`, nothing here is ever popped or reused mid-frame -- every reservation is exclusive until the whole arena is consumed once, at frame's end. First real use: `tre-engine`'s `FrameArena`, merging a `SubCanvas`'s locally-recorded vertices/indices/commands into one shared destination.

```rust
impl<T: Copy> ScatterArena<T> {
    pub fn with_capacity(capacity: usize) -> Self;
    pub fn capacity(&self) -> usize;
    pub fn reserve(&self, count: usize) -> Option<ScatterSlice<'_, T>>;
    pub fn into_vec(self) -> Vec<T>;             // consumes self
    pub fn reset(&mut self);                     // reuse without reallocating
    pub fn drain_into(&mut self, out: &mut Vec<T>); // extract + reset in one call
}

impl<T> ScatterSlice<'_, T> {
    pub fn start_index(&self) -> usize;
    // Deref/DerefMut to [T]
}
```

`reserve` returns `None` (report, never grow) if the reservation would exceed capacity. `into_vec`/`reset`/`drain_into` all require that every `ScatterSlice` this arena ever granted has finished writing and been dropped first -- `reset`/`drain_into` take `&mut self`, so the borrow checker itself enforces that no outstanding `ScatterSlice` (which only ever borrows `&self`) can still exist.

!!! note "REVIEW.md finding #132"
    `reserve` uses `fetch_update`, deliberately, not a bare `fetch_add`. An unconditional `fetch_add` would advance `len` *before* checking whether the reservation fits -- since `len` only increases, one overflowing call would permanently leave `len` above `capacity` for the arena's entire remaining lifetime, poisoning every later `reserve` call from any thread, however small. `fetch_update`'s closure only commits the advance when it actually fits, so a failed reservation leaves `len` untouched. A dedicated regression test reproduces this exact scenario.

### `SpscRingBuffer<T>`

The canonical Single-Producer Single-Consumer lock-free ring buffer -- the engine's original ring buffer (OS input events, one consumer: the UI framework's logic tick), which `MpscRingBuffer` above generalizes to multiple producers.

```rust
impl<T> SpscRingBuffer<T> {
    pub fn with_capacity(capacity: usize) -> Self;  // panics if capacity == 0
    pub fn push(&self, item: T) -> Result<(), T>;
    pub fn pop(&self) -> Option<T>;
    pub fn is_empty(&self) -> bool;
    pub fn capacity(&self) -> usize;                // hides one internal sentinel slot
}
```

Allocates `capacity + 1` internal slots -- the classic trick for distinguishing "full" from "empty" without a separate length counter; `capacity()` reports the real usable capacity, hiding that detail.

### `SwmrSlotTable<K>`

*(`K: Copy + Eq + Into<u64>`.)* A fixed-capacity, open-addressed **S**ingle-**W**riter/**M**ulti-**R**eader publish table: the atlas owner is the only writer (`Ordering::Release` stores), and any window's rendering thread reads (`Ordering::Acquire` loads) without ever taking a lock or performing a CAS. Unusually for this crate, it needs **no** `unsafe` at all -- every per-slot field (key, value, recency timestamp) is a plain `AtomicU64`.

```rust
impl<K: Copy + Eq + Into<u64>> SwmrSlotTable<K> {
    pub fn with_capacity(capacity: usize) -> Self;              // panics if capacity == 0
    pub fn insert(&self, key: K, value: u64) -> bool;           // single writer only
    pub fn remove(&self, key: K) -> bool;                       // single writer only
    pub fn get(&self, key: K) -> Option<u64>;                   // any number of readers
    pub fn get_and_touch(&self, key: K, frame: u64) -> Option<u64>; // get + recency stamp
    pub fn scan_older_than(&self, cutoff_frame: u64, visit: impl FnMut(u64, u64));
    pub fn capacity(&self) -> usize;
}
```

Linear probing with tombstones (textbook, not novel): `insert`/`remove` are single-writer-only; `get`/`get_and_touch` are safe from any number of concurrent readers, concurrently with the writer. `remove` stores a `TOMBSTONE_KEY` sentinel rather than reverting to empty -- reverting to empty would let a *different* key's probe sequence, which happens to pass through the same slot, stop early and report a false miss for an entry still present further along. `get_and_touch` records recency via `AtomicU64::fetch_max`, so a lower frame number never regresses an already-recorded one. `scan_older_than` is infrastructure for a not-yet-built eviction policy, returning raw `u64` keys (the caller reconstructs its own `K`).

!!! note "REVIEW.md finding #131"
    `get`'s doc comment documents a real ABA bug: a reader could observe a torn read if the writer evicted a key and immediately reused its slot for a different key mid-read. A first fix attempt (re-checking the key once after reading the value) was insufficient. The actual fix is a 4-point seqlock -- a per-slot `epoch` counter read before and after the value/key, requiring both matching and even for the read to be trusted. A dedicated 50,000-round concurrent regression test (`a_reader_never_observes_a_different_keys_value_when_the_writer_evicts_and_immediately_reuses_its_slot`) proves the fix holds under real concurrency.

## Cross-crate references worth knowing

Both crates are used before their consumers formally depend on them -- several public functions document a real, specific, *not-yet-wired-up* caller by name:

| Function | Documented (not yet actual) caller |
|---|---|
| `Affine2::invert` | `tre-engine`'s `ShapeRegistry::hit_test` |
| `lerp_points_batch` | `tre-svg::morph` |
| `tone_map` | `tre-rhi-vulkan`'s `VulkanSwapchain::new` (blocked on real HDR-capable hardware) |
| `MpscRingBuffer` + `SwmrSlotTable` | the dynamic atlas's background owner thread (two halves of the same subsystem -- see [Text, SVG & Atlas](text-svg-atlas.md)) |
| `ScatterArena` | `tre-engine`'s `FrameArena`/`SubCanvas` multi-threaded recording (see [Canvas & Intermediate Representation](canvas-and-ir.md)) |
