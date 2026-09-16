"""M6 Phase 3 (§8): real, repeatable coverage of `Window.add_rect`/
`add_canvas`'s new optional `x`/`y` kwargs -- `Position::Absolute`
exposure from Python, the concrete blocker M5 Phase 4 hit while trying
to write a positioned node-graph example.

No Python-level pixel readback exists anywhere in this project (M6
Phase 2's own corrected scope finding) and there is no raw-coordinate
hit-test entry point either -- `Window.click(node)` always resolves to
`node`'s *own* current center point before dispatching. So these tests
prove positioning the same real way every other FFI test in this suite
proves a claim: through real dispatch, not introspection. A node with
no explicit `x`/`y` sits at its default flex-row position (the window's
own root padding-box origin, `(16, 16)`, for the first child -- real
data from `PyWindow::new`'s own `PADDING` constant). A second node
explicitly positioned via `x=16, y=16` (added afterward, so topmost in
paint order) overlaps it exactly *only if* the explicit position
genuinely took effect -- `window.click(the_first_node)` resolves to the
first node's own center, but real hit-testing at that point then finds
whichever node is actually there, topmost-wins. If positioning silently
fell back to the old flex-row-only behavior, the second node would
instead sit in-flow (not at `(16, 16)`), and the click would still
reach the first node instead.
"""

from tre import Window


def test_add_rect_with_explicit_position_overlaps_the_default_flow_position():
    window = Window(width=200, height=200)

    hits = []
    default_positioned = window.add_rect(background=(0, 0, 0, 255), width=40, height=40)
    default_positioned.set_on_click(lambda: hits.append("default"))

    # PADDING = 16.0 (PyWindow::new) -- the first, unpositioned child's
    # own real, default flex-row position.
    explicitly_positioned = window.add_rect(
        background=(255, 0, 0, 255), width=40, height=40, x=16.0, y=16.0
    )
    explicitly_positioned.set_on_click(lambda: hits.append("explicit"))

    window.click(default_positioned)

    assert hits == ["explicit"], (
        "the explicitly-positioned (topmost) node must win real hit-testing at the "
        f"default node's own center point, got {hits!r}"
    )


def test_add_rect_without_x_or_y_is_unchanged():
    """The real backward-compatibility claim: omitting `x`/`y` entirely
    must behave byte-for-byte like before this phase -- a plain node in
    the implicit flex-row flow, clickable at its own resolved position."""
    window = Window(width=200, height=200)
    hits = []
    node = window.add_rect(background=(0, 0, 0, 255), width=40, height=40)
    node.set_on_click(lambda: hits.append(True))

    window.click(node)

    assert hits == [True]


def test_add_canvas_with_explicit_position_overlaps_the_default_flow_position():
    window = Window(width=200, height=200)

    hits = []
    default_positioned = window.add_rect(background=(0, 0, 0, 255), width=40, height=40)
    default_positioned.set_on_click(lambda: hits.append("default"))

    explicitly_positioned = window.add_canvas(
        width=40, height=40, draw=lambda ctx: None, x=16.0, y=16.0
    )
    explicitly_positioned.set_on_click(lambda: hits.append("explicit"))

    window.click(default_positioned)

    assert hits == ["explicit"], (
        "an explicitly-positioned Canvas (topmost) must win real hit-testing at the "
        f"default node's own center point, got {hits!r}"
    )
