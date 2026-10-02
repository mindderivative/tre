#!/usr/bin/env python3
"""A fractal behind a panel (0.5.1): a *fill* shader paints a node's box
itself, behind the node's own paint, and its children lay out and draw on
top as usual. Here an animated Julia set (`shaders/julia.wgsl`) fills a
rounded panel, with an ordinary label and button-like box over it.

A fill shader is clipped to the node's rounded corners. A `fill` with some
transparency on the same node would tint the shader beneath it.

Self-checking: the shader is validated when it is created, a bad one is
shown to raise `tre.ShaderError`, and the checks run before the window
opens. Headless-CI-safe: it renders `max_frames=180` and exits.
"""

from pathlib import Path

import tre
from tre import App, Shader, Window

SOURCE = (Path(__file__).parent / "shaders" / "julia.wgsl").read_text()

# A mistake in the WGSL is reported when the shader is created, positioned in
# the source -- no GPU, no window.
try:
    Shader("fn shade(p: Pixel) -> vec4<f32> {\n    return vec4<f32>(1.0, ;\n}\n")
except tre.ShaderError as error:
    assert error.line == 2, error
else:
    raise AssertionError("a broken shader should not be accepted")

window = Window(width=560, height=360, title="tre -- shader panel")
window.root.set(
    fill=(0x10, 0x12, 0x1C, 0xFF), align_items="center", justify_content="center", padding=24
)

julia = Shader(
    SOURCE,
    uniforms={"zoom": 1.2, "speed": 0.25, "inside": (0.02, 0.02, 0.08, 1.0)},
    mode="fill",
    animated=True,
)
panel = window.create(
    "box",
    width=480,
    height=300,
    corner_radius=28,
    shader=julia,
    flex_direction="vertical",
    align_items="center",
    justify_content="flex_end",
    padding=20,
)
window.root.add_child(panel)

label = window.create(
    "text", text="Julia set, drawn by a fill shader", font_size=22, fill=(255, 255, 255, 255)
)
panel.add_child(label)

assert panel.get("shader") == julia and julia.mode == "fill"

app = App()
app.add_window(window)
app.run(max_frames=180)
print("shader_panel.py: exited cleanly after 180 frames")
