# LOG — Branch `0.3.4`: Milestone 95

- User-directed: "Push, then start M95."

## Done

1. `0.3.4` pushed to origin (M93 and M94).
2. M95 scoped against the source.
   - The animation core already retargets from the current value, and
     it never fires a replaced animation's completion. Easing already
     defaults to linear.
   - `ShapeKey` morphs one closed contour's vertices only.
   - `kurbo` parses SVG `d` data.
   - Opacity is per node. The legacy dialog and sheet scrims are the
     parents of their panels, at 32% opacity.
   - About 99 sites read corner radii.
   - Placeholder text and password obscuring don't exist yet.

3. Phase 1 done:
   - `engine-core` gained a `path` module: SVG data, view-box fitting,
     arc-length trim, and a resampling morph.
   - `NodeKind::Path` is ticked and painted.
   - `window.create("box" | "path", **props)`.
   - `set`/`get` gained `width`/`height` (a number, `"auto"`, or a
     percentage) and the path properties; `animate` gained
     `data`/`trim_*`.
   - Finding: `with_opacity` replaces each color's alpha instead of
     multiplying it. Fixed in Phase 2.
   - pytest 1036 passed; cargo release 564 passed.

## Status

**M95 Phase 2 in progress.**
