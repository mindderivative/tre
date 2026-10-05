# Performance baseline and feature inventory (0.5.3)

This is the first measured baseline of the engine, and an inventory of what it
does against what a UI rendering engine is expected to do. It is research: it
changes no engine code. Each finding that is worth acting on is its own issue
in the project's Backlog; this page is the evidence they point back to.

**Where it was measured.** One machine: an AMD Radeon 880M (an integrated GPU)
with 24 CPU threads, Linux on Wayland, the release build at 0.5.3, and
Mesa's `lavapipe` software renderer, the adapter CI uses, for comparison.
There was no Mac, no Windows PC and no low-end hardware, so "all desktop
hardware" is not yet measured; what follows is a floor of evidence, not a
verdict.

## How to rerun it

```bash
cargo test -p engine-render --test baseline --release -- --ignored --nocapture --test-threads=1
# the software renderer CI uses:
VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json \
  cargo test -p engine-render --test baseline --release -- --ignored --nocapture --test-threads=1
BASELINE_FILTER=text cargo test ...   # only scenarios whose name contains "text"
```

[`crates/engine-render/tests/baseline.rs`](https://github.com/mindderivative/tre/blob/main/crates/engine-render/tests/baseline.rs)
runs the app's own frame sequence (`WindowRenderer`: `prepare`, `draw`, then a
wait for the GPU) at 1920×1080 and times each stage of 60 frames after 8 warm-up
frames. Idle CPU, wake-ups and startup were measured on a real Python window
by sampling `/proc` over four seconds.

## Frame cost by stage

Milliseconds, median of 60 frames. `prepare` is the damage walk; `draw` is the
CPU work of building and encoding the scene; `gpu` is submit and wait.

| Scenario | Total | update | tick | layout | prepare | draw | gpu |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 576 nodes, nothing changes | 0.55 | 0 | 0.03 | 0.06 | 0.21 | 0 | 0.25 |
| 2,304 nodes, nothing changes | 1.61 | 0 | 0.16 | 0.21 | 0.91 | 0 | 0.30 |
| 9,216 nodes, nothing changes | 8.14 | 0 | 1.32 | 0.85 | 5.47 | 0 | 0.47 |
| one card changes, 576 nodes | 0.83 | 0 | 0.03 | 0.05 | 0.21 | 0.20 | 0.33 |
| one card changes, 2,304 nodes | 2.35 | 0 | 0.16 | 0.21 | 0.93 | 0.54 | 0.40 |
| one card changes, 9,216 nodes | 6.24 | 0 | 0.61 | 0.59 | 3.19 | 1.27 | 0.55 |
| every card animates, 2,304 | 8.34 | 0 | 0.14 | 0.15 | 0.65 | 6.20 | 1.15 |
| 1,200 text labels, one changes | 1.47 | 0 | 0.04 | 0.08 | 0.32 | 0.47 | 0.57 |
| scroll view, 3,000 rows, scrolling | 3.66 | 0 | 0.17 | 1.57 | 0.19 | 0.62 | 1.07 |
| 300 cards with blur-12 shadows | 0.56 | 0 | 0.01 | 0.02 | 0.08 | 0.13 | 0.32 |
| animated fill shader, 800×600 | 0.62 | 0 | 0 | 0 | 0 | 0.17 | 0.45 |
| animated effect over 60 children | 0.58 | 0 | 0 | 0 | 0.02 | 0.16 | 0.39 |
| 720p video, a new frame each tick | 1.89 | 0.32 | 0 | 0 | 0.69 | 0.12 | 0.76 |

On the software renderer (`lavapipe`) the CPU stages are the same and the `gpu`
column is where the time goes:

| Scenario | Total | gpu |
| --- | --- | --- |
| one card changes, 576 nodes | 2.08 | 1.65 |
| one card changes, 9,216 nodes | 8.07 | 1.57 |
| every card animates, 2,304 | **36.69** | **28.96** |
| scroll view, 3,000 rows | **15.39** | **12.36** |
| animated fill shader, 800×600 | 7.50 | 7.15 |
| animated effect over 60 children | 6.03 | 5.67 |
| 720p video | **10.32** | 8.98 |

What the numbers say:

- **Every scenario fits a 16.6 ms frame on the real GPU, most by a wide margin.**
  On the software renderer a full redraw of 2,304 cards (36.7 ms) and a scrolling
  list (15.4 ms) do not.
- **The tree walks scale with node count, not with what changed.** At 9,216
  nodes, `tick` costs 0.6 to 1.3 ms and the damage walk 3.2 to 5.5 ms *per
  frame*, even when one card changed. The walk costs about 0.35 to 0.6 µs
  per node; a change that repaints 1,000 pixels still pays for all 9,216.
- **Scrolling re-runs layout every frame.** 1.57 ms per frame for 3,000 rows,
  although a scroll offset moves pixels and nothing's size.
- **Building a scene costs about 2.7 µs per drawn node** (6.2 ms for 2,304
  cards), the dominant CPU cost of a full redraw.
- **The effects added in 0.5.1 are cheap on a GPU** (0.6 ms), and are
  GPU-bound on a software renderer (6 to 7 ms).

## Idle and power

Measured on a real window with 41 nodes, four seconds after a warm-up:

| State | CPU (one core) | Context switches/s |
| --- | --- | --- |
| window left alone | 0.0% | 0 |
| a box animating opacity (default) | **97.5%** | 192 to 565 |
| an animated shader (default) | **97.5%** | 270 |
| the same two, with vsync forced | **8.2%** | 173 to 194 |

An idle window costs nothing, which is the engine's best property. But **any
continuous animation makes it use a whole core.** The cause is the swapchain's
present mode: `surface.get_default_config` takes the first mode the driver lists,
and on this machine the list is `[Mailbox, Fifo, Immediate]`. `Mailbox` never
blocks, so the loop renders as fast as it can: 28,456 frames in an 8-second run
(about 3,500 frames a second) for a display that shows 60 or 144. Forcing
`AutoVsync` for the same window dropped it to 8.2%. (The experiment was a
throwaway edit, reverted; it is not in the code.) The code's own comment says "its
wait for the display is what paces the loop"; with `Mailbox` it does not.

Startup, from the start of a Python process to the first frame callback, was
130 to 165 ms; `import tre` itself is 2.5 ms.

## Memory

| | Bytes |
| --- | --- |
| `Node` | 3,048 |
| `PaintProperties` (inside it) | 1,640 |
| `NodeKind` (the largest variant) | 616 |
| `taffy::Style` (kept twice: in the node and in `taffy`) | 560 |
| measured per rect node | 3,700 to 4,300 |
| measured per text node | 3,100 |

50,000 plain boxes take about 210 MB. A box needs far less than its largest
variant or its full paint block (every colour, width and radius is its own
`Animated<T>`, which carries room for an animation in flight), and a larger
node also means fewer nodes per cache line in the walks above.

## Inventory: what the engine has, and what it lacks

Checked against the source, not recalled. "Lacks" means a search of the engine
crates found nothing.

**Has.** Nine node kinds (box, text, text input, image, path, canvas, scroll view,
virtual list, terminal); flexbox and CSS Grid; per-corner radii, layered shadows,
group opacity, 2D transforms, SVG path data with morphing and trimming;
cubic-bezier easing; partial redraw with damage tracking; text shaping, IME,
selection and clipboard; accessibility through AccessKit; layers (menus,
dialogs); docking; custom title bars; WGSL fill and effect shaders; GPU health
reporting; multi-window; a terminal emulator; a virtual list; an idle-free event
loop.

**Lacks, and matters most for "crisp, expected" rendering:**

| Gap | Evidence |
| --- | --- |
| **Automatic HiDPI scaling** | The window opens at a logical size, then layout runs in the physical pixels `winit` reports, with no conversion (a comment says so); `scale_factor` is only reported. On a 2× display every size the app wrote is half the size on screen unless it multiplies by `scale_factor` itself. Not testable on this 1× machine. |
| **Fonts beyond Latin and Arabic** | Only Roboto, Noto Sans Arabic and Hack Nerd Font Mono are bundled and system fonts are deliberately never loaded: no CJK, Hebrew, Indic, or colour emoji unless the app registers a font. |
| **Gradient fills** | Fills are one colour; no gradient paint on any node, though the renderer supports gradients. |
| **Blur and filters** | Blur exists only inside shadows; no `blur`, backdrop filter, blend modes or masks, although the renderer has a blur layer. |
| **Rich text in a text node** | A text node is one style; spans exist only for the text input. No underline or strikethrough on text nodes. Static text isn't selectable. |
| **A pixel snapshot from Python** | No way to read the rendered pixels of a window, so tests cannot assert on what is drawn. |
| **Touch and gestures** | No touch, pinch or multi-pointer events. |
| **Dropped files** | No `DroppedFile` or `HoveredFile` events. |
| **Reduced-motion and contrast preferences** | Not read. Dark/light is. |
| **Spring animation** | Easing is cubic-bezier only. |

**Lacks, smaller:** custom cursor images, window transparency, `position: sticky`,
momentum and overscroll on scrolling, wide-gamut and HDR output, subpixel (LCD)
text, SVG documents (path data only), and a frame-statistics or profiling
interface for apps.

## What this suggests, in order

Ranked by benefit, effort and risk; each is a Backlog issue.

1. **Pick vsync by default.** One line, about 12× less CPU for any animation.
2. **Automatic HiDPI scaling.** Without it the engine is not crisp on most modern
   laptops.
3. **Stop walking the whole tree per frame**: an active-animation list for
   `tick`, and cached or incremental damage fingerprints.
4. **Scroll without relayout**, and a cheaper redraw of scrolled content.
5. **Smaller nodes**: split rarely used paint state out of `PaintProperties`.
6. **A pixel snapshot API**, which also makes rendering regressions testable.
7. **Gradients, blur and blend modes**, the renderer already supports them.
8. **Fonts**: an opt-in system-font fallback and a documented way to ship
   CJK/emoji.
9. **Rich text and text decoration**, then touch/gestures and dropped files.
10. **Measure on a Mac, a Windows PC and low-end hardware** (held with #47, #48
    and #91).
