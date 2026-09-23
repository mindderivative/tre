# PLAN — M61: Styling API Breadth II: Token-Reference Substitution in `StyleSpec`

*(Replaces the prior M59 plan in this file — M59 is complete, committed.
Fifth of six milestones from the approved M57-M62 plan; see
`/home/phil/.claude/plans/reflective-sleeping-falcon.md` for the full
roadmap. M60, dispatched to a background agent in an isolated worktree,
is still running as of this writeup — tracked separately, not part of
this plan.)*

## Goal
The second of the 3 genuinely separate pieces the "Scope the following
Known Gaps" investigation found bundled in one "styling API breadth"
bullet (see M60/M62). `engine_md3::shape` already had real named shape/
elevation constants; nothing let a theme YAML author reference them by
name (`corner_radius: small`) instead of a plain literal number.

## Real investigation
`engine_md3::shape` (`shape.rs:25-44`) already has real named constants
(`SHAPE_NONE`/`EXTRA_SMALL`/`SMALL`/`MEDIUM`/`LARGE`/`EXTRA_LARGE`,
`ELEVATION_LEVEL_0..5`). The precedent to mirror is `StyleSpec.
background: Option<String>`, resolved by `resolve_color` (tries a theme
role first, falls back to a literal parse) -- but `corner_radius`/
`elevation` are `f32`, not `String`, so this needed a genuinely new
type: `Literal(f64) | TokenRef(String)`, applied to both `StyleSpec`
(declarative) and `ComponentOverride` (imperative theme path).

## Design (2 phases)
1. New `ShapeOrElevationSpec` enum + `engine_md3::shape::named`/
   `elevation_named` lookups.
2. Resolution wired into both crates' real consumers.

## Status

**Complete, both phases.**

1: `engine_md3::shape` gained `pub fn named`/`elevation_named` (exact-
match lookups, `None` for anything unrecognized, re-exported from the
crate root). New `ShapeOrElevationSpec` enum (`#[serde(untagged)]`, the
same real convention M59's own `SpacingSpec` established) with a
`.resolve(is_elevation) -> Option<f64>` method, applied to `StyleSpec.
corner_radius`/`elevation` and `engine_spec::theme::ComponentOverride.
corner_radius`/`elevation`. Widening away from `Copy` (`TokenRef` holds
an owned `String`) cascaded into `cascade.rs`'s `merge()` needing
`.clone_from(&...)` and every existing bare-float test literal across
`cascade.rs`/`theme.rs`/`window_factory.rs` needing `ShapeOrElevation
Spec::Literal(...)` wrapping -- all fixed and re-verified.

2: `engine-spec::build.rs` -- new `SpecError::UnknownShapeToken { id,
field, token }` variant + `resolve_shape_value(...)` helper wired into
`node_kind_and_paint`. `engine-py::window.rs` -- the real design fork
this phase turned on: widening `ThemeState::shape`/`elevation`'s own
return type to fallible would have rippled into dozens of existing
`theme.shape(...).unwrap_or(...)` call sites across `window_factory.
rs`'s 58-entry catalog, way beyond scope. Resolved instead by a new
`resolve_components(...)` function that resolves every `ComponentOverride`
token *eagerly*, once, inside `Window.set_theme` (a real Python
`ValueError` naming the offending component key/field on the first bad
token), converting into a new engine-py-local `ResolvedComponentOverride`
struct -- `ThemeState::shape`/`elevation`'s own public signature (and
every one of its call sites) stays completely unchanged.

Tests: 5 new `engine-spec::build.rs` unit tests (a real `corner_radius:
small`/`elevation: level_3` string resolves end-to-end through real YAML
parsing + node building to the exact same constant `engine_md3::named`/
`elevation_named` return; an unrecognized token on either field produces
a real `SpecError::UnknownShapeToken`; a plain literal still works
unchanged). 2 new `tests/test_theme.py` pytest tests (`Window.set_theme`'s
`components: {card: {corner_radius: small, elevation: level_2}}`
resolves to the real constants on a real `add_card` node; an unknown
component shape token raises a real Python `ValueError`). `_core.pyi`
checked, not changed -- `Window.set_theme`'s own Python signature is
unchanged, this milestone's whole surface is YAML-string-level.
`examples/theme_customization_custom_theme.yaml` gained a new
`components: {card: {corner_radius: small, elevation: level_2}}` entry;
`examples/theme_customization.py` extended with a real `add_card(...)`
call asserting the token resolved correctly.

Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
clean; `cargo test --workspace --release` (`engine-md3` 21 up from 19,
`engine-spec` 74 up from 69, every other suite unchanged); `maturin
develop --release`; `pytest tests/` (818 passed, up from 816, +2, 2
skipped unchanged); all examples; `demo/showcase.py` all 5 phases, exit
0. `BUILD_TRACKER.md` updated (Top Metrics, full Milestone 61 section,
the closed "token-reference substitution" clause moved out of "Known
gaps" into "Fixed gaps"), tracker regenerated (13 milestones/45 phases/
111 items/2 known gaps/24 fixed gaps), artifact republished. Committing
locally now.

Next: M62 (styling API breadth III: typography theming, largest of the
6). M60 (border kwargs, dispatched to a background agent) still running
independently -- to be reviewed and merged in when it completes.
