"""A tray-style menu as a `PopupWindow` hanging off a thin bar (#223).

The bar is a `LayerWindow`; clicking its button makes a `PopupWindow` parented to it, placed by
the compositor below (or, for a bottom bar, above) the button, with two items; item B opens a
submenu, a popup parented to the first. Driven by `test_layer_window_live.py` (`MENU_ARGS` says
where the bar goes and how long to run).
"""
import json, os, sys, threading, time
from tre import App, LayerWindow, PopupWindow, Window

args = json.loads(os.environ["MENU_ARGS"])
app = App()

def say(kind, **kw):
    print(json.dumps({"kind": kind, **kw}), flush=True)

if not args.get("no_window"):  # a popup is dismissed by a click outside the client, not on its windows
    behind = Window(width=200, height=100, title="tre-test-behind")
    behind.root.set(fill=(255, 0, 0, 255))
    behind.root.on("click", lambda e: say("window_click"))
    behind.maximize()  # a compositor that floats it (KWin) would leave it small
    app.add_window(behind)

BAR_H = 32
bar = LayerWindow("popup-bar", anchor=args["bar_anchor"], size=(0, BAR_H), exclusive_zone=BAR_H,
                  output=args["output"], keyboard=args.get("bar_keyboard", "none"))
bar.root.set(fill=(40, 40, 60, 255))
button = bar.create("box", width=60, height=24, x=100, y=4, position="absolute", fill=(232, 180, 76, 255))
bar.root.add_child(button)
app.add_window(bar)

menus = []

def watch(popup, name):
    popup.on("configure", lambda e: say("configure", menu=name, x=e.x, y=e.y, width=e.width, height=e.height))
    popup.on("closed", lambda e: say("closed", menu=name, reason=e.reason))

def open_submenu(item_popup):
    sub = PopupWindow(item_popup, size=(120, 60), anchor_rect=(0, 36, 160, 30), anchor="top_right",
                      gravity="bottom_right", constraint_adjustment=("flip_x", "slide_y"))
    sub.root.set(fill=(30, 90, 140, 255))
    s1 = sub.create("box", width=108, height=40, x=6, y=10, position="absolute", fill=(210, 220, 240, 255))
    s1.on("click", lambda e: say("item", name="B1"))
    sub.root.add_child(s1)
    watch(sub, "submenu")
    menus.append(sub)
    app.add_window(sub)

def open_menu(e=None):
    try:
        if menus:
            return
        popup = PopupWindow(bar, size=(160, 90), anchor_rect=(100, 0, 60, BAR_H), anchor="bottom_left",
                            gravity="bottom_right", constraint_adjustment=("flip_y", "slide_x"),
                            grab=args.get("grab", True))
        popup.root.set(fill=(30, 140, 90, 255))
        a = popup.create("box", width=148, height=30, x=6, y=6, position="absolute", fill=(200, 230, 210, 255))
        b = popup.create("box", width=148, height=30, x=6, y=42, position="absolute", fill=(200, 230, 210, 255))
        a.on("click", lambda e: say("item", name="A"))
        b.on("click", lambda e: (say("item", name="B"), open_submenu(popup)))
        popup.root.add_child(a)
        popup.root.add_child(b)
        watch(popup, "menu")
        menus.append(popup)
        app.add_window(popup)
        say("opened")
    except Exception:
        import traceback
        say("error", text=traceback.format_exc()[-700:])

button.on("click", open_menu)
threading.Thread(target=lambda: (time.sleep(2.5), say("ready")), daemon=True).start()
threading.Thread(target=lambda: (time.sleep(args["seconds"]), os._exit(0)), daemon=True).start()
app.run()
