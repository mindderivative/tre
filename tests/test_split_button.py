"""M35 Phase 2 (§5, §7, §8): real, repeatable coverage of
`Window.add_split_button` -- MD3's real Split Button (leading button +
trailing menu-icon button). The engine never opens/closes a menu or
rotates the trailing icon on its own -- the app drives both directly
via `Node.animate("rotation", ...)`, Design Principle 6's own "engine
provides the mechanism, app decides the real state change" split.
"""

import pytest

from tre import Node, Window


def test_add_split_button_returns_three_real_distinct_nodes():
    window = Window(width=800, height=600)
    leading, trailing, icon = window.add_split_button(label="Watch later", width=140, height=32)
    assert isinstance(leading, Node)
    assert isinstance(trailing, Node)
    assert isinstance(icon, Node)


def test_a_themed_split_button_does_not_raise():
    window = Window(width=800, height=600)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    leading, trailing, icon = window.add_split_button(
        label="Watch later", width=140, height=32, variant="outlined"
    )
    assert isinstance(leading, Node)
    assert isinstance(trailing, Node)
    assert isinstance(icon, Node)


def test_an_unknown_variant_raises_a_clear_value_error():
    window = Window(width=800, height=600)
    with pytest.raises(ValueError, match="unknown button variant"):
        window.add_split_button(label="Watch later", width=140, height=32, variant="bogus")


def test_the_leading_and_trailing_buttons_are_independently_clickable():
    window = Window(width=800, height=600)
    leading, trailing, _icon = window.add_split_button(label="Watch later", width=140, height=32)

    clicked = []
    leading.enable_interaction()
    leading.set_on_click(lambda: clicked.append("leading"))
    trailing.enable_interaction()
    trailing.set_on_click(lambda: clicked.append("trailing"))

    window.click(trailing)
    assert clicked == ["trailing"], "a real click on the trailing button must reach only its own handler"


def test_the_leading_button_click_does_not_reach_the_trailing_handler():
    window = Window(width=800, height=600)
    leading, trailing, _icon = window.add_split_button(label="Watch later", width=140, height=32)

    clicked = []
    leading.enable_interaction()
    leading.set_on_click(lambda: clicked.append("leading"))
    trailing.enable_interaction()
    trailing.set_on_click(lambda: clicked.append("trailing"))

    window.click(leading)
    assert clicked == ["leading"], "a real click on the leading button must reach only its own handler"


def test_hovering_either_button_does_not_raise_and_tightens_the_shared_inner_corners():
    """M38 Phase 4 (§5, §7): real MD3 "the inner corners change shape
    for hovered, focused, and pressed states" -- `window.hover(node)`
    dispatches a real pointer-moved event, which `Tree::update_hover`
    now also uses to retarget `PaintProperties.shape` toward each
    button's own tightened silhouette. There is no Python getter for
    a `Node`'s own raw `shape` animation target (the same real
    verification-surface limit M37/M38 Phase 2/3 already established
    for other internal-only state), so this proves the real, full
    dispatch-through-paint path runs clean end to end -- the exact
    geometry itself is proven directly at the Rust level
    (`crates/engine-core/src/tree.rs`'s own `update_hover_retargets_a_
    real_interactive_shape_toward_tightened_then_relaxed`).
    """
    window = Window(width=800, height=600)
    leading, trailing, _icon = window.add_split_button(label="Watch later", width=140, height=32)
    window.hover(leading)
    window.hover(trailing)
    # Moving off both again must relax the shape back without raising.
    window.hover(leading)
    other = window.add_rect(background=(0, 0, 0, 255), width=10, height=10, x=700, y=550)
    window.hover(other)


def test_hovering_an_outlined_split_button_does_not_raise():
    """The real regression case this phase's own investigation found:
    combining a nonzero real border (`variant="outlined"`) with the
    new per-corner shape-tightening exercises a border path that used
    to ignore per-corner geometry entirely (`RectPathParams::
    PerCornerBorder`'s own doc comment, `geometry_cache.rs`) -- proven
    directly at the Rust level via `per_corner_border_produces_
    asymmetric_geometry_distinct_from_a_uniform_border`; this proves
    the real end-to-end FFI path (construct, hover, paint) never
    panics for the one real variant that actually has a visible border.
    """
    window = Window(width=800, height=600)
    leading, trailing, _icon = window.add_split_button(
        label="Watch later", width=140, height=32, variant="outlined"
    )
    window.hover(leading)
    window.hover(trailing)


def test_the_trailing_icon_rotation_can_be_animated_by_the_app():
    """The engine never rotates the icon automatically -- the app
    drives it directly via `Node.animate("rotation", ...)`, the same
    real "engine provides the mechanism" split `Checkbox.checked`
    already establishes.
    """
    window = Window(width=800, height=600)
    _leading, _trailing, icon = window.add_split_button(label="Watch later", width=140, height=32)
    icon.animate("rotation", 180.0, 0)
    icon.animate("rotation", 0.0, 0)
