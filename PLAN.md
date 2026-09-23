# PLAN — M57: Fix `Window`'s `self.active` Bypass

*(Replaces the prior archiving-task plan in this file — that task is
complete, committed, and pushed. This is the first of six milestones
from the approved M57-M62 plan; see `/home/phil/.claude/plans/
reflective-sleeping-falcon.md` for the full roadmap.)*

## Goal
User pasted `BUILD_TRACKER.md`'s own 4 non-environmental "Known Gaps"
bullets and asked to scope them all. A dedicated Explore agent
investigated this specific gap's real fix shape before any code was
written: `Window`'s own "act on the currently focused node" methods
still read `self.tree`/`self.handlers`/`self.root` directly instead of
through `self.active`, so they don't follow a real `Window.show_view()`
switch, unlike `click`/`hover`/`scroll`/`focus` and the M56-fixed
`*_system_clipboard` trio.

## Real investigation
`ActiveTree`/`SharedActiveTree` (`window.rs:254-261`); `show_view`
(`window.rs:581-590`) swaps `self.active` only, never the plain
`self.tree`/`root`/`handlers`/`context_menus` fields. **Real finding
beyond the Known Gaps bullet's own text:** the same bug exists at 2
more real call sites never named in the bullet -- `copy_terminal_
selection` (`window_input.rs:651-654`, identical shape to `copy`) and
`route_control_char_to_terminal` (`window_input.rs:831-851`, used by
`press_ctrl`, identical shape to `route_to_terminal`). `resize` is
architecturally split: `self.width`/`height.set()` correctly stay
window-level (`SharedSize`, shared regardless of active view), but its
own `Tree::dispatch` call must route through `active`. `View` has no
equivalent bug at all -- no `active`/swap concept exists there.

## Design (5 phases)
1. Trivial reads: `select_all`/`copy`/`copy_terminal_selection`.
2. `press_key`/`type_text` + `route_to_terminal`/`route_control_char_
   to_terminal` (fixes `press_ctrl` too).
3. `cut` (also rebuilds its local `NodeContext` from `active`).
4. `resize` (splits window-level `width`/`height.set()` from the
   `active`-routed `Tree::dispatch` call).
5. Tests, docs, full verification.

## Status

**Complete, all 5 phases.**

1-4: `window_input.rs`'s `select_all`/`copy`/`copy_terminal_selection`/
`press_key`/`type_text`/`cut`/`resize`, plus the two terminal-routing
helper functions, all now clone `tree`/`root`/`handlers`/
`context_menus` out of `self.active.borrow()` before use -- mirroring
`copy_to_system_clipboard`'s exact M56 pattern. `theme`/`completions`
stay plain `self.*` reads throughout, per `ActiveTree`'s own doc
comment (deliberately outside the swapped bundle). No signature changes
needed for `route_to_terminal`/`route_control_char_to_terminal` -- both
already took `&PyWindow`, so they just read `window.active` instead of
`window.tree` internally.

5: New `tests/test_window_active_bypass.py`, 4 new tests, extending
`test_view_in_window.py`'s own established `show_view`-switch pattern:
`select_all`/`copy` on `view_b`'s own field after switching; `press_
key`/`type_text` writing into `view_b`'s own field; `cut` removing
`view_b`'s own real selection and firing its own `Change` handler
against the right tree; `resize` not raising and `view_b`'s own node
staying clickable afterward. **Real, honest finding, not glossed
over:** `route_to_terminal`/`route_control_char_to_terminal`'s own fix
can't be proven through a `show_view` switch at all -- confirmed via
grep that `NodeKind::Terminal` has no declarative YAML representation
anywhere in `engine-spec`, so a `Terminal` can never live inside a
`View`'s own tree. `tests/test_terminal.py`'s existing, unmodified
coverage on a plain `Window` already gives full regression proof for
the ordinary case instead (`self.active.tree` is the same `Rc` as
`self.tree` at construction, so routing through `active` is provably a
no-op change there). `resize`'s own test proves "does not raise + the
active view's own node stays clickable afterward" -- the same honest
limit `test_view_in_window.py`'s own `test_from_view_shares_the_same_
live_size_cell_as_the_window` already names (no layout-inspection API
is exposed to Python to prove more precisely).

Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
clean (zero `engine-core` changes); `cargo test --workspace --release`
unchanged; `maturin develop --release`; `pytest tests/` (788 passed, up
from 784, +4, 2 skipped unchanged); all 88 examples; `demo/showcase.py`
all 5 phases, exit 0. `BUILD_TRACKER.md` updated (Top Metrics, full
Milestone 57 section, the closed gap moved from "Known gaps" to "Fixed
gaps"), tracker regenerated (13 milestones/45 phases/102 items/4 known
gaps/21 fixed gaps), artifact republished. Committing locally now.

Next: M58 (MD3 theming catalog loose ends).
