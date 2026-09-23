# PLAN — M59: Layout API Breadth: Per-Side Padding/Margin + Flex/Align

*(Replaces the prior M58 plan in this file — M58 is complete, committed.
Third of six milestones from the approved M57-M62 plan; see
`/home/phil/.claude/plans/reflective-sleeping-falcon.md` for the full
roadmap.)*

## Goal
Same "scope the following Known Gaps" request as M57/M58. This gap:
per-side padding/margin stayed uniform-scalar-only; flex-grow/shrink/
basis and align-items/justify-content didn't exist anywhere in this
codebase.

## Real investigation
**Key finding: `engine-core` needs zero changes** -- `Tree::
set_layout_style` (`tree.rs:1289-1298`) already takes a raw `taffy::
Style` and is fully general. The gap was entirely at the Python/YAML-
facing API surface: `Node.set_layout` only patched 4 scalar fields;
`StyleSpec` had no `margin` at all and only uniform-scalar `padding`/
`gap`; no `flex_grow`/`flex_shrink`/`flex_basis`/`align_items`/
`justify_content` anywhere (confirmed via grep). No existing "scalar-
or-object" serde precedent in `spec.rs` for per-side padding/margin --
this milestone establishes the first one (`#[serde(untagged)]`).
`FlexDirectionSpec`'s existing unit-variant shape *is* a real precedent
for the new alignment enums, unlike the per-side spacing shape.

## Design (4 phases)
1. `engine-spec`: per-side padding/margin (`SpacingSpec`).
2. `engine-spec`: flex-grow/shrink/basis + align/justify.
3. `engine-py`: `Node.set_layout` widened.
4. Tests, docs, verification.

## Status

**Complete, all 4 phases.**

1: new `SpacingSpec` enum (`Uniform(f32) | PerSide{top,right,bottom,
left}`, `#[serde(untagged)]`) applied to widened `StyleSpec.padding`
and new `StyleSpec.margin`. `build.rs`'s new `spacing_to_rect<T:
FromLength>` helper builds the real per-side `taffy::Rect` for both
(`LengthPercentage`/`LengthPercentageAuto` respectively, generic over
the same helper since taffy's own `length()`/`auto()` are already
generic).

2: new `AlignItemsSpec`/`JustifyContentSpec` enums (7/9 variants, the
real common flexbox vocabulary, deliberately skipping taffy's own
niche `Self*`/`Safe*` overflow-position variants -- the same bounded-
subset precedent `FlexDirectionSpec` already set). New `StyleSpec`
fields `flex_grow`/`flex_shrink`/`flex_basis`. `build.rs` gained
`align_items`/`justify_content` resolver functions (exhaustive matches
to taffy's own real associated consts) and `flex_grow`/`flex_shrink`
fallbacks confirmed to match `Style::default()`'s own real values
(`0.0`/`1.0`) exactly, so un-set YAML behaves identically to before.

3: `Node.set_layout` widened with per-side padding/margin (each layers
on top of the uniform `padding=`/`margin=` when both given), `flex_
grow`/`flex_shrink`/`flex_basis`, and `align_items`/`justify_content`
(plain lowercase-snake-case strings, `ValueError` on unrecognized,
matching `press_key`'s own established convention -- no dedicated
Python-facing enum type). `view.rs`'s own `apply_binding_value` call
site updated for the widened signature (all new params `None`, since
none of the new fields are reachable from a `{{ }}` binding).

4: 5 new `engine-spec` unit tests (scalar vs. per-side padding/margin;
all new fields' real YAML string forms; an unknown `align_items`
keyword errors clearly). 18 new pytest tests in `tests/test_live_
style.py`, extending M48's own established "no pixel-box readback,
prove the FFI call succeeds" honest limit. `_core.pyi` widened.
`examples/live_style.py` extended: per-side `padding_top` applied to
`root` (the real flex container, layered on top of its existing
uniform `padding: 16` from YAML -- a real demonstration of the "per-
side wins for that one side" contract) plus a real `align_items`/
`justify_content` pair, cycled alongside the existing border/size demo.

**Explicitly out of scope, named not silent:** widening `Window.
add_rect`/other factories' own constructor kwargs with the same fields
-- `Node.set_layout` already gives every already-built node the
identical imperative path regardless of which factory created it.

Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
clean (zero `engine-core` changes); `cargo test --workspace --release`
(`engine-spec` 69, up from 65, +4; every other suite unchanged);
`maturin develop --release`; `pytest tests/` (816 passed, up from 794,
+22, 2 skipped unchanged); all 88 examples; `demo/showcase.py` all 5
phases, exit 0. `BUILD_TRACKER.md` updated (Top Metrics, full Milestone
59 section, the closed gap moved to "Fixed gaps"), tracker regenerated
(13 milestones/45 phases/105 items/2 known gaps/23 fixed gaps),
artifact republished. Committing locally now.

Next: M60 (styling API breadth I: border kwargs).
