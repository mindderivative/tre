#!/usr/bin/env python3
"""M9 Phase 2's real Python-facing `on_complete` callbacks (§5):
`CompletionHandle`/`ActiveAnimation.on_complete` existed as real,
exported types since M3 Phase 5 step 9 but were genuinely unused
anywhere until this phase. `Node.animate(..., on_complete=callback)`
mints a real handle; `App.run()`'s own per-frame loop now drains
`Tree::tick_all`'s real completions and invokes the matching callback
exactly once, the real moment the animation genuinely finishes -- not a
frame-count guess.

What this script proves automatically (headless-CI-safe, no human
needed): a real animation, driven through the real render loop, calls
its own `on_complete` callback exactly once -- printed once, not on
every later frame. Uses `duration_ms=0` deliberately (see the comment
below) so completion is deterministic regardless of this headless
environment's own real, unpredictable software-rendered frame rate; the
definitive proof that a real, *non-zero* duration also reports its
completion exactly on the completing tick, never early, never twice, is
`engine-core`'s own `animation.rs` tests (M9 Phase 1), not this script.
"""

from tre import App, Window

window = Window(width=200, height=200, title="tre v2 -- animation completion")

card = window.add_rect(background=(0xFF, 0x00, 0x00, 0xFF), width=80, height=80)

calls = []


def on_fade_complete():
    calls.append(len(calls))
    print(f"animation_completion.py: on_complete fired (call #{len(calls)})")


# duration_ms=0 (§5's own real, existing semantics -- confirmed via
# `engine-core`'s own `zero_duration_animation_snaps_immediately_no_
# panic` test): `elapsed >= duration` is true from the very first real
# tick, so completion is deterministic regardless of this headless
# environment's own real (and unpredictable) software-rendered frame
# rate -- a real duration would risk `max_frames` running out before
# enough wall-clock time passed, especially in CI.
card.animate("opacity", 0.0, duration_ms=0, on_complete=on_fade_complete)

app = App()
app.add_window(window)
app.run(max_frames=60)

assert calls == [0], f"expected on_complete to fire exactly once, got {len(calls)} calls"
print("animation_completion.py: exited cleanly after 60 frames, on_complete fired exactly once")
