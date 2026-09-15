# Log: Phase 5, Step 5.3.1 -- `Canvas::tag_accessibility_node`

## No design bugs found -- every scope decision held up on the first real run

Every call locked into `PLAN.md` before implementation began -- reducing
four transformed corners to a real axis-aligned min/max bounding box
(reusing `draw_rounded_rect`'s own multi-corner-transform technique,
then adding the min/max reduction specific to a11y); `f32` bounds with
no OS-native rounding yet; no clip-stack intersection; a small
4-variant starter `AccessibilityRole`; a flat per-frame node list with
no engine-built tree; mandatory `SubCanvas`/`FrameArena`/`stitch_into`
integration this sub-step, not deferred -- compiled and passed every
new test, including the rotation-correctness case and the real
4-worker-thread test, without needing to revise any of them.

## One expected mechanical fixup, not a design bug

`FrameArena::with_capacity` gained a fourth parameter
(`accessibility_capacity: usize`). Three pre-existing `tre-engine` unit
tests and `canvas_sub_canvas_demo.rs`'s own call site all needed `, 0`
appended, since none of them tag accessibility nodes. This was
anticipated in `PLAN.md`'s own Verification plan section before
implementation began (the same class of "every example touches this
shared code path" reasoning Step 5.2.2 already used), not discovered
as a surprise.

## The one genuinely tricky correctness case: rotation

A naive implementation of `tag_accessibility_node` -- transform only
the local rect's top-left corner and reuse its local width/height --
would silently produce the wrong bounds under any active rotation. The
rotation-correctness unit test makes this concrete: a 10x4 local rect
under a 90-degree CCW rotation has its four corners land near `(0,0)`,
`(0,10)`, `(-4,10)`, `(-4,0)`; the real axis-aligned bounding box of
those four points is `(-4, 0, 4, 10)` -- the opposite aspect ratio from
the naive approach's wrong `(0, 0, 10, 4)`. `f32::sin_cos` on
`FRAC_PI_2` doesn't land on exactly `0.0`/`1.0`, so the test compares
each of x/y/width/height within a small epsilon rather than asserting
exact equality (the only test this sub-step added that isn't an exact
literal comparison, unlike this crate's usual whole-number-position
convention).

## What else was verified, not just unit-tested in isolation

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets
  -- -D warnings` / `cargo build --workspace --all-targets` / `cargo test
  --workspace`: all clean, zero warnings, zero failures across every
  crate.
- `tre-engine` now has 48 unit tests (up from 44): a pure-translation
  sanity check (a node tagged inside a `transform(&Affine2::
  from_translation(10.0, 5.0))` block reports bounds shifted by exactly
  that offset); the rotation-correctness test described above; a
  `SubCanvas` tagging a node and calling `stitch_into` carries it into
  a `FrameArena` correctly (bounds and role both preserved exactly,
  since accessibility nodes need no rebasing at all when merged); and a
  real 4-worker-thread test extending Step 5.2.3's own capstone
  pattern -- each thread draws a rect *and* tags a node at the same
  position, `stitch_into`s into a shared `FrameArena`, and the final
  frame's `accessibility_nodes` list is searched (not assumed ordered)
  for each thread's own node, confirming survival with correct bounds
  regardless of scheduling.
- No new demo this sub-step, matching 5.2.1/5.2.2's own precedent:
  tagged data has nowhere real to go until 5.3.2's OS bridge exists;
  the real end-to-end proof (a live AT-SPI2/D-Bus query confirming
  reported bounds match what was actually rendered) is Step 5.3.3's
  capstone job.
- All 4 pre-existing examples touching the shared `flatten`/
  `stitch_into`/`FrameArena` code path (`canvas_state_stack_demo`,
  `canvas_draw_text_demo`, `canvas_batch_flattening_demo`,
  `canvas_sub_canvas_demo`) re-run manually against real Vulkan
  hardware: zero regressions, output unchanged from before this
  sub-step's changes.
- Docs: DESIGN.md Section 5.2 gained an "Implementation status" note
  (the established convention already used throughout TECHNICAL.md);
  IMPLEMENTATION.md gained the Step 5.3.1 subsection and a split note
  on Step 5.3's own Technical Rationale; REVIEW.md gained a "no
  numbered findings" entry, matching 5.2.2's own precedent for a
  sub-step whose design decisions all held up unchanged.
