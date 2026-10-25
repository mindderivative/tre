"""0.5.8 (#134): the OS reduced-motion and contrast preferences, end to end through
a stand-in settings portal on a private D-Bus, so the live-change path is checked
without touching the desktop's own settings.

A real window's loop is needed for the events, so those cases skip without a display;
the reads need only the bus. Skips without `dbus-daemon` or PyGObject (the portal
stand-in), and off Linux.
"""

import json
import os
import select
import shutil
import subprocess
import sys
import time
from pathlib import Path

import pytest

pytestmark = pytest.mark.skipif(
    not sys.platform.startswith("linux") or shutil.which("dbus-daemon") is None,
    reason="needs Linux and dbus-daemon",
)

HERE = Path(__file__).parent
APPEARANCE = "org.freedesktop.appearance"
GNOME = "org.gnome.desktop.interface"
GNOME_A11Y = "org.gnome.desktop.a11y.interface"

def gi_python():
    """A python that can import PyGObject (the portal stand-in needs it): this one, or the
    system's, since a venv often lacks it."""
    for candidate in (sys.executable, "/usr/bin/python3", shutil.which("python3")):
        if candidate and subprocess.run(
            [candidate, "-c", "import gi"], capture_output=True
        ).returncode == 0:
            return candidate
    return None


GI_PYTHON = gi_python() if sys.platform.startswith("linux") and shutil.which("dbus-daemon") else None

CHILD = """
import json, sys, threading, time
from tre import App, Window
window = Window(width=120, height=80)
print(json.dumps({"read": [window.get("reduced_motion"), window.get("high_contrast")]}), flush=True)
window.on("reduced_motion", lambda e: print(json.dumps({"reduced_motion": e.reduced_motion}), flush=True))
window.on("high_contrast", lambda e: print(json.dumps({"high_contrast": e.high_contrast}), flush=True))
app = App()
app.add_window(window)
app.thread_handle().call_soon(lambda: print("running", flush=True))
app.run()
"""


class Session:
    """A private bus, a stand-in portal on it, and a tre child process on that bus."""

    def __init__(self, initial):
        self.bus = subprocess.Popen(
            ["dbus-daemon", "--session", "--nofork", "--print-address"],
            stdout=subprocess.PIPE, text=True,
        )
        self.address = self.bus.stdout.readline().strip()
        self.env = dict(os.environ, DBUS_SESSION_BUS_ADDRESS=self.address)
        self.portal = subprocess.Popen(
            [GI_PYTHON, str(HERE / "fake_portal.py"), json.dumps(initial)],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, env=self.env,
        )
        assert self.portal.stdout.readline().strip() == "ready"
        self.child = None
        self.buffer = b""

    def send(self, **command):
        self.portal.stdin.write(json.dumps(command) + "\n")
        self.portal.stdin.flush()

    def start_child(self):
        self.child = subprocess.Popen(
            [sys.executable, "-I", "-c", CHILD],
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, env=self.env,
        )
        return json.loads(self.next_line(10))

    def fill(self, timeout):
        """Reads whatever the child has printed into a byte buffer (select on the
        file descriptor, not on a text wrapper that may hold lines back)."""
        fd = self.child.stdout.fileno()
        ready, _, _ = select.select([fd], [], [], timeout)
        if ready:
            self.buffer += os.read(fd, 65536)

    def next_line(self, timeout):
        end = time.time() + timeout
        while b"\n" not in self.buffer:
            assert time.time() < end, "the child printed nothing in time"
            self.fill(0.2)
        line, _, self.buffer = self.buffer.partition(b"\n")
        return line.decode().strip()

    def events(self, count, timeout=5.0):
        got = []
        end = time.time() + timeout
        while len(got) < count and time.time() < end:
            if b"\n" not in self.buffer:
                self.fill(0.2)
                continue
            line, _, self.buffer = self.buffer.partition(b"\n")
            line = line.decode().strip()
            if line.startswith("{"):
                got.append(json.loads(line))
        return got

    def close(self):
        for process in (self.child, self.portal, self.bus):
            if process is not None:
                process.kill()
                process.wait()


@pytest.fixture
def session_factory():
    made = []

    def make(initial):
        if GI_PYTHON is None:
            pytest.skip("PyGObject (the portal stand-in) is not installed")
        s = Session(initial)
        made.append(s)
        return s

    yield make
    for s in made:
        s.close()


def has_display():
    return bool(os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY"))


def test_a_portal_with_the_new_keys_is_read_as_they_say(session_factory):
    s = session_factory([[APPEARANCE, "reduced-motion", "u", 1], [APPEARANCE, "contrast", "u", 0]])
    assert s.start_child() == {"read": [True, False]}


def test_a_portal_without_the_new_keys_falls_back_to_the_gnome_settings(session_factory):
    # enable-animations off is reduced motion; high-contrast on is more contrast.
    s = session_factory([[GNOME, "enable-animations", "b", False], [GNOME_A11Y, "high-contrast", "b", True]])
    assert s.start_child() == {"read": [True, True]}
    s.close()
    s = session_factory([[GNOME, "enable-animations", "b", True], [GNOME_A11Y, "high-contrast", "b", False]])
    assert s.start_child() == {"read": [False, False]}


def test_no_portal_at_all_says_none(session_factory):
    s = session_factory([])
    assert s.start_child() == {"read": [None, None]}


@pytest.mark.skipif(not has_display(), reason="the events need a window's loop")
def test_a_change_of_the_new_keys_reaches_the_window_once(session_factory):
    s = session_factory([[APPEARANCE, "reduced-motion", "u", 0], [APPEARANCE, "contrast", "u", 0]])
    assert s.start_child() == {"read": [False, False]}
    assert s.next_line(10) == "running"
    s.send(set=[APPEARANCE, "reduced-motion", "u", 1])
    s.send(set=[APPEARANCE, "reduced-motion", "u", 1])  # unchanged: no second event
    s.send(set=[APPEARANCE, "contrast", "u", 1])
    s.send(set=[APPEARANCE, "reduced-motion", "u", 0])
    assert s.events(3) == [{"reduced_motion": True}, {"high_contrast": True}, {"reduced_motion": False}]
    assert s.events(1, timeout=0.5) == []


@pytest.mark.skipif(not has_display(), reason="the events need a window's loop")
def test_a_change_of_the_gnome_settings_reaches_the_window_where_the_new_keys_are_absent(session_factory):
    s = session_factory([[GNOME, "enable-animations", "b", True], [GNOME_A11Y, "high-contrast", "b", False]])
    assert s.start_child() == {"read": [False, False]}
    assert s.next_line(10) == "running"
    s.send(set=[GNOME, "enable-animations", "b", False])
    s.send(set=[GNOME_A11Y, "high-contrast", "b", True])
    assert s.events(2) == [{"reduced_motion": True}, {"high_contrast": True}]


@pytest.mark.skipif(not has_display(), reason="the events need a window's loop")
def test_the_gnome_settings_do_not_override_a_portal_that_has_the_new_keys(session_factory):
    s = session_factory([[APPEARANCE, "reduced-motion", "u", 0], [GNOME, "enable-animations", "b", True]])
    assert s.start_child() == {"read": [False, None]}
    assert s.next_line(10) == "running"
    s.send(set=[GNOME, "enable-animations", "b", False])  # the new key says otherwise
    assert s.events(1, timeout=1.0) == []
