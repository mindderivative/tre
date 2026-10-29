"""0.5.9 (#243): `read_clipboard` and `write_clipboard` from a `LayerWindow`, over Wayland data-control.

tre reads and writes the clipboard through arboard's `wayland-data-control` feature, which binds
`ext_data_control_manager_v1` first and `zwlr_data_control_manager_v1` second (KWin 6.8 offers only
the ext one; sway offers both). Data-control needs no keyboard focus, so a bar that never takes the
keyboard can still use it. These tests prove that on a real compositor, with no key injection and no
external tool: one tre process writes, a second one reads.

Two things make the proof honest.

* **`DISPLAY` is unset in the children.** If the Wayland setup fails, arboard logs a warning and falls
  back to the X11 clipboard (XWayland) when `DISPLAY` is set, so a test that passes with it set proves
  nothing about data-control. Each child reports whether `DISPLAY` was set; the tests assert it was not.
* **A write is readable while its writer is alive.** The writing child stays up for the read. (A
  clipboard manager, Klipper on KDE, takes the text over and keeps it after the writer exits; without
  one the text goes with the writer. Nothing here relies on either.)

**This overwrites the real clipboard on a real desktop**, so it only runs there when asked:
`TRE_LIVE_CLIPBOARD=1`. Under `tools/sway_live.sh` the compositor is private, with a clipboard of its
own, so it runs without asking (`TRE_PRIVATE_COMPOSITOR=1`). The text that was on the clipboard is saved
in memory (never printed; failures name lengths, not contents), and put back at the end. That restoring
process keeps the text only as long as it lives unless a clipboard manager keeps it: with no manager, a
desktop's previous clipboard text is lost when the test ends. Text only: images and other formats are
not covered, nor the primary selection, nor pasting into a focused field.
"""

import json
import os
import select
import subprocess
import sys
import time
import uuid

import pytest

import tre

pytestmark = pytest.mark.skipif(
    not sys.platform.startswith("linux") or not tre.supports("layer_shell"),
    reason="needs a Wayland compositor with the layer-shell protocol",
)

OPTED_IN = os.environ.get("TRE_LIVE_CLIPBOARD") == "1" or os.environ.get("TRE_PRIVATE_COMPOSITOR") == "1"

CHILD = r"""
import json, os, sys, threading, time
from tre import App, LayerWindow

mode = sys.argv[1]
hold = float(sys.argv[2])
text = sys.stdin.read() if mode == "write" else None
app = App()
# A bar nobody sees or can click: 8 x 8, transparent, input passes through, no keyboard.
bar = LayerWindow("clipboard-live-test", layer="top", anchor=("bottom", "left"), size=(8, 8),
                  exclusive_zone=0, keyboard="none", input_region=[])
bar.root.set(fill=(0, 0, 0, 0))
app.add_window(bar)
handle = app.thread_handle()

def work():
    time.sleep(1.3)  # the surface is up
    done = threading.Event()
    result = {"display_set": bool(os.environ.get("DISPLAY"))}

    def on_loop():
        if mode == "read":
            result["text"] = bar.read_clipboard()
        else:
            result["ok"] = bar.write_clipboard(text)
        done.set()

    handle.call_soon(on_loop)
    done.wait(5)
    print(json.dumps(result), flush=True)
    time.sleep(hold)
    handle.stop()

threading.Thread(target=work, daemon=True).start()
app.run()
"""


class Child:
    """A tre process running one `LayerWindow`, with no X11 to fall back on."""

    def __init__(self, mode, text=None, hold=0.0):
        env = {k: v for k, v in os.environ.items() if k != "DISPLAY"}
        self.process = subprocess.Popen(
            [sys.executable, "-I", "-c", CHILD, mode, str(hold)],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, env=env,
        )
        if mode == "write":
            self.process.stdin.write(text)
        self.process.stdin.close()

    def result(self, timeout=20):
        end = time.time() + timeout
        while time.time() < end:
            if select.select([self.process.stdout], [], [], 0.2)[0]:
                line = self.process.stdout.readline()
                if line.startswith("{"):
                    return json.loads(line)
            if self.process.poll() is not None:
                break
        return None

    def finish(self, timeout=30):
        try:
            self.process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()


def read():
    child = Child("read")
    try:
        got = child.result()
    finally:
        child.finish()
    assert got is not None, "the reading process printed nothing"
    assert got["display_set"] is False, "DISPLAY leaked into the child: the X11 fallback could pass this"
    return got["text"]


def write_and_hold(text, hold):
    """Starts a process that writes `text` and stays alive `hold` seconds; returns it once written."""
    child = Child("write", text=text, hold=hold)
    got = child.result()
    assert got is not None, "the writing process printed nothing"
    assert got["display_set"] is False, "DISPLAY leaked into the child: the X11 fallback could pass this"
    assert got["ok"] is True, "write_clipboard reported failure"
    time.sleep(0.5)  # the compositor has the selection
    return child


@pytest.fixture(scope="module")
def clipboard():
    """Opt-in gate, a probe that data-control works at all, and the previous text put back after."""
    if not OPTED_IN:
        pytest.skip(
            "overwrites the real clipboard: set TRE_LIVE_CLIPBOARD=1 (a private compositor from "
            "tools/sway_live.sh needs no opt-in)"
        )
    previous = read()  # text or None; kept in memory, never printed
    yield previous
    if previous is not None:
        write_and_hold(previous, hold=2.0).finish()


def test_text_written_by_one_layer_window_is_read_by_another_process(clipboard):
    token = f"tre-clipboard-{uuid.uuid4().hex}"
    writer = write_and_hold(token, hold=6.0)
    try:
        got = read()
        assert got == token, f"read {len(got) if got is not None else None} chars, wrote {len(token)}"
    finally:
        writer.finish()


def test_multiline_unicode_text_survives_the_round_trip_exactly(clipboard):
    text = "first line\n\tindented — “quoted”\nsnowman ☃, kanji 日本語, emoji 🧩\nlast line without a newline"
    writer = write_and_hold(text, hold=6.0)
    try:
        got = read()
        assert got == text, f"read {len(got) if got is not None else None} chars, wrote {len(text)}"
    finally:
        writer.finish()


def test_a_later_write_from_another_process_replaces_the_earlier_one(clipboard):
    first = f"tre-first-{uuid.uuid4().hex}"
    second = f"tre-second-{uuid.uuid4().hex}"
    early = write_and_hold(first, hold=9.0)
    try:
        assert read() == first
        late = write_and_hold(second, hold=5.0)
        try:
            got = read()
            assert got == second, f"read {len(got) if got is not None else None} chars, wrote {len(second)}"
        finally:
            late.finish()
    finally:
        early.finish()
