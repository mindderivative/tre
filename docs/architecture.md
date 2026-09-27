# Architecture

A short orientation. The full design reference is
[`ARCHITECTURE.md`](https://github.com/mindderivative/tre/blob/main/ARCHITECTURE.md)
in the repository root.

## Four crates

```
engine-py        PyO3 classes, the per-frame loop, listeners, callbacks
   │
   ├── engine-platform   winit event loop, input translation, AccessKit adapter
   └── engine-render     scene building, text shaping, paths, shadows
         │
         └── engine-core  node tree, Animated<T>, layout, dispatch, focus, layers
```

`engine-core` depends on none of the others and has no Python, windowing,
or GPU code, so it's tested on its own. Only `engine-py` imports `pyo3`, and
its Python surface is the one stability contract. See
[Rust Crates](api/rust.md) for each crate's role.

## How a frame is made

1. **Input** from `winit` (or `window.simulate`) is hit-tested, moves focus,
   edits text, and reaches your listeners, bubbling from the target.
2. Callbacks queued with **`LoopHandle.call_soon`** run.
3. **One central tick** advances every running animation; completion
   callbacks run after it.
4. **Layout** runs through `taffy`'s cache — paint-only changes never
   dirty it — and virtual lists build their visible rows.
5. **Paint** walks the tree into a `vello_hybrid` scene, culling what's off
   screen, and renders it on the GPU. The **AccessKit** tree is built from
   the same node tree.

When nothing changed and nothing animates, the loop sleeps until the next
input or `call_soon`.

## Design principles

- **Rust owns every frame; Python owns intent.** Python never runs during
  layout, paint, or interpolation — only inside callbacks.
- **One animation mechanism.** Every animatable value is the same
  `Animated<T>`, ticked by one pass.
- **Mechanism in the engine, meaning and look in the framework.** `tre`
  reports what it can know — pointer, focus, whether focus came from the
  keyboard — and draws only what it's told.
- **Data in, not files.** Pixels, fonts, and tree content arrive as data;
  the framework owns every format.

## History

`tre` is a from-scratch second iteration of an earlier Vulkan engine,
archived under
[`archive/`](https://github.com/mindderivative/tre/tree/main/archive) with
its lessons learned.
[`BUILD_TRACKER.md`](https://github.com/mindderivative/tre/blob/main/BUILD_TRACKER.md)
records every milestone, including the 0.3.5 program that moved Material
Design 3 and the declarative layer out to the framework.
