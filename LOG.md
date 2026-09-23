# LOG — M57: Fix `Window`'s `self.active` Bypass

- User pasted `BUILD_TRACKER.md`'s own 4 non-environmental "Known Gaps"
  bullets: "Scope the following Known Gaps" (this one, MD3 theming
  loose ends, layout API breadth, styling API breadth). Dispatched 4
  parallel Explore agents to investigate each gap's real fix shape
  before any design decisions were made. Asked via `AskUserQuestion`
  how much to take on: user chose "Everything, as several milestones
  back-to-back" -- 6 milestones total (M57-M62), this being the first.
- Investigation: `ActiveTree`/`SharedActiveTree` (`window.rs:254-261`);
  `show_view` swaps `self.active` only, never the plain `self.tree`/
  `root`/`handlers`/`context_menus` fields. **Real finding beyond the
  Known Gaps bullet's own text:** the identical bug exists at 2 more
  real call sites the bullet never named -- `copy_terminal_selection`
  and `route_control_char_to_terminal` (used by `press_ctrl`) -- both
  confirmed via direct source read, not assumed. `resize` is
  architecturally split: `self.width`/`height.set()` correctly stay
  window-level, but its own `Tree::dispatch` call needed to move.
  `View` has no equivalent bug at all.

## What shipped (all 5 phases)

1. `select_all`/`copy`/`copy_terminal_selection` switched to `self.
   active.borrow().tree.clone()`, mirroring `copy_to_system_clipboard`'s
   exact M56 pattern.
2. `press_key`/`type_text` clone `(tree, root, handlers, context_menus)`
   from `self.active.borrow()`, mirroring `click`'s pattern. `route_to_
   terminal`/`route_control_char_to_terminal` fixed too -- both already
   took `&PyWindow`, so no signature change was needed, just reading
   `window.active.borrow().tree` instead of `window.tree` internally.
   `window.terminals` stays a plain window-level read (terminal PTY
   sessions are owned by the `Window` itself, never swapped by `show_
   view`, the same real reason `theme`/`completions` stay outside the
   `active` bundle too). This also fixes `press_ctrl`, which calls
   `route_control_char_to_terminal`.
3. `cut` clones `(tree, handlers, context_menus)` from `active` and
   rebuilds its own local `NodeContext` from those clones, so the
   `Change` handler it fires targets the right tree too, not just the
   raw text mutation.
4. `resize`: `self.width`/`height.set()` stay window-level (correct,
   real `SharedSize`, unaffected by which View is active); its own
   `Tree::dispatch`/`run_dispatch_outcome` call now routes through
   `active`-cloned `tree`/`root`/`handlers`/`context_menus`.
5. New `tests/test_window_active_bypass.py` (4 tests), extending
   `test_view_in_window.py`'s own established `show_view`-switch
   pattern to every fixed method. **Real, honest finding, not glossed
   over:** confirmed via grep that `NodeKind::Terminal` has no
   declarative YAML representation anywhere in `engine-spec` -- a
   `Terminal` can never live inside a `View`'s own tree, so `route_to_
   terminal`/`route_control_char_to_terminal`'s own fix has no way to
   be proven through a `show_view` switch specifically. `tests/test_
   terminal.py`'s existing, unmodified coverage on a plain `Window`
   already gives full regression proof for the ordinary case (`self.
   active.tree` is the same `Rc` as `self.tree` at construction, so
   routing through `active` is provably a no-op change there). No
   misleading test was written to paper over that real limit.
   `resize`'s own test proves "does not raise + the active view's own
   node stays clickable afterward" -- the same honest "no layout-
   inspection API exposed to Python" limit `test_view_in_window.py`'s
   own `test_from_view_shares_the_same_live_size_cell_as_the_window`
   already names, not a fabricated pixel-precision proof.
- `BUILD_TRACKER.md`: full Milestone 57 section, Top Metrics row at
  100%, the closed gap moved from "Known gaps" to "Fixed gaps" (the
  other 3 stay, annotated with which milestone will close each).
  Tracker regenerated (13 milestones/45 phases/102 items/4 known
  gaps/21 fixed gaps), artifact republished.
- Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean (zero `engine-core` changes, confirmed by `cargo test -p
  engine-core` staying unchanged); `cargo test --workspace --release`
  unchanged across every crate; `maturin develop --release`; `pytest
  tests/` (788 passed, up from 784, +4, 2 skipped unchanged); all 88
  examples (zero failures); `demo/showcase.py` (all 5 phases, exit 0).

## Status

**M57 is complete, all 5 phases.** The real capability gap this
milestone closes -- 8 real `Window` methods silently acting on stale
state after a `show_view` switch -- is closed, with zero regression to
any pre-existing test or example. Two real call sites found beyond the
Known Gaps bullet's own original text, both fixed and tested (where
testable) or explicitly named as untestable through the current API
(where not, rather than faked). Committing locally now; push deferred
pending explicit user confirmation, per this session's own established,
unwavering convention. Next: M58 (MD3 theming catalog loose ends).
