"""A private AT-SPI environment for live tests of what a screen reader would see (0.5.7, #200).

It touches nothing of the real desktop: two throwaway `dbus-daemon`s with no service
directories (so nothing is activated on them), a small stand-in for `org.a11y.Bus` on the
session one (what `at-spi-bus-launcher` provides: the a11y bus's address and the
`IsEnabled` switch that makes an application publish its tree), and the real
`at-spi2-registryd` on the a11y one. An application started with `env()` publishes its
accessibility tree there once `enable()` is called, and `busctl` plays the screen reader.

Needs `dbus-daemon`, `busctl`, `/usr/lib/at-spi2-registryd` and `dbus-fast` (`pip install
dbus-fast`); `available()` says whether they are all here.

As a module it is the stand-in `org.a11y.Bus` process.
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

REGISTRYD = "/usr/lib/at-spi2-registryd"
CONFIG = """<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:tmpdir=%s</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"""


def available():
    try:
        import dbus_fast  # noqa: F401
    except ImportError:
        return False
    return bool(shutil.which("dbus-daemon") and shutil.which("busctl") and os.path.exists(REGISTRYD))


class AtspiEnvironment:
    def __init__(self):
        self.dir = tempfile.mkdtemp(prefix="tre-atspi-")
        self.processes = []
        self.session = self._daemon("session")
        self.a11y = self._daemon("a11y")
        mock = subprocess.Popen(
            [sys.executable, os.path.abspath(__file__), self.session, self.a11y],
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True,
        )
        self.processes.append(mock)
        assert mock.stdout.readline().strip() == "ready", "the stand-in org.a11y.Bus did not start"
        registry = subprocess.Popen(
            [REGISTRYD], env=dict(os.environ, DBUS_SESSION_BUS_ADDRESS=self.session),
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        self.processes.append(registry)
        time.sleep(1.0)

    def _daemon(self, name):
        directory = os.path.join(self.dir, name)
        os.mkdir(directory)
        config = os.path.join(self.dir, name + ".conf")
        with open(config, "w") as f:
            f.write(CONFIG % directory)
        process = subprocess.Popen(
            ["dbus-daemon", "--nofork", "--print-address", "--config-file=" + config],
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True,
        )
        self.processes.append(process)
        return process.stdout.readline().strip()

    def env(self):
        """The environment for an application that should publish to this bus."""
        return dict(os.environ, DBUS_SESSION_BUS_ADDRESS=self.session)

    def enable(self):
        """What turning a screen reader on does: the application starts publishing its tree."""
        subprocess.run(
            ["busctl", "--address=" + self.session, "set-property", "org.a11y.Bus", "/org/a11y/bus",
             "org.a11y.Status", "IsEnabled", "b", "true"],
            check=True, capture_output=True, timeout=15,
        )

    def call(self, destination, path, interface, method, *args):
        """A method call on the a11y bus, as the parsed JSON `data` list."""
        out = subprocess.run(
            ["busctl", "--address=" + self.a11y, "--json=short", "call", destination, path, interface, method, *args],
            capture_output=True, text=True, timeout=15,
        )
        return json.loads(out.stdout)["data"] if out.returncode == 0 and out.stdout.strip() else None

    def children(self, destination, path):
        """`[(bus name, object path), ...]` of an accessible's children."""
        data = self.call(destination, path, "org.a11y.atspi.Accessible", "GetChildren")
        return [tuple(c) for c in data[0]] if data else []

    def role(self, destination, path):
        """The AT-SPI role number (`ATSPI_ROLE_*`): 43 is a push button, 23 a frame."""
        data = self.call(destination, path, "org.a11y.atspi.Accessible", "GetRole")
        return data[0] if data else None

    def name(self, destination, path):
        data = self.call(destination, path, "org.freedesktop.DBus.Properties", "Get",
                         "ss", "org.a11y.atspi.Accessible", "Name")
        return data[0]["data"] if data else None

    def do_action(self, destination, path, index=0):
        data = self.call(destination, path, "org.a11y.atspi.Action", "DoAction", "i", str(index))
        return bool(data and data[0])

    def tree(self, destination, path, depth=0, limit=6):
        """`[(depth, role, name, destination, path), ...]` of a subtree, for finding a node."""
        out = [(depth, self.role(destination, path), self.name(destination, path), destination, path)]
        if depth < limit:
            for child_destination, child_path in self.children(destination, path):
                out += self.tree(child_destination, child_path, depth + 1, limit)
        return out

    def close(self):
        for process in reversed(self.processes):
            process.terminate()
        for process in self.processes:
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
        shutil.rmtree(self.dir, ignore_errors=True)


def _mock(session_address, a11y_address):  # pragma: no cover - runs in the child process
    import asyncio

    from dbus_fast import BusType  # noqa: F401
    from dbus_fast.aio import MessageBus
    from dbus_fast.service import ServiceInterface, dbus_property, method
    from dbus_fast.constants import PropertyAccess

    class Bus(ServiceInterface):
        def __init__(self):
            super().__init__("org.a11y.Bus")

        @method()
        def GetAddress(self) -> "s":
            return a11y_address

    class Status(ServiceInterface):
        def __init__(self):
            super().__init__("org.a11y.Status")
            self.enabled = False
            self.reader = False

        @dbus_property(access=PropertyAccess.READWRITE)
        def IsEnabled(self) -> "b":
            return self.enabled

        @IsEnabled.setter
        def IsEnabled(self, value: "b"):
            self.enabled = value
            self.emit_properties_changed({"IsEnabled": value})

        @dbus_property(access=PropertyAccess.READWRITE)
        def ScreenReaderEnabled(self) -> "b":
            return self.reader

        @ScreenReaderEnabled.setter
        def ScreenReaderEnabled(self, value: "b"):
            self.reader = value
            self.emit_properties_changed({"ScreenReaderEnabled": value})

    async def main():
        bus = await MessageBus(bus_address=session_address).connect()
        bus.export("/org/a11y/bus", Bus())
        bus.export("/org/a11y/bus", Status())
        await bus.request_name("org.a11y.Bus")
        print("ready", flush=True)
        await asyncio.get_event_loop().create_future()

    asyncio.run(main())


if __name__ == "__main__":
    _mock(sys.argv[1], sys.argv[2])
