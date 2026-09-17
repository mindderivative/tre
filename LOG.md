# Log: M25 Phase 1 — Real Animation Completion Callback Firing (§5, §9)

Corresponds to `BUILD_TRACKER.md` M25 Phase 1: diagnose and fix why
`on_complete` never fired for `examples/animation_completion.py`/
`container_transform.py`.

## Real finding: there was no engine bug at all

Traced the full real call chain end to end, reading every link
directly rather than assuming: `Node.animate(..., on_complete=cb)`
registers the callback into `CompletionRegistry` and mints a
`CompletionHandle`, `Animated::animate_to_with_completion` stores it
on the active animation, `Animated::tick` pushes it into `Tree::
tick_all`'s own `completed: Vec<CompletionHandle>` the instant
`elapsed >= duration` (true from the very first tick for `duration_ms
=0`), and `engine-py::app.rs`'s own per-frame loop calls `run_
completions` with exactly that vector. Every one of these, read
directly, was already correct — confirmed by adding `RUST_LOG=warn`
and finding the real, decisive clue instead: `no display available,
exiting cleanly`.

**Real root cause: this session's own test-running convention, not
the engine.** Every example script this whole session was run via
`env -u DISPLAY -u WAYLAND_DISPLAY ...`, adopted early on as a
"headless-CI-safe" habit. This sandboxed dev environment actually has
a real, working display the whole time (`DISPLAY=:0`, `WAYLAND_
DISPLAY=wayland-0`, a live `/tmp/.X11-unix/X0` socket, `Xvfb`
installed) — stripping those variables made `winit::EventLoop::build()`
fail with "neither WAYLAND_DISPLAY nor WAYLAND_SOCKET nor DISPLAY is
set," which `App::run`'s own real, deliberate, already-correct
"gracefully exit with `Ok(())` when no display is reachable" handling
(TRE v1 finding #261's own convention, confirmed via direct read)
caught and converted into a silent, successful, **zero-real-frames**
return. `animation_completion.py`/`container_transform.py` are the
two example scripts in this whole workspace that happen to assert on
a value only a real per-frame `tick_all` call can ever produce —
every other example either never reads back a ticked value at all, or
(the pixel tests) uses its own separate, real wgpu-adapter path that
never goes through `winit`'s `EventLoop` in the first place. So this
was the one real place the zero-frames condition was ever actually
*visible* — not because it was the one broken thing, but because it
was the one script capable of *detecting* it.

**Re-verified with the real display available (no env stripping):**
both scripts now pass — `on_complete` fires exactly once in each,
correctly. Re-ran all thirty-two examples this way: all thirty-two
pass. `cargo test --workspace --release`/pytest already never touched
`App.run()`'s own real winit loop the same way, so both suites were
unaffected by this the whole time regardless.

**Real, decisive confirmation this never touched CI:** `.github/
workflows/ci.yml` deliberately has no display at all (its own header
comment states this explicitly) and only ever runs `examples/
animate_rect.py` — a script whose own doc comment already states "App.
run() itself exits 0... when no display/GPU is reachable, so this
script is safe to run unattended," and which never asserts on a
ticked value. CI was never at risk from this; the false alarm was
confined entirely to this session's own local verification runs.

## What actually changed

Nothing in `engine-core`/`engine-py`/`engine-render` — there was
nothing to fix. Going forward this session, example scripts are run
with the real display available (no `env -u DISPLAY -u
WAYLAND_DISPLAY`), which is strictly more thorough verification (a
real winit `EventLoop`, a real GPU-adapter render loop, real per-frame
ticking) than the accidental zero-frames graceful exit this session
was silently accepting as "passed" for every prior milestone's own
example verification.

Full `cargo test --workspace --release`/clippy/fmt already clean
(untouched by this finding). `pytest` (187 passed, 1 pre-existing
skip, unaffected) and all thirty-two examples now confirmed to pass
with a real display and real per-frame execution, not just a graceful
zero-frame exit.

M25 Phase 1 is complete — no code fix was needed; the real fix was
correcting this session's own test-running convention and confirming,
with real verification, that the engine was correct the whole time.
