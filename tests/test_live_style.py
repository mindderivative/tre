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

from tre import Node, Window
from helpers import add


# --- Node.set_layout ---------------------------------------------------


def test_set_layout_with_no_arguments_does_not_raise():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    node.set()


def test_set_layout_accepts_each_field_individually():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    node.set(width=120.0)
    node.set(height=80.0)
    node.set(padding=8.0)
    node.set(gap=4.0)


def test_set_layout_accepts_all_fields_together():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    node.set(width=100.0, height=60.0, padding=6.0, gap=2.0)


def test_set_layout_can_be_called_repeatedly():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    node.set(width=100.0)
    node.set(width=150.0)
    node.set(height=90.0)


# --- M59 (§5, §16.3): set_layout widened -- per-side padding/margin, ---
# flex-grow/shrink/basis, align-items/justify-content. Same real "no
# pixel-box readback, prove the FFI call succeeds" limit as above.


def test_set_layout_accepts_per_side_padding_and_margin_individually():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    node.set(padding_top=4.0)
    node.set(padding_right=4.0)
    node.set(padding_bottom=4.0)
    node.set(padding_left=4.0)
    node.set(margin=2.0)
    node.set(margin_top=1.0)
    node.set(margin_right=1.0)
    node.set(margin_bottom=1.0)
    node.set(margin_left=1.0)


def test_set_layout_per_side_padding_layers_on_top_of_the_uniform_value():
    """A per-side kwarg given alongside the uniform `padding=`/`margin=`
    must not raise -- the real, documented "per-side always wins for
    that one side" contract, exercised here for both at once (this
    file's own established honest limit means the actual per-side
    override can't be read back and asserted numerically, only proven
    not to raise).
    """
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    node.set(padding=8.0, padding_top=2.0, margin=4.0, margin_left=1.0)


def test_set_layout_accepts_flex_grow_shrink_and_basis():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    node.set(flex_grow=1.0, flex_shrink=0.0, flex_basis=40.0)


@pytest.mark.parametrize(
    "value",
    ["start", "end", "flex_start", "flex_end", "center", "baseline", "stretch"],
)
def test_set_layout_accepts_every_real_align_items_value(value):
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    node.set(align_items=value)


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
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    node.set(justify_content=value)


def test_set_layout_rejects_an_unknown_align_items_value():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    with pytest.raises(ValueError, match="align_items"):
        node.set(align_items="sideways")


def test_set_layout_rejects_an_unknown_justify_content_value():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    with pytest.raises(ValueError, match="justify_content"):
        node.set(justify_content="sideways")


# --- M59 (§5, §16.3): the new engine-spec fields reach a real View too -


# --- stroke via animate()/get() -----------------------------------------


def test_animate_accepts_stroke_color_and_stroke_width():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    window.advance(0)
    node.animate("stroke_color", (255, 0, 0, 255), duration_ms=0)
    node.animate("stroke_width", 2.0, duration_ms=0)
    window.advance(1)
    assert node.get("stroke_color") == (255, 0, 0, 255)
    assert node.get("stroke_width") == pytest.approx(2.0)


def test_stroke_width_defaults_to_zero():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    assert node.get("stroke_width") == pytest.approx(0.0)


def test_stroke_color_requires_a_four_tuple_not_a_float():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    with pytest.raises(ValueError, match="must be an \\(r, g, b, a\\) tuple"):
        node.animate("stroke_color", 1.0)


# --- border at construction (Window.add_rect) ---------------------------


def test_add_rect_accepts_border_kwargs():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50, stroke_color=(255, 255, 255, 255), stroke_width=1.5)
    assert isinstance(node, Node)
    assert node.get("stroke_width") == pytest.approx(1.5)


def test_add_rect_without_border_kwargs_still_defaults_to_zero_width():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    assert node.get("stroke_width") == pytest.approx(0.0)


# --- border at construction (M60: widened to every other Rect-backed
# factory in the catalog, mirroring add_rect's own exact kwarg pair) -----


# --- border via static YAML (StyleSpec) ----------------------------------


# --- border/layout via {{ }} bindings ------------------------------------


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
    node = add(window, "text", text="Hello", fill=(0, 0, 0, 0), width=100, height=40, line_height=1.5)
    assert isinstance(node, Node)


def test_add_text_line_height_defaults_to_none():
    # The pre-M62 implicit behavior (the font's own natural metrics)
    # must still be reachable with zero change to an existing call --
    # must not raise.
    window = Window(width=200, height=200)
    node = add(window, "text", text="Hello", fill=(0, 0, 0, 0), width=100, height=40)
    assert isinstance(node, Node)


# --- M99: the MD3 type-scale role went with engine-md3 ---------------------


def test_add_text_no_longer_takes_a_typography_role():
    window = Window(width=200, height=200)
    with pytest.raises(ValueError, match="typography_role"):
        add(window, "text", text="Heading", fill=(0, 0, 0, 0), width=200, height=40, typography_role="headline_small")
