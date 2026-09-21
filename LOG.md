# LOG — M48: General Live Property Exposure (Layout Mutation + Border)

- User's governing instruction: "I want all properties usable and
  exposed for a users use. Customization is a key function of any GUI
  framework. Even changing the MD3 theme should be customizable by a
  user as the MD3 specification is just the default setting." Preceded
  by a direct factual question ("Do we have bindable properties for all
  renderable nodes for yaml and python? Like width, height, border
  thickness, colors, states like checked, etc…?"), answered with a
  real, cited audit before any scoping began.
- Entered Plan Mode. Investigation confirmed no live resize/reposition
  API existed anywhere (`Tree::set_layout_style` genuinely general but
  only ever reached through one narrow caller, `resize_terminal`), and
  `border_color`/`border_width` (real `PaintProperties` fields since
  M30 Phase 1) were unreachable from `animate()`, YAML bindings,
  `StyleSpec`, and every widget constructor.
- **Mid-plan redirect, not anticipated at first:** a follow-up user
  message described a 4-tier cascade (default theme < custom theme <
  widget-wide styles < inline) for MD3 customization. A dedicated
  Explore investigation found roughly half of this already exists --
  `crates/engine-spec/src/cascade.rs`'s `Stylesheet`/`StyleRule`/
  `resolve_style`, a real, tested, fully-wired per-field cascade
  (`baseline < kind < classes < id < inline`). A follow-up
  `AskUserQuestion` clarified theme YAML should carry both a token/
  palette section and per-kind default styles ("Both" of three options)
  -- scoped as M49/M50, deliberately deferred, sketched only at the
  mechanism level. The plan file was rewritten to reflect this before
  `ExitPlanMode`.
- Implementation (M48, approved plan's own concrete first piece):
  - `crates/engine-py/src/node.rs`: `Node.set_layout(width, height,
    padding, gap)` -- new, generalizing `resize_terminal`'s exact
    pattern; `animate()`/`get()` gained `border_color`/`border_width`
    arms mirroring `background`/`corner_radius`.
  - `crates/engine-spec/src/spec.rs`+`build.rs`+`cascade.rs`:
    `StyleSpec` gained `border_width`/`border_color`; `node_kind_and_
    paint` split into itself + a new `node_kind_and_base_paint`
    carrying every pre-existing match arm unchanged; `merge()` gained
    the matching two field-overlay arms so border cascades through the
    existing stylesheet mechanism too.
  - `crates/engine-py/src/view.rs`: `apply_binding_value`'s `Str`-as-
    color guard widened to `border_color`; new pre-`animate()` branch
    routes `width`/`height`/`padding`/`gap` bindings to `set_layout`
    (these can never reach `animate()`'s own dispatch, not `Animated<T>`
    fields).
  - `crates/engine-py/src/window_factory.rs`: `add_rect` gained
    optional `border_color`/`border_width` construction kwargs
    (confirmed via grep: the one genuinely generic imperative shape
    factory, no `add_container` exists at all).
- **Two real test-authoring mistakes caught by actually running the
  tests, not shipped:** a `node.get("border_width")` assertion
  immediately after `node.animate(..., duration_ms=0)` on a plain
  `Window`-created node failed -- `animate()`'s own documented contract
  only snaps on the next tick, which nothing drains without a running
  render loop; fixed to match every sibling `animate()` test's own
  "must not raise" convention, moving the real readback assertion to
  the `View`-binding-path test (which ticks eagerly). A layout-binding
  type-mismatch test expected `TypeError` but the real code (matching
  `checked`/`text`'s own sibling branches) raises `ValueError` -- fixed.
- `python/tre/_core.pyi` updated. New `tests/test_live_style.py` (19
  tests). New `examples/live_style.py`/`.yaml`.
- `BUILD_TRACKER.md`: new M48 milestone section, Top Metrics row,
  "Just closed" prepended, "Up next" pointed forward to M49. **A real
  parser-format mistake caught by re-running the generator, not
  shipped:** Step 1's own note had an unbalanced trailing paren (an
  outer wrapping paren left unclosed) -- the generator's own `ITEM_RE`
  correctly refused to parse it rather than silently truncating;
  matched to the file's own established "close the outer wrapper after
  the trailing period" convention (`...exit 0).)`), confirmed by
  re-running the generator until parse counts matched expectations
  exactly (+1 milestone/+1 phase/+5 items, known/fixed gap counts
  unchanged).
- Full chain green: `cargo check`/`clippy -D warnings`/`fmt` clean,
  `cargo test --workspace --release` (every pre-existing suite
  unchanged -- zero new Rust-level `#[test]`s, no new `engine-core`
  logic added this milestone), `maturin develop --release`, `pytest
  tests/` (648 passed, up from 629, +19, 1 skipped unchanged), all 83
  examples (+1), showcase demo. Tracker generator: 48 milestones/140
  phases/238 items/2 known gaps/19 fixed gaps. Artifact republished to
  the existing URL.

## Status

**M48 -- General Live Property Exposure -- is now fully complete,
single phase.** Closes the first, concrete piece of the user's broader
customization request. M49 (theme as a YAML cascade tier) and M50 (live
re-theme) remain sketched-but-unscoped in the approved plan, each
needing its own dedicated plan-mode pass before implementation begins.
Per the standing "push after a full milestone closes" convention, a
`git push` is now appropriate.
