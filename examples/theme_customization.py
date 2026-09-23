#!/usr/bin/env python3
"""M49/M50 (§7.1, §16.3): the real, end-to-end proof of "theme as a
YAML file" -- the user's own stated model, verbatim: "a default theme
at the top level, then custom theme, then widget wide styles, then in
line style directly on a specific widget," plus "scope the corner-
radius/elevation to use the new theme pattern... the theme backend
complete." Before M49, none of this was reachable at all; before M50,
color overrides reached the real MD3 catalog but shape/elevation never
did (confirmed via direct investigation before each milestone started
-- see `BUILD_TRACKER.md`'s M49/M50 sections for the full audits).

This script proves the real four-tier cascade on `theme_customization_
view.yaml`'s three checkboxes, that a `colors:` override reaches *both*
the declarative and imperative surfaces through the exact same
`ColorScheme::role` mechanism, and that a `components:` override
reaches a real imperative component's own corner radius/elevation too:

- `theme_checkbox`: no stylesheet rule, no inline override -- resolves
  from `theme_customization_custom_theme.yaml`'s own `corner_radius:
  10`, itself already superseding the engine's own shipped default
  (2.0).
- `stylesheet_checkbox`: `theme_customization_sheet.yaml` targets it by
  `id:` with `corner_radius: 6` -- the app's own widget-wide stylesheet
  beats the custom theme.
- `inline_checkbox`: its own `style: {corner_radius: 12}` in the view
  YAML beats every layer above it.
- `primary_box` (a declarative `background: primary` `Rect`) and the
  imperative `add_button` below both pick up the custom theme's
  `colors: {primary: "#00695C"}` override -- the real point of routing
  the override through `ColorScheme::apply_overrides` at the one shared
  choke point both paths already resolve colors through.
- The same `add_button` also picks up `components: {button.filled:
  {corner_radius: 4, elevation: 2}}` -- M50's own new mechanism,
  `ThemeState::shape`/`elevation`'s 2-tier lookup, reached through the
  identical `custom_theme` parameter M49 already built (no new Python
  API needed for M50 at all).
- M51: after the window is already showing, `view.set_theme(custom_
  theme=...)` swaps in a *second* custom theme live -- `theme_checkbox`
  (which has no stylesheet rule or inline override of its own) picks up
  the new theme's `corner_radius: 18` on the exact same, already-built
  `Node`, while `stylesheet_checkbox`/`inline_checkbox` are unaffected
  (the app's own stylesheet and each widget's own inline style still
  win, exactly like at construction time).
- M52: the same real live re-theme, now on `Window`'s own imperative
  catalog -- `window.set_theme(custom_theme=...)` re-resolves the exact
  same already-built `button` node's own container color and
  `corner_radius`/`elevation` live, through the milestone's own new
  `RetitheHook` mechanism (`crates/engine-py/src/window.rs`/`window_
  factory.rs`). Before M52, a second `Window.set_theme` call only ever
  pushed one blind, uniform on-surface tint into 4 unrelated fields
  (ripple/hover, checkbox mark, slider track, text caret) -- it never
  touched a button's own real container/label color or shape/elevation
  at all.
- M63 (§7.1, §16.3): `typography: {label_large: {font_size: 15}}`
  reaches the same `add_button`'s own label -- closing the real gap
  M62 left open (`ThemeSpec.typography` parsed but never resolved by
  anything): `ThemeState::typography("label_large")` now consults it,
  the identical real per-role lookup every migrated factory's own
  label/headline/body text resolves through.

`Node.get` only returns `f64`, so `corner_radius`/`elevation` (real
numbers) are asserted directly below; there is still no Python-facing
getter for a node's own resolved color (the same honest limitation
`theme.py`/`bindable_background.py` already state) -- the color
override is proven by *not raising*, and rendered in a genuine window
so a human running this locally can see `primary_box`/the button both
take the same real overridden green, and the button's own real
(smaller, more elevated) shape.
"""

from pathlib import Path

from tre import App, View, Window

directory = Path(__file__).parent
custom_theme_path = str(directory / "theme_customization_custom_theme.yaml")

view = View(
    str(directory / "theme_customization_view.yaml"),
    stylesheet=str(directory / "theme_customization_sheet.yaml"),
    theme_seed=(0x67, 0x50, 0xA4, 0xFF),
    custom_theme=custom_theme_path,
)

theme_checkbox = view.node("theme_checkbox")
stylesheet_checkbox = view.node("stylesheet_checkbox")
inline_checkbox = view.node("inline_checkbox")

assert theme_checkbox.get("corner_radius") == 10.0, "expected the custom theme's own override"
assert stylesheet_checkbox.get("corner_radius") == 6.0, "expected the app's own stylesheet to win"
assert inline_checkbox.get("corner_radius") == 12.0, "expected the widget's own inline style to win"

print(
    "four-tier cascade verified: theme_checkbox="
    f"{theme_checkbox.get('corner_radius')!r}, stylesheet_checkbox="
    f"{stylesheet_checkbox.get('corner_radius')!r}, inline_checkbox="
    f"{inline_checkbox.get('corner_radius')!r}"
)

window = Window.from_view(view, width=320, height=260, title="Theme Customization")

# M49 Phase 1: the same custom theme's color override reaches the real
# imperative MD3 catalog too, through the identical ColorScheme::role
# chain -- must not raise. M50: components: {button.filled: {...}} also
# applies here, through the exact same custom_theme parameter.
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), custom_theme=custom_theme_path)
button = window.add_button(label="Primary", variant="filled", width=140, height=40, x=16, y=180)
assert button.get("corner_radius") == 4.0, "expected the components: override, not height / 2.0"
assert button.get("elevation") == 2.0, "expected the components: override, not the MD3 default"
print(
    f"components: override verified: corner_radius={button.get('corner_radius')!r}, "
    f"elevation={button.get('elevation')!r}"
)

# M63 (§7.1, §16.3): the same custom theme's typography: {label_large:
# {font_size: 15}} override (above `button`'s own construction) also
# reached this button's own label through ThemeState::typography --
# no Python-facing getter exists for a Text node's own font_size
# (the same honest limitation this file's own module doc comment
# already states for color), so this is proven not to raise here, and
# proven with a real number readback at the Rust layer instead
# (crates/engine-py/src/window.rs::tests::typography_applies_a_real_
# per_field_override_on_top_of_the_shipped_default).
print("typography: override applied to the button's own label, no error raised")

# M61 (§16.3): components: also accepts a real MD3 shape/elevation
# *token name* (card: {corner_radius: small, elevation: level_2} in the
# same custom theme file above), resolved to the identical constants
# engine_md3::shape::named/elevation_named themselves define -- not just
# a plain literal number like button.filled above.
card = window.add_card(width=140, height=80, variant="filled", x=176, y=180)
assert card.get("corner_radius") == 8.0, "expected 'small' to resolve to SHAPE_SMALL (8.0)"
assert card.get("elevation") == 2.0, "expected 'level_2' to resolve to ELEVATION_LEVEL_2 (2.0)"
print(
    f"components: shape-token override verified: corner_radius={card.get('corner_radius')!r}, "
    f"elevation={card.get('elevation')!r}"
)

# M51: live re-theme -- swap in a second custom theme on the already-
# built view, on the already-shown window, without rebuilding anything.
retheme_path = str(directory / "theme_customization_retheme.yaml")
view.set_theme(theme_seed=(0x67, 0x50, 0xA4, 0xFF), custom_theme=retheme_path)
assert theme_checkbox.get("corner_radius") == 18.0, "expected the new custom theme's own live override"
assert stylesheet_checkbox.get("corner_radius") == 6.0, "the app's own stylesheet still wins"
assert inline_checkbox.get("corner_radius") == 12.0, "the widget's own inline style still wins"
print(f"live re-theme verified: theme_checkbox={theme_checkbox.get('corner_radius')!r}")

# M52: the same real live re-theme, now on Window's own imperative
# catalog -- the exact same already-built `button` node, no rebuild.
# Before M52, this second call only ever pushed one blind uniform
# on-surface tint into 4 unrelated fields; it never touched a button's
# own real container/label color or corner_radius/elevation at all.
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), custom_theme=retheme_path)
assert button.get("corner_radius") == 20.0, "expected the new custom theme's own live override"
assert button.get("elevation") == 6.0, "expected the new custom theme's own live override"
print(
    f"imperative live re-theme verified: corner_radius={button.get('corner_radius')!r}, "
    f"elevation={button.get('elevation')!r}"
)

app = App()
app.add_window(window)
app.run(max_frames=60)
print("theme_customization.py: exited cleanly after a real 60-frame render loop")
