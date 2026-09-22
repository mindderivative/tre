#!/usr/bin/env python3
"""M55 (§10, §16.2): real `FocusEnter`/`FocusExit` events -- the real,
still-open candidate M54's own scoping explicitly deferred, now closed.

Before M55, keyboard focus genuinely moved (real click-to-focus, real
Tab navigation, a real AccessKit `Action::Focus` request), but nothing
in this codebase ever told a registered handler it happened -- only
`Node.is_focused()`, a plain, un-eventful poll. `FocusEnter`/`FocusExit`
are a real pair, not a single `Focus` kind, mirroring `HoverEnter`/
`HoverExit` exactly -- `HandlerMap`'s own per-node key can never give
one event two real sources, so the node losing focus and the node
gaining it each fire their own handler.

`Window.focus(node)` (new, M55) is the direct, no-live-window-needed
way to request focus explicitly -- there's no real `InputEvent` for
"focus this specific node" (the identical real reason AccessKit's own
`Action::Focus` handling calls the same underlying mechanism directly
too), so this is a genuinely new capability, not just a new event on
an existing action.
"""

from tre import App, Window

window = Window(width=380, height=220, title="tre v2 -- real FocusEnter/FocusExit")
window.set_theme(seed=(0x63, 0x50, 0xA4, 0xFF))

log: list[str] = []


def report(line: str) -> None:
    log.append(line)
    print(line)


field_a = window.add_text_field(
    background=(0xFF, 0xFF, 0xFF, 0xFF), width=150, height=40, content="field a", x=20, y=20
)
field_a.set_on_focus_enter(lambda: report("field a: focus_enter"))
field_a.set_on_focus_exit(lambda: report("field a: focus_exit"))

field_b = window.add_text_field(
    background=(0xFF, 0xFF, 0xFF, 0xFF), width=150, height=40, content="field b", x=190, y=20
)


def on_b_focus_enter(event):
    report(f"field b: {event.kind} (position={event.position}, source is a stable int={isinstance(event.source, int)})")


field_b.set_on_focus_enter(on_b_focus_enter)
field_b.set_on_focus_exit(lambda: report("field b: focus_exit"))

# --- Real, functional, headless-CI-safe verification ---

# Explicit Window.focus() -- the new, direct way to request focus, no
# click/Tab side effect needed.
window.focus(field_a)
assert log[-1] == "field a: focus_enter"

# Real click-to-focus (M18/M30/M53) now genuinely observable.
window.click(field_b)
assert log[-2] == "field a: focus_exit"
assert log[-1].startswith("field b: focus_enter")

# Real right-click-to-focus -- a genuine, found-while-scoping fix this
# milestone made: Window.right_click's own dispatch used to discard
# its outcome with no variable at all, so this was silently
# unobservable before M55.
window.click(field_a)
log.clear()
window.right_click(field_b)
assert log[-2] == "field a: focus_exit"
assert log[-1].startswith("field b: focus_enter")

# Real Tab navigation.
log.clear()
window.press_key("tab")
assert any("focus_enter" in line for line in log)

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"focus_events.py: exited cleanly after 60 frames, {len(log)} events logged")
