# Plan: M25 Phase 2 — Real `PaintProperties.opacity` Compounding Across Every Remaining Paint Site (§5, §6), closing M25

Corresponds to `BUILD_TRACKER.md` M25 Phase 2: six real paint sites in
`engine-render::paint_node` currently ignore `node.paint.opacity.
current` entirely, or apply only their own local alpha without
compounding it with the node's own real opacity.

## Investigation (already done, confirmed via direct read)

Every `scene.set_paint(...)` call site in `paint_node` audited
directly:

1. `NodeKind::Canvas`'s own `DrawCommand::FillRect`/`FillCircle`/
   `StrokePath` — paints `*color` raw, zero opacity handling.
2. `NodeKind::Image`'s `draw_texture_rects` call — no opacity
   parameter used at all (`SampleRect`/`ImageQuality` carry none).
3. `NodeKind::Slider`'s own track fill — paints `state.track_tint`
   raw; only the thumb (a separate fill) multiplies by `node.paint.
   opacity.current`, a real internal inconsistency within one kind.
4. `NodeKind::Checkbox`'s own checkmark stroke — multiplies only by
   `state.check_progress.current`, never compounded with `node.paint.
   opacity.current`, so a checked checkbox mid-fade-out would show its
   checkmark at full alpha while its own box correctly fades.
5. Elevation/shadow painting (`shadow_color(0.15)`/`shadow_color(0.3)`,
   the real fixed MD3 ambient/key alphas) — never multiplied by the
   node's own opacity at all.
6. The ripple/hover interaction overlay (`hover_opacity`/each ripple's
   own `opacity`) — never compounded with `node.paint.opacity.current`
   either.

Every real, already-correct site (`Rect`/`Splitter`, `Text`,
`TextField`, `Checkbox`'s own box, `Slider`'s own thumb, `Icon`) uses
the identical `with_opacity(color, node.paint.opacity.current)`
pattern already — this phase extends that same pattern to the six
sites above, not a new mechanism.

## What will change

- `crates/engine-render/src/lib.rs`:
  - `Canvas` arm: each `DrawCommand` fill/stroke's own `*color`
    becomes `with_opacity(*color, node.paint.opacity.current)`.
  - `Image` arm: `Scene::draw_texture_rects` has no opacity parameter
    (confirmed via direct source read of `vello_hybrid` 0.2.0) — apply
    opacity the same way the `Icon` arm's own transform restoration
    already establishes a "wrap in a temporary scene-state change"
    precedent, here via `scene.push_layer(None, None, Some(opacity),
    None, None)` / `pop_layer()` around the `draw_texture_rects` call
    (the identical real opacity-only-layer mechanism ripple/hover
    already uses) — skip the call entirely at `opacity <= 0.0` to
    avoid an unnecessary GPU draw, matching the elevation section's
    own existing `if elevation > 0.0` early-skip precedent.
  - `Slider` arm: track fill becomes `with_opacity(state.track_tint,
    node.paint.opacity.current)`.
  - `Checkbox` arm: checkmark stroke becomes `with_opacity(state.
    mark_tint, state.check_progress.current * node.paint.opacity.
    current)` — compounds both real alpha sources multiplicatively,
    the correct way two independent "how visible" factors combine.
  - Elevation/shadow section: both `shadow_color(0.15)`/`shadow_color
    (0.3)` calls become `shadow_color(0.15 * node.paint.opacity.
    current)`/`shadow_color(0.3 * node.paint.opacity.current)`.
  - Ripple/hover section: `hover_opacity.current` and each `ripple.
    opacity.current` each multiplied by `node.paint.opacity.current`
    before use.
- New/updated `engine-render` pixel tests proving each of the six
  sites genuinely fades with `node.paint.opacity`, not just "doesn't
  crash": a `Canvas` node at `opacity: 0.5` paints its own real
  half-alpha-blended `FillRect`; an `Image` node at `opacity: 0.5`
  blends with its own real background; a `Slider` at `opacity: 0.5`
  shows both track and thumb blended; a checked `Checkbox` at
  `opacity: 0.5` shows its checkmark at real half alpha (not full);
  elevation shadow at `opacity: 0.0` paints no shadow at all
  (currently it would still paint one); ripple/hover overlay at
  `opacity: 0.0` shows nothing (currently the ripple would still
  paint at full ripple-opacity).

## Testing

- `cargo test --workspace --release`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --check`
- `maturin develop --release`
- `pytest tests/ -v`
- Run every example script (real display, no env stripping per the
  corrected convention from Phase 1).
