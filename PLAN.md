# PLAN — Branch `0.3.4`: Milestone 95, Paint and Animation Building Blocks

*(Replaces the M94 plan — M94 is complete and pushed. Every step is in
`BUILD_TRACKER.md`.)*

## Goal

Generic replacements for the MD3-specific visuals:
- vector paths with trim and morph;
- the target paint names;
- per-corner radii;
- shadows;
- group opacity;
- bezier easing;
- a theme-free text input, scrollbar, and terminal.

Additive: legacy names and behavior stay until 0.3.5.

## Design (from the source)

- **`path` kind.** `NodeKind::Path(PathState)` holds:
  - `data: Animated<PathData>`, a `BezPath` parsed from SVG `d` by
    `kurbo`;
  - `view_box: Option<Rect>`, fitted uniformly and centered into the
    layout box;
  - `trim_start`/`trim_end: Animated<f64>`.

  Fill comes from `paint.background` (the target `fill`). The stroke
  comes from `border_color`/`border_width`, centered on the path as in
  SVG, and trim applies to the stroke. The path is transformed before
  stroking, so `stroke_width` stays in pixels.
- **Morph.** `PathData`'s `Interpolate` resamples each subpath by arc
  length to a shared count and lerps. A closed subpath is aligned by
  the starting offset with the least travel. A different subpath count,
  or open against closed, switches at `t = 0.5`. `t = 1` gives the exact
  target path.
- **`window.create`.** `window.create(kind, **props)` builds a detached
  `box` (`Rect`) or `path` and applies `set`. `width`/`height` join
  `set`.
- **Paint names on every node.**
  - `fill` maps to `background` (the icon tint on an icon).
  - `stroke_color`/`stroke_width` map to `border_*`.
  - `corner_radius` takes a number, or a 4-tuple through
    `corner_radii_override`, which becomes `Option<Animated<[f64; 4]>>`.
  - `shadows` is `Animated<Shadows>`, padded pairwise with transparent
    shadows when lengths differ.
  - `opacity` becomes group opacity: `push_layer(opacity)` around the
    node and its subtree, with the node's own paints at full alpha.
    Legacy scrims carry their 32% as color alpha instead.
- **Easing.** `MotionCurve::Bezier(x1, y1, x2, y2)`. `animate` gains
  `easing`, which defaults to linear as today.
- **Semantics.** `stop_animation(name)` and `get_target(name)`.
- **Text input.**
  - `placeholder` text drawn in `placeholder_fill` when the input is
    empty;
  - `caret_color` and `selection_fill`;
  - `obscured` draws bullets, and copy/cut refuse.
- **Other colors.** `ScrollView` gets `scrollbar_fill`/`scrollbar_width`.
  The terminal gets a `palette` of 16 ANSI colors plus foreground,
  background, cursor and selection.

## Phases

1. Vector paths.
2. Paint, shadows, and easing.
3. Pixel tests, a proof ripple, stubs, docs, and the full chain.

## Status

Phase 1 in progress.
