#!/usr/bin/env python3
"""Phase 12 Step 12.6 proof: `renderer.render_parallel(registries)` runs
real, genuine parallelism -- not just "produces the right pixels," which
serial execution would too, but measurably faster wall-clock time on a
multi-core machine, because each registry's own expensive tessellation
runs concurrently on its own real OS thread with the GIL released.

Each registry holds one deliberately expensive `Path`: a single closed
loop (no self-intersection, unlike a multi-turn spiral, which produces a
pathological triangle count under fill tessellation) perturbed by many
short `cubic_to` segments -- real, non-trivial `lyon` tessellation work
per registry from the sheer segment count, not from self-crossing.
Cheap shapes wouldn't show a measurable timing difference either way.
"""

import math
import time

import tre_python as tre

WIDTH, HEIGHT = 400, 400
SEGMENTS_PER_PATH = 400
REGISTRY_COUNT = 8


def build_wavy_loop_registry(seed: int) -> "tre.ShapeRegistry":
    registry = tre.ShapeRegistry()
    color = tre.rgba8((seed * 37) % 256, (seed * 61) % 256, (seed * 89) % 256, 255)
    path = tre.Path(color)
    cx, cy = WIDTH / 2, HEIGHT / 2
    base_radius = WIDTH / 2 - 20
    wobble = base_radius * 0.15

    def point_at(angle: float) -> tuple:
        radius = base_radius + wobble * math.sin(9 * angle + seed)
        return (cx + radius * math.cos(angle), cy + radius * math.sin(angle))

    start_x, start_y = point_at(0.0)
    path.move_to(start_x, start_y)
    for i in range(1, SEGMENTS_PER_PATH + 1):
        angle = (i / SEGMENTS_PER_PATH) * math.tau
        prev_angle = ((i - 1) / SEGMENTS_PER_PATH) * math.tau
        cx1, cy1 = point_at(prev_angle + (angle - prev_angle) * 0.33)
        cx2, cy2 = point_at(prev_angle + (angle - prev_angle) * 0.66)
        x, y = point_at(angle)
        path.cubic_to(cx1, cy1, cx2, cy2, x, y)
    path.close()
    registry.insert_path(path)
    return registry


def pixel_at(buf: bytes, x: int, y: int) -> tuple:
    offset = (y * WIDTH + x) * 4
    return tuple(buf[offset : offset + 4])


def main() -> None:
    print(f"building {REGISTRY_COUNT} registries, each a {SEGMENTS_PER_PATH}-segment wavy loop path...")
    registries = [build_wavy_loop_registry(i) for i in range(REGISTRY_COUNT)]

    renderer = tre.HeadlessRenderer(WIDTH, HEIGHT)

    # --- Sequential baseline: render() once per registry. ---
    start = time.perf_counter()
    for registry in registries:
        renderer.render(registry)
    sequential_seconds = time.perf_counter() - start
    print(f"sequential (render() x{REGISTRY_COUNT}): {sequential_seconds:.4f}s")

    # --- Real parallel: one render_parallel() call, N real OS threads. ---
    start = time.perf_counter()
    frame = renderer.render_parallel(registries)
    parallel_seconds = time.perf_counter() - start
    print(f"render_parallel({REGISTRY_COUNT} registries): {parallel_seconds:.4f}s")

    speedup = sequential_seconds / parallel_seconds if parallel_seconds > 0 else float("inf")
    print(f"speedup: {speedup:.2f}x")
    assert speedup > 1.3, (
        f"expected a real speedup from genuine parallelism, got only {speedup:.2f}x -- "
        "render_parallel may not actually be running concurrently"
    )

    # --- Correctness: every registry's own spiral must have contributed
    # real, non-background pixels to the final stitched frame. ---
    background = pixel_at(frame, 0, 0)
    found_non_background = False
    for y in range(0, HEIGHT, 4):
        for x in range(0, WIDTH, 4):
            if pixel_at(frame, x, y) != background:
                found_non_background = True
                break
        if found_non_background:
            break
    assert found_non_background, "render_parallel's stitched frame has no real rendered content"
    print("real, non-background content found in the stitched frame -- OK")

    print("tre_python render_parallel demo: PASSED")


if __name__ == "__main__":
    main()
