# LOG — M72: `Node.set_layout(flex_direction=...)`

- Real, genuinely necessary gap found immediately upon starting
  Tesserae-side Part 2 (the widget catalog) on top of M71's own new
  `window.theme` API: no imperative way existed anywhere to set a
  node's own flex main axis. Every `Window.add_*` factory returns a
  node whose real taffy default (`Display::DEFAULT = Display::Flex`,
  confirmed directly from the vendored `taffy` source) is already
  flex-capable, but `flex_direction` itself had no constructor kwarg
  on any factory and `Node.set_layout` (M59's own real widening) never
  included it either -- confirmed via direct re-read of its exact
  signature before assuming otherwise. No `add_container`/generic-
  container factory exists either. A Python-composed widget needing a
  real vertical stack (a dialog's headline/body/actions, a snackbar's
  text/action, any list) had no way to ask for one at all.

## What shipped

1. `Node.set_layout` widened with `flex_direction: Option<&str> =
   None`, accepting `"horizontal"`/`"vertical"` -- deliberately not
   taffy's own `"row"`/`"column"`, extending the identical real
   vocabulary fix M70 already made to the declarative
   `FlexDirectionSpec` layer, so an app author sees one consistent
   axis vocabulary across both the imperative and declarative
   surfaces instead of two competing ones.
2. New `parse_flex_direction` helper (`node.rs`), mirroring `parse_
   align_items`/`parse_justify_content`'s own exact "small vocabulary,
   `ValueError` on unrecognized" shape.
3. One pre-existing internal caller (`view.rs`'s own binding-
   application `temp_node.set_layout(...)` call, M59's own scoped-
   `None` pattern) updated for the new argument count.
- Tests: 5 new pytest tests (`test_set_layout_flex_direction.py`) --
  both real values accepted, the old taffy `"row"` vocabulary
  correctly rejected (naming it), composes with `align_items`/
  `justify_content`/`gap`, and omitting it entirely is a true no-op.
  Same real, honest limitation `test_live_style.py` already states for
  `set_layout` generally (no Python-facing pixel-box/position readback
  exists) -- matches the established precedent rather than inventing a
  new testing standard.
- Verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean; `cargo test --workspace --release` every crate's own count
  unchanged; `maturin develop --release`; `pytest tests/` 855 passed,
  2 skipped, up from 850, +5; every file in `examples/` ran clean;
  `demo/showcase.py` all 5 phases, exit 0. A real, direct dispatched-
  click sanity check -- two children vertically stacked via `set_
  layout(flex_direction="vertical")`, both independently clickable via
  `Window.click(node)` -- also run manually before committing.

## Status

**M72 is complete, both phases.** The real, last remaining primitive
gap blocking Tesserae's own Python widget composition is closed.
Committed locally on the `0.3.1` branch, not `main`; push deferred
pending explicit user confirmation, per standing policy.

Next: reinstall into `tesserae/.venv`, then resume Tesserae-side Part
2 of the approved plan (the widget catalog), tracked in Tesserae's own
`BUILD_TRACKER.md`.
