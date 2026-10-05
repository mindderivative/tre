#!/usr/bin/env python3
"""Frame statistics: what a window's frames cost.

`window.frame_stats()` reports the stage times, what was redrawn and how many
frames a second. Here a card animates for a second and a half and the app prints
its own frame budget: the number to watch is `cpu_ms`, the frame without the
waits for the display.

Needs a display to draw frames (without one `App.run()` returns quietly and
the statistics are empty). See docs/reference/window.md.
"""

from tre import App, Window

window = Window(width=320, height=200, title="tre -- frame stats")
window.set(profile_nodes=True)
card = window.create("box", width=120, height=80, x=100, y=60, position="absolute",
                     fill=(0x67, 0x50, 0xA4, 0xFF), corner_radius=16)
window.root.add_child(card)
card.animate("opacity", 0.4, 1500)

app = App()
app.add_window(window)
app.run(max_frames=90)

stats = window.frame_stats()
if not stats["frames"]:
    print("frame_stats.py: no display, no frames drawn")
else:
    r = stats["recent"]
    print(f"frame_stats.py: {stats['frames']} frames, {r['fps']:.0f} fps")
    print(f"  cpu per frame: mean {r['cpu_ms']['mean']:.2f} ms, p95 {r['cpu_ms']['p95']:.2f} ms, "
          f"max {r['cpu_ms']['max']:.2f} ms")
    print("  stages (ms):", {k: round(v, 3) for k, v in r["stage_ms"].items()})
    print("  redraws:", r["redraws"], "| nodes:", stats["last"]["nodes"])
    # GPU time (where the adapter can measure it) and, with profile_nodes on, where the
    # scene-building time went.
    print("  gpu (ms, mean):", stats["recent"]["gpu_ms"], "| timed:", stats["gpu_timing"])
    profile = stats["profile"]
    if profile:
        print("  by node kind:", {k: round(v["ms"], 3) for k, v in profile["by_kind"].items()})
print("frame_stats.py: exited cleanly")
