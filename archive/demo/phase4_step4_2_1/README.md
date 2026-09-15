# Demo: Phase 4, Step 4.2.1 -- Guillotine Atlas Bin-Packing

```bash
./demo/phase4_step4_2_1/run_atlas_packing_demo.sh
```

**A real Guillotine bin-packer, not a placeholder.** `tre_atlas::AtlasPacker`
maintains a free-rectangle list, finds the tightest-fitting free rectangle
for each request (Best Area Fit), and splits it into exactly two
non-overlapping leftover rectangles -- a genuine partition of the
"L-shaped" leftover region, not the overlapping candidate-rectangle set a
MaxRects packer would keep instead. Which of the two possible cuts to
make is chosen by actually building both candidates and comparing their
larger resulting piece's area, not approximated from raw leftover
dimensions alone -- a worked example in `tre-atlas`'s own unit tests shows
why the shortcut approximation gives the wrong answer on a real case.

**A new crate, `tre-atlas`**, not folded into `tre-text` or
`tre-rhi-vulkan` -- ARCHITECTURE.md Section 2.3/DESIGN.md Section 10.2
describe the dynamic texture atlas as shared by MSDF glyphs *and*
plain-color UI icons/vector decals, not a text-only concern, and the
packer itself is pure CPU bookkeeping with no GPU dependency at all.

**Verified by rendering the actual packing, not just asserting `Option`s.**
This demo packs 12 deliberately varied rectangle sizes (mimicking a
realistic mix of small glyph-sized and larger icon-sized atlas entries)
into a 256x256 atlas, draws every successfully-placed rectangle as its
own distinct flat color through the pre-existing, unmodified flat-color
pipeline, and reads back real pixels: every rectangle's own center is
exactly its own color, and a point the packer never used is still the
background -- so the returned coordinates can be inspected by eye in the
output PNG as well as asserted.

**A real, previously-undiscovered engine finding surfaced along the way**
(documented in `documentation/REVIEW.md`'s "Phase 4 Step 4.2.1
Implementation" section): `walking_skeleton.frag` never actually performs
the sRGB-to-linear conversion `UiVertex::color`'s own doc comment promises
-- every prior demo used exclusively pure white/black fills, which are
fixed points of any gamma curve, so this went unnoticed until this demo
became the first to render genuinely distinct mid-tone colors. Confirmed
via `TECHNICAL.md` Section 6's own canonical formula that this is
IMPLEMENTATION.md Step 7.1's explicit, already-scheduled job, not a
regression to patch here -- this demo's own palette deliberately uses only
the 7 "pure" (each channel 0 or 255) colors, which round-trip correctly
both today and after Step 7.1 lands, so nothing about this demo needs
revisiting once that step is built.
