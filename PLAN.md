# PLAN — M73: `instantiate(..., source=...)` — the Embedded-Component Macro-Expansion Gap

*(Replaces the prior M72 plan in this file — M72 is complete, committed.
Continues Tesserae-side follow-up work: "Start 3, then move to 2 and
then 1" — this is item 3.)*

## Goal

`View::new`'s own M71 doc comment already named the real, confirmed
need: Tesserae's `component:` macro-expansion layer (its own M15)
pre-processes a view's raw YAML text before `tre` ever sees it, using
`View(path=, source=)` to hand off the expanded result while `path`
still supplies real hot-reload/`include:` context. But that layer had
no way to reach an *embedded* component (`tesserae.instantiate()`,
e.g. one row of a real list) — `instantiate_component`
(`crates/engine-py/src/component.rs`), the one real shared
implementation both `View.instantiate` and `Component.instantiate`
call, always read `path` straight from disk with no override at all.
Confirmed by direct read before writing any code, not assumed.

## Status

**Complete, both phases.**

`instantiate_component` widened with an 8th parameter, `source:
Option<String>` — when given, used directly instead of reading `path`
from disk, mirroring `View::new`'s own exact M71 pattern. `View.
instantiate`/`Component.instantiate` (the nested-component call site)
both widened with `#[pyo3(signature = (path, into, source=None))]`,
forwarding straight through. `#[allow(clippy::too_many_arguments)]`
added (now 8 real params).

2 new Rust unit tests (`component.rs`, GIL-free) — built a real outer
`Tree`/`Reconciler` by hand (matching `View::new`'s own internal
construction recipe, since `View::new` itself is private to its own
module) to prove `source=` is genuinely used, not just accepted. 3 new
pytest tests (`test_component.py`) — the real Python binding wiring
end to end for both `view.instantiate(..., source=...)` and nested
`component.instantiate(..., source=...)`, plus a no-`source` regression
check.

Full verification chain: `cargo check`/`clippy -D warnings`/
`fmt --check` clean; `cargo test --workspace --release` every crate's
own count unchanged except `engine-py` +2; `maturin develop --release`;
`pytest tests/` 858 passed, 2 skipped, up from 855, +3;
`examples/component_list.py` (the one example that actually exercises
`instantiate`) ran clean; `demo/showcase.py` all 5 phases, exit 0.
`BUILD_TRACKER.md` updated, tracker artifact regenerated (24
milestones/69 phases/186 items/3 known gaps/25 fixed gaps) and
republished. Committing locally on the `0.3.1` branch now.

Next: reinstall into `tesserae/.venv`, then item 2 of the user's own
ordering — wiring Tesserae's `App.load()`/`tesserae.instantiate()` to
use `load_view`/macro-expansion by default.
