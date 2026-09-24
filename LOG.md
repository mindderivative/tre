# LOG — M73: `instantiate(..., source=...)` — the Embedded-Component Macro-Expansion Gap

- User-directed, continuing Tesserae-side follow-up work from M15:
  "Start 3, then move to 2 and then 1" — this is item 3. Real,
  confirmed gap, checked directly before writing any code: `View::
  new`'s own M71 doc comment had already named the real need
  (Tesserae's `component:` macro-expansion layer reaching an
  *embedded* component, not just a top-level `View`), but
  `instantiate_component` (`component.rs`) always read `path` straight
  from disk with no override at all.

## What shipped

1. `instantiate_component` (`crates/engine-py/src/component.rs`)
   widened with `source: Option<String>` -- when given, used directly
   instead of reading `path` from disk, mirroring `View::new`'s own
   exact M71 pattern; `path` still supplies the real base directory
   `include:` resolves against.
2. `View.instantiate` (`view.rs`) and `Component.instantiate`
   (`component.rs`, the nested-component call site) both widened with
   `#[pyo3(signature = (path, into, source=None))]`, forwarding
   straight through.
3. `#[allow(clippy::too_many_arguments)]` added to `instantiate_
   component` (now 8 real params) -- the one real clippy finding this
   milestone hit.
4. 2 new Rust unit tests (`component.rs`, GIL-free), mirroring
   `view.rs`'s own M71 `source_override_is_used_instead_of_reading_
   path_from_disk` test exactly. Real correction while writing them:
   `View::new` is private to its own module, unreachable from
   `component.rs`'s test module -- built a real outer `Tree`/
   `Reconciler` by hand instead, matching `View::new`'s own internal
   construction recipe (confirmed by reading it first, not guessed).
5. 3 new pytest tests (`test_component.py`) -- the real Python binding
   wiring end to end for both `view.instantiate(..., source=...)` and
   nested `component.instantiate(..., source=...)`, plus a no-`source`
   regression check.
- Verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean; `cargo test --workspace --release` every crate's own count
  unchanged except `engine-py` +2; `maturin develop --release`;
  `pytest tests/` 858 passed, 2 skipped, up from 855, +3;
  `examples/component_list.py` (the one example that actually
  exercises `instantiate`, confirmed via `grep -l instantiate
  examples/*.py`) ran clean; `demo/showcase.py` all 5 phases, exit 0.

## Status

**M73 is complete, both phases.** The real, last piece blocking
Tesserae's embedded-component `component:` support is closed.
Committed locally on the `0.3.1` branch, not `main`; push deferred
pending explicit user confirmation, per standing policy.

Next: reinstall into `tesserae/.venv`, then item 2 of the user's own
ordering -- wiring Tesserae's `App.load()`/`tesserae.instantiate()` to
use `load_view`/macro-expansion by default.
