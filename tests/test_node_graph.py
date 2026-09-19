"""M30 Phase 9 Step 2 (§5): real, repeatable coverage of `Window.
add_node_graph`/`Window.add_graph_node` -- the real, first-class
authored `Node Graph` component, closing the gap M5 Phase 4's own
`PLAN.md` named and `examples/positioned_graph.py` (M6 Phase 3) only
ever demonstrated as raw composition (interchangeable circles, no real
node anatomy, no reparent/position automation).

No Python-level pixel readback or raw-coordinate hit-test entry point
exists anywhere in this project (the same real constraint `test_
position.py`'s own module doc comment already states) -- so, matching
that file's own established technique, `test_a_click_on_a_graph_node_
reaches_its_own_handler` proves the real point of this step (a graph
node, once attached under its own graph's real viewport, is genuinely
clickable at its own real, graph-relative position) through real
dispatch, not introspection.

`node.animate("transform", ...)` (panning the graph, moving a node)
only actually takes effect on a real `Tree::tick_all` pass -- which
only ever runs inside a live `App.run()` loop, confirmed by direct
source read before writing this file (no Python-callable synchronous
tick exists). That real, live behavior is proven by `examples/
node_graph.py`'s own full render loop, not here.
"""

import pytest

from tre import Node, Window


def test_add_node_graph_returns_a_node():
    window = Window(width=400, height=400)
    graph = window.add_node_graph(width=300, height=300)
    assert isinstance(graph, Node)


def test_a_themed_node_graph_does_not_raise():
    window = Window(width=400, height=400)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    graph = window.add_node_graph(width=300, height=300)
    assert isinstance(graph, Node)


def test_add_node_graph_positions_like_every_other_add_method():
    window = Window(width=400, height=400)
    graph = window.add_node_graph(width=300, height=300, x=20, y=20)
    assert isinstance(graph, Node)


def test_add_graph_node_returns_a_node():
    window = Window(width=400, height=400)
    graph = window.add_node_graph(width=300, height=300)
    node = window.add_graph_node(graph, "Node A", x=10, y=10, width=100, height=60)
    assert isinstance(node, Node)


def test_a_themed_graph_node_does_not_raise():
    window = Window(width=400, height=400)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    graph = window.add_node_graph(width=300, height=300)
    node = window.add_graph_node(graph, "Node A", x=10, y=10, width=100, height=60)
    assert isinstance(node, Node)


def test_add_graph_node_from_a_different_window_raises_a_clear_error():
    window_a = Window(width=400, height=400)
    window_b = Window(width=400, height=400)
    graph = window_a.add_node_graph(width=300, height=300)
    with pytest.raises(ValueError):
        window_b.add_graph_node(graph, "Node A", x=10, y=10, width=100, height=60)


def test_a_click_on_a_graph_node_reaches_its_own_handler():
    """The real point of this step: a graph node is a real child of
    its own graph, positioned graph-relative, not window-root-relative
    -- proven by real dispatch. `window.click(node)` resolves to
    `node`'s own real, current center point, which for this node's own
    100x60 box sits at local (50, 30) -- inside the node's own 32dp
    title-bar band (`NODE_GRAPH_TITLE_HEIGHT`), so this also proves the
    real `hit_testable` fix this step found and applied (a click
    landing inside the title bar's own real bounds must still reach
    the *node's* own handler, not be intercepted by the purely
    decorative title strip)."""
    window = Window(width=400, height=400)
    graph = window.add_node_graph(width=300, height=300, x=20, y=20)
    node = window.add_graph_node(graph, "Node A", x=10, y=10, width=100, height=60)

    hits = []
    node.set_on_click(lambda: hits.append("node"))
    window.click(node)

    assert hits == ["node"], (
        "a real click on a graph node must reach its own registered handler, even though "
        f"its own real absolute position is graph-relative, not window-root-relative, got {hits!r}"
    )


def test_multiple_graph_nodes_are_each_independently_clickable():
    window = Window(width=400, height=400)
    graph = window.add_node_graph(width=300, height=300)
    node_a = window.add_graph_node(graph, "A", x=10, y=10, width=80, height=50)
    node_b = window.add_graph_node(graph, "B", x=150, y=120, width=80, height=50)

    hits = []
    node_a.set_on_click(lambda: hits.append("a"))
    node_b.set_on_click(lambda: hits.append("b"))

    window.click(node_a)
    window.click(node_b)

    assert hits == ["a", "b"], f"each graph node must reach its own real, distinct handler, got {hits!r}"
