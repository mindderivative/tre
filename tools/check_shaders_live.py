"""A live check of the shader pipeline on the machine it runs on (#91): a video-style
input repainting every frame, an effect over children across real window resizes
(equal to a direct draw), and idle and animated CPU. Prints PASS/FAIL lines.

    python tools/check_shaders_live.py                       # the current session
    env -u WAYLAND_DISPLAY DISPLAY=:0 python tools/check_shaders_live.py   # X11 (XWayland)
    VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.json ...  # pin the adapter (Linux)

Needs a display. Run it on Windows (DX12) and macOS (Metal) for #187 and #188.
"""
import os, sys, threading, time, resource
from tre import App, Shader, Window

print("session:", os.environ.get("XDG_SESSION_TYPE"), "DISPLAY", os.environ.get("DISPLAY"), "WAYLAND", os.environ.get("WAYLAND_DISPLAY"))
INV = "fn shade(p: Pixel) -> vec4<f32> { let c = input_photo(p.uv); return vec4<f32>(c.rgb, c.a); }"
W, H = 64, 48
def frame(v): return bytes([v, 255 - v, 128, 255]) * (W * H)

win = Window(width=500, height=360, title="x11 check #91")
win.root.set(fill=(0x10, 0x12, 0x1C, 0xFF))
src = win.create("image", rgba=frame(0), pixel_width=W, pixel_height=H, width=W, height=H, position="absolute", x=-500, y=-500)
win.root.add_child(src)
view = win.create("box", width=200, height=150, position="absolute", x=20, y=20, shader=Shader(INV, inputs={"photo": src}))
win.root.add_child(view)
# an effect over a node with children, for the resize check
panel = win.create("box", width=200, height=120, position="absolute", x=250, y=20, fill=(0x67,0x50,0xA4,0xFF), flex_direction="vertical", padding=10, gap=6)
for k in range(3): panel.add_child(win.create("box", width=120, height=20, fill=(255, 200, 60 + k*50, 255)))
win.root.add_child(panel)
panel.set(shader=Shader("fn shade(p: Pixel) -> vec4<f32> { return content(p.uv); }", mode="effect"))

app = App(); app.add_window(win); h = app.thread_handle()
res = {}
def snap_center(node):
    data, w, hh = win.snapshot()
    x = int(node.get("layout_x") + node.get("layout_width")/2); y = int(node.get("layout_y") + node.get("layout_height")/2)
    i = (y * w + x) * 4
    return tuple(data[i:i+3])
def run_on_loop(fn):
    ev = threading.Event(); box = []
    def go():
        try: box.append(fn())
        finally: ev.set()
    h.call_soon(go); ev.wait(10); return box[0] if box else None
def idle(label, secs=4.0):
    r0 = resource.getrusage(resource.RUSAGE_SELF); t0 = time.time(); f0 = run_on_loop(lambda: win.frame_stats()["frames"])
    time.sleep(secs)
    r1 = resource.getrusage(resource.RUSAGE_SELF); f1 = run_on_loop(lambda: win.frame_stats()["frames"])
    cpu = (r1.ru_utime + r1.ru_stime - r0.ru_utime - r0.ru_stime) / (time.time() - t0) * 100
    print(f"IDLE {label}: cpu {cpu:.1f}% of a core, frames drawn {f1 - f0}")
def driver():
    time.sleep(1.0)
    idle("at start (still shaders)")
    # video input: 30 frames pushed, each must repaint the shader's output
    seen = []
    for k in range(30):
        v = (k * 8) % 256
        run_on_loop(lambda v=v: src.set(rgba=frame(v), pixel_width=W, pixel_height=H))
        time.sleep(0.034)
        seen.append(run_on_loop(lambda: snap_center(view)))
    ok = all(abs(s[0] - ((k*8) % 256)) <= 2 for k, s in enumerate(seen))
    print("VIDEO input repaints every frame:", "PASS" if ok else "FAIL", seen[:3], seen[-2:])
    # effect over children across resizes (min size raised = real window resizes)
    direct = {}
    def effect_vs_direct():
        a = win.snapshot()[0]
        panel.set(shader=None); b = win.snapshot()[0]
        panel.set(shader=Shader("fn shade(p: Pixel) -> vec4<f32> { return content(p.uv); }", mode="effect"))
        n = sum(1 for i in range(0, len(a), 4) if a[i:i+3] != b[i:i+3]); return n, len(a)//4
    diffs = []
    for mw, mh in [(560, 400), (700, 460), (820, 520)]:
        run_on_loop(lambda mw=mw, mh=mh: win.set(min_width=mw, min_height=mh)); time.sleep(0.6)
        diffs.append(run_on_loop(effect_vs_direct))
    print("EFFECT over children vs direct, across resizes:", "PASS" if all(d[0] == 0 for d in diffs) else "FAIL", diffs)
    # idle: still shader (view has none animated) for 5 s
    idle("after the resizes")
    # animated shader for 3 s
    run_on_loop(lambda: view.set(shader=Shader("fn shade(p: Pixel) -> vec4<f32> { return vec4<f32>(fract(frame.time), p.uv, 1.0); }", animated=True)))
    r0 = resource.getrusage(resource.RUSAGE_SELF); t0 = time.time(); f0 = run_on_loop(lambda: win.frame_stats()["frames"])
    time.sleep(3.0)
    r1 = resource.getrusage(resource.RUSAGE_SELF); f1 = run_on_loop(lambda: win.frame_stats()["frames"])
    cpu = (r1.ru_utime + r1.ru_stime - r0.ru_utime - r0.ru_stime) / (time.time() - t0) * 100
    print(f"ANIMATED shader: cpu {cpu:.1f}% of a core, {(f1 - f0)/3:.0f} frames/s")
    sys.stdout.flush(); os._exit(0)
threading.Thread(target=driver, daemon=True).start()
app.run()
