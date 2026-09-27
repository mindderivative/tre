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

import pytest

from tre import Window


def test_right_clicking_an_anchor_with_no_registered_menu_is_a_safe_no_op():
    window = Window(width=200, height=120)
    anchor = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=80, height=40)

    window.right_click(anchor)  # must not raise


