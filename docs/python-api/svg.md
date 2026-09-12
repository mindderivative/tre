# SVG

## `Svg`

Parses and tessellates a real SVG document. There is no plain constructor -- build one via the static `parse` method.

```python
svg = tre.Svg.parse(open("icon.svg", "rb").read(), tre.rgba8(30, 30, 30, 255))
registry.insert_svg(svg)
```

```
Svg.parse(
    data: bytes,
    fill_color: int,
    fill_rule: FillRule = FillRule.NonZero,
    x: float = 0.0, y: float = 0.0,
    opacity: float = 1.0,
    scale_x: float = 1.0, scale_y: float = 1.0, rotation: float = 0.0,
    max_bytes: int = 10_000_000,
    max_points: int = 1_000_000,
) -> Svg
```

Raises `ValueError` if `data` exceeds `max_bytes`, resolves to more than `max_points` path points, or fails to parse; `TreError` if the fill tessellator itself fails internally.

**Real, disclosed scope limit, inherited from the underlying SVG library:** only `<path>` fill geometry is extracted. Strokes, `<image>`/`<text>` nodes, gradients, and each path's own individual fill color are all discarded during parsing -- you supply **one** solid `fill_color` for the whole document. A multi-color SVG icon renders as a single flat color, not its original per-path colors.

Fields (read/write): `x`, `y`, `fill_color: int`, `opacity`, `scale_x`, `scale_y`, `rotation`. Read-only: `triangle_count: int` -- useful for deciding whether a document is cheap enough to render every frame versus something to cache.

**`FillRule`** variants: `NonZero` (default), `EvenOdd`.

## Vertex-morph animation

```python
Svg.morph(from_: Svg, to: Svg, t: float) -> Svg
```

Interpolates two SVGs' already-tessellated positions at parameter `t` (typically `0.0..=1.0`, though nothing clamps it -- overshoot is a legitimate easing technique). **`from_` and `to` must share the same triangulation** -- typically two frames of the same document at different states. Raises `ValueError` if their position counts differ ("topology mismatch").

```python
frame = tre.Svg.morph(keyframe_a, keyframe_b, tween.sample(elapsed))
```

## SMIL keyframe parsing

SVG's own `<animate>`/`<animateTransform>` elements are discarded by the parser `Svg.parse` uses -- `tre.parse_smil` is a separate pass that extracts them directly, for you to drive through `tre.Tween`/`tre.Timeline` yourself.

```python
tre.parse_smil(data: bytes) -> ParsedSmil
```

Raises `ValueError` if `data` isn't valid UTF-8 or well-formed XML.

`ParsedSmil` fields (read-only): `animates: list[SmilAnimate]`, `animate_translates: list[SmilAnimateTranslate]`.

- **`SmilAnimate`** (a single scalar `<animate>`): `attribute_name: str`, `keyframes: list[float]`, `duration_seconds: float`.
- **`SmilAnimateTranslate`** (`<animateTransform type="translate">`): `keyframes: list[tuple[float, float]]`, `duration_seconds: float`.

**Real, disclosed v1 scope**: a single scalar attribute per `<animate>` (`values` or `from`/`to`), no `begin`/`repeatCount`/`calcMode`; `type="scale"`/`"rotate"` and `<animateMotion>` aren't extracted at all. Driving the extracted keyframes frame-by-frame is your own job -- compose them with `tre.Tween`/`tre.Timeline` (see [Animation](animation.md)).