"""0.5.10 (#253): `Window.build`, `Window.apply` and `Node.set_children`."""

import pytest

from tess_re import Window


def window():
    return Window(width=200, height=120)


def spec(n=6):
    return {
        "kind": "box",
        "id": "row",
        "props": {"flex_direction": "horizontal", "gap": 2, "width": 200, "height": 40},
        "children": [
            {
                "kind": "text",
                "id": f"t{i}",
                "props": {
                    "text": f"item {i}",
                    "font_size": 14,
                    "width": 30,
                    "height": 16,
                },
            }
            for i in range(n)
        ],
    }


def by_hand(w, n=6):
    row = w.create("box", flex_direction="horizontal", gap=2, width=200, height=40)
    for i in range(n):
        row.add_child(
            w.create("text", text=f"item {i}", font_size=14, width=30, height=16)
        )
    return row


def test_build_makes_the_tree_that_per_node_calls_make():
    a, b = window(), window()
    root, nodes = a.build(spec())
    a.root.add_child(root)
    b.root.add_child(by_hand(b))
    assert a.snapshot() == b.snapshot()
    assert set(nodes) == {"row"} | {f"t{i}" for i in range(6)}
    assert nodes["row"].get("kind") == "box" and nodes["t3"].get("text") == "item 3"
    assert [c.get("text") for c in root.children()] == [f"item {i}" for i in range(6)]


def test_a_bad_description_makes_nothing_and_names_the_place():
    w = window()
    drawn = []
    bad = spec()
    bad["children"].insert(
        0, {"kind": "canvas", "props": {"draw": lambda p: drawn.append(1)}}
    )
    bad["children"][4]["props"]["font_size"] = "big"
    with pytest.raises(ValueError, match=r"spec\.children\[4\]"):
        w.build(bad)
    assert drawn == [], "a canvas earlier in the spec was not made, so it was not drawn"
    for broken, message in [
        ({"kind": "nope"}, "unknown node kind"),
        ({"kind": "box", "extra": 1}, "unknown key"),
        ({"props": {}}, "needs `kind`"),
        ({"kind": "box", "children": [1]}, r"children\[0\]"),
        ({"kind": "text"}, "needs `text`"),
        (
            {
                "kind": "box",
                "children": [{"kind": "box", "id": "a"}, {"kind": "box", "id": "a"}],
            },
            "used twice",
        ),
        ({"kind": "box", "id": ""}, "non-empty"),
        ("box", "must be a dict"),
    ]:
        with pytest.raises(ValueError, match=message):
            w.build(broken)


def test_nesting_is_capped_and_deep_specs_do_not_recurse():
    w = window()
    deep = {"kind": "box"}
    for _ in range(200):
        deep = {"kind": "box", "children": [deep]}
    root, _ = w.build(deep)
    assert root.get("kind") == "box"
    too_deep = {"kind": "box"}
    for _ in range(300):
        too_deep = {"kind": "box", "children": [too_deep]}
    with pytest.raises(ValueError, match="deep"):
        w.build(too_deep)


def test_apply_changes_many_nodes_or_none():
    w = window()
    root, nodes = w.build(spec(3))
    a, b = nodes["t0"], nodes["t1"]
    w.apply([(a, {"text": "A"}), (b, {"text": "B"}), (a, {"text": "A2"})])
    assert (a.get("text"), b.get("text")) == ("A2", "B")
    with pytest.raises(ValueError, match=r"changes\[1\]"):
        w.apply([(a, {"text": "never"}), (b, {"font_size": "big"})])
    assert a.get("text") == "A2", "an earlier entry was not applied"
    with pytest.raises(ValueError, match="another window"):
        w.apply([(window().create("box"), {"opacity": 0.5})])
    with pytest.raises(ValueError, match=r"changes\[0\]"):
        w.apply([a])  # type: ignore[list-item]


def test_set_children_replaces_orders_and_detaches():
    w = window()
    root = w.create("box")
    kids = [w.create("box", width=i + 1) for i in range(4)]
    for k in kids[:3]:
        root.add_child(k)
    elsewhere = w.create("box")
    elsewhere.add_child(kids[3])
    root.set_children([kids[2], kids[3], kids[0]])
    assert [c.get("width") for c in root.children()] == [3, 4, 1]
    assert elsewhere.children() == []
    assert kids[1].parent() is None
    root.set_children([])
    assert root.children() == []


def test_set_children_rejects_bad_lists_without_changing_anything():
    w = window()
    root, other = w.create("box"), w.create("box")
    a, b = w.create("box"), w.create("box")
    root.set_children([a, b])
    child = w.create("box")
    a.add_child(child)
    for bad, message in [
        ([a, a], "twice"),
        ([root], "itself"),
        ([window().create("box")], "different Window"),
        ([1], "must be a node"),
    ]:
        with pytest.raises(ValueError, match=message):
            root.set_children(bad)
    with pytest.raises(ValueError):
        child.set_children([root])
    assert [c.get("kind") for c in root.children()] == ["box", "box"]
    assert len(root.children()) == 2 and len(a.children()) == 1


def test_the_depth_limit_is_exact_and_children_may_be_a_tuple():
    w = window()

    def nest(levels):
        node = {"kind": "box"}
        for _ in range(levels):
            node = {"kind": "box", "children": (node,)}
        return node

    root, _ = w.build(nest(256))
    assert root.get("kind") == "box"
    with pytest.raises(ValueError, match="deep"):
        w.build(nest(257))


def test_a_draw_that_raises_while_building_keeps_its_own_error():
    w = window()

    def boom(painter):
        raise ZeroDivisionError("draw")

    with pytest.raises(ZeroDivisionError):
        w.build(
            {
                "kind": "box",
                "children": [
                    {"kind": "box"},
                    {"kind": "canvas", "props": {"draw": boom}},
                ],
            }
        )


def test_apply_allows_one_shader_entry_only():
    w = window()
    a, b = w.create("box"), w.create("box")
    shader = __import__("tess_re").Shader(
        "fn shade(p: Pixel) -> vec4<f32> { return vec4<f32>(1.0, 0.0, 0.0, 1.0); }"
    )
    with pytest.raises(ValueError, match="only one entry"):
        w.apply([(a, {"shader": shader}), (b, {"shader": shader})])
    w.apply([(a, {"shader": shader}), (b, {"opacity": 0.5})])


def test_set_children_frees_a_dropped_child_whose_child_it_keeps_and_refuses_layers():
    w = window()
    root = w.create("box")
    dropped = w.create("box")
    kept = w.create("box", width=7)
    root.add_child(dropped)
    dropped.add_child(kept)
    del dropped
    root.set_children([kept])
    assert [c.get("width") for c in root.children()] == [7]
    menu = w.create("box")
    w.root.add_child(root)
    w.show_layer(menu, root)
    with pytest.raises(ValueError, match="open layer"):
        w.root.set_children([menu])
