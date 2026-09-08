# Demo: Phase 5, Step 5.3.3 -- The Capstone: A Real Rendered Scene, Verified Live

```bash
./demo/phase5_step5_3_3/run_canvas_accessibility_demo.sh
```

Closes Step 5.3 (5.3.1-5.3.3) in full. Step 5.3.1 proved
`tag_accessibility_node`'s IR-level math is correct (a unit test, no
GPU, no D-Bus). Step 5.3.2 proved `tre-a11y` publishes correctly onto a
real AT-SPI2 bus (a real D-Bus round trip, but the tagged node was a
synthetic literal, never actually rendered). This demo is the first
place a *single* real render produces data that flows through *all
three* systems at once -- the GPU framebuffer, the tagged
`AccessibilityNode` IR, and a real, live AT-SPI2 bus -- checked against
each other, not three isolated tests that each merely trust the others'
contracts.

Three rects are drawn and tagged from the exact same local coordinates:
a plain `Generic` rect, a plain `Button` rect, and a **rotated** `Image`
rect via `Affine2::from_translation_rotation_scale` -- the one
genuinely tricky case Step 5.3.1's own plan singled out, here proven end
to end for the first time. The rotated rect's real transformed center
point is used both to pick the exact pixel read back from the GPU
framebuffer and to derive the bounds independently checked against the
real AT-SPI2 `Component.GetExtents` response.

**A real, previously-undiscovered regression this demo caught.**
`tre-a11y`'s `AccessibilityRole::Generic` mapped to `accesskit::Role::
GenericContainer` since Step 5.3.2 shipped -- but
`accesskit_consumer::common_filter` hard-codes `GenericContainer` as
always excluded from the platform accessibility tree entirely (its real
semantics are ARIA's `role="none"`/`"presentation"`, the opposite of
what a caller tagging a real element wants). Every `Generic`-tagged
node had been silently invisible to real assistive technology the whole
time -- undetected because Step 5.3.2's own tests never happened to tag
a `Generic` node against a live bus. Fixed here to map onto
`Role::Unknown` instead (confirmed via `accesskit_consumer`'s own filter
source to be the correct, unfiltered choice) -- exactly the kind of gap
a real, combined, end-to-end capstone exists to catch that three
isolated unit tests did not.

A second, real timing bug surfaced during this demo's own first-draft
development: a `thread::scope` whose main closure panicked (on the very
same regression above, before the fix) hung forever instead of
reporting the failure, because the spawned "keep publishing" thread's
stop flag was never set on the panicking path. Fixed with an RAII guard
that clears the flag on any exit from the scope's closure, panic or not
-- the assertion failure now reports promptly rather than hanging the
whole process.

**A real, two-process split, not a single self-verifying binary.**
A seven-real-push CI investigation (REVIEW.md finding #126) found the
actual reason this demo kept failing in CI: `accesskit_unix`'s own
background thread, which does the real AT-SPI2 registration, cannot
complete that registration inside a process that also links real
Vulkan/X11 shared libraries. `canvas_accessibility_demo` now only
renders and tags -- it hands its tagged nodes to
`canvas_accessibility_verify`, a separate binary confirmed via `ldd` to
link zero Vulkan/X11/Wayland libraries, which does the real publish and
verify. This also matches real AT-SPI2 practice more closely than a
self-verifying process ever did: a real screen reader is always a
separate process from the application it inspects.
