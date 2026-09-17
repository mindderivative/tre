#!/usr/bin/env python3
"""M19 Phase 1's real hot-reload wiring (§16.4): `View.poll_reload()`
-- `ViewWatcher`/`Reconciler` both already existed as tested
`engine-spec` primitives, but nothing ever called them from a real,
live `View` before this phase.

`View` has no live-window/render-loop concept of its own (`view.rs`'s
own module doc comment, confirmed by `examples/two_way_binding.py`'s
own docstring) -- there's no `App.run()` here either. `poll_reload` is
a real, explicit method the caller invokes wherever its own script's
equivalent of "between frames" is, not something that fires
automatically on a timer.

What this script proves automatically (headless-CI-safe, no human
needed): a real edit to `hot_reload.yaml`, written to disk by this
same script (standing in for a developer editing the file in an
editor), is detected by a real filesystem watcher and reconciled into
the live `Tree` -- `box`'s own `corner_radius` genuinely changes from
what was loaded at construction -- the same real claim `crates/
engine-spec/src/reconcile.rs`'s own tests already prove at the
`Reconciler` level in isolation, reached here through the real Python
FFI surface for the first time.

Restores `hot_reload.yaml` to its own original content afterward, so a
repeat run (or a real `git status`) sees no stray diff.
"""

import time
from pathlib import Path

from tre import View

yaml_path = Path(__file__).parent / "hot_reload.yaml"
original_yaml = yaml_path.read_text()

view = View(str(yaml_path))
box = view.node("box")
print(f"before reload: corner_radius={box.get('corner_radius')}")
assert box.get("corner_radius") == 0.0

assert view.poll_reload() is False, "no real change yet -- must report False"

# A real edit, standing in for a developer's own editor save.
time.sleep(0.1)
yaml_path.write_text(original_yaml.replace("corner_radius: 0", "corner_radius: 16"))

deadline = time.time() + 5
changed = False
while time.time() < deadline:
    if view.poll_reload():
        changed = True
        break
    time.sleep(0.05)

print(f"real file edit detected: {changed}")
assert changed, "poll_reload never reported the real write within 5s"

# The real NodeId-preservation claim: a fresh Node handle for the same
# unchanged widget still reaches the identical live node, reading the
# reconciled value -- not a stale reference or a freshly-rebuilt one.
box_after = view.node("box")
print(f"after reload: corner_radius={box_after.get('corner_radius')}")
assert box_after.get("corner_radius") == 16.0

print("hot_reload.py: exited cleanly, a real file edit reached the live Tree")

yaml_path.write_text(original_yaml)
