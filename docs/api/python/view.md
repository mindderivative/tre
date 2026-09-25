# `View`

Loads a declarative `view.yaml` file into its own node tree. See
[Declarative Views](../../guide/declarative-views.md) for a full
walkthrough of the YAML schema and data binding.

## `View`

**`View(path=None, stylesheet=None, theme_seed=None, dark=False, default_theme=None, custom_theme=None, source=None, spec=None, json=None, stylesheet_spec=None, default_theme_spec=None, custom_theme_spec=None)`**

Reads and parses the file at `path` (resolving `include:` directives
relative to its own directory, recursively), builds the widget tree, and
starts a filesystem watcher for hot-reload. Raises `RuntimeError` if the
file can't be read, `ValueError` if the YAML fails to parse or build.

```python
view = View("counter.yaml")
```

`stylesheet` (a path to a stylesheet YAML file) and `theme_seed` (an
`(r, g, b, a)` tuple, `dark` a bool) enable the real stylesheet cascade
and MD3 color token resolution — see
[Declarative Views → Stylesheets & MD3 color tokens](../../guide/declarative-views.md#stylesheets-md3-color-tokens).
Both default to `None`/`False`, byte-for-byte the prior literal-colors-
only behavior. Both are remembered and reused by `poll_reload()` on
every future hot-reload, not just the initial load.

`default_theme`/`custom_theme` (paths to theme YAML files) are two more
cascade tiers, resolved *beneath* `stylesheet` and a widget's own inline
`style:` — `default theme < custom theme < stylesheet < inline`. Either
theme's own `seed:` (if present) sets the seed when `theme_seed` isn't
explicitly given (an explicit `theme_seed` always wins). Omitting
`default_theme` uses the engine's own shipped default.

`stylesheet_spec`/`default_theme_spec`/`custom_theme_spec` are the data
forms of `stylesheet`/`default_theme`/`custom_theme`: a plain `dict` in
the same schema the YAML file holds, for a caller (a framework like
Tesserae) that loads its own files and hands `tre` data only. They
cascade identically to their path forms. Each is mutually exclusive with
its path twin — passing both raises `ValueError`, as does an unknown key
(the same typo check the YAML form gets).

```python
view = View(
    spec={"id": "root", "kind": "Checkbox", "style": {"width": 20, "height": 20}},
    custom_theme_spec={"colors": {"primary": "#FF0000"}},
    stylesheet_spec={"styles": [{"kind": "Checkbox", "style": {"corner_radius": 4}}]},
)
```

`View` has no width/height/render-loop of its own — it's never embedded
in a live `winit` window.

### Building without a file on disk

`path` is optional when `spec=`/`json=` is given instead — see
[Declarative Views → Building from already-parsed content](../../guide/declarative-views.md#building-from-already-parsed-content)
for the full walkthrough:

- **`source`** — YAML text, used directly instead of reading `path`
  from disk. `path` is still required — it supplies the base directory
  `include:`/`image.src:` resolve against and the file `poll_reload()`
  watches.
- **`spec`** — a real Python object (a `dict` shaped like the view's own
  YAML tree) built directly into the tree, no text parsing at all.
  `path` becomes optional.
- **`json`** — JSON text, parsed directly into the tree. Grouped with
  `spec`, not `source`: no real backing file is implied, so `path` is
  optional here too.

At most one of `spec`/`source`/`json` may be given; at least one of
`spec`/`json`/`path` is required. Raises `ValueError` if more than one
of `spec`/`source`/`json` is given, or if none of `spec`/`json`/`path`
is given.

## `node`

**`node(widget_id) -> Node`**

Returns the [`Node`](node.md) for the widget with the given author-
assigned YAML `id`. Raises `ValueError` if no widget with that id exists.

```python
checkbox = view.node("agree")
```

## `poll_reload`

**`poll_reload(source=None) -> bool`**

Checks whether the underlying file changed since the last call (or
construction) and, if so, re-parses and reconciles it into the live tree
in place. Returns `False` if nothing changed. Raises `RuntimeError` if
re-reading fails, `ValueError` if reconciliation fails.

An unchanged widget (same `id`, same `kind`) keeps its runtime identity —
preserving focus, scroll position, and in-flight animations.
`bindings:`/`handlers:`/`two_way:` are **not** re-resolved automatically;
call `_attach` again if a reload adds a genuinely new one.

`source`, when given, is reconciled instead of a fresh disk read of
`path` once a change is detected — the change-detection gate itself
still watches `path` on disk regardless. A `View` built with `spec=`
(no `path=`) has no watcher at all and always returns `False` here —
use [`reconcile`](#reconcile) instead.

## `reconcile`

**`reconcile(source=None, spec=None, json=None)`**

The ungated sibling of `poll_reload` for a `View` with no backing file
to watch (built via `spec=`/`json=`). Reconciles against `source` (YAML
text), `spec` (a real Python object), or `json` (JSON text) —
unconditionally, with no "did anything change" check, since the call
itself is the change signal (typically driven by a `tre.Effect`).

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

Only the static style cascade is recomputed — a `{{ }}` binding's own
currently-applied value is not re-run, so a bound field reverts to its
spec's own static value (the same as any content-only `poll_reload()`
already does).

## `instantiate`

**`instantiate(path, into, source=None, spec=None) -> Component`**

Embeds another view's own YAML as a real, independent
[`Component`](#component) — its own bindings/handlers, ready for its
own separate `ViewModel` to `_attach` to — spliced into this `View`'s
live tree as a child of `into` (a [`Node`](node.md)). Call this once
per instance for multiple simultaneous instances (e.g. one per row in a
list); each instantiation is fully independent, even when the same
`path` is used repeatedly.

```python
row = view.instantiate("row.yaml", into=view.node("list_container"))
```

`source`, when given, is used directly instead of reading `path` from
disk — the same real `source=` precedent `View.__init__` has, widened
here for a framework layer with its own pre-processed component YAML.

`spec`, when given, is a real Python object built directly into the
tree, mirroring `View.__init__`'s own `spec=` — no YAML text at all.
Unlike `View.__init__`, **`path` stays required** here: every real call
site already calls `instantiate(path, into)` positionally, and `into`
(also required) comes right after it, so making `path` optional would
break every one of them. Pass `path=""` when using `spec=`/`source=`
with no real file to name — the same "no base directory" outcome an
omitted `path` means for `View.__init__`.

At most one of `spec`/`source` may be given; at least one of
`spec`/`source`/a non-empty `path` is required.

## `Component`

One real, embedded instance created via `View.instantiate`/
`Component.instantiate` — not constructed directly. Behaves like a
small `View` scoped to just this instance's own widgets, sharing the
same live tree as whatever it was instantiated into.

```python
inner = component.instantiate("nested.yaml", into=component.node("slot"))
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
axes, since `View` has no fixed window size — a real `view.yaml`'s root
is expected to declare explicit `style.width`/`style.height`.
