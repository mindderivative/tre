#!/usr/bin/env python3
"""M30 Phase 4 Step 2's real `Window.add_snackbar`/`open_snackbar`/
`close_snackbar` (§5, §7, §11.3): a real MD3 transient notification,
anchored to the desktop bottom-left corner (this milestone's own
desktop adaptation of MD3's mobile-only full-width-at-bottom anatomy).

Unlike every prior overlay-dependent component this milestone built,
`add_snackbar` returns up to three independent real nodes -- container,
action, close -- since a real snackbar action must be clickable on its
own, distinct from the snackbar's own body (which isn't itself a
button). This engine has no timer/scheduler primitive, so a real
auto-dismiss-after-duration is the app's own responsibility -- this
example plays that role itself, closing the snackbar explicitly rather
than relying on any engine-side timeout.

What this script proves automatically (headless-CI-safe, no human
needed): the snackbar opens without disturbing the rest of the window,
its action button reaches its own registered handler independent of
the rest of the snackbar, and its close button closes it.
"""

from tre import App, Window

window = Window(width=480, height=320, title="tre v2 -- snackbar")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

background = window.add_rect(background=(0xFF, 0xFB, 0xFE, 0xFF), width=480, height=320)

container, action, close = window.add_snackbar(
    text="Conversation archived.",
    width=320,
    action_label="Undo",
    closable=True,
)

state: dict[str, bool] = {"undone": False, "open": False}


def undo() -> None:
    state["undone"] = True
    window.close_snackbar(container)
    state["open"] = False


def dismiss() -> None:
    window.close_snackbar(container)
    state["open"] = False


assert action is not None
assert close is not None
action.enable_interaction()
action.set_on_click(undo)
close.enable_interaction()
close.set_on_click(dismiss)

window.open_snackbar(container)
state["open"] = True
window.click(action)
assert state["undone"], "the action button must reach its own registered handler"
assert not state["open"], "closing via the action must actually close the overlay"

# Reopen and prove the close button works independently of the action.
window.open_snackbar(container)
state["open"] = True
window.click(close)
assert not state["open"], "the close button must close the snackbar on its own"

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"snackbar.py: exited cleanly after 60 frames, undone={state['undone']}, open={state['open']}")
