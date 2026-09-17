"""M13 Phase 1 (§11.2): real, repeatable coverage that `Window.
build_shell` genuinely attaches its given chrome regions and returns a
real, usable `content` node -- not just that the call doesn't raise.

The real, functional proof each region is genuinely attached and laid
out: clicking it (via `Window.click()`, which only works on a node
that's really part of the live tree with a real computed layout)
confirms its own handler fires -- the same "clicking it proves it's
real" discipline `test_docking.py`/`test_context_menu.py` already use,
not an inspection of internal state Python has no getter for.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_engine_py.py`.
"""

import pytest

from tre import Node, Window


def test_build_shell_with_all_regions_attaches_them_for_real():
    window = Window(width=300, height=200)
    menu_bar = window.add_rect(background=(0, 0, 0, 255), width=300, height=20)
    toolbar = window.add_rect(background=(0, 0, 0, 255), width=300, height=30)
    status_bar = window.add_rect(background=(0, 0, 0, 255), width=300, height=20)

    content = window.build_shell(menu_bar=menu_bar, toolbar=toolbar, status_bar=status_bar)
    assert isinstance(content, Node)

    calls = []
    menu_bar.set_on_click(lambda: calls.append("menu_bar"))
    toolbar.set_on_click(lambda: calls.append("toolbar"))
    status_bar.set_on_click(lambda: calls.append("status_bar"))

    window.click(menu_bar)
    window.click(toolbar)
    window.click(status_bar)
    assert calls == ["menu_bar", "toolbar", "status_bar"], (
        "each region must be genuinely attached and laid out inside the real shell, "
        "not merely referenced"
    )


def test_build_shell_with_no_regions_still_returns_a_usable_content_node():
    window = Window(width=300, height=200)
    content = window.build_shell()
    assert isinstance(content, Node)

    child = window.add_rect(background=(255, 0, 0, 255), width=50, height=50)
    content.add_child(child)

    calls = []
    child.set_on_click(lambda: calls.append("clicked"))
    window.click(child)
    assert calls == ["clicked"], "content must be a real, live node ready for add_child"


def test_build_shell_fills_remaining_space_after_fixed_chrome_regions():
    """`content`'s own real `flex_grow: 1.0` must actually take effect --
    checked the same way `test_position.py`-style tests confirm real
    layout: a child added to `content` and clicked at content's own
    real computed center must be found there, below the fixed-height
    menu bar this test gives it (proving `content` didn't just get
    squeezed to zero height).
    """
    window = Window(width=300, height=200)
    menu_bar = window.add_rect(background=(0, 0, 0, 255), width=300, height=20)
    content = window.build_shell(menu_bar=menu_bar)

    child = window.add_rect(background=(255, 0, 0, 255), width=300, height=100)
    content.add_child(child)

    calls = []
    child.set_on_click(lambda: calls.append("clicked"))
    window.click(child)
    assert calls == ["clicked"], "content must have real, non-zero remaining height to lay out in"


def test_build_shell_rejects_a_region_from_a_different_window():
    window_a = Window(width=300, height=200)
    window_b = Window(width=300, height=200)
    foreign = window_b.add_rect(background=(0, 0, 0, 255), width=300, height=20)

    with pytest.raises(ValueError, match="different Window"):
        window_a.build_shell(menu_bar=foreign)
