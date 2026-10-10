"""0.5.6 (#159): `App.add_window` while `run()` is going.

Each case runs in its own process (it needs a display, and skips without one).
"""

import os
import subprocess
import sys
import textwrap

import pytest

pytestmark = pytest.mark.skipif(not os.environ.get("DISPLAY") and not os.environ.get("WAYLAND_DISPLAY"),
                                reason="needs a display")


def run(body: str) -> str:
    script = textwrap.dedent(
        """
        import sys
        from tre import App, Window

        def make(title):
            w = Window(width=200, height=120)
            w.set(title=title)
            box = w.create("box", width=60, height=40, fill=(255, 0, 0, 255))
            w.root.add_child(box)
            box.animate("opacity", 0.3, 600000)     # something animating: frames keep coming
            return w
        """
    ) + textwrap.dedent(body)
    result = subprocess.run([sys.executable, "-c", script], capture_output=True, text=True, timeout=60)
    assert result.returncode == 0, result.stderr
    return result.stdout.strip()


def test_a_window_added_from_a_frame_listener_opens_and_draws():
    out = run(
        """
        first, second = make("first"), make("second")
        app = App()
        app.add_window(first)
        state = {"added": False, "n": 0}

        def on_frame(event):
            state["n"] += 1
            if not state["added"]:
                state["added"] = True
                app.add_window(second)
            if second.frame_stats()["frames"] >= 5:
                first.close()
                second.close()
            elif state["n"] > 600:
                first.close()
                second.close()

        first.on("frame", on_frame)
        app.run()
        print(second.frame_stats()["frames"] >= 5)
        """
    )
    assert out.splitlines()[-1] == "True"


def test_closing_the_added_window_leaves_the_first_running():
    out = run(
        """
        first, second = make("first"), make("second")
        app = App()
        app.add_window(first)
        state = {"phase": 0, "frames_after": 0}

        def on_frame(event):
            if state["phase"] == 0:
                state["phase"] = 1
                app.add_window(second)
            elif state["phase"] == 1 and second.frame_stats()["frames"] >= 3:
                state["phase"] = 2
                second.close()
            elif state["phase"] == 2:
                state["frames_after"] += 1
                if state["frames_after"] >= 5:
                    first.close()

        first.on("frame", on_frame)
        app.run()
        print(state["frames_after"] >= 5)
        """
    )
    assert out.splitlines()[-1] == "True"


def test_a_window_that_is_already_open_raises():
    out = run(
        """
        first = make("first")
        app = App()
        app.add_window(first)
        seen = []

        def on_frame(event):
            if not seen:
                try:
                    app.add_window(first)
                except ValueError as err:
                    seen.append(str(err))
                first.close()

        first.on("frame", on_frame)
        app.run()
        print(seen)
        """
    )
    assert "already open" in out.splitlines()[-1]


def test_a_window_can_be_added_from_call_soon_on_another_thread():
    out = run(
        """
        import threading, time
        first, second = make("first"), make("second")
        app = App()
        app.add_window(first)
        handle = app.thread_handle()

        def later():
            time.sleep(0.5)
            handle.call_soon(lambda: app.add_window(second))

        def on_frame(event):
            if second.frame_stats()["frames"] >= 3:
                first.close()
                second.close()

        first.on("frame", on_frame)
        threading.Thread(target=later, daemon=True).start()
        app.run()
        print(second.frame_stats()["frames"] >= 3)
        """
    )
    assert out.splitlines()[-1] == "True"
