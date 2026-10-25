#!/usr/bin/env python3
"""A scripted X11 drag source: drags files onto a window the way a file manager does (0.5.8, #133).

It speaks XDND (version 5) through libX11 with ctypes, so it needs only an X server
(XWayland counts) and no toolkit. It is how tre's file drag and drop is exercised on
X11 without anyone holding the mouse.

    python tools/xdnd_source.py --title "tre -- file drop" --at 200,150 /tmp/a.txt /tmp/b.txt
    python tools/xdnd_source.py --title "tre -- file drop" --cancel /tmp/a.txt

`--at x,y` is where the drag points, in the target window's own pixels (repeat it to
move the drag); `--cancel` leaves instead of dropping; `--hold SECONDS` waits between
messages (default 0.3). As a module, `drag(...)` does the same and returns what the
target answered. Needs `DISPLAY`.
"""

import argparse
import ctypes
import ctypes.util
import os
import sys
import time
from ctypes import POINTER, Structure, Union, byref, c_char_p, c_int, c_long, c_uint, c_ulong, c_void_p

x11 = ctypes.CDLL(ctypes.util.find_library("X11"))

CLIENT_MESSAGE, SELECTION_REQUEST, SELECTION_NOTIFY = 33, 30, 31


class ClientMessage(Structure):
    _fields_ = [("type", c_int), ("serial", c_ulong), ("send_event", c_int), ("display", c_void_p),
                ("window", c_ulong), ("message_type", c_ulong), ("format", c_int), ("data", c_long * 5)]


class SelectionRequest(Structure):
    _fields_ = [("type", c_int), ("serial", c_ulong), ("send_event", c_int), ("display", c_void_p),
                ("owner", c_ulong), ("requestor", c_ulong), ("selection", c_ulong),
                ("target", c_ulong), ("property", c_ulong), ("time", c_ulong)]


class SelectionNotify(Structure):
    _fields_ = [("type", c_int), ("serial", c_ulong), ("send_event", c_int), ("display", c_void_p),
                ("requestor", c_ulong), ("selection", c_ulong), ("target", c_ulong),
                ("property", c_ulong), ("time", c_ulong)]


class XEvent(Union):
    _fields_ = [("type", c_int), ("client", ClientMessage), ("request", SelectionRequest),
                ("notify", SelectionNotify), ("pad", c_long * 24)]


x11.XOpenDisplay.restype = c_void_p
x11.XOpenDisplay.argtypes = [c_char_p]
x11.XDefaultRootWindow.restype = c_ulong
x11.XDefaultRootWindow.argtypes = [c_void_p]
x11.XInternAtom.restype = c_ulong
x11.XInternAtom.argtypes = [c_void_p, c_char_p, c_int]
x11.XCreateSimpleWindow.restype = c_ulong
x11.XCreateSimpleWindow.argtypes = [c_void_p, c_ulong, c_int, c_int, c_uint, c_uint, c_uint, c_ulong, c_ulong]
x11.XSetSelectionOwner.argtypes = [c_void_p, c_ulong, c_ulong, c_ulong]
x11.XSendEvent.argtypes = [c_void_p, c_ulong, c_int, c_long, c_void_p]
x11.XFlush.argtypes = [c_void_p]
x11.XPending.argtypes = [c_void_p]
x11.XNextEvent.argtypes = [c_void_p, c_void_p]
x11.XChangeProperty.argtypes = [c_void_p, c_ulong, c_ulong, c_ulong, c_int, c_int, c_char_p, c_int]
x11.XQueryTree.argtypes = [c_void_p, c_ulong, POINTER(c_ulong), POINTER(c_ulong), POINTER(POINTER(c_ulong)), POINTER(c_uint)]
x11.XFetchName.argtypes = [c_void_p, c_ulong, POINTER(c_char_p)]
x11.XFree.argtypes = [c_void_p]
x11.XTranslateCoordinates.argtypes = [c_void_p, c_ulong, c_ulong, c_int, c_int, POINTER(c_int), POINTER(c_int), POINTER(c_ulong)]


def find_window(dpy, window, title):
    """The window under `window` whose WM_NAME is `title`, or None."""
    name = c_char_p()
    if x11.XFetchName(dpy, window, byref(name)) and name.value:
        found = name.value.decode(errors="replace") == title
        x11.XFree(name)
        if found:
            return window
    root, parent, children, count = c_ulong(), c_ulong(), POINTER(c_ulong)(), c_uint()
    if x11.XQueryTree(dpy, window, byref(root), byref(parent), byref(children), byref(count)):
        kids = [children[i] for i in range(count.value)]
        if children:
            x11.XFree(children)
        for kid in kids:
            hit = find_window(dpy, kid, title)
            if hit:
                return hit
    return None


def drag(title, paths, points=((10, 10),), cancel=False, hold=0.3, timeout=3.0):
    """Drags `paths` onto the window titled `title`, pointing at each of `points`
    (window pixels) in turn, then drops (or leaves, with `cancel`). Returns a dict:
    `status` (the target's XdndStatus replies: accepted?), `finished` (it said it was
    done) and `requests` (how many times it asked for the data)."""
    dpy = x11.XOpenDisplay(None)
    if not dpy:
        raise SystemExit("no X display (DISPLAY is %r)" % os.environ.get("DISPLAY"))
    root = x11.XDefaultRootWindow(dpy)
    target = find_window(dpy, root, title)
    if target is None:
        raise SystemExit("no window titled %r on this X display" % title)
    atom = lambda name: x11.XInternAtom(dpy, name.encode(), 0)  # noqa: E731
    enter, position, status = atom("XdndEnter"), atom("XdndPosition"), atom("XdndStatus")
    leave, drop, finished = atom("XdndLeave"), atom("XdndDrop"), atom("XdndFinished")
    selection, uris, copy = atom("XdndSelection"), atom("text/uri-list"), atom("XdndActionCopy")
    source = x11.XCreateSimpleWindow(dpy, root, 0, 0, 1, 1, 0, 0, 0)
    x11.XSetSelectionOwner(dpy, selection, source, 0)
    payload = "".join("file://%s\r\n" % os.path.abspath(p) for p in paths).encode()
    result = {"status": [], "finished": False, "requests": 0}

    def send(message_type, *data):
        event = XEvent()
        event.client = ClientMessage(CLIENT_MESSAGE, 0, 1, dpy, target, message_type, 32,
                                     (c_long * 5)(*(list(data) + [0] * (5 - len(data)))))
        x11.XSendEvent(dpy, target, 0, 0, byref(event))
        x11.XFlush(dpy)

    def pump(seconds, until=None):
        end = time.time() + seconds
        while time.time() < end:
            while x11.XPending(dpy):
                event = XEvent()
                x11.XNextEvent(dpy, byref(event))
                if event.type == SELECTION_REQUEST:
                    request = event.request
                    result["requests"] += 1
                    reply = XEvent()
                    prop = request.property
                    if request.target == uris:
                        x11.XChangeProperty(dpy, request.requestor, prop, uris, 8, 0, payload, len(payload))
                    else:
                        prop = 0
                    reply.notify = SelectionNotify(SELECTION_NOTIFY, 0, 1, dpy, request.requestor,
                                                   request.selection, request.target, prop, request.time)
                    x11.XSendEvent(dpy, request.requestor, 0, 0, byref(reply))
                    x11.XFlush(dpy)
                elif event.type == CLIENT_MESSAGE:
                    kind = event.client.message_type
                    if kind == status:
                        result["status"].append(bool(event.client.data[1] & 1))
                    elif kind == finished:
                        result["finished"] = True
            if until and until():
                return
            time.sleep(0.005)

    ox, oy, child = c_int(), c_int(), c_ulong()
    x11.XTranslateCoordinates(dpy, target, root, 0, 0, byref(ox), byref(oy), byref(child))
    send(enter, source, 5 << 24, uris, 0, 0)
    for x, y in points:
        send(position, source, 0, ((ox.value + x) << 16) | (oy.value + y), 0, copy)
        pump(hold)
    if cancel:
        send(leave, source)
        pump(hold)
    else:
        send(drop, source, 0, 0)
        pump(timeout, until=lambda: result["finished"])
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--title", required=True, help="the target window's title")
    parser.add_argument("--at", action="append", default=[], help="x,y in the window; repeat to move")
    parser.add_argument("--cancel", action="store_true")
    parser.add_argument("--hold", type=float, default=0.3)
    parser.add_argument("paths", nargs="+")
    args = parser.parse_args()
    points = [tuple(int(v) for v in at.split(",")) for at in args.at] or [(10, 10)]
    print(drag(args.title, args.paths, points, args.cancel, args.hold))


if __name__ == "__main__":
    sys.exit(main())
