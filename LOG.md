# Log: M30 Phase 3 Step 1 — Badge

Opens Phase 3 (Communication & Containment, Part 1) — the first
component after M30's first two phases (Actions, Selection) closed.

## Real MD3 data, verified before writing any code

Checked Material Web's own real token source (`_md-comp-badge.scss`)
directly: two real sizes, a 6dp dot (no label) and a 16dp labeled
pill, both `error`-filled, `corner-full`. The labeled variant's real
type role is Label Small (11sp/500 weight) — MD3's own smallest label
size, genuinely smaller than every other component's Label Large
(14sp) used in this catalog so far, confirmed from the real type
scale rather than assumed to be the same size.

## Deliberately no anchoring machinery

A real badge is always overlaid on the corner of some other real
component (an icon, an avatar) — but this doesn't need any new
positioning primitive. `add_badge` is a plain, caller-positioned node,
the same `x`/`y` contract `add_rect` already establishes; the app
picks the offset that overlays it correctly on whatever it's meant to
decorate, matching `examples/badge.py`'s own demonstration against a
real icon.

## Verification

`cargo check --workspace --all-targets`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo fmt --check` — all clean. `cargo
test --workspace --release`: 42 binaries, all green, unchanged (a pure
composition needed no new engine-render capability). `maturin develop
--release` rebuilt. `pytest tests/`: 278 passed, 1 skipped (5 new in
`test_badge.py`, zero regressions). All 39 examples and the showcase
demo re-run clean. `mypy --strict` clean against `examples/badge.py`.
