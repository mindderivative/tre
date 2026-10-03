# Scene building: where the CPU time goes

0.5.4 research (#107). A full redraw spent about 2.7 µs per drawn node building
its scene (6.2 ms for 2,304 cards). This page says what that time is, what was
tried, and what is worth doing. One machine (AMD Radeon 880M, Linux); the
numbers are for comparing options, not for promising a figure elsewhere.

Method: `engine-render/tests/scene_cost.rs` builds a scene for about 1,150
nodes and times `build_tree_scene` (the CPU walk and vello's command
recording, including its path flattening and strip generation) separately from
`render_into` plus submit. Run it with `cargo test -p engine-render --release
--test scene_cost -- --ignored --nocapture`. Runs vary by about 30%; read the
ratios.

## What a node costs

| Node | Build per node | Render and submit per node |
| --- | --- | --- |
| Square box, `fill_path` (before) | 0.9 µs | 0.25 µs |
| Square box, `fill_rect` (now) | 0.2 µs | 0.09 µs |
| Rounded box (radius 6) | 2.1 to 3.0 µs | 0.3 to 0.4 µs |
| Square box with 1 px border, stroke (before) | 2.6 µs | 0.28 µs |
| Square box with 1 px border, four rects (now) | 0.7 to 0.9 µs | 0.27 µs |
| Rounded box with 1 px border | 9 µs | 0.5 µs |
| Rounded box at opacity 0.9 (a layer) | 2.3 µs | 0.7 µs |
| 14 px label, 36 characters, shaping cached | 11 µs | 0.55 µs |

Findings:

- **Curves are the cost.** A square box is nearly free; each rounded corner adds
  flattening and strip generation on the CPU, which vello_gpu does in the
  calling thread (it has no thread setting in this revision). A rounded
  border costs about three times a rounded fill.
- **Text is the largest per-node cost, by far**: 11 µs for a short label, about
  300 ns per glyph, with the shaped layout already cached. Reducing it needs a
  glyph cache or a retained text run, which is the biggest remaining lever.
- **Layers are cheap to build, dearer to render**: an opacity layer adds about
  0.4 µs on the GPU side per node. Avoiding one at opacity 1 is already done.
- **`Scene::new` per frame costs 0.6 µs.** Reusing it would save nothing.

## What was done

Two exact fast paths, with pixel tests (`square_box_paint.rs`):

- A box with no corner radius is filled with `fill_rect`: 4x cheaper to build.
- A box with no corner radius and a border of 1 px or less draws the border as
  four rects: 3x cheaper. Wider borders keep the stroke, because `Stroke::new`
  joins with round joins, which rounds a wide border's outer corners by half
  its width; rects would square them, a visible change from today.

## What was tried and rejected

- **Analytic rounded rect** (`fill_blurred_rounded_rect` with a small blur, no
  path): 6x cheaper to build (0.5 µs), but not the same pixels. Against a path
  fill, edge pixels differ by 8 to 40 of 255 on average and up to 90 at rounded
  corners, for every blur width from 0.25 to 0.5 (`analytic_fill_quality.rs`).
  Rejected as a default; it would soften every rounded edge.
- **A border as a filled ring** (outer path minus inner path) instead of a
  stroke: no faster than the stroke (about 10 µs), so stroke expansion is not
  the cost; the curves are.

## What is not possible here

- **Retained scene fragments.** This vello_gpu revision has no way to record a
  subtree once and replay it: a `Scene` can be reset and rebuilt, nothing more.
  Caching static subtrees would mean rendering them to a texture and drawing
  that, which is what shader effects already do, at a memory cost and with
  care for partial redraw. It is only worth it for large static, expensive
  subtrees (many rounded, bordered, or text nodes) and needs its own design.

## Recommendations

1. Text: measure a glyph or run cache (the 11 µs per label). Biggest gain.
2. Rounded borders: a hand-built ring with fewer segments, or fewer curve
   segments per corner (the corner path is tessellated at tolerance 0.1),
   checked against the same pixel tests.
3. Static-subtree texture caching, only if a real app shows the need.
