# LOG — M62: Styling API Breadth III: Typography Theming

- The third, largest, and most design-heavy of the 3 genuinely separate
  pieces the "Scope the following Known Gaps" investigation found
  bundled in one "styling API breadth" bullet (see M60/M61). No
  typography-scale module existed anywhere in `engine-md3`; `TextSpec`
  was per-node literals only (confirmed via grep before this milestone).
- **Real scope amendment, via `AskUserQuestion`:** the approved plan
  described the new type scale as "role -> family/weight/size/line-
  height," but `TextState`/`TextSpec` had no `line_height` field at
  all, and no real line-height concept existed anywhere in the render/
  layout pipeline (only an unrelated, hardcoded per-line scroll-math
  ratio local to `CodeEditor`, confirmed via grep). User chose to also
  add real line-height plumbing to `TextState`/the render/layout path
  this milestone (not just theme data) -- M62 grew from 3 planned
  phases to 5.

## What shipped (all 5 phases)

1. **Real `line_height` support in `TextState`/text layout.**
   `engine_core::TextState` gained `line_height: Option<f32>` -- a
   font-size-relative multiplier matching MD3's own type-scale
   convention AND `parley::LineHeight::FontSizeRelative`'s own real
   shape (verified against `parley` 0.11.1's own vendored source).
   `None` means "use the font's own natural metrics" (`parley::
   LineHeight::MetricsRelative(1.0)`, the library's own real default
   and this codebase's exact pre-M62 behavior), a real distinct value,
   not a stand-in for some concrete number. 44 struct-literal call
   sites across `engine-core`/`engine-render`/`engine-py`/`engine-spec`
   updated -- 42 mechanically via a small Python script matching the
   uniform `align: TextAlign::...,` -> `}),` pattern every real site
   shared, 2 by hand (`text_align.rs`'s own `align,` shorthand form).
   Threaded into `engine-render::text.rs`'s `shaped_layout` (the one
   real `parley` shaping path both `draw`/`draw_field` share) -- a real
   `StyleProperty::LineHeight` push on `Some`, nothing at all on `None`;
   added to `LayoutCacheKey` so a changed line-height correctly
   invalidates the shaping cache. `draw_field`'s own call site always
   passes `None` -- `TextFieldState` is a distinct struct outside this
   milestone's scope (MD3's type scale targets display text, not
   editable fields). **Real capability proven, not just plumbed:** a
   new Rust test shapes identical 2-line content at the default and at
   `Some(2.0)`, reads back real `parley::Layout::lines()` geometry, and
   asserts the per-line advance genuinely widens -- a real, non-obvious
   finding surfaced while writing it: a larger line-height also nudges
   the *first* line's own top position via half-leading, not only the
   gap between lines, corrected in the test rather than assumed away.
   **Real design refinement from the plan's original wording:** rather
   than widening all ~32 `window_factory.rs` text-creating factories
   with a redundant raw `line_height` kwarg (most build fixed-role
   labels whose MD3 role, and thus line-height, is already implicit in
   *which* factory it is, not a free per-call choice -- Phase 4 wires
   those through the real typography-role system instead, the identical
   "don't duplicate a capability a more general mechanism already
   covers" discipline M59 established for `Node.set_layout`), only
   `add_text` (imperative) and declarative `kind: Text` gained the real
   kwarg/field -- the one genuinely free-form, caller-controlled text
   surface in the whole catalog, the direct analog of `add_rect`.
2. **New `engine_md3::typography` module.** A `TypeStyle` struct
   (`font_family`/`font_weight`/`font_size`/`line_height`) + 15 real
   `const`s (all of MD3's published type-scale roles: display/headline/
   title/body/label × large/medium/small) + `type_style_named(name) ->
   Option<TypeStyle>`, mirroring `shape.rs`'s own exact structure and
   doc-comment style. **Real values, verified against a real published
   source, not recalled from memory:** fetched directly from Flutter's
   own `packages/flutter/lib/src/material/typography.dart`
   (`_M3Typography.englishLike`) after confirming `m3.material.io`'s
   own page is JS-rendered with no literal numbers reachable and
   `material-web`'s `_md-sys-typescale.scss` only references computed
   values, not literals -- Flutter's own already-shipped independent
   implementation was the real, citable source used instead. `family`
   is `"Roboto"` for every role, matching this codebase's own pre-
   existing, unconditional default at every real text-creating call
   site (no naming split introduced). 3 new unit tests: the real
   display/headline/title tier size ordering; `title_medium`/
   `body_large`'s real same-size-different-weight relationship (the one
   place two roles share a literal number); every real role name
   resolves, an unknown one doesn't.
3. **New `typography:` `ThemeSpec` YAML section.** `ThemeSpec.
   typography: HashMap<String, TypographyOverride>`, mirroring
   `components:`'s own exact shape -- `TypographyOverride` has all 4
   fields (`font_family`/`font_weight`/`font_size`/`line_height`)
   independently optional, so a theme can widen just one real attribute
   of a role while the rest still resolves from `engine_md3::type_
   style_named`'s own shipped default. Parse-time data only, per this
   codebase's own established "parsed at apply time" precedent -- no
   resolver consumes it yet in this milestone (named directly as a real
   remaining gap in `BUILD_TRACKER.md`'s own Known Gaps and in
   `examples/typography.py`'s own doc comment, not silently left
   dangling). 3 new unit tests.
4. **`TextSpec` wiring.** **Real design refinement from the plan's own
   original "reuse M61's `Literal | TokenRef`-shaped machinery" wording:**
   that exact enum resolves one single field to one number; a
   typography role supplies 4 real fields (family/weight/size/line-
   height) at once, not a fit for that shape. Instead, a new `TextSpec.
   role: Option<String>` field was added, with `font_family`/`font_
   weight`/`font_size` widened from required/defaulted to `Option<...>`
   (now genuinely role-derivable) -- the real, applicable *spirit* of
   M61's precedent carried over (a token name resolves against a fixed
   `engine_md3` vocabulary, a literal always still works, an unknown
   name is a clear error), not its literal enum shape. Resolved in a
   new `build.rs::resolve_text_style` helper, shared by both the `Text`
   and `TextField` arms of `node_kind_and_base_paint`: `role`, if
   given, resolves via `engine_md3::type_style_named` (a new
   `SpecError::UnknownTypographyRole` on an unrecognized name); any
   literal field, if *also* given, overrides just that one field on top
   of the role's own default; with neither, `font_weight` still falls
   back to `400.0` (byte-for-byte the pre-M62 behavior) and `font_
   family`/`font_size` become a real, build-time `SpecError::
   MissingField` (previously caught by serde's own automatic "required
   field" check instead -- a real, inherent, explicitly-tested
   consequence of the widening, not silently different behavior for a
   case that used to succeed). Imperative parity scoped to `add_text`
   only (the identical scope discipline Phase 1 already applied) --
   gained a `typography_role: Option<&str>` kwarg, widened `font_
   family`/`font_weight`/`font_size` from concretely-defaulted values
   to `Option`s (a real, necessary signature change: pyo3 can't
   otherwise distinguish "the caller omitted this" from "the caller
   passed exactly the old default"), resolving through the identical
   lookup and raising a real Python `ValueError` for an unrecognized
   role. **Explicitly out of scope, named not silent, mirroring
   `engine_md3::shape`'s own real M49 precedent exactly** ("`window_
   factory.rs`'s own existing constants are left exactly as they are...
   migrating them to reference these instead is real, separate,
   explicitly out-of-scope follow-up"): the other ~31 factories' own
   already-shipped, already-correct per-component font constants are
   not migrated to reference `engine_md3::typography` in this
   milestone. 4 new `build.rs` unit tests + 2 pre-existing `spec.rs`
   tests fixed for the new `Option`-typed fields (parse-time defaulting
   moved to build time, noted honestly in the updated test comments,
   not silently changed). 3 new pytest tests. `python/tre/_core.pyi`
   updated for `add_text`'s widened signature.
5. **Tests, docs, verification.** New `examples/typography.py` +
   `examples/typography_view.yaml`: `headline_small` vs. `body_large`
   roles side by side; the identical `body_large` content rendered
   twice, once at its own real line-height and once with an explicit
   `line_height: 3.0` override -- a real, visibly wider gap between
   wrapped lines; a `label_large` role with a literal `font_size`
   override, proving the per-field cascade visually too. Plus a real
   imperative `add_text(typography_role="label_large", ...)` call for
   parity. `ThemeSpec.typography`'s own current "parsed, not yet
   consumed" status named directly in the example's own doc comment.
- `BUILD_TRACKER.md`: full Milestone 62 section (all 5 phases), Top
  Metrics row at 100%, the bundled "styling API breadth" Known Gaps
  bullet moved to Fixed gaps with 2 narrower real gaps named in its
  place (`ThemeSpec.typography` has no consumer; ~31 factories not
  migrated), Just-closed/Up-next refreshed to reflect the entire
  M57-M62 roadmap now closing. Tracker regenerated (13 milestones/46
  phases/115 items/3 known gaps/25 fixed gaps), artifact republished.
- Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean; `cargo test --workspace --release` (`engine-core` 227
  unchanged, `engine-md3` 24 up from 21 +3, `engine-render` 29 up from
  28 +1, `engine-spec` 83 up from 74 +9, zero regressions across all 6
  milestones of this mega-task); `maturin develop --release`;
  `pytest tests/` (829 passed, up from 824, +5, 2 skipped unchanged);
  every file in `examples/` ran clean including the 2 new ones;
  `demo/showcase.py` (all 5 phases, exit 0).

## Status

**M62 is complete, all 5 phases.** This closes the entire 6-milestone
"Scope the following Known Gaps" mega-task (M57-M62), started when the
user pasted this file's own 4 non-environmental Known Gaps bullets and
asked to scope them, and chose "everything, as several milestones
back-to-back" when asked how much to take on. Two narrower, real gaps
remain from the original 4 bullets, both named directly rather than
silently dropped: `ThemeSpec.typography` has no resolver yet, and most
of `window_factory.rs`'s own text-creating factories still use
independent per-component font constants instead of the new centralized
module -- the identical, deliberate "data only, migration is separate"
scope boundary `engine_md3::shape`'s own M49 precedent already
established for corner-radius/elevation. Committing locally now; push
deferred pending explicit user confirmation -- 12 unpushed local commits
now await a fresh, explicit confirmation before any push. Next: nothing
currently scoped.
