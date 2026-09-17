"""M15 Phase 1 (§5, §16.7): real, repeatable coverage of `Window.
add_text_field`/`Node.get_text`/`Node.is_focused` -- the FFI boundary
for a real, single-line, editable text field. No editing yet (M15
Phase 2's own scope) -- this file proves: `add_text_field` returns a
real, usable `Node`; `get_text` reaches real `TextFieldState.content`;
and a real Tab press (`Window.press_key("tab")`, §10's already-real
focus model) actually reaches a `TextField`, the first real Python-
observable proof of that (`Node.is_focused`, also new this phase).

The definitive pixel-level proof that a `TextField` genuinely paints a
real caret only while focused is `crates/engine-render/tests/
text_field_paint.rs`, not this file -- the same "FFI wiring only"
split this project's test suite has used throughout.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as every other FFI test in this suite.
"""

import pytest

from tre import Node, Window


def test_add_text_field_returns_a_node():
    window = Window(width=200, height=100)
    node = window.add_text_field(background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24)
    assert isinstance(node, Node)


def test_add_text_field_defaults_to_empty_content():
    window = Window(width=200, height=100)
    field = window.add_text_field(background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24)
    assert field.get_text() == ""


def test_add_text_field_accepts_initial_content():
    window = Window(width=200, height=100)
    field = window.add_text_field(
        background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24, content="hello"
    )
    assert field.get_text() == "hello"


def test_get_text_rejects_a_non_text_field_node():
    window = Window(width=200, height=100)
    rect = window.add_rect(background=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="Rect has no property 'text'"):
        rect.get_text()


def test_a_freshly_created_text_field_is_not_focused():
    window = Window(width=200, height=100)
    field = window.add_text_field(background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24)
    assert field.is_focused() is False


def test_a_real_tab_press_reaches_the_text_field():
    """The same real, already-established §10 focus model every other
    interactive `NodeKind` already uses (`Tree::move_focus`, driven by
    `Window.press_key("tab")`) -- `Window.add_text_field` opts a field
    into it at construction (`Role::TextInput` + `Action::Focus`, see
    `PLAN.md`), the first real `engine-py` caller of `Tree::set_access`
    for an imperative node.
    """
    window = Window(width=200, height=100)
    field = window.add_text_field(background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24)

    window.press_key("tab")

    assert field.is_focused() is True, "a real Tab press must reach the one real TextField"


def test_shift_tab_from_a_focused_field_moves_focus_away():
    window = Window(width=200, height=100)
    field = window.add_text_field(background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24)
    other = window.add_checkbox(background=(0x63, 0x50, 0xA4, 0xFF), width=24, height=24)
    other.set_on_click(lambda: None)  # the established way a node opts into Tab-reachability

    window.press_key("tab")
    assert field.is_focused() is True

    window.press_key("tab")
    assert other.is_focused() is True
    assert field.is_focused() is False, "focus must genuinely move, not stay on both"
