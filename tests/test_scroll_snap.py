"""0.5.6 (#166): `scroll_snap` / `snap_align` on scroll views, `scroll_offset`
on a virtual list, and the wheel on a horizontal strip."""

import pytest

from tre import Window
from helpers import add


def strip(snap=None, item=100, count=5, view=100, **item_props):
    """A horizontal scroll view `view` wide over a row of `count` items."""
    window = Window(width=400, height=200)
    scroller = add(window, "scroll_view", orientation="horizontal", width=view, height=60)
    row = window.create("box", flex_direction="horizontal", fill=(0, 0, 0, 0))
    scroller.add_child(row)
    items = []
    for _ in range(count):
        node = window.create("box", width=item, height=40, fill=(255, 0, 0, 255), **item_props)
        row.add_child(node)
        items.append(node)
    if snap:
        scroller.set(scroll_snap=snap)
    window.advance(0)
    return window, scroller, items


def settle(window):
    """Time for the input to stop, then for the 200 ms settle to run."""
    window.advance(0)
    window.advance(150)
    window.advance(300)


def wheel(window, scroller, dx):
    window.simulate("wheel", node=scroller, delta_x=dx, delta_y=0)


def test_wheel_without_snap_stays_where_it_stopped():
    window, scroller, _ = strip()
    wheel(window, scroller, 140)
    settle(window)
    assert scroller.get("scroll_offset") == pytest.approx(140)


def test_start_snap_settles_on_the_nearest_item_start():
    window, scroller, _ = strip(snap="start")
    wheel(window, scroller, 140)
    assert scroller.get("scroll_offset") == pytest.approx(140)        # not yet
    settle(window)
    assert scroller.get("scroll_offset") == pytest.approx(100)
    wheel(window, scroller, 60)                                       # 160 -> nearer 200
    settle(window)
    assert scroller.get("scroll_offset") == pytest.approx(200)


def test_center_and_end_snap_line_the_item_up_with_the_middle_and_the_end():
    # 60-wide items in a 100-wide view: item i spans [60i, 60i+60].
    window, scroller, _ = strip(snap="center", item=60, count=6)
    wheel(window, scroller, 100)
    settle(window)
    assert scroller.get("scroll_offset") == pytest.approx(60 + 30 - 50 + 60)    # item 2 centred: 100
    window, scroller, _ = strip(snap="end", item=60, count=6)
    wheel(window, scroller, 100)
    settle(window)
    # item 1 ends at 120, item 2 at 180: 100 is nearer 80 (item 1's end) than 140
    assert scroller.get("scroll_offset") == pytest.approx(80)


def test_the_last_item_snaps_to_the_end_of_the_content():
    window, scroller, _ = strip(snap="start")
    wheel(window, scroller, 1000)
    settle(window)
    assert scroller.get("scroll_offset") == pytest.approx(400)        # 500 wide, 100 view


def test_a_child_can_snap_differently_from_the_view():
    window, scroller, items = strip(snap="start")
    items[2].set(snap_align="center")                                  # item 2: [200, 300]
    assert items[2].get("snap_align") == "center" and items[0].get("snap_align") is None
    wheel(window, scroller, 190)
    settle(window)
    assert scroller.get("scroll_offset") == pytest.approx(200)         # centred: 250 - 50


def test_more_input_delays_the_settle():
    window, scroller, _ = strip(snap="start")
    wheel(window, scroller, 140)
    window.advance(0)
    window.advance(100)                                                # under 120 ms
    wheel(window, scroller, 10)
    window.advance(100)
    assert scroller.get("scroll_offset") == pytest.approx(150)         # still not settled
    window.advance(100)
    window.advance(300)                                                # settle starts
    window.advance(300)                                                # and ends
    assert scroller.get("scroll_offset") in (pytest.approx(100), pytest.approx(200))


def test_your_own_scroll_offset_is_not_snapped():
    window, scroller, _ = strip(snap="start")
    scroller.set(scroll_offset=140)
    settle(window)
    assert scroller.get("scroll_offset") == pytest.approx(140)
    scroller.animate("scroll_offset", 170, 50)
    settle(window)
    assert scroller.get("scroll_offset") == pytest.approx(170)


def test_snapping_works_vertically_too():
    window = Window(width=200, height=300)
    scroller = add(window, "scroll_view", width=100, height=100)
    column = window.create("box", flex_direction="vertical", fill=(0, 0, 0, 0))
    scroller.add_child(column)
    for _ in range(5):
        column.add_child(window.create("box", width=100, height=50, fill=(255, 0, 0, 255)))
    scroller.set(scroll_snap="start")
    window.advance(0)
    window.simulate("wheel", node=scroller, delta_x=0, delta_y=70)
    settle(window)
    assert scroller.get("scroll_offset") == pytest.approx(50)


def test_scroll_snap_reads_back_and_rejects_bad_values():
    _, scroller, items = strip()
    assert scroller.get("scroll_snap") == "none"
    for value in ("start", "center", "end", "none"):
        scroller.set(scroll_snap=value)
        assert scroller.get("scroll_snap") == value
    with pytest.raises(ValueError, match="scroll_snap"):
        scroller.set(scroll_snap="middle")
    with pytest.raises(ValueError, match="snap_align"):
        items[0].set(snap_align="none")
    with pytest.raises(ValueError, match="scroll_view"):
        items[0].set(scroll_snap="start")


# --- wheel on a horizontal strip ------------------------------------------------

def test_a_plain_wheel_still_has_no_horizontal_part_but_shift_wheel_scrolls_a_strip():
    # Unchanged from 0.4.4: a plain vertical wheel over a carousel scrolls the page.
    window, scroller, _ = strip()
    window.simulate("wheel", node=scroller, delta_x=0, delta_y=60)
    assert scroller.get("scroll_offset") == 0
    window.simulate("wheel", node=scroller, delta_y=60, shift=True)
    assert scroller.get("scroll_offset") == pytest.approx(60)


# --- virtual_list scroll_offset ------------------------------------------------

def make_list(window):
    return add(window, "virtual_list", item_count=100, item_extent=20.0,
               materialize=lambda _i: window.create("box", fill=(255, 0, 0, 255)),
               width=100, height=100)


def test_a_virtual_list_has_a_settable_readable_animatable_offset():
    window = Window(width=200, height=200)
    rows = make_list(window)
    window.advance(0)
    assert rows.get("scroll_offset") == 0
    rows.set(scroll_offset=300)
    assert rows.get("scroll_offset") == 300
    rows.animate("scroll_offset", 500, 100)
    window.advance(50)
    assert 300 < rows.get("scroll_offset") < 500
    window.advance(100)
    assert rows.get("scroll_offset") == 500 and rows.get_target("scroll_offset") == 500
    seen = []
    rows.on("animation_end", lambda e: seen.append((e.property, e.finished)))
    rows.animate("scroll_offset", 700, 50)
    window.advance(100)
    assert seen == [("scroll_offset", True)]


def test_a_virtual_list_offset_is_clamped_to_its_content():
    window = Window(width=200, height=200)
    rows = make_list(window)
    window.advance(0)
    rows.animate("scroll_offset", 99999, 50)
    window.advance(100)
    assert rows.get("scroll_offset") == pytest.approx(100 * 20 - 100)    # 1900


def test_scroll_offset_on_a_node_that_cannot_scroll_raises():
    window = Window(width=200, height=200)
    plain = add(window, "box", width=10, height=10)
    for call in (lambda: plain.get("scroll_offset"), lambda: plain.set(scroll_offset=5),
                 lambda: plain.animate("scroll_offset", 5, 10)):
        with pytest.raises(ValueError, match="scroll_view or virtual_list"):
            call()
