# `View`

A declarative view: a tree of widget specs built into its own node tree.
See [Declarative Views](../../guide/declarative-views.md) for a full
walkthrough of the schema and data binding, and
[Working with Files](../../guide/working-with-files.md) for the
file-based conveniences.

## `View`

**`View(path=None, stylesheet=None, theme_seed=None, dark=False, default_theme=None, custom_theme=None, source=None, spec=None, json=None, stylesheet_spec=None, default_theme_spec=None, custom_theme_spec=None)`**

Builds the widget tree from exactly one content source:

- **`spec`** — a Python object (nested `dict`s/`list`s shaped like the
  [view schema](../../guide/declarative-views.md#the-view-schema)),
  built directly into the tree with no text parsing.
- **`json`** — the same structure as JSON text.
- **`path`** *(file convenience)* — reads and parses a YAML view file,
  resolving `include:`/`image.src:` relative to its directory, and
  starts a filesystem watcher for [`poll_reload`](#poll_reload).
- **`source`** *(file convenience)* — YAML text used instead of reading
  `path`; requires `path` alongside it for the base directory and the
  watched file.

```python
view = View(spec={"id": "root", "kind": "Container", "style": {"width": 100, "height": 40}})
```

At most one of `spec`/`source`/`json` may be given, and at least one of
`spec`/`json`/`path` is required — `ValueError` otherwise. A build or
schema error raises `ValueError`; a file that can't be read raises
`RuntimeError`.

Styling and theming:

- **`theme_seed`** — an `(r, g, b, a)` seed; builds an MD3 color scheme
  so style values like `background: primary` resolve as role names.
  `dark` picks the dark scheme.
- **`stylesheet_spec`** — a stylesheet `dict` (`{"styles": [...]}`),
  enabling the [cascade](../../guide/declarative-views.md#stylesheets-md3-color-tokens).
- **`default_theme_spec`**/**`custom_theme_spec`** — [theme documents](../../guide/theming-and-accessibility.md#theme-documents)
  as `dict`s, two more cascade tiers beneath the stylesheet —
  `default theme < custom theme < stylesheet < inline`. Omitting the
  default uses the theme shipped inside `tre`. A theme's own `seed`
  applies when `theme_seed` isn't given (an explicit `theme_seed` wins,
  then the custom theme's, then the default's).
- **`stylesheet`**/**`default_theme`**/**`custom_theme`** *(file
  conveniences)* — the same three as YAML file paths. Each is mutually
  exclusive with its `*_spec` twin; passing both raises `ValueError`.

The stylesheet and theme are remembered and reused by `poll_reload`/
`reconcile`. `View` has no width/height/render loop of its own — show
it in a window with [`Window.from_view`](window.md).

## `node`

**`node(widget_id) -> Node`**

Returns the [`Node`](node.md) for the widget with the given author-
assigned `id`. Raises `ValueError` if no widget with that id exists.

```python
checkbox = view.node("agree")
```

## `poll_reload`

**`poll_reload(source=None) -> bool`** *(file convenience)*

For a `View` built from a `path`: checks whether the underlying file changed since the last call (or
construction) and, if so, re-parses and reconciles it into the live tree
in place. Returns `False` if nothing changed. Raises `RuntimeError` if
re-reading fails, `ValueError` if reconciliation fails.

An unchanged widget (same `id`, same `kind`) keeps its runtime identity —
preserving focus, scroll position, and in-flight animations. After every update the attached `ViewModel` is re-applied against the
new spec: bound fields keep their live values, bindings and handlers the
update added start working, and ones it removed stop.

`source`, when given, is reconciled instead of a fresh disk read of
`path` once a change is detected — the change-detection gate itself
still watches `path` on disk regardless. A `View` built with `spec=`
(no `path=`) has no watcher at all and always returns `False` here —
use [`reconcile`](#reconcile) instead.

## `reconcile`

**`reconcile(source=None, spec=None, json=None)`**

Diffs new content against the live tree and patches it in place —
unconditionally, with no "did anything change" check, since the call
itself is the change signal (typically driven by a `tre.Effect`). Takes
`spec` (a Python object), `json` (JSON text), or `source` (YAML text).
An unchanged widget (same `id`, same `kind`) keeps its runtime
identity, and the attached `ViewModel`'s bindings are re-applied. To
call it from a background thread while `App.run()` is
running, go through [`App.thread_handle()`](app.md#thread_handle).

```python
view.reconcile(spec=new_spec)
```

At most one of `spec`/`source`/`json` may be given; exactly one is
required. Raises `ValueError` otherwise.

## `set_theme`

**`set_theme(default_theme=None, custom_theme=None, theme_seed=None, dark=False, default_theme_spec=None, custom_theme_spec=None)`**

Live re-theme: re-resolves the theme exactly like `View(...)` does at
construction, then walks every already-built node and recomputes its
`PaintProperties`/layout style from its own spec against the new theme
layers, overwriting in place. `NodeId`/children/focus are preserved; a
widget's own inline `style:` still wins over any theme layer, exactly
like at construction time. `default_theme_spec`/`custom_theme_spec` are
the `dict` forms of `default_theme`/`custom_theme`, same contract as on
`View(...)`.

Each call is a complete, fresh theme selection — omitting
`default_theme`/`custom_theme` resets to the engine's shipped default /
no custom override, not "keep whatever the previous call used." A
`poll_reload()` called after this continues resolving against the theme
this call installed.

The attached `ViewModel`'s bindings are re-applied afterward, so a
bound field keeps its live value. Re-applying a `checked` or `text` binding fires `Change`, as the initial
`_attach` does, so a declared `on_change` handler runs once per update.

## `set_stylesheet`

**`set_stylesheet(stylesheet_spec=None, stylesheet=None)`**

Replaces this view's stylesheet and re-resolves every node in place,
like [`set_theme`](#set_theme) — `NodeId`s, focus, and in-flight
animations are preserved, and bindings are re-applied afterward.
`stylesheet_spec` (a `dict`) and `stylesheet` *(file convenience — a
YAML path)* are mutually exclusive; passing neither clears the
stylesheet. The new stylesheet is kept for later `set_theme`/`reconcile`/
`poll_reload` calls.

```python
view.set_stylesheet(stylesheet_spec={"styles": [{"kind": "Rect", "style": {"corner_radius": 8}}]})
```

## `instantiate`

**`instantiate(path, into, source=None, spec=None) -> Component`**

Embeds another view as a real, independent
[`Component`](#component) — its own bindings/handlers, ready for its
own separate `ViewModel` to `_attach` to — spliced into this `View`'s
live tree as a child of `into` (a [`Node`](node.md)). Call this once
per instance for multiple simultaneous instances (e.g. one per row in a
list); each instantiation is fully independent, even when built from
the same spec.

```python
row = view.instantiate("", into=view.node("list_container"), spec=row_spec)
```

`spec` is a Python object built directly into the tree, mirroring
`View.__init__`'s own `spec=`. **`path` is positional and required**
(changing that would break every positional call); pass `""` with
`spec`.

*(File convenience)* `instantiate("row.yaml", into)` reads the component
from a YAML file instead, and `source` supplies already-read YAML text
for that file — see [Working with Files](../../guide/working-with-files.md#components-from-files).

At most one of `spec`/`source` may be given; at least one of
`spec`/`source`/a non-empty `path` is required.

## `Component`

One real, embedded instance created via `View.instantiate`/
`Component.instantiate` — not constructed directly. Behaves like a
small `View` scoped to just this instance's own widgets, sharing the
same live tree as whatever it was instantiated into.

```python
inner = component.instantiate("", into=component.node("slot"), spec=nested_spec)
```

- **`node(widget_id) -> Node`** — looks up a declared widget by its own
  `id:`, scoped to this component instance.
- **`instantiate(path, into, source=None, spec=None) -> Component`** —
  embeds another component inside this one; components nest
  recursively, the identical mechanism `View.instantiate` itself uses.
  Same `source=`/`spec=` contract as `View.instantiate` above.

Has no `click`/`hover`/`right_click` of its own — dispatch on one of its
nodes goes through the *owning* `View`/`Window`'s existing method
instead, e.g. `view.click(component.node("button"))`.

## `_attach`

**`_attach(viewmodel)`**

Called by [`ViewModel.__init__`](signal-and-viewmodel.md#viewmodel) —
not usually called directly. Wires every declared `bindings:`/
`handlers:`/`two_way:` entry against `viewmodel`. See
[Declarative Views → Binding to a ViewModel](../../guide/declarative-views.md#binding-to-a-viewmodel).

## Synthetic input dispatch

Mirrors [`Window`](window.md)'s synthetic dispatch, for headless testing:

```python
view.click(node)
view.hover(node)
view.right_click(node)
```

Layout is computed with unconstrained (`MaxContent`) width/height on both
axes, since `View` has no fixed window size — a view's root is
expected to declare explicit `style.width`/`style.height`.
