# Plan: Phase 4, Step 4.2.1 -- Guillotine Atlas Bin-Packing

## Scope decisions (confirmed with the project owner, 2026-09-06)

**IMPLEMENTATION.md's Step 4.2 bundles four largely independent chunks of
work** (Guillotine bin-packing, MSDF glyph generation, the MSDF evaluation
shader, and multi-window atlas concurrency) -- comparable in kind to Step
3.3's four-chunk bundle that got split into 3.3.1-3.3.3. Per the project
owner's direction, this is split the same way: 4.2.1 (this plan, the
packer itself), 4.2.2 (MSDF glyph generation), 4.2.3 (the evaluation
shader plus a real anti-aliased glyph rendered end-to-end -- the step that
directly resolves the jagged 'X' from Step 4.1's demo), and 4.2.4
(multi-window atlas concurrency, last, since it's about scaling to many
windows rather than visible quality, and needs 4.2.1-4.2.3 already proven
as its single-window foundation).

**MSDF generation itself (4.2.2, not this plan) will use `fdsm`, a real
pure-Rust reimplementation of msdfgen's actual published algorithm**
(edge coloring, true/pseudo-distance handling, sign correction --
following Chlumský's own thesis, not a simplified approximation), rather
than hand-rolling this specific algorithm from scratch, per the project
owner's direction given how failure-prone a first attempt at genuine MSDF
generation is and how much "getting text correct" matters here. Noted now
because it shapes this plan's own crate-boundary decision below, even
though `fdsm` isn't a dependency of this sub-step itself.

**A new crate, `tre-atlas`, not code inside `tre-text` or `tre-rhi-vulkan`.**
DESIGN.md Section 10.2/ARCHITECTURE.md Section 2.3 describe the dynamic
texture atlas as a resource shared across MSDF glyphs *and* plain-color UI
vector icons/decals -- not exclusively a text concern, so it doesn't
belong in the font-specific `tre-text` crate. It's also pure CPU
bookkeeping (a free-rectangle list and simple 2D geometry, no GPU handles,
no `ash` dependency) -- matching `tre-svg`'s and `tre-math`'s own
"backend-agnostic, no RHI dependency" precedent, not `tre-rhi-vulkan`'s.
This crate will also eventually hold Step 4.2.4's `AtlasKey`/`AtlasSlot`
concurrency primitives (ARCHITECTURE.md Section 2.3's design), but this
sub-step only needs the packer itself. `#![forbid(unsafe_code)]`, matching
every other non-RHI crate in this workspace.

**Insertion only, no removal/eviction this sub-step.** IMPLEMENTATION.md's
Step 4.2 task list names insertion/packing, not eviction -- LRU-based
atlas eviction (DESIGN.md Section 10.2) is separately-described future
work, the same category of "named elsewhere, not yet scheduled" as this
step's own multi-window concurrency was before this plan explicitly
picked it up as 4.2.4. `AtlasPacker::insert` returns `None` when nothing
fits; deciding what a caller does with that (placeholder-glyph fallback,
DESIGN.md Section 2.6) is a later step's concern, not this one's.

**No free-rectangle merging.** A classic, simple Guillotine packer (the
kind IMPLEMENTATION.md's task 1 describes -- "maintain a list of free
rectangles... find the best fit and split... horizontally or vertically")
accepts some fragmentation over a long insertion sequence rather than
merging adjacent free rectangles back together after a split; that's a
deliberate, documented simplification here, not an oversight -- revisit
only if a concrete fragmentation problem shows up in a later step's real
usage.

**Heuristics: Best Area Fit for choosing which free rectangle to use,
shorter-leftover-axis for how to split it.** Both are named, standard
choices from the same body of packing-algorithm literature
IMPLEMENTATION.md's task 1 references (MaxRects/Guillotine), picked for
being simple to reason about and to unit-test deterministically, not
because they're provably optimal -- a different heuristic can replace
either later without changing the packer's public API.

## Goal

Given a sequence of `(width, height)` rectangle requests against a
fixed-size atlas, produce non-overlapping placements via a real Guillotine
split/best-fit algorithm, correctly reporting failure once the atlas is
full -- proven both by unit tests checking a real overlap invariant across
many insertions, and by a visual demo that renders every placed rectangle
as a distinct flat-colored quad through the existing pipeline, so the
packing can be inspected by eye as well as asserted.

## Tasks

1. **New `tre-atlas` crate** (`crates/tre-atlas`), added to the workspace
   `Cargo.toml`. No dependencies beyond the standard library --
   `#![forbid(unsafe_code)]`.

2. **`PackedRect { x: u32, y: u32, width: u32, height: u32 }`** and
   **`AtlasPacker`** (`AtlasPacker::new(width: u32, height: u32) -> Self`,
   `AtlasPacker::insert(&mut self, width: u32, height: u32) ->
   Option<PackedRect>`, exact names/shapes TBD during implementation):
   internally holds a `Vec` of free `PackedRect`s (seeded with one
   full-atlas rectangle), a real Best-Area-Fit search over the free list
   on each `insert`, and a real Guillotine split of the chosen free
   rectangle into up to two new free rectangles along whichever axis
   leaves the larger single leftover piece -- removing the chosen free
   rectangle and pushing its replacement(s) back onto the list. Returns
   `None` (no panic, no silent wraparound) when no free rectangle is large
   enough.

3. **Overlap-invariant unit tests**: insert a real sequence of varied
   rectangle sizes (not all-identical, which would hide axis-choice bugs)
   into a small atlas, assert every pair of *returned* placements is
   non-overlapping, assert every placement stays fully within the atlas
   bounds, and assert a request too large for any remaining free space
   returns `None` rather than an incorrect placement. Also a hand-picked,
   exactly-fills-the-atlas case (e.g. two rectangles that together tile a
   known small atlas exactly) as a fully worked-out, human-verifiable
   example alongside the property-style checks.

4. **New example** (`crates/tre-rhi-vulkan/examples/atlas_packing_demo.rs`,
   `demo/phase4_step4_2_1/`): packs a real sequence of differently-sized
   rectangles (deliberately varied, mimicking a realistic mix of glyph and
   icon sizes rather than uniform squares) into a modest atlas (e.g.
   256x256, small enough to see individual rectangles clearly in a
   screenshot), renders each successfully-placed rectangle as a distinct
   flat-colored quad via the existing, unmodified flat-color pipeline, and
   reads back real pixels: a probe inside each placed rectangle is that
   rectangle's own color, and a probe in whatever atlas space is left
   unpacked is the background -- proving the returned coordinates are
   real, correctly non-overlapping placements, not just "some `Option`
   came back `Some`."

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace, including `tre-atlas`'s own `#![forbid(unsafe_code)]`.
- `atlas_packing_demo` run under `VK_LAYER_KHRONOS_validation`, zero
  errors -- it reuses the existing, unmodified flat-color pipeline (four
  vertices/six indices per rectangle, the same shape every rounded-rect
  and flat-fill primitive already uses), so this is confirming no
  regression, not exercising new RHI surface area.
- All 12 pre-existing Vulkan examples re-run manually, unaffected (this
  sub-step touches no RHI/vertex-format code at all).
- CI: add `atlas_packing_demo` to the `vulkan-validation` job's example
  list; push, confirm green.

## Explicitly out of scope for this sub-step

- MSDF glyph generation (task 2, via `fdsm`) -- Step 4.2.2.
- The MSDF evaluation shader and a real anti-aliased glyph render (task
  3) -- Step 4.2.3.
- Multi-window atlas concurrency: the bounded MPSC `AtlasInsertRequest`
  ring buffer, the `AtomicU64` `AtlasSlot` publish table, and the
  single-atlas-owner thread (task 4, ARCHITECTURE.md Section 2.3) --
  Step 4.2.4. This sub-step's `AtlasPacker` is plain, single-threaded,
  `&mut self` state; it becomes the one thing only Step 4.2.4's atlas
  owner is ever allowed to touch, per that section's own design.
- Eviction/removal from the packed atlas (LRU-based reclamation, DESIGN.md
  Section 10.2) -- not named in this step's own task list; revisit when a
  later step actually needs to free atlas space.
- Free-rectangle merging/defragmentation -- a deliberate simplification,
  not a gap; see "Scope decisions" above.
- Wiring the packer into any real glyph/icon insertion path or
  `RenderingCanvas`'s public API -- proven directly via this sub-step's
  own dedicated demo first, matching this project's "prove the primitive
  before its real consumer exists" precedent.
