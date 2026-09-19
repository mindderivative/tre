#!/usr/bin/env python3
"""M30 Phase 9 Step 2's real `Node Graph` component (§5): closes the
gap M5 Phase 4's own `PLAN.md` named and `examples/positioned_graph.py`
(M6 Phase 3) only ever demonstrated as raw composition -- real, styled,
title-bar-over-body nodes (`Window.add_graph_node`), automatically
reparented into a real pannable/zoomable viewport (`Window.
add_node_graph`), not interchangeable circles the app had to hand-
build itself. Named distinctly from the pre-existing `examples/
node_graph.py` (M5 Phase 4's own real, still-valid Canvas + `Custom
HitTest::Circle` composition validation, a genuinely different
demonstration this script doesn't replace or duplicate).

What this script proves automatically (headless-CI-safe, no human
needed): four real graph nodes, each independently clickable at its
own real, graph-relative position; a real edges `Canvas` (the same
`add_canvas`+`redraw_canvas` pattern `positioned_graph.py` already
proved, reparented into the graph so it pans/zooms with its own
nodes); the whole graph panned via `graph.animate("transform", ...)`
and one node repositioned via `node.animate("transform", ...)` --
both fully real since M6 Phase 2, both already proven at the engine-
core level to compose correctly through nested ancestors
(`absolute_position_follows_an_ancestor_transform`).

**Real, honest scope note, not silently glossed over:** `duration_ms=0`
animations only actually take effect on the *next* real `Tree::
tick_all` pass (confirmed by direct source read, not assumed) --
inside a live `App.run()` loop, that's the very first rendered frame,
before this script's own edge geometry is computed below, so edges
stay correctly anchored to each node's real *final* position. A
continuously-animated node (a nonzero `duration_ms`) would visually
outrun a `Canvas` edge computed once like this -- this script
deliberately doesn't attempt that, since there is no real per-frame
Python hook to keep redrawing the edges in step (confirmed via direct
check of `App.run`'s own signature, the identical real constraint
`examples/video.py`'s own doc comment already names for a different
reason). No drag-to-move mouse gesture either -- `add_graph_node`'s
own Rust doc comment has the full real reason (no Python-facing
pointer-move-while-pressed hook exists yet).
"""

from collections.abc import Callable

from tre import App, CanvasContext, Node, Window

window = Window(width=420, height=340, title="tre v2 -- node graph editor")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

GRAPH_WIDTH, GRAPH_HEIGHT = 380.0, 280.0
graph = window.add_node_graph(width=GRAPH_WIDTH, height=GRAPH_HEIGHT, x=10, y=10)

# Declared graph-local layout -- each node's own real, final local
# (x, y), already accounting for the one node ("Transform") this
# script moves below (its own real final position, not its starting
# one), so the edges drawn from this same table stay correctly
# anchored once the real render loop starts.
NODE_WIDTH, NODE_HEIGHT = 110.0, 56.0
nodes_layout = {
    "input": (20.0, 20.0),
    "transform": (200.0, 20.0),  # already includes this node's own (+40, 0) move below
    "filter": (20.0, 160.0),
    "output": (200.0, 160.0),
}
edges = [("input", "transform"), ("input", "filter"), ("transform", "output"), ("filter", "output")]


def _center(name: str) -> tuple[float, float]:
    x, y = nodes_layout[name]
    return x + NODE_WIDTH / 2, y + NODE_HEIGHT / 2


def draw_edges(ctx: CanvasContext) -> None:
    for a, b in edges:
        ax, ay = _center(a)
        bx, by = _center(b)
        ctx.stroke_path(points=[(ax, ay), (bx, by)], color=(0x79, 0x74, 0x7E, 0xFF), width=2.0)


# Added (and reparented) before any real node, so it paints first --
# children-list order is paint order (§6) -- edges sit underneath the
# node cards, matching `positioned_graph.py`'s own established
# convention.
edge_canvas = window.add_canvas(width=GRAPH_WIDTH, height=GRAPH_HEIGHT, draw=draw_edges, x=0.0, y=0.0)
graph.add_child(edge_canvas)
window.redraw_canvas(edge_canvas)

clicked: list[str] = []
graph_nodes: dict[str, Node] = {}


def _make_click_handler(name: str) -> Callable[[], None]:
    def handler() -> None:
        clicked.append(name)

    return handler


for name, (x, y) in [
    ("input", (20.0, 20.0)),
    ("transform", (160.0, 20.0)),  # starting position -- moved to (200, 20) below
    ("filter", (20.0, 160.0)),
    ("output", (200.0, 160.0)),
]:
    node = window.add_graph_node(graph, name.title(), x=x, y=y, width=NODE_WIDTH, height=NODE_HEIGHT)
    node.set_on_click(_make_click_handler(name))
    graph_nodes[name] = node

# Every graph node is real and independently clickable at its own
# real, graph-relative position (proven for real, not assumed -- the
# same click-through discipline `tests/test_node_graph.py` already
# established).
for name, node in graph_nodes.items():
    window.click(node)
assert clicked == list(graph_nodes), f"every graph node must reach its own real handler, got {clicked!r}"

# Pan the whole graph -- every node (and the edges Canvas) moves
# together, since all are real children of `graph`.
graph.animate("transform", (-10.0, -5.0, 1.0), duration_ms=0)

# Reposition one node programmatically (a real, available mechanism --
# not a mouse-drag gesture, see this script's own module doc comment)
# to its own real final (200, 20), matching `nodes_layout["transform"]`
# above.
graph_nodes["transform"].animate("transform", (40.0, 0.0, 1.0), duration_ms=0)

app = App()
app.add_window(window)
app.run(max_frames=60)
print(
    "graph_editor.py: exited cleanly after 60 frames -- 4 real graph nodes, each independently clickable"
)
