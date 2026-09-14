# Changelog

All notable changes to TRE (Tesserae Render Engine) are documented here.
Format loosely follows [Keep a Changelog](https://keepachangelog.com/), but
entries are curated summaries — for the full phase-by-phase build history
and every real bug found along the way, see
[documentation/IMPLEMENTATION.md](documentation/IMPLEMENTATION.md) and
[documentation/REVIEW.md](documentation/REVIEW.md).

## [Unreleased]

### Security
- `HeadlessRenderer.create_custom_shader` now rejects GLSL source over
  1 MiB with `ValueError` before it reaches `shaderc`, and runs the compile
  with the GIL released (REVIEW.md finding #247).

### Performance
- Wrapped `Text` shapes no longer allocate one `Vec` per visual line on
  every frame; `RenderingCanvas` gains a slice-taking `draw_glyphs` that
  `draw_text` delegates to (finding #247).

### Added
- `tre.max_parallel_registries()` returns this machine's `render_parallel`
  concurrency cap, so callers can size a batch without catching a
  `ValueError` (finding #257).

### Changed
- The three framebuffer-fetch RHI trait methods are renamed from their
  Vulkan-extension names to vendor-neutral ones
  (`supports_framebuffer_fetch`, `framebuffer_fetch_blend_supported`,
  `insert_framebuffer_fetch_barrier`; finding #255).
- Every GitHub Action in CI is pinned to a commit SHA (finding #256).
- **Minimum supported Rust version is now 1.88** (was declared 1.75, which
  was never true; finding #248). The floor is enforced on every commit by
  a new `msrv` CI job that builds at exactly the declared version with the
  locked dependency set. Resolved dependency versions are unchanged.
- `RhiSwapchain::supports_local_read_input_attachment`,
  `RhiDevice::local_read_blend_supported`, and
  `RhiCommandBuffer::insert_blend_read_barrier` now have default bodies
  (`false`/`false`/no-op), so a backend without the capability compiles
  without overriding them (finding #247).
- Shared external dependencies are declared once in the root
  `[workspace.dependencies]` table and inherited by member crates
  (finding #247). Resolved versions are unchanged.

### Dependencies
- `rustc-hash` 2.1.1 to 2.1.3 (finding #249).
- `usvg` 0.45.1 to 0.48.1 (finding #250).
- `skrifa` 0.33.2 to 0.47.0 (finding #251).
- `accesskit` 0.17.1 to 0.25.0 and `accesskit_unix` 0.13.1 to 0.23.0; the
  `zbus` dev-dependency 4.4 to 5.19 with it (finding #252). `A11yBridge::
  connect` keeps its signature, but its `app_name` is no longer forwarded:
  `accesskit_unix` now derives the application name from the executable.
- `pyo3` 0.27.2 to 0.29.2 (finding #253). No Python-visible behavior change.

### Fixed
- Textures no longer risk a use-after-free at interpreter shutdown: a
  `Texture` (or a `ShapeRegistry` holding one) now keeps its device alive,
  so its GPU image is always freed before the device is destroyed
  regardless of Python GC order (finding #258).
- At the RHI level, `vkDestroyDevice`/`vkDestroyInstance` are now
  refcount-ordered via a shared `DeviceOwner`, so any `VulkanTexture` that
  outlives its `VulkanDevice` (over the C ABI or any binding) frees its
  image before the device is destroyed -- no teardown use-after-free
  (finding #259).
- `ARCHITECTURE.md` Section 6 annotated for finding #244's collapse of
  `begin_frame_with_timeout`/`begin_frame_with_viewport_crop` into
  `begin_frame_with_options`; three stale "only C ABI boundary is Vulkan"
  claims corrected (finding #247).

### Tests
- A `python` CI job now builds the real `tre_python` wheel with maturin and
  runs every `demo/*/demo.py` on every commit (finding #254). Before this,
  nothing in CI built or ran the extension.
- The SDF soft-shadow demo's spread assertion now expects the small,
  driver-dependent undershoot an 8-bit sRGB framebuffer imposes on a
  smoothstep tail, instead of a fixed 3-pixel band it already sat on
  (finding #254); the focus demo runs under a minimal window manager in CI.
- `tre-rhi-vulkan` gains its first ten unit tests, covering its GPU-free
  helpers and the validation-layer message predicates (finding #247).

## [0.1.0] - 2026-09-13

**First public release — early alpha.**

TRE is a low-overhead, hardware-accelerated 2D rendering engine. This
release supports **Linux with a Vulkan 1.2+ GPU only**. DirectX 12 and
Metal backends, and Windows/macOS windowing, are real, disclosed
placeholders — not yet implemented. See the README's "Platform support"
section for the current state and roadmap.

### Rendering (Vulkan)
- A complete Vulkan 1.2+ backend: headless and windowed rendering,
  multi-window and multi-threaded scenes.
- Vector shapes (rectangles, circles, polygons, paths) with fill, border,
  and non-uniform rounded corners.
- Gradients (linear and radial) and texture fills via a bindless texture
  system.
- Non-`Normal` blend modes (Multiply, Screen, Overlay, SoftLight,
  ColorDodge).
- Real shadows: both Dual-Kawase-blur-based and SDF-based.
- 64-bit sort-key batching and dynamic index stitching for single-digit
  draw calls per frame.
- A zero-allocation steady state during the render loop, mechanically
  enforced in CI.

### Text & Typography
- Dynamic MSDF typography with bidi/script shaping, line-breaking, and
  font fallback cascades.
- A full multi-line text editor with clipboard and IME support.

### SVG & Animation
- Native animated SVG rendering, including SMIL parsing and keyframe
  morphing.
- An easing/tweening library (including a real damped spring) and a
  `Timeline` animation sequencer.

### Platform & Accessibility
- Native window chrome, clipboard, file dialogs, and system tray
  (Linux only).
- Focus management and Tab-order keyboard navigation.
- A real Linux AT-SPI2 accessibility bridge with in-app widget focus.

### Language Bindings
- `tre-python`: a privileged, direct PyO3 binding exposing the full
  feature set above to Python — the project's primary UI-framework
  integration path.
- `tre-ffi`: a stable `#[repr(C)]`/`extern "C"` boundary for every other
  language — a real first slice (headless rendering, `Rectangle`/
  `Circle`/`Polygon`/`Path`), with gradients/textures/text/windowing as
  disclosed follow-up work.

### Testing & Performance
- A CI-gated criterion benchmark with a relative, CI-hardware-calibrated
  performance budget.
- A manually-triggered performance/optimization test suite
  (`tre-perf-suite`): geometric load-ramp stress tests and an
  interaction-driven window-resize stress test, with FPS/CPU/GPU/memory
  telemetry.
- Full CI coverage on Linux: formatting, clippy (`-D warnings`), build,
  test (including real-GPU and real-AT-SPI2-bus tests), and ~30 Vulkan
  validation-layer-gated example runs.

### Known limitations (tracked as roadmap, not regressions)
- DirectX 12 and Metal backends are unimplemented placeholders.
- Windows and macOS windowing are unimplemented placeholders.
- `tre-ffi` covers headless rendering and basic shapes only; gradients,
  textures, text, and windowing are not yet exposed through the C-ABI.
