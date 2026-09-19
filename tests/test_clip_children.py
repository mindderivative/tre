"""M32 Phase 3 (§5, §7, §11.7/§11.8): real, repeatable FFI coverage of
`Node.set_clip_children` -- the real, general form of the clip
`VirtualList`/`Carousel` already have built into their own paint,
closing "no `NodeKind` besides `VirtualList` clips its own children
today" (`Code Editor`'s own stated gap, M30 Phase 9 Step 3). The real
per-pixel proof that clipping actually hides an oversized child (and
that leaving it off is a true no-op) is
`crates/engine-render/tests/clip_children.rs` -- not reachable from
Python at all (no pixel-readback API exists here), so this file proves
the real FFI wiring instead: the call doesn't raise, is universal
across `NodeKind`s (unlike `set_syntax_spans`/`set_folded_ranges`,
which are `TextField`-only), and never touches `get_text()`/content.
"""

from tre import Window


def test_set_clip_children_does_not_raise_on_a_plain_rect():
    window = Window(width=400, height=300)
    rect = window.add_rect(background=(255, 0, 0, 255), width=50, height=50)
    rect.set_clip_children(True)
    rect.set_clip_children(False)


def test_set_clip_children_works_on_a_container_with_real_children():
    window = Window(width=400, height=300)
    container = window.add_rect(background=(0, 0, 0, 0), width=100, height=50)
    child = window.add_rect(background=(255, 0, 0, 255), width=100, height=200)
    container.add_child(child)
    container.set_clip_children(True)


def test_set_clip_children_is_not_text_field_specific_unlike_syntax_spans():
    """The real, deliberate contrast with `set_syntax_spans`/
    `set_folded_ranges`: those reject a non-`TextField` node.
    `set_clip_children` must not -- it's a universal `PaintProperties`
    field, not a `TextField`-only one.
    """
    window = Window(width=400, height=300)
    rect = window.add_rect(background=(0, 255, 0, 255), width=50, height=50)
    rect.set_clip_children(True)  # must not raise for a plain Rect

    editor = window.add_code_editor(
        content="def f():\n    pass",
        background=(255, 255, 255, 255),
        width=200,
        height=100,
    )
    editor.set_clip_children(True)  # must not raise for a TextField either
    assert editor.get_text() == "def f():\n    pass", "clipping must never touch real content"
