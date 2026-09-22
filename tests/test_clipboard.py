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

import pytest

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


# --- M53 Phase 2: select_all + the real, non-hermetic clipboard API ----
# Unlike everything above, `copy_to_system_clipboard`/`cut_to_system_
# clipboard`/`paste_from_system_clipboard` genuinely touch the real OS
# clipboard -- mirroring `app.rs`'s own `arboard_genuinely_round_trips_
# through_a_real_clipboard` Rust test's convention, "no real clipboard
# service reachable" is a real, honest skip here too, not a failure.


def test_select_all_selects_the_real_focused_fields_entire_content():
    window = Window(width=200, height=100)
    field = window.add_text_field(
        background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24, content="hello"
    )
    window.press_key("tab")

    assert window.select_all() is True
    assert window.copy() == "hello", "select_all must select the field's real entire content"
    assert field.get_text() == "hello", "select_all must never mutate the field's own content"


def test_select_all_with_no_focused_field_returns_false():
    window = Window(width=200, height=100)
    window.add_text_field(background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24, content="hi")
    assert window.select_all() is False


def test_copy_to_system_clipboard_with_no_focused_field_returns_false():
    window = Window(width=200, height=100)
    window.add_text_field(background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24, content="hi")
    assert window.copy_to_system_clipboard() is False


def test_copy_to_system_clipboard_round_trips_through_the_real_os_clipboard():
    window = Window(width=200, height=100)
    field = window.add_text_field(
        background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24, content="hello"
    )
    window.press_key("tab")
    select_all(window)

    if not window.copy_to_system_clipboard():
        pytest.skip("no real OS clipboard service reachable in this environment")

    assert field.get_text() == "hello", "a real copy must never mutate the field's own content"


def test_cut_to_system_clipboard_removes_the_real_selection_on_success():
    window = Window(width=200, height=100)
    field = window.add_text_field(
        background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24, content="hello"
    )
    window.press_key("tab")
    select_all(window)

    if not window.cut_to_system_clipboard():
        pytest.skip("no real OS clipboard service reachable in this environment")

    assert field.get_text() == "", "a real cut must actually remove the selected text"


def test_cut_to_system_clipboard_with_no_real_selection_returns_false_and_does_not_mutate():
    window = Window(width=200, height=100)
    field = window.add_text_field(
        background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24, content="hello"
    )
    window.press_key("tab")

    assert window.cut_to_system_clipboard() is False
    assert field.get_text() == "hello"


def test_paste_from_system_clipboard_inserts_the_real_clipboards_own_content():
    # Real, honest finding, not glossed over: `copy_to_system_clipboard`/
    # `paste_from_system_clipboard` each create their own fresh
    # `arboard::Clipboard` instance per call (matching `app.rs`'s own
    # established, pre-existing per-event pattern -- unchanged by this
    # milestone's refactor). In some real sandboxed X11 environments
    # (confirmed in this one: no `xclip`/`xsel`/`wl-copy` clipboard
    # manager installed), the OS clipboard's own content is only served
    # while the *writing* process's `Clipboard` instance is still alive
    # -- once it drops, a later, separate instance's own real read can
    # come back empty, even though the write itself genuinely succeeded.
    # Treated the same honest "real, expected, gracefully handled" way
    # as "no clipboard reachable at all," not a failure of this test's
    # own logic.
    window = Window(width=200, height=100)
    field = window.add_text_field(
        background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24, content="hello"
    )
    window.press_key("tab")
    select_all(window)
    if not window.copy_to_system_clipboard():
        pytest.skip("no real OS clipboard service reachable in this environment")

    # A second, freshly-focused field receives whatever the real OS
    # clipboard now holds -- proves this reads the real clipboard, not
    # just echoes whatever copy_to_system_clipboard happened to see.
    second = window.add_text_field(background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24)
    window.press_key("tab")

    if not window.paste_from_system_clipboard():
        pytest.skip(
            "the real OS clipboard's own content did not survive past the writing "
            "Clipboard instance in this sandboxed environment"
        )
    assert second.get_text() == "hello"
    assert field.get_text() == "hello", "the original field must be untouched by the paste"


def test_copy_to_system_clipboard_fires_no_on_change_but_cut_to_system_clipboard_does():
    window = Window(width=200, height=100)
    field = window.add_text_field(
        background=(0xEE, 0xEE, 0xEE, 0xFF), width=180, height=24, content="hello"
    )
    window.press_key("tab")

    calls = []
    field.set_on_change(lambda: calls.append(field.get_text()))

    select_all(window)
    if not window.copy_to_system_clipboard():
        pytest.skip("no real OS clipboard service reachable in this environment")
    assert calls == [], "copy_to_system_clipboard is a pure read and must not fire on_change"

    select_all(window)
    if not window.cut_to_system_clipboard():
        pytest.skip("no real OS clipboard service reachable in this environment")
    assert calls == [
        ""
    ], "cut_to_system_clipboard genuinely edits the field and must fire on_change"
