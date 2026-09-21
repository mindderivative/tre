"""M48 (§5, §7, §11): real, repeatable coverage of the general live
property-exposure work this milestone adds -- `Node.set_layout`
(width/height/padding/gap, live, post-construction), `border_color`/
`border_width` reachable from `animate()`/`get()`/YAML bindings/
`Window.add_rect`'s own constructor kwargs. Before this milestone none
of this was reachable at all (confirmed via grep before writing any
code -- see `BUILD_TRACKER.md`'s M48 section for the full audit).

`Node.set_layout` has no Python-facing pixel-box readback to assert
against -- confirmed via grep, the same real, stated limit `test_resize
.py`'s own docstring already establishes for `Window.resize` ("No
Python-level getter exists for a node's own real pixel box"). These
tests prove the real FFI call succeeds (fire-and-forget, §8's own
established `animate()` precedent) rather than asserting on exact
pixel dimensions; `Tree::set_layout_style` itself (the one real
primitive both `set_layout` and the pre-existing `resize_terminal`
share) already has direct Rust-level geometry coverage in `engine-core`
-- this file is the pyo3-facing behavior, per this project's own
standing "no new engine-core logic here, so pytest is the right layer"
discipline.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_engine_py.py`.
"""

import pytest

from tre import Node, Signal, View, ViewModel, Window


def write_view(tmp_path, yaml):
    path = tmp_path / "view.yaml"
    path.write_text(yaml)
    return str(path)


# --- Node.set_layout ---------------------------------------------------


def test_set_layout_with_no_arguments_does_not_raise():
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    node.set_layout()


def test_set_layout_accepts_each_field_individually():
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    node.set_layout(width=120.0)
    node.set_layout(height=80.0)
    node.set_layout(padding=8.0)
    node.set_layout(gap=4.0)


def test_set_layout_accepts_all_fields_together():
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    node.set_layout(width=100.0, height=60.0, padding=6.0, gap=2.0)


def test_set_layout_can_be_called_repeatedly():
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    node.set_layout(width=100.0)
    node.set_layout(width=150.0)
    node.set_layout(height=90.0)


# --- border via animate()/get() -----------------------------------------


def test_animate_accepts_border_color_and_border_width():
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    node.animate("border_color", (255, 0, 0, 255), duration_ms=0)
    node.animate("border_width", 2.0, duration_ms=0)


def test_animate_border_width_with_zero_duration_does_not_raise():
    # `animate(property, value, duration_ms=0)` only *registers* the
    # target -- it snaps on the next tick, which a plain `Window`-
    # created node (no running render loop in this test) never gets.
    # `get()` right afterward would still read the pre-animation value,
    # the same real, documented `animate()`/`get()` contract every
    # sibling test in this suite already treats as "must not raise"
    # rather than asserting a readback (`test_engine_py.py`'s own
    # `test_animate_defaults_duration_to_an_instant_snap`, etc.) -- only
    # `View`'s own binding path ticks eagerly (`test_border_width_
    # binding_applies_its_initial_value`, below), which is where a real
    # readback assertion belongs.
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    node.animate("border_width", 3.5, duration_ms=0)


def test_border_width_defaults_to_zero():
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    assert node.get("border_width") == pytest.approx(0.0)


def test_border_color_requires_a_four_tuple_not_a_float():
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    with pytest.raises(TypeError, match="expects an \\(r, g, b, a\\) tuple"):
        node.animate("border_color", 1.0)


# --- border at construction (Window.add_rect) ---------------------------


def test_add_rect_accepts_border_kwargs():
    window = Window(width=200, height=200)
    node = window.add_rect(
        background=(0, 0, 0, 255),
        width=50,
        height=50,
        border_color=(255, 255, 255, 255),
        border_width=1.5,
    )
    assert isinstance(node, Node)
    assert node.get("border_width") == pytest.approx(1.5)


def test_add_rect_without_border_kwargs_still_defaults_to_zero_width():
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    assert node.get("border_width") == pytest.approx(0.0)


# --- border via static YAML (StyleSpec) ----------------------------------


def test_style_spec_parses_border_width_and_color(tmp_path):
    path = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 10, height: 10, background: "#112233", border_width: 2.0, border_color: "#ffffff"}
""",
    )
    view = View(path)
    node = view.node("root")
    assert node.get("border_width") == pytest.approx(2.0)


def test_style_spec_with_an_invalid_border_color_raises(tmp_path):
    yaml = """
id: root
kind: Rect
style: {width: 10, height: 10, background: "#112233", border_color: "not-a-real-color"}
"""
    path = tmp_path / "view.yaml"
    path.write_text(yaml)
    with pytest.raises(ValueError):
        View(str(path))


# --- border/layout via {{ }} bindings ------------------------------------


def test_border_width_binding_applies_its_initial_value(tmp_path):
    path = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 10, height: 10, background: "#112233", border_width: 0.0}
bindings: {border_width: "{{ thickness.get() }}"}
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.thickness = Signal(4.0)
            super().__init__(view)

    VM(view)
    node = view.node("root")
    assert node.get("border_width") == pytest.approx(4.0)


def test_border_color_binding_parses_a_bound_hex_string(tmp_path):
    path = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 10, height: 10, background: "#112233"}
bindings: {border_color: "{{ hue.get() }}"}
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.hue = Signal("#ff0000")
            super().__init__(view)

    # Must not raise -- the same real hex-color-string dispatch
    # "background" bindings already establish (M44), widened to
    # "border_color" this milestone.
    VM(view)


@pytest.mark.parametrize("property_name", ["width", "height", "padding", "gap"])
def test_layout_property_binding_does_not_raise(tmp_path, property_name):
    path = write_view(
        tmp_path,
        f"""
id: root
kind: Rect
style: {{width: 10, height: 10, background: "#112233"}}
bindings: {{{property_name}: "{{{{ size.get() }}}}"}}
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.size = Signal(25.0)
            super().__init__(view)

    VM(view)


def test_layout_property_binding_rejects_a_non_numeric_value(tmp_path):
    path = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 10, height: 10, background: "#112233"}
bindings: {width: "{{ label.get() }}"}
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.label = Signal("not a number")
            super().__init__(view)

    with pytest.raises(ValueError, match="expects a numeric binding"):
        VM(view)
