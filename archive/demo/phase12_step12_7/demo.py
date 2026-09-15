#!/usr/bin/env python3
"""Phase 12 Step 12.7 proof: cursor customization and IME opt-in/opt-out,
end to end through `tre_python`, against a real window and a real winit
backend.

Real, disclosed scope limit: this sandbox has no input-injection tool
(no xdotool/wtype/ydotool -- confirmed absent), so a real file drag-drop
or real IME composition cannot be synthesized here to prove event
*delivery* end to end. What this demo proves instead: every new API call
(`set_cursor`, `set_ime_allowed`) succeeds against a real window with no
error, and `poll_events()` keeps working normally with the widened
`InputEvent` type. The event *translation* itself (`WindowEvent::
DroppedFile`/`HoveredFile`/`Ime(..)` -> `InputEvent::FileDropped`/
`FileHovered`/`ImeEnabled`/...) was verified by reading winit 0.30.13's
own real source directly, not assumed -- see
`crates/tre-platform/src/winit_backend.rs`.
"""

import time

import tre_python as tre


def main() -> None:
    renderer = tre.WindowedRenderer("cursor/IME passthrough demo", 300, 200)
    window = renderer.main_window

    # Real cursor changes -- cycle through several real icons.
    icons = [
        tre.CursorIcon.Pointer,
        tre.CursorIcon.Text,
        tre.CursorIcon.Grab,
        tre.CursorIcon.EwResize,
        tre.CursorIcon.NotAllowed,
        tre.CursorIcon.Default,
    ]
    for icon in icons:
        renderer.set_cursor(window, icon)
    print(f"set_cursor: {len(icons)} real icons applied without error -- OK")

    # Real IME opt-in/opt-out toggle (winit requires this before any IME
    # composition event will ever be delivered).
    renderer.set_ime_allowed(window, True)
    renderer.set_ime_allowed(window, False)
    print("set_ime_allowed: real toggle without error -- OK")

    # poll_events() must keep working normally with the widened
    # InputEvent type (7 new variants) -- report whatever real events
    # this run happens to observe.
    seen_event_types: set[str] = set()
    for _ in range(30):
        for event in renderer.poll_events():
            seen_event_types.add(type(event).__name__)
        time.sleep(1 / 60)
    print(f"real InputEvent variants observed this run: {sorted(seen_event_types)}")

    print("tre_python cursor/IME passthrough demo: PASSED")


if __name__ == "__main__":
    main()
