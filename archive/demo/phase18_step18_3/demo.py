#!/usr/bin/env python3
"""Phase 18 Step 18.3 proof: GUI Readiness recommendation 7 -- `tre.
A11yBridge` wires `tre_a11y`'s own real AT-SPI2 bridge directly into
`tre-python`, in the SAME process that does real Vulkan rendering.

This is the first time this project's own real "does Vulkan +
accesskit_unix registration actually work in one process" question has
been answered through the actual Python binding, not a throwaway Rust
example. Phase 18 Step 18.1 fixed the real, previously-unimplemented
bug (`ensure_accessibility_enabled` querying the wrong D-Bus bus) that
fully explains every CI symptom this project's own investigation
(REVIEW.md finding #126) ever observed; Step 18.2's own isolated
feasibility check then confirmed empirically, for the first time, that
a real Vulkan-linked process CAN publish its own AT-SPI2 tree and be
correctly, independently queried -- the "process split" this project's
own GUI Readiness assessment assumed was needed for years was never a
real requirement for an *application* (the role `tre-python` fills),
only a real, separate, still-valid reason to keep TEST verification in
its own process (matching real AT-SPI2 deployment practice).

This demo reuses `canvas_accessibility_single_process_check_verify`
(Step 18.2's own real, generic query-only verifier binary, built via
`cargo build -p tre-rhi-vulkan --example
canvas_accessibility_single_process_check_verify` first) as its own
real, independent AT-SPI2 client -- exactly the role a real screen
reader would play, run as a genuinely separate OS process while this
Python process publishes.
"""

import os
import subprocess
import time
import uuid

import tre_python as tre

WIDTH, HEIGHT = 100, 100
RECT = (10.0, 10.0, 60.0, 40.0)  # x, y, width, height
VERIFY_BINARY = os.path.join(
    os.path.dirname(__file__),
    "..",
    "..",
    "target",
    "debug",
    "examples",
    "canvas_accessibility_single_process_check_verify",
)


def pixel_at(buf: bytes, x: int, y: int) -> tuple:
    offset = (y * WIDTH + x) * 4
    return tuple(buf[offset : offset + 4])


def main() -> None:
    if not os.path.exists(VERIFY_BINARY):
        raise SystemExit(
            f"real verifier binary not found at {VERIFY_BINARY} -- build it first: "
            "cargo build -p tre-rhi-vulkan --example canvas_accessibility_single_process_check_verify"
        )

    toolkit_name = f"tre-python-a11y-demo-{os.getpid()}-{uuid.uuid4().hex[:8]}"

    # --- Real render + real tagging, through the actual Python binding ---
    renderer = tre.HeadlessRenderer(WIDTH, HEIGHT)
    canvas = tre.Canvas()
    registry = tre.ShapeRegistry()
    registry.insert_rectangle(tre.Rectangle(*RECT, tre.rgba8(255, 255, 255, 255)))
    canvas.tag_accessibility_node(1, *RECT, tre.AccessibilityRole.Button)

    # `accessibility_nodes()` must be read BEFORE `render_canvas` --
    # `render_canvas` deliberately consumes the canvas's own recorded
    # content (vertices, commands, AND tagged nodes), leaving it freshly
    # empty so the same `Canvas` object can be reused next frame,
    # exactly like `ShapeRegistry.flatten`'s own established "consumes,
    # doesn't just read" precedent.
    nodes = canvas.accessibility_nodes()
    assert len(nodes) == 1, f"exactly one tagged node was expected, got {len(nodes)}"
    node = nodes[0]
    assert (node.x, node.y, node.width, node.height) == RECT, (
        f"canvas.accessibility_nodes() must report the exact tagged bounds, got "
        f"{(node.x, node.y, node.width, node.height)}"
    )
    assert node.role == tre.AccessibilityRole.Button
    print(f"canvas.accessibility_nodes() reports the exact tagged node: {RECT}, role=Button -- OK")

    renderer.flatten_into(canvas, registry)
    frame = renderer.render_canvas(canvas)
    center_x, center_y = int(RECT[0] + RECT[2] / 2), int(RECT[1] + RECT[3] / 2)
    assert pixel_at(frame, center_x, center_y) == (255, 255, 255, 255), (
        "the real rect must render at its own center -- this is a genuine Vulkan render, "
        "not a simulated stand-in"
    )
    print("real Vulkan render confirmed at the rect's own center pixel -- OK")

    # --- The real test: publish via A11yBridge, in this SAME process
    # that just did the real Vulkan render above ---
    bridge = tre.A11yBridge("tre-python-a11y-demo", toolkit_name, "0.0.0")
    print(f"A11yBridge connected in this real Vulkan-linked process (toolkit_name={toolkit_name!r})")

    # `PyA11yBridge` is deliberately `unsendable` (matching `PyTrayIcon`/
    # `PyClipboard`'s own precedent for platform-connection state) --
    # `publish()` must be called from the same Python thread that
    # constructed it, so the publish loop stays on the MAIN thread here;
    # the real, separate AT-SPI2 client runs as a genuinely separate OS
    # PROCESS instead (real concurrency, not a second Python thread),
    # started non-blocking so this thread can keep publishing while it
    # runs its own real, independent query.
    env = dict(os.environ, TRE_A11Y_SINGLE_PROCESS_TOOLKIT_NAME=toolkit_name)
    verifier = subprocess.Popen(
        [VERIFY_BINARY], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True
    )

    deadline = time.monotonic() + 15.0
    while verifier.poll() is None and time.monotonic() < deadline:
        # Publishes the SAME `nodes` list captured above (before
        # `render_canvas` consumed the canvas) -- a real caller
        # re-reads `canvas.accessibility_nodes()` fresh each frame,
        # right before that frame's own `render_canvas` call; this demo
        # renders once, so it keeps republishing that one real frame's
        # own tagged data for the verifier's own bounded query window.
        bridge.publish(nodes)
        time.sleep(0.02)

    stdout, stderr = verifier.communicate(timeout=5)

    print("--- real, independent verifier process output ---")
    print(stderr.strip())
    assert verifier.returncode == 0, (
        f"the real, separate verifier process failed (exit {verifier.returncode}):\n{stderr}"
    )
    assert "SINGLE-PROCESS FEASIBILITY CONFIRMED" in stderr, (
        "the verifier's own real confirmation message must appear -- something genuinely broke, "
        "not just a missing print"
    )
    print(
        "tre_python A11yBridge (Phase 18 Step 18.3) demo: PASSED -- a real Vulkan-linked "
        "tre_python process published its own real accessibility tree, independently confirmed "
        "over live AT-SPI2 by a genuinely separate process"
    )


if __name__ == "__main__":
    main()
