"""M30 Phase 6 Step 2 (§1, §3, §5, §7): real, repeatable coverage of
`Window.add_accordion_header` -- MD3 has no official Accordion page
(confirmed by pyCopper's own prior research, grounded instead in the
Lists guideline's own "expand and collapse" text). Reuses List Item's
own real anatomy for the header; the collapsible content region has no
distinctive MD3 styling of its own, so this suite doesn't test one --
the app composes it from any existing container.

Also covers a real, confirmed engine limitation this step found and
worked around: `Node.animate("transform", ...)` has no rotation
capability at all (translate+scale only) -- a uniform negative scale
(`-1.0`) is mathematically identical to a 180° rotation for the
curated `expand_more` chevron's own point-symmetric shape. `Node.get`
only ever reads scalar properties (`transform` isn't one, matching
this engine's own established "no pixel/value readback from Python"
limitation elsewhere), so this suite proves the animate call succeeds
end to end, not the resulting matrix value.
"""

from tre import Node, Window


def test_add_accordion_header_returns_header_and_chevron():
    window = Window(width=400, height=200)
    header, chevron = window.add_accordion_header(title="Section 1")
    assert isinstance(header, Node)
    assert isinstance(chevron, Node)


def test_a_themed_accordion_header_does_not_raise():
    window = Window(width=400, height=200)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    header, chevron = window.add_accordion_header(title="Section 1")
    assert isinstance(header, Node)
    assert isinstance(chevron, Node)


def test_expanded_true_does_not_raise():
    window = Window(width=400, height=200)
    header, chevron = window.add_accordion_header(title="Section 1", expanded=True)
    assert isinstance(header, Node)
    assert isinstance(chevron, Node)


def test_the_header_is_a_real_independently_clickable_node():
    window = Window(width=400, height=200)
    header, _chevron = window.add_accordion_header(title="Section 1")

    calls = []
    header.enable_interaction()
    header.set_on_click(lambda: calls.append("toggled"))
    window.click(header)
    assert calls == ["toggled"], "a real click on the header must reach its own registered handler"


def test_the_chevron_can_be_flipped_via_animate_transform():
    """The real, honest workaround this step found: no rotation
    primitive exists, so a uniform negative scale stands in for a
    180-degree flip on this glyph's own point-symmetric shape.
    """
    window = Window(width=400, height=200)
    _header, chevron = window.add_accordion_header(title="Section 1", expanded=False)

    chevron.animate("transform", (0.0, 0.0, -1.0))  # must not raise -- expand
    chevron.animate("transform", (0.0, 0.0, 1.0))  # must not raise -- collapse back
