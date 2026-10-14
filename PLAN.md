# PLAN — after 0.5.6

*(Replaces the 0.5.0 plan, custom windowing (#28), which shipped in 0.5.0
and is recorded in `LOG.md`. From 0.5.0 every step is tracked in the GitHub
project Tesserae Rendering Engine; the old trackers are archived as
`BUILD_TRACKER_ARCHIVE_*.md`.)*

## Where we are

`main` is at 0.5.6 (released 2026-10-09, PR #167); CI is green. Nothing is
`Ready` or `In progress`. Per `CLAUDE.md`, work starts from a scoped `Backlog`
entry that the user moves to `Ready`.

## Backlog (not scoped)

Found by running `tools/verify.sh` on a fresh machine (2026-10-10), and since:

- **#176** — an effect shader on a node with padding draws nothing
  (`examples/shader_shine.py` renders blank).
- **#177** — a text node in a vertical flex container with `align_items`
  `center` or `flex_start` shrinks to its longest word and wraps.

Done on 2026-10-11 (PRs #174 and #173, merged to `main`): **#168** (colour
emoji: COLR draws, bitmap-only emoji fonts are a documented limit, and the
`system_fonts` test no longer assumes the machine's font draws) and **#169**
(the live minimum-size test fits Wayland tiling and X11). `tools/verify.sh
--cross` passed on KDE Plasma Wayland (CachyOS) for both, alone and merged.

Done on 2026-10-14 (PR #180, merged to `main`): **#175** (bitmap colour emoji:
a CBDT or `sbix` emoji draws in colour, through glifo's glyph atlas whatever
`set_glyph_cache` says; `sbix` is the same code path but unchecked without a
Mac). The 0.5.6.1 and 0.5.6.2 changes are on `main` and not yet released or
tagged.

Verification on hardware or other platforms (each needs a machine we do not
have here): #47 macOS and #48 Windows title bars, #91 WGSL on real GPUs, #118
the baseline on a Mac, a Windows PC and low-end hardware, #132 touch, #133
OS file drag and drop, #134 reduced-motion and contrast preferences.

Also open: **#158** — a burst of 12 `wp_fifo_v1` barriers within 50 ms when
vsync is restored after a resize.

## Small follow-ups

- **#171** — done: `python3-dev` is in the apt line in
  `docs/contributing.md` (the `engine-py` tests need it to link). In review.
- Next release (0.5.7 or 0.6): to be decided by the user from the Backlog.
