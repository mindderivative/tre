"""M30 Phase 2 Step 4 (§5, §7, §11.3): real, repeatable coverage of
`Window.add_menu_item`/`build_menu`/`open_menu`/`close_menu` -- the
FFI boundary for a real MD3 dropdown menu, built on the same real
`Tree::open_overlay`/`close_overlay` primitive the existing right-
click context-menu mechanism already uses (deliberately left
untouched by this step -- see `test_context_menu.py`, unmodified).
"""

import pytest

from tre import Node, Window


def test_add_menu_item_returns_a_node():
    window = Window(width=300, height=300)
    node = window.add_menu_item(label="Copy")
    assert isinstance(node, Node)


def test_add_menu_item_with_an_icon_does_not_raise():
    window = Window(width=300, height=300)
    node = window.add_menu_item(label="Copy", icon="add")
    assert isinstance(node, Node)


def test_an_unknown_icon_name_raises_a_clear_value_error():
    window = Window(width=300, height=300)
    with pytest.raises(ValueError, match="unknown icon"):
        window.add_menu_item(label="Copy", icon="not-a-real-icon-name")


def test_build_menu_returns_a_node():
    window = Window(width=300, height=300)
    items = [window.add_menu_item(label=label) for label in ["Copy", "Paste", "Delete"]]
    menu = window.build_menu(items)
    assert isinstance(menu, Node)


def test_build_menu_with_no_items_raises_a_clear_value_error():
    window = Window(width=300, height=300)
    with pytest.raises(ValueError, match="at least 1 item"):
        window.build_menu([])


def test_build_menu_rejects_an_item_from_a_different_window():
    window_a = Window(width=300, height=300)
    window_b = Window(width=300, height=300)
    foreign_item = window_b.add_menu_item(label="Foreign")
    with pytest.raises(ValueError, match="different Window"):
        window_a.build_menu([foreign_item])


def test_open_menu_and_close_menu_do_not_raise():
    window = Window(width=300, height=300)
    anchor = window.add_rect(background=(0, 0, 0, 255), width=100, height=40)
    items = [window.add_menu_item(label=label) for label in ["Copy", "Paste"]]
    menu = window.build_menu(items)

    window.open_menu(anchor, menu)  # must not raise
    window.open_menu(anchor, menu)  # a real, safe no-op -- already open
    window.close_menu(menu)  # must not raise


def test_open_menu_rejects_an_anchor_from_a_different_window():
    window_a = Window(width=300, height=300)
    window_b = Window(width=300, height=300)
    foreign_anchor = window_b.add_rect(background=(0, 0, 0, 255), width=100, height=40)
    items = [window_a.add_menu_item(label="Copy")]
    menu = window_a.build_menu(items)
    with pytest.raises(ValueError, match="different Window"):
        window_a.open_menu(foreign_anchor, menu)


def test_reopening_a_menu_after_closing_it_does_not_raise():
    """The real `Tree::close_overlay` contract this reuses: detach,
    not destroy -- a closed menu must be safely reopenable, the same
    real reason `Node.set_context_menu`'s own content is never
    destroyed on dismissal.
    """
    window = Window(width=300, height=300)
    anchor = window.add_rect(background=(0, 0, 0, 255), width=100, height=40)
    items = [window.add_menu_item(label="Copy")]
    menu = window.build_menu(items)

    window.open_menu(anchor, menu)
    window.close_menu(menu)
    window.open_menu(anchor, menu)  # must not raise -- real reopen, not a stale/destroyed node


def test_a_menu_item_is_a_real_clickable_node_once_its_menu_is_open():
    """`build_menu` re-parents each item under a panel that isn't
    attached anywhere yet -- a real, deliberate design (a menu item
    genuinely isn't on-screen, and so genuinely isn't clickable, until
    `open_menu` actually attaches the panel to the tree). This proves
    the item's own real click-dispatch identity survives both the
    re-parent (`build_menu`) and the later attach (`open_menu`).
    """
    window = Window(width=300, height=300)
    anchor = window.add_rect(background=(0, 0, 0, 255), width=100, height=40)
    item = window.add_menu_item(label="Copy")
    menu = window.build_menu([item])
    window.open_menu(anchor, menu)
    item.enable_interaction()  # must not raise

    calls = []
    item.set_on_click(lambda: calls.append("clicked"))
    window.click(item)
    assert calls == ["clicked"], (
        "a real click on a menu item must reach its own registered handler once its menu "
        "is actually open"
    )
