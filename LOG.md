# Log: M25 Phase 2 — Real `PaintProperties.opacity` Compounding Across Every Remaining Paint Site (§5, §6), closing M25

Corresponds to `BUILD_TRACKER.md` M25 Phase 2: six real paint sites in
`engine-render::paint_node` were silently ignoring `node.paint.
opacity.current`, or applying only their own local alpha without
compounding it with the node's own real opacity.

## Investigation (confirmed via direct read of every `scene.set_paint`
call site in `paint_node`)

1. `NodeKind::Canvas`'s own `DrawCommand::FillRect`/`FillCircle`/
   `StrokePath` — painted `*color` raw, zero opacity handling.
2. `NodeKind::Image`'s `draw_texture_rects` call — no opacity
   parameter used at all.
3. `NodeKind::Slider`'s own track fill — painted `state.track_tint`
   raw; only the thumb multiplied by `node.paint.opacity.current`, a
   real internal inconsistency within one `NodeKind`.
4. `NodeKind::Checkbox`'s own checkmark stroke — multiplied only by
   `check_progress`, never compounded with `node.paint.opacity.
   current`.
5. Elevation/shadow painting (`shadow_color(0.15)`/`shadow_color(0.3)`,
   the fixed MD3 ambient/key alphas) — never multiplied by the node's
   own opacity at all.
6. The ripple/hover interaction overlay (`hover_opacity`/each
   ripple's own `opacity`) — never compounded with `node.paint.
   opacity.current` either.

Every real, already-correct site (`Rect`/`Splitter`, `Text`,
`TextField`, `Checkbox`'s own box, `Slider`'s own thumb, `Icon`)
already used `with_opacity(color, node.paint.opacity.current)`. This
phase extends the identical pattern to the six sites above.

## What happened

`Canvas`/`Slider` track: each real color now wrapped in `with_opacity
(*color, node.paint.opacity.current)`. `Checkbox`'s checkmark: now
`with_opacity(state.mark_tint, state.check_progress.current * node.
paint.opacity.current)` — two independent real alpha sources
multiplied together, the correct way two "how visible" factors
compound. Elevation/shadow: both `shadow_color` calls multiplied by
`node.paint.opacity.current`, and the whole shadow block now skips
entirely at `opacity <= 0.0` too (mirroring the existing `elevation <=
0.0` skip). Ripple/hover: `hover_opacity.current` and each `ripple.
opacity.current` each multiplied by `node.paint.opacity.current`
before use.

**Real, necessary design decision for `Image`:** `Scene::
draw_texture_rects` has no opacity parameter of its own at all
(confirmed via direct source read) — unlike every `set_paint`-based
fill, opacity can't be baked into a color argument. Wrapped the call
in `scene.push_layer(None, None, Some(opacity), None, None)`/
`pop_layer()` instead — the identical real "opacity-only layer, no
clip" mechanism the ripple/hover overlay already relies on. Skipped
entirely at `opacity <= 0.0`, avoiding an unnecessary GPU draw.

Six new pixel tests, one per site, each proving real compounding, not
merely "doesn't crash": `canvas_paint.rs`/`image_paint.rs`/`slider_
paint.rs` each use the same real range-check pattern `animated_
rect.rs`'s own mid-flight test already established (a blended channel
strictly between the fully-opaque and fully-transparent extremes, not
an exact predicted byte value, since exact blend math depends on
`vello_hybrid`'s own internals); `checkbox_paint.rs` proves the
checkmark paints genuinely different pixels at opacity 1.0 vs. 0.5;
`elevation_shadow.rs` proves a fully faded elevated node (`opacity:
0.0`) now casts no shadow at all; `ripple_hover_dispatch.rs` proves a
fully faded node's own ripple is now completely invisible (its origin
pixel matches an untouched point, not just "the base fill is also
invisible").

Every pre-existing test passed unmodified — every node's own real
default `opacity = 1.0` makes `with_opacity(color, 1.0)`/`x * 1.0` a
true no-op, so nothing visually changed for any node that never
touches `opacity` at all.

Full `cargo test --workspace --release` (new `engine-render` tests:
Canvas/Image/Slider/Checkbox/Shadow/Ripple, six total), `cargo clippy
--workspace --all-targets -- -D warnings`, `cargo fmt --check` all
clean. `maturin develop --release` + `pytest tests/` (187 passed,
unchanged, 1 pre-existing skip) and all thirty-two examples run with
the real display (the corrected convention from Phase 1) — all pass.

M25 — Known Gap Resolution is now fully complete.
