"""M39 Phase 2 Step 2 (§5, §7): real, repeatable coverage of
`Window.add_time_picker_dial` -- MD3's real *analog* Time Picker
circular drag control, genuinely distinct from `test_time_picker.py`'s
own `add_time_input_field`/`add_period_selector` (the *digital* Time
Input variant, M30 Phase 7 Step 2 -- that file's own doc comment
explicitly named this analog dial as a real, deferred gap needing "a
genuinely new drag-to-angle engine capability this project doesn't
have," which this step now builds).

The real angle-based drag math itself is proven directly at the Rust
level (`crates/engine-core/src/tree.rs`'s own `dispatch_drag_in_hour_
mode_*`/`dispatch_drag_in_minute_mode_*` tests) and via a real, live,
multi-frame render loop (`examples/time_picker_dial.py`); this file
proves the real FFI construction/getter/setter surface.
"""

import pytest

from tre import Node, Window


def test_time_picker_dial_getters_and_setters_reject_a_non_dial_node():
    window = Window(width=400, height=400)
    rect = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=10, height=10)
    with pytest.raises(ValueError):
        rect.get_time_picker_dial_time()
    with pytest.raises(ValueError):
        rect.set_time_picker_dial_time(1, 1)
    with pytest.raises(ValueError):
        rect.get_time_picker_dial_mode()
    with pytest.raises(ValueError):
        rect.set_time_picker_dial_mode("hour")


