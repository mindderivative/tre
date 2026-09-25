# PLAN — Branch `0.3.4`: Milestone 96, Layer, Structure, and Update Building Blocks

*(Replaces the M95 plan — M95 and the M94 focus follow-up are complete and
pushed. Every step is in `BUILD_TRACKER.md`.)*

## Goal

The last additive milestone before the Tesserae migration gate:
- the tree operations a keyed reconciler needs;
- node lifetime that can't leak and can't crash off-thread (issue #10);
- deterministic headless time;
- `create` for every kind and `set`/`get` for every property;
- text measurement and truncation;
- one layer mechanism for dialogs, menus, tooltips, snackbars, and sheets.

Legacy names and behavior stay until 0.3.5.

## Findings from the source

- **Structure.**
  - `Tree` has `add_child`, `try_add_child` (checked, reparents),
    `detach` (keeps the node), and `remove` (frees the subtree).
  - Python's `Node.remove()` currently *frees*; R5 makes it detach.
  - There's no insert-at-index, `children()`, `parent()`, or `destroy()`.
  - `detach` clears focus, so a move must keep focus explicitly.
- **Lifetime (issue #10).**
  - `App`, `Window`, `Node`, `Theme`, `View`, `Component`, and two
    `view.rs` binding callbacks are `#[pyclass(unsendable)]`.
  - pyo3 0.29 leaks such an object when the cyclic collector frees it on
    another thread, and panics in `__clear__`.
  - A class that isn't `unsendable` must be `Send + Sync`. So each class
    becomes a thin `Send + Sync` shell around a thread-checked value
    (`ThreadBound<T>`):
    - using it from another thread panics, as today;
    - dropping it there hands the value to a queue drained on the
      owning thread;
    - `__traverse__`/`__clear__` do nothing off-thread.
- **Auto-free.**
  - Only a node detached through the new API (`create`, `remove()`,
    `hide_layer`) is collectible; legacy detached content (context menus,
    inactive dock panels) never is.
  - Handle counts live in `Tree`. When the last handle to anything in a
    collectible subtree goes, the subtree is freed and its listeners are
    pruned.
- **Time.**
  - `engine-core` never reads the clock; timestamps are parameters.
  - `engine-py` reads `Instant::now()` at 24 sites. `window.advance(ms)`
    pins a per-tree virtual clock (`Tree::now`) that those sites use;
    `App.run()` unpins it.
- **Batching.**
  - Layout runs once per frame (`app.rs` frame closure), never per
    property change, and no frame can run inside a Python call.
  - So `window.batch()` has nothing to defer unless a measurement shows
    otherwise. It is measured in Phase 2 and dropped from the spec if
    it's a no-op.
- **Window content.**
  - `show_view` swaps a whole tree because each `View` owns one. With
    every node in the window's one tree, `set_content` is
    `root.remove()`-children plus `add_child`, so it is likely redundant
    too (decided in Phase 2).
- **Text.**
  - Text nodes have no intrinsic size; every factory passes one.
  - Parley 0.11 has `LetterSpacing`, `FontStyle`, and `TextWrapMode`,
    but no line limit or ellipsis. The renderer truncates to `max_lines`
    and fits an ellipsis itself.
  - `get_monospace_cell_size` builds a new `TextRenderer` (re-registering
    every font) per call; `measure_text` and it share one per thread.
- **Layers.**
  - The legacy overlay mechanism already appends absolutely positioned
    content to the root, so layout, paint, hit-testing, and access work
    unchanged, and has modal blocking and outside/Escape dismissal.
  - It is extended rather than duplicated:
    - ordered stacking instead of a `HashMap`;
    - an optional anchor with flip/shift placement and a reported side;
    - dismissal reported as a `dismiss` event instead of self-closing,
      for new layers;
    - focus trap and restore;
    - per-layer focus scope;
    - bubbling stops at the layer.
  - `add_child` inserts before open layers so content never paints over
    them.
- **Missing properties.**
  - `visible` (not painted, not hit, no layout space, not in access or
    tab order) and `z_index` (sibling paint and hit order) don't exist.
  - `flex_wrap`, `align_self`, `aspect_ratio`, and min/max sizes are
    plain taffy fields.

## Phases

1. Structure, lifetime, and time.
2. Creation and properties.
3. Text measurement and truncation.
4. Layers.
5. Verification: stubs, docs, spec, full chain.

## Status

Scoped (2026-09-25). Phase 1 next.
