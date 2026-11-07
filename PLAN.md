# PLAN — after 0.6.4

*(Replaces the 0.5.0 plan, custom windowing (#28), which shipped in 0.5.0
and is recorded in `LOG.md`. From 0.5.0 every step is tracked in the GitHub
project Tesserae Rendering Engine; the old trackers are archived as
`BUILD_TRACKER_ARCHIVE_*.md`.)*

## Where we are

`main` is at 0.6.4 (2026-11-07: TRE-REQ-013, #269-#272: transform origin, concave corners, shader joins, structure morph, parametric paths). 0.6.3 (2026-11-06: PR #267, tag `v0.6.3`): `PopupWindow(geometry=)`
(#266, TRE-REQ-007). 0.6.2 (2026-11-06): #263 (a text node measures to its content) and #264 (a cross-window change is
drawn at once). 0.6.1 (2026-11-03): the loop no longer runs a self-queuing `call_soon`
callback once it is exiting (#260; macOS unverified). 0.6.0 (2026-11-03) renamed the package `tre` to `tess_re` and the variables to `TESS_RE_*`, no alias
(#257; `docs/migrating-0.6.md`). 0.5.10 (2026-11-03) was `Window.build` / `apply` / `Node.set_children` (#253), SMIL
`svg_time` (#245) and occlusion handling (#246). Open issues: #187 and #188 (Windows and macOS hardware checks), #189 (X11
drop position), #247 (timers wait while every Wayland window is hidden), #248 (KWin fullscreen frame rate). The other
projects (`tess_ui`, `tess_el`, `tess_dl`, `tess_shell`) migrate on their own side. Per `CLAUDE.md`, work starts from a
scoped `Backlog` entry that the user moves to `Ready`.

## Backlog (not scoped)

Nothing from the 2026-10-10 fresh-machine `tools/verify.sh` run or from Tesserae's
requests (#178, #179) is open; what is, is below.

Done on 2026-10-16 (PR #183, merged to `main`): **#179**, anchored layout, from
Tesserae (#262). Layers take `top_start`, `top_end`, `bottom_start`, `bottom_end`
and `center` (the node's centre on that point of the anchor, never flipped or
shifted), plus `offset=(dx, dy)` in the anchor's own coordinates and `flip=False`;
and an ordinary node takes `anchor`, `placement` and `offset`, placed against the
anchor in the same layout pass, through scrolling and transforms, and staying a
child of its own parent (clipped, ordered and bubbling as one). An in-tree node
has no `flip`. Tesserae has been told (its #261 and #262); the 0.5.7
work is on `main` and not yet released or tagged, so Tesserae's pinned tests
(`test_size_recipes.py`, `test_anchored_overlay.py`) change when it adopts the
release.

Done on 2026-10-15 (PR #182, merged to `main`): **#178**, from Tesserae (#261):
`left`, `top`, `right`, `bottom` and `inset` on a node; `width`/`height` as a
percentage plus pixels (`"calc(100% - 16)"`), resolved by the tree after layout
because taffy's own calc resolver is a stub; and a node `layout` event for a
watched node's size. Not for `min_*`/`max_*`, and in a grid a calc size is taken
of the container.

Done on 2026-10-15 (PR #181, merged to `main`): **#176** and **#177**, one cause:
a text node has no size of its own, so a centred label was 0 wide (wrapped one
word to a line) and an effect over padded text cropped it to the padding. The
fix is opt-in: `width="fit-content"` and `height="fit-content"` on a text node.
Still open from #176: a passthrough effect over a box with children differs from
a direct draw by about 0.4% of pixels at 300x140; not explained by this.

Done on 2026-10-11 (PRs #174 and #173, merged to `main`): **#168** (colour
emoji: COLR draws, bitmap-only emoji fonts are a documented limit, and the
`system_fonts` test no longer assumes the machine's font draws) and **#169**
(the live minimum-size test fits Wayland tiling and X11). `tools/verify.sh
--cross` passed on KDE Plasma Wayland (CachyOS) for both, alone and merged.

Done on 2026-10-14 (PR #180, merged to `main`): **#175** (bitmap colour emoji:
a CBDT or `sbix` emoji draws in colour, through glifo's glyph atlas whatever
`set_glyph_cache` says; `sbix` is the same code path but unchecked without a
Mac). These shipped in 0.5.7.

Open verification on hardware we do not have here: **#187** (all Windows-only checks) and **#188** (all Mac-only
checks). They absorbed the earlier per-item issues #47, #48, #91, #118, #132, #133 and #134, which are closed (the
Linux halves were done and recorded in `LOG.md`; #158 was closed too, not a bug: swapchain frame depth).
Also open: **#189** (X11 file drop gets the drag's own position), **#247** (timers and `call_soon` wait while every
Wayland window is hidden) and **#248** (a KWin fullscreen window counts about 480 frames/s under vsync).

## Small follow-ups

- **#171** — done and closed: `python3-dev` is in the apt line in
  `docs/contributing.md` (the `engine-py` tests need it to link).
- The release after 0.6.4: to be decided by the user from the Backlog.
