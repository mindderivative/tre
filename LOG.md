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

## Status

**M95 Phase 1 in progress.**
