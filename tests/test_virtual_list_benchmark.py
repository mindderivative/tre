"""§14 build-order step 15's own named acceptance gate (ARCHITECTURE.md
§15 Risk Register, "Virtualization FFI shape" row): "Spike this against a
real large dataset at build-order step 15... if per-call GIL overhead
dominates, batch the callback." A stated-but-unmeasured number is the
exact §6 frame-budget mistake restated for this callback -- this is that
measurement, not an assumption.

Scrolls a 100,000-row `VirtualList` one row at a time (the worst-case
shape for this callback: exactly one `materialize` call per
`set_virtual_list_window` call, never several amortized together),
timing the real, already-warmed-up steady state. Each call crosses from
Rust back into a live Python callable via `Py<PyAny>::call1` -- the
actual, real overhead this Risk Register row is about, not a
synthetic Rust-only loop.

`skipif`-gated on an env var by default, matching `frame_budget.rs`'s own
"meaningless noise in the default fast test run" precedent -- a bare
`@pytest.mark.skip` has no pytest flag that un-skips it, so this needs an
actual condition to gate on, not just a marker. Run explicitly with:
    TRE_RUN_BENCHMARK=1 pytest tests/test_virtual_list_benchmark.py -v -s
"""

import os
import time

import pytest

from tre import Window

ROW_COUNT = 100_000
WARMUP_ROWS = 1_000
# Generous on purpose (see `frame_budget.rs`'s own "enforced but with
# real margin" precedent) -- this only needs to catch a real regression
# (e.g. an accidental per-call GIL re-attach, or a marshaling change that
# adds real allocation), not chase a specific microbenchmark number.
# A 16.6ms/60Hz frame has room for hundreds of calls this size before
# this callback alone would threaten the budget.
MAX_MICROS_PER_CALL = 200.0


@pytest.mark.skipif(
    not os.environ.get("TRE_RUN_BENCHMARK"),
    reason="perf benchmark -- run explicitly with: "
    "TRE_RUN_BENCHMARK=1 pytest tests/test_virtual_list_benchmark.py -v -s",
)
def test_materialize_callback_gil_overhead_scales_to_100k_rows():
    window = Window(width=200, height=200)

    def materialize(idx):
        return (idx % 256, 0, 0, 255)

    vl = window.add_virtual_list(item_count=ROW_COUNT, item_extent=20.0, materialize=materialize)

    # Warm-up: the first several calls pay one-time costs (hash map
    # growth in `PyWindow.materializers`/`VirtualListState.materialized`,
    # slotmap's own initial allocation) that a real, long-running app
    # wouldn't repeatedly pay -- excluded from the timed measurement for
    # the same reason `frame_budget.rs` excludes GPU pipeline compilation
    # from its own timed loop.
    for i in range(WARMUP_ROWS):
        window.set_virtual_list_window(vl, i, i + 1)

    start = time.perf_counter()
    for i in range(WARMUP_ROWS, ROW_COUNT):
        window.set_virtual_list_window(vl, i, i + 1)
    elapsed_s = time.perf_counter() - start

    timed_calls = ROW_COUNT - WARMUP_ROWS
    micros_per_call = (elapsed_s / timed_calls) * 1_000_000

    print(
        f"\nengine-py §14 step 15 Stage C: {timed_calls} scroll-by-one calls "
        f"(each: 1 recycle + 1 real materialize callback across the GIL) "
        f"in {elapsed_s:.4f}s -- {micros_per_call:.3f}us/call "
        f"(enforced ceiling: {MAX_MICROS_PER_CALL}us/call)"
    )

    assert micros_per_call < MAX_MICROS_PER_CALL, (
        f"materialize callback overhead {micros_per_call:.3f}us/call exceeds the "
        f"{MAX_MICROS_PER_CALL}us/call ceiling across {timed_calls} calls -- if this "
        f"is a real regression, the Risk Register's own stated mitigation is to batch "
        f"the callback (materialize a range, not one index at a time)"
    )
