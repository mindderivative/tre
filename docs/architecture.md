# Architecture

*This page is a stub -- content coming as the documentation rewrite proceeds.*

Planned contents:

- The crate layout: `tre-engine`, `tre-platform`, `tre-rhi-vulkan`, `tre-python`, and supporting crates (`tre-math`, `tre-memory`, `tre-text`, `tre-svg`, `tre-tween`, `tre-animation`, `tre-a11y`)
- The Canvas / IR / RHI pipeline: how a `Canvas` recording becomes a `FlattenedFrame`, gets sorted and batched, and is executed against the RHI
- The multi-threaded recording model (`SubCanvas`, `FrameArena`, lock-free stitching)
- The input event pipeline (`InputEventQueue`, winit translation)
- Where the true source of truth lives (`documentation/DESIGN.md`, `documentation/ARCHITECTURE.md`, `documentation/TECHNICAL.md` in the repository) versus what this page summarizes for a reader