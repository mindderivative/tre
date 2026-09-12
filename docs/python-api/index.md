# Python API

`tre-python` binds `tre-engine` (the Rust rendering engine) directly to Python via PyO3 -- there is no intermediate C-ABI layer, no opaque-handle marshalling. Every class below is a thin wrapper over a real Rust type; every method call crosses into real engine code.

```python
import tre
```

This section is the complete API reference: every public class, method, and function `tre` exposes, with parameter types, return types, and the exceptions each call can raise. For task-oriented guides (setting up a tray icon, wiring accessibility, handling window chrome quirks per platform) see **Platform Integration**, which links back into this reference rather than repeating it.

## Reference pages

| Page | Covers |
|---|---|
| [Canvas & Shapes](canvas-and-shapes.md) | `Canvas`, `ShapeRegistry`, and the shape primitives (`Rectangle`, `Circle`, `Polygon`, `Path`, `Text`, `CustomShaded`) |
| [Fonts, Gradients & Textures](fonts-gradients-textures.md) | `Font`, `Gradient`/`GradientId`, `Texture`/`TextureFormat` -- the fill and text resources shapes reference |
| [SVG](svg.md) | `Svg`, vertex-morph animation, and SMIL keyframe parsing |
| [Text Editing](text-editing.md) | `EditableText` -- caret, selection, IME, clipboard integration |
| [Animation](animation.md) | `Clock`, `Tween`, `Easing`, `Spring`, `Timeline` |
| [Rendering](rendering.md) | `HeadlessRenderer`, `WindowedRenderer`, custom shaders, `CursorIcon` |
| [Input Events](input-events.md) | `InputEvent` and its variants, `WindowId`, `MouseButton`, `ElementState` |
| [Accessibility & Focus](accessibility-and-focus.md) | `AccessibilityNode`, `A11yBridge`, `FocusableNode`, `FocusManager` |
| [Desktop Integration](desktop-integration.md) | `Clipboard`, native file dialogs, system tray (`TrayIcon`, `Menu`) |

## Conventions that hold across the whole module

**One exception type for engine failures.** Every recoverable engine-side failure (GPU device loss, pipeline creation failure, resource exhaustion, shader compilation errors that survive validation, ...) raises `tre.TreError`, a single exception type -- not one class per failure kind. A programmer error (invalid arguments, calling a method on a torn-down object) raises the ordinary Python exception you'd expect (`ValueError`, `TypeError`, `RuntimeError`), never `TreError`.

```python
try:
    frame = renderer.render(registry)
except tre.TreError as e:
    ...  # a real, recoverable engine failure
```

**Classes marked "must stay on their constructing thread."** A few classes wrap a real OS/platform connection handle that is not safe to use from a different thread than the one that created it: `WindowedRenderer`, `Clipboard`, `A11yBridge`, `Menu`, and `TrayIcon`. Call every method on one of these from the same Python thread that constructed it. Calling from another thread raises a PyO3 panic (`... is unsendable, but sent to another thread`), not a catchable Python exception -- this is a hard constraint, not a style preference. Pure-data and pure-logic classes (`Canvas`, `ShapeRegistry`, `Tween`, `Timeline`, `FocusManager`, everything shape-shaped) have no such restriction.

**Polymorphic `fill_color`.** `Rectangle`, `Circle`, `Polygon`, and `Path` accept `int | GradientId | Texture` for `fill_color` -- a plain packed RGBA `int` (usually built with `tre.rgba8(r, g, b, a)`), a `GradientId` from `registry.create_gradient(...)`, or a `Texture` from a renderer's `create_texture(...)`. `Text`, `Svg`, and `CustomShaded` take a plain `int` only -- their real rendering paths are solid-fill-only.

**Every shape carries a transform.** `x`/`y`, `scale_x`/`scale_y` (default `1.0`), and `rotation` (radians, default `0.0`) are present on every shape primitive, mirroring the engine's own `Transform2D`. What `x`/`y` measures differs per shape -- see each shape's own page for the exact convention (a `Circle`'s `x`/`y` is its bounding box's top-left, *not* its center).

**Validation happens at `ShapeRegistry.insert_*` time, not at shape construction.** Constructing a `Rectangle(...)` never raises; passing it to `registry.insert_rectangle(...)` is where non-finite numbers, negative dimensions, and invalid `fill_color` values are caught and raise `ValueError`.