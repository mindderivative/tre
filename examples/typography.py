#!/usr/bin/env python3
"""M62 (§7.1, §16.3): the real, end-to-end proof of MD3 typography
theming -- before this milestone, `TextSpec`/`add_text` only ever took
plain literal `font_family`/`font_weight`/`font_size` numbers; there was
no real, centralized MD3 type scale anywhere in this codebase (dozens of
scattered per-factory constants in `window_factory.rs` were the closest
thing, confirmed via grep before this milestone started), and no way to
reference a role like "this is a headline" by name at all.

`typography_view.yaml` shows the declarative surface:
- `heading`: `typography_role: headline_small` -- one of the 15 real MD3 type-scale
  roles (`engine_md3::typography`), resolved to its own real family/
  weight/size/line-height with zero literal numbers written by the
  author.
- `body` / `loose_body`: the identical real content and `typography_role:
  body_large`, but `loose_body` also sets `line_height: 3.0` -- a real,
  visible difference in how far apart the wrapped lines sit, proving
  `line_height` is real capability (M62 Phase 1), not inert stored data
  -- see `crates/engine-render/src/text.rs`'s own `a_larger_line_height_
  genuinely_widens_the_real_per_line_advance` for the exact geometry
  proof, at the Rust layer.
- `overridden_label`: `typography_role: label_large` with `font_size: 20` also
  given -- proves a literal field overrides just that one field on top
  of the role's own real default, the rest (weight, in this case) still
  coming from `label_large` itself.

`window.add_text(typography_role=...)` below is the identical real
capability's imperative counterpart -- `add_text`'s own real parity with
`kind: Text`'s `text.role`, both resolving through the same
`engine_md3::type_style_named` lookup.

Honest, stated limitation this example inherits from every other
color/style-related example in this suite (`theme_customization.py`'s
own docstring states the identical real limit for color): `Node.get`
has no Python-facing getter for a text node's own resolved `font_size`/
`font_weight`/`line_height` (confirmed via grep), so this script can only
prove the real FFI/YAML-parse calls succeed and render in a real window
over real frames -- the exact numeric-resolution claims (a role really
does resolve to its own real constants; a literal field really does
override just one) are proven at the Rust layer instead (`crates/
engine-spec/src/build.rs`'s own `text_role_resolves_every_field_to_the_
real_named_type_style`/`a_literal_field_overrides_just_that_one_field_
on_top_of_the_role`).

**Explicitly out of scope, named not silent:** `ThemeSpec.typography`
(§16.3, `theme.rs`) parses a real per-role override document already,
but nothing resolves it yet -- migrating `window_factory.rs`'s own
~40 already-shipped, already-correct per-factory font constants to
reference `engine_md3::typography` (and consult a theme's own
`typography:` override) instead of their own independent literals is
real, separate, future follow-up, the identical "data only, this
milestone" precedent `engine_md3::shape`'s own M49 doc comment already
established for corner-radius/elevation.
"""

from pathlib import Path

from tre import App, View, Window

directory = Path(__file__).parent

view = View(str(directory / "typography_view.yaml"))
window = Window.from_view(view, width=360, height=340, title="Typography")

# Imperative parity: add_text(typography_role=...) resolves through the
# identical engine_md3::type_style_named lookup the declarative text:
# {typography_role: ...} block above does.
label = window.add_text(
    content="Imperative label_large",
    foreground=(0x1C, 0x1B, 0x1F, 0xFF),
    width=328,
    height=24,
    typography_role="label_large",
    x=16,
    y=300,
)

app = App()
app.add_window(window)
app.run(max_frames=60)
print("typography.py: exited cleanly after a real 60-frame render loop")
