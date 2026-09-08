# Plan: Phase 5, Step 5.3.2 -- The Real Linux AT-SPI2 Bridge

## Scope decisions

**Built on `accesskit`/`accesskit_unix`, not a hand-rolled `zbus` AT-SPI2
implementation.** `accesskit` is a real, maintained cross-platform
accessibility crate already used by production GUI toolkits (egui,
Xilem, and others); its `accesskit_unix` backend already implements the
AT-SPI2 D-Bus wire protocol correctly, including real registry
embedding (`Socket.Embed`). This matches the project's own established
precedent of depending on a vetted crate for a complex, easy-to-get-
subtly-wrong wire format rather than reimplementing it from scratch
(`fdsm` for MSDF distance fields, `wide` for portable SIMD, `x11rb`/
`wayland-client` for windowing) -- the same reasoning, applied to the
AT-SPI2 wire protocol instead of a rendering format. It also sets up a
near-free path to Windows UIA / macOS NSAccessibility later, since
`accesskit` exposes one API across all three, even though only the
Linux backend is wired up this sub-step.

**A new `tre-a11y` crate**, not an addition to `tre-platform`.
`accesskit_unix::Adapter` needs no raw window handle at all (unlike its
Windows/macOS counterparts) -- it is a pure D-Bus service keyed on
application metadata, not on a `raw-window-handle` the way
`tre-platform`'s existing Wayland/X11 code is. Keeping it separate also
keeps `tre-platform`'s own real `unsafe` surface (XCB FFI,
`raw-window-handle` construction -- TECHNICAL.md Section 9.1's closed
list) uncontaminated by an unrelated concern. `tre-a11y` depends
directly on `tre-engine` (for `AccessibilityNode`/`AccessibilityNodeId`/
`AccessibilityRole`), the same direct-dependency shape `tre-platform`
already has on `tre-engine` -- not `tre-atlas`'s own "stay content-
agnostic behind a trait object" shape, since there is only ever one
content producer (the engine's own tagged-node list) here, not multiple
independent producers sharing a cache. `tre-a11y` carries
`#![forbid(unsafe_code)]`, same as `tre-engine`/`tre-text`/`tre-svg` --
`accesskit`/`accesskit_unix` are safe Rust APIs, so nothing in this
sub-step needs to extend TECHNICAL.md Section 9.1's closed 4-crate
`unsafe`-permitted list.

**A real background thread owns the `accesskit_unix::Adapter`,** fed by
a bounded (capacity-1) channel from whatever thread calls `publish()`,
using `try_send` and replacing (not queueing) a not-yet-consumed stale
frame. This is the concrete mechanism satisfying IMPLEMENTATION.md
Step 5.3's own task 2 wording ("a metadata extractor that runs in
parallel with the RHI submission phase... without blocking GPU
submission") -- the same "dedicated background thread, lock-free/
bounded handoff, never block the caller" shape already established by
the transient-pool GC thread (TECHNICAL.md Section 3.3) and the atlas
owner thread (Section 8's Multi-Window Atlas Concurrency). A dropped,
stale a11y frame under sustained backpressure is an accepted, disclosed
tradeoff -- DESIGN.md's own "Zero Layout Re-evaluation" promise is
about not recomputing layout for a11y, not a hard guarantee that every
single frame's a11y state reaches an assistive technology.

**One synthesized root `NodeId`, since 5.3.1's tagged nodes are a flat
list with no parent/child structure.** `accesskit::TreeUpdate` requires
exactly one root; every tagged `AccessibilityNode` becomes a child of a
single, stable, engine-owned root node (a reserved sentinel ID, chosen
so it can never collide with a real `AccessibilityNodeId` a caller
assigns). This is a genuine, load-bearing scope decision this sub-step
must get right, not an incidental implementation detail -- documented
here rather than left implicit in code.

**`AccessibilityRole` maps onto a small, explicit subset of
`accesskit::Role`** (`Generic`/`Button`/`TextLabel`/`Image` -> the
closest matching real `accesskit::Role` variant each) -- confirmed
against `accesskit`'s own real `Role` enum once it's a dependency
(Task 1), not guessed at in this plan. No attempt to consume
`accesskit::Role`'s full taxonomy, matching 5.3.1's own "starts with 4
concrete variants" precedent.

**Minimal, real (not stubbed-into-uselessness) `ActivationHandler`/
`ActionHandler`/`DeactivationHandler` implementations.** An initial-
tree request returns whatever was most recently published (an empty
root if `publish()` hasn't been called yet), matching real AT client
expectations. Action requests (an AT invoking "click this button") are
accepted -- the protocol requires *some* handler to exist for the
adapter to function -- but perform no UI effect: `tag_accessibility_node`
never asked for actionability, so dispatching a real UI action from
here would be scope invention, disclosed explicitly as out of scope
rather than silently no-op'd without comment. Deactivation is a no-op.

**No coordinate-space guessing.** `AccessibilityNode`'s bounds are
already real window/world-space pixels (5.3.1's own contract); whether
`accesskit`'s `Rect`/bounds convention wants root-relative or
screen-relative values, and whether the synthesized root itself needs
its own bounds set, is confirmed by reading `accesskit`/`accesskit_unix`'s
real source/docs once added as a dependency (Task 1) -- the same "read
the real API before writing code against it" discipline Step 5.3.1
used for `tre_math::Affine2` before hand-computing its rotation test.

**Verification needs a real D-Bus session bus, so CI needs one too.**
`tre-a11y`'s tests prove a real wire round trip (a second, independent
D-Bus connection queries the object `A11yBridge` published and asserts
the real values come back) rather than merely exercising `accesskit`'s
Rust types in-process -- matching this project's consistent "prove it
against the real thing" testing philosophy (the real 8-thread stress
tests, the real GPU pixel readbacks). A real desktop session normally
already runs `at-spi2-registryd`/a D-Bus session bus, so this needs no
extra setup for local development, but GitHub's hosted runners have
neither by default -- the same "headless CI is missing what a real
desktop already provides" gap `xvfb-run`/the software Vulkan ICD
already exist to close for the `vulkan-validation` job. Task 7 adds the
Linux packages and a `dbus-run-session`-wrapped invocation to close it
for D-Bus the same way.

## Goal

A new `tre-a11y` crate exposes `A11yBridge::connect(app_name,
toolkit_name, toolkit_version) -> Result<A11yBridge, A11yError>`, which
spawns a real background thread publishing a real, minimal AT-SPI2
accessible tree (one synthesized root plus one child per tagged
`AccessibilityNode`) onto the real Linux accessibility bus via
`accesskit_unix`. `A11yBridge::publish(&self, nodes: &[AccessibilityNode])`
hands a frame's tagged nodes to that thread without ever blocking the
caller, proven by a real timing-bounded test against an artificially
stalled consumer. A second, independent D-Bus connection in a real
integration test queries the exact object `A11yBridge` published --
`org.a11y.atspi.Component.GetExtents`, role, and the synthesized tree
shape -- and gets back the real values that were published, not a
mocked stand-in.

## Tasks

1. **Add `tre-a11y` to the workspace** (new crate directory, `Cargo.toml`
   depending on `tre-engine`, `accesskit`, `accesskit_unix`; add to the
   root `Cargo.toml`'s `members`). Read `accesskit`/`accesskit_unix`'s
   real, current docs/source for: the exact `Role` enum (for Task 2's
   mapping), the exact bounds/coordinate-space contract for `Node`
   (for Task 3), and `Adapter`'s real constructor and update-call
   signatures (for Task 4) -- resolving every API detail this plan
   deliberately left open above.

2. **`AccessibilityRole -> accesskit::Role` mapping** and
   `AccessibilityNode -> accesskit::Node` conversion (id, bounds, role;
   no name/label yet -- 5.3.1's schema carries none, a disclosed gap,
   not silently patched over with a placeholder string here).

3. **The synthesized single-root `TreeUpdate` builder**: one reserved
   root `NodeId`, every converted `AccessibilityNode` from the latest
   published frame as its child, confirmed against `accesskit`'s real
   coordinate-space contract (Task 1) rather than assumed.

4. **Minimal `ActivationHandler`/`ActionHandler`/`DeactivationHandler`**
   per the Scope decisions above.

5. **`A11yBridge`**: `connect(...)` spawns the real background thread
   owning a real `accesskit_unix::Adapter`; `publish(&self, nodes: &[
   AccessibilityNode])` does the bounded-channel, `try_send`-and-replace
   handoff (never blocks); `Drop` shuts the thread down cleanly (a real
   shutdown signal + `join`, not a detached/leaked thread).

6. **Tests** (all real, no mocked D-Bus layer):
   - a real round trip: `connect()`, `publish()` one node with known
     bounds/role, then a second, independent D-Bus connection queries
     the published object's real `Component.GetExtents`/role and
     asserts an exact match;
   - `publish()` never blocks: an artificially stalled consumer (a test
     hook or a deliberately slow first frame) plus a wall-clock bound
     on `publish()`'s own return time;
   - the role-mapping table, one assertion per `AccessibilityRole`
     variant;
   - construct-then-immediately-drop causes no panic and no hang (the
     background thread's shutdown path actually runs to completion).

7. **CI**: install a D-Bus session bus and `at-spi2-core` (for a real
   `at-spi2-registryd`, so `Socket.Embed`'s success path is genuinely
   exercised in CI, not just assumed to work because it's untested) in
   the `test` job (or a new dedicated job, whichever keeps the existing
   job's runtime and failure surface cleanest -- decide during
   implementation once the real package list and startup sequence are
   known); wrap the relevant test invocation in `dbus-run-session`,
   matching `vulkan-validation`'s own "install what's missing, wrap
   what needs wrapping" precedent for a headless runner.

8. **Docs**: IMPLEMENTATION.md Step 5.3.2 subsection; TECHNICAL.md
   Section 2.3 "Implementation status" note (Linux real via `accesskit`/
   `accesskit_unix`; Windows UIA/macOS NSAccessibility still deferred);
   extend DESIGN.md Section 5.2's existing 5.3.1 status note now that
   the "OS Accessibility Bridge" bullet is real for Linux; REVIEW.md
   entry.

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace, including the new `tre-a11y` crate.
- The real D-Bus round-trip test is the load-bearing proof this
  sub-step exists for -- a second connection genuinely querying what
  was published, not an in-process assertion on Rust values alone.
- No new `tre-rhi-vulkan` demo this sub-step, matching 5.2.1/5.2.2/
  5.3.1's own precedent, and DESIGN.md Section 5's own framing
  ("Event, Signal & Accessibility Architecture (Decoupled from
  Rendering)") -- this feature has no rendering dependency at all. The
  real end-to-end proof, wiring a rendered scene's tagged nodes through
  this bridge and confirming a live query matches what was actually
  drawn, is Step 5.3.3's capstone job.
- All pre-existing examples re-run manually as a regression check --
  `tre-a11y` is a new, additive crate touching no existing shared code
  path, so this is a lighter check than 5.3.1's (confirming nothing
  about adding it to the workspace broke an unrelated build), not a
  full behavioral re-verification.

## Explicitly out of scope for this sub-step

- Windows UIA / macOS NSAccessibility -- `accesskit` supports both, but
  wiring either up is real, untested-by-us work belonging to a later
  step, matching Steps 1.1/1.2/4.1's own "Linux complete; Windows/macOS
  deferred" precedent.
- Real UI action dispatch from AT-SPI2 `Action` requests -- accepted
  but a no-op, since `tag_accessibility_node`'s own signature never
  asked for actionability.
- Human-readable accessible names/labels/descriptions -- 5.3.1's schema
  carries none; nodes publish with role + bounds only. A real, disclosed
  gap against a genuinely useful screen-reader experience, likely the
  next schema extension once a real consumer (5.3.3, or a future UI
  framework) demands it.
- Any tree hierarchy beyond one synthesized root with flat children --
  matching 5.3.1's own "no parent/child tree" scope; the UI framework
  still owns the real hierarchy.
- Wiring this bridge into any GPU-rendered example -- Step 5.3.3.
- Incremental/diffed `TreeUpdate`s -- every `publish()` rebuilds and
  sends the full node list; per-node add/update/remove diffing is a
  legitimate later optimization, not a correctness gap this sub-step
  must close.
