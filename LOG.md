# Log: M16 Phase 2 — Migrate Real Call Sites to `tracing` (§9)

Corresponds to `BUILD_TRACKER.md` M16 Phase 2, closing M16 entirely
(both phases). `dispatch.rs`'s `call_handler`/`run_completions` and
`app.rs`'s two `eprintln!` sites become real `tracing::error!`/
`tracing::warn!` events.

## Investigation before writing code

**Real finding beyond the original scoping:** `run_completions` had
its own real `err.print(py)` site the M16 scoping paragraph never
named — confirmed via grep, three real call sites to migrate, not two.
`PyErr` has no single method giving both the *full* traceback (frames
included) and an owned `String` — `err.traceback(py)`'s own real
`.format()` (`pyo3::types::PyTracebackMethods`, confirmed via direct
source read) is the real way to get it.

## What happened

New `dispatch::log_uncaught_exception(err, py)` renders the full
traceback (falling back to plain `Display` if missing/unformattable)
and emits `tracing::error!`. `call_handler`/`run_completions` both
call it in place of `err.print(py)`. `app.rs`'s two `eprintln!` sites
become `tracing::warn!` (expected, gracefully-handled conditions, not
errors).

**Real finding #1, caught immediately by running the full pytest
suite, not anticipated in `PLAN.md`:** three existing tests asserting
on `capsys.readouterr().err` started failing with empty captures, even
though the real event genuinely fired (visible in pytest's own
"Captured stderr call" section). `capsys` monkeypatches Python's
`sys.stderr` object; `tracing_subscriber`'s own writer is a raw OS-
level write from Rust that bypasses it entirely — `PyErr::print` (what
this phase replaces) *did* go through `sys.stderr` internally, so
`capsys` used to work by coincidence, not because it was the right
tool. Fixed by switching all three (plus a fourth, `test_two_way_
binding.py`'s own recursion regression test, which asserted `captured.
err == ""` — a real, silent-weakening risk: with `capsys`, that
assertion would trivially pass regardless of whether a real recursion
error fired underneath, silently losing the exact guarantee the test
exists for) to `capfd`, which captures at the file-descriptor level,
real for both Python and Rust writes.

**Real finding #2, caught only by manually running an example, not by
any pytest test:** `App::run`'s own subscriber install (M16 Phase 1)
is *not* the one guaranteed place a subscriber needs to be live —
`Window.click`/`Node.set_checked`/`View.click` (the whole no-live-
window-needed synthetic dispatch surface this project's own test
suite relies on since M4 Phase 1 step 3) are all real, independently
callable without `App::run()` ever running. Two pytest tests using
exactly those entry points captured empty stderr, because no
subscriber had been installed yet in that pytest process — cross-file
test *order* had been silently doing the installing until then, via
whichever file happened to call `App.run()` first (confirmed by
running the affected files in isolation, with `App.run()` never
called, and watching them still fail). Fixed by extracting subscriber
install into `dispatch::ensure_tracing_subscriber()`, called from both
`App::run` (early, for a real app) and `log_uncaught_exception` itself
(the one real place every uncaught-callback-exception log actually
funnels through) — `try_init` is already idempotent and cheap, so a
second call site costs nothing and guarantees correctness regardless
of entry point. Re-verified by running the four affected test files in
total isolation, `App.run()` never called, all passing.

**Real finding #3, caught only by manually re-running an example after
finding #2's own fix:** `tracing_subscriber::fmt`'s default `MakeWriter`
is `fn() -> io::Stdout` (confirmed via direct source read of
`SubscriberBuilder`'s own default type parameter) — surprising, and
wrong for this codebase's own real convention (`PyErr::print` always
wrote to `sys.stderr`; diagnostic output belongs on stderr, not mixed
into a program's real stdout). Fixed with `.with_writer(std::io::
stderr)` on the builder.

**Real finding #4, caught only by manually re-running an example after
finding #3's own fix:** switching from the free function `tracing_
subscriber::fmt::try_init()` (which specially wires `EnvFilter::
from_default_env()` for you, confirmed via direct source read) to the
builder chain (needed for `.with_writer`) silently dropped `RUST_LOG`
support entirely — the builder's own default filter is a flat
`LevelFilter::INFO` (`Subscriber::DEFAULT_MAX_LEVEL`, confirmed via
direct source read), completely ignoring the environment. Every
example started emitting real INFO events unconditionally. Fixed by
adding `.with_env_filter(EnvFilter::from_default_env())` explicitly.
New `tests/test_tracing.py` (2 tests): a real subscriber's filter is
captured once, at construction time, not re-read per event, so this
can only be tested with genuinely fresh subprocesses, not by toggling
`RUST_LOG` mid-pytest-process — both cases (no `RUST_LOG`, stays
silent; `RUST_LOG=info`, genuinely verbose, real `app_run` span
visible) verified via a real `subprocess.run` against a fresh
`python3` process, both passed on the first run after the fix.

Full `cargo test --workspace --release`/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo fmt --check` all clean. `maturin
develop --release` + full `pytest tests/` (153 passed, up from 151, 1
skipped) and all twenty-three examples confirmed clean, including a
manual `RUST_LOG=info` vs. default-quiet re-check after every real
finding above was fixed.

M16 — Structured Logging via `tracing` is now fully complete: both
phases (subscriber wiring, real call-site migration) closed §3/§9's
own stated-but-unbuilt policy — four real, non-obvious findings along
the way, none caught by unit tests alone, all caught by actually
running real code end to end.
