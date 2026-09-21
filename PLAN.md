# PLAN — M51: Live Re-Theme for the Declarative Surface (`View.set_theme`)

## Goal
User: "Scope M51" -- following M50's own writeup renumbering this as the
next roadmap item once the theme backend (color + shape/elevation) was
complete across the whole MD3 catalog. Scoped via a formal plan
(`EnterPlanMode`/`ExitPlanMode`).

## Design (single phase)
Real investigation before designing confirmed this is a small,
well-bounded addition, not a new subsystem: `Reconciler` already
retains the parsed `WidgetSpec`/`id`->`NodeId` map for a `View`'s
lifetime (for hot-reload), and `patch_node` already does exactly
"recompute a node's `PaintProperties`/`layout_style` from its spec +
active theme layers, overwrite in place." `reconcile_node` only skips
`patch_node` when the spec is unchanged (a hot-reload fast path) --
live re-theme is the opposite case: spec unchanged, theme changed, so
`patch_node` should run unconditionally for every node instead.

1. `resolve_theme_layers` (`engine-py/src/view.rs`) -- extracted
   `View::new`'s own inline theme-resolution logic into a shared
   private helper, once `View.set_theme` needed the identical logic.
2. `Reconciler::retheme` (`engine-spec/src/reconcile.rs`) -- walks
   `self.spec` via a new `retheme_node` recursive helper, calling
   `patch_node` unconditionally for every node. `&self`, not `&mut
   self`. No changes to `patch_node`/`build_tree` themselves.
3. `View.set_theme(default_theme=None, custom_theme=None,
   theme_seed=None, dark=False)` -- calls `resolve_theme_layers` then
   `retheme`, then updates `self.default_theme`/`custom_theme`/`scheme`
   so a later `poll_reload()` keeps using the new theme. Each call is a
   complete, fresh theme selection, matching `Window.set_theme`'s own
   precedent -- omitting params resets to the shipped default, not
   "keep the previous call's theme."

## Explicitly out of scope (named, not silent)
- Live re-theme for `Window`'s imperative catalog -- would need a new
  per-node "how was this constructed" tracking mechanism, the "live
  token-linkage" concept flagged as highest-risk/highest-value since
  the earliest investigation in this session, long before M49 existed.
- Re-applying `{{ }}` bindings after `retheme()` -- `patch_node` only
  recomputes the static cascade, identical to any content-only
  `poll_reload` today. Verified by a dedicated test, not assumed safe.

## Status
Complete. 7 new pytest tests (`tests/test_theme.py`), 2 new Rust unit
tests (`reconcile.rs`, GIL-free). `python/tre/_core.pyi` updated
(`View.set_theme` stub). Extended `examples/theme_customization.py` +
new `theme_customization_retheme.yaml` fixture with a real live-retheme
call proving an already-built node's `corner_radius` changes in place
while stylesheet/inline overrides still win. Full chain green:
`cargo check`/`clippy -D warnings`/`fmt` clean, `cargo test --workspace
--release` (`engine-spec` 65, up from 63, +2; every other suite
unchanged), `maturin develop --release`, `pytest tests/` (709 passed,
up from 702, +7, 1 skipped unchanged), all 84 examples (one extended),
showcase demo. Tracker generator re-verified (51 milestones/150
phases/262 items/2 known gaps/19 fixed gaps, up from 50/149/258/2/19).
**M51 -- is now fully complete, single phase.**
