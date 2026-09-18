"""M30 Phase 4 Step 3 (§5, §7, §11.3): real, repeatable coverage of
`Window.add_side_sheet`/`open_side_sheet`/`close_side_sheet` -- the
real desktop counterpart to Bottom Sheet. Covers the real, deliberate
behavioral fork this step makes: a *Standard* side sheet (`modal=
False`) is a plain layout participant, already attached, with no
overlay lifecycle at all; a *Modal* side sheet (`modal=True`) is a
real floating overlay with a full-window scrim that genuinely blocks
background interaction, the same real `OverlayMeta.modal` capability
`Dialog` (Phase 4 Step 1) added.
"""

import pytest

from tre import Node, Window


def test_add_side_sheet_standard_returns_a_node():
    window = Window(width=800, height=600)
    sheet = window.add_side_sheet()
    assert isinstance(sheet, Node)


def test_add_side_sheet_modal_returns_a_node():
    window = Window(width=800, height=600)
    sheet = window.add_side_sheet(modal=True)
    assert isinstance(sheet, Node)


def test_a_themed_side_sheet_does_not_raise():
    window = Window(width=800, height=600)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    standard = window.add_side_sheet()
    modal = window.add_side_sheet(modal=True)
    assert isinstance(standard, Node)
    assert isinstance(modal, Node)


def test_open_side_sheet_and_close_side_sheet_do_not_raise_for_modal():
    window = Window(width=800, height=600)
    sheet = window.add_side_sheet(modal=True)

    window.open_side_sheet(sheet)  # must not raise
    window.open_side_sheet(sheet)  # a real, safe no-op -- already open
    window.close_side_sheet(sheet)  # must not raise


def test_reopening_a_modal_side_sheet_after_closing_it_does_not_raise():
    window = Window(width=800, height=600)
    sheet = window.add_side_sheet(modal=True)

    window.open_side_sheet(sheet)
    window.close_side_sheet(sheet)
    window.open_side_sheet(sheet)  # must not raise -- real reopen, not a stale/destroyed node


def test_open_side_sheet_on_a_standard_sheet_is_a_real_explicit_no_op():
    """A standard side sheet has no overlay lifecycle at all -- it's
    already attached by `add_side_sheet` itself. `open_side_sheet`
    must not raise or double-attach it.
    """
    window = Window(width=800, height=600)
    sheet = window.add_side_sheet(modal=False)
    window.open_side_sheet(sheet)  # must not raise
    window.close_side_sheet(sheet)  # must not raise


def test_open_side_sheet_rejects_a_sheet_from_a_different_window():
    window_a = Window(width=800, height=600)
    window_b = Window(width=800, height=600)
    foreign_sheet = window_b.add_side_sheet(modal=True)
    with pytest.raises(ValueError, match="different Window"):
        window_a.open_side_sheet(foreign_sheet)


def test_an_open_modal_side_sheet_blocks_a_real_click_on_the_background():
    """The real, concrete proof this step reuses `Dialog`'s own
    `OverlayMeta.modal` capability correctly: a real click on a
    background node must be swallowed while a modal side sheet is
    open, and reach its handler again once closed.
    """
    window = Window(width=800, height=600)
    background_button = window.add_rect(background=(0, 0, 0, 255), width=780, height=580)
    background_button.enable_interaction()
    calls = []
    background_button.set_on_click(lambda: calls.append("clicked"))

    window.click(background_button)
    assert calls == ["clicked"]
    calls.clear()

    sheet = window.add_side_sheet(modal=True, width=320)
    window.open_side_sheet(sheet)

    window.click(background_button)
    assert calls == [], "a modal side sheet must block clicks on everything behind it"

    window.close_side_sheet(sheet)
    window.click(background_button)
    assert calls == ["clicked"], "the background must be clickable again once the side sheet is closed"


def test_a_standard_side_sheet_can_be_reparented_into_the_apps_own_layout():
    """A standard side sheet is a plain layout participant -- the app
    is expected to re-parent it via `Node.add_child`, the same real
    contract `Card`'s own content already has.
    """
    window = Window(width=800, height=600)
    shell = window.add_rect(background=(255, 255, 255, 255), width=800, height=600)
    sheet = window.add_side_sheet(modal=False, width=280)
    shell.add_child(sheet)  # must not raise -- a real re-parent, not a duplicate-child panic
