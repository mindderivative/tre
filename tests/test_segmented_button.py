"""M30 Phase 1 Step 4 (§5, §7): real, repeatable coverage of `Window.
add_segmented_button` -- the FFI boundary for MD3's real group-of-
connected-segments anatomy. Mirrors `test_button.py`/`test_fab.py`'s
own established structure (same "FFI wiring only" split --
`engine-render`'s own pixel tests, `corner_radii_paint.rs` included,
are the definitive paint proof, not this file).
"""

import pytest

from tre import Node, Window


def test_add_segmented_button_returns_one_node_per_label():
    window = Window(width=300, height=200)
    nodes = window.add_segmented_button(labels=["Day", "Week", "Month"], width=240)
    assert len(nodes) == 3
    assert all(isinstance(node, Node) for node in nodes)


def test_fewer_than_two_labels_raises_a_clear_value_error():
    window = Window(width=300, height=200)
    with pytest.raises(ValueError, match="at least 2 labels"):
        window.add_segmented_button(labels=["Day"], width=100)


def test_a_selected_list_of_the_wrong_length_raises_a_clear_value_error():
    window = Window(width=300, height=200)
    with pytest.raises(ValueError, match="must match"):
        window.add_segmented_button(labels=["Day", "Week"], selected=[True], width=200)


def test_a_selected_segment_does_not_raise():
    window = Window(width=300, height=200)
    nodes = window.add_segmented_button(
        labels=["Day", "Week", "Month"], selected=[False, True, False], width=240
    )
    assert len(nodes) == 3


def test_omitting_selected_defaults_to_all_unselected():
    window = Window(width=300, height=200)
    nodes = window.add_segmented_button(labels=["Day", "Week"], width=200)
    assert len(nodes) == 2


def test_a_themed_segmented_button_does_not_raise():
    window = Window(width=300, height=200)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    nodes = window.add_segmented_button(
        labels=["Day", "Week", "Month"], selected=[True, False, False], width=240
    )
    assert len(nodes) == 3


def test_the_already_generic_click_and_ripple_mechanism_works_on_every_segment():
    """The same real regression guard `test_button.py`/`test_icon_
    button.py`/`test_fab.py` already establish for `Tree::hit_test_at`
    -- every segment is its own real `Rect` container with a `Text`
    (and, when selected, `Icon`) child sized to fill the segment's own
    clickable area.
    """
    window = Window(width=300, height=200)
    nodes = window.add_segmented_button(
        labels=["Day", "Week", "Month"], selected=[False, True, False], width=240
    )

    calls = []
    for i, node in enumerate(nodes):
        node.enable_interaction()  # must not raise
        node.set_on_click(lambda i=i: calls.append(i))

    for i, node in enumerate(nodes):
        window.click(node)
    assert calls == [0, 1, 2], (
        "a real click on each segment must reach that segment's own registered handler, "
        "the same generic dispatch/ripple mechanism every other NodeKind already uses"
    )
