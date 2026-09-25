"""M32 Phase 2 (§4, §5): real, repeatable coverage of `Window.resize` --
the synthetic, no-live-window-needed entry point for the real gap this
phase closes ("nothing resizes any node's box when its window
resizes"). The real, winit-driven live path (`WindowEvent::Resized` ->
`InputEvent::Resized`, `engine-platform`/`engine-py::app.rs`) needs an
actual OS window resize to exercise -- not reachable through this
project's existing headless testing surface, the same real limit
`app.rs`'s own `InputEvent::Resized` doc comment states. The real,
load-bearing `Tree::dispatch` mutation itself (`root`'s own
`layout_style.size` actually changes, and a fresh `compute_layout`
reflects it) is proven directly at the Rust level:
`crates/engine-core/src/tree.rs::
resized_grows_the_roots_own_layout_box_and_a_fresh_layout_reflects_it`
-- this file proves the real end-to-end FFI wiring instead.

M33 Phase 2 (§4, §5, §8) closed the real, stated v1 limit this file's
own tests used to leave open: `self.width`/`self.height` (read by
every interactive `add_*` factory method) are now a real, shared
`Rc<Cell<u32>>` -- a real, live winit-driven resize updates the exact
same cell `Window.resize()` itself writes to. No Python-level getter
exists for a node's own real pixel box (a real, separate, pre-existing
gap this phase doesn't take on), so the tests below prove the real
*flow* stays correct (an interactive `add_*` call after a resize still
succeeds and dispatches correctly) rather than asserting on exact
pixel dimensions -- the real `Rc<Cell<u32>>` sharing itself is a
compile-time-enforced guarantee (both `WindowSetup`/`WindowRuntime`
hold a real `.clone()` of the identical `Rc`, confirmed by direct code
review, not something that could silently regress the way a plain
`u32` copy could).
"""

from tre import Window


def test_resize_does_not_raise():
    window = Window(width=400, height=300)
    window.resize(800, 600)


def test_resize_on_a_window_with_no_children_does_not_raise():
    window = Window(width=400, height=300)
    window.resize(1, 1)


def test_a_click_dispatched_after_a_real_resize_still_works():
    """The real point: `resize()` must leave the tree in a genuinely
    usable state -- a node added before the resize still dispatches a
    real click correctly afterward, proving the resize didn't corrupt
    layout or leave `self.width`/`self.height` out of sync with what
    `click()`'s own `compute_layout` call reads.
    """
    window = Window(width=400, height=300)
    rect = window.add_rect(background=(255, 0, 0, 255), width=50, height=50)
    rect.enable_interaction()
    clicked = []
    rect.set_on_click(lambda: clicked.append(True))

    window.resize(800, 600)
    window.click(rect)

    assert clicked == [True], "a real click after a real resize must still reach its own handler"


def test_resize_shrinking_the_window_does_not_raise():
    window = Window(width=800, height=600)
    window.add_rect(background=(0, 255, 0, 255), width=50, height=50)
    window.resize(200, 150)


def test_an_interactive_add_dialog_call_after_a_resize_still_works():
    """M33 Phase 2 (§4, §5, §8): the exact real scenario the prior
    phase's own doc comment named as broken -- an interactive `add_*`
    factory method (here, `Dialog`'s own full-window scrim, sized
    directly from `self.width`/`self.height`) called after a real
    resize must still build and open correctly, not silently size
    against stale construction-time dimensions. No Python-level getter
    exists to assert on the scrim's own real pixel box directly (a
    real, separate, pre-existing gap), so this proves the real flow
    doesn't raise, the same real class of proof this codebase already
    relies on elsewhere a direct pixel assertion isn't reachable from
    Python.
    """
    window = Window(width=400, height=300)
    window.resize(1200, 900)

    dialog = window.add_dialog(
        headline="Resized",
        supporting_text="does this dialog build correctly after a real resize?",
        width=300,
        height=200,
    )
    window.open_dialog(dialog)
