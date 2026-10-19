"""A stand-in for the XDG settings portal on a private session bus (0.5.7, #134).

Run by tests/test_os_preferences_live.py with DBUS_SESSION_BUS_ADDRESS pointing at
a throwaway `dbus-daemon`, so nothing on the user's desktop is touched. Serves
`org.freedesktop.portal.Settings` (`ReadOne`, `Read`, `ReadAll`) and emits
`SettingChanged`. argv[1] is the starting values as JSON,
`[[namespace, key, "u"|"b", value], ...]`; each line on stdin is a JSON command,
`{"set": [namespace, key, "u"|"b", value]}` (stores it and announces it) or
`{"unset": [namespace, key]}`. Prints `ready` once it owns the name.
"""

import json
import sys
import threading

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

PATH = "/org/freedesktop/portal/desktop"
IFACE = "org.freedesktop.portal.Settings"
XML = f"""<node><interface name='{IFACE}'>
  <method name='ReadOne'><arg type='s' direction='in'/><arg type='s' direction='in'/><arg type='v' direction='out'/></method>
  <method name='Read'><arg type='s' direction='in'/><arg type='s' direction='in'/><arg type='v' direction='out'/></method>
  <signal name='SettingChanged'><arg type='s'/><arg type='s'/><arg type='v'/></signal>
  <property name='version' type='u' access='read'/>
</interface></node>"""

values = {}
connection = None


def variant(kind, value):
    return GLib.Variant(kind, value if kind != "b" else bool(value))


def on_call(conn, sender, path, iface, method, params, invocation):
    namespace, key = params.unpack()
    found = values.get((namespace, key))
    if found is None:
        invocation.return_dbus_error("org.freedesktop.portal.Error.NotFound", "no such key")
    elif method == "ReadOne":
        invocation.return_value(GLib.Variant("(v)", (found,)))
    else:
        # The deprecated `Read` wraps the value in a second variant.
        invocation.return_value(GLib.Variant("(v)", (GLib.Variant("v", found),)))


def on_property(conn, sender, path, iface, name):
    return GLib.Variant("u", 2)


def apply(command):
    if "set" in command:
        namespace, key, kind, value = command["set"]
        values[(namespace, key)] = variant(kind, value)
        connection.emit_signal(
            None, PATH, IFACE, "SettingChanged",
            GLib.Variant("(ssv)", (namespace, key, values[(namespace, key)])),
        )
    elif "unset" in command:
        values.pop(tuple(command["unset"]), None)
    return False


def read_commands(loop):
    for line in sys.stdin:
        if line.strip():
            GLib.idle_add(apply, json.loads(line))
    GLib.idle_add(loop.quit)


def on_bus_acquired(conn, name):
    global connection
    connection = conn
    info = Gio.DBusNodeInfo.new_for_xml(XML)
    conn.register_object(PATH, info.interfaces[0], on_call, on_property, None)


def on_name_acquired(conn, name):
    print("ready", flush=True)


loop = GLib.MainLoop()
for namespace, key, kind, value in json.loads(sys.argv[1]):
    values[(namespace, key)] = variant(kind, value)
Gio.bus_own_name(
    Gio.BusType.SESSION, "org.freedesktop.portal.Desktop", Gio.BusNameOwnerFlags.NONE,
    on_bus_acquired, on_name_acquired, None,
)
threading.Thread(target=read_commands, args=(loop,), daemon=True).start()
loop.run()
