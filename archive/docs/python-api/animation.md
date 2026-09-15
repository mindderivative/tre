# Animation

A small, composable stack: a real hardware clock, a generic tweener, a spring integrator, and a timeline that applies sampled values back onto arbitrary Python objects.

## `Clock`

A thin wrapper over the engine's real, hardware-backed frame clock.

```python
clock = tre.Clock()
...
dt = clock.tick()       # seconds since the previous tick() call; 0.0 on the first call
total = clock.elapsed()  # total seconds since this Clock was constructed
```

`elapsed()` is independent of how many times (or how recently) `tick()` has been called.

## `Easing`

An enum of 13 easing curves: `Linear` (default), `EaseInQuad`, `EaseOutQuad`, `EaseInOutQuad`, `EaseInCubic`, `EaseOutCubic`, `EaseInOutCubic`, `EaseInQuart`, `EaseOutQuart`, `EaseInOutQuart`, `EaseInQuint`, `EaseOutQuint`, `EaseInOutQuint`.

## `Tween`

```python
Tween(from_, to, duration: float, easing: Easing = Easing.Linear)
tween.sample(elapsed: float) -> float | tuple[float, float]
```

`from_`/`to` must both be a plain number, or both a real `(x, y)` tuple -- `sample()` returns whichever shape you constructed it with. Raises `TypeError` if `from_`/`to` don't match this contract or mismatch each other's shape.

```python
tween = tre.Tween(0.0, 100.0, duration=0.5, easing=tre.Easing.EaseOutCubic)
x = tween.sample(clock.elapsed())
```

Only scalar and 2D-vector tweening are exposed today; 3D is a real, straightforward future addition, not a fundamental limitation.

## `Spring`

A real damped mass-spring-damper integrator -- a genuine second-order ODE that can overshoot and oscillate, unlike a plain exponential smoother.

```python
Spring(stiffness: float, damping: float, mass: float, initial_position: float = 0.0)
spring.update(target: float, dt: float) -> float  # advances by dt seconds toward target
```

Read-only properties: `position: float`, `velocity: float`.

## `Timeline`

Sequences concurrent tweens against real Python objects, applying each sampled value back via `setattr`.

```python
timeline = tre.Timeline()
timeline.animate(rect, "x", to=200.0, duration=0.4, easing=tre.Easing.EaseInOutCubic)
timeline.animate(rect, "opacity", to=0.0, duration=0.4)

while timeline.advance(clock.tick()):
    registry = tre.ShapeRegistry()
    registry.insert_rectangle(rect)
    renderer.render(registry)
```

| Method | Signature | Notes |
|---|---|---|
| `__init__` | `()` | |
| `animate` | `(target: Any, property: str, to: float, duration: float, easing: Easing = Easing.Linear)` | Raises `AttributeError` if `target` has no `property` attribute; `TypeError` if that attribute isn't a real number |
| `advance` | `(dt: float) -> bool` | Returns whether at least one scheduled animation hasn't finished yet. Raises whatever `setattr` raises (e.g. a read-only attribute) |
| `prune_finished` | `()` | Drops every already-finished scheduled animation, so a long-lived `Timeline` doesn't accumulate dead entries |

Read-only properties: `elapsed: float`, `active_count: int`.

`animate` works against **any** Python object's exposed attribute with zero per-shape dispatch code -- it doesn't need to know it's animating a `Rectangle` specifically.