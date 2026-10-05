"""0.5.4 (#139): `sticky` -- a node that holds the edge of its scroller.

Checked by pixels, since a node's layout position never changes with a scroll:
a snapshot shows where the header really is.
"""

import pytest

from tre import Window

WHITE = (255, 255, 255, 255)
BODY = (60, 60, 200, 255)


def scene(inset=0, sticky=True, horizontal=False):
    window = Window(width=100, height=100)
    window.root.set(padding=0)
    view = window.create("scroll_view", width=100, height=100)
    window.root.add_child(view)
    content = window.create("box", width=100, height=400, fill=(10, 10, 10, 255))
    view.add_child(content)
    headers = []
    for i in range(4):
        section = window.create("box", width=100, height=100, x=0, y=i * 100, position="absolute")
        content.add_child(section)
        body = window.create("box", width=100, height=80, x=0, y=20, position="absolute", fill=BODY)
        section.add_child(body)
        header = window.create("box", width=100, height=20, x=0, y=0, position="absolute",
                               fill=WHITE, z_index=1)
        section.add_child(header)
        if sticky:
            header.set(sticky=inset)
        headers.append(header)
    window.snapshot()
    return window, view, headers


def column(window, x=50):
    rgba, width, height = window.snapshot()
    return [tuple(rgba[(y * width + x) * 4 : (y * width + x) * 4 + 4]) for y in range(height)]


def scroll_to(view, offset):
    view.set(scroll_offset=offset)


def white_rows(window):
    return [y for y, px in enumerate(column(window)) if px == WHITE]


def test_it_defaults_to_none_and_reads_back():
    window = Window()
    box = window.create("box", width=10, height=10)
    assert box.get("sticky") is None
    box.set(sticky=8)
    assert box.get("sticky") == 8
    box.set(sticky=None)
    assert box.get("sticky") is None


@pytest.mark.parametrize("value", [-1, "top", True, float("nan")])
def test_a_bad_sticky_is_a_value_error_and_changes_nothing(value):
    box = Window().create("box", width=10, height=10)
    box.set(sticky=3)
    with pytest.raises(ValueError, match="sticky"):
        box.set(sticky=value)
    assert box.get("sticky") == 3


def test_the_header_holds_the_top_edge_while_its_section_scrolls():
    window, view, _ = scene()
    scroll_to(view, 0)
    assert white_rows(window)[:3] == [0, 1, 2], "the first header at the top"
    scroll_to(view, 60)
    rows = white_rows(window)
    assert rows[0] == 0 and 19 in rows, "still at the top, whole"
    # The second section's header is on its way: natural 100 - 60 = 40.
    assert 40 in rows and 39 not in rows


def test_a_stuck_header_is_pushed_out_by_the_next_section():
    window, view, _ = scene()
    scroll_to(view, 90)
    col = column(window)
    # The first section ends at 100 - 90 = 10: its header (20 tall) is pushed up
    # to touch that, so only its bottom 10 rows show; the next header is at 10.
    assert all(px == WHITE for px in col[0:10])
    assert all(px == WHITE for px in col[10:30])


def test_an_inset_holds_it_that_far_from_the_edge():
    window, view, headers = scene(sticky=False)
    headers[0].set(sticky=15)
    scroll_to(view, 50)
    rows = white_rows(window)
    assert rows[0] == 15, f"starts 15 down: {rows[:3]}"


def test_without_sticky_the_header_scrolls_away():
    window, view, _ = scene(sticky=False)
    scroll_to(view, 60)
    assert 0 not in white_rows(window)


def test_turning_it_off_scrolls_it_with_the_content_again():
    window, view, headers = scene()
    scroll_to(view, 60)
    assert 0 in white_rows(window)
    headers[0].set(sticky=None)
    assert 0 not in white_rows(window)


def test_the_stuck_header_gets_the_pointer_over_the_content_under_it():
    window, view, headers = scene()
    scroll_to(view, 60)
    hits = []
    headers[0].on("click", lambda: hits.append("header"))
    window.simulate("click", x=50, y=5)
    assert hits == ["header"]


def test_a_sticky_node_in_a_scroll_view_also_works_after_it_scrolls_back():
    window, view, _ = scene()
    scroll_to(view, 200)
    scroll_to(view, 0)
    assert white_rows(window)[:3] == [0, 1, 2]
