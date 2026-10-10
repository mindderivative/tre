# LOG — Branch `0.5.x`: custom windowing to 0.5.6

## Status

**0.5.6 released (2026-10-09, PR #167, tag `v0.5.6`).** `main` is at 0.5.6 and
CI is green. No branch is in flight: nothing is scoped for 0.5.7 or 0.6 yet.
Tracking lives in the GitHub project Tesserae Rendering Engine (the old
`BUILD_TRACKER_ARCHIVE_*.md` files are the pre-GitHub trackers). Open
issues: #47, #48 (macOS and Windows title bars on hardware), #91 (WGSL on
real GPUs), #118 (baseline on Mac, Windows, low-end hardware), #132-#134
(touch, OS file drag and drop, reduced-motion and contrast on hardware),
#158 (a burst of `wp_fifo_v1` barriers after a resize), and the two found
by the 2026-10-10 verify run, below (#168, #169).

## 0.5.6.1 (in progress)

Milestone 0.5.6.1 (Ready: #91 Linux real-GPU check, #168 emoji colour,
#169 Wayland minimum size). Versioning slows down: a fourth number for small
releases.

**#169, step 1 (2026-10-10).** The fullscreen-ordering theory in the scope was
wrong: on COSMIC Wayland a plain window does not grow to a larger minimum
either, because winit's Wayland `request_inner_size` only resizes when the
compositor's last configure is not maximized, fullscreen or tiled, and
otherwise answers `Some(current size)`. `grow_to_minimum` took that `Some` for
"applied" and reported a resize, whose `Resized` ran it again: about 60 calls a
second while the window stayed below its minimum (0.05 s of CPU in 2 s,
against 0.00 with the fix). It now reports only when the size changed.

**#169, steps 2-4 (2026-10-10).** Why winit declined, decoded off the Wayland
socket: COSMIC's `xdg_toplevel.configure` for the window carries
`tiled_left, tiled_right, tiled_top, tiled_bottom` (then `activated`) even with
auto-tiling off, and winit never resizes a tiled window. COSMIC also did not
answer `set_min_size(400, 300)` with a larger configure, so on COSMIC a minimum
above the window's size is never reached. X11 (via XWayland) differs: a plain
window does grow to the minimum, and leaving fullscreen keeps the fullscreen
size (1920x1080) rather than restoring 300x200. So the live test's exact
`400x300` was wrong on two counts. It now asserts "at least the minimum", and
skips (a probe in a fresh interpreter) where the compositor holds the size.
The contract is written down in `docs/reference/window.md`. Passes on X11;
skips on COSMIC Wayland; KDE Wayland is untested (not available here).

## Releases

| Version | Date | What |
|---|---|---|
| 0.5.0 | 2026-09-30 | Custom windowing (#28): undecorated windows, window controls and state events, drag and resize regions, the window menu, macOS's overlay title bar, the Custom Title Bars guide |
| 0.5.0.1 | 2026-09-30 | A real click in a text field no longer panics (#51) |
| 0.5.1 | 2026-10-01 | WGSL shaders (fill, effect, sampling, animated; `tre.Shader`), `padding` on every kind, GPU health reporting, idle windows release the GIL (#92) |
| 0.5.2 | 2026-10-02 | Python 3.12 is the floor (wheels for 3.9-3.11 and PyPy dropped); docs reorganised, Rust and Python API pages |
| 0.5.3 | 2026-10-02 | A text node's explicit width is laid out rounded up (#98) |
| 0.5.4 | 2026-10-05 | The performance baseline and a round of speed work (damage walk, glyph cache, scroll reuse); transparent windows, blur, click-through; SVG nodes; gradients; spring easing; sticky positioning; custom cursors; selectable and linked text with accessibility runs |
| 0.5.5 | 2026-10-06 | A window being resized presents without a vsync barrier (#155); `configure_ms` frame stat |
| 0.5.6 | 2026-10-09 | Text input options and IME (#162, #165); animatable layout properties and `animation_end` (#161); `mask` (#164); accessibility states (#160); `Window.after`/`every` timers (#163); more `Window.set` keys, `center()`, and `add_window` while running (#159); `scroll_snap` and `snap_align` (#166); set-up from a fresh machine (`requirements-dev.txt`, `tools/verify.sh`, contributing guide), the project's Claude Code skills, and the intake routine write-up |

## 0.5.0 history (custom windowing, 2026-09-30)

Branch `0.5.0` off `main` at `a6f1853`. winit 0.30.13 was surveyed first:
decorations off everywhere; `drag_window` everywhere (right after a press);
`drag_resize_window` not on macOS; Windows undecorated shadow;
`show_window_menu`. M1 decided the design page (`d734290`); tracking then
moved to the GitHub project (milestone `0.5.0`, #31-#36, phases #37-#41).
M2 (#32, #37): undecorated windows, minimize/maximize/restore/close,
fullscreen, minimum size (grown to after every resize, for Wayland), icon,
platform, live-checked on KDE Wayland. M3 (#33, #38): drag regions, the
resize border, a double-click on the title bar, an opt-in window menu. M4
(#34, #39): macOS's overlay title bar. M5 (#35, #40): the example, docs and
upgrade page. M6 (#36, #41): the release. The step-by-step trail is in git
history for this file and in `BUILD_TRACKER_ARCHIVE_0.5.md`.

## Verify on a fresh machine (2026-10-10)

`tools/verify.sh` was run on a new Pop!_OS 24.04 box (Wayland, Rust 1.98.0,
Python 3.12) after installing rustup and the system packages. rustfmt,
clippy, maturin, the generated `docs/api/python.md`, stubtest, mypy
`--strict` and `mkdocs --strict` pass. Rust tests: 704 passed, 14 ignored,
1 failed. pytest: 1021 passed, 1 skipped, 1 failed. Both failures are in
paths CI skips (it has no emoji font and no display), and neither comes from
0.5.6:

- **#168:** `engine-render` `system_fonts` — with Noto Color Emoji installed
  the emoji render with ink but no colour (`system_fonts.rs:113`).
- **#169:** `test_live_fullscreen_and_a_minimum_larger_than_the_window` —
  the minimum comes out 300x200, expected 400x300, on Wayland.

Both are in the Backlog. One set-up gap (#171): the guide's apt line
(`docs/contributing.md`) lacked `python3-dev`, without which the `engine-py`
tests fail to link (`unable to find library -lpython3.12`). Fixed on branch
`docs-python3-dev-171`; CI's package list needs no change, since its runners
ship the Python headers.
