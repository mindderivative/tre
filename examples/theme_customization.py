#!/usr/bin/env python3
"""M49 (§7.1, §16.3): the real, end-to-end proof of this milestone's own
"theme as a YAML file" work -- the user's own stated model, verbatim:
"a default theme at the top level, then custom theme, then widget wide
styles, then in line style directly on a specific widget." Before this
milestone, none of this was reachable at all (confirmed via direct
investigation before it started -- see `BUILD_TRACKER.md`'s M49
section for the full audit): `DynamicTheme::from_seed` was the only
constructor, no per-widget-kind default styles existed anywhere, and
there was no way to override even a single MD3 color role.

This script proves the real four-tier cascade on `theme_customization_
view.yaml`'s three checkboxes, and that a `colors:` override reaches
*both* the declarative and imperative surfaces through the exact same
`ColorScheme::role` mechanism, not two separate paths:

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

`Node.get` only returns `f64`, so `corner_radius` (a real number) is
asserted directly below; there is still no Python-facing getter for a
node's own resolved color (the same honest limitation `theme.py`/
`bindable_background.py` already state) -- the color override is proven
by *not raising* through both real construction paths, and rendered in
a genuine window so a human running this locally can see `primary_box`/
the button both take the same real overridden green.
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
# chain -- must not raise.
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), custom_theme=custom_theme_path)
window.add_button(label="Primary", variant="filled", width=140, height=40, x=16, y=180)

app = App()
app.add_window(window)
app.run(max_frames=60)
print("theme_customization.py: exited cleanly after a real 60-frame render loop")
