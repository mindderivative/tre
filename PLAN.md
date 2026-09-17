# Plan: M16 Phase 1 — Real `tracing` Subscriber Wiring (§3, §9)

Corresponds to `BUILD_TRACKER.md` M16 Phase 1's own scoping: add
`tracing`/`tracing-subscriber` to the workspace, wire a real subscriber
once at `App::run`'s own real entry point, establishing the real
span/event vocabulary this codebase will use going forward.

## Investigation before writing code

- No `[workspace.dependencies]` section exists (confirmed via direct
  read of the root `Cargo.toml`) — every crate manages its own
  dependencies independently, so `tracing`/`tracing-subscriber` land
  in `engine-py`'s own `Cargo.toml`, the one crate that actually needs
  either.
- **Real finding:** `tracing = 0.1.44` is already a real transitive
  dependency of `engine-py` — confirmed via `cargo tree -p engine-py -i
  tracing`, pulled in by `winit`/`accesskit_unix`/`zbus`. Depending on
  it directly adds no new crate to the tree, only a name.
  `tracing-subscriber` is genuinely new (confirmed via the same
  `cargo tree` check, zero hits before this) — an application-level
  crate no library dependency pulls in on its own.
- `tracing-subscriber`'s own `env-filter` feature is not in its
  default feature set (confirmed via direct read of its `Cargo.toml`)
  — needed explicitly for a real `RUST_LOG`-driven verbosity dial,
  matching §3's own "correlate with frame timing" framing.
  `tracing_subscriber::fmt::try_init()` (confirmed real via direct
  source read) already wires `EnvFilter::from_default_env()`
  automatically once that feature is on — no manual builder needed.
- **Real design risk, found before writing, not after:** a global
  `tracing` subscriber can only ever be installed once per process.
  `test_engine_py.py`'s own `test_app_requires_at_least_one_window`
  (and any real app constructing more than one `App` in a process)
  can call `App.run()` more than once in the same process — `try_init`
  (which returns a `Result`, confirmed via direct source read), not
  `init` (which panics on a second call), is the load-bearing choice.

## Design

- `engine-py/Cargo.toml` gains `tracing = "0.1.44"` and `tracing-
  subscriber = { version = "0.3.23", features = ["env-filter"] }`.
- `App::run` calls `tracing_subscriber::fmt::try_init().ok()` at its
  own real top — before even the "no windows" early return, so the
  idempotency claim is exercised by the cheapest possible real call.
  `.ok()` discards the real, expected "already set" error on every
  call after the first, deliberately, not by accident.
- One real span, `tracing::info_span!("app_run", windows = ...)`,
  entered via RAII guard for the rest of a genuine run session (past
  the "no windows" check) — the first real span this codebase
  establishes, the vocabulary Phase 2's own call-site migration will
  build on.

## Verification plan

`cargo test --workspace --release`/`clippy -D warnings`/`fmt --check`;
`maturin develop --release`; new `tests/test_engine_py.py` test
proving `App.run()` called a second time in the same process does not
panic (real proof, not just reasoning about the API); manually confirm
`RUST_LOG=info python3 examples/checkbox.py` genuinely emits real,
span-wrapped `tracing` events (including from a transitive dependency,
`wgpu_hal`, a real bonus this phase gives for free) while the default
(no `RUST_LOG`) run stays exactly as quiet as before; every example
re-run; `LOG.md`/`BUILD_TRACKER.md`/tracker artifact/commit/memory.
