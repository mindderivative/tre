"""M71 (§5, §8, §16.1): real coverage for `Node.set_layout`'s new
`flex_direction=` kwarg -- a real gap found while building the sibling
Tesserae project's own Python widget catalog on top of this method: no
imperative way existed anywhere to set a node's own main axis (every
`Window.add_*` factory returns a node whose real taffy default is
already `Row`, with no constructor kwarg or `Node` method to ever
change it), so a Python-composed multi-child widget needing a real
vertical stack had no way to ask for one.

Same real, honest limitation `test_live_style.py` already states for
`set_layout` generally: no Python-facing pixel-box/position readback
exists to assert a real geometry change against (`Tree::set_layout_style`
itself is the one real primitive with no readback counterpart) -- these
tests prove the real FFI call accepts the new real vocabulary and
rejects an unrecognized one, matching `parse_align_items`/`press_key`'s
own established "small vocabulary, ValueError on unrecognized" pattern.
"""

import pytest

from tre import Window


def test_set_layout_flex_direction_horizontal_does_not_raise():
    window = Window(width=200, height=100)
    node = window.add_rect(background=(0, 0, 0, 255), width=100, height=50)
    node.set_layout(flex_direction="horizontal")


def test_set_layout_flex_direction_vertical_does_not_raise():
    window = Window(width=200, height=100)
    node = window.add_rect(background=(0, 0, 0, 255), width=100, height=50)
    node.set_layout(flex_direction="vertical")


def test_set_layout_flex_direction_unknown_value_raises_naming_it():
    window = Window(width=200, height=100)
    node = window.add_rect(background=(0, 0, 0, 255), width=100, height=50)
    with pytest.raises(ValueError, match="row"):
        node.set_layout(flex_direction="row")  # the old, deliberately-rejected taffy vocabulary


def test_set_layout_flex_direction_composes_with_other_real_kwargs():
    window = Window(width=200, height=100)
    node = window.add_rect(background=(0, 0, 0, 255), width=100, height=50)
    node.set_layout(flex_direction="vertical", align_items="center", justify_content="center", gap=8)


def test_set_layout_omitting_flex_direction_is_a_true_no_op():
    window = Window(width=200, height=100)
    node = window.add_rect(background=(0, 0, 0, 255), width=100, height=50)
    # The real, pre-existing behavior -- every other real kwarg still
    # works with flex_direction entirely omitted.
    node.set_layout(width=120, height=60)
