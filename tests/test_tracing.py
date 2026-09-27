"""`RUST_LOG` sets the `tracing` subscriber's verbosity: with it unset only
errors reach stderr; `RUST_LOG=info` surfaces INFO events and the `app_run`
span. The subscriber is installed once per process with its filter fixed at
that point, so each case runs a fresh `python3` subprocess.
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
        f"with no RUST_LOG set, the default ERROR-level filter must suppress every "
        f"INFO-level event -- got stderr: {result.stderr!r}"
    )


def test_rust_log_info_raises_the_verbosity():
    result = run_with_env({"RUST_LOG": "info"})
    assert result.returncode == 0
    assert "done" in result.stdout
    assert "INFO" in result.stderr, (
        f"RUST_LOG=info must surface INFO-level tracing events on stderr -- "
        f"got: {result.stderr!r}"
    )
    assert "app_run" in result.stderr, "the app_run span must appear too"
