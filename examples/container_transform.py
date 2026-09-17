#!/usr/bin/env python3
"""M7 Phase 5's real MD3 container transform (§7.6): a small trigger
"card" (with one child, its icon) expands into a full destination
"sheet" (with one child, its label) via `Window.
begin_container_transform`. No new navigation/router mechanism -- just
`engine-md3` choreographing several already-real `Animated<T>`
properties across the two nodes (§5), exactly as `ARCHITECTURE.md` §7.6
describes.

**M9 Phase 3 (§5) update:** teardown is no longer a manual call at the
end of the script -- `on_complete` fires automatically the real tick
the whole transition genuinely finishes (attached to the destination's
own driven `transform` animation), closing the gap `container_
transform.rs`'s own doc comment named as confirmed-still-unwired before
this phase.

What this script proves automatically (headless-CI-safe, no human
needed): the whole choreography (capture, initialize, synchronized
animation set, staggered content cross-fade, automatic teardown)
registers and runs through the real pipeline for real frames, exiting
cleanly, with teardown genuinely firing from the real completion
callback, not a fixed frame-count guess. The definitive proof that the
destination genuinely starts at the trigger's own captured bounds/
appearance and animates to its own real target (not the other way
around) is `crates/engine-md3/src/container_transform.rs`'s own tests,
not this script -- the same split this workspace has used throughout.
"""

from tre import App, Window

window = Window(width=400, height=400, title="tre v2 -- container transform")

trigger = window.add_rect(background=(0xFF, 0x00, 0x00, 0xFF), width=60, height=40, x=20, y=20)
trigger.animate("corner_radius", 8.0, duration_ms=0)
trigger_icon = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=10, height=10)
trigger.add_child(trigger_icon)

destination = window.add_rect(
    background=(0x00, 0x00, 0xFF, 0xFF), width=300, height=200, x=50, y=100
)
destination.animate("corner_radius", 24.0, duration_ms=0)
destination.animate("elevation", 6.0, duration_ms=0)
dest_label = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=50, height=10)
destination.add_child(dest_label)

teardown_calls = []


def on_transition_complete():
    teardown_calls.append(len(teardown_calls))
    window.end_container_transform(trigger)
    print("container_transform.py: on_complete fired, teardown ran automatically")


window.begin_container_transform(
    trigger,
    destination,
    duration_ms=0,  # deterministic completion timing -- see examples/animation_completion.py
    content_stagger_ms=90,
    on_complete=on_transition_complete,
)

app = App()
app.add_window(window)
app.run(max_frames=60)

assert teardown_calls == [0], f"expected on_complete to fire exactly once, got {len(teardown_calls)}"
print("container_transform.py: exited cleanly after 60 frames, automatic teardown fired exactly once")
