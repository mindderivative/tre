"""M30 Phase 4 Step 1 (§5, §7, §11.3): real, repeatable coverage of
`Window.add_dialog`/`open_dialog`/`close_dialog` -- the FFI boundary
for a real MD3 modal dialog, and the real, confirmed engine-core fix
this step needed: `OverlayMeta.modal` genuinely blocks interaction
with everything behind an open dialog, not just that the API
compiles.
"""

import pytest

from tre import Node, Window


def test_add_dialog_returns_a_node():
    window = Window(width=300, height=300)
    node = window.add_dialog(headline="Delete file?", text="This cannot be undone.", width=240, height=140)
    assert isinstance(node, Node)


def test_a_themed_dialog_does_not_raise():
    window = Window(width=300, height=300)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    node = window.add_dialog(headline="Delete file?", text="This cannot be undone.", width=240, height=140)
    assert isinstance(node, Node)


def test_open_dialog_and_close_dialog_do_not_raise():
    window = Window(width=300, height=300)
    dialog = window.add_dialog(headline="Title", text="Body", width=240, height=140)

    window.open_dialog(dialog)  # must not raise
    window.open_dialog(dialog)  # a real, safe no-op -- already open
    window.close_dialog(dialog)  # must not raise


def test_reopening_a_dialog_after_closing_it_does_not_raise():
    window = Window(width=300, height=300)
    dialog = window.add_dialog(headline="Title", text="Body", width=240, height=140)

    window.open_dialog(dialog)
    window.close_dialog(dialog)
    window.open_dialog(dialog)  # must not raise -- real reopen, not a stale/destroyed node


def test_open_dialog_rejects_a_dialog_from_a_different_window():
    window_a = Window(width=300, height=300)
    window_b = Window(width=300, height=300)
    foreign_dialog = window_b.add_dialog(headline="Title", text="Body", width=240, height=140)
    with pytest.raises(ValueError, match="different Window"):
        window_a.open_dialog(foreign_dialog)


def test_an_open_modal_dialog_blocks_a_real_click_on_the_background():
    """The real, confirmed engine-core fix this step made: a real
    click on a background button, while a modal dialog is open, must
    not reach that button's own handler -- proven via a real click
    dispatch, not just that `open_dialog` doesn't raise.
    """
    window = Window(width=300, height=300)
    background_button = window.add_rect(background=(0, 0, 0, 255), width=280, height=280)
    background_button.enable_interaction()
    calls = []
    background_button.set_on_click(lambda: calls.append("clicked"))

    # Sanity: the background button is genuinely clickable before any
    # dialog is open.
    window.click(background_button)
    assert calls == ["clicked"]
    calls.clear()

    dialog = window.add_dialog(headline="Title", text="Body", width=100, height=80)
    window.open_dialog(dialog)

    # A point on the background button, well outside the dialog's own
    # centered panel -- must not reach the button's handler now.
    window.click(background_button)
    assert calls == [], (
        "a real click on a background node must not reach its own handler while a modal "
        "dialog is open"
    )


def test_after_closing_the_dialog_the_background_is_clickable_again():
    window = Window(width=300, height=300)
    background_button = window.add_rect(background=(0, 0, 0, 255), width=280, height=280)
    background_button.enable_interaction()
    calls = []
    background_button.set_on_click(lambda: calls.append("clicked"))

    dialog = window.add_dialog(headline="Title", text="Body", width=100, height=80)
    window.open_dialog(dialog)
    window.close_dialog(dialog)

    window.click(background_button)
    assert calls == ["clicked"], "the background must be clickable again once the dialog is closed"
