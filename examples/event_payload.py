#!/usr/bin/env python3
"""M54 (§8, §16.2): a real Event payload for handlers -- the capability
this milestone exists to build.

Before M54, every registered handler (`Click`/`HoverEnter`/`HoverExit`/
`Change`) was invoked with zero arguments, no matter what actually
happened -- an `on_click` couldn't tell which mouse button fired it or
where, and an `on_change` had no way to see what changed without
re-reading the node's own current state itself (the real workaround
`demo/showcase.py`'s own `toggle_checkbox` still uses, since a `Click`
handler's own "what to toggle to" question isn't something a `Change`
event's `old_value` would answer anyway -- this example doesn't force
that one into a shape it doesn't fit).

Real, deliberate backward compatibility, not a breaking change: a
handler can still declare zero parameters (every example/test in this
catalog before M54 does exactly that, and keeps working unmodified) --
`dispatch::wants_event_payload` arity-sniffs each handler once, at
registration, and only ever calls a handler with the new `Event`
argument when it actually declared it wants one.

M56 (§8, §16.2): `event.node`, added below alongside everything above
-- a real, live `Node` handle for the node an event fired on, additive
next to the existing `event.source: int`. The one real scenario a bare
opaque id can't serve on its own: a *single* handler registered
generically across several nodes, with no way to know which one just
fired without either a live handle or a caller-maintained id lookup of
its own.
"""

from tre import App, Window

window = Window(width=420, height=280, title="tre v2 -- real Event payload")
window.set_theme(seed=(0x63, 0x50, 0xA4, 0xFF))

log: list[str] = []


def report(line: str) -> None:
    log.append(line)
    print(line)


# --- Click: real position + button, only for a real pointer click ---
button = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=140, height=48, x=20, y=20)
button.enable_interaction()


def on_click(event):
    if event.position is not None:
        x, y = event.position
        report(f"click: real {event.button} press at ({x:.0f}, {y:.0f})")
    else:
        report("click: a real keyboard Enter/Space activation -- no pointer data to report")


button.set_on_click(on_click)

# --- HoverEnter/HoverExit: real position, shared by both halves of one real move ---
hover_target = window.add_rect(background=(0xE8, 0xDE, 0xF8, 0xFF), width=100, height=100, x=200, y=20)
hover_target.set_on_hover_enter(lambda event: report(f"hover_enter at {event.position}"))
hover_target.set_on_hover_exit(lambda event: report(f"hover_exit at {event.position}"))

# --- Change: real old/new value, the piece no existing workaround could ever recover ---
field = window.add_text_field(
    background=(0xFF, 0xFF, 0xFF, 0xFF), width=180, height=40, content="edit me", x=20, y=100
)


def on_field_change(event):
    report(f"change: {event.old_value!r} -> {event.new_value!r}")


field.set_on_change(on_field_change)

checkbox = window.add_checkbox(background=(0x63, 0x50, 0xA4, 0xFF), width=24, height=24, x=20, y=160)


def on_checkbox_change(event):
    report(f"change: {event.old_value} -> {event.new_value}")


checkbox.set_on_change(on_checkbox_change)

# A plain zero-argument handler, side by side with the one-argument ones
# above, on its own separate node -- proves the two calling conventions
# genuinely coexist, not that one replaced the other.
legacy_calls: list[str] = []
legacy_button = window.add_rect(background=(0xCC, 0xCC, 0xCC, 0xFF), width=140, height=32, x=20, y=200)
legacy_button.set_on_click(lambda: legacy_calls.append("legacy zero-arg handler still works"))

# --- event.node (M56): one handler, shared across several real nodes,
# telling them apart via event.node's own live state -- the one real
# scenario event.source's bare opaque id can't serve without a caller-
# maintained id lookup of its own.
option_a = window.add_checkbox(background=(0x63, 0x50, 0xA4, 0xFF), width=24, height=24, x=200, y=140)
option_b = window.add_checkbox(background=(0x63, 0x50, 0xA4, 0xFF), width=24, height=24, x=200, y=180)
option_c = window.add_checkbox(background=(0x63, 0x50, 0xA4, 0xFF), width=24, height=24, x=200, y=220)


def on_any_option_clicked(event):
    # event.node is the exact same live handle event.source's own
    # opaque id names -- calling straight back into the tree through it
    # (the same real pattern a handler touching any Node it already
    # holds relies on) is what makes "one handler, many nodes" actually
    # practical: toggle whichever one just fired, read straight back.
    event.node.set_checked(not event.node.get_checked())
    report(f"option toggled -> {event.node.get_checked()}")


for option in (option_a, option_b, option_c):
    option.set_on_click(on_any_option_clicked)

# --- Real, functional, headless-CI-safe verification ---
window.click(button)
assert log[-1].startswith("click: real primary press at")

window.click(legacy_button)
assert legacy_calls == ["legacy zero-arg handler still works"]

window.hover(hover_target)
window.hover(button)
assert log[-2] == "hover_enter at (250.0, 70.0)"
assert log[-1].startswith("hover_exit at")

field.set_text("changed")
assert log[-1] == "change: 'edit me' -> 'changed'"

window.click(field)  # focuses the field
window.press_key("backspace")
assert log[-1] == "change: 'changed' -> 'change'"

checkbox.set_checked(True)
assert log[-1] == "change: False -> True"

window.click(option_b)
assert log[-1] == "option toggled -> True"
assert option_b.get_checked() is True
assert option_a.get_checked() is False and option_c.get_checked() is False, (
    "the shared handler must only ever touch the one real node event.node names"
)

window.click(option_b)
assert log[-1] == "option toggled -> False", "event.node must see the real current value, not a stale one"

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"event_payload.py: exited cleanly after 60 frames, {len(log)} events logged")
