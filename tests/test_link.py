"""M30 Phase 8 Step 2 (§5, §7): real, repeatable coverage of
`Window.add_link` -- a real, standalone clickable label, backed by the
new `NodeKind::Link` this step added to `engine-core`. Fulfills a
real, explicit commitment this codebase already made to itself
(Phase 1's own `Tree::hit_test_at` fix, `NodeKind::Text(_) => false`'s
own doc comment): "a future standalone clickable label... gets its own
dedicated `NodeKind`... not a handler bolted onto bare `Text`."
"""

from tre import Node, Window


def test_add_link_returns_a_node():
    window = Window(width=400, height=100)
    link = window.add_link(content="Learn more", width=120)
    assert isinstance(link, Node)


def test_a_themed_link_does_not_raise():
    window = Window(width=400, height=100)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    link = window.add_link(content="Learn more", width=120)
    assert isinstance(link, Node)


def test_a_link_is_a_real_independently_clickable_node():
    """The real point of this step's own new NodeKind: a link must
    genuinely claim its own click -- proven the same real way every
    other independently-clickable node in this catalog already is.
    """
    window = Window(width=400, height=100)
    link = window.add_link(content="Learn more", width=120)

    calls = []
    link.enable_interaction()
    link.set_on_click(lambda: calls.append("clicked"))
    window.click(link)
    assert calls == ["clicked"], "a real click on the link must reach its own registered handler"


def test_a_link_claims_its_own_click_where_a_plain_text_label_would_not():
    """The real, direct contrast this step's own engine-core unit
    test already proves at the Rust level (`link_independently_
    claims_a_hit_where_text_would_defer`), reproduced here through
    the real Python FFI: a bare `add_text` label never independently
    claims its own click (a real, established fact since Phase 1, not
    new to this step) -- a `Link` at otherwise-identical anatomy does.
    """
    window = Window(width=400, height=100)
    label = window.add_text(content="Plain label", foreground=(0, 0, 0, 0), width=120, height=20)
    link = window.add_link(content="Learn more", width=120)

    label_calls = []
    label.enable_interaction()
    label.set_on_click(lambda: label_calls.append("clicked"))
    window.click(label)
    assert label_calls == [], "a bare Text label must never independently claim its own click"

    link_calls = []
    link.enable_interaction()
    link.set_on_click(lambda: link_calls.append("clicked"))
    window.click(link)
    assert link_calls == ["clicked"], "a Link, unlike Text, must claim its own click directly"
