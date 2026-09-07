# Log: Phase 5, Step 5.2.1 -- SubCanvas & Thread-Local Recording

## No real bugs found -- every design decision held up on the first real run

`SubCanvas`, the shared `Arc<AtomicU32>` Depth ID counter, and the
compare-exchange-based concurrency cap all compiled, passed clippy
pedantic, and passed every new unit test -- including the real 4-thread
concurrent stress test -- on the first attempt. The riskiest call in
`PLAN.md` (promoting only `next_depth_id` to a shared atomic, leaving
Layer ID and everything else thread-local) turned out to need no
revision at all once implemented.

## One mechanical fix during implementation (not a design bug)

`compare_exchange_weak`'s `Ok` arm binds the *previous* value on
success (a `usize`, since the compare succeeded and matches `current`),
not `()` -- my first draft wrote `Ok(()) => break` by habit (matching
the shape of a `Result<(), E>` match elsewhere in this file) and the
compiler caught the mismatch immediately. Fixed to `Ok(_) => break`.

## What else was verified, not just unit-tested in isolation

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets
  -- -D warnings` / `cargo build --workspace --all-targets` / `cargo test
  --workspace`: all clean, zero warnings, zero failures across every
  crate. `tre-engine` now has 41 unit tests (up from 36): 5 new ones for
  `SubCanvas` --
  - a real concurrency test: 4 real `std::thread::spawn` threads, each
    holding its own `SubCanvas` (via a `#[cfg(test)]`-only
    `new_with_sub_canvas_cap(4)` root, avoiding any dependence on the
    test runner's actual core count), each recording 50
    `draw_rounded_rect` calls, each reporting its own commands' Depth
    IDs back through its own `JoinHandle`; the main thread confirms all
    200 collected Depth IDs are pairwise distinct after a sort+dedup;
  - the cap panics on exactly the call that would exceed it, not
    before or after;
  - dropping a `SubCanvas` frees its slot for a subsequent
    `create_sub_canvas()` to reuse;
  - two independent sub-canvases both drawing at Layer 0, and both
    calling `begin_overlay(OverlayLayerPriority(0))`, is not an error --
    contrasting Layer ID's lack of a uniqueness requirement against
    Depth ID's real one;
  - a `SubCanvas`'s own `save`/`transform`/`push_clip`/
    `draw_rounded_rect` calls work identically to the root canvas's via
    `Deref`/`DerefMut` delegation (the same translated-vertex-position
    assertion style the root canvas's own `save`/`transform` test
    already uses).
- All 19 pre-existing examples re-run manually end to end against real
  Vulkan hardware: zero regressions. This mattered specifically because
  `next_sort_key`'s internal change (`fetch_add` on a shared atomic,
  replacing a plain `+= 1` on a private field) touches the exact code
  path every single example that draws anything goes through, even
  though none of them use `SubCanvas` yet.
- No new demo this sub-step, matching Steps 4.3.1/4.3.2's own
  precedent: a `SubCanvas` genuinely cannot be rendered without Step
  5.2.2's merge primitive (`RenderingCanvas::flatten(self)` takes
  `self` by value, which `Deref`/`DerefMut` cannot forward, and this
  sub-step adds no escape hatch of its own on purpose).
- Zero new dependencies confirmed -- `Arc`, `AtomicU32`, `AtomicUsize`,
  and `std::thread::available_parallelism` are all `std`; no
  `Cargo.toml` in the workspace changed for this sub-step.
