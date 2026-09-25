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

4. Phase 2 done:
   - Paint names on every node, with exact readback.
   - Per-corner radii and shadows, both animatable.
   - Group opacity, and color alpha that multiplies instead of being
     replaced. The legacy scrims' 32% moved into their color alpha.
   - Bezier easing, `get_target`, and `stop_animation`.
   - Text-input placeholder, caret and selection colors, plus
     `obscured`, which refuses copy and cut.
   - Scrollbar fill and width.
   - A live terminal palette: cells keep the color the program asked
     for, resolved at paint time.
   - pytest 1057 passed; cargo release 565 passed; 89 examples and the
     showcase clean.

## Status

**M95 Phase 3 in progress** (pixel tests, proof ripple, docs).
