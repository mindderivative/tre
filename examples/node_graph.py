#!/usr/bin/env python3
"""M5 Phase 4's own validation charter (§11.11): "no new framework
mechanism -- both compose entirely from what's already specified."
A small, real node graph -- five circular "nodes" joined by six
stroked "edges," all drawn as real `DrawCommand`s inside one
`NodeKind::Canvas` (M5 Phase 3), with a real `CustomHitTest::Circle`
on one specific node (§11.10's own "a specific plotted data point"
example, verbatim).

Real, honest constraint this example works within, checked directly
against the current `engine-py` API before writing it (not assumed):
Python has no way yet to position a `Node` arbitrarily (`Window.
add_rect`/`add_canvas` both attach as flex-row children of the
window's own root -- no `Position::Absolute` exposed to Python) or to
animate a node's `transform` at all (`Node.animate()`'s own real
property list is `opacity`/`corner_radius`/`elevation`/`background`
only, confirmed by reading `engine-py/src/node.rs` directly). So this
example draws the whole graph -- nodes *and* edges -- as `DrawCommand`s
inside one `Canvas`, rather than each node as its own separately
click-positioned `Rect`: a real, legitimate way to build a node graph
(arguably the more realistic shape for "many nodes" per §11.11's own
culling discussion -- batching into one `Canvas` rather than one real
`Tree` node per graph node), not a workaround. The Rust-level proof
that transform composition + per-node/per-edge custom hit-testing
*also* compose when each lives as its own separately-positioned
`Tree` node (which Rust's fuller API does support today) is
`crates/engine-render/tests/graph_composition.rs`, not this script.

What this script proves automatically (headless-CI-safe, no human
needed): the graph's `draw` callback runs, every `DrawCommand`/
`CustomHitTest` call accepts real arguments, and the whole scene
renders through the real pipeline for real frames, exiting cleanly.
"""

from tre import App, Window

window = Window(width=320, height=240, title="tre v2 -- node graph")

positions = [(40, 40), (160, 30), (260, 70), (200, 160), (70, 150)]
edges = [(0, 1), (1, 2), (2, 3), (3, 4), (4, 0), (1, 3)]
node_colors = [
    (0xFF, 0xA5, 0x00, 0xFF),
    (0x03, 0xDA, 0xC6, 0xFF),
    (0xCF, 0x62, 0x79, 0xFF),
    (0x67, 0x50, 0xA4, 0xFF),
    (0x38, 0x8E, 0x3C, 0xFF),
]
NODE_RADIUS = 12


def draw(ctx):
    # Edges first, so the node circles paint on top of them.
    for a, b in edges:
        ax, ay = positions[a]
        bx, by = positions[b]
        ctx.stroke_path(points=[(ax, ay), (bx, by)], color=(0x63, 0x50, 0xA4, 0xFF), width=2.0)

    for (x, y), color in zip(positions, node_colors):
        ctx.fill_circle(cx=x, cy=y, radius=NODE_RADIUS, color=color)

    # §11.10's own "a specific plotted data point" example, verbatim --
    # a precise circular hit-test on node 2 alone; clicking anywhere
    # else in the canvas's wider bounding box misses.
    cx, cy = positions[2]
    ctx.set_hit_test_circle(cx=cx, cy=cy, radius=NODE_RADIUS)


graph = window.add_canvas(width=320, height=240, draw=draw)
window.redraw_canvas(graph)

app = App()
app.add_window(window)
app.run(max_frames=180)
print("node_graph.py: exited cleanly after 180 frames")
