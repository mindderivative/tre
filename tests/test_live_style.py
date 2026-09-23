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


# --- M59 (§5, §16.3): set_layout widened -- per-side padding/margin, ---
# flex-grow/shrink/basis, align-items/justify-content. Same real "no
# pixel-box readback, prove the FFI call succeeds" limit as above.


def test_set_layout_accepts_per_side_padding_and_margin_individually():
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    node.set_layout(padding_top=4.0)
    node.set_layout(padding_right=4.0)
    node.set_layout(padding_bottom=4.0)
    node.set_layout(padding_left=4.0)
    node.set_layout(margin=2.0)
    node.set_layout(margin_top=1.0)
    node.set_layout(margin_right=1.0)
    node.set_layout(margin_bottom=1.0)
    node.set_layout(margin_left=1.0)


def test_set_layout_per_side_padding_layers_on_top_of_the_uniform_value():
    """A per-side kwarg given alongside the uniform `padding=`/`margin=`
    must not raise -- the real, documented "per-side always wins for
    that one side" contract, exercised here for both at once (this
    file's own established honest limit means the actual per-side
    override can't be read back and asserted numerically, only proven
    not to raise).
    """
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    node.set_layout(padding=8.0, padding_top=2.0, margin=4.0, margin_left=1.0)


def test_set_layout_accepts_flex_grow_shrink_and_basis():
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    node.set_layout(flex_grow=1.0, flex_shrink=0.0, flex_basis=40.0)


@pytest.mark.parametrize(
    "value",
    ["start", "end", "flex_start", "flex_end", "center", "baseline", "stretch"],
)
def test_set_layout_accepts_every_real_align_items_value(value):
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    node.set_layout(align_items=value)


@pytest.mark.parametrize(
    "value",
    [
        "start",
        "end",
        "flex_start",
        "flex_end",
        "center",
        "stretch",
        "space_between",
        "space_around",
        "space_evenly",
    ],
)
def test_set_layout_accepts_every_real_justify_content_value(value):
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    node.set_layout(justify_content=value)


def test_set_layout_rejects_an_unknown_align_items_value():
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    with pytest.raises(ValueError, match="align_items"):
        node.set_layout(align_items="sideways")


def test_set_layout_rejects_an_unknown_justify_content_value():
    window = Window(width=200, height=200)
    node = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    with pytest.raises(ValueError, match="justify_content"):
        node.set_layout(justify_content="sideways")


# --- M59 (§5, §16.3): the new engine-spec fields reach a real View too -


def test_declarative_view_parses_per_side_padding_margin_and_flex_align(tmp_path):
    """The real declarative-path counterpart -- `StyleSpec`'s own new
    fields (`engine-spec`) must reach a genuinely built `View` without
    raising, the same honest "no pixel-box readback" limit as the
    imperative path above.
    """
    path = write_view(
        tmp_path,
        """
id: root
kind: Container
style:
  width: 200
  height: 100
  padding: {top: 4, right: 8, bottom: 4, left: 8}
  margin: 2
  flex_grow: 1
  flex_shrink: 0
  flex_basis: 40
  align_items: Center
  justify_content: SpaceBetween
""",
    )
    view = View(path)
    node = view.node("root")
    assert isinstance(node, Node)


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


# --- border at construction (M60: widened to every other Rect-backed
# factory in the catalog, mirroring add_rect's own exact kwarg pair) -----


def test_add_card_accepts_border_kwargs():
    # A simple single-`Node`-returning factory -- `add_card`'s own
    # `PaintProperties` is already themed with a (usually zero-width)
    # border from `resolve_card_colors`, so this also proves the new
    # kwargs win over that pre-existing theme-derived value, the same
    # "explicit literal always wins" contract `add_rect` establishes.
    window = Window(width=300, height=300)
    node = window.add_card(
        width=120,
        height=80,
        border_color=(10, 20, 30, 255),
        border_width=2.0,
    )
    assert isinstance(node, Node)
    assert node.get("border_width") == pytest.approx(2.0)


def test_add_chip_accepts_border_kwargs():
    window = Window(width=300, height=300)
    node = window.add_chip(
        label="Filter",
        width=100,
        border_color=(0, 0, 0, 255),
        border_width=1.0,
    )
    assert isinstance(node, Node)
    assert node.get("border_width") == pytest.approx(1.0)


def test_add_badge_accepts_border_kwargs_for_both_the_dot_and_labeled_shapes():
    window = Window(width=200, height=200)
    dot = window.add_badge(border_color=(255, 0, 0, 255), border_width=1.0)
    labeled = window.add_badge(
        label="9+", border_color=(255, 0, 0, 255), border_width=1.0
    )
    assert dot.get("border_width") == pytest.approx(1.0)
    assert labeled.get("border_width") == pytest.approx(1.0)


def test_add_snackbar_accepts_border_kwargs_and_styles_only_the_returned_container():
    # `add_snackbar` returns a `(container, action, close)` tuple -- by
    # this catalog's own "first tuple element is the primary node"
    # convention, the border kwargs style `container` only.
    window = Window(width=300, height=300)
    container, action, close = window.add_snackbar(
        text="Saved",
        width=250,
        action_label="Undo",
        closable=True,
        border_color=(255, 255, 255, 255),
        border_width=1.0,
    )
    assert isinstance(container, Node)
    assert container.get("border_width") == pytest.approx(1.0)
    assert action is not None
    assert close is not None


def test_add_tabs_accepts_border_kwargs_and_applies_uniformly_to_every_tab():
    # `add_tabs` returns a `Vec<Node>` of peer tab containers (no single
    # "primary" one distinguishable the way a tuple's first element is),
    # so the border kwargs apply uniformly to every returned tab.
    window = Window(width=300, height=300)
    tabs = window.add_tabs(labels=["One", "Two", "Three"], border_color=(1, 2, 3, 255), border_width=1.5)
    assert len(tabs) == 3
    for tab in tabs:
        assert tab.get("border_width") == pytest.approx(1.5)


def test_add_pagination_accepts_border_kwargs_and_styles_only_previous():
    # `add_pagination` returns `(previous, pages, next)` -- the border
    # kwargs style `previous` (the first/primary tuple element) only;
    # `next` is deliberately left un-bordered, the same literal "first
    # tuple element" convention `add_snackbar`'s own test above proves.
    window = Window(width=300, height=300)
    previous, pages, next_ = window.add_pagination(
        page_count=3,
        border_color=(9, 9, 9, 255),
        border_width=1.0,
    )
    assert previous.get("border_width") == pytest.approx(1.0)
    assert next_.get("border_width") == pytest.approx(0.0)
    assert len(pages) == 3


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


# --- M62 Phase 1 (§7.1, §16.3): real line_height support -------------------


def test_add_text_accepts_a_line_height_kwarg():
    # No Python-level getter exists for a Text node's own real per-line
    # advance (`font_size`/`font_weight` have the identical honest
    # limitation, confirmed via grep -- `Node.get` supports neither) --
    # this proves the real FFI call succeeds with a real, non-default
    # value, the same "fire-and-forget" bar `set_layout`'s own tests
    # above already establish. `crates/engine-render/src/text.rs`'s own
    # `a_larger_line_height_genuinely_widens_the_real_per_line_advance`
    # is where the actual geometry change is proven, at the Rust layer.
    window = Window(width=200, height=200)
    node = window.add_text(
        content="Hello",
        background=(0, 0, 0, 0),
        width=100,
        height=40,
        line_height=1.5,
    )
    assert isinstance(node, Node)


def test_add_text_line_height_defaults_to_none():
    # The pre-M62 implicit behavior (the font's own natural metrics)
    # must still be reachable with zero change to an existing call --
    # must not raise.
    window = Window(width=200, height=200)
    node = window.add_text(content="Hello", background=(0, 0, 0, 0), width=100, height=40)
    assert isinstance(node, Node)


# --- M62 Phase 4 (§7.1, §16.3): add_text's real typography_role parity ----


def test_add_text_accepts_a_typography_role():
    # No Python-level getter exists for a Text node's own real font
    # properties (the identical honest limitation `line_height`'s own
    # tests above already state) -- proves the real FFI call succeeds
    # with a real MD3 role name. `crates/engine-spec/src/build.rs`'s own
    # `text_role_resolves_every_field_to_the_real_named_type_style` is
    # where the actual field-by-field resolution is proven, for the
    # declarative surface -- the real mechanism both surfaces share.
    window = Window(width=200, height=200)
    node = window.add_text(
        content="Heading",
        background=(0, 0, 0, 0),
        width=200,
        height=40,
        typography_role="headline_small",
    )
    assert isinstance(node, Node)


def test_add_text_typography_role_can_be_overridden_by_a_literal_field():
    window = Window(width=200, height=200)
    node = window.add_text(
        content="Heading",
        background=(0, 0, 0, 0),
        width=200,
        height=40,
        typography_role="headline_small",
        font_size=30.0,
    )
    assert isinstance(node, Node)


def test_add_text_unknown_typography_role_raises_value_error():
    window = Window(width=200, height=200)
    with pytest.raises(ValueError, match="typography_role"):
        window.add_text(
            content="Heading",
            background=(0, 0, 0, 0),
            width=200,
            height=40,
            typography_role="subtitle_huge",
        )
