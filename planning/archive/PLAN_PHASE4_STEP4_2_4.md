# Plan: Phase 4, Step 4.2.4 -- Multi-Window Atlas Concurrency

## Scope decisions

**The closing sub-step of the whole Step 4.2 arc.** Everything built in
4.2.1-4.2.3 (the packer, the MSDF generator, the evaluation shader) has
so far been proven one glyph, one thread, at a time. This sub-step wires
them together behind the real concurrency model ARCHITECTURE.md Section
2.3/TECHNICAL.md Section 8 already specify, so multiple independent
producers (real OS threads, standing in for what will eventually be
per-window worker threads once Phase 5's `SubCanvas` infrastructure
exists) can request atlas space concurrently without ever blocking on
each other or on the single atlas owner.

**Two new concurrency primitives, split across two crates by the same
reasoning Step 4.2.1 already used for the packer itself:**

- **`tre_memory::MpscRingBuffer<T>`** -- a new, fully generic primitive
  (not atlas-specific), living alongside the existing
  `tre_memory::SpscRingBuffer<T>` it explicitly generalizes (TECHNICAL.md
  Section 8's own wording: "generalizes the SPSC ring buffer already used
  for OS input events... to the multi-producer case"). This is also where
  the real `unsafe` code belongs -- TECHNICAL.md Section 9.1's `unsafe`
  policy groups "the atlas's MPSC/SWMR concurrency primitives" under the
  *same* "ring-buffer/arena allocators" exemption `SpscRingBuffer` already
  uses, i.e. `tre-memory`'s, not a new exemption for `tre-atlas`. Real
  multi-producer correctness is genuinely harder than the existing SPSC
  case (multiple threads can race to claim the same slot), implemented as
  a bounded MPMC ring buffer restricted to one consumer (Dmitry Vyukov's
  well-known bounded MPMC queue design -- per-slot sequence numbers
  resolve the producer-side race; the single-consumer side needs no CAS
  at all, simplifying that half relative to a full MPMC queue).
- **A generic `tre_memory::SwmrSlotTable<K>`** (fixed-capacity,
  open-addressed, `K: Copy + Eq + Hash`, `AtomicU64` values) -- corrected
  after actually reading `tre-memory`'s own crate doc comment, which
  already says this crate is for "the dynamic atlas's lock-free MPSC
  request queue *plus* single-writer/multi-reader publish table," written
  before this plan existed. `tre-atlas` supplies only what's genuinely
  atlas-specific on top: the `AtlasKey` type itself and the
  pack/unpack logic turning a `(PackedRect, generation)` pair into the
  `u64` payload -- the open-addressing/hashing mechanics live in the
  generic table, not duplicated per key type.

**`AtlasInsertRequest` carries a boxed rasterization callback, not a
direct dependency on `tre-text`.** Step 4.2.1 deliberately kept
`tre-atlas` content-agnostic (shared by glyphs *and* icons); this step
must not quietly break that by making the atlas crate depend on the font
crate just to call `generate_msdf`. ARCHITECTURE.md's own field name --
`raster_source: RasterSourceHandle` -- already implies a handle to a
deferred operation, not raw pixels up front. Modeled here as a small
`RasterSource` trait (`fn size(&self) -> (u32, u32); fn rasterize(&self)
-> Vec<u8>;`), boxed and `Send`. `tre-text`'s own demo-side glue
implements it by calling `tre_text::generate_msdf` inside `rasterize`;
`tre-atlas` itself never needs to know that's what's happening.

**The atlas owner runs on a real, dedicated background thread**, not a
per-frame drain on whichever thread happens to call it -- the same
precedent Phase 2 Step 2.3 already established for the generational GC
thread, and the more faithful reading of TECHNICAL.md Section 8's actual
goal ("no window ever blocks on another window's atlas insertion": a
thread that is *always* draining, independent of any window's own frame
timing, rather than one that only drains when some window's frame
happens to trigger it).

**No eviction, no generation-counter reuse semantics exercised yet** --
consistent with Step 4.2.1's own already-stated deferral (LRU reclamation
is separate, DESIGN.md Section 10.2 future work). The packed `AtomicU64`
slot format includes a generation-counter field so the *format* is
forward-compatible, but every slot in this step is write-once; nothing
here tests reuse.

**The demo's shared atlas texture is uploaded to the GPU once, at the
end, not incrementally per-insertion.** This project's RHI has no
"update a sub-region of an existing texture" operation yet (Step 2.1's
`create_texture` is upload-once); adding one would be real, separate RHI
scope creep beyond "implement the... ring buffer... and... slot table."
The demo instead maintains the shared atlas as a plain CPU-side pixel
buffer during the concurrent phase (exactly what a real implementation
would also need internally before any GPU upload), and uploads the
finished buffer as one real texture afterward -- proving the concurrency
model and the eventual GPU consumption without inventing new RHI surface
this step doesn't call for.

## Goal

Several real producer threads request atlas space for several different
real glyphs concurrently, through the real bounded MPSC ring buffer;
a single, real background atlas-owner thread drains requests, performs
real Guillotine packing (Step 4.2.1) and real MSDF generation (Step
4.2.2) for each, and publishes results into the real SWMR slot table;
readers (including the producer threads themselves, after rejoining)
observe correct, non-overlapping, correctly-generated results with no
lost or duplicated requests -- proven under genuine concurrent stress,
not simulated with a single thread standing in for many. As a capstone,
the fully-packed shared atlas is rendered in one frame via Step 4.2.3's
existing MSDF shader, every glyph's real, concurrently-obtained UV rect
used unmodified.

## Tasks

1. **`tre_memory::MpscRingBuffer<T>`** (`crates/tre-memory/src/mpsc.rs`,
   exact name TBD): bounded, pre-allocated (matching `SpscRingBuffer`'s
   own "allocate once at construction" contract), `push(&self, item: T)
   -> Result<(), T>` callable concurrently from any number of producer
   threads, `pop(&self) -> Option<T>` for the single consumer. Per-slot
   atomic sequence numbers (Vyukov's design) resolve concurrent producer
   slot claims; real `// SAFETY:` comments on every `unsafe` block,
   matching `SpscRingBuffer`'s own documentation standard.

2. **`tre_memory::SwmrSlotTable<K>`** (generic, `crates/tre-memory/src/swmr.rs`):
   fixed-capacity `Box<[AtomicU64]>`, open-addressed (linear probing) on
   `K: Copy + Eq + Hash`, one writer (`Ordering::Release` publish),
   any number of readers (`Ordering::Acquire` load) -- entries are
   add-only, never removed in place, matching TECHNICAL.md Section 8's
   own stated access pattern. In `tre-atlas`: `AtlasKey` (the concrete key
   type) and pack/unpack functions turning a `(PackedRect, generation)`
   pair into the raw `u64` payload, with a reserved sentinel value
   meaning "not yet resident" (matching DESIGN.md Section 2.6's
   placeholder-glyph fallback contract: a reader seeing the sentinel
   falls back and re-checks later, never blocks).

3. **`RasterSource` trait + `AtlasInsertRequest`** (`tre-atlas`): the
   content-agnostic rasterization handle described above.

4. **`AtlasOwner`** (`tre-atlas`): owns the `AtlasPacker` (Step 4.2.1,
   exclusively -- the one piece of state only this type's own background
   thread ever touches), the shared CPU-side atlas pixel buffer, the
   `Arc`-shared `MpscRingBuffer<AtlasInsertRequest>` and slot table.
   `spawn(atlas_width, atlas_height) -> (AtlasOwnerHandle, JoinHandle)`
   (exact shape TBD): starts the real background thread, which loops
   draining the queue, and for each request calls `size()`+`rasterize()`,
   packs via `AtlasPacker::insert`, copies the rasterized bytes into the
   shared CPU buffer at the packed offset, and publishes the slot.
   `AtlasOwnerHandle` (the `Arc`-cloneable, `Send`/`Sync` producer-facing
   type) exposes `request_insert`/`lookup`.

5. **New example**
   (`crates/tre-rhi-vulkan/examples/atlas_concurrency_demo.rs`,
   `demo/phase4_step4_2_4/`): spawns a real `AtlasOwner` background
   thread over a modest shared atlas (e.g. `256x256`, matching Step
   4.2.1's own demo scale); spawns several real producer threads
   (standing in for future per-window worker threads), each concurrently
   requesting MSDF space for a handful of distinct real glyphs from a
   real cascade font (a mix including at least one hole-having glyph);
   joins every producer, then polls the slot table until every requested
   key resolves; asserts every returned rect is non-overlapping and
   matches an independently-recomputed MSDF for that same glyph (byte
   comparison, not just "a slot exists"); uploads the finished shared
   atlas as one real GPU texture and renders every packed glyph in one
   frame via the existing, unmodified `msdf.frag` pipeline, reading back
   real pixels to confirm each glyph rendered correctly at its real,
   concurrently-obtained UV rect.

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace, including a real multi-threaded stress test for
  `MpscRingBuffer` (many real producer threads, one consumer, asserting
  no lost/duplicated items -- the MPSC analogue of `SpscRingBuffer`'s own
  `genuinely_concurrent_producer_and_consumer_never_lose_or_duplicate_items`
  test) and for the full `AtlasOwner` round trip.
- `atlas_concurrency_demo` run under `VK_LAYER_KHRONOS_validation`, zero
  errors.
- All 14 pre-existing Vulkan examples re-run manually, unaffected.
- CI: add `atlas_concurrency_demo` to the `vulkan-validation` job's
  example list; push, confirm green.

## Explicitly out of scope for this sub-step

- LRU eviction and generation-counter reuse (DESIGN.md Section 10.2) --
  the slot format supports it; nothing here exercises it. Consistent with
  Step 4.2.1's own deferral.
- Incremental/partial GPU texture updates -- this project's RHI has no
  such operation yet; the demo uploads the finished CPU atlas buffer once,
  at the end.
- Real per-window worker threads (Phase 5's `SubCanvas` infrastructure) --
  this step's producer threads are a deliberate stand-in proving the
  concurrency primitives work under genuine concurrent stress, not a
  claim that per-window threading itself now exists.
- Wiring any of this into `RenderingCanvas`'s public API -- proven
  directly via this sub-step's own dedicated demo first, matching this
  project's "prove the primitive before its real consumer exists"
  precedent throughout every phase so far.
- Icon/vector-decal atlas entries -- this step's own demo only exercises
  glyphs (via the `RasterSource` trait's `tre-text`-side implementation);
  the trait itself is content-agnostic, but nothing here builds an icon
  rasterizer to prove that half.
