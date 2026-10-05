# The smaller gaps: sized and ranked

0.5.4 research (#117). Eight things an app might expect that the engine does not
do. Each was checked against the engine's code and the pinned dependencies
(`winit` 0.30.13, the pinned `vello_gpu` and `glifo`), sized, and given a
recommendation. Sizes: **S** is days or less, **M** about a week, **L** weeks and
a new dependency or a design decision. Nothing here was built; the worthwhile
ones are filed as their own issues.

## Ranking

| # | Gap | Size | Value | Verdict |
| --- | --- | --- | --- | --- |
| 1 | [Momentum scrolling](#1-momentum-and-overscroll-scrolling) | S to M | High on touch, nice with a wheel | **Do**, #136 |
| 2 | [Window transparency and blur-behind](#2-window-transparency-and-blur-behind) | M | High for frameless windows | **Do**, #137 |
| 3 | [Spring animation](#3-spring-animation) | S (curve), M (true springs) | Medium | **Do**, #138 |
| 4 | [`position: sticky`](#4-sticky-positioning) | M | Medium | **Do**, #139 |
| 5 | [Custom cursor images](#5-custom-cursors) | M | Medium | **Do**, #140 |
| 6 | [SVG documents](#6-svg-documents) | L | High for icons and illustrations | **Decide**, #141 |
| 7 | [Subpixel (LCD) text](#7-subpixel-lcd-text) | L | Low | Decline |
| 8 | [Wide gamut and HDR output](#8-wide-gamut-and-hdr-output) | XL | Low today | Decline until the renderer supports it |

## 1. Momentum and overscroll scrolling

**Found.** A scroll view scrolls by wheel, keys, thumb drag and (0.5.4) touch pan,
all with no inertia: a flick stops when the finger lifts. `Gesture::velocity`
(touch pan, #113) already reports the release speed, `scroll_offset` already
animates with an easing curve, and since #105 an offset is a paint-time shift, so
animating it costs no layout.

**Design.** On a pan's end (and on a fast wheel burst), animate `scroll_offset`
toward `offset + v * tau` with an exponential-decay curve, stop on a touch or a
manual scroll, and clamp at the ends. Overscroll (a rubber band past the ends,
returning with a spring) is a separate, optional second step; it needs the offset
allowed outside `0..max` while a drag or fling is live.

**Risk.** Low: the pieces exist. The tuning (friction, velocity cutoff) wants a
touch screen, which isn't available here.

## 2. Window transparency and blur-behind

**Found.** `WindowAttributes` never sets `transparent`, and the surface is
configured with the driver's default alpha mode (opaque). `winit` 0.30 supports
`with_transparent(true)` on X11, Wayland, Windows and macOS, and `set_blur` only
on Wayland with KDE's blur protocol (and macOS), nothing on X11 or Windows.

**Design.** A window property `transparent` (set at open, since X11 can only set
it then) that asks `winit` for a transparent window and picks a pre- or
post-multiplied alpha mode from the surface's capabilities; the root's own fill
may then be translucent or clear, and a frameless window can have rounded
corners. `blur_behind` as a best-effort second property where the platform has it.
This is the missing half of the custom-windowing work (0.5.0), where a framework
draws its own title bar and corners.

**Risk.** Medium: premultiplied alpha through the whole renderer (the renderer
draws straight alpha into the target; the persistent-target copy and the
snapshot path need checking), and compositor behaviour differs per platform.

## 3. Spring animation

**Found.** `MotionCurve` is `Linear` or a cubic Bézier: a fixed duration, no
overshoot beyond what a Bézier with control points past 1 gives, and an animation
interrupted midway restarts from the current *value* with no memory of its
*velocity*, so a retargeted animation visibly stalls.

**Design.** Two steps. First, a `spring` easing (stiffness and damping, or
a `bounce` and `duration` pair as in Apple's and Material's APIs): an analytic
damped-spring curve over a computed settling time, which gives overshoot with the
existing fixed-duration machinery (**S**). Second, true springs: `Animated<T>`
keeps a velocity so a retarget continues the motion (**M**; needs a per-value
velocity and a step-integrated, rather than closed-form, tick).

**Risk.** Low for the first step.

## 4. Sticky positioning

**Found.** Nothing in layout or paint. Scroll offsets became a single paint-time
shift (`Tree::scroll_shift`, #105), which is exactly the place sticky needs to
live.

**Design.** A `sticky` property (an inset from the scroller's edge) on a child of
a scroll view: `scroll_shift` for that child clamps so it stops at the edge until
its own container scrolls away, by the CSS rules (relative to the nearest
scrolling ancestor, bounded by its parent's box). Hit testing, damage and
accessibility bounds already read `scroll_shift`, so they follow.

**Risk.** Medium: the damage tracker sees a sticky node move with every scroll
frame; headers are small, so the cost is small.

## 5. Custom cursors

**Found.** The `Cursor` enum names the OS's own cursors (default, pointer, text,
resize and so on). `winit` 0.30 has `CustomCursor::from_rgba` and
`ActiveEventLoop::create_custom_cursor` on X11, Wayland, Windows and macOS.

**Design.** A cursor property taking `(rgba, width, height, hotspot_x,
hotspot_y)` (the shape `Window.set(icon=...)` already takes for the window icon),
created on the event loop where `winit` allows it and cached by content. The
engine owns no decoding.

**Risk.** Low to medium: cursor creation needs the event loop, so it goes through
the same waker path as window creation.

## 6. SVG documents

**Found.** A `path` node takes SVG path data and a `view_box` (a single icon, such
as a Material Symbol, works today); there is no document: no groups, no
per-element fills or strokes, no gradients, no clipping, no text. `usvg`/`resvg`
are not in the lockfile; `vello_svg` targets `vello`, not the `vello_gpu` the
engine renders with.

**Options.** (a) Leave it to the framework: parse SVG in Python and build `path`
nodes (works for flat icons, and keeps the engine free of a parser). (b) An
engine-side `svg` node using `usvg` to flatten a document into paths, fills,
strokes and gradients and paint them as one retained scene (a new dependency
and a walk of its tree, and `usvg` does the hard parts: `use`, `style`, units,
text-to-paths). (c) Rasterize with `resvg` to an image node (a dependency, and
blurry when scaled; fine for fixed sizes).

**Recommendation.** Decide between (a) and (b) with the owner; (b) is the
useful one and is **L**. Filed as a decision issue.

## 7. Subpixel (LCD) text

**Found.** `glifo`, the glyph renderer, positions glyphs at subpixel offsets
(horizontal buckets) and caches them in an atlas, but renders grayscale
coverage only: no LCD filtering, and the blending is not subpixel-aware.

**Verdict: decline.** It would mean changing the glyph renderer upstream, and its
value is falling: HiDPI (#102) makes grayscale text sharp, and LCD text is
wrong on rotated, scaled or translucent content anyway. Revisit only if the
renderer grows it.

## 8. Wide gamut and HDR output

**Found.** Colours are 8-bit sRGB `(r, g, b, a)` throughout, the surface format is
an 8-bit one (#128), and the renderer's gradient interpolation is fixed to sRGB;
`vello_gpu` has no output colour management.

**Verdict: decline.** Wide gamut needs a colour-managed pipeline (float colours,
a `Rgba16Float` surface, and per-platform colour-space declarations), HDR needs
more, and neither exists in the renderer. Park it until `vello_gpu` supports it;
the colour API (`(r, g, b, a)` bytes) would need a floating-point sibling first.
