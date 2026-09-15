# Plan: Phase 5, Step 5.3.3 -- The Capstone: A Real Rendered Scene, Verified Live

## Scope decisions

**What this capstone actually proves, that neither 5.3.1 nor 5.3.2
individually could.** Step 5.3.1 proved `tag_accessibility_node`'s IR-
level math is correct (a unit test computed a rotated rect's real
bounding box and compared it against `flatten()`'s own output -- no GPU,
no D-Bus). Step 5.3.2 proved `tre-a11y` publishes correctly onto a real
AT-SPI2 bus (a real D-Bus round trip -- but the "tagged node" was a
synthetic literal, never actually rendered anywhere). Neither step, on
its own, proves DESIGN.md Section 5.2's real claim: "$100\%$ alignment
between what is visually rendered on screen and what is reported to
assistive technology." This sub-step is the first (and last, by design
-- 5.3 closes here) place a *single* real render produces data that
flows through *all three* systems at once -- the GPU framebuffer, the
tagged `AccessibilityNode` list, and the real AT-SPI2 bus -- checked
against each other, not verified in three separate isolated tests that
each merely trust the others' contracts.

**One new demo, `canvas_accessibility_demo.rs`, in `tre-rhi-vulkan`**,
following every established demo convention exactly: `HeadlessSwapchain`
+ `VulkanDevice` real GPU rendering, the shared `support/
pixel_helpers.rs::bgra_pixel_at` for readback (not a new, one-off
closure -- REVIEW.md finding #93/#108 already exists specifically
because that pattern went wrong once), and a `demo/phase5_step5_3_3/`
folder (README, run script, output PNG) matching every prior demo-
bearing step.

**The scene draws 3 rects, each both rendered and tagged from the exact
same local `(x, y, width, height)` numbers** -- one plain
`AccessibilityRole::Generic`, one plain `AccessibilityRole::Button`, and
one **rotated** `AccessibilityRole::Image` via
`Affine2::from_translation_rotation_scale`. The rotated one is the
deliberate centerpiece: it is the one case 5.3.1's own `PLAN.md` called
"the one genuinely tricky correctness case this sub-step exists to get
right," but 5.3.1 only ever checked it at the IR level. Here, the same
rotated rect's real transformed center point
(`transform.transform_point([width / 2.0, height / 2.0])`) is used to
pick the exact pixel to read back from the GPU framebuffer *and* is
inside the exact bounds independently re-derived and checked against
the real AT-SPI2 `Component.GetExtents` response -- three independent
checks (pixel color, IR bounds, AT-SPI2-reported bounds) against one
shared source of truth, closing the loop for the hardest case, not just
the easy translate-only ones. `AccessibilityRole::TextLabel` is not
separately re-exercised -- Step 5.3.2's own unit tests already cover
role-mapping exhaustively at the CPU level; this sub-step's job is
proving the *real, combined* pipeline, not re-proving role coverage.

**No clipping, overlays, or multi-threading in this scene.** Clip-stack
intersection is 5.3.1's own disclosed, still-open gap -- introducing a
clip here would make a real mismatch (clipped pixels vs. unclipped
reported bounds) look like a new bug rather than the already-disclosed
limitation it actually is. Multi-threaded tagging-and-stitching was
already the star of Step 5.2.3's own capstone and is already unit-
tested for accessibility nodes specifically in 5.3.1's own 4-worker-
thread test; repeating it here would re-prove already-closed ground
rather than close new ground, which is what a capstone should do.

**The demo is its own verifier, matching every prior demo's own
self-contained assert-and-print pattern** (`walking_skeleton`,
`canvas_sub_canvas_demo`, etc.) -- not a separate `#[test]`. It publishes
via a real `tre_a11y::A11yBridge` (a new `tre-rhi-vulkan` dev-dependency,
`examples`-only, matching how `tre-svg`/`tre-text`/`tre-atlas` are
already dev-dependencies there for other demos), then acts as its own
second, independent AT-SPI2 client (`zbus`, also a new dev-dependency,
matching `tre-a11y`'s own test's exact discovery pattern: poll the real
registry, filter by a per-run-unique `ToolkitName`, descend through the
synthesized root to the tagged nodes) to query back what it just
published -- the same "publish steadily on a background thread while
the main thread verifies" shape `tre-a11y`'s own round-trip test already
established, reused rather than reinvented.

**CI:** this demo needs both a virtual display *and* a real D-Bus/AT-
SPI2 bus, but currently only `vulkan-validation` has the former and only
`test` has the latter. Rather than merging the two jobs or duplicating
Vulkan's setup into `test`, `vulkan-validation` gains `dbus-user-session`/
`at-spi2-core` in its own install list (small, cheap packages, matching
that job's existing "install exactly what's needed" pattern) and this
one demo's own run line is wrapped as `dbus-run-session -- xvfb-run -a
cargo run ...` -- every other line in that job is untouched, keeping the
blast radius to exactly the one command that needs both real
environments at once.

## Goal

A real GPU-rendered scene of 3 rects (one rotated) is simultaneously:
(1) read back from the real Vulkan framebuffer and confirmed to show
real, correctly-positioned, non-background pixels at each rect's real
transformed center; (2) tagged via `Canvas::tag_accessibility_node`
using the identical local coordinates the same draw calls used; and (3)
published via a real `tre_a11y::A11yBridge` and independently re-queried
from a real, live AT-SPI2 registry, confirming `Component.GetExtents`
for every tagged node matches the same real bounds the pixel check and
the IR both agree on -- closing Step 5.3 (5.3.1-5.3.3) and DESIGN.md
Section 5.2's "100% alignment" claim in full, for Linux.

## Tasks

1. **New dev-dependencies**: `tre-a11y = { path = "../tre-a11y" }` and
   `zbus = "4.4.0"` (matching the version already resolved elsewhere in
   the workspace) added to `tre-rhi-vulkan`'s `[dev-dependencies]`.

2. **The scene**: 3 `draw_rounded_rect` calls plus 3 matching
   `tag_accessibility_node` calls from the same local coordinates --
   `Generic` (plain), `Button` (plain), `Image` (via
   `save()`/`transform(&Affine2::from_translation_rotation_scale(...))`/
   `restore()`, matching this crate's own established transform-scoping
   convention). Real GPU render via `HeadlessSwapchain`/`VulkanDevice`,
   matching every prior canvas demo's own setup exactly.

3. **Pixel verification**: for each of the 3 rects, compute its real
   transformed center point and assert `bgra_pixel_at` at that pixel is
   the real, expected, non-background color -- the same "prove it
   actually rendered" check every prior demo already makes, extended to
   a point derived from the *same* transform used for tagging (Task 4),
   not an independently-guessed coordinate.

4. **Accessibility publish + real AT-SPI2 verification**: connect a real
   `A11yBridge` with a per-run-unique `toolkit_name`; publish
   `frame.accessibility_nodes` on a background thread (steady republish,
   matching `tre-a11y`'s own round-trip test); on the main thread, poll
   the real registry for the app, descend to the synthesized root's
   children, and assert each tagged node's real `Component.GetExtents`
   matches the same bounds Task 3 already confirmed on screen (hand-
   computed once per rect, shared by both checks) -- including the
   rotated rect's real axis-aligned bounding box, not just the two plain
   ones.

5. **CI**: add `dbus-user-session`/`at-spi2-core` to `vulkan-validation`'s
   install list; add this demo's own run step, wrapped in
   `dbus-run-session -- xvfb-run -a cargo run -p tre-rhi-vulkan --example
   canvas_accessibility_demo`.

6. **`demo/phase5_step5_3_3/`**: README, run script, output PNG --
   matching every prior demo-bearing step's own convention exactly.

7. **Docs**: IMPLEMENTATION.md Step 5.3.3 subsection, explicitly closing
   Step 5.3 (5.3.1-5.3.3) in full, matching Step 5.2.3's own "closes
   Step 5.2 in full" precedent; DESIGN.md Section 5.2's existing status
   notes gain a final entry declaring the "100% alignment" claim proven
   end to end (for Linux); REVIEW.md entry.

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace.
- The demo itself is the load-bearing proof: real GPU pixels, real IR
  bounds, and a real AT-SPI2 query all checked against one shared source
  of truth in one process, one render.
- All pre-existing examples re-run manually as a regression check, since
  this is the first time `tre-rhi-vulkan` depends on `tre-a11y` at all
  (a purely additive dev-dependency, so this is the lighter "did adding
  it break an unrelated build" check Step 5.3.2 used for its own
  workspace-member addition, not a full behavioral re-verification).

## Explicitly out of scope for this sub-step

- Windows UIA / macOS NSAccessibility -- still deferred (unchanged from
  5.3.2's own scope).
- Clip-stack intersection of reported bounds -- still 5.3.1's own
  disclosed, open gap; this sub-step's scene deliberately avoids
  exercising it rather than papering over it.
- Multi-threaded tagging-and-stitching in the rendered demo -- already
  proven at the IR level (5.3.1) and as Step 5.2.3's own capstone;
  re-proving it here would not close new ground.
- Human-readable accessible names/labels, real UI action dispatch,
  incremental `TreeUpdate` diffing -- all still 5.3.2's own disclosed
  gaps, unchanged by this sub-step.
- Any change to `tre-a11y`'s or `tre-engine`'s own public API -- this
  sub-step is purely a consumer of both, proving they compose correctly
  together in a real render.
