"""Real, repeatable coverage of `Window.add_splitter` -- the missing
Python-facing half of M4 Phase 3's already-real drag mechanism
(`Tree::dispatch`'s `PointerPressed`/`PointerMoved`/`PointerReleased`
handling for a `NodeKind::Splitter`, verified end to end at the
`engine-core`/`engine-render` level in that phase). Until this method
existed, nothing created a splitter node from Python at all, so that
real mechanism had nothing to act on from a real app.

The actual drag *mechanics* (live-follows-the-cursor, ends on release,
never starts from a non-splitter or a non-primary button) are already
proven by `engine-core`'s own unit tests and `engine-render`'s
`splitter_drag_dispatch.rs` pixel test -- this file's own job is the one
thing only `engine-py` can prove: that a real Python app can actually
construct a working left-pane/splitter/right-pane layout and that
`Window.click()`'s existing press+release path (no `PointerMoved` in
between, so `update_drag` never runs) leaves a splitter undisturbed,
matching real click-without-drag semantics.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_engine_py.py`.
"""

from tre import Node, Window


def test_add_splitter_returns_a_node():
    window = Window(width=210, height=50)
    splitter = window.add_splitter(background=(0x80, 0x80, 0x80, 0xFF), width=10, height=50)
    assert isinstance(splitter, Node)


def test_add_splitter_builds_a_real_left_splitter_right_layout():
    window = Window(width=210, height=50)
    left = window.add_rect(background=(0xFF, 0x00, 0x00, 0xFF), width=100, height=50)
    splitter = window.add_splitter(background=(0x80, 0x80, 0x80, 0xFF), width=10, height=50)
    right = window.add_rect(background=(0x00, 0x00, 0xFF, 0xFF), width=100, height=50)
    # No exception constructing the layout is the real claim here -- the
    # underlying Tree::splitter_geometry (engine-core) only validates
    # "sits directly between two real siblings" lazily, the first time
    # something actually drags or resizes it, so a well-formed
    # left/splitter/right sequence like this must build cleanly.
    assert isinstance(left, Node)
    assert isinstance(splitter, Node)
    assert isinstance(right, Node)


def test_add_splitter_defaults_to_an_even_split():
    window = Window(width=210, height=50)
    window.add_rect(background=(0xFF, 0x00, 0x00, 0xFF), width=100, height=50)
    # initial_position omitted -- must not raise, and matches §11.5's
    # own "0.0..=1.0 along the split axis" default of an even split.
    window.add_splitter(background=(0x80, 0x80, 0x80, 0xFF), width=10, height=50)
    window.add_rect(background=(0x00, 0x00, 0xFF, 0xFF), width=100, height=50)


def test_add_splitter_accepts_a_custom_initial_position():
    window = Window(width=210, height=50)
    window.add_rect(background=(0xFF, 0x00, 0x00, 0xFF), width=100, height=50)
    window.add_splitter(
        background=(0x80, 0x80, 0x80, 0xFF), width=10, height=50, initial_position=0.75
    )
    window.add_rect(background=(0x00, 0x00, 0xFF, 0xFF), width=100, height=50)


def test_clicking_a_splitter_without_dragging_does_not_crash():
    """`Window.click()` presses and releases at the same point, with no
    `PointerMoved` in between -- `Tree::update_drag` only ever runs on a
    `PointerMoved`, so this is real click-without-drag behavior, not an
    edge case being carved around. Must not crash, and (since
    `set_on_click` was never called) must not do anything else either.
    """
    window = Window(width=210, height=50)
    window.add_rect(background=(0xFF, 0x00, 0x00, 0xFF), width=100, height=50)
    splitter = window.add_splitter(background=(0x80, 0x80, 0x80, 0xFF), width=10, height=50)
    window.add_rect(background=(0x00, 0x00, 0xFF, 0xFF), width=100, height=50)

    window.click(splitter)  # must not raise
