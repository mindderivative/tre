# Log: Phase 5, Step 5.1.3 -- Real Sort Key, Overlay Routing, Real Batch Flattening

## The new logic itself: no bugs -- every design decision held up on the first real run

`compute_sort_key`, `begin_overlay`/`end_overlay`, and `flatten_run`'s
sort-and-merge algorithm all compiled, passed clippy pedantic, and
passed every new unit test (including a direct reproduction of
DESIGN.md Section 8's own worked example) on the first attempt. The
"markers are hard barriers" design decision -- the riskiest call in
`PLAN.md` -- turned out to be exactly right with no surprises: it made
the merge algorithm simpler (no need to actually verify `clipBounds`
equality within a run, since it's guaranteed by construction) while
staying strictly correct.

## Two real, predicted-in-advance regressions in pre-existing demos

Both are the exact class of breakage `PLAN.md` anticipated ("any two
adjacent `DrawGeometry` commands sharing Layer+Pipeline+Texture+
`clip_bounds` now merge into one command") -- caught by re-running
every pre-existing example, not by the type checker (both files still
compiled fine; only their own runtime assertions failed):

- **`canvas_state_stack_demo.rs`**: `RECT_C_COMMAND_INDEX` assumed Rect
  A and Rect B (both default Layer/Pipeline/Texture, drawn before any
  `push_clip`) could never merge. They now do. `PLAN.md` named this
  exact case in advance (Task 8) -- fixed by updating the constant from
  `3` to `2` and rewriting the surrounding comment.
- **`canvas_draw_text_demo.rs`**: the frame-2 assertion expected one
  command per glyph. All 4 glyphs of "TEXT" share Layer+Pipeline+
  Texture+`clip_bounds`, so they now merge into one command
  (`element_count == 24`). `PLAN.md` anticipated this *category* of
  regression generically but did not name this specific file -- found
  during the "re-run every pre-existing example" verification pass.

REVIEW.md findings #120/#121 record both, filed as Nice-to-have (a
correctly-predicted, intentional behavior change requiring a test
update, not a design flaw).

## What else was verified, not just unit-tested in isolation

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets
  -- -D warnings` / `cargo build --workspace --all-targets` / `cargo test
  --workspace`: all clean, zero warnings, zero failures across every
  crate. `tre-engine` now has 36 unit tests (up from 24): 12 new ones
  covering `compute_sort_key`'s bit-packing and overflow asserts,
  `next_depth_id`'s monotonic-never-resets guarantee,
  `begin_overlay`/`end_overlay`'s Layer ID assignment, clip reset/
  restore, and balance panics, the DESIGN.md Section 8 worked example
  reproduced exactly (3 batches from 4 logical draws), a white-box test
  of `flatten_run`'s `clip_bounds` check (unreachable via the public API
  today -- `clip_bounds` only ever changes alongside a marker command,
  which is already a hard barrier on its own -- kept as a forward-looking
  regression guard), and confirmation that an intervening `push_clip`/
  `pop_clip` pair stays a hard barrier even when its net clip effect is
  unchanged.
- New capstone example `canvas_batch_flattening_demo` reproduces
  DESIGN.md Section 8's worked example with real geometry: two SDF
  rects that merge, one that stays separate via `begin_overlay`, and one
  real MSDF text glyph. Passed on its first real run -- both the
  IR-level assertion (exactly 3 batches) and the real GPU pixel readback
  (all 4 logical shapes render at their own correct, distinct position,
  with an explicit background check between the two merged rects
  proving they're genuinely two shapes, not one overdrawn blob). This is
  the first demo in this codebase to record more than one `draw_indexed`
  call and switch pipelines (`sdf_rounded_rect` and
  `bindless_textured.vert`/`msdf.frag`) within a single frame --
  `bind_texture(0, u32::MAX)` (the sentinel "no texture" value) is
  called explicitly before each SDF-rect draw to reset any stale
  `texture_index` state left over from a preceding textured draw in the
  same command buffer, since `VulkanCommandBuffer::texture_index`
  persists across `draw_indexed` calls and no prior demo needed to
  reset it mid-frame.
- Every one of the 19 pre-existing examples (18 prior plus this step's
  own new one) re-run manually end to end against real Vulkan hardware:
  zero validation errors, only the two named regressions above needed a
  real fix, nothing else broke.
- Added `canvas_batch_flattening_demo` to the `vulkan-validation` CI job.
- This closes Step 5.1 (5.1.1, 5.1.2, 5.1.3) in full.
