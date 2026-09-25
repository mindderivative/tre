# Architecture

This page is a short orientation. The full design reference —
`ARCHITECTURE.md`, the living document 88 milestones have been built
against as of `v0.3.2` — lives in the repository root:

[**Read the full `ARCHITECTURE.md`**](https://github.com/mindderivative/tre/blob/main/ARCHITECTURE.md){ .md-button }

## System layers

```
engine-py  (PyO3 boundary — the only crate depending on pyo3)
   │
   ├── engine-spec     (view/stylesheet/theme schema, YAML/JSON parsing, reconciliation)
   ├── engine-platform  (winit event loop, accesskit_winit)
   ├── engine-render    (Vello scene building, GPU rendering)
   └── engine-md3       (MD3 dynamic color, icons, container-transform)
         │
         └── engine-core  (node tree, Animated<T>, generic interfaces)
```

`engine-core` sits at the bottom with zero dependencies on the others;
everything layers on top of it, and only `engine-py` ever depends on
`pyo3`. See [Rust Crates](api/rust.md) for what each crate owns.

## Where each topic lives in `ARCHITECTURE.md`

| Topic | Section |
| --- | --- |
| Vision & scope | §1 |
| Design principles | §2 |
| Technology stack | §3 |
| System architecture | §4 |
| Core data model (`Node`, `NodeKind`, `Animated<T>`) | §5 |
| Per-frame pipeline | §6 |
| Material Design 3 subsystem | §7 |
| Python/Rust FFI boundary | §8 |
| Threading & event loop model | §9 |
| Accessibility | §10 |
| Desktop shell & workspace (docking, splitters, lists, overlays) | §11 |
| Project structure | §12 |
| Environment setup & packaging | §13 |
| Suggested build order | §14 |
| Risk register | §15 |
| Declarative authoring: YAML views & stylesheets | §16 |

## Build history

See
[`BUILD_TRACKER.md`](https://github.com/mindderivative/tre/blob/main/BUILD_TRACKER.md)
for the complete, phase-by-phase build history — every milestone this
project has built against `ARCHITECTURE.md`, what was learned along the
way, and every real gap found and closed.

`tre` v2 is a from-scratch second iteration of an earlier project
(`TRE`, a Vulkan-based 2D rendering engine), archived in full under
[`archive/`](https://github.com/mindderivative/tre/tree/main/archive)
along with its own lessons-learned document that shaped several of this
project's design decisions.
