"""A persistent wlroots virtual pointer, for live tests on a headless sway.

A compositor with no pointer device offers its clients no `wl_pointer`, and a short-lived
tool such as `wlrctl` creates its virtual device and destroys it before a client has bound
the pointer, so its clicks are lost. This keeps one `zwlr_virtual_pointer_v1` alive for the
whole test and takes commands over a pipe. It needs `pywayland` (`pip install pywayland`);
the protocol bindings are generated from `data/wlr-virtual-pointer-unstable-v1.xml` (wlroots'
protocol, MIT) with pywayland's scanner into a temporary package.

As a module it is the helper process; `Pointer` starts and drives one.
"""

import os
import subprocess
import sys
import tempfile
import time

HERE = os.path.dirname(os.path.abspath(__file__))
XML = os.path.join(HERE, "data", "wlr-virtual-pointer-unstable-v1.xml")
CORE_XML = "/usr/share/wayland/wayland.xml"
BTN_LEFT = 0x110


def available():
    try:
        import pywayland  # noqa: F401
    except ImportError:
        return False
    return os.path.exists(CORE_XML)


def _bindings():
    """A directory holding the generated `wlrvp` package."""
    root = tempfile.mkdtemp(prefix="tre-wlrvp-")
    package = os.path.join(root, "wlrvp")
    os.mkdir(package)
    subprocess.run(
        [sys.executable, "-m", "pywayland.scanner", "-i", CORE_XML, XML, "-o", package],
        check=True, capture_output=True,
    )
    open(os.path.join(package, "__init__.py"), "w").close()
    return root


class Pointer:
    def __init__(self, env):
        self.env = env
        self.root = _bindings()
        self.process = subprocess.Popen(
            [sys.executable, os.path.abspath(__file__), self.root],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            env=env, text=True, bufsize=1,
        )
        assert self.process.stdout.readline().strip() == "ready"

    def _send(self, line):
        self.process.stdin.write(line + "\n")
        self.process.stdin.flush()
        assert self.process.stdout.readline().strip() == "done"

    def move(self, x, y, layout_width, layout_height):
        """Moves to `(x, y)` in a layout `layout_width` x `layout_height` pixels."""
        self._send(f"abs {x} {y} {layout_width} {layout_height}")
        time.sleep(0.1)

    def press(self):
        self._send("down")
        time.sleep(0.1)

    def release(self):
        self._send("up")
        time.sleep(0.1)

    def click(self):
        self._send("click")
        time.sleep(0.1)

    def escape(self):
        subprocess.run(["wtype", "-s", "300", "-k", "Escape"], env=self.env, timeout=20)

    def close(self):
        try:
            self.process.stdin.close()
            self.process.wait(timeout=5)
        except Exception:
            self.process.kill()


def _helper(root):  # pragma: no cover - runs in the child process
    sys.path.insert(0, root)
    from pywayland.client import Display
    from pywayland.protocol.wayland import WlSeat
    from wlrvp.wlr_virtual_pointer_unstable_v1 import ZwlrVirtualPointerManagerV1

    state = {}
    display = Display()
    display.connect()
    registry = display.get_registry()

    def on_global(reg, name, interface, version):
        if interface == "wl_seat" and "seat" not in state:
            state["seat"] = reg.bind(name, WlSeat, min(version, 1))
        elif interface == "zwlr_virtual_pointer_manager_v1":
            state["manager"] = reg.bind(name, ZwlrVirtualPointerManagerV1, 1)

    registry.dispatcher["global"] = on_global
    display.roundtrip()
    pointer = state["manager"].create_virtual_pointer(state["seat"])
    display.roundtrip()
    print("ready", flush=True)

    def now():
        return int(time.monotonic() * 1000) & 0xFFFFFFFF

    for line in sys.stdin:
        parts = line.split()
        if parts and parts[0] == "abs":
            x, y, w, h = (int(v) for v in parts[1:5])
            pointer.motion_absolute(now(), x, y, w, h)
            pointer.frame()
        elif parts and parts[0] == "down":
            pointer.button(now(), BTN_LEFT, 1)
            pointer.frame()
        elif parts and parts[0] == "up":
            pointer.button(now(), BTN_LEFT, 0)
            pointer.frame()
        elif parts and parts[0] == "click":
            pointer.button(now(), BTN_LEFT, 1)
            pointer.frame()
            pointer.button(now(), BTN_LEFT, 0)
            pointer.frame()
        display.flush()
        display.roundtrip()
        print("done", flush=True)
    os._exit(0)  # pywayland's teardown can crash; there is nothing left to release


if __name__ == "__main__":
    _helper(sys.argv[1])
