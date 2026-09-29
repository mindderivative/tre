# tre

**A GPU-rendered retained-mode UI engine for Python, written in Rust.**

`tre` gives Python the building blocks of a desktop UI — nodes, layout,
paint, animation, input, text, accessibility, and layers — and renders them
on the GPU. It has no widgets and no theme of its own: a framework built on
it, such as Tesserae, turns the blocks into buttons, dialogs, and design
systems. You write Python; the engine underneath uses:

- [`vello_gpu`](https://github.com/linebender/vello) for GPU rendering
- [`taffy`](https://github.com/DioxusLabs/taffy) for flexbox layout
- [`parley`](https://github.com/linebender/parley) for text shaping
- [`AccessKit`](https://github.com/AccessKit/accesskit) for accessibility
- [`winit`](https://github.com/rust-windowing/winit) for windows and input

```python
from tre import App, Window

window = Window(width=320, height=120, title="Hello")
button = window.create("box", width=120, height=40, corner_radius=20,
                       fill=(0x67, 0x50, 0xA4, 0xFF), role="button",
                       label="Say hello", focusable=True)
button.on("click", lambda: print("hello"))
window.root.add_child(button)

app = App()
app.add_window(window)
app.run()
```

## The building blocks

- **[Nodes and layout](guide/nodes-and-layout.md)** — boxes, text, text
  inputs, images, paths, canvases, scroll views, virtual lists, and
  terminals, laid out with flexbox or placed absolutely.
- **[Painting](guide/painting.md)** — fills, strokes, per-corner radii,
  layered shadows, group opacity, transforms, and SVG paths.
- **[Animation](guide/animation.md)** — any paint or transform property,
  eased on a cubic-bezier curve, retargetable mid-flight, with path morphing.
- **[Events and input](guide/events-and-input.md)** — bubbling listeners,
  pointer capture, keyboard focus, text editing, and the clipboard.
- **[Text](guide/text.md)** — shaped text, measurement, custom fonts, text
  inputs, and a terminal emulator.
- **[Accessibility](guide/accessibility.md)** — roles, names, states, and
  actions for screen readers.
- **[Layers](guide/layers.md)** — one mechanism for menus, dialogs, and
  tooltips: stacking, anchoring, modality, and dismissal.
- **[Threading](guide/threading.md)** — hand work from any thread to the
  event loop.
- **[Docking](guide/docking.md)** — panels in zones, dragged between them.

[Building a Widget](guide/building-a-widget.md) puts them together into a
complete switch.

## What `tre` leaves to the framework

- **Widgets and design** — Material Design 3 or any other design system is
  built from the blocks; `tre` draws no default styling, focus ring, or
  scrim.
- **Declarative views and state** — views, bindings, and reactivity sit on
  top of `window.create`, `node.set`, and `insert_child`.
- **Files** — images arrive as decoded pixels and fonts as bytes; `tre`
  reads no files and chooses no formats.

## Where to go next

- **[Installation](installation.md)** — `pip install tesserae-engine`, a released wheel, or a build from
  source.
- **[Getting Started](getting-started.md)** — a first window, step by step.
- **[Python API Reference](api/python/index.md)** — every class, method,
  property, and event.
- **[Migrating to 0.3.5](migrating-0.3.5.md)** — from 0.3.4: what moved to
  the framework and what was renamed.
- **[Architecture](architecture.md)** — the crates and how a frame is made.

`tre` is a second, from-scratch iteration of an earlier Vulkan engine,
archived under
[`archive/`](https://github.com/mindderivative/tre/tree/main/archive) with
the lessons learned that shaped it. Its full build history is in
[`BUILD_TRACKER.md`](https://github.com/mindderivative/tre/blob/main/BUILD_TRACKER.md).
