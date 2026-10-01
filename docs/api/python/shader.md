# Shader

*New in 0.5.1.* A `Shader` is WGSL that paints a node. This page covers the
object and the `shader` property; the design, and what is still to come, is
[WGSL shaders](../../design/wgsl.md).

!!! note "Not drawn yet"
    In this milestone a shader is created, checked, and stored on a node, but
    the renderer does not draw it yet. Everything below is the API that
    drawing will use.

```python
import tre

glow = tre.Shader(
    """
    fn shade(p: Pixel) -> vec4<f32> {
        let d = distance(p.uv, vec2<f32>(0.5, 0.5));
        return vec4<f32>(u.tint.rgb * (1.0 - d), 1.0);
    }
    """,
    uniforms={"tint": (1.0, 0.6, 0.2, 1.0)},
)
panel.set(shader=glow)
```

You write one function, `fn shade(p: Pixel) -> vec4<f32>`. `p.uv` is the
position in the node, 0 to 1, with the origin at the top left; `p.px` is the
same in pixels. Your uniforms are the fields of `u`, and `frame.size` and
`frame.time` are the node's size in pixels and the seconds since the window
opened. The result is straight-alpha RGBA.

## `Shader(wgsl, uniforms=None, inputs=None, mode="fill", animated=False)`

| Argument | |
| --- | --- |
| `wgsl` | the source. It must define `shade`, and no entry points of its own |
| `uniforms` | `dict[str, float | tuple]` — a number is an `f32`; a tuple of 2, 3 or 4 numbers is a `vec2`, `vec3` or `vec4`. Names must be WGSL identifiers that are not reserved |
| `inputs` | `dict[str, Node]` — nodes of one window, read as `input_<name>(uv)` |
| `mode` | `"fill"` paints the node's box behind its content; `"effect"` transforms the node's own rendered content, read as `content(uv)` |
| `animated` | `True` redraws the node every frame; otherwise it redraws only when something changes |

Read-only properties: `wgsl`, `mode`, `animated`, `uniforms` (a new dict), and
`inputs` (a new dict of the same `Node`s).

**`set(*, uniforms)`** replaces the uniforms all at once. A mistake raises and
changes nothing. A `Shader` is shared: give it to several nodes and one `set`
updates them all.

`==` compares identity, so `node.get("shader") == shader` after
`node.set(shader=shader)`.

## Errors

A mistake in the source, a missing or misshapen `shade`, or a bad name raises
**`tre.ShaderError`**, a `ValueError`, when the shader is created — no GPU
needed. Its `line`, `column` and `source_line` place the problem in *your*
source, not tre's generated code; each is `None` when the compiler gives the
problem no position (a type mismatch, say). `str(error)` prints the line and a
caret under the column.

Wrong types (`uniforms={"a": "x"}`, `True`, a tuple of 5) raise `TypeError` or
`ValueError`; a name tre provides (`Pixel`, `frame`, `u`, `content`, …) used in
your source is reported with the list of names tre provides.

## The `shader` property

Every kind of node takes `shader`: `node.set(shader=shader)`,
`window.create(kind, shader=shader)`, `node.get("shader")`. `None` clears it.
Every input node must belong to the same window as the node, or `set` raises
and changes nothing.
