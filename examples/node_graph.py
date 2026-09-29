#!/usr/bin/env python3
"""A node graph: each graph node is a real, round box placed with
`position="absolute"`, so it has its own listeners, and one canvas behind
them draws the edges. Clicking a node selects it; the selected node's
edges redraw highlighted. Pan and zoom the whole graph by animating its
container's `translate_x`, `translate_y`, and `scale` -- hit testing
follows the transform.

The script selects a node and zooms with `window.simulate` and
`window.advance`, then opens the window. Headless-CI-safe: `App.run()`
renders `max_frames=60` and returns quietly without a display.
"""

from tre import App, Window

POSITIONS = [(40, 40), (160, 30), (260, 70), (200, 160), (70, 150)]
EDGES = [(0, 1), (1, 2), (2, 3), (3, 4), (4, 0), (1, 3)]
SIZE = 24
EDGE, HIGHLIGHT = (0xCA, 0xC4, 0xD0, 0xFF), (0x67, 0x50, 0xA4, 0xFF)
NODE, SELECTED = (0x62, 0x5B, 0x71, 0xFF), (0x67, 0x50, 0xA4, 0xFF)

window = Window(width=340, height=240, title="tre -- node graph")
window.root.set(padding=0)
graph = window.create("box", width=320, height=220)
window.root.add_child(graph)
selected = None


def draw_edges(painter):
    for a, b in EDGES:
        (ax, ay), (bx, by) = POSITIONS[a], POSITIONS[b]
        color = HIGHLIGHT if selected in (a, b) else EDGE
        painter.stroke_path([[ax + SIZE / 2, ay + SIZE / 2], [bx + SIZE / 2, by + SIZE / 2]],
                            color, 2.0)


edges = window.create("canvas", draw=draw_edges, position="absolute", x=0, y=0,
                      width=320, height=220, hit_testable=False)
graph.add_child(edges)  # added first, so the nodes paint over it


def select(index):
    global selected
    if selected is not None:
        nodes[selected].animate("fill", NODE, 150)
    selected = index
    nodes[index].animate("fill", SELECTED, 150)
    edges.redraw()


nodes = []
for i, (x, y) in enumerate(POSITIONS):
    node = window.create("box", position="absolute", x=x, y=y, width=SIZE, height=SIZE,
                         corner_radius=SIZE / 2, fill=NODE, cursor="pointer",
                         role="button", label=f"node {i}", focusable=True)
    node.on("click", lambda i=i: select(i))
    graph.add_child(node)
    nodes.append(node)

# -- checks ------------------------------------------------------------------
window.advance(0)
window.simulate("click", node=nodes[3])
window.advance(150)
assert selected == 3 and nodes[3].get("fill") == SELECTED

graph.animate("scale", 1.5, 300)  # zoom about the graph's center
window.advance(300)
window.simulate("click", node=nodes[1])  # simulate aims through the transform
assert selected == 1
print("node_graph.py: checks passed")

app = App()
app.add_window(window)
app.run(max_frames=60)
print("node_graph.py: exited cleanly")
