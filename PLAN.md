# Plan: M3 Phase 2, Step 4 — `parley` Typography Spike (§14 step 4)

Corresponds to `BUILD_TRACKER.md` M3 Phase 2, step 4 of 4 (closes Phase 2).

## Goal

Per §14 step 4: "Wire `parley` for text -- don't stop at one static
label. Render a small sample of MD3's real type scale (at least two
type roles, e.g. Body and Headline, at their real weights/sizes) plus
one non-trivial string (mixed-direction or a non-Latin script...) to
get real signal on `parley`'s current line-breaking/BiDi/font-fallback
behavior before component work depends on it." Wire `NodeKind::Text`
into the real `Tree`/`build_tree_scene` pipeline step 3 built, using
`parley` for shaping and `vello_hybrid`'s low-level glyph API for
drawing.

## Scope

In scope:
- `engine-core::node`: `NodeKind::Text(TextState)`, `TextState { content,
  font_family, font_weight, font_size }` -- no `Animated` fields (no MD3
  component in scope yet animates a text property), no wrapping/overflow
  policy beyond `parley`'s own line-breaking against a fixed box width.
- `engine-render::text::TextRenderer`: owns `parley::FontContext` +
  `LayoutContext` across frames, with the three vendored fonts
  registered directly (system font discovery off, for a hermetic test).
- Vendored fonts under `crates/engine-render/assets/fonts/`: Roboto
  Regular + Medium (MD3's own default typeface, two real weights) and
  Noto Sans Arabic Regular (the non-Latin/RTL string).
- `build_tree_scene` extended to handle `NodeKind::Text` via
  `TextRenderer`; `FrameRenderer::resources_mut()` added since glyph
  atlasing happens during scene construction, not inside `render()`.
- A headless integration test (`tests/text_layout.rs`) proving each type
  role draws real ink in its own box, and the RTL string's ink sits on
  the right edge of its box, not the left.
- Extending the windowed demo with a live Body/Headline/Arabic text
  block below the step 3 animated-rects row.

Out of scope: `engine_md3`'s formal MD3 type-scale token table (this
step uses representative real values, not a designed token system);
text wrapping/overflow UX beyond `parley`'s own line-breaking; any
`Animated` text property (no component needs one yet).

## Verification

`cargo test --workspace` (all green, `frame_budget` still reports
`ignored`), `cargo clippy --workspace --all-targets` and `cargo fmt
--check` clean, and the real windowed demo showing the animated rects
plus a live, legible text block including the Arabic string.
