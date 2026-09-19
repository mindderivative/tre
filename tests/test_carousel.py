"""M30 Phase 9 Step 5 (§5, §7, §11.7): real, repeatable coverage of
`Window.add_carousel` -- a real MD3 carousel (`COMPONENT_CAROUSEL.md`).
Checked with the user before starting, given the real scope (an
animated value that also invalidates layout, real wheel/drag input
this codebase didn't have anywhere else); the user chose "full real
MD3 carousel" over a scoped-down v1.

Every real item-width/position claim below is proven at the Rust level
(`crates/engine-core/src/tree.rs`'s own `carousel_*` tests, GPU-free,
reading `Tree::layout` directly) -- this suite proves the FFI surface:
`add_carousel` itself, the real synchronous wheel/scroll dispatch
(`Window.scroll`, no tick needed -- `index`/`scroll_x` move the instant
the wheel event is dispatched, the same real synchronous-mutation shape
`Tree::set_splitter_position` already has), and the real, honest limit
`test_checkbox.py`'s own doc comment already states for every other
`Animated<T>`-backed field: pytest alone can prove a real tick *moved*
a value, never that it *finished*, since a headless render loop's own
frame count bears no fixed relationship to real wall-clock duration.
"""

import pytest

from tre import Node, Window


def test_add_carousel_returns_a_node():
    window = Window(width=500, height=300)
    node = window.add_carousel(
        layout="hero", width=400, height=160, background=(20, 20, 20, 255)
    )
    assert isinstance(node, Node)


def test_add_carousel_positions_like_every_other_add_method():
    window = Window(width=500, height=300)
    node = window.add_carousel(
        layout="hero", width=400, height=160, background=(20, 20, 20, 255), x=10, y=20
    )
    assert isinstance(node, Node)


def test_add_carousel_rejects_an_unknown_layout():
    window = Window(width=500, height=300)
    with pytest.raises(ValueError, match="unknown carousel layout"):
        window.add_carousel(layout="bogus", width=400, height=160, background=(0, 0, 0, 255))


def test_a_themed_carousel_does_not_raise():
    window = Window(width=500, height=300)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    node = window.add_carousel(
        layout="multi_browse", width=400, height=160, background=(20, 20, 20, 255)
    )
    assert isinstance(node, Node)


def test_a_fresh_carousel_starts_at_index_zero_and_position_zero():
    window = Window(width=500, height=300)
    carousel = window.add_carousel(
        layout="hero", width=400, height=160, background=(20, 20, 20, 255)
    )
    assert carousel.get_carousel_index() == 0
    assert carousel.get_carousel_position() == 0.0


def test_items_are_added_the_same_generic_way_as_any_other_container():
    window = Window(width=500, height=300)
    carousel = window.add_carousel(
        layout="hero", width=400, height=160, background=(20, 20, 20, 255)
    )
    for _ in range(3):
        item = window.add_rect(background=(255, 0, 0, 255), width=100, height=100)
        carousel.add_child(item)  # must not raise -- Node.add_child is fully generic


def test_a_wheel_notch_over_a_hero_item_snaps_to_the_next_index_synchronously():
    """`Tree::set_carousel_index` mutates `index` the instant it's
    called (the destination, not the eased-toward value) -- no real
    tick needed to observe this half, unlike `position` below.
    """
    window = Window(width=500, height=300)
    carousel = window.add_carousel(
        layout="hero", width=400, height=160, background=(20, 20, 20, 255)
    )
    items = []
    for _ in range(3):
        item = window.add_rect(background=(255, 0, 0, 255), width=100, height=100)
        carousel.add_child(item)
        items.append(item)

    window.scroll(items[0], 120.0)
    assert carousel.get_carousel_index() == 1


def test_wheel_snapping_starts_a_real_eased_animation_not_an_instant_jump():
    window = Window(width=500, height=300)
    carousel = window.add_carousel(
        layout="hero", width=400, height=160, background=(20, 20, 20, 255)
    )
    item = window.add_rect(background=(255, 0, 0, 255), width=100, height=100)
    carousel.add_child(item)
    second = window.add_rect(background=(255, 0, 0, 255), width=100, height=100)
    carousel.add_child(second)  # a second item to move to

    window.scroll(item, 120.0)
    # The real destination moved, but the real eased value has not
    # caught up yet -- no tick has run between the two calls.
    assert carousel.get_carousel_index() == 1
    assert carousel.get_carousel_position() == 0.0


def test_uncontained_scroll_moves_and_clamps_to_the_real_content_extent():
    window = Window(width=500, height=300)
    carousel = window.add_carousel(
        layout="uncontained", width=300, height=160, background=(20, 20, 20, 255)
    )
    items = []
    for _ in range(5):
        item = window.add_rect(background=(0, 255, 0, 255), width=200, height=100)
        carousel.add_child(item)
        items.append(item)

    assert carousel.get_carousel_scroll() == 0.0
    window.scroll(items[0], 100_000.0)
    # Real extent: 2*16 (PAD_X) + 5*200 + 4*8 (GAP) = 1064; clamped max
    # scroll = 1064 - 300 = 764.
    assert abs(carousel.get_carousel_scroll() - 764.0) < 0.5


def test_set_carousel_index_moves_the_real_destination_and_clamps():
    window = Window(width=500, height=300)
    carousel = window.add_carousel(
        layout="hero", width=400, height=160, background=(20, 20, 20, 255)
    )
    for _ in range(3):
        item = window.add_rect(background=(255, 0, 0, 255), width=100, height=100)
        carousel.add_child(item)

    carousel.set_carousel_index(2)
    assert carousel.get_carousel_index() == 2

    carousel.set_carousel_index(99)  # past the real child count
    assert carousel.get_carousel_index() == 2, "an out-of-range index must clamp, not raise"


def test_set_carousel_scroll_moves_and_clamps():
    """Unlike `Window.scroll` (which computes a real layout pass as a
    side effect via `node_center`, `dispatch.rs`'s own doc comment),
    `Node.set_carousel_scroll` is a raw `Tree` entry point with no such
    guarantee -- its own real clamp reads each item's last-computed
    `Layout` the identical "one frame stale is fine" way `update_
    slider_drag`/`update_splitter_drag` already do, so it needs a real
    layout pass to have happened at least once first, the same real
    precondition every one of those already has. A zero-delta real
    scroll is the cheapest way to trigger one from Python.
    """
    window = Window(width=500, height=300)
    carousel = window.add_carousel(
        layout="uncontained", width=300, height=160, background=(20, 20, 20, 255)
    )
    items = []
    for _ in range(5):
        item = window.add_rect(background=(0, 255, 0, 255), width=200, height=100)
        carousel.add_child(item)
        items.append(item)
    window.scroll(items[0], 0.0)

    carousel.set_carousel_scroll(500.0)
    assert abs(carousel.get_carousel_scroll() - 500.0) < 0.5

    carousel.set_carousel_scroll(-10.0)
    assert carousel.get_carousel_scroll() == 0.0, "scroll must clamp at zero, not go negative"


def test_carousel_accessors_reject_a_non_carousel_node():
    window = Window(width=500, height=300)
    rect = window.add_rect(background=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="Rect has no property 'index'"):
        rect.get_carousel_index()
    with pytest.raises(ValueError, match="Rect has no property 'index'"):
        rect.set_carousel_index(0)
    with pytest.raises(ValueError, match="Rect has no property 'position'"):
        rect.get_carousel_position()
    with pytest.raises(ValueError, match="Rect has no property 'scroll_x'"):
        rect.get_carousel_scroll()
    with pytest.raises(ValueError, match="Rect has no property 'scroll_x'"):
        rect.set_carousel_scroll(0.0)
