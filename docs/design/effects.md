# Blur, backdrop filters, blend modes, and masks

0.5.4 research (#110). What the pinned `vello_gpu` can do as a layer effect,
what was built on it, and what could not be. Probed directly with
`engine-render/tests/vello_effects.rs` (`cargo test -p engine-render --release
--test vello_effects -- --ignored --nocapture`), without any tre node involved.

## What the renderer supports

| Effect | Result at the pinned commit |
| --- | --- |
| Blur layer (`push_filter_layer`, `FilterFunction::Blur`) | Works. A Gaussian blur of the layer's content; the filter reads and writes in the layer's own transform, so it scales with the display |
| Colour filters (`Saturate`, `Brightness`, `Contrast`, `Grayscale`, `HueRotate`, `Invert`, `Sepia`, `Opacity`) | **Panic**: `Filter::from_function` is `unimplemented!` for all but `Blur`. The primitives behind them (colour matrix) are not on the GPU path either |
| Drop shadow, offset, flood (filter primitives) | Work |
| Blend layers (`push_blend_layer`) | Work for every mixing function with `SrcOver` (multiply, screen, overlay, difference and the rest checked against the formulas). Destructive composition modes (`Clear`, `Copy`, `SrcIn`, `DestIn`, `SrcOut`, `DestAtop`) are refused by the renderer |
| Clip layers (rounded path) | Work (already used) |
| Mask layers (`push_mask_layer`) | **Panic**: "mask layers are currently not supported" |
| Backdrop input to a filter (`FilterSource::BackgroundImage`) | Not usable: a filter layer only sees its own content, never what is behind it |

## What was built

- **`blur`**: the node and its subtree become a filter layer (alongside
  opacity and blend, in one layer). The blur reaches three deviations past the
  box, and the damage tracker counts that margin as part of what the node paints.
- **`blend_mode`**: the same layer takes a blend mode. A blended node's pixels
  depend on what is behind it, which partial redraw already handles: the node is
  redrawn whenever a damage rect touches it.
- **`backdrop_blur`** (frosted glass): since a filter cannot read the backdrop,
  the content behind is drawn again. The painter walks the tree a second time,
  limited to the node's box plus the blur's reach (so cost follows the box) and
  stopping at the node, inside a clip layer of the node's rounded box wrapping a
  blur layer. The node's own paint then goes on top. Partial redraw stays exact
  because a backdrop node's damage fingerprint includes the fingerprints of
  everything painted before it that touches its reach (a test compares a partial
  redraw with a full one).

## What was not built, and why

- **Masks.** The renderer panics on them. A rounded clip (`corner_radius` with
  `clip_children`) covers shaped clipping; a luminance or alpha mask can be done
  in a WGSL effect shader, which can sample its node's content.
- **Colour filters.** Unimplemented upstream. A WGSL effect shader does the same
  work today (it reads the node's rendered content and returns a colour), so
  saturate, brightness and the like need no engine support; a ready-made
  preset could be added on top of effect shaders later.
- **`BackgroundImage`-style backdrop filters beyond blur**: the same
  re-draw technique would carry other filters, but only blur is implemented in
  the renderer, so only blur is offered.

## Costs

A blur layer costs a texture pass over the node's box plus margin. A backdrop
blur costs that, plus drawing the content behind the box again; with several
frosted panels over a busy scene, expect the frame to cost accordingly. Both
are paid only by nodes that use them.
