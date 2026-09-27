"""Benchmark: the cost of `materialize` crossing from Rust into Python.

Scrolls a 100,000-row virtual list one row at a time through a one-row
viewport -- the worst case, exactly one `materialize` call per step, never
several amortized together -- and times the warmed-up steady state of each
step (wheel, `advance(0)`, one callback). If per-call GIL overhead dominated,
the fix would be to batch the callback.

Skipped unless `TRE_RUN_BENCHMARK` is set (a bare `skip` marker has no flag
to lift it):
    TRE_RUN_BENCHMARK=1 pytest tests/test_virtual_list_benchmark.py -v -s
"""

import os
import time

import pytest

from tre import Window

ROW_COUNT = 100_000
ROW_EXTENT = 20.0
WARMUP_ROWS = 1_000
# Generous on purpose (like `frame_budget.rs`): this catches a regression
# (e.g. an accidental per-call GIL re-attach, or marshaling that adds
# allocation), not a specific microbenchmark number. A 16.6ms/60Hz frame has
# room for hundreds of steps this size.
MAX_MICROS_PER_CALL = 200.0


@pytest.mark.skipif(
    not os.environ.get("TRE_RUN_BENCHMARK"),
    reason="perf benchmark -- run explicitly with: "
    "TRE_RUN_BENCHMARK=1 pytest tests/test_virtual_list_benchmark.py -v -s",
)
def test_materialize_callback_gil_overhead_scales_to_100k_rows():
    window = Window(width=200, height=200)
    built = []

    def materialize(idx):
        built.append(idx)
        return window.create("box", fill=(idx % 256, 0, 0, 255))

    # One row tall: each one-row scroll builds exactly one new row.
    rows = window.create(
        "virtual_list", item_count=ROW_COUNT, item_extent=ROW_EXTENT,
        materialize=materialize, width=100, height=ROW_EXTENT,
    )
    window.root.add_child(rows)
    window.advance(0)

    def scroll_one_row():
        window.simulate("wheel", node=rows, delta_y=ROW_EXTENT)
        window.advance(0)

    # Warm-up: the first steps pay one-time allocation costs a long-running
    # app wouldn't keep paying -- excluded from the timing, as
    # `frame_budget.rs` excludes pipeline compilation.
    for _ in range(WARMUP_ROWS):
        scroll_one_row()

    before = len(built)
    timed_calls = ROW_COUNT - 1 - WARMUP_ROWS
    start = time.perf_counter()
    for _ in range(timed_calls):
        scroll_one_row()
    elapsed_s = time.perf_counter() - start
    assert len(built) - before == timed_calls, "each step must build exactly one row"

    micros_per_call = (elapsed_s / timed_calls) * 1_000_000

    print(
        f"\n{timed_calls} scroll-by-one steps "
        f"(each: 1 wheel + 1 materialize callback across the GIL) "
        f"in {elapsed_s:.4f}s -- {micros_per_call:.3f}us/step "
        f"(enforced ceiling: {MAX_MICROS_PER_CALL}us/step)"
    )

    assert micros_per_call < MAX_MICROS_PER_CALL, (
        f"materialize callback overhead {micros_per_call:.3f}us/call exceeds the "
        f"{MAX_MICROS_PER_CALL}us/call ceiling across {timed_calls} calls -- if this "
        f"is a real regression, the mitigation is to batch "
        f"the callback (materialize a range, not one index at a time)"
    )
