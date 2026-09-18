# Log: M30 Phase 3 Step 5 — Tooltip (closes Phase 3)

## Real MD3 data, verified before writing any code

Checked Material Web's own real token source (`_md-comp-plain-
tooltip.scss`) directly: `inverse_surface` fill, `inverse_on_surface`
text, `corner-extra-small` (4dp). A real, notable finding: the label
uses Body Small (12sp/400 weight) — a genuine *body* type role, not a
*label* role — the first component in this whole catalog to use one;
every interactive component so far (Button, Chip, Menu Item, ...) has
used a label role instead. Confirmed from MD3's own real type scale,
not assumed the same label convention applies to a tooltip's own
supporting text.

## Real, deliberate reuse, not new overlay machinery

A tooltip's own real panel is returned genuinely unattached anywhere
— the identical real contract `build_menu`'s own panel already
established in Step 4. Shown and hidden through the exact same
`Window.open_menu`/`close_menu` that step built (itself a thin wrapper
over `Tree::open_overlay`/`close_overlay`), triggered from the
anchor's own already-generic `Node.set_on_hover_enter`/`set_on_hover_
exit` rather than a dedicated `open_tooltip`/`close_tooltip` pair —
`overlay.rs`'s own module doc comment already named tooltips as a
real intended consumer of the identical one primitive dropdown/
context menus use, so this is the third real overlay consumer this
milestone has connected to it (context menus pre-existed; dropdown
menus, Step 4; tooltips, this step), not a fourth mechanism invented
from scratch.

`tests/test_tooltip.py`'s own hover-dispatch test proves this reuse
actually works end to end — a real synthetic `Window.hover` call
fires the registered `on_hover_enter` handler, which calls
`open_menu`, not just that the handlers can be registered without
raising.

## Verification

`cargo check --workspace --all-targets`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo fmt --check` — all clean. `cargo
test --workspace --release`: 43 binaries, all green, unchanged (pure
composition plus existing overlay primitives, no new engine-render
capability). `maturin develop --release` rebuilt. `pytest tests/`:
301 passed, 1 skipped (4 new in `test_tooltip.py`, zero regressions).
All 43 examples and the showcase demo re-run clean. `mypy --strict`
clean against `examples/tooltip.py`.

This closes M30 Phase 3 (Communication & Containment, Part 1)
entirely: `Badge`, `Progress Indicator` (Linear and Circular), `Card`,
`Divider`, `Tooltip`, all with paired `.pyi` stubs.
