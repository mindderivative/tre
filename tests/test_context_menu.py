"""M4 Phase 7 (§11.3): real, repeatable coverage that a right-click
actually opens a registered context menu -- new `DispatchOutcome::
SecondaryActivated`, `Node.set_context_menu`, and `dispatch::
open_context_menu` (the real, existing `Tree::open_overlay` mechanism,
finally reached from Python for the first time).

`Window.right_click(node)`/`View.right_click(node)` mirror `.click()`/
`.hover()`'s own no-live-window-needed proof pattern: a real
`Tree::dispatch` secondary-button press+release at the node's own
computed center.

The real, functional proof that the menu's content genuinely became a
live, laid-out, dispatchable part of the tree (not an inspection of
internal state Python has no getter for): right-click the anchor, then
click the menu item and confirm *its own* handler fired.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_engine_py.py`.
"""

from tre import Window


def test_right_clicking_an_anchor_opens_its_registered_context_menu():
    window = Window(width=200, height=120)
    anchor = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=80, height=40)
    menu_item = window.add_rect(background=(0x00, 0x80, 0x00, 0xFF), width=60, height=20)

    calls = []
    menu_item.set_on_click(lambda: calls.append("selected"))
    anchor.set_context_menu(menu_item)

    window.right_click(anchor)

    # The real, functional proof: the menu item is now a genuinely
    # live, laid-out, dispatchable node -- clicking it fires its own
    # handler, which is only possible if `open_overlay` really attached
    # it to the tree.
    window.click(menu_item)
    assert calls == ["selected"]


def test_right_clicking_an_anchor_with_no_registered_menu_is_a_safe_no_op():
    window = Window(width=200, height=120)
    anchor = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=80, height=40)

    window.right_click(anchor)  # must not raise


def test_right_clicking_the_same_anchor_twice_does_not_crash():
    """A real guard against double-`add_child`-ing the same still-open
    overlay content -- `dispatch::open_context_menu` checks
    `overlay_meta` before reopening.
    """
    window = Window(width=200, height=120)
    anchor = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=80, height=40)
    menu_item = window.add_rect(background=(0x00, 0x80, 0x00, 0xFF), width=60, height=20)
    anchor.set_context_menu(menu_item)

    window.right_click(anchor)
    window.right_click(anchor)  # must not raise or duplicate the child

    calls = []
    menu_item.set_on_click(lambda: calls.append("selected"))
    window.click(menu_item)
    assert calls == ["selected"]


def test_a_real_click_outside_an_open_context_menu_dismisses_it():
    """M10 Phase 1 (§11.3): the real FFI-level proof that outside-click
    dismissal reaches all the way through -- a click somewhere else
    entirely closes the menu for real (its own content is genuinely
    gone from the tree, not just hidden), proven the same functional
    way the other tests in this file do: the menu item's own click
    handler no longer fires once it's gone.
    """
    window = Window(width=200, height=200)
    anchor = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=80, height=40)
    menu_item = window.add_rect(background=(0x00, 0x80, 0x00, 0xFF), width=60, height=20)
    elsewhere = window.add_rect(background=(0x00, 0x00, 0xFF, 0xFF), width=20, height=20)
    anchor.set_context_menu(menu_item)

    window.right_click(anchor)
    window.click(elsewhere)  # real click, well away from the open menu

    calls = []
    menu_item.set_on_click(lambda: calls.append("selected"))
    window.click(menu_item)
    assert calls == [], "the menu must be genuinely gone after a real outside click"


def test_a_real_escape_press_dismisses_an_open_context_menu():
    window = Window(width=200, height=120)
    anchor = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=80, height=40)
    menu_item = window.add_rect(background=(0x00, 0x80, 0x00, 0xFF), width=60, height=20)
    anchor.set_context_menu(menu_item)

    window.right_click(anchor)
    window.press_key("escape")

    calls = []
    menu_item.set_on_click(lambda: calls.append("selected"))
    window.click(menu_item)
    assert calls == [], "the menu must be genuinely gone after a real Escape press"


def test_left_clicking_an_anchor_with_a_context_menu_does_not_open_it():
    """Only a secondary-button press/release opens a context menu --
    a primary click must not trigger it, matching real desktop
    convention.
    """
    window = Window(width=200, height=120)
    anchor = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=80, height=40)
    menu_item = window.add_rect(background=(0x00, 0x80, 0x00, 0xFF), width=60, height=20)
    anchor.set_context_menu(menu_item)

    window.click(anchor)  # primary click, not right_click

    calls = []
    menu_item.set_on_click(lambda: calls.append("selected"))
    window.click(menu_item)
    assert calls == [], "a plain left-click on the anchor must not open its context menu"
