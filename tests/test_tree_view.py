"""M30 Phase 6 Step 3 (§1, §3, §5, §7): real, repeatable coverage of
`Window.add_tree_node` -- the identical real grounding `Accordion`
already established (no official M3 page, the Lists guideline's own
"expand and collapse" text), applied recursively: each row is
`Accordion`'s own header anatomy again, with real per-depth left
indentation as the one real difference "recursively" means here.
"""

from tre import Node, Window


def test_add_tree_node_non_leaf_returns_header_and_chevron():
    window = Window(width=400, height=300)
    header, chevron = window.add_tree_node(title="src/")
    assert isinstance(header, Node)
    assert isinstance(chevron, Node)


def test_add_tree_node_leaf_returns_header_and_no_chevron():
    window = Window(width=400, height=300)
    header, chevron = window.add_tree_node(title="main.rs", leaf=True)
    assert isinstance(header, Node)
    assert chevron is None


def test_a_themed_tree_node_does_not_raise():
    window = Window(width=400, height=300)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    header, chevron = window.add_tree_node(title="src/")
    assert isinstance(header, Node)
    assert isinstance(chevron, Node)


def test_a_nested_tree_node_at_a_real_depth_does_not_raise():
    window = Window(width=400, height=300)
    header, chevron = window.add_tree_node(title="lib.rs", depth=2, leaf=True)
    assert isinstance(header, Node)
    assert chevron is None


def test_expanded_true_does_not_raise():
    window = Window(width=400, height=300)
    header, chevron = window.add_tree_node(title="src/", expanded=True)
    assert isinstance(header, Node)
    assert isinstance(chevron, Node)


def test_the_header_is_a_real_independently_clickable_node():
    window = Window(width=400, height=300)
    header, _chevron = window.add_tree_node(title="src/")

    calls = []
    header.enable_interaction()
    header.set_on_click(lambda: calls.append("toggled"))
    window.click(header)
    assert calls == ["toggled"], "a real click on the header must reach its own registered handler"


def test_a_deeply_nested_header_is_still_a_real_independently_clickable_node():
    """The real point of adding indentation on top of `Accordion`'s
    own already-working header anatomy: extra left padding must not
    reintroduce a hit-test problem.
    """
    window = Window(width=400, height=300)
    header, _chevron = window.add_tree_node(title="lib.rs", depth=3, leaf=True)

    calls = []
    header.enable_interaction()
    header.set_on_click(lambda: calls.append("clicked"))
    window.click(header)
    assert calls == ["clicked"], "a real click on a deeply nested row must reach its own registered handler"


def test_the_chevron_can_be_flipped_via_animate_transform():
    window = Window(width=400, height=300)
    _header, chevron = window.add_tree_node(title="src/", expanded=False)

    assert chevron is not None
    chevron.animate("transform", (0.0, 0.0, -1.0))  # must not raise -- expand
    chevron.animate("transform", (0.0, 0.0, 1.0))  # must not raise -- collapse back


def test_a_real_tree_of_nodes_each_remain_independently_clickable():
    window = Window(width=400, height=300)
    root, _root_chevron = window.add_tree_node(title="src/", depth=0)
    child_a, _a_chevron = window.add_tree_node(title="main.rs", depth=1, leaf=True)
    child_b, _b_chevron = window.add_tree_node(title="lib.rs", depth=1, leaf=True)

    clicked = []
    for i, node in enumerate([root, child_a, child_b]):
        node.enable_interaction()
        node.set_on_click(lambda i=i: clicked.append(i))

    window.click(child_b)
    assert clicked == [2], "clicking one row must reach only its own registered handler"
