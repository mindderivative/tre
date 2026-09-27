"""Single-line text inputs: creation and initial `text`, focus by Tab, click,
or secondary click, typing, Backspace/Delete, caret keys, Shift-selection,
`multiline`/`show_whitespace`, `set(text=...)` firing no `change`, and which
edits do fire `change`. The caret's pixels are `text_field_paint.rs`'s job.
"""

import pytest

from tre import Node, Window
from helpers import add


def test_create_text_input_returns_a_node():
    window = Window(width=200, height=100)
    node = add(window, "text_input", width=180, height=24)
    assert isinstance(node, Node)


def test_create_text_input_defaults_to_empty_text():
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=180, height=24)
    assert field.get("text") == ""


def test_create_text_input_accepts_initial_text():
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=180, height=24, text="hello")
    assert field.get("text") == "hello"


def test_reading_text_from_a_box_raises():
    window = Window(width=200, height=100)
    rect = add(window, "box", fill=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="`text` applies only to a text or text_input node"):
        rect.get("text")


def test_a_freshly_created_text_input_is_not_focused():
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=180, height=24)
    assert field.get("focused") is False


def test_a_tab_press_focuses_the_text_input():
    """A text input is in the Tab order from creation."""
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=180, height=24)

    window.simulate("key_down", key="tab")

    assert field.get("focused") is True, "a Tab press must reach the one text input"


def test_a_click_focuses_the_text_input():
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=180, height=24)
    assert field.get("focused") is False

    window.simulate("click", node=field)

    assert field.get("focused") is True, "a click on a text input must move focus there"


def test_a_secondary_click_focuses_the_text_input():
    """So a context menu opened by that click acts on this field, not
    whichever one was last clicked."""
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=180, height=24)
    assert field.get("focused") is False

    window.simulate("secondary_click", node=field)

    assert field.get("focused") is True, "a secondary click on a text input must move focus there"


def test_input_inserts_into_the_focused_field():
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=180, height=24)
    window.simulate("key_down", key="tab")

    window.simulate("input", text="hi")

    assert field.get("text") == "hi"


def test_input_with_no_focused_field_is_a_safe_no_op():
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=180, height=24, text="untouched")
    window.simulate("input", text="x")  # must not raise, and must not touch the unfocused field
    assert field.get("text") == "untouched"


# --- multiline / show_whitespace --------------------------------------------


def test_a_text_input_defaults_to_single_line_enter_does_not_insert_a_newline():
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=180, height=24)
    window.simulate("key_down", key="tab")
    window.simulate("input", text="hi")
    window.simulate("key_down", key="enter")
    assert field.get("text") == "hi", "single-line is the default"


def test_multiline_true_makes_enter_insert_a_newline():
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=180, height=60, multiline=True)
    window.simulate("key_down", key="tab")
    window.simulate("input", text="hi")
    window.simulate("key_down", key="enter")
    window.simulate("input", text="there")
    assert field.get("text") == "hi\nthere", (
        "multiline=True must make Enter insert a newline"
    )


def test_show_whitespace_does_not_raise():
    # Paint-only; test_multiline_input.py checks it leaves the text alone.
    window = Window(width=200, height=100)
    add(window, "text_input", width=180, height=24, show_whitespace=True)


def test_backspace_and_delete_edit_the_focused_field():
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=180, height=24, text="hello")
    window.simulate("key_down", key="tab")

    window.simulate("key_down", key="backspace")
    assert field.get("text") == "hell"

    window.simulate("key_down", key="home")
    window.simulate("key_down", key="delete")
    assert field.get("text") == "ell"


def test_arrow_and_home_end_keys_move_the_cursor_without_changing_content():
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=180, height=24, text="hello")
    window.simulate("key_down", key="tab")

    window.simulate("key_down", key="home")
    window.simulate("key_down", key="arrow_right")
    window.simulate("input", text="X")

    assert field.get("text") == "hXello", "the cursor must have genuinely moved before typing"


def test_set_text_overwrites_content_and_fires_no_change():
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=180, height=24)

    calls = []
    field.on("change", lambda: calls.append(field.get("text")))

    field.set(text="hello")

    assert field.get("text") == "hello"
    assert calls == [], "set(text=...) is the program's own write; change is for user edits"


def test_setting_text_on_a_box_raises():
    window = Window(width=200, height=100)
    rect = add(window, "box", fill=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="`text` applies only to a text or text_input node"):
        rect.set(text="nope")


def test_typing_fires_change():
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=180, height=24)
    window.simulate("key_down", key="tab")

    calls = []
    field.on("change", lambda: calls.append(field.get("text")))

    window.simulate("input", text="a")
    window.simulate("input", text="b")

    assert calls == ["a", "ab"], "each edit must fire change again, with the current text"


def test_pure_cursor_movement_does_not_fire_change():
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=180, height=24, text="hi")
    window.simulate("key_down", key="tab")

    calls = []
    field.on("change", lambda: calls.append("called"))

    window.simulate("key_down", key="arrow_left")
    window.simulate("key_down", key="arrow_right")
    window.simulate("key_down", key="home")
    window.simulate("key_down", key="end")

    assert calls == [], "pure cursor navigation must not fire change -- the text never changed"


def test_backspace_at_start_and_delete_at_end_do_not_fire_change():
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=180, height=24, text="hi")
    window.simulate("key_down", key="tab")

    calls = []
    field.on("change", lambda: calls.append("called"))

    window.simulate("key_down", key="home")
    window.simulate("key_down", key="backspace")  # already at start -- a no-op
    window.simulate("key_down", key="end")
    window.simulate("key_down", key="delete")  # already at end -- a no-op

    assert calls == [], "a no-op edit must not fire change"


def test_shift_arrow_selects_and_backspace_deletes_the_selected_range():
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=180, height=24, text="hello")
    window.simulate("key_down", key="tab")

    window.simulate("key_down", key="home")
    window.simulate("key_down", key="arrow_right", shift=True)
    window.simulate("key_down", key="arrow_right", shift=True)  # selects "he"

    window.simulate("key_down", key="backspace")

    assert field.get("text") == "llo", "Backspace over a selection must delete the whole range"


def test_typing_over_a_selection_replaces_it():
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=180, height=24, text="hello")
    window.simulate("key_down", key="tab")

    window.simulate("key_down", key="home")
    window.simulate("key_down", key="arrow_right", shift=True)
    window.simulate("key_down", key="arrow_right", shift=True)  # selects "he"

    window.simulate("input", text="HI")

    assert field.get("text") == "HIllo"
