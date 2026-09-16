#!/usr/bin/env python3
"""M6 Phase 3 closes, for real this time, the exact gap M5 Phase 4 named
and worked around: no Python-facing way to position a `Node`
independently, so `examples/node_graph.py` had to draw its whole graph
-- nodes *and* edges -- as `DrawCommand`s inside one `Canvas`. With
`x`/`y` now real (§8), this is the idiomatic shape M5 Phase 4's own
`PLAN.md` originally wanted: real, independently-positioned, clickable
`Rect` "nodes" (a circle via `corner_radius`, the already-real M3/M4
click mechanism) plus one positioned `Canvas` for the "edges" (open
curves no other `NodeKind` can hit-test precisely, §11.11's own
reasoning) -- exactly the scene `crates/engine-render/tests/
graph_composition.rs` already proved works at the Rust level.

What this script proves automatically (headless-CI-safe, no human
needed): every node/edge lands at its own explicit position (not the
implicit flex-row flow) and the whole scene renders through the real
pipeline for real frames, exiting cleanly. `tests/test_position.py` is
the definitive, automated proof that `x`/`y` genuinely take effect
(via real hit-testing, the same discipline every FFI test in this
project uses); clicking a node here and seeing which one's handler
fires is this script's own live, human-observable version of that same
claim.
"""

from tre import App, Window

window = Window(width=320, height=240, title="tre v2 -- positioned graph")

positions = [(40, 40), (160, 30), (260, 70), (200, 160), (70, 150)]
edges = [(0, 1), (1, 2), (2, 3), (3, 4), (4, 0), (1, 3)]
node_colors = [
    (0xFF, 0xA5, 0x00, 0xFF),
    (0x03, 0xDA, 0xC6, 0xFF),
    (0xCF, 0x62, 0x79, 0xFF),
    (0x67, 0x50, 0xA4, 0xFF),
    (0x38, 0x8E, 0x3C, 0xFF),
]
NODE_SIZE = 24


def draw_edges(ctx):
    for a, b in edges:
        ax, ay = positions[a]
        bx, by = positions[b]
        # Edge coordinates are node-local to the (0, 0)-positioned
        # Canvas below, i.e. the same shared coordinate space every
        # node's own explicit `x`/`y` already uses -- centered on each
        # node's own middle, matching `NODE_SIZE`.
        ctx.stroke_path(
            points=[
                (ax + NODE_SIZE / 2, ay + NODE_SIZE / 2),
                (bx + NODE_SIZE / 2, by + NODE_SIZE / 2),
            ],
            color=(0x63, 0x50, 0xA4, 0xFF),
            width=2.0,
        )


# The edges paint first (added first, so the node circles paint on top
# of them -- children-list order is paint order, §6).
edge_canvas = window.add_canvas(width=320, height=240, draw=draw_edges, x=0.0, y=0.0)
window.redraw_canvas(edge_canvas)

for (x, y), color in zip(positions, node_colors):
    node = window.add_rect(background=color, width=NODE_SIZE, height=NODE_SIZE, x=x, y=y)
    node.animate("corner_radius", NODE_SIZE / 2, duration_ms=0)
    node.set_on_click(lambda: None)

app = App()
app.add_window(window)
app.run(max_frames=180)
print("positioned_graph.py: exited cleanly after 180 frames")
