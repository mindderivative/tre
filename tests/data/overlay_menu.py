"""A tray-style menu on a transparent full-screen overlay `LayerWindow` (#206, phase 6a).

The bar is one `LayerWindow`; clicking its button maps a second one on the `overlay`
layer, covering the output, with `input_region="auto"` and `keyboard="exclusive"`, and
shows the menu in it with `show_layer`. The overlay exists only while the menu is open.
Driven by `test_layer_window_live.py` (`MENU_ARGS` says what to do and where).
"""
import json, os, sys, threading, time
from tre import App, LayerWindow, Window

args = json.loads(os.environ["MENU_ARGS"])
app = App()
handle = app.thread_handle()

def say(kind, **kw):
    print(json.dumps({"kind": kind, **kw}), flush=True)

behind = Window(width=200, height=100, title="tre-test-behind")
behind.root.set(fill=(255, 0, 0, 255))
behind.root.on("click", lambda e: say("window_click"))
behind.maximize()  # a compositor that floats it (KWin) would leave it small
app.add_window(behind)

BAR_H = 32
bar = LayerWindow("menu-bar", anchor=args["bar_anchor"], size=(0, BAR_H), exclusive_zone=BAR_H,
                  output=args["output"])
bar.root.set(fill=(40, 40, 60, 255))
button = bar.create("box", width=60, height=24, x=100, y=4, position="absolute", fill=(232, 180, 76, 255))
bar.root.add_child(button)
app.add_window(bar)

overlay = None
state = {"menu": None, "sub": None}

def close_menu():
    global overlay
    if overlay is not None:
        o, overlay = overlay, None
        state["menu"] = state["sub"] = None
        o.close()

def open_menu(e=None):
    try:
        _open_menu()
    except Exception:
        import traceback
        say('error', text=traceback.format_exc()[-700:])

def _open_menu():
    global overlay
    if overlay is not None:
        return
    overlay = LayerWindow("menu-overlay", layer="overlay", anchor=("top", "bottom", "left", "right"),
                          size=(0, 0), exclusive_zone=-1, keyboard="exclusive", input_region="auto",
                          output=args["output"])
    overlay.root.set(fill=(0, 0, 0, 0))
    # An invisible anchor where the bar's button is (output coordinates).
    ax = 100
    ay = 0 if "top" in args["bar_anchor"] else args["output_height"] - BAR_H
    anchor = overlay.create("box", width=60, height=BAR_H, x=ax, y=ay, position="absolute")
    anchor.set(hit_testable=False)
    overlay.root.add_child(anchor)
    menu = overlay.create("box", width=160, height=120, fill=(30, 140, 90, 255), corner_radius=8,
                          flex_direction="vertical", padding=6, gap=6)
    for name in ("A", "B", "C"):
        item = overlay.create("box", width=148, height=30, fill=(200, 230, 210, 255), corner_radius=4)
        item.on("click", lambda e, n=name: (say("item", name=n), close_menu()))
        if name == "B":
            def enter_b(e, item=item):
                if state["sub"] is None:
                    sub = overlay.create("box", width=120, height=60, fill=(30, 90, 140, 255), corner_radius=6,
                                         flex_direction="vertical", padding=6)
                    s1 = overlay.create("box", width=108, height=40, fill=(210, 220, 240, 255))
                    s1.on("click", lambda e: (say("item", name="B1"), close_menu()))
                    sub.add_child(s1)
                    state["sub"] = sub
                    sub.on("dismiss", lambda: None)
                    overlay.show_layer(sub, anchor=item, placement="end", dismissible=False)
                    say("submenu")
            def guarded(e, item=item):
                try:
                    enter_b(e, item)
                except Exception:
                    import traceback
                    say('error', text=traceback.format_exc()[-600:])
            item.on("pointer_enter", guarded)
        menu.add_child(item)
    menu.on("dismiss", lambda: (say("dismissed"), close_menu()))
    state["menu"] = menu
    overlay.on("closed", lambda e: say("closed"))
    app.add_window(overlay)
    overlay.show_layer(menu, anchor=anchor, placement="below", dismissible=True, flip=True)
    say("opened")

button.on("click", open_menu)

def idle_probe():
    time.sleep(args.get("probe_after", 3.0))
    box = []
    done = threading.Event()
    def read():
        box.append(overlay.frame_stats()["frames"] if overlay is not None else None); done.set()
    handle.call_soon(read); done.wait(3)
    first = box[0]
    time.sleep(2.0)
    box.clear(); done.clear()
    handle.call_soon(read); done.wait(3)
    say("idle_frames", frames=(box[0] - first) if (box[0] is not None and first is not None) else None)

threading.Thread(target=lambda: (time.sleep(2.5), say("ready")), daemon=True).start()
threading.Thread(target=lambda: (time.sleep(args["seconds"]), os._exit(0)), daemon=True).start()
if args.get("probe"):
    handle0 = handle
    # Opens the menu itself, then measures its idle frames.
    def auto():
        time.sleep(3.0)
        handle0.call_soon(open_menu)
        idle_probe()
    threading.Thread(target=auto, daemon=True).start()
    def later_close():
        time.sleep(args.get('close_at', 99)); handle0.call_soon(close_menu)
    threading.Thread(target=later_close, daemon=True).start()
app.run()
