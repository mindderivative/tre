"""M14 Phase 3 (§5, §7.3): real, repeatable coverage of the new
`EventKind::Change` FFI boundary -- `Node.set_on_change`/`Node.
get_checked`, and the real, direct `Change` firing `Node.set_checked`
now does (mirroring `test_checkbox.py`'s own "FFI wiring only" split:
the real *mechanical* Slider-drag-release `Change` path has no Python
entry point at all, per `test_slider.py`'s own note, and stays proven
only by `engine-core`'s own `dispatch_release_ending_a_real_slider_
drag_produces_changed` test).

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_checkbox.py`.
"""

import pytest

from tre import Window


def test_get_checked_reads_back_the_real_checkbox_state():
    window = Window(width=200, height=200)
    checkbox = window.add_checkbox(background=(0x63, 0x50, 0xA4, 0xFF), width=24, height=24)
    assert checkbox.get_checked() is False

    checkbox.set_checked(True)
    assert checkbox.get_checked() is True


def test_get_checked_rejects_a_non_checkbox_node():
    window = Window(width=200, height=200)
    rect = window.add_rect(background=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="Rect has no property 'checked'"):
        rect.get_checked()


def test_set_checked_fires_a_real_registered_on_change_handler():
    window = Window(width=200, height=200)
    checkbox = window.add_checkbox(background=(0x63, 0x50, 0xA4, 0xFF), width=24, height=24)

    calls = []
    checkbox.set_on_change(lambda: calls.append(checkbox.get_checked()))

    checkbox.set_checked(True)
    assert calls == [True], "set_checked(True) must fire the real registered on_change handler"

    checkbox.set_checked(False)
    assert calls == [True, False], "each real set_checked call fires on_change again"


def test_set_checked_with_no_registered_handler_does_not_raise():
    window = Window(width=200, height=200)
    checkbox = window.add_checkbox(background=(0x63, 0x50, 0xA4, 0xFF), width=24, height=24)
    checkbox.set_checked(True)  # must not raise -- an unregistered Change handler is a no-op


def test_a_raising_on_change_handler_is_caught_logged_and_non_fatal(capfd):
    """Matches `Window.click`'s own established policy (§9): an uncaught
    exception from a real handler is caught and logged via `tracing::
    error!` (M16 Phase 2), not propagated -- `call_handler` is the same
    shared mechanism `Node.set_checked` now reuses for a direct `Change`
    fire.

    `capfd`, not `capsys`: `tracing_subscriber`'s own writer is a raw
    OS-level stderr write from Rust, bypassing Python's `sys.stderr`
    object entirely -- `capsys` (which only monkeypatches that Python
    object) can't see it, confirmed by actually running this test with
    `capsys` and watching it fail with an empty capture even though the
    real event was genuinely emitted (`capfd` captures at the file-
    descriptor level, real for both Python and Rust writes).
    """
    window = Window(width=200, height=200)
    checkbox = window.add_checkbox(background=(0x63, 0x50, 0xA4, 0xFF), width=24, height=24)

    def on_change():
        raise RuntimeError("boom")

    checkbox.set_on_change(on_change)
    checkbox.set_checked(True)  # must not raise

    captured = capfd.readouterr()
    assert "boom" in captured.err
    assert "RuntimeError" in captured.err
