# Log: M30 Phase 3 Step 3 — Card

## Real MD3 data, verified before writing any code

Checked Material Web's own real token source for all three variants
directly: identical `corner-medium` shape (12dp) across Elevated/
Filled/Outlined, differing only in container color/elevation/border —
the same real Elevated/Filled/Outlined pattern `Button` already
established (Phase 1), reused here at the container level rather than
re-derived from scratch. One real, easy-to-miss distinction caught by
checking: Outlined Card's border role is `outline_variant`, genuinely
different from `outline` (`Button`'s own Outlined variant's role) —
confirmed from the real token file, not assumed the same token name
applies everywhere "outlined" appears in MD3's vocabulary.

## A plain container, not a fixed anatomy

Unlike every other component this phase, Card's real content is
always arbitrary (a title, a body, an image, buttons — whatever the
app needs). Built as a plain `Rect`, populated via the already-generic
`Node.add_child`. Checked `Tree::try_add_child`'s real source directly
before relying on it for the example/test: re-parenting an
already-attached node (e.g. a label `add_text` already put under
`root`) correctly detaches from its old parent first rather than
duplicating it — confirmed real, tested behavior, not assumed safe.

## Verification

`cargo check --workspace --all-targets`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo fmt --check` — all clean. `cargo
test --workspace --release`: 43 binaries, all green, unchanged (a
plain `Rect` reuse needed no new engine-render capability). `maturin
develop --release` rebuilt. `pytest tests/`: 294 passed, 1 skipped (8
new in `test_card.py`, zero regressions). All 41 examples and the
showcase demo re-run clean. `mypy --strict` clean against
`examples/card.py`.
