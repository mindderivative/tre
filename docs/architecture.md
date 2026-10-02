# Architecture

A short orientation. The full design reference is
[`ARCHITECTURE.md`](https://github.com/mindderivative/tre/blob/main/ARCHITECTURE.md)
in the repository root.

## Four crates

```
engine-py        PyO3 classes, the per-frame loop, listeners, callbacks
   │
   ├── engine-platform   winit event loop, input translation, AccessKit adapter
   └── engine-render     scene building, damage tracking, persistent target, text
         │
         └── engine-core  node tree, Animated<T>, layout, dispatch, focus, layers
```

`engine-core` depends on none of the others and has no Python, windowing,
or GPU code, so it's tested on its own. Only `engine-py` imports `pyo3`, and
its Python surface is the one stability contract. `engine-platform` and
`engine-render` both build on `engine-core`. See
[Rust Crates](api/rust.md) for each crate's role.

## How a frame is made

1. **Input** from `winit` (or `window.simulate`) is hit-tested, moves focus,
   edits text, and reaches your listeners, bubbling from the target. A press
   on a drawn title bar or resize border then goes to the OS, to move or
   resize the window ([Custom Title Bars](guide/custom-title-bars.md)).
2. Callbacks queued with **`LoopHandle.call_soon`** run.
3. **One central tick** advances every running animation; completion
   callbacks run after it.
4. **Layout** runs through `taffy`'s cache — paint-only changes never
   dirty it — and virtual lists build their visible rows.
5. **Paint** walks the tree into a `vello_gpu` scene, culling what's off
   screen. A damage tracker compares the frame with the last one, and only
   the changed rects (at most four, or the whole window) are rendered into
   the window's persistent target, which is then copied to the screen.
   `window.set(partial_redraw=False)` redraws in full, and
   `show_damage=True` tints what was redrawn. The **AccessKit** tree is
   built from the same node tree.

When nothing changed and nothing animates, the loop sleeps until the next
input or `call_soon`.

Each turn the loop also polls the GPU device. A GPU error becomes a
`gpu_error` event instead of a panic, and a lost GPU fires `gpu_lost` and ends
the run; while GPU work is still running the loop wakes about every 100 ms to
poll, so a hang in the last frame is still seen
([GPU health](api/python/window.md#gpu-health)).

A node may carry a WGSL [shader](guide/shaders.md). Before the scene renders,
the renderer runs one pass per drawn shader node into a texture the size of
its box, and the scene draws that texture in the node's place; an effect
shader first renders the node's own subtree offscreen to read it. These passes
run only when something changed, and a window drawing an `animated` shader keeps
running like any animation.

## Design principles

- **Rust owns every frame; Python owns intent.** Python never runs during
  layout, paint, or interpolation — only inside callbacks.
- **One animation mechanism.** Every animatable value is the same
  `Animated<T>`, ticked by one pass.
- **Mechanism in the engine, meaning and look in the framework.** Tesserae Engine
  reports what it can know — pointer, focus, whether focus came from the
  keyboard — and draws only what it's told.
- **Data in, not files.** Pixels, fonts, and tree content arrive as data;
  the framework owns every format.

## History

Tesserae Engine is a from-scratch second iteration of an earlier Vulkan engine,
archived under
[`archive/`](https://github.com/mindderivative/tre/tree/main/archive) with
its lessons learned.
The GitHub project [Tesserae Rendering Engine](https://github.com/users/mindderivative/projects/3)
records the work from 0.5.0 on,
[`BUILD_TRACKER_ARCHIVE_0.4.md`](https://github.com/mindderivative/tre/blob/main/BUILD_TRACKER_ARCHIVE_0.4.md) the 0.4.x line, and
[`BUILD_TRACKER_ARCHIVE_0.3.md`](https://github.com/mindderivative/tre/blob/main/BUILD_TRACKER_ARCHIVE_0.3.md)
every earlier milestone, including the 0.3.5 program that moved Material
Design 3 and the declarative layer out to the framework.
