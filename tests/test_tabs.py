"""M30 Phase 5 Step 4 (§5, §7): real, repeatable coverage of
`Window.add_tabs` -- MD3's real *Primary Navigation Tab* variant.
Built as a plain composition, the same real dividing line `Segmented
Button`/`Filter Chip`/`Navigation Rail`/`Navigation Drawer` already
established: a tab's own "active" state is app-owned group-select
state, not a new engine-owned toggle. This step's own real geometric
design (a thin 3dp indicator flush against the tab's own bottom edge,
never overlapping its geometric center) is verified directly by the
click-dispatch test below, not assumed safe from `Navigation Rail`'s
own real hit-test lesson.
"""

import pytest

from tre import Node, Window


def test_add_tabs_returns_one_node_per_label():
    window = Window(width=600, height=100)
    tabs = window.add_tabs(labels=["Recents", "Favorites", "Nearby"])
    assert len(tabs) == 3
    assert all(isinstance(tab, Node) for tab in tabs)


def test_add_tabs_with_icons_does_not_raise():
    window = Window(width=600, height=100)
    tabs = window.add_tabs(labels=["Recents", "Favorites"], icons=["add", "add"])
    assert len(tabs) == 2


def test_add_tabs_with_a_selected_index_does_not_raise():
    window = Window(width=600, height=100)
    tabs = window.add_tabs(labels=["Recents", "Favorites", "Nearby"], selected=1)
    assert len(tabs) == 3


def test_a_themed_tabs_row_does_not_raise():
    window = Window(width=600, height=100)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    tabs = window.add_tabs(labels=["Recents", "Favorites"])
    assert len(tabs) == 2


def test_empty_labels_raises_a_clear_value_error():
    window = Window(width=600, height=100)
    with pytest.raises(ValueError, match="at least 1 item"):
        window.add_tabs(labels=[])


def test_mismatched_icons_raises_a_clear_value_error():
    window = Window(width=600, height=100)
    with pytest.raises(ValueError, match="one icon per label"):
        window.add_tabs(labels=["Recents", "Favorites"], icons=["add"])


def test_out_of_range_selected_raises_a_clear_value_error():
    window = Window(width=600, height=100)
    with pytest.raises(ValueError, match="out of range"):
        window.add_tabs(labels=["Recents"], selected=1)


def test_an_unknown_icon_name_raises_a_clear_value_error():
    window = Window(width=600, height=100)
    with pytest.raises(ValueError, match="unknown icon"):
        window.add_tabs(labels=["Recents"], icons=["not-a-real-icon-name"])


def test_each_tab_is_a_real_independently_clickable_node():
    """The real point of this step's own geometric design: a click
    at each tab's own center must reach that tab's own handler, not
    be intercepted by the thin active-indicator strip at its bottom
    edge -- the same real class of bug `Navigation Rail` found, here
    verified to not reproduce rather than assumed safe from geometry
    alone.
    """
    window = Window(width=600, height=100)
    tabs = window.add_tabs(labels=["Recents", "Favorites", "Nearby"])

    clicked = []
    for i, tab in enumerate(tabs):
        tab.enable_interaction()
        tab.set_on_click(lambda i=i: clicked.append(i))

    window.click(tabs[1])
    assert clicked == [1], "clicking one tab must reach only its own registered handler"
