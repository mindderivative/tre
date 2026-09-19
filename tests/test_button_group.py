"""M35 Phase 3 (§5, §7, §11.7): real, repeatable coverage of
`Window.add_button_group` -- MD3's real Standard Button Group. The
real reflow mechanic (pressing one button grows it and shrinks its
immediate neighbors) is driven entirely by `engine-core::Tree::
sync_button_group_layouts`, reading the already-tracked live pointer-
press state -- no app-side wiring needed, unlike `Split Button`'s own
real rotation, which the app must drive itself. This file focuses on
the real Python-facing construction/click-independence contract; the
reflow math itself is proven directly at the Rust level
(`crates/engine-core/src/tree.rs::sync_button_group_layouts_*`).
"""

import pytest

from tre import Node, Window


def test_add_button_group_returns_one_real_group_and_n_real_buttons():
    window = Window(width=800, height=600)
    group, buttons = window.add_button_group(labels=["One", "Two", "Three"], width=80, height=40)
    assert isinstance(group, Node)
    assert len(buttons) == 3
    assert all(isinstance(b, Node) for b in buttons)


def test_a_themed_button_group_does_not_raise():
    window = Window(width=800, height=600)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    group, buttons = window.add_button_group(labels=["A", "B"], width=60, height=32, variant="outlined")
    assert isinstance(group, Node)
    assert len(buttons) == 2


def test_an_unknown_variant_raises_a_clear_value_error():
    window = Window(width=800, height=600)
    with pytest.raises(ValueError, match="unknown button variant"):
        window.add_button_group(labels=["A"], width=60, height=32, variant="bogus")


def test_an_empty_label_list_returns_an_empty_real_button_list():
    window = Window(width=800, height=600)
    group, buttons = window.add_button_group(labels=[], width=60, height=32)
    assert isinstance(group, Node)
    assert buttons == []


def test_each_button_is_independently_clickable():
    window = Window(width=800, height=600)
    _group, buttons = window.add_button_group(labels=["One", "Two", "Three"], width=80, height=40)

    clicked = []
    for i, b in enumerate(buttons):
        b.enable_interaction()
        b.set_on_click(lambda i=i: clicked.append(i))

    window.click(buttons[1])
    assert clicked == [1], "clicking one button in the group must reach only its own registered handler"


def test_a_real_click_on_one_button_does_not_reach_a_sibling():
    window = Window(width=800, height=600)
    _group, buttons = window.add_button_group(labels=["One", "Two"], width=80, height=40)

    clicked = []
    for i, b in enumerate(buttons):
        b.enable_interaction()
        b.set_on_click(lambda i=i: clicked.append(i))

    window.click(buttons[0])
    window.click(buttons[1])
    assert clicked == [0, 1], "each real click must reach only its own button's own handler, in order"


def test_clicking_a_button_group_child_does_not_raise():
    """M38 Phase 5 (§5, §7): real MD3 Expressive "buttons reshape as
    you press them" -- `window.click(node)` dispatches a real primary
    press+release, exercising `Tree::set_pressed`'s own new shape
    retarget (`PaintProperties.press_interactive_shape`) on both the
    press transition (real relaxed -> tightened) and the release
    transition (tightened -> relaxed) in one call. There is no Python
    getter for a `Node`'s own raw `shape` animation target, the same
    real verification-surface limit already established for Split
    Button's own hover-driven case (M38 Phase 4) -- the exact geometry
    is proven directly at the Rust level (`crates/engine-core/src/
    tree.rs`'s own `set_pressed_retargets_a_real_press_interactive_
    shape_toward_tightened_then_relaxed`); this proves the real,
    full dispatch-through-paint path runs clean end to end, for both
    the default and the `"outlined"` variant (a real border, the same
    combination that surfaced a genuine bug for Split Button).
    """
    window = Window(width=800, height=600)
    _group, buttons = window.add_button_group(labels=["One", "Two"], width=80, height=40)
    window.click(buttons[0])
    window.click(buttons[1])

    window2 = Window(width=800, height=600)
    _group2, outlined_buttons = window2.add_button_group(
        labels=["One", "Two"], width=80, height=40, variant="outlined"
    )
    window2.click(outlined_buttons[0])
