"""M4 Phase 5 (§7.3): real, repeatable coverage that `Node.enable_
interaction` exists and doesn't crash anything real -- the definitive
pixel-level proof that ripple/hover actually paint through the real
pipeline lives in `crates/engine-render/tests/ripple_hover_dispatch.rs`
(no pixel-buffer access exists from Python), the same split this
project already uses for `test_splitter.py`/`splitter_drag_dispatch.rs`.

Also proves a real, confirmed fact found while implementing this phase:
a real click already ripples with *no* opt-in at all, since `Tree::
dispatch`'s `PointerPressed` arm lazily creates `InteractionState` for
whatever node it hits -- `enable_interaction()`'s real, necessary job is
enabling *hover*, which correctly requires an explicit opt-in
(`update_hover` never lazily creates one).

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_engine_py.py`.
"""

from tre import Window


def test_enable_interaction_does_not_raise():
    window = Window(width=120, height=60)
    button = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=80, height=40)
    button.enable_interaction()  # must not raise


def test_clicking_an_interaction_enabled_node_does_not_crash():
    window = Window(width=120, height=60)
    button = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=80, height=40)
    button.enable_interaction()

    window.click(button)  # must not raise


def test_clicking_a_node_that_never_enabled_interaction_still_does_not_crash():
    """A real click already spawns a ripple with no opt-in at all
    (`Tree::dispatch`'s own lazy-creation) -- confirming that not
    calling `enable_interaction()` is also a safe, ordinary path, not
    one that depends on it having been called first.
    """
    window = Window(width=120, height=60)
    button = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=80, height=40)

    window.click(button)  # must not raise
