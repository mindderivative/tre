"""A stand-in XDG settings portal, for `test_appearance.py`.

Run with a Python that has PyGObject, on a private session bus
(`dbus-run-session`). It answers `color-scheme` reads with 1 (dark), then
announces light, light again (a repeat), a setting in another namespace,
and dark, half a second or more apart. It prints `ready` once it owns the
portal's name.
"""

from gi.repository import Gio, GLib

PATH = "/org/freedesktop/portal/desktop"
INTERFACE = "org.freedesktop.portal.Settings"
XML = f"""<node><interface name="{INTERFACE}">
<method name="ReadOne"><arg type="s" direction="in"/><arg type="s" direction="in"/>
<arg type="v" direction="out"/></method>
<method name="Read"><arg type="s" direction="in"/><arg type="s" direction="in"/>
<arg type="v" direction="out"/></method>
<signal name="SettingChanged"><arg type="s"/><arg type="s"/><arg type="v"/></signal>
</interface></node>"""

scheme = 1
bus = None


def answer(_bus, _sender, _path, _interface, _method, _params, invocation):
    invocation.return_value(GLib.Variant("(v)", (GLib.Variant("u", scheme),)))


def announce(namespace, value):
    bus.emit_signal(
        None, PATH, INTERFACE, "SettingChanged",
        GLib.Variant("(ssv)", (namespace, "color-scheme", GLib.Variant("u", value))),
    )
    return False


def switch(value):
    global scheme
    scheme = value
    return announce("org.freedesktop.appearance", value)


def on_bus(connection, _name):
    global bus
    bus = connection
    interface = Gio.DBusNodeInfo.new_for_xml(XML).interfaces[0]
    connection.register_object(PATH, interface, answer, None, None)


def on_name(_connection, _name):
    print("ready", flush=True)
    GLib.timeout_add(1500, switch, 2)
    GLib.timeout_add(2000, switch, 2)
    GLib.timeout_add(2500, announce, "org.example.other", 1)
    GLib.timeout_add(3000, switch, 1)


loop = GLib.MainLoop()
Gio.bus_own_name(Gio.BusType.SESSION, "org.freedesktop.portal.Desktop", 0, on_bus, on_name, None)
GLib.timeout_add(15000, lambda: loop.quit() or False)
loop.run()
