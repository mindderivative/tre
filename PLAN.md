# PLAN — M62: Styling API Breadth III: Typography Theming

*(Replaces the prior M60 plan in this file — M60 is complete, committed.
Sixth and final milestone from the approved M57-M62 plan; see
`/home/phil/.claude/plans/reflective-sleeping-falcon.md` for the full
roadmap. This closes the entire "Scope the following Known Gaps"
mega-task.)*

## Goal
The third, largest, and most design-heavy of the 3 genuinely separate
pieces the "Scope the following Known Gaps" investigation found bundled
in one "styling API breadth" bullet (see M60/M61). No typography-scale
module existed anywhere in `engine-md3`; `TextSpec` was per-node
literals only.

## Real scope amendment (`AskUserQuestion`)
The approved plan's own Phase 1 described the new type-scale module as
"role -> family/weight/size/line-height." Investigation found `TextState`/
`TextSpec` had no `line_height` field at all, and no real line-height
concept existed anywhere in the render/layout pipeline (only an
unrelated, hardcoded per-line scroll-math ratio local to `CodeEditor`).
Asked the user whether to (a) ship family/weight/size only, (b) store
line-height as inert theme data, or (c) also add real line-height
plumbing to `TextState`/the render/layout path. **User chose (c)** --
M62 grew to include genuine new `engine-core`/`engine-render` capability,
not just theme data.

## Design (5 phases, widened from the plan's original 3)
1. Real `line_height` support in `TextState`/text layout.
2. New `engine_md3::typography` module (the real MD3 type scale).
3. `typography:` `ThemeSpec` YAML section.
4. `TextSpec` wiring (`role:` field + `add_text` imperative parity).
5. Tests, docs, verification.

## Status

**Complete, all 5 phases.**

1: `TextState` gained a real `line_height: Option<f32>` field (a
font-size-relative multiplier, `None` = the font's own natural metrics,
`parley::LineHeight::MetricsRelative(1.0)`'s own real default -- byte-
for-byte the same behavior as before this field existed). 44 struct-
literal call sites across `engine-core`/`engine-render`/`engine-py`/
`engine-spec` updated (42 mechanically via a small Python script
matching a uniform pattern, 2 by hand). Threaded into `engine-render::
text.rs`'s real `shaped_layout` (`Some(ratio)` pushes a real `parley::
StyleProperty::LineHeight`, `None` pushes nothing at all), added to the
shaping cache key. **Real capability proven, not just plumbed:** a new
Rust test shapes identical content at the default and at `Some(2.0)`,
reads back real `parley::Layout::lines()` geometry, and asserts the
per-line advance genuinely widens -- a real, non-obvious finding
surfaced by writing this test: a larger line-height also nudges the
*first* line's own top position (half-leading), corrected in the test
rather than assumed away. **Real design refinement:** rather than
widening all ~32 `window_factory.rs` text-creating factories with a
redundant raw kwarg (their own MD3 role is already implicit in which
factory it is), only `add_text` (imperative) and declarative `kind:
Text` gained the real literal capability -- the one genuinely free-form
text surface, the direct analog of `add_rect`.

2: New `engine_md3::typography` module -- a `TypeStyle` struct (family/
weight/size/line-height) + 15 real `const`s (all of MD3's published
type-scale roles) + `type_style_named(name) -> Option<TypeStyle>`,
mirroring `shape.rs`'s own exact structure. **Real values, verified
against a real published source, not recalled from memory:** fetched
directly from Flutter's own `typography.dart` (`_M3Typography.
englishLike`) after confirming `m3.material.io`'s own page is JS-
rendered with no literal numbers reachable and `material-web`'s SCSS
only references computed values.

3: New `ThemeSpec.typography: HashMap<String, TypographyOverride>`
field, mirroring `components:`'s own exact shape (per-field-optional
overrides, keyed by role name). Parse-time data only -- no resolver
consumes it yet, named directly as a real remaining gap, not silently
left dangling.

4: **Real design refinement from the plan's own wording:** rather than
reusing M61's exact `Literal | TokenRef` enum (built for one field
resolving to one number; a role supplies 4 fields at once), a new
`TextSpec.role: Option<String>` field was added, with `font_family`/
`font_weight`/`font_size` widened to `Option<...>` (role-derivable).
Resolved in a new `build.rs::resolve_text_style` helper (shared by
`Text`/`TextField`): `role` resolves via `engine_md3::type_style_named`
(a new `SpecError::UnknownTypographyRole` on a bad name); any literal
field, if also given, overrides just that field on top of the role's
own default; with neither role nor literal, `font_weight` still falls
back to `400.0` and `font_family`/`font_size` become a real, build-time
`SpecError::MissingField` (previously a serde-automatic check -- a real,
tested, inherent consequence of the widening). Imperative parity scoped
to `add_text` only (mirroring Phase 1's own `line_height` scope
discipline) -- gained `typography_role: Option<&str>`, widened
`font_family`/`font_weight`/`font_size` to `Option` (a real, necessary
change: pyo3 can't otherwise tell "omitted" from "passed the old
default"). **Explicitly out of scope, named not silent, mirroring
`shape.rs`'s own real M49 precedent exactly:** the other ~31 factories'
own already-correct per-component font constants are not migrated to
reference the new module; `ThemeSpec.typography` still has no consumer.

5: New `examples/typography.py` + `examples/typography_view.yaml` --
`headline_small` vs. `body_large` roles side by side; the identical
`body_large` content rendered twice, once at its own real line-height
and once with an explicit `line_height: 3.0` override (a real, visibly
wider gap between wrapped lines); a `label_large` role with a literal
`font_size` override, proving the cascade visually. Plus a real
imperative `add_text(typography_role=...)` call for parity.

Tests: 3 new `engine-md3::typography` unit tests; 4 new `engine-spec::
build.rs` unit tests (role resolves every field to the real constants;
a literal overrides just one field; unknown role errors clearly; no
role + no literal fields errors clearly) + 2 pre-existing tests fixed
for the new `Option`-typed `TextSpec` fields; 3 new `theme.rs` tests
(typography: parses; empty theme's typography: is empty; unknown field
errors); 1 new `engine-render` test (the real geometry-change proof);
5 new `tests/test_live_style.py` pytest tests (line_height ×2,
typography_role ×3). `python/tre/_core.pyi` updated for `add_text`'s
widened signature.

Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
clean; `cargo test --workspace --release` (`engine-core` 227 unchanged,
`engine-md3` 24 up from 21 +3, `engine-render` 29 up from 28 +1,
`engine-spec` 83 up from 74 +9, zero regressions across all 6 milestones
of this mega-task); `maturin develop --release`; `pytest tests/` (829
passed, up from 824, +5, 2 skipped unchanged); every file in `examples/`
ran clean including the 2 new ones; `demo/showcase.py` all 5 phases,
exit 0. `BUILD_TRACKER.md` updated (Top Metrics, full Milestone 62
section, the "styling API breadth" Known Gaps bullet moved to Fixed
gaps with 2 narrower real gaps named in its place, Just-closed/Up-next
refreshed to reflect the whole M57-M62 roadmap closing), tracker
regenerated (13 milestones/46 phases/115 items/3 known gaps/25 fixed
gaps), artifact republished. Committing locally now.

Next: nothing currently scoped -- the full 6-milestone "Scope the
following Known Gaps" mega-task (M57-M62) is complete. 6 unpushed local
commits from this mega-task alone (plus 6 more from earlier in this
session) await a fresh, explicit user push confirmation, per this
session's own unwavering standing policy.
