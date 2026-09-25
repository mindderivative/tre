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

### Building from already-parsed content

`View` can also build straight from YAML text, JSON text, or a real
Python object, instead of reading `path` from disk — useful for a
framework layer that has already parsed or pre-processed a view (e.g.
expanding its own macro syntax) before handing it to `tre`:

```python
view = View(source="id: root\nkind: Container\nstyle: {width: 10, height: 10}\n")
view = View(json='{"id": "root", "kind": "Container", "style": {"width": 10, "height": 10}}')
view = View(spec={"id": "root", "kind": "Container", "style": {"width": 10, "height": 10}})
```

- **`source=`** — YAML text, used directly instead of reading `path`
  from disk. `path` is still required alongside it — it supplies the
  base directory `include:`/`image.src:` resolve against, and the real
  file `poll_reload()`/hot-reload watches (the developer keeps editing
  the real file on disk; `source=` just supplies its already-read
  content for this one construction).
- **`json=`** — JSON text, parsed directly into the tree — no YAML
  involved at all. Grouped with `spec=`, not `source=`: no real backing
  file is implied, so `path=` is optional.
- **`spec=`** — a real Python object (a `dict` shaped like the YAML
  tree above) built directly into the tree — no text parsing of any
  kind. `path=` is optional; when omitted, there's no base directory to
  resolve `include:`/`image.src:` against and no file to watch, so
  `poll_reload()` always returns `False` — use
  [`reconcile()`](#reconcile) instead.

At most one of `spec=`/`source=`/`json=` may be given; at least one of
`spec=`/`json=`/`path=` is required — `View()` with none of them raises
`ValueError`.

## The YAML schema

```yaml
id: root
kind: Container
style: {flex_direction: Horizontal, padding: 12, gap: 8}
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
| `kind` | One of `Rect`, `Container`, `Text`, `Checkbox`, `Slider`, `TextField`, `Image`, `Icon`, `Link`, `RadioButton`, `Switch`, `CircularProgress`, `LinearProgress`, `LoadingIndicator`, `TimePickerDial` |
| `style` | See table below |
| `classes` | A list of style-class strings (parsed, but see the note on stylesheets below) |
| `children` | A list of nested widgets |

`style:` fields (all optional):

| Field | Type |
| --- | --- |
| `width`, `height` | number |
| `flex_direction` | `Horizontal` or `Vertical` |
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

- `kind: Text`, `kind: TextField`, or `kind: Link` — a required `text:`
  block: `{content, font_family, font_weight: 400.0, font_size}` (or
  `role:`, an MD3 typography role name like `body_large`, supplying
  `font_family`/`font_weight`/`font_size`/`line_height` as defaults —
  any of those fields, if also given, override just that one field on
  top of the role's own default)
- `kind: Checkbox` or `kind: RadioButton` — an optional top-level
  `checked: true` (`RadioButton`'s own initial `selected` state)
- `kind: Slider`, `kind: CircularProgress`, or `kind: LinearProgress` —
  an optional top-level `value: 0.5` (the two progress indicators'
  own initial progress fraction, `0.0`–`1.0`)
- `kind: Switch` — an optional top-level `checked: true` (its own
  initial `on` state)
- `kind: TimePickerDial` — optional top-level `hour: 0`/`minute: 0`
  (a real 24-hour value and `0`–`59` respectively)
- `kind: Image` — an optional `image:` block: `{src, fit: Fill}` (`fit`
  is one of `Cover`, `Contain`, `Fill`); `src` is a path relative to the
  `view.yaml` file's own directory. Omit `image:` (or `src:` inside it)
  entirely for a blank, fully-transparent placeholder — the same
  synthetic 1×1 image `Window.add_video` builds imperatively — meant to
  be filled in later via `Node.push_frame` from Python.
- `kind: Icon` — a required `icon:` block: `{name}`, `name` a real
  icon name from `tre`'s own curated set (the same vocabulary
  `Window.add_icon` uses imperatively). The glyph's own color reuses
  `style.background`, the same "background means paint color" contract
  `kind: Text` already has.
- `kind: LoadingIndicator` — no kind-specific block; `style.width`/
  `height` size it and `style.background` is its glyph tint (again,
  the same "background means paint color" contract, not a fill behind
  content), both required.

`RadioButton`/`Switch`/`CircularProgress`/`LinearProgress`/
`TimePickerDial` take no `style.background` at all — their entire real
visual lives in internal, MD3-themed tint fields, resolved
automatically against the active theme (falling back to the real MD3
baseline colors when no `theme_seed=`/theme file is given) — the same
resolution every corresponding `Window.add_*` factory already does
imperatively.

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

!!! note "Stylesheets and themes as data"
    `stylesheet_spec=`, `default_theme_spec=`, and `custom_theme_spec=`
    take a `dict` in the same schema as the corresponding YAML file,
    mutually exclusive with the path form — for a framework that loads
    its own files. See
    [Theming & Accessibility → Themes as data](theming-and-accessibility.md#themes-as-data).

## Composing with `include:`

Split a view across files — an `include:` entry splices the target
file's own widget tree in as an ordinary child, indistinguishable from an
inline one once loaded:

```yaml
id: root
kind: Container
style: {flex_direction: Vertical, width: 240, height: 120, gap: 8, padding: 8}
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
style: {flex_direction: Vertical, width: 240, height: 120, gap: 8, padding: 8}
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

`poll_reload(source=...)` reconciles the given YAML text instead of a
fresh disk read of `path` — the change-detection gate still watches
`path` on disk regardless, so this only changes what gets reconciled
once a real file change is detected, not whether one is.

### `reconcile`

A `View` built with `spec=`/`json=` and no `path=` has no file to watch
at all, so `poll_reload()` always returns `False` for it. `reconcile()`
is the ungated sibling for exactly that case — call it directly
(typically driven by a `tre.Effect`) whenever your own data changes:

```python
view.reconcile(spec=new_spec)
# or: view.reconcile(source=new_yaml_text)
# or: view.reconcile(json=new_json_text)
```

Unlike `poll_reload()`, `reconcile()` always reconciles against
whatever you pass — there's no "did anything change" check to skip, the
call itself is the change signal. Exactly one of `source=`/`spec=`/
`json=` is required; `ValueError` otherwise.

### Live re-theme

`view.set_theme(default_theme=None, custom_theme=None, theme_seed=None, dark=False)`
re-resolves the theme exactly like `View(...)` does at construction,
then walks every already-built node and recomputes its style from its
own spec against the new theme layers, in place — `NodeId`s, children,
and focus are preserved, and a widget's own inline `style:` still wins
over any theme layer. Each call is a complete, fresh theme selection —
omitting `default_theme`/`custom_theme` resets to no override, not
"keep whatever the previous call used." Only the static style cascade
is recomputed; a `{{ }}` binding's own currently-applied value isn't
re-run (reverts to its spec's static value, the same as a content-only
`poll_reload()`).

## Testing without a live window

`View` mirrors `Window`'s synthetic dispatch methods for headless
testing:

```python
view.click(node)
view.hover(node)
view.right_click(node)
```
