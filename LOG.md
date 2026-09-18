# Log: M30 Phase 1 Step 4 — Segmented Button (closes Phase 1)

## The scoped precedent didn't transfer — checked, not assumed

`BUILD_TRACKER.md`'s own Step 4 scope text pointed at "this codebase's
own docking tab anatomy" as a precedent to check before inventing new
anatomy. Checked directly: `dock.rs`'s `set_active_tab` is purely
logical panel-visibility switching within a dock zone — no real
shared-border, divider, or per-corner-shape visual anatomy exists
anywhere in docking to reuse. This is exactly the kind of finding the
milestone's own scope note already allowed for ("each phase gets its
own real investigation when it starts"), not a failure of the earlier
scoping pass — genuine, independent anatomy design was the real next
step.

## The real engine gap: no per-corner radius control

MD3's real Segmented Button anatomy rounds a group's first segment
only on its own outer-left edge and the last segment only on its
outer-right edge — both square on the edge touching a neighbor. The
existing `PaintProperties.corner_radius: Animated<f64>` is a single
uniform scalar, unable to express this. Checked kurbo's own real API
before assuming new geometry was needed: `RoundedRect::new` already
accepts `impl Into<RoundedRectRadii>`, with a real `From<(f64,f64,f64,
f64)>` impl for independent per-corner radii (confirmed via direct
source read of the pinned `kurbo 0.13.1`). Added a purely-additive
`corner_radii_override: Option<[f64; 4]>` (true no-op default,
`paint_node` falls through to the existing uniform-scalar path when
`None`) rather than widening `corner_radius` itself, which would have
been a breaking change to `PaintProperties::new`'s ~60 existing call
sites. Proven with a real pixel-readback test
(`corner_radii_paint.rs`): a 30px top-left-only override genuinely
clears a pixel 2px from that corner while the other three corners
(radius 0.0 in the same override) stay filled right up to their own
edges.

## Real MD3 data, verified rather than assumed

Checked directly against Material Web's own real token source
(`tokens/versions/v0_192/_md-comp-outlined-segmented-button.scss`)
before writing any code: 40dp container height, 1px outline width (the
same token draws both the group's shared outer border and the internal
dividers between segments), `corner-full` shape, and — a real, easy-to-
get-wrong number — an 18dp checkmark, not the 24dp icon token every
other component in this catalog uses. Selected-state colors
(`secondary_container`/`on_secondary_container`) and unselected-state
color (`on_surface`) both confirmed from the same file. Segment
padding/icon-label gap were not found in this particular token file —
stated honestly as real, reasonable values, not re-presented as
independently verified the way the others were.

## Real, explicit design decision: no group-exclusivity here

`BUILD_TRACKER.md`'s own Phase 2 scope for the future `Radio Button`
already states the principle this component reuses verbatim:
group-exclusivity is application state, not engine-owned (Design
Principle 6). `add_segmented_button` paints only the real *initial*
selected state from its `selected` argument and returns every
segment's own real container `Node` — an app wires up live
re-toggling with the exact same already-generic primitives every
other component in this catalog uses (`set_on_click`, `Node.animate`),
demonstrated end to end in `examples/segmented_button.py` (clicking a
segment animates the previously-selected segment's background out and
the newly-selected one in).

## Verification

`cargo check --workspace --all-targets`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo fmt --check` — all clean. `cargo
test --workspace --release`: 40 binaries, all green (`corner_radii_
paint.rs` included). `maturin develop --release` rebuilt. `pytest
tests/`: 234 passed, 1 skipped (7 new in `test_segmented_button.py`,
zero regressions). All 34 examples and the showcase demo re-run clean.
`mypy --strict` clean against `examples/segmented_button.py`.

This closes M30 Phase 1 (Actions) entirely: `Button`, `Icon Button`,
`FAB`/`Extended FAB`, `Segmented Button`, all with paired `.pyi`
stubs.
