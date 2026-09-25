# Declarative Views

The declarative path describes a UI as data — a tree of widget specs —
instead of imperative Python calls, with data binding against a plain
Python `ViewModel`. It's the same underlying node tree either way: a
`View`'s nodes support the same `Node` methods (`animate`, `get`, etc.)
as a `Window`'s.

!!! note "Using `tre` with view files on disk?"
    This page builds views from data (`spec=` dicts), the form a
    framework hands `tre`. Loading `view.yaml` files, `include:`, image
    and theme paths, and file-watching hot reload are all covered in
    [Working with Files](working-with-files.md).

## Building a view

```python
from tre import View

view = View(spec={
    "id": "root",
    "kind": "Container",
    "style": {"flex_direction": "horizontal", "padding": 12, "gap": 8},
    "children": [
        {"id": "swatch", "kind": "Rect",
         "style": {"width": 40, "height": 40, "background": "#6750A4"}},
        {"id": "label", "kind": "Text",
         "text": {"content": "Hello", "typography_role": "body_large"},
         "style": {"foreground": "#1D1B20"}},
    ],
})
node = view.node("label")  # look up a widget by its author-assigned id
```

`spec=` takes a plain Python object — nested `dict`s and `list`s
shaped like [the view schema](#the-view-schema) — and builds it directly
into the tree, with no text parsing. However your application produced
that data (generated it, loaded it from YAML/TOML/JSON, expanded its own
macro syntax), `tre` only sees the finished structure.

`json=` takes the same structure as JSON text:

```python
view = View(json='{"id": "root", "kind": "Container", "style": {"width": 10, "height": 10}}')
```

At most one of `spec=`/`json=`/`source=` may be given, and `View()` with
no content at all raises `ValueError`. (`source=`, YAML text, belongs to
the [file-based path](working-with-files.md#pre-processed-content-for-a-real-file):
it requires `path=` alongside it.)

## The view schema

Shown here in YAML for readability; as a `spec=` dict it's exactly the
same structure.

```yaml
id: root
kind: Container
style: {flex_direction: horizontal, padding: 12, gap: 8}
children:
  - id: swatch
    kind: Rect
    style: {width: 40, height: 40, background: "#6750A4", corner_radius: 8}
  - id: label
    kind: Text
    text: {content: "Hello", font_family: Roboto, font_size: 16}
    style: {foreground: "#1D1B20"}
```

Every widget has:

| Field | Meaning |
| --- | --- |
| `id` | Author-assigned, stable identifier — used by `view.node(id)` and by reconciliation |
| `kind` | One of `Rect`, `Container`, `Text`, `Checkbox`, `Slider`, `TextField`, `Image`, `Icon`, `Link`, `RadioButton`, `Switch`, `CircularProgress`, `LinearProgress`, `LoadingIndicator`, `TimePickerDial` |
| `style` | See table below |
| `classes` | A list of style-class strings, matched by [stylesheet](#stylesheets-md3-color-tokens) rules |
| `children` | A list of nested widgets |

`style:` fields (all optional):

| Field | Type |
| --- | --- |
| `width`, `height` | number |
| `flex_direction` | `horizontal` or `vertical` |
| `padding`, `margin` | number, **or** a per-side object `{top, right, bottom, left}` (each defaults to `0`) |
| `gap` | number |
| `flex_grow`, `flex_shrink` | number |
| `flex_basis` | number (a pixel length, not a percentage) |
| `align_items` | `start`, `end`, `flex_start`, `flex_end`, `center`, `baseline`, `stretch` |
| `justify_content` | `start`, `end`, `flex_start`, `flex_end`, `center`, `stretch`, `space_between`, `space_around`, `space_evenly` |
| `background` | a fill: a hex (`"#6750A4"`, `"#6750A4FF"`) or CSS named color string, or (with a theme) an MD3 role name — see below |
| `foreground` | the glyph/text color of `Text`, `Link`, `Icon`, and `LoadingIndicator`, which have no fill — same string forms as `background` |
| `corner_radius` | number, **or** a named shape token: `none`, `extra_small`, `small`, `medium`, `large`, `extra_large` |
| `elevation` | number, **or** a named elevation token: `level_0` through `level_5` |
| `opacity` | number |
| `border_width` | number |
| `border_color` | a hex/CSS-name/MD3-role string, same parsing as `background` |

```yaml
style:
  padding: {top: 16, right: 12, bottom: 16, left: 12}
  flex_grow: 1
  align_items: Center
  corner_radius: small     # resolves to 8.0 via engine_md3::shape
  elevation: level_2       # resolves to 2.0
  border_width: 1
  border_color: outline
```

Kind-specific blocks:

- `kind: Text`, `kind: TextField`, or `kind: Link` — a required `text:`
  block: `{content, font_family, font_weight: 400.0, font_size}` (or
  `typography_role:`, an MD3 type-scale role name like `body_large`, supplying
  `font_family`/`font_weight`/`font_size`/`line_height` as defaults —
  any of those fields, if also given, override just that one field on
  top of the role's own default)
- `kind: Checkbox` — an optional top-level `checked: true`
- `kind: Switch` or `kind: RadioButton` — an optional top-level
  `selected: true`, MD3's own term for both. (`checked:` on either, or
  `selected:` on a `Checkbox`, is an error naming the right field.)
- `kind: Slider`, `kind: CircularProgress`, or `kind: LinearProgress` —
  an optional top-level `value: 0.5` (the two progress indicators'
  own initial progress fraction, `0.0`–`1.0`)
- `kind: TimePickerDial` — optional top-level `hour: 0`/`minute: 0`
  (a 24-hour value and `0`–`59` respectively)
- `kind: Image` — a required `image:` block: `{fit: Fill}` (`fit` is
  one of `cover`, `contain`, `fill`; `image: {}` takes the default).
  With no `src:` in it, the node starts as a blank, fully transparent
  placeholder; supply pixels with `view.node(id).push_frame(rgba,
  width, height)` — decoded RGBA8 data your application produced.
  (`src:`, a file path `tre` decodes itself, is covered in
  [Working with Files](working-with-files.md#images-from-files).)
- `kind: Icon` — a required `icon:` block: `{name}`, `name` an icon
  name from `tre`'s own curated set (the same vocabulary
  `Window.add_icon` uses imperatively). Its color is `style.foreground`,
  required.
- `kind: LoadingIndicator` — no kind-specific block; `style.width`/
  `height` size it and `style.foreground` is its color, all required.

`Text`, `Link`, `Icon`, and `LoadingIndicator` paint only a glyph or
text, so their color is `style.foreground` and they have no fill. A
`style.background` written on one of them is an error naming
`foreground`; one that reaches them only through a selector-less
stylesheet rule meant for every widget is ignored.

`RadioButton`/`Switch`/`CircularProgress`/`LinearProgress`/
`TimePickerDial` take no `style.background` at all — their visuals live
in internal, MD3-themed tint fields, resolved against the active theme
(falling back to the MD3 baseline colors when no theme is given) — the
same resolution every corresponding `Window.add_*` factory does.

A typo'd field name is a load-time error naming the bad key, not a
silently ignored style — unknown fields are rejected everywhere in this
schema.

## Stylesheets & MD3 color tokens

Pass `stylesheet_spec=` (a stylesheet as a `dict`) and `theme_seed=` (an
`(r, g, b, a)` tuple, the same shape `Window.set_theme` takes) to enable
the stylesheet cascade and MD3 token resolution:

```python
view = View(
    spec=gallery_spec,
    stylesheet_spec={"styles": [
        {"style": {"corner_radius": 4}},
        {"kind": "Rect", "style": {"corner_radius": 8, "background": "primary"}},
        {"classes": ["accent"], "style": {"corner_radius": 16, "background": "secondary"}},
    ]},
    theme_seed=(0x67, 0x50, 0xA4, 0xFF),
    dark=False,
)
```

A stylesheet is `{styles: [...]}`, a list of rules applied in cascade
precedence — baseline (no selector) → `kind:` → `classes:` (more classes
beat fewer) → `id:` → the widget's own inline `style:`. Two theme layers
sit beneath the stylesheet — `default theme < custom theme < stylesheet
< inline`; see [Theming & Accessibility → Theme documents](theming-and-accessibility.md#theme-documents).

With a `theme_seed` given, `style.background`/`border_color` values that
name a recognized MD3 role (`primary`, `on_primary`, `secondary`,
`surface`, `error`, and every other `ColorScheme` role) resolve against
that scheme instead of being parsed as a literal color — a role name
always wins over a same-named CSS color. Anything that isn't a
recognized role name falls back to literal color parsing (`"#6750A4"`,
`"transparent"`), so a stylesheet can freely mix token names and
literal colors. `corner_radius`/`elevation` use the same "named token
first, literal number always valid" contract against
`engine_md3::shape`'s vocabulary (`small`, `level_2`, etc., see the
`style:` table above) — an unrecognized token name for either is a
clear error, never a silent fallback to `0`.

!!! note
    `stylesheet_spec=`/`theme_seed=` are both optional and independent.
    With neither, a view uses literal colors only, no cascade. A token
    name given with no theme fails to parse as a literal color.

## Embedding components

A `View` can embed another view's spec as an independent
[`Component`](../api/python/view.md#component) — its own bindings and
handlers, ready for its own `ViewModel` — as a child of any node. Call
it once per instance, e.g. once per row in a list:

```python
for item in items:
    row = view.instantiate("", into=view.node("list"), spec=row_spec(item))
```

The first argument is a path, used only by the
[file-based form](working-with-files.md#components-from-files); pass
`""` with `spec=`. Components nest: `row.instantiate("", into=..., spec=...)`.

## Binding to a `ViewModel`

```python
from tre import Signal, View, ViewModel

view = View(spec={
    "id": "root",
    "kind": "Container",
    "style": {"flex_direction": "vertical", "width": 240, "height": 120, "gap": 8, "padding": 8},
    "children": [
        {"id": "agree", "kind": "Checkbox",
         "style": {"width": 24, "height": 24, "background": "#6750A4"},
         "bindings": {"checked": "{{ agreed.get() }}"}, "two_way": "checked"},
        {"id": "volume", "kind": "Slider",
         "style": {"width": 200, "height": 32, "background": "#03DAC6"},
         "bindings": {"value": "{{ level.get() }}"}, "two_way": "value"},
        {"id": "username", "kind": "TextField",
         "text": {"content": "", "font_family": "Roboto", "font_size": 16},
         "style": {"width": 200, "height": 32, "background": "#EEEEEE"},
         "bindings": {"text": "{{ name.get() }}"}, "two_way": "text"},
    ],
})


class SettingsVM(ViewModel):
    def __init__(self, view):
        self.agreed = Signal(False)
        self.level = Signal(0.3)
        self.name = Signal("jane")
        super().__init__(view)  # must run after the Signals exist --
                                 # _attach evaluates every binding immediately

    def bump(self):
        self.level.update(lambda v: v + 0.1)


vm = SettingsVM(view)
```

`ViewModel.__init__(view)` wires every declared `bindings:`/`handlers:`/
`two_way:` entry in one call:

- **`bindings: {property: "{{ expression }}"}`** — evaluates the
  expression once against the `ViewModel` immediately (applying the
  result to the node), then subscribes a re-evaluation on every `Signal`
  the expression read during that evaluation. Supported expressions:
  attribute access (`vm.attr`), indexing (`obj[i]`), zero-argument method
  calls (`signal.get()`), and `+ - * /`/comparison operators. The
  resolved value's type must match the property: numeric for animated
  properties (routed through the same path as `Node.animate(prop, v, 0)`),
  `bool` for `checked`, `str` for `text`.
- **`handlers: {on_click: "method_name"}`** — looks up `method_name` on
  the `ViewModel` and calls it with no arguments on the event.
  `on_click`, `on_hover_enter`, `on_hover_exit`, `on_change`,
  `on_focus_enter`, and `on_focus_exit` all fire, the same `EventKind`
  set `Node.set_on_click`/etc. dispatch from imperatively.
- **`two_way: <binding-key>`** — makes that binding a two-way write-back:
  on a `Change` (a `Slider` drag ending, or `Node.set_checked`/
  `set_text` being called), the node's new value is read back and written
  into the bound `Signal`. **The bound expression must be exactly a bare
  `signal.get()` call** — a computed expression (e.g. `{{ a.get() + b.get() }}`)
  can't be reversed into a single `Signal` write target and raises
  `ValueError` at attach time.

`Signal` is a minimal reactive cell: `.get()` records a read-dependency
when called during a binding's evaluation, `.set(value)`/`.update(fn)`
notify subscribers — but only when the value actually changes, which is
what keeps a two-way binding's forward/reverse wiring from recursing
forever.

## Updating a live view

`reconcile()` diffs a new spec against the live tree and patches it in
place — call it whenever your data changes (typically from a
`tre.Effect`):

```python
view.reconcile(spec=new_spec)
# or: view.reconcile(json=new_json_text)
```

An unchanged widget (same `id`, same `kind`) keeps its runtime identity,
preserving focus, scroll position, and in-flight animations. There's no
"did anything change" check to skip — the call itself is the change
signal. Exactly one content argument is required; `ValueError`
otherwise. After every update the attached `ViewModel` is re-applied against the
new spec: bound fields keep their live values, bindings and handlers the
update added start working, and ones it removed stop.

### Updating from another thread

Once `App.run()` starts, it owns the main thread, and a `View` can't be
touched from any other thread. When new data arrives on a background
thread — a file watcher, a network client — hand the update to the
event loop with [`App.thread_handle()`](../api/python/app.md#thread_handle).
`call_soon` wakes the loop even when it's idle:

```python
handle = app.thread_handle()

def on_new_data(spec):  # called on a background thread
    handle.call_soon(lambda: view.reconcile(spec=spec))
```

Do the slow work (reading, parsing, expanding) on the background thread
and hand only the `reconcile` to the loop.

### Live re-theme

`view.set_theme(theme_seed=None, dark=False, default_theme_spec=None, custom_theme_spec=None)`
re-resolves the theme exactly like `View(...)` does at construction,
then walks every already-built node and recomputes its style from its
own spec against the new theme layers, in place — `NodeId`s, children,
and focus are preserved, and a widget's own inline `style:` still wins
over any theme layer. Each call is a complete, fresh theme selection —
omitting the theme arguments resets to no override, not "keep whatever
the previous call used." Bindings are re-applied afterward, so a bound
field keeps its live value.

`view.set_stylesheet(stylesheet_spec=...)` replaces the stylesheet the
same way — every node re-resolved in place, bindings re-applied. Pass
nothing to clear it.

Re-applying a `checked` or `text` binding fires `Change`, as the initial
`_attach` does, so a declared `on_change` handler runs once per update.

## Testing without a live window

`View` mirrors `Window`'s synthetic dispatch methods for headless
testing:

```python
view.click(node)
view.hover(node)
view.right_click(node)
```
