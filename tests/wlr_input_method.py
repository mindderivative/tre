"""A stand-in input method (IME) for live tests on a headless sway (0.5.7, #199).

No input method is installed on the dev machine, and wiring a real one into KDE would change
the desktop's input settings. sway implements `zwp_text_input_v3` (what a client talks to) and
`zwp_input_method_v2` (what an IME talks to), so this plays the IME: it reports when a text
input is activated or deactivated, and sets a preedit or commits text on command. Needs
`pywayland`; the protocol bindings are generated from `data/input-method-unstable-v2.xml`
(wayland-protocols, MIT) and the system's `text-input-unstable-v3.xml` into a temporary package.

It also keeps one virtual keyboard alive: a headless sway with no keyboard device gives no window
the keyboard focus (and so no text-input focus), and that device is what a real session has.

As a module it is the helper process; `InputMethod` starts and drives one. Commands on stdin:
`preedit TEXT`, `commit TEXT`, `clear`; it prints `ready`, `content_type HINT PURPOSE`,
`activated`, `deactivated`, `done`.
"""

import os
import select
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
IM_XML = os.path.join(HERE, "data", "input-method-unstable-v2.xml")
VK_XML = os.path.join(HERE, "data", "virtual-keyboard-unstable-v1.xml")
CORE_XML = "/usr/share/wayland/wayland.xml"
TEXT_XML = "/usr/share/wayland-protocols/unstable/text-input/text-input-unstable-v3.xml"


def available():
    try:
        import pywayland  # noqa: F401
    except ImportError:
        return False
    return all(os.path.exists(p) for p in (CORE_XML, TEXT_XML, IM_XML, VK_XML)) and bool(
        __import__("shutil").which("xkbcli")
    )


def _bindings():
    root = tempfile.mkdtemp(prefix="tre-wlrim-")
    package = os.path.join(root, "wlrim")
    os.mkdir(package)
    subprocess.run(
        [sys.executable, "-m", "pywayland.scanner", "-i", CORE_XML, TEXT_XML, IM_XML, VK_XML, "-o", package],
        check=True, capture_output=True,
    )
    open(os.path.join(package, "__init__.py"), "w").close()
    return root


class InputMethod:
    def __init__(self, env):
        self.root = _bindings()
        self.process = subprocess.Popen(
            [sys.executable, os.path.abspath(__file__), self.root],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            env=env, text=True, bufsize=1,
        )
        self._buffer = b""
        assert self._line(__import__("time").time() + 20) == "ready"

    def send(self, line):
        self.process.stdin.write(line + "\n")
        self.process.stdin.flush()

    def _line(self, deadline):
        """The next line from the helper, or `None` by the deadline. Read below the text layer:
        `select` does not see lines Python has already buffered."""
        import time
        while b"\n" not in self._buffer:
            if time.time() >= deadline:
                return None
            ready, _, _ = select.select([self.process.stdout], [], [], 0.2)
            if ready:
                chunk = os.read(self.process.stdout.fileno(), 65536)
                if not chunk:
                    return None
                self._buffer += chunk
        line, _, self._buffer = self._buffer.partition(b"\n")
        return line.decode().strip()

    def wait_for(self, word, timeout=10):
        """Reads lines until one equals `word`; the lines read, or `None` on timeout."""
        import time
        seen = []
        deadline = time.time() + timeout
        while True:
            line = self._line(deadline)
            if line is None:
                return None
            seen.append(line)
            if line == word:
                return seen

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
    from wlrim.input_method_unstable_v2 import ZwpInputMethodManagerV2
    from wlrim.virtual_keyboard_unstable_v1 import ZwpVirtualKeyboardManagerV1

    state = {"done": 0, "active": False, "pending_active": False}
    display = Display()
    display.connect()
    registry = display.get_registry()

    def on_global(reg, name, interface, version):
        if interface == "wl_seat" and "seat" not in state:
            state["seat"] = reg.bind(name, WlSeat, min(version, 1))
        elif interface == "zwp_input_method_manager_v2":
            state["manager"] = reg.bind(name, ZwpInputMethodManagerV2, 1)
        elif interface == "zwp_virtual_keyboard_manager_v1":
            state["vk_manager"] = reg.bind(name, ZwpVirtualKeyboardManagerV1, 1)

    registry.dispatcher["global"] = on_global
    display.roundtrip()
    method = state["manager"].get_input_method(state["seat"])
    if "vk_manager" in state:
        keymap = subprocess.run(
            ["xkbcli", "compile-keymap", "--layout", "us"], capture_output=True, text=True, check=True
        ).stdout.encode() + b"\0"
        keymap_file = tempfile.TemporaryFile()
        keymap_file.write(keymap)
        keymap_file.flush()
        state["keyboard"] = state["vk_manager"].create_virtual_keyboard(state["seat"])
        state["keyboard"].keymap(1, keymap_file.fileno(), len(keymap))
        state["keymap_file"] = keymap_file

    def on_activate(_):
        state["pending_active"] = True

    def on_deactivate(_):
        state["pending_active"] = False

    def on_content_type(_, hint, purpose):
        state["content_type"] = (hint, purpose)

    def on_done(_):
        state["done"] += 1
        if "content_type" in state:
            print("content_type %d %d" % state.pop("content_type"), flush=True)
        now = state["pending_active"]
        if now != state["active"]:
            state["active"] = now
            print("activated" if now else "deactivated", flush=True)
        print("done", flush=True)

    def on_unavailable(_):
        print("unavailable", flush=True)

    method.dispatcher["unavailable"] = on_unavailable
    method.dispatcher["activate"] = on_activate
    method.dispatcher["deactivate"] = on_deactivate
    method.dispatcher["content_type"] = on_content_type
    method.dispatcher["done"] = on_done
    display.roundtrip()
    print("ready", flush=True)
    fd = display.get_fd()
    stdin = sys.stdin
    while True:
        display.flush()
        readable, _, _ = select.select([fd, stdin], [], [], 0.2)
        if fd in readable:
            display.dispatch(block=True)
        if stdin in readable:
            line = stdin.readline()
            if not line:
                break
            command, _, text = line.rstrip("\n").partition(" ")
            if command == "preedit":
                size = len(text.encode())
                method.set_preedit_string(text, size, size)
                method.commit(state["done"])
            elif command == "commit":
                method.commit_string(text)
                method.commit(state["done"])
            elif command == "clear":
                method.set_preedit_string("", 0, 0)
                method.commit(state["done"])
    os._exit(0)


if __name__ == "__main__":
    _helper(sys.argv[1])
