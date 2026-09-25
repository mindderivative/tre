# Migrating to 0.3.3

`tre` 0.3.3 renames properties so that one concept has one name across
the Python API (`Window.add_*`, `Node`) and declarative views. It's a
breaking release: code and view files written for 0.3.2 need the
changes below. Old names fail loudly — none is silently ignored.

## What changed

| Concept | 0.3.2 | 0.3.3 |
| --- | --- | --- |
| Text/glyph color | `add_text(background=)`, `add_icon(color=)`, `add_loading_indicator(color=)`; `style.background` on `Text`/`Link`/`Icon`/`LoadingIndicator`; `Node.animate("background")` on those | `foreground` in every one of those places. `background` now always means a real fill |
| Toolbar color style | `add_toolbar(color="standard" \| "vibrant")` | `add_toolbar(vibrant=False \| True)` |
| Switch/radio state | `add_switch(on=)`, `Node.set_on`/`get_on`; declarative `checked:` on `Switch`/`RadioButton` | `add_switch(selected=)`, `Node.set_selected`/`get_selected` (shared with `RadioButton`); declarative `selected:`. `Checkbox` keeps `checked` |
| Slider position | `"thumb_position"` property (`Node.animate`/`get`, `bindings:`, `two_way:`) | `"value"`, matching `add_slider(value=)` and declarative `value:` |
| Orientation | `add_divider(vertical=True)`, `add_scroll_view(horizontal=True)` | `orientation="vertical"` / `orientation="horizontal"` on both, as on `add_toolbar` |
| Enum values in views | `flex_direction: Horizontal`, `align_items: FlexStart`, `justify_content: SpaceBetween`, `fit: Cover` | lowercase `snake_case`: `horizontal`, `flex_start`, `space_between`, `cover` — the same values `Node.set_layout` and `add_image(fit=)` already used |
| Text-string parameters | `add_link(text=)`, `add_dialog(text=)`, `add_popover(text=)` | `add_link(content=)`, matching `add_text`; `add_dialog`/`add_popover(supporting_text=)`, matching `add_list_item` |
| Typography role in views | `text: {role: body_large}` | `text: {typography_role: body_large}`, matching `add_text(typography_role=)` |

`kind:` values (`Text`, `Rect`, `Switch`, …) are unchanged: they name
widget types, like class names, rather than being option values.

## Migrating view files

`tools/migrate_views_0_3_3.py` in the `tre` repository rewrites YAML
views, stylesheets, and themes in place, keeping comments and
formatting:

```bash
python tools/migrate_views_0_3_3.py --check views/   # report only
python tools/migrate_views_0_3_3.py views/           # rewrite
```

It works per widget, because two renames depend on the widget's kind:
`background` → `foreground` only on `Text`/`Link`/`Icon`/
`LoadingIndicator`, and `checked` → `selected` only on `Switch`/
`RadioButton`. It handles YAML (including YAML inside Python strings),
not `spec=` dicts or Python calls — update those by hand using the table
above. Review its diff: a widget whose `kind:` is written after a nested
`children:` list isn't recognized.

## What the old names do now

| You write | You get |
| --- | --- |
| A removed keyword, e.g. `add_switch(on=True)` | `TypeError` naming the unexpected keyword |
| `style.background` on a `Text`/`Link`/`Icon`/`LoadingIndicator` | `ValueError`: the kind has no fill; its color is `style.foreground` |
| `Node.animate("background", ...)` on one of those | `ValueError` pointing at `"foreground"` |
| `checked:` on a `Switch`/`RadioButton`, or `selected:` on a `Checkbox` | `ValueError` naming the right field |
| `"thumb_position"` | `ValueError`: renamed to `"value"` |
| A PascalCase enum value | `ValueError` listing the accepted lowercase values |
| `text: {role: ...}` | `ValueError` for the unknown field `role` |

## Behavior changes

- **Layout fields in stylesheets and themes now apply.** In 0.3.2 a
  stylesheet or theme rule setting `margin`, `flex_grow`, `flex_shrink`,
  `flex_basis`, `align_items`, or `justify_content` was silently
  dropped — only a widget's own inline `style:` reached the tree. 0.3.3
  applies them through the cascade like every other field, so a view
  relying on such a rule may lay out differently.
- **Check your text colors.** Because `background` used to be the text
  color of a label, code that passed a transparent or background-matching
  color there drew invisible text — `tre`'s own examples had nine such
  labels. Migrating makes the intent explicit; a `foreground` of
  `(0, 0, 0, 0)` is still invisible.
- **Live updates keep bound values** ([issue #8](https://github.com/mindderivative/tre/issues/8)).
  `set_theme`, `reconcile`, and `poll_reload` now re-apply the attached
  `ViewModel` afterward. In 0.3.2 a bound field fell back to its static
  spec value until its `Signal` next changed, and bindings or handlers a
  reload added needed another `_attach`. One consequence: re-applying a
  `checked`/`text` binding fires `Change`, so a declared `on_change`
  handler runs once per update. New: `View.set_stylesheet` replaces a
  view's stylesheet in place.
- **An `Icon`'s color isn't animated.** `Node.animate("foreground", ...)`
  on an `Icon` sets the color immediately rather than easing it; on
  `Text`/`Link`/`LoadingIndicator` it animates normally.
