# Log: M30 Phase 1 Step 2 — Icon Button

`Button`'s own anatomy (Step 1) with a centered `Icon` child instead
of `Text` — the exact framing `BUILD_TRACKER.md`'s own M30 scope text
used before this step started.

## Anatomy and color reuse

A `Rect` container (corner radius `size / 2.0`) with one centered
`Icon` child, sized to a fixed real MD3 token (24dp) regardless of the
container's own `size` (MD3's own default touch target is 40dp).
Centered on both axes via plain `justify_content`/`align_items` flex —
no `TextAlign`-equivalent needed, since an `Icon`'s own box is already
exactly its glyph's bounds, unlike `Text` which needed real alignment
machinery to center within a box wider than its own content.

Reused `resolve_button_colors` verbatim rather than a second,
near-duplicate color table — but kept real MD3 naming fidelity at the
boundary: Icon Button's four real variants are Filled/Filled
Tonal/Outlined/**Standard**, not `Button`'s five (there's no "Elevated
Icon Button" in MD3's own vocabulary), and MD3 calls the transparent
variant "Standard" here, not "Text". `"standard"` is translated to
`resolve_button_colors`'s own `"text"` at the `add_icon_button` call
site, not aliased inside `resolve_button_colors` itself — so
`add_button`'s own error message still only ever lists names that are
real for `Button`.

## The hit-test fix, applied proactively this time

Step 1's own real, confirmed bug (a centered child silently eating
clicks meant for its container) is not `Text`-specific — it's a
property of `Tree::hit_test_at`'s no-bubbling recursion applied to any
composite interactive node with a same-shaped child. `Icon Button` is
the next real instance of that same shape, so `NodeKind::Icon(_) =>
false` was added to `hit_test_at` *before* writing
`test_icon_button.py`'s own click-dispatch test, not after it failed.
Confirmed first via grep that `demo/showcase.py`'s only `add_icon`
usage (its icon gallery) is purely decorative, never independently
clicked — the click-dispatch test passed on the first run.

## Verification

`cargo check --workspace --all-targets`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo fmt --check` — all clean. `cargo
test --workspace --release`: 39 binaries, all green. `maturin develop
--release` rebuilt. `pytest tests/`: 207 passed, 1 skipped (10 new in
`test_icon_button.py`, zero regressions). All 32 examples and the
showcase demo re-run clean. `mypy --strict` clean against
`examples/icon_button.py`.
