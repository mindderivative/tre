"""M30 Phase 8 Step 5 (§5, §7, §11.2): real, repeatable coverage of
`Window.add_status_bar`. MD3 has no official Status Bar page (confirmed
via the same directory-listing technique this milestone already
uses). Real, deliberate reuse: passes directly into `AppShell`'s own
already-real `build_shell(..., status_bar=...)` parameter -- no new
shell-level wiring, just the real, styled bar content this step adds.
"""

from tre import Node, Window


def test_add_status_bar_returns_a_node():
    window = Window(width=800, height=600)
    bar = window.add_status_bar(text="Ready")
    assert isinstance(bar, Node)


def test_a_themed_status_bar_does_not_raise():
    window = Window(width=800, height=600)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    bar = window.add_status_bar(text="Ready")
    assert isinstance(bar, Node)


def test_add_status_bar_with_an_explicit_width_does_not_raise():
    window = Window(width=800, height=600)
    bar = window.add_status_bar(text="Ready", width=400)
    assert isinstance(bar, Node)


def test_status_bar_can_be_passed_directly_into_build_shell():
    """The real, deliberate reuse this step's own design makes:
    `add_status_bar`'s own return value works directly as
    `build_shell`'s own already-real `status_bar` parameter, no
    adapter needed.
    """
    window = Window(width=800, height=600)
    bar = window.add_status_bar(text="Ready")
    shell = window.build_shell(menu_bar=None, toolbar=None, status_bar=bar)
    assert isinstance(shell, Node)
