# Shaders

*New in 0.5.1.* A node can be painted by your own WGSL: a fractal behind a
panel, shining text, a colour grade over a picture. You write one function,
give it to a node, and tre does the rest. The details of the object and its
arguments are in the [Shader reference](../reference/shader.md); this page is
how to use it.

## Two kinds of shader

A **fill** paints the node's box itself, behind the node's own paint. Use it
when the pixels come from maths: gradients, noise, a fractal.

An **effect** receives what the node has already painted, its own paint and
all its descendants, as `content(uv)`, and returns what is drawn in its place.
Use it when you want to change something that exists: a shine over text, a
blur over a panel, a grade over children.

```python
glow = tre.Shader(open("shine.wgsl").read(),
                  uniforms={"speed": 0.35, "glow": (0.9, 0.85, 0.6)},
                  mode="effect", animated=True)
title.set(shader=glow)
```

[`examples/shader_shine.py`](https://github.com/mindderivative/tre/blob/main/examples/shader_shine.py)
(an effect over text) and
[`examples/shader_panel.py`](https://github.com/mindderivative/tre/blob/main/examples/shader_panel.py)
(a fill behind a panel's children) are runnable, with their WGSL in
[`examples/shaders/`](https://github.com/mindderivative/tre/tree/main/examples/shaders).

## What you write

One function, and nothing else:

```wgsl
fn shade(p: Pixel) -> vec4<f32> {
    return vec4<f32>(p.uv, 0.0, 1.0);
}
```

- `p.uv` is the position in the node, 0 to 1, from the top left. `p.px` is the
  same in pixels.
- `frame.size` is the node's size in pixels; `frame.time` is the seconds since
  the window opened.
- Your uniforms are the fields of `u`: `u.speed`, `u.tint.rgb`.
- In an effect, `content(uv)` reads the node's own pixels. With `inputs`,
  `input_<name>(uv)` reads another node.
- You return straight-alpha RGBA. tre handles the rest, including blending
  over what is behind.

You never write a vertex shader, a binding, or an entry point, so tre can
change how it runs shaders without breaking yours.

## Uniforms

`uniforms={"zoom": 1.5, "tint": (1.0, 0.5, 0.2, 1.0)}`: a number is an `f32`,
and a tuple of 2, 3 or 4 numbers is a `vec2`, `vec3` or `vec4`. Change values
with `shader.set(uniforms=...)`: every node using the shader updates, and only
the node's box is repainted. Changing a name or a type (or removing a uniform
the source still uses) is checked against the source first, and a mistake
raises and changes nothing.

## Mistakes

A shader is checked when it is created, with no GPU and no window:

```python
try:
    tre.Shader("fn shade(p: Pixel) -> vec4<f32> {\n    return vec4<f32>(1.0, ;\n}\n")
except tre.ShaderError as error:
    print(error.line, error.column)   # 2, 27 -- in *your* source
    print(error)                      # the message, the line, and a caret
```

`line`, `column` and `source_line` refer to the WGSL you wrote, not tre's
generated code. Some mistakes (a type mismatch) have no position and say what
is wrong in the message. Where the source comes from, and checking the data you
feed a shader, are your framework's job: tre reports only this.

A problem only the GPU finds is logged once, and the node paints as if it had
no shader; it never stops the window. A GPU that hangs or is lost is reported
for every node, not just shaders: see
[GPU health](../reference/window.md#gpu-health).

## Animation

A shader that reads `frame.time` needs `animated=True`: the node is repainted
every frame, and the window keeps drawing frames while one is on screen, like
any animation. Only the node's box is repainted. A shader that doesn't read
the time should leave `animated` off: it then costs nothing while the window is
still, and repaints only when a uniform, an input, or the node's size changes.

A hidden node, a fully transparent one, and one scrolled out of view run no
shader and keep the window asleep. `frame.time` is a single-precision number of
seconds, so its steps grow with the window's age (about 8 ms after a day):
fine for hours of motion, and `sin` or `fract` of it keeps repeating motion
tidy, but not for a window left open for weeks.

## Effects in detail

An effect's content is the node's **box**: a shadow or children that overflow
it are not part of the content or the result. The result replaces the node and
its subtree, with the node's own transform and opacity applied afterwards, so a
translated, half-opaque effect behaves as the node would. The node stays where
it was laid out, so clicks, focus and accessibility are unchanged; its
descendants still receive input while an effect draws over them.

An effect inside an effect works (the inner one runs first), and so does a fill
shader on a descendant. Any change in the subtree repaints the effect's whole
box, since a blur spreads it, and re-renders the content once. A change to the
shader's own uniforms reuses the content.

## Inputs

`inputs={"photo": node}` lets the shader read another node: an `image` or
`video` node is its pixels (a new video frame repaints the shader), and any
other node must have a shader, whose output is read. See
[Inputs](../reference/shader.md#inputs).

## What it costs

One texture the size of the node's box and one render pass per shader node,
run only when something changed. An effect adds a second texture and an
offscreen render of its subtree, redone only when the subtree changes. Many
shader nodes cost many passes, so a hundred small animated tiles are a hundred
passes a frame; one big shader over a big node is usually cheaper than many.
tre does not cap the number of shader nodes.

## Trust

A shader is code that runs on the GPU. tre takes shaders from Python only:
there is no way to create one from a view file or other data, so a shader is as
trusted as the Python that creates it. If your framework builds shaders from
user-supplied text, treat that text as code: a shader can loop for a long time.
tre reports a hung or lost GPU (and can warn about a stalled frame with
`window.set(gpu_watchdog=seconds)`), but it cannot stop a shader that is
running. Don't build shaders from untrusted input.
