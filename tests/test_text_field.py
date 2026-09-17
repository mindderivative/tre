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


def test_type_text_inserts_into_the_focused_field():
    window = Window(width=200, height=100)
    field = window.add_text_field(background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24)
    window.press_key("tab")

    window.type_text("hi")

    assert field.get_text() == "hi"


def test_type_text_with_no_focused_field_is_a_safe_no_op():
    window = Window(width=200, height=100)
    field = window.add_text_field(
        background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24, content="untouched"
    )
    window.type_text("x")  # must not raise, and must not touch the unfocused field
    assert field.get_text() == "untouched"


def test_backspace_and_delete_edit_the_real_focused_field():
    window = Window(width=200, height=100)
    field = window.add_text_field(
        background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24, content="hello"
    )
    window.press_key("tab")

    window.press_key("backspace")
    assert field.get_text() == "hell"

    window.press_key("home")
    window.press_key("delete")
    assert field.get_text() == "ell"


def test_arrow_and_home_end_keys_move_the_cursor_without_changing_content():
    window = Window(width=200, height=100)
    field = window.add_text_field(
        background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24, content="hello"
    )
    window.press_key("tab")

    window.press_key("home")
    window.press_key("right")
    window.type_text("X")

    assert field.get_text() == "hXello", "the cursor must have genuinely moved before typing"


def test_set_text_overwrites_content_and_fires_on_change():
    window = Window(width=200, height=100)
    field = window.add_text_field(background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24)

    calls = []
    field.set_on_change(lambda: calls.append(field.get_text()))

    field.set_text("hello")

    assert field.get_text() == "hello"
    assert calls == ["hello"], "set_text must fire a real on_change handler, mirroring set_checked"


def test_set_text_rejects_a_non_text_field_node():
    window = Window(width=200, height=100)
    rect = window.add_rect(background=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="Rect has no property 'text'"):
        rect.set_text("nope")


def test_typing_fires_a_real_on_change_handler():
    window = Window(width=200, height=100)
    field = window.add_text_field(background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24)
    window.press_key("tab")

    calls = []
    field.set_on_change(lambda: calls.append(field.get_text()))

    window.type_text("a")
    window.type_text("b")

    assert calls == ["a", "ab"], "each real edit must fire on_change again, with the real current text"


def test_pure_cursor_movement_does_not_fire_on_change():
    window = Window(width=200, height=100)
    field = window.add_text_field(
        background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24, content="hi"
    )
    window.press_key("tab")

    calls = []
    field.set_on_change(lambda: calls.append("called"))

    window.press_key("left")
    window.press_key("right")
    window.press_key("home")
    window.press_key("end")

    assert calls == [], "pure cursor navigation must not fire Change -- content never changed"


def test_backspace_at_start_and_delete_at_end_do_not_fire_on_change():
    window = Window(width=200, height=100)
    field = window.add_text_field(
        background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24, content="hi"
    )
    window.press_key("tab")

    calls = []
    field.set_on_change(lambda: calls.append("called"))

    window.press_key("home")
    window.press_key("backspace")  # already at start -- a real no-op
    window.press_key("end")
    window.press_key("delete")  # already at end -- a real no-op

    assert calls == [], "a genuine no-op edit must not fire Change"
