#!/usr/bin/env python3
"""A platform check for transparent windows, to run by hand on each OS and
desktop (Windows, macOS, GNOME, ...), then paste the report into the issue.

    python tools/check_transparency.py

It opens a frameless, transparent window with a half-transparent red card and
a clear margin, asks the compositor for blur-behind, and steps through three
phases of about 8 seconds each, printing what to look at. The window closes
itself and prints a report. Nothing here is automated pixel-reading of the
desktop: the lines marked LOOK are for your eyes.

1. NORMAL: click the card a few times. Each click is counted. LOOK: is the
   card see-through (the desktop shows through the red)? is the margin fully
   clear? does a shadow or border appear around the window (DWM on Windows)?
   is what shows through blurred (blur-behind)?
2. CLICK-THROUGH: `click_through` is on. Click the card where another window
   or desktop icon is behind it. LOOK: the click must reach what is behind;
   the count must not rise.
3. NORMAL again: click once more; the count should rise, so the window takes
   the pointer back.
"""

import importlib.metadata
import platform
import sys
import threading
import time

from tre import App, Window

PHASE_SECONDS = 8.0

window = Window(width=420, height=260, title="tre transparency check", decorations=False)
window.set(transparent=True, blur_behind=True)
window.root.set(fill=(0, 0, 0, 0), padding=0)
clicks = {"normal": 0, "click_through": 0, "normal_again": 0}
phase = {"name": "normal"}

card = window.create("box", width=380, height=220, x=20, y=20, position="absolute",
                     corner_radius=28, fill=(0xE0, 0x20, 0x20, 0x80),
                     stroke_color=(0xFF, 0xFF, 0xFF, 0x60), stroke_width=2, padding=20)
text = window.create("text", text="phase: normal -- click me", font_size=20, width=340,
                     height=60, fill=(0xFF, 0xFF, 0xFF, 0xFF))
card.add_child(text)
window.root.add_child(card)


def on_click() -> None:
    clicks[phase["name"]] += 1


card.on("click", on_click)

app = App()
app.add_window(window)
handle = app.thread_handle()
report: dict[str, object] = {}


def enter(name: str, label: str) -> None:
    phase["name"] = name
    text.set(text=f"phase: {label}")
    print(f"\n== {label} ({PHASE_SECONDS:.0f}s) ==")


def sequence() -> None:
    time.sleep(1.0)
    handle.call_soon(lambda: report.update(
        transparent_active=window.get("transparent_active"),
        blur_behind=window.get("blur_behind"),
    ))
    print("== NORMAL: click the card; LOOK for see-through red, a clear margin,")
    print("   a shadow/border (Windows DWM), and a blurred backdrop ==")
    time.sleep(PHASE_SECONDS)

    def to_click_through() -> None:
        try:
            window.set(click_through=True)
            report["click_through_set"] = "ok"
        except ValueError as err:
            report["click_through_set"] = f"unsupported: {err}"
        enter("click_through", "click-through: click what is BEHIND the card")

    handle.call_soon(to_click_through)
    time.sleep(PHASE_SECONDS)

    def back() -> None:
        try:
            window.set(click_through=False)
        except ValueError:
            pass
        enter("normal_again", "normal again: click the card")

    handle.call_soon(back)
    time.sleep(PHASE_SECONDS)
    handle.call_soon(window.close)


threading.Thread(target=sequence, daemon=True).start()
print(f"tre {importlib.metadata.version('tesserae-engine')} on {platform.platform()} "
      f"(python {sys.version.split()[0]}, window platform: {window.get('platform')})")
print("Set DISPLAY/WAYLAND_DISPLAY as usual. XDG_CURRENT_DESKTOP =",
      __import__("os").environ.get("XDG_CURRENT_DESKTOP", "(unset)"))
app.run()

verdict = (
    "INCONCLUSIVE: you never clicked the card in the normal phase"
    if clicks["normal"] == 0
    else "PASS"
    if clicks["click_through"] == 0
    else "FAIL: the window still took clicks"
)
print("\n== REPORT (paste this) ==")
print(f"transparent_active : {report.get('transparent_active')}")
print(f"blur_behind (asked): {report.get('blur_behind')}  (LOOK: was the backdrop blurred?)")
print(f"click_through set  : {report.get('click_through_set')}")
print(f"clicks, normal     : {clicks['normal']}   (expect > 0)")
print(f"clicks, click-thru : {clicks['click_through']}   (expect 0: {verdict})")
print(f"clicks, normal 2   : {clicks['normal_again']}   (expect > 0)")
