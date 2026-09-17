# Log: M16 Phase 1 — Real `tracing` Subscriber Wiring (§3, §9)

Corresponds to `BUILD_TRACKER.md` M16 Phase 1. `tracing`/`tracing-
subscriber` added to the workspace, a real subscriber wired at
`App::run`'s own real entry point, establishing the first real span
this codebase uses.

## Investigation before writing code

No `[workspace.dependencies]` section exists — every crate manages its
own dependencies, so both land in `engine-py`'s own `Cargo.toml`.
**Real finding:** `tracing = 0.1.44` is already a real transitive
dependency (confirmed via `cargo tree -p engine-py -i tracing`, pulled
in by `winit`/`accesskit_unix`/`zbus`) — depending on it directly adds
no new crate to the tree. `tracing-subscriber` is genuinely new.
`env-filter` is not in `tracing-subscriber`'s own default feature set
(confirmed via direct read) — enabled explicitly for a real
`RUST_LOG`-driven verbosity dial; `tracing_subscriber::fmt::try_init()`
already wires `EnvFilter::from_default_env()` automatically once it's
on. **Real design risk, found before writing:** a global subscriber
can only ever be installed once per process, and `App.run()` can
legitimately be called more than once in one process (confirmed by
`test_engine_py.py`'s own existing test) — `try_init` (returns a
`Result`), not `init` (panics on a second call), is the load-bearing
choice.

## What happened

`engine-py/Cargo.toml` gains `tracing = "0.1.44"` and `tracing-
subscriber = { version = "0.3.23", features = ["env-filter"] }`.
`App::run` calls `tracing_subscriber::fmt::try_init().ok()` at its own
real top, before even the "no windows" early return, so the
idempotency claim is exercised by the cheapest real call path. One
real span, `tracing::info_span!("app_run", windows = ...)`, entered
via RAII guard for the rest of a genuine run session.

New `tests/test_engine_py.py::test_app_run_tracing_subscriber_init_
does_not_panic_across_multiple_calls`: calls `App().run()` a second
time in the same pytest process (the first real call already happened
in the existing `test_app_requires_at_least_one_window`), proving
`try_init`'s own idempotency with real proof, not just reasoning about
the API — passed on the first run. Manually confirmed `RUST_LOG=info
python3 examples/checkbox.py` genuinely emits real, span-wrapped
`tracing` events, including from a transitive dependency (`wgpu_hal`'s
own cooperative-matrix log line) — a real bonus this phase gives for
free, previously silently discarded since no subscriber ever existed.
The default (no `RUST_LOG`) run stays exactly as quiet as before —
confirmed by re-running every example script.

Full `cargo test --workspace --release`/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo fmt --check` all clean — every
prior test passed unmodified. `maturin develop --release` + full
`pytest tests/` (151 passed, up from 150, 1 skipped) and all
twenty-three examples confirmed clean.
