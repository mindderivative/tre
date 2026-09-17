"""M17 Phase 1 (§8): real, repeatable coverage of `Window.copy`/`cut`/
`paste` -- the FFI boundary for a `TextField`'s own clipboard-adjacent
selection handling.

Deliberately **hermetic**: none of these tests touch the real OS
clipboard. The real, live winit-driven path -- `engine-platform::
translate_clipboard_shortcut` detecting a real Ctrl+C/Ctrl+X/Ctrl+V
press and `App::run`'s own `on_input` closure doing the actual
`arboard` I/O -- has no synthetic entry point from Python at all
(confirmed real via `PLAN.md`/`LOG.md`: unlike `press_key`/`type_text`,
which dispatch through `Tree::dispatch` exactly like a real `winit`
event would, a real Ctrl+C only ever originates from an actual OS-level
keyboard event reaching `engine-platform` directly). `Window.copy`/
`cut` instead prove the real, pure `Tree::text_field_selected_text`/
`cut_text_field_selection` read/mutation directly; `Window.paste`
proves the real `InputEvent::TextInput` insertion `type_text` already
established, given an explicit string rather than a real clipboard
read.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as every other FFI test in this suite.
"""

from tre import Window


def select_all(window):
    """Selects a field's own full real content via Home then
    Shift+End -- the same real keyboard path `test_text_field.py`
    already establishes.
    """
    window.press_key("home")
    window.press_key("end", shift=True)


def test_copy_with_no_focused_field_returns_none():
    window = Window(width=200, height=100)
    window.add_text_field(background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24, content="hi")
    assert window.copy() is None


def test_copy_with_no_real_selection_returns_none():
    window = Window(width=200, height=100)
    window.add_text_field(background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24, content="hi")
    window.press_key("tab")
    assert window.copy() is None


def test_copy_reads_the_real_selected_text_without_mutating_it():
    window = Window(width=200, height=100)
    field = window.add_text_field(
        background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24, content="hello"
    )
    window.press_key("tab")
    select_all(window)

    assert window.copy() == "hello"
    assert field.get_text() == "hello", "a real copy must never mutate the field's own content"


def test_cut_reads_and_removes_the_real_selected_text():
    window = Window(width=200, height=100)
    field = window.add_text_field(
        background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24, content="hello"
    )
    window.press_key("tab")
    select_all(window)

    assert window.cut() == "hello"
    assert field.get_text() == "", "a real cut must actually remove the selected text"


def test_cut_with_no_real_selection_returns_none_and_does_not_mutate():
    window = Window(width=200, height=100)
    field = window.add_text_field(
        background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24, content="hello"
    )
    window.press_key("tab")

    assert window.cut() is None
    assert field.get_text() == "hello"


def test_paste_inserts_the_given_text_at_the_real_cursor():
    window = Window(width=200, height=100)
    field = window.add_text_field(
        background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24, content="hello"
    )
    window.press_key("tab")
    window.press_key("home")

    window.paste("XY")

    assert field.get_text() == "XYhello"


def test_paste_replaces_a_real_active_selection():
    window = Window(width=200, height=100)
    field = window.add_text_field(
        background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24, content="hello"
    )
    window.press_key("tab")
    select_all(window)

    window.paste("HI")

    assert field.get_text() == "HI"


def test_copy_fires_no_on_change_but_cut_does():
    """A real Copy is a pure read -- must never fire `Change`. A real
    Cut genuinely edits the field, mirroring Backspace/Delete's own
    established `Change`-firing behavior for a real content change.
    """
    window = Window(width=200, height=100)
    field = window.add_text_field(
        background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24, content="hello"
    )
    window.press_key("tab")

    calls = []
    field.set_on_change(lambda: calls.append(field.get_text()))

    select_all(window)
    window.copy()
    assert calls == [], "Window.copy is a pure read and must not fire on_change"

    window.cut()
    assert calls == [""], "Window.cut genuinely edits the field and must fire on_change"
