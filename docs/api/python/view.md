# `View`

Loads a declarative `view.yaml` file into its own node tree. See
[Declarative Views](../../guide/declarative-views.md) for a full
walkthrough of the YAML schema and data binding.

## `View`

**`View(path, stylesheet=None, theme_seed=None, dark=False)`**

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

`View` has no width/height/render-loop of its own — it's never embedded
in a live `winit` window.

## `node`

**`node(widget_id) -> Node`**

Returns the [`Node`](node.md) for the widget with the given author-
assigned YAML `id`. Raises `ValueError` if no widget with that id exists.

```python
checkbox = view.node("agree")
```

## `poll_reload`

**`poll_reload() -> bool`**

Checks whether the underlying file changed since the last call (or
construction) and, if so, re-parses and reconciles it into the live tree
in place. Returns `False` if nothing changed. Raises `RuntimeError` if
re-reading fails, `ValueError` if reconciliation fails.

An unchanged widget (same `id`, same `kind`) keeps its runtime identity —
preserving focus, scroll position, and in-flight animations.
`bindings:`/`handlers:`/`two_way:` are **not** re-resolved automatically;
call `_attach` again if a reload adds a genuinely new one.

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
