# Log: Phase 5, Step 5.3.3 -- The Capstone: A Real Rendered Scene, Verified Live

## The whole point of a capstone: it caught what three isolated tests missed

Step 5.3.1's own unit tests never touched a real bus. Step 5.3.2's own
round-trip test never rendered anything and only ever tagged a
`Button`. Neither could have caught what this sub-step's very first
real run caught immediately: a `Generic`-tagged rect was completely
absent from the real AT-SPI2 registry's `GetChildren` response --
`left: 2, right: 3` on the very first attempt, not a rare flake.

## Bug 1 (real, Should-fix, REVIEW.md #124): `AccessibilityRole::Generic` had been invisible to real assistive technology since Step 5.3.2 shipped

Reading `accesskit_consumer::common_filter`'s own real source (the
filter `accesskit_atspi_common` actually applies before anything
reaches the platform tree) showed the root cause immediately:

```rust
if role == Role::GenericContainer || role == Role::TextRun {
    return FilterResult::ExcludeNode;
}
```

`tre-a11y`'s `map_role` (written in Step 5.3.2) mapped
`AccessibilityRole::Generic -> accesskit::Role::GenericContainer` --
but `GenericContainer`'s real semantics are ARIA's `role="none"`/
`"presentation"` (a "hide this from assistive technology" signal), the
exact opposite of what a caller tagging a real, generic UI element
wants. `Role::Unknown` (this enum's own `#[default]`) is not
special-cased anywhere in `common_filter` and is the correct match.

**Fix:** remapped `Generic -> Role::Unknown` in `tre-a11y::map_role`,
with `tre-a11y`'s own existing unit test updated to match. Re-ran
`tre-a11y`'s full test suite (all 6 tests, including its own real
round-trip test) to confirm the fix didn't disturb anything else, then
retried the capstone demo.

## Bug 2 (real, Nice-to-have, REVIEW.md #125): a failing assertion inside `thread::scope` hung the whole process instead of reporting

Before Bug 1 was fixed, the demo's very first real run didn't just
fail cleanly -- it hung. `cargo run` was moved to the background after
its 120-second foreground timeout with zero output yet captured.
`ps`/`/proc/<pid>/wchan` showed the process genuinely still running,
blocked in a `futex_wait`, with 3 open sockets (real D-Bus
connections) -- meaning it had gotten well past rendering and into the
accessibility-verification phase, not stuck at startup.

The real cause: the demo's `thread::scope` block spawns a background
thread that loops `bridge.publish(&nodes)` until a shared
`AtomicBool` flag goes false, while the main closure verifies via
D-Bus. The flag was only ever cleared at the very end of the main
closure, on the success path. When an assertion inside that closure
panicked (on Bug 1, still unfixed at this point), the closure never
reached that line -- and `thread::scope`'s own contract (every spawned
thread must be joined before the scope returns, even while unwinding)
meant the whole process waited forever for a thread that would never
exit.

**Fix:** an RAII guard (`StopOnDrop`, wrapping a reference to the
`AtomicBool`) whose `Drop` impl clears the flag unconditionally,
constructed as the first statement inside the scope's closure -- it
runs on any exit path, panic included, so the spawned thread's loop
condition becomes false and `thread::scope` can actually join it and
propagate the real panic promptly.

## Confirmed via 3+ consecutive real runs, not just one

After both fixes, the demo was re-run 3 times in direct succession
(plus once more via its own `run_canvas_accessibility_demo.sh`) against
this development machine's real, live AT-SPI2 stack -- identical,
correct output every time:

```
rect A (Generic): real AT-SPI2 GetExtents = (20, 20, 60, 40), GetRole = 67
rect B (Button): real AT-SPI2 GetExtents = (200, 20, 60, 40), GetRole = 43
rect C (Image, rotated): real AT-SPI2 GetExtents = (80, 120, 71, 64), GetRole = 27
```

Rect C's real, independently-queried bounds (`80, 120, 71, 64`) match
the rotated rect's real transformed bounding box -- confirmed by hand
estimate during design (corners transform to roughly x in [80, 152],
y in [120, 185)) and now confirmed for real, not just estimated.

## What else was verified

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets
  -- -D warnings` / `cargo build --workspace --all-targets` / `cargo test
  --workspace`: all clean, zero warnings, zero failures across every
  crate, including the fixed `tre-a11y` and the new demo.
- Pixel-level verification passed on every run: all 3 rects (including
  the rotated one, sampled at its own real transformed center point,
  computed via the same `Affine2` used to draw and tag it) render
  real, non-background pixels at their real positions; the gap between
  Rect A and Rect B stays background.
- `demo/phase5_step5_3_3/` added (README, run script, output PNG),
  matching every prior demo-bearing step's own convention. The run
  script itself was executed end to end as a final check, writing its
  output to the correct `demo/phase5_step5_3_3/` location.
- `vulkan-validation`'s CI job gained `dbus-user-session`/`at-spi2-core`
  in its install list, with only this one new demo's own run line
  wrapped in `dbus-run-session -- xvfb-run -a ...` -- every other line
  in that job untouched, keeping the blast radius to exactly the one
  command that needs both a virtual display and a real D-Bus/AT-SPI2
  bus at once.
- All pre-existing examples re-run manually as the lighter "did adding
  a new dev-dependency break an unrelated build" check (`tre-rhi-vulkan`
  now depends on `tre-a11y`/`zbus` for examples only, for the first
  time): `sdf_rounded_rect_demo` and `canvas_sub_canvas_demo` re-run,
  zero regressions.
- Docs: IMPLEMENTATION.md gained the Step 5.3.3 subsection, explicitly
  closing Step 5.3 (5.3.1-5.3.3) in full; DESIGN.md Section 5.2 gained a
  final status note declaring the "100% alignment" claim proven end to
  end for Linux; REVIEW.md gained two real, numbered findings (#124
  Should-fix, #125 Nice-to-have) -- unlike 5.2.2/5.3.1/5.3.2's own "no
  numbered findings" sub-steps, this capstone earned its numbers.
