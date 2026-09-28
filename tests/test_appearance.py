"""The OS's light/dark appearance: `window.get("dark")` and, on Linux, the
`color_scheme` event from the XDG settings portal (issue #18).

The live test runs in its own processes -- a stand-in portal
(`portal_stub.py`) on a private session bus, and an `App.run()` -- so no
second `App.run()` shares this pytest process.
"""

import os
import shutil
import subprocess
import sys
import textwrap
from pathlib import Path

import pytest

from tre import Window

HERE = Path(__file__).parent


def test_get_dark_is_a_bool_or_none():
    assert Window(width=100, height=100).get("dark") in (True, False, None)


def test_get_dark_is_none_without_a_session_bus():
    script = "import tre; print(tre.Window(width=10, height=10).get('dark'))"
    env = {**os.environ, "DBUS_SESSION_BUS_ADDRESS": "unix:path=/nonexistent"}
    out = subprocess.run([sys.executable, "-c", script], env=env,
                         capture_output=True, text=True, timeout=30)
    assert out.stdout.strip() == "None"


def gi_python():
    """A Python with PyGObject for the stand-in portal, or `None`."""
    for candidate in ("/usr/bin/python3", shutil.which("python3")):
        if candidate and subprocess.run(
            [candidate, "-c", "from gi.repository import Gio"], capture_output=True
        ).returncode == 0:
            return candidate
    return None


LIVE = textwrap.dedent("""
    import time, tre
    window = tre.Window(width=200, height=120, title="appearance")
    print("before", window.get("dark"), flush=True)
    window.on("color_scheme", lambda e: print("event", e.dark, window.get("dark"), flush=True))
    app = tre.App()
    app.add_window(window)
    handle = app.thread_handle()
    def slow():  # about 100 frames a second, so 700 frames outlast the portal's changes
        time.sleep(0.01)
        handle.call_soon(slow)
    handle.call_soon(slow)
    app.run(max_frames=700)
    print("done", flush=True)
""")


@pytest.mark.skipif(sys.platform != "linux", reason="the XDG portal is Linux's")
def test_the_portal_answers_get_and_announces_changes():
    portal_python = gi_python()
    if portal_python is None or shutil.which("dbus-run-session") is None:
        pytest.skip("needs dbus-run-session and a Python with PyGObject")
    if not (os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY")):
        pytest.skip("needs a display for App.run()")
    runner = (
        f'"{portal_python}" "{HERE / "portal_stub.py"}" > "$PORTAL_LOG" & '
        'until grep -q ready "$PORTAL_LOG" 2>/dev/null; do sleep 0.05; done; '
        f'"{sys.executable}" -c "$LIVE"; kill %1'
    )
    env = {**os.environ, "LIVE": LIVE, "PORTAL_LOG": str(HERE / ".portal_stub.log")}
    try:
        out = subprocess.run(["dbus-run-session", "--", "bash", "-c", runner], env=env,
                             capture_output=True, text=True, timeout=60)
    finally:
        (HERE / ".portal_stub.log").unlink(missing_ok=True)
    lines = [line for line in out.stdout.splitlines()
             if line.split(" ")[0] in ("before", "event", "done")]
    assert lines == [
        "before True",        # read from the portal before the window opens
        "event False False",  # light; the repeat and the other namespace send nothing
        "event True True",    # dark again
        "done",
    ], out.stdout + out.stderr
