#!/usr/bin/env python3
"""Shining text (0.5.1): an *effect* shader on a text node. The shader gets
the node's own rendered pixels as `content(uv)` and returns what is drawn in
their place; here, a band of light that sweeps across the glyphs.

The WGSL lives in `shaders/shine.wgsl` -- where a shader's source comes from
is the app's business, not `tre`'s. `animated=True` redraws the node every
frame (`frame.time` changes); a shader that doesn't read the time would
leave `animated` off and cost nothing while the window is still.

Self-checking: the shader is validated when it is created (a mistake would
raise `tre.ShaderError` here, with the line and column in the .wgsl file),
and the checks below run before the window opens. Headless-CI-safe: it
renders `max_frames=180` and exits.
"""

from pathlib import Path

from tre import App, Shader, Window

SOURCE = (Path(__file__).parent / "shaders" / "shine.wgsl").read_text()

window = Window(width=520, height=200, title="tre -- shader shine")
window.root.set(fill=(0x16, 0x18, 0x24, 0xFF), align_items="center", justify_content="center")

title = window.create(
    "text",
    text="SHINE",
    font_size=96,
    font_weight=800,
    fill=(0xE8, 0xB4, 0x4C, 0xFF),
    padding=12,
)
window.root.add_child(title)

shine = Shader(
    SOURCE,
    uniforms={"speed": 0.35, "glow": (0.9, 0.85, 0.6)},
    mode="effect",
    animated=True,
)
title.set(shader=shine)

# What the node holds, and that a uniform can be changed live.
assert title.get("shader") == shine
assert shine.mode == "effect" and shine.animated
assert shine.uniforms["speed"] == 0.35 or abs(shine.uniforms["speed"] - 0.35) < 1e-6
shine.set(uniforms={"speed": 0.5, "glow": (1.0, 0.9, 0.6)})
assert abs(shine.uniforms["speed"] - 0.5) < 1e-6

app = App()
app.add_window(window)
app.run(max_frames=180)
print("shader_shine.py: exited cleanly after 180 frames")
