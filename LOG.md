# Log: CI fix — `App::run` needed a real, first-party INFO event (§3, §9)

Found while checking CI on the freshly-pushed M15/M16/M17 commits
(this was the first CI run to ever include them — the eight commits
were pushed together, so no intermediate commit in that range had run
through CI individually). Not part of the M15/M16/M17 scoping itself;
a real, previously-unvalidated gap in M16 Phase 2's own test, only
surfacing now that CI actually ran against this code for the first
time.

## What was found

`test_rust_log_info_genuinely_raises_the_real_verbosity`
(`tests/test_tracing.py`, M16 Phase 2) failed in the Linux `test` job:
`RUST_LOG=info` produced only a `WARN`-level "no display available,
exiting cleanly" line, no `INFO` anywhere in stderr. Investigated, not
assumed: `crates/engine-py/src/app.rs` has exactly one `tracing::
info_span!("app_run", ...)` and zero bare `tracing::info!` events
anywhere in the crate. A span alone produces no visible output under
`tracing_subscriber::fmt`'s default formatting (no `.with_span_events`
configured) — it only prefixes *nested* events with its own context.
The test only ever passed locally because a real display let a real
`wgpu` adapter get created, and `wgpu_hal` happens to log its own
internal `INFO`-level messages as a side effect, incidentally landing
inside the `app_run` span's context. CI's `test` job deliberately has
no Xvfb (`.github/workflows/ci.yml`'s own header comment states this
explicitly), so `App::run` always takes the "no display available"
early-exit path there — never reaching adapter creation, so never
getting any incidental `wgpu_hal` logging, so never producing a real
`INFO` line at all.

## The fix

A real, deliberate `tracing::info!("starting a real app run session")`
now fires immediately after the `app_run` span is entered, in
`App::run` (`crates/engine-py/src/app.rs`) — unconditionally, before
any code path that can early-exit (no adapter, no display). This is
genuine, permanent instrumentation, not test scaffolding: before this
fix, `RUST_LOG=info` on a real, successful run gave a user nothing
first-party to look at at all, only whatever a transitive dependency
happened to log — a real observability gap, not just a test-fragility
one. The fix makes the "a real run session started" claim provable in
every environment, display or no display, adapter or no adapter — the
same thing the test was always trying to prove.

## Verification

Reproduced the CI failure locally first, not guessed at: `env -u
DISPLAY -u WAYLAND_DISPLAY pytest tests/test_tracing.py -v` failed
identically before the fix, passed after it. Full `cargo test
--workspace --release`/`cargo clippy --workspace --all-targets -- -D
warnings`/`cargo fmt --check` clean. `maturin develop --release` +
full `pytest tests/` run twice — once with a real display, once with
`DISPLAY`/`WAYLAND_DISPLAY` both unset (matching CI exactly) — both
161 passed, 1 skipped. All twenty-four examples re-run with a display;
`examples/animate_rect.py` (the one CI itself runs) additionally
re-run with no display, exiting cleanly as expected.
