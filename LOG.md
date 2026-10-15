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

## 0.5.6.2 (in progress)

**#175, bitmap colour emoji (2026-10-11).** #168 left a bitmap emoji (CBDT on
Linux, `sbix` on macOS) drawing nothing, with three ways forward listed. None
was needed. glifo has two bitmap paths: the uncached one gives `vello_gpu` an
`ImageSource::Pixmap`, which it panics on (still so on upstream `main`,
2026-10-09); the atlas one (`atlas_cache(true)`) gives it an `OpaqueId`, which
it supports. #168 had tried glifo's `png` feature only without the atlas. With
`png` on and the glyph cache on, the CBDT emoji drew in colour (511 coloured
pixels of 683) in the first try; with the cache off it still panicked.

The fix: `png` on, `skrifa` as a direct dependency (the version glifo already
pulls in), and `atlas_for` in `text.rs`: a glyph run whose font has bitmap
strikes always draws through the atlas, at all four `fill_glyphs` sites (labels,
both text-field paths, the terminal); every other font follows `set_glyph_cache`
as before, so outline and COLR text is pixel-identical. The text-field sites had
no atlas call at all, so with `png` on an emoji typed into a field would have
panicked, not drawn nothing: they are covered by the test.

`colour_emoji_bitmap.rs` was the "draws nothing" pin; it now asserts colour with
the cache off (the case that panicked), the same pixels with it on, colour at
2x, colour in a plain and a selected text field, and that Latin text is
unchanged by registering the bitmap font. On this machine's real Noto Color
Emoji the `system_fonts` ink went from `emoji 184/0` to `184/937`. `sbix` is the
same code path but is unchecked without a Mac.

**#176 and #177, text that fits its content (2026-10-14).** The two issues had
one cause. A text node has no size of its own (`docs/guide/text.md` said so):
`compute_layout` ran taffy with no measure function, so an unsized text node is a
leaf of zero content size. #177's centred label was 0 wide and painted one word to
a line; #176's text with `padding=12` was a 24 px box, and an effect shader's
texture is the node's box, so it cropped 1003 bright pixels to 126 (with an
explicit size the effect pass handled padding correctly: 1003 against 1003). With
no padding the box is 0 wide and the effect is skipped, which hid it.

The decision was opt-in, so no existing layout moves: `width="fit-content"` and
`height="fit-content"` on a text node. It has to be a new value: an unset size
already reads back as `"auto"`, so "auto means fit" could not be told from "never
set". `engine-core`: `Node.fit_content` (`FitContent { width, height }`),
`Tree::set_text_measure` and `set_fit_content`, and `run_taffy`, which runs taffy
with a measure function (text leaves carry their own `NodeId` as taffy's node
context). A fit width is the text on one line, narrowed to the offered space when
it wraps (min-content is the longest word); a fit height is the wrapped text's,
at the width the node got; both rounded up as #98 does. `engine-py` installs
`window.measure_text`'s shaper as the measure, parses the value (any other value
clears it) and lays the node out again when its text, font or wrapping changes.
Stretch still fills the box, as in CSS, so #177's `stretch` row is 440 wide, one
line; centre and flex-start are the text's 299.

Tests: `engine-core/tests/fit_content.rs` (a stand-in measure: the three
alignments, wrapping, no-wrap, the legacy zero, a text change) and
`tests/test_text_fit_content.py` (against `measure_text`, a passthrough effect over
padding 0, 12 and 30 equal to a direct draw). `shader_shine.py` and
`shader_panel.py` use it and now assert their layout, so CI would catch this.
Not done: #176's second note (a passthrough effect over a box with children
differs by about 0.4% of pixels at 300x140), which this does not explain.

**#178, insets, mixed-unit sizes and a layout event (2026-10-14), from Tesserae
0.5.0.8 (#261).** All three of what it asked, none deferred.

*Insets.* `left`, `top`, `right`, `bottom` and `inset` on a node (`x` and `y` are
`left` and `top`). A taffy-level probe showed the engine already did the work: an
absolute child with `left 8, right 8` and a `auto` width is 284 in a 300 parent
(x=8), margins shrink it (276), percentages work (`10%`/`10%` is 240), a width
that is not `auto` beats the second inset (`100%` stays 300), and the insets are
from the parent's edge, not its padding. A relative node with `right` is offset
left, as in CSS. Only the property names were missing.

*Mixed units.* `width`/`height` take `"calc(100% - 16)"`, `"100% - 16"`, `"33% +
10"`. taffy 0.14 has a `calc` feature but `TaffyTree::resolve_calc_value` is a
stub that returns 0.0, so a calc size would silently be 0; the tree resolves it
itself (`Node.calc_size`, `Tree::set_calc_size`): the side starts `auto`, and
after taffy lays out the parent `settle_calc_sizes` writes
`percent x base + px` into the style and lays out again, up to 4 times. The base
is taffy's own for a plain percentage, measured: the parent's content box for a
node in the flow (280 inside `padding=10`), its padding box for an absolute one
(300). A side still changing after 4 passes has no fixed point (a row as wide as a
child that is the row less 16): it goes back to `auto` with a warning and is
listed by `take_calc_conflicts`, so it cannot drift every frame as Tesserae
feared. Reads back as `"calc(100% - 16px)"`; a lone percentage or pixel count is
the plain size. Not for `min_*`/`max_*`; in a grid it is taken of the container.
engine-core gains a `tracing` edge (already in the lock through engine-render).

*Layout event.* `node.on("layout", ...)`: `Tree::watch_layout` and
`take_layout_changes` compare a watched node's last computed size with the last
reported, as `take_scroll_changes` does, so any cause is seen (a `set`, a parent
resize, a calc size resolving once at its final size, new text). A node waits
until it has a computed layout (taffy hands out a zero placeholder otherwise,
which reported a bogus 0x0 in the first test run). `event.width`/`height`,
`old_value` (`None` first) and `new_value`; fired after layout in `advance` and
the frame loop, not from the `layout_x` getter; does not bubble.

Tests: `engine-core/tests/calc_size.rs` (10), `layout_watch.rs` (6),
`tests/test_layout_insets.py` (29), `test_calc_size.py` (24),
`test_layout_event.py` (11).

**#179, anchored layout (2026-10-15), from Tesserae 0.5.0.8 (#262).** Option 3
of the scope: the layer and an ordinary node share one placement model.

*Findings.* Layers already did what an in-tree node needs: `place_layers` runs
after layout, reads the anchor's window position, writes the layer's `inset` and
lays out again. An absolute node does not change its anchor's layout, so there is
no cycle with it and the same pass can place an ordinary node; positions go
through `composed_transform` (scroll offsets, scale and rotation included), so the
arithmetic stays right under a scrolling or transformed ancestor, which was
Tesserae's worry about doing it from Python.

*Model.* `Placement` gains `TopStart`, `TopEnd`, `BottomStart`, `BottomEnd` and
`Center`, which name a *point* of the anchor and put the node's centre there; the
four sides are as before. `Placement::origin` is the one piece of placement
arithmetic, and an `AnchorBox` is the bounds of the anchor's laid-out box through
its transform. `offset` moves the attachment point **in the anchor's own
coordinates** (a scaled anchor scales it): the user's reading, which I had first
described as a shift of the result; for an untransformed anchor they agree. A
point is never flipped or shifted to fit; `OverlayMeta.pinned` (`flip=False`)
stops a side doing it too. The default is unchanged, so every existing layer test
passed untouched.

*Layers.* `show_layer(..., offset=(dx, dy), flip=True)` and the five new names.

*In-tree.* `Node.anchored` (`Anchored { anchor, placement, offset }`),
`Tree::set_anchor` (makes the node absolute; `None` puts it back in the flow),
`anchor_cycle` (an anchor that is the node or inside it is refused), and a
`settle_anchored` pass beside the calc settle in `compute_layout`: it runs every
call, not only after a taffy pass, because an anchor moves with a scroll or a
transform without any layout; at most 4 passes (a chain needs one a link); two
nodes anchored to each other have no fixed point, so they are put back in the flow
with a warning (`take_anchor_conflicts`). The window-space target is mapped into
the parent's space with the inverse of its composed transform, less the node's own
scroll shift. Python: `anchor`, `placement` and `offset` properties (a node, a
layer-placement name, a pair); `check_anchor_changes` validates a `set` and a
`create` alike (the first version only checked `set`, which a test caught).
Deliberate gap from the proposal: an in-tree node has no `flip`, since it has no
bounding region to flip within; layers cover menus.

Tests: `engine-core/tests/anchored.rs` (9), `tests/test_layer_points.py` (12),
`tests/test_anchored_layout.py` (25).

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

**#168, found (2026-10-10, 0.5.6.1).** The scope read the test's `off/on`
output backwards: with system fonts on, the emoji draws *nothing* (0 inked
pixels; 184 with them off is the missing-glyph box). The machine's Noto Color
Emoji is a CBDT bitmap font, and at the pinned Vello revision (`b408cd00`) a
bitmap glyph cannot be drawn on the GPU: glifo decodes bitmap glyphs only with
its `png` feature, and with it on `vello_gpu` panics ("pixmap image sources
are not supported by Vello GPU", `wgpu/mod.rs:705`), so the feature stays off
and the glyph is skipped. COLR emoji do draw in colour (new
`colour_emoji_colr.rs`, with a registered 5.6 KB subset, passes). A new
`colour_emoji_bitmap.rs` pins the bitmap limit (no colour, no panic) so it
cannot change silently. `system_fonts.rs` now checks emoji colour only where
the machine's font draws, and `docs/guide/text.md` no longer claims bitmap
emoji draw in colour (it said so; that was wrong) and says to register a COLR
font. macOS's sbix is a bitmap format too: expected blank, not checked. The
real fix is upstream (a bitmap-glyph path in `vello_gpu`) or our own drawing of
the strike as an image: not in 0.5.6.1.
