# Declarative Views (YAML)

The declarative path builds a UI from a `view.yaml` file instead of
imperative Python calls, with data binding against a plain Python
`ViewModel`. It's the same underlying node tree either way — a `View`'s
nodes support the same `Node` methods (`animate`, `get`, etc.) as a
`Window`'s.

## Loading a view

```python
from tre import View

view = View("counter.yaml")
node = view.node("my_widget")  # look up a widget by its author-assigned id
```

`View(path)` reads and parses the file (resolving any `include:`
directives relative to the file's own directory), and starts a filesystem
watcher for hot-reload.

## The YAML schema

```yaml
id: root
kind: Container
style: {flex_direction: Row, padding: 12, gap: 8}
children:
  - id: swatch
    kind: Rect
    style: {width: 40, height: 40, background: "#6750A4", corner_radius: 8}
  - id: label
    kind: Text
    text: {content: "Hello", font_family: Roboto, font_size: 16}
```

Every widget has:

| Field | Meaning |
| --- | --- |
| `id` | Author-assigned, stable identifier — used by `view.node(id)` and by hot-reload reconciliation |
| `kind` | One of `Rect`, `Container`, `Text`, `Checkbox`, `Slider`, `TextField`, `Image` |
| `style` | See table below |
| `classes` | A list of style-class strings (parsed, but see the note on stylesheets below) |
| `children` | A list of nested widgets |

`style:` fields (all optional):

| Field | Type |
| --- | --- |
| `width`, `height` | number |
| `flex_direction` | `Row` or `Column` |
| `padding`, `margin` | number, **or** a per-side object `{top, right, bottom, left}` (each defaults to `0`) |
| `gap` | number |
| `flex_grow`, `flex_shrink` | number |
| `flex_basis` | number (a pixel length, not a percentage) |
| `align_items` | `Start`, `End`, `FlexStart`, `FlexEnd`, `Center`, `Baseline`, `Stretch` |
| `justify_content` | `Start`, `End`, `FlexStart`, `FlexEnd`, `Center`, `Stretch`, `SpaceBetween`, `SpaceAround`, `SpaceEvenly` |
| `background` | a hex (`"#6750A4"`, `"#6750A4FF"`) or CSS named color string, or (with `theme_seed=`) an MD3 role name — see below |
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

- `kind: Text` or `kind: TextField` — a required `text:` block:
  `{content, font_family, font_weight: 400.0, font_size}`
- `kind: Checkbox` — an optional top-level `checked: true`
- `kind: Slider` — an optional top-level `value: 0.5`
- `kind: Image` — a required `image:` block: `{src, fit: Fill}` (`fit` is
  one of `Cover`, `Contain`, `Fill`); `src` is a path relative to the
  `view.yaml` file's own directory

A typo'd field name is a load-time error naming the bad key and its line
number, not a silently-ignored style — unknown fields are rejected
everywhere in this schema.

## Stylesheets & MD3 color tokens

Pass `stylesheet=` (a path to a stylesheet YAML file) and `theme_seed=`
(an `(r, g, b, a)` tuple, the same shape `Window.set_theme` takes) to
`View(...)` to enable the real stylesheet cascade and MD3 token
resolution:

```python
view = View(
    "gallery.yaml",
    stylesheet="gallery_sheet.yaml",
    theme_seed=(0x67, 0x50, 0xA4, 0xFF),
    dark=False,
)
```

A stylesheet is `{styles: [...]}`, a list of rules applied in real
cascade precedence — baseline (no selector) → `kind:` → `classes:`
(more classes beat fewer) → `id:` → the widget's own inline `style:`:

```yaml
styles:
  - style: {corner_radius: 4}
  - kind: Rect
    style: {corner_radius: 8, background: primary}
  - classes: [accent]
    style: {corner_radius: 16, background: secondary}
```

With a real `theme_seed` given, `style.background`/`border_color` values
that name a recognized MD3 role (`primary`, `on_primary`, `secondary`,
`surface`, `error`, and every other real `ColorScheme` role) resolve
against that scheme instead of being parsed as a literal color — a role
name always wins over a same-named coincidental CSS color. Anything
that isn't a recognized role name still falls back to literal color
parsing (`"#6750A4"`, `"transparent"`), so a stylesheet can freely mix
token names and literal colors. `corner_radius`/`elevation` use the
same "named token first, literal number always still valid" contract,
just against `engine_md3::shape`'s own vocabulary (`small`, `level_2`,
etc., see the `style:` table above) instead of a color scheme — an
unrecognized token name for either is a real, clear parse-time error,
never a silent fallback to `0`.

!!! note
    `stylesheet=`/`theme_seed=` are both optional and independent — a
    `View(path)` call with neither given (or `stylesheet=` given but no
    `theme_seed=`) works exactly as before: literal colors only, no
    cascade. A token name given with no `theme_seed=` fails to parse as
    a literal color, the same real error it always would have.
    `poll_reload()` re-resolves against the same stylesheet/theme on
    every hot-reload, not just the initial load.

## Composing with `include:`

Split a view across files — an `include:` entry splices the target
file's own widget tree in as an ordinary child, indistinguishable from an
inline one once loaded:

```yaml
id: root
kind: Container
style: {flex_direction: Column, width: 240, height: 120, gap: 8, padding: 8}
children:
  - id: header
    kind: Rect
    style: {width: 200, height: 16, background: "#6750A4"}
  - include: confirm_dialog.yaml
```

Included paths are resolved relative to the including file's own
directory and confined to stay within it (no `../` escape), cycles are
detected, and includes nest up to 8 deep.

## Binding to a `ViewModel`

```python
from pathlib import Path
from tre import Signal, View, ViewModel

view = View(str(Path(__file__).parent / "settings.yaml"))


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

```yaml
# settings.yaml
id: root
kind: Container
style: {flex_direction: Column, width: 240, height: 120, gap: 8, padding: 8}
children:
  - id: agree
    kind: Checkbox
    style: {width: 24, height: 24, background: "#6750A4"}
    bindings: {checked: "{{ agreed.get() }}"}
    two_way: checked
  - id: volume
    kind: Slider
    style: {width: 200, height: 32, background: "#03DAC6"}
    bindings: {thumb_position: "{{ level.get() }}"}
    two_way: thumb_position
  - id: username
    kind: TextField
    text: {content: "", font_family: Roboto, font_size: 16}
    style: {width: 200, height: 32, background: "#EEEEEE"}
    bindings: {text: "{{ name.get() }}"}
    two_way: text
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
  the `ViewModel` and calls it with no arguments on the real event.
  `on_click`, `on_hover_enter`, `on_hover_exit`, `on_change`,
  `on_focus_enter`, and `on_focus_exit` all fire today, the same real
  `EventKind` set `Node.set_on_click`/etc. dispatch from imperatively.
- **`two_way: <binding-key>`** — makes that binding a two-way write-back:
  on a real `Change` (a `Slider` drag ending, or `Node.set_checked`/
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

## Hot reload

```python
if view.poll_reload():
    print("view changed on disk, reconciled in place")
```

Call `poll_reload()` periodically (e.g. once per frame in your own loop)
to check whether the underlying file changed and, if so, re-parse and
reconcile it into the live tree — an unchanged widget (same `id`, same
`kind`) keeps its runtime identity, preserving focus, scroll position,
and in-flight animations. **`bindings:`/`handlers:`/`two_way:` are not
re-resolved automatically** — if a reload adds a genuinely new binding or
handler, call `_attach` again (constructing a fresh `ViewModel`, or
calling `view._attach(vm)` directly) to pick it up.

## Testing without a live window

`View` mirrors `Window`'s synthetic dispatch methods for headless
testing:

```python
view.click(node)
view.hover(node)
view.right_click(node)
```
