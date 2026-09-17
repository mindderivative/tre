"""M16 Phase 2 (§3, §9): real, repeatable coverage that `RUST_LOG`
genuinely dials the real `tracing` subscriber's own verbosity --
`ensure_tracing_subscriber`'s own real, corrected `EnvFilter::
from_default_env()` wiring (`crates/engine-py/src/dispatch.rs`).

A real finding while building this phase, not caught by any unit test:
switching the subscriber install from `tracing_subscriber::fmt::
try_init()` (the free function, which specially wires `EnvFilter` for
you) to the builder chain `fmt().with_writer(...).try_init()` (needed
to correct stderr, since the builder's own default writer is stdout --
also a real finding, see `PLAN.md`/`LOG.md`) silently dropped `RUST_
LOG` support entirely -- every real app started emitting unconditional
INFO events, only caught by manually re-running an example script.

A real `tracing` subscriber can only ever be installed once per
process, and its filter is captured at construction time, not re-read
per event -- so this can't be tested by toggling `RUST_LOG` mid-
process (the already-installed subscriber wouldn't see the change).
Each case below runs a genuinely fresh `python3` subprocess instead,
the only real way to test what "no `RUST_LOG` set" vs. "`RUST_LOG=
info`" actually does to a brand-new process.
"""

import os
import subprocess
import sys

SCRIPT = """
from tre import App, Window

window = Window(width=100, height=100)
app = App()
app.add_window(window)
app.run(max_frames=1)
print("done")
"""


def run_with_env(extra_env):
    env = dict(os.environ)
    env.pop("RUST_LOG", None)
    env.update(extra_env)
    return subprocess.run(
        [sys.executable, "-c", SCRIPT],
        env=env,
        capture_output=True,
        text=True,
        timeout=30,
    )


def test_default_run_with_no_rust_log_emits_no_tracing_noise():
    result = run_with_env({})
    assert result.returncode == 0
    assert "done" in result.stdout
    assert result.stderr == "", (
        f"with no RUST_LOG set, the default ERROR-level filter must suppress every real "
        f"INFO-level event -- got stderr: {result.stderr!r}"
    )


def test_rust_log_info_genuinely_raises_the_real_verbosity():
    result = run_with_env({"RUST_LOG": "info"})
    assert result.returncode == 0
    assert "done" in result.stdout
    assert "INFO" in result.stderr, (
        f"RUST_LOG=info must genuinely surface real INFO-level tracing events on stderr -- "
        f"got: {result.stderr!r}"
    )
    assert "app_run" in result.stderr, "the real app_run span (M16 Phase 1) must appear too"
