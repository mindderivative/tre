# Rust Crates

`tre` is a Cargo workspace of six crates with a strict, one-directional
dependency layering — the Python API (`engine-py`) is a thin boundary
over the rest, never the other way around. Full API docs for the Rust
side are generated with `rustdoc`, not this site. See
[Architecture](../architecture.md) for the design principles behind this
layering.

```bash
cargo doc --workspace --no-deps --open
```

## Crate layout

| Crate | Role |
| --- | --- |
| [`engine-core`](https://github.com/mindderivative/tre/tree/main/crates/engine-core) | Pure-Rust node tree, the `Animated<T>` animation core, and the generic `AppHandler`/`InputEvent`/`BindingResolver` interfaces the other crates build on. No `pyo3`, no `winit`, MD3-agnostic. |
| [`engine-md3`](https://github.com/mindderivative/tre/tree/main/crates/engine-md3) | Material Design 3 theming: dynamic color science (HCT/tonal palettes/scheme roles), curated icons, container-transform choreography. Depends on `engine-core`, never the reverse. |
| [`engine-render`](https://github.com/mindderivative/tre/tree/main/crates/engine-render) | Vello scene building and GPU rendering. Depends on `engine-core` (walks `Node`/`PaintProperties` for painting); no `winit`/`engine-platform` dependency. |
| [`engine-platform`](https://github.com/mindderivative/tre/tree/main/crates/engine-platform) | `winit` `EventLoop`/`ApplicationHandler` and the `accesskit_winit` adapter — the only crate depending on `winit`. |
| [`engine-spec`](https://github.com/mindderivative/tre/tree/main/crates/engine-spec) | YAML view/stylesheet parsing, reconciliation, and the `BindingResolver` trait. Depends on `engine-core` and `engine-md3`; no `pyo3`, no `winit`. |
| [`engine-py`](https://github.com/mindderivative/tre/tree/main/crates/engine-py) | PyO3 bindings — the only crate depending on `pyo3`, and the only stability contract for framework users. Everything under [Python API Reference](python/index.md) lives here. |

## Design principles

A few principles worth knowing before reading the Rust source:

- A `Node`'s `NodeKind` payload holds only state, never behavior — all
  dispatch and painting logic lives in `Tree`/the render crate, keyed on
  the kind.
- Every animatable property is an `Animated<T>` field, ticked centrally
  by one mechanism rather than each component rolling its own animation
  loop.
- `Tree` only knows *that* something happened (e.g. `DispatchOutcome::
  Activated`); giving that meaning (calling a registered handler) is
  deliberately left to the layer above (`engine-py`).

## Building the docs locally

```bash
cargo doc --workspace --no-deps --open
```

opens the generated rustdoc site in your browser, covering every public
item across all six crates with real, extracted doc comments.
