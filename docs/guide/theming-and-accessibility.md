# Theming & Accessibility

## Dynamic color theming

```python
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
```

Builds a full MD3 `DynamicTheme` (both light and dark color schemes) from
one seed color via real HCT/tonal-palette resolution, and makes it this
window's active theme. `dark` picks which of the two schemes is active
right now — flip it again at runtime (e.g. in response to a real OS
theme-change event) to live-switch every themed node at once.

`set_theme` immediately re-resolves and pushes the real "on-surface"
color into:

- every node that already called `enable_interaction()` (its ripple/hover
  tint)
- every `Checkbox`/`Slider`/`TextField` created **after** this call (their
  default checkmark/track/text color) — a component created before
  `set_theme` keeps its original hardcoded default until you call
  `set_theme` again, or opts in individually via `enable_interaction()`.

A `Window` that never calls `set_theme` sees zero behavior change from
every component's own plain historical default (black interaction tint,
white checkmark, gray track, dark text).

## Shape & elevation tokens

Beyond color, a theme can override a component's own corner radius and
elevation by name, and both accept the same named-token vocabulary
declarative YAML `style:` blocks do (see
[Declarative Views](declarative-views.md)):

```yaml
# theme.yaml
components:
  button: {corner_radius: small}
  button.filled: {corner_radius: medium, elevation: level_1}  # variant-specific beats bare
  card: {elevation: level_2}
```

Pass `custom_theme=` (a path to this file) to `Window.set_theme(...)` to
load it. Lookup is per-field and two-tier: a `"<component>.<variant>"`
key is checked first, then the bare `"<component>"` key, independently
for `corner_radius` and `elevation` — a variant entry that only sets one
of the two doesn't block the bare key's own value for the other. A
component with no override anywhere falls back to its own built-in
formula default (usually `height / 2.0`, MD3's own real "fully rounded"
shape). The real named tokens, resolved against `engine_md3::shape`:

| Kind | Names |
|---|---|
| `corner_radius` | `none`, `extra_small`, `small`, `medium`, `large`, `extra_large` |
| `elevation` | `level_0` through `level_5` |

## Typography theming

A real, published MD3 type scale — 15 roles (`display_large/medium/
small`, `headline_large/medium/small`, `title_large/medium/small`,
`body_large/medium/small`, `label_large/medium/small`), each a real
`family`/`weight`/`size`/`line_height` — lives in `engine_md3::
typography`, sourced directly from Flutter's own published MD3 type
scale rather than approximated. Reference a role by name instead of
literal font values:

```python
heading = window.add_text(
    "Settings", background=(0, 0, 0, 0), width=300, height=32,
    typography_role="headline_small",
)
```

```yaml
# a declarative view.yaml
- id: heading
  kind: Text
  text: {content: "Settings", role: headline_small}
```

Any of `font_family`/`font_weight`/`font_size`/`line_height` given
*alongside* `role`/`typography_role` overrides just that one field on
top of the role's own resolved default — the same per-field-override
shape a theme's own `typography:` section (below) uses. An unrecognized
role name is a real, clear error, never a silent fallback.

A theme can also override individual fields of a role globally, the
same `components:` shape shape/elevation already use:

```yaml
# theme.yaml
typography:
  body_large: {font_family: Inter}
  headline_small: {font_size: 26, line_height: 1.4}
```

## Keyboard focus & Tab order

A node becomes part of the Tab order once it's given real interactive
meaning:

- `set_on_click` adds a `Click` accessibility action and makes the node
  focusable.
- `add_slider`/`add_text_field` opt into `Role::Slider`/`Role::TextInput`
  + a `Focus` action automatically at construction — they're
  Tab-reachable from the moment they're created, with no extra call
  needed.

Drive focus and keyboard interaction directly, without a live window:

```python
window.press_key("tab")            # move focus forward
window.press_key("tab", shift=True)  # move focus backward
window.press_key("enter")          # activate the focused node
node.is_focused()                  # True if this node currently has focus
```

Accepted `press_key` values: `"tab"`, `"enter"`, `"space"`, `"escape"`,
`"backspace"`, `"delete"`, `"left"`, `"right"`, `"home"`, `"end"`.

## Screen readers

A real [AccessKit](https://github.com/AccessKit/accesskit) tree is built
fresh from the node tree every frame — the same structure a screen reader
walks and drives actions through: a screen-reader-triggered `Click` or
`Focus` action routes through the identical input pipeline a real mouse
click or Tab press would use, not a separate code path.

## Clipboard

```python
selected = window.copy()   # returns the focused TextField's selected text, or None
cut_text = window.cut()    # also edits the field and fires Change
window.paste("some text")  # inserts at the cursor, same as typing
```

`copy`/`cut`/`paste` are deliberately **hermetic** in this synthetic form
— they never touch the real OS clipboard, only the pure text-selection
state, which keeps headless tests deterministic. The real OS-clipboard
path is driven automatically by `App.run()`'s own live keyboard handling
(a real Ctrl+C/X/V while a `TextField` is focused) and isn't reachable
synthetically without a live window.
