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

Found by running `tools/verify.sh` on a fresh machine (2026-10-10):

- **#168** — `system_fonts` test fails where Noto Color Emoji is installed:
  emoji render with no colour. Decide whether the renderer's colour-bitmap
  path is missing or the test's precondition is too loose.
- **#169** — `test_live_fullscreen_and_a_minimum_larger_than_the_window`
  fails on Wayland (300x200, expected 400x300). Compare against KDE Wayland
  and X11 before deciding if it is the code or the test.

Verification on hardware or other platforms (each needs a machine we do not
have here): #47 macOS and #48 Windows title bars, #91 WGSL on real GPUs, #118
the baseline on a Mac, a Windows PC and low-end hardware, #132 touch, #133
OS file drag and drop, #134 reduced-motion and contrast preferences.

Also open: **#158** — a burst of 12 `wp_fifo_v1` barriers within 50 ms when
vsync is restored after a resize.

## Small follow-ups (not yet filed)

- `docs/contributing.md`: add `python3-dev` to the apt line (the `engine-py`
  tests need it to link); everything else a fresh Linux box needs is listed.
- Next release (0.5.7 or 0.6): to be decided by the user from the Backlog.
