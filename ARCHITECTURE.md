# GUI Framework Architecture

**Python-facing declarative/imperative GUI framework, Rust-native rendering backend.**

Status: pre-implementation design document. This is the reference for engineering decisions made so far — update it as decisions change, don't let it drift from the actual code.

---

## 1. Vision & Scope

- Application authors write **Python**. They never touch Rust, WGPU, or Vello directly.
- The rendering, layout, animation, and accessibility engine is **Rust-native**, exposed through a thin, deliberately stable PyO3 boundary.
- Target: desktop apps (Windows/macOS/Linux) with a full **Material Design 3** visual language — dynamic color, elevation shadows, state layers/ripple, shape morphing, and MD3 motion (tweening/easing).
- **Explicitly out of scope: mobile (iOS/Android), web (WASM/browser), and bindings for any language other than Python.** `engine-py` (PyO3) is the only planned binding layer — there is no general C ABI crate (unlike TRE's `tre-ffi`, which existed for zero actual consumers) and none is planned. This is a deliberate scope narrowing versus TRE: every hour spent on binding-layer work goes toward the one binding that ships, not toward a hypothetical second consumer.
- Explicitly **not** built on Masonry. Same underlying libraries Masonry itself uses (`vello_hybrid`, `taffy`, `parley`, `accesskit`), but a purpose-built retained tree designed around a uniform, centrally-ticked animation system — see [ADR-001](#adr-001-hand-rolled-vs-masonry) for the reasoning.

### Locked Decisions

A running log of resolved architectural questions, kept in one canonical place rather than scattered across sections — a direct application of `archive/LESSONS_LEARNED.md` §6 ("separate the audit trail from the live worklist"). Add a row here each time a genuinely open question gets settled, in whichever section we're discussing.

| Decision | Choice | Why |
|---|---|---|
| Scope | Desktop only (Windows/macOS/Linux); no mobile, no web; Python is the only binding language | Deliberate narrowing vs. TRE — see the scope bullet above |
| Python packaging | Per-version wheels, no `abi3` | Full PyO3 API access; matches TRE's own precedent. Accepted trade-off: a larger OS × Python-version CI/packaging matrix than `abi3` would need |
| GPU backend selection | Pinned per OS, not `wgpu`'s automatic detection — `Backends::VULKAN` on Linux, `Backends::DX12` on Windows, `Backends::METAL` on macOS | Deterministic: the backend exercised in CI is the exact one every user gets on that OS, so a driver/validation issue found once stays found and fixed |
| Platform rollout order | Primary OS first (**Linux**, confirmed), Windows/macOS CI added later | Faster early iteration. Accepted trade-off is the same pattern that left TRE's Python bindings uncovered for three-quarters of that project — mitigated below with a concrete trigger instead of an open-ended "later" |
| Node data model | Common `PaintProperties` core + per-`NodeKind` payload structs, unified via a shared `AnimatedNodeState` trait (§5) | Avoids one ever-growing "god struct" as MD3's real component catalog (dozens of components, many with unique animatable state) gets built out |
| App entry point | Single blocking `app.run()`; no non-blocking/embeddable mode planned (§2) | Matches Design Principle 1 exactly and every major desktop GUI toolkit (Qt, GTK, Tkinter) |
| Linux display protocol | Both Wayland and X11 (`winit` `x11`+`wayland`+`wayland-dlopen` features), Wayland primary (§3) | Broadest real-world Linux desktop compatibility; matches TRE's own precedent |
| MD3 icon pipeline | MD3's own icon set embedded as `kurbo::BezPath` data at build time; `usvg`+`vello_svg` reserved for user-supplied custom SVG only (§3) | Sidesteps `vello_svg`'s documented gaps for the framework's own default, most-used icon set |
| System tray / native menus | Deferred past v1 — `tray-icon`/`muda` not included (§3) | Both require GTK on Linux with no portal alternative; v1 carries zero GTK dependency, avoiding the exact dependency class behind TRE's wheel-vendoring segfault |
| `engine-core` / `engine-md3` coupling | Keep `engine-core` MD3-agnostic — it defines generic types (`MotionCurve`, a generic interpolation trait); `engine-md3` depends on it, not the reverse (§4) | Provably one-directional dependency graph, `engine-core` stays testable in total isolation, even though no second design language is currently planned |
| Windowing crate | A dedicated `engine-platform` crate owns `winit`/`accesskit_winit`; `engine-render` depends only on `raw-window-handle` (§4) | Matches TRE's own `tre-platform` precedent; keeps `engine-render` renderable/testable with zero windowing dependency, and keeps accessibility's platform-adapter wiring out of the rendering crate |
| Animation completion callbacks | Queue-drain: `tick()` pushes finished `CompletionHandle`s to a plain `Vec`; `engine-py` drains it once per frame and resolves against its own `PyObject` map (§5) | Keeps `tick()` a pure, reentrancy-free state-mutation pass; `ActiveAnimation<T>` stays a plain data struct with no `Send`-closure coupling to one downstream consumer |
| `NodeKind` scope | Closed enum, one variant per MD3 component (§5) | Multi-design-language support is out of scope (§1) — a generic/extensible mechanism would be the same never-exercised abstraction LESSONS_LEARNED.md §1 warns against (TRE's stub RHI backends) |
| `NodeId` identity | Generational index (slot index + generation), not a plain integer (§5) | A stale `PyNode` handle held past node removal (§8) fails a checked comparison instead of silently addressing the wrong node — the generational-index answer to TRE's finding #259 (a dangling raw handle across a GC-controlled boundary) |
| Layout dirty-scoping | Rely on Taffy's own incremental cache; no hand-rolled `dirty_layout`/`dirty_paint` flag system. Named exception: `Splitter`/`DockZone` explicitly `mark_dirty()` (§6, §11.5) | `PaintProperties`/most `NodeKind` payloads share no fields with `taffy::Style`, so a paint-only animation structurally can't mark Taffy dirty — automatic, not manually classified, for every component except the two that are genuinely layout-affecting by nature |
| Frame budget | 16.6ms (60Hz) target, 8.3ms (120Hz) stretch goal, enforced by a CI benchmark added no later than build-order step 3 (§6, §14) | A stated-but-unenforced number is exactly TRE's MSRV mistake (LESSONS_LEARNED.md §5) restated in a new form |
| Ripple concurrency | Bounded list of concurrent ripples per node (`interaction: Option<InteractionState>`, §5, §7.3), not one scalar pair | Matches MD3's real overlapping-ripple behavior under rapid taps; reuses §5's completion-queue mechanism per-ripple |
| Dynamic color source | Third-party MCU-port crate, gated on passing Material Color Utilities' own published reference test vectors before pinning (§7.1) | HCT/tonal-palette/contrast math is a large, easy-to-get-subtly-wrong subsystem — reimplementing it is bigger and riskier than it looks, unlike shape morphing's one missing step |
| Live theme switching | Supported at runtime via `winit`'s `ThemeChanged` event, forwarded through the existing `AppHandler`/`InputEvent` inversion (§4, §7.1) | A rare, discrete, whole-tree-repaint event — matches modern desktop UX expectations at negligible design cost given the event-dispatch path already exists |
| Container transform | In scope for v1 (§7.6) — implemented as `engine-md3` choreographing existing `ActiveAnimation`s across two nodes; no navigation/router subsystem added | User's explicit choice, opposite of the recommended default (defer, no nav model exists); resolved by keeping the framework itself navigation-agnostic — it exposes the transition choreography, not "what screens exist" |
| `animate()` dispatch | Single generic, string-keyed method — no per-property typed methods (§8) | Typed methods would push toward one Python class per `NodeKind` variant to cover component-specific properties, multiplying the FFI surface by MD3's component count — the same growth cost §7 already chose to pay once, not again |
| Callback cyclic-GC support | Implemented now via one `PyWindow` (`#[pyclass(gc)]`) per OS window, each owning that window's callback maps, not deferred (§8, §11.1) | This project's own target — long-running apps with dynamically created/destroyed widgets — is exactly the profile where an uncollected reference cycle accumulates during normal operation |
| `Tree` ownership | `Rc<RefCell<Tree>>` + `#[pyclass(unsendable)]`, not `Arc<Mutex<>>` (§9) | Matches actual v1 scope (single main thread only); asyncio bridging is optional/future and its threading shape is undecided — revisit this first if that work is ever built |
| Callback exception policy | Caught, logged via `tracing`, non-fatal — the render loop always survives a bad callback (§9) | Matches every mainstream main-thread-owned GUI toolkit (Tkinter, Qt, GTK); a broken handler shouldn't take down a shipped app for its end user |
| `accesskit::TreeUpdate` builder | Lives in `engine-core` (new direct dependency on plain `accesskit`), not `engine-render` (§4, §10) | `accesskit` is small and OS-agnostic like `taffy`/`parley`; keeps a11y-semantic interpretation with the crate that already owns `AccessNodeData`'s meaning |
| Keyboard focus model | Minimal model specified now in `engine-core` (Tab-order focus + `Action::Default`/`Action::Focus`); component-specific arrow-key nav deferred per-component (§10) | Keyboard operability is WCAG 2.1's baseline requirement, not a nice-to-have — an explicit, written scope decision rather than a silent gap (LESSONS_LEARNED §7) |
| Multiple windows | One `Tree` per OS window, not a shared multi-root tree (§11.1) | Zero changes needed to the already-locked Node/`NodeId`/`Tree` model; cross-window node transfer is an accepted gap, not a blocker |
| App shell / navigation | Optional `AppShell` composition (named regions + one content region) within a single `Tree`; content swap, not a multi-screen router (§11.2) | Matches the single-page-app model requested; reverses §7.6's "no navigation model" only as far as this one persistent-shell case, not a general router |
| Menus, popups, dialogs | Custom-rendered overlay layer — one mechanism for menu bars, dropdowns, context menus, tooltips, and dialogs (§11.3) | Reinforces §3's zero-GTK decision rather than reopening it; reuses container-transform's existing "insert above root" trick (§7.6) |
| Docking | In scope for v1 — a fixed 5-zone (`Left`/`Right`/`Top`/`Bottom`/`Center`) model, not arbitrary nested splits (§11.4) | User's explicit choice, opposite the recommended default (defer); scoped to the fixed-zone case specifically to keep a large, MD3-precedent-free feature shippable |
| Virtualization | Framework-level `NodeKind::VirtualList`, not an app-level responsibility (§11.7) | Matches §1's own goal (Python needs everything without extra Rust work) for one of the most common, non-optional desktop patterns |
| MVVM data binding | A pure-Python `bind()` helper built on the existing `animate()` FFI call, not a new Rust binding subsystem (§8) | Smaller than it looks — reuses an already-locked mutation path; GC-safe for free since the binding closure never leaves Python's own object graph |

**Secondary-OS CI trigger (proposed, adjust as needed):** add minimal build + smoke-test CI for Windows and macOS no later than build-order step 6 (§14, wiring `accesskit`) — the first point where real platform-specific behavior (UIA vs. NSAccessibility vs. AT-SPI) becomes load-bearing, and a natural forcing function to confirm the deferred OSes still build before investing further past it.

---

## 2. Design Principles

1. **Rust owns every frame. Python owns intent.** Python never runs during layout, paint, or animation interpolation. It triggers state changes and registers callbacks; Rust computes and renders. Concretely: the framework's Python entry point is a single blocking call (`app.run()`) that hands control to `winit`'s event loop and does not return until the app exits — the only Python code that runs after that point is inside a registered callback. No non-blocking/embeddable mode is planned (§1 Locked Decisions).
2. **One node type, uniformly animatable.** No per-widget-type animation boilerplate. Every animatable property anywhere in the tree — universal (`PaintProperties`, common to every node) or component-specific (a Slider's thumb position, a Checkbox's check-progress) — is the same `Animated<T>` primitive, ticked by one central system. This is a *mechanism* guarantee, not a single-struct guarantee: component-specific state lives in per-`NodeKind` payloads (§5), not bolted onto one ever-growing shared struct (§1 Locked Decisions).
3. **The PyO3 boundary is the only stability contract.** Internal crates (`engine-core`, `engine-render`, `engine-md3`) can churn freely. `engine-py`'s public surface is what you version and document for framework users.
4. **No dependency leaks across the boundary.** Vello, kurbo, peniko, taffy, accesskit types never appear in Python-facing signatures. Python sees Python types.
5. **De-risk the unknowns before building on them.** Vello's shadow/blur support and shape morphing have no prior art in this exact combination — spike them standalone before writing MD3 components against them.

---

## 3. Technology Stack

| Layer | Crate | Role |
|---|---|---|
| Windowing / input / IME | `winit` (`x11` + `wayland` + `wayland-dlopen` features) | Event loop (owns the main thread), window/surface handles — both Linux display protocols supported, Wayland primary (§1 Locked Decisions) |
| GPU | `wgpu` | Device/surface management |
| 2D scene + rasterization | `vello_hybrid` | CPU preprocess, GPU raster of paths/gradients/images/text/shadows |
| 2D geometry | `kurbo` | `BezPath`, `Affine`, `Point`, `Rect` — Vello's native geometry vocabulary |
| Brush / color / blend | `peniko` | `Color`, `Brush`, `BlendMode`, `Fill` |
| Text shaping/layout | `parley` | Shaping, line-breaking, BiDi → glyph runs into a `Scene` |
| Layout | `taffy` | Flexbox/Grid; leaf sizing for text comes from Parley |
| Accessibility | `accesskit` + `accesskit_winit` | Cross-platform a11y tree (UIA / NSAccessibility / AT-SPI) |
| MD3 icon set | embedded `kurbo::BezPath` data, generated at build time | Material Symbols pre-converted from source SVG once, at build time (a small internal tool, itself likely using `usvg` — just never at runtime); sidesteps `vello_svg`'s gaps entirely for the framework's own default icons (§1 Locked Decisions) |
| Custom/user SVG import | `usvg` + `vello_svg` | SVG → `Scene`, for user-supplied assets only, never MD3's own icon set (gaps: text, clipping, masking, filters, patterns — verify per-asset; now scoped to this optional path, not the default experience) |
| Raster assets | `image` | PNG/JPEG/etc. decode |
| MD3 dynamic color | `material-colors` (or equivalent MCU port — compare current maintenance state before pinning) | HCT color space, tonal palettes, scheme generation from a seed color |
| Clipboard | `arboard` | No GTK dependency on Linux |
| File dialogs | `rfd` (`xdg-portal` feature, not the GTK backend) | Matches TRE's own precedent — avoids GTK on Linux entirely |
| Python bindings | `pyo3` | Rust ⇄ Python FFI |
| Build/packaging | `maturin` | Builds the extension module as an installable wheel |
| Async bridge (optional) | `pyo3-async-runtimes` | Only if the Python framework needs asyncio-native app code; keep conceptually separate from the winit/render loop |
| Errors | `thiserror` | Typed errors at crate boundaries |
| Logging | `tracing` | Structured logs, correlate with frame timing |

**Deferred past v1:** system tray icons and native application menus (`tray-icon`, `muda`) — both need GTK on Linux with no portal-based alternative, and v1 deliberately carries zero GTK dependency (§1 Locked Decisions). Revisit once the packaging story is proven end-to-end.

> **Review note (from the TRE archive):** `vello_hybrid`, `parley`, `kurbo`, and `peniko` are the same young, co-evolving Linebender project, not four independently-versioned dependencies — a `vello_hybrid` API bump is likely to force compatible bumps in the others at the same time (or vice versa). Pin this family as one set with one shared compatibility note, not four separate per-crate pins, or you'll hit a period where no combination of individually-latest versions actually compiles together.
>
> **Review note (from the TRE archive):** `peniko::BlendMode` is likely a ready-made, portable answer to the exact problem TRE spent real effort solving via a Vulkan extension (`VK_KHR_dynamic_rendering_local_read`) and then debugging real validation-layer false positives around for multiple review cycles. Worth confirming early that it covers every MD3 blend mode you need (Multiply/Screen/Overlay/etc.) — if it does, this is a place where the new stack is strictly better than the old one, not just different.

---

## 4. System Architecture

Every edge below means the same thing — **compile-time "depends on."** Runtime call/data direction (which is not always the same as the dependency arrow — see the `AppHandler` note below) is spelled out in prose, not folded into the diagram; conflating the two was exactly what made the previous version of this diagram contradict §5's own code (it drew `engine-core → engine-md3` while `engine-core`'s `ActiveAnimation<T>` held an `engine-md3` type, an actual dependency in the opposite direction).

```mermaid
graph TD
    subgraph Python["Python Layer — App Authors"]
        A[App code]
    end
    subgraph FFI["engine-py — the only stability contract"]
        C[PyO3 bindings]
    end
    subgraph Core["engine-core — pure Rust, no pyo3, no winit, MD3-agnostic"]
        D[Node Tree + Animated&lt;T&gt; + central tick]
        E[Taffy Layout]
        F[Parley Text Shaping]
        N[Generic InputEvent enum + AppHandler trait]
        O[accesskit TreeUpdate builder — §10]
    end
    subgraph MD3["engine-md3 — MD3 theming"]
        G[Color scheme / shadow &amp; ripple helpers / shape morph / motion-curve presets]
    end
    subgraph Render["engine-render — rendering only, no winit/engine-platform dependency"]
        H[Vello Scene Builder]
        I[vello_hybrid + wgpu]
    end
    subgraph Platform["engine-platform — windowing + accessibility adapter"]
        K[winit EventLoop / ApplicationHandler]
        L[accesskit_winit adapter]
    end
    C --> D
    C --> G
    C --> H
    C --> K
    G --> D
    K --> D
    L --> K
    D --> E
    D --> F
    D --> O
```

**Crate boundary rule:** `engine-core` has zero knowledge of Python, PyO3, `winit`, or `engine-md3` — it's testable and reusable standalone, and defines the generic types (`MotionCurve`, a generic interpolation trait, the `InputEvent` enum, the `AppHandler` trait) that other crates build on. It does directly depend on the plain `accesskit` crate (not `accesskit_winit`) — unlike `winit`/`pyo3`, `accesskit` itself is small, portable, OS-agnostic data types (`Node`, `TreeUpdate`, `Role`, `Action`), architecturally the same class of dependency as `taffy`/`parley`, both already here — so `Tree::build_access_update() -> accesskit::TreeUpdate` (§10) lives alongside the tree it's built from, not in a crate with no other reason to know accessibility semantics. `engine-py` is the only crate that imports `pyo3`. `engine-md3` depends on `engine-core` (§1 Locked Decisions: "keep `engine-core` MD3-agnostic") to supply *named presets* of those generic types (e.g. `engine_md3::motion::STANDARD: engine_core::MotionCurve`) — it never feeds data back into `engine-core`, so there's no cycle. `engine-render` depends only on `raw-window-handle` (a tiny interface crate `winit::Window` already implements), not on `engine-platform` or `winit` directly, so it stays renderable/testable against any window-handle-shaped value with zero windowing dependency; its own dev-only examples (§14 step 1) pull in `engine-platform` purely to get a real window to render into. `engine-platform` owns the actual `winit::EventLoop`/`ApplicationHandler` and the `accesskit_winit` adapter — a dedicated crate, not folded into `engine-render`, matching TRE's own `tre-platform` precedent (§1 Locked Decisions); since `engine-platform` already depends on `engine-core` (the diagram's `K --> D` edge), it pulls a fresh `accesskit::TreeUpdate` from `engine-core` each frame and hands it to its own `accesskit_winit` adapter — a plain value read through an existing dependency edge, not a new inversion.

**Runtime event dispatch is a dependency *inversion*, not a direct call.** `engine-core` can't call into `engine-py` (that would require depending on it, which would create the exact cycle the crate boundary rule forbids). Instead, `engine-core` defines a generic `AppHandler` trait; `engine-platform`'s `run()` entry point is generic over it, translating raw `winit` events into `engine-core`'s own `InputEvent` enum and calling the trait's methods — `engine-platform` never knows a concrete implementation exists. `engine-py` is the crate that actually *implements* `AppHandler` (it alone has GIL access and the Python callback map) and calls `engine_platform::run(my_handler)` from inside its own `app.run()`. So the compile-time dependency graph reads `engine-py → engine-platform`, while the runtime call direction for input events reads `engine-platform → (the AppHandler impl engine-py supplied)` — the same shape of solution as the `on_complete` callback-queue mechanism already noted in §5.

---

## 5. Core Data Model

Every visual property is wrapped in the same generic animation primitive — no per-widget-type animation code.

```rust
/// A generational index, not a plain integer. `index` names a slot that
/// gets reused after removal; `generation` increments every time a slot is
/// reused. A `PyNode` (§8) that outlives its node's removal fails a checked
/// generation comparison on the next call — `None`/a `PyResult` error —
/// instead of silently resolving to whatever now occupies that slot. This
/// is the generational-index answer to TRE's finding #259 (a cloned raw
/// device handle in `Drop` that dangled after real teardown, invisible on
/// the real GPU driver, only caught by the software rasterizer): the same
/// bug class, moved from GPU handles to tree nodes, made structurally
/// unreachable instead of merely fixed after the fact (§1 Locked Decisions).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId {
    index: u32,
    generation: u32,
}

pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub kind: NodeKind,               // carries any component-specific animatable state
    pub layout_style: taffy::Style,
    pub paint: PaintProperties,       // universal — every node has these
    pub access: AccessNodeData,       // feeds AccessKit TreeUpdate directly
    pub interaction: Option<InteractionState>, // ripple/state-layer, only on interactive nodes — see §7.3
}

pub struct PaintProperties {
    pub background: Animated<peniko::Color>,
    pub corner_radius: Animated<f64>,
    pub elevation: Animated<f64>,     // drives fill_blurred_rounded_rect
    pub opacity: Animated<f64>,
    pub transform: Animated<kurbo::Affine>,
    pub shape: Animated<ShapeKey>,    // morph target — see §7.4
}

/// Component-specific animatable state lives HERE, per kind, not bolted
/// onto `PaintProperties` — §1 Locked Decisions ("common core + per-kind
/// payload"). Variants with no component-specific animatable state beyond
/// `PaintProperties` (Rect, Container, Canvas) carry no payload at all.
pub enum NodeKind {
    Rect,
    Text(TextState),
    Image(ImageState),
    Container,
    Canvas,
    Slider(SliderState),
    Checkbox(CheckboxState),
    // ... one variant per MD3 component that needs its own animatable state
}

pub struct SliderState {
    pub thumb_position: Animated<f64>,   // 0.0..=1.0 along the track
}

pub struct CheckboxState {
    pub check_progress: Animated<f64>,   // 0.0..=1.0, drives the checkmark draw
}

/// Implemented by `PaintProperties` and by every `NodeKind` payload — the
/// one interface the central tick needs, so the tick system itself never
/// matches on a specific component. Returns `true` if any owned
/// `Animated<T>` is still active (this node stays dirty for another frame).
pub trait AnimatedNodeState {
    fn tick(&mut self, now: Instant) -> bool;
}

/// Implemented by every type an `Animated<T>` can wrap — `f64`, `peniko::
/// Color`, `kurbo::Affine`, `ShapeKey` (whose impl is the correspondence-
/// then-lerp technique in §7.4). Ordinary types get ordinary linear
/// interpolation; `ShapeKey` is the one non-trivial impl.
pub trait Interpolate {
    fn interpolate(&self, other: &Self, t: f64) -> Self;
}

pub struct Animated<T: Interpolate> {
    pub current: T,
    pub active: Option<ActiveAnimation<T>>,
}

pub struct ActiveAnimation<T> {
    pub from: T,
    pub to: T,
    pub start: Instant,
    pub duration: Duration,
    pub curve: MotionCurve,           // MD3 named curve, see §7.5
    pub on_complete: Option<CompletionHandle>, // opaque handle, drained by engine-py — never invoked directly, see below
}
```

**Central tick, not per-widget:** one system runs once per frame, walks only the *active* animation set (not the whole tree), and for each dirty node calls `node.paint.tick(now)` and — if `NodeKind` carries a payload — `node.kind.tick(now)`, both through the same `AnimatedNodeState` trait, so the tick system itself never matches on a specific component. Either call advancing `current` and returning `true` keeps the node dirty for repaint next frame. This is what replaces per-widget animation boilerplate you'd get bolting a tweening system onto an existing widget-trait framework — the uniformity is in the *mechanism* (`Animated<T>` + `AnimatedNodeState`), not in one single struct shape.

**NodeKind stays a closed, MD3-enumerating enum — a deliberate, accepted coupling, not an oversight.** Multi-design-language support is explicitly out of scope (§1); a generic/extensible alternative (a primitive-only core enum plus a named animatable-scalar bag that `engine-md3` interprets) would be exactly the kind of abstraction LESSONS_LEARNED.md §1 warns against — real, maintained machinery built to decouple from a second consumer (a second design language) that current scope says will never arrive, the same shape of mistake as TRE's `tre-rhi-dx12`/`tre-rhi-metal` stubs. "`engine-core` is MD3-agnostic" (§1 Locked Decisions) means it has no dependency on the `engine-md3` crate and no MD3 color/theming/motion logic — not that it may never name a component. Every new MD3 component still costs a `NodeKind` edit in `engine-core`; that's the accepted trade-off, stated here explicitly so it reads as a decision, not a contradiction of the crate-boundary rule in §4.

**Completion delivery: queue-drain, not a direct callback.** The central tick never invokes anything — when an `ActiveAnimation` finishes, `tick()` only pushes its `CompletionHandle` onto a plain `Vec<CompletionHandle>` that the tick system owns and clears every frame. `engine-py` drains that vector once per frame, immediately after calling `tick()`, and resolves each handle against its own `HashMap<CompletionHandle, PyObject>` under the GIL. This keeps `tick()` a pure state-mutation pass with no reentrancy risk — nothing a Python callback does, including registering a brand-new animation, can happen while `tick()` is still mid-iteration over the active set — and keeps `ActiveAnimation<T>` a plain `Clone`/`Debug` data struct rather than one carrying a `Send`-closure bound purely to serve `engine-py`.

> **Review note (from the TRE archive):** the completion-callback wiring wasn't specified in the original draft of this section. `engine-core` has zero pyo3 dependency by design, and the central tick — which is what actually notices an animation has finished — lives there. But `on_complete: Option<CompletionHandle>` needs to reach an actual Python callback, which only `engine-py` knows how to invoke. This is exactly the class of boundary-crossing plumbing that turned out subtle in TRE's own AccessKit `ActionHandler`/`ActivationHandler` wiring — worth designing explicitly now rather than discovering the gap mid-implementation.
>
> **Decision recorded:** queue-drain, as described above — chosen specifically to avoid the reentrancy hazard of invoking an arbitrary Python callback synchronously from inside the tick loop.

---

## 6. Per-Frame Pipeline

```mermaid
flowchart TD
    EV[winit event: input / resize / animation-frame tick] --> APP[State mutation: PyO3 call or Rust-internal animation tick]
    APP --> TREE[Dirty-subtree marking]
    TREE --> LAYOUT[Taffy layout pass]
    LAYOUT --> TEXT[Parley: shape + measure text nodes]
    TEXT --> PAINT[Paint pass: build vello::Scene]
    PAINT --> A11Y[AccessKit TreeUpdate, built from the same tree]
    A11Y --> ADAPTER[accesskit_winit to platform adapter]
    PAINT --> RENDER[vello_hybrid renders Scene]
    RENDER --> PRESENT[wgpu surface present]
```

Accessibility tree construction happens in the same pass as paint, from the same node tree — not bolted on afterward.

**Layout dirty-scoping: lean on Taffy's own cache, don't reinvent it.** `PaintProperties` and *most* `NodeKind` payloads (§5) share no fields with `taffy::Style` — they're structurally disjoint types. That means an `animate()` call touching only paint state (a ripple radius, an opacity fade, an elevation change) never has any reason to call `mark_dirty()` on Taffy's tree at all, and a call that *does* change `layout_style` is the only thing that ever does. `compute_layout()` runs unconditionally every frame, but Taffy's own internal caching makes that a cheap no-op for every subtree nothing marked dirty — no parallel `dirty_layout`/`dirty_paint` bookkeeping to build, keep in sync, or mis-classify a property against as MD3's component catalog grows. The same reasoning applies to `parley`: text-shaping is only re-run for a node whose actual text content, font, size, or available width changed, never for a purely visual animation — whether that needs an explicit shaped-run cache or falls out of the same "only touch what changed" discipline is left to the typography spike (§14 step 4) to determine from real measurements, not decided here in the abstract.

**Named exception:** `Splitter`/`DockZone` (§11.4, §11.5) are the one deliberate case where an `Animated<f64>` genuinely *is* layout-affecting — a splitter drag has to change actual allocated space, not just paint. Their tick handler calls `taffy.mark_dirty()` on the affected siblings directly, unlike every other `NodeKind` payload so far. This doesn't break the mechanism above — Taffy's cache still scopes the recompute to just those two siblings, not the whole tree — it just means "paint-only by construction" was true for every component that existed before §11, not a universal property of `Animated<T>` itself.

**Paint is a full scene re-encode, not a hand-rolled patch — and that's expected, not a gap.** `vello_hybrid`'s `Scene` is a command-recording API rebuilt by walking the tree each frame, the same way Vello's own reference consumers (Xilem, Masonry) work — there's no supported way to patch a persisted partial scene, so this pipeline doesn't attempt one. The actual per-frame cost lever isn't "skip re-encoding," it's "skip the *expensive host-side inputs* to encoding" (unchanged layout, unchanged shaped text) — which is exactly what the layout- and text-caching above already do.

**AccessKit updates scope to the dirty set for free.** `accesskit::TreeUpdate` is itself a partial-update API — a `Vec` of just the nodes that changed, not a mandatory full-tree snapshot every frame. The paint pass only needs to emit `accesskit::Node` entries for nodes in the same dirty set already computed for paint, not the whole tree.

**Frame budget: a stated target, paired with an enforcing trigger.** Steady state should comfortably fit inside a 60 Hz frame (16.6 ms), with 120 Hz (8.3 ms) as a stretch goal — not TRE's sub-millisecond, hand-tuned-Vulkan, zero-allocation target, since `vello_hybrid`/`wgpu`'s GPU-side compute model doesn't offer the same hand-tuning surface and this project's stated priority (§1) is Python-framework completeness over raw frame-time supremacy. Per LESSONS_LEARNED.md §5 ("any claim of the form 'we require X' needs a CI job that actually tests it, added in the same commit that makes the claim"), this number is fiction until it's mechanically checked: add a frame-time benchmark to CI no later than build-order step 3 (§14, once `taffy` layout is wired and a real, if minimal, render+layout+tick pipeline exists to measure) — the same discipline TRE's MSRV claim skipped for most of that project's life.

> **Review note (from the TRE archive):** no performance budget or re-layout scoping was stated in the original draft of this section. "Dirty-subtree marking" was named as a pipeline step, but the diagram still showed one monolithic Taffy layout pass and one scene-build pass per frame with no stated scoping mechanism. TRE's most expensive-to-retrofit engineering discipline was exactly this — a mechanically-enforced, near-zero-allocation, sub-millisecond frame budget — and it was far cheaper to have designed in from the start than to bolt on after the fact.
>
> **Decision recorded:** layout/text work scopes itself for free via structural type separation (paint state literally isn't visible to Taffy) plus Taffy's own incremental cache, not a hand-rolled parallel dirty system; a 16.6ms/8.3ms target is stated above with a CI benchmark trigger at build-order step 3, so the number can't quietly become fiction the way TRE's MSRV claim did.

---

## 7. Material Design 3 Subsystem (`engine-md3`)

### 7.1 Dynamic color
Use `material-colors` (or compare current MCU-port crates — several exist): HCT color space, tonal palette generation, scheme generation from a seed color. Generates a full light/dark scheme; `engine-md3` maps scheme roles (primary, on-primary, surface, etc.) onto `peniko::Color` values consumed by `PaintProperties`.

**Acceptance gate, not just "verify maintenance status":** whichever crate is chosen must pass Material Color Utilities' own published reference test vectors (HCT round-trip conversions, tonal palette values, contrast ratios) before it's pinned — not just "compiles and the colors look plausible." HCT color-space conversion, tonal-palette generation, and dynamic-scheme contrast math are a large, easy-to-get-subtly-wrong subsystem; porting it in-house was considered and rejected here as a bigger, riskier undertaking than it looks (unlike shape morphing's one missing correspondence step, §7.4 — there, no reference implementation exists to lean on at all; here, one does, so use it and hold the dependency to its standard rather than re-deriving the standard from scratch).

**Live theme switching is in scope.** `winit` emits a `ThemeChanged` event when the OS light/dark preference changes at runtime; `engine-platform` forwards it through the same `AppHandler`/`InputEvent` inversion already wired for every other input event (§4) rather than requiring an app restart. The one thing this needs that per-frame animation doesn't: regenerating a scheme and repainting **every** themed node is a whole-tree paint-dirty event, not a scoped one — acceptable because a theme change is a rare, discrete event, not a per-frame cost (§6's frame budget concerns the steady-state animation path, not this).

### 7.2 Elevation / shadows
`vello_hybrid`'s `Scene::fill_blurred_rounded_rect` (with `invert` for inset shadows) and `FilterPrimitive::DropShadowOnly` cover MD3 elevation shadows directly.

**Flag:** early-stage per Vello's own release notes — no API stability guarantee yet, uneven feature parity across the `vello` / `vello_cpu` / `vello_hybrid` variants. **Spike this standalone** (render one elevated rounded rect through the exact pinned `vello_hybrid` version) before building MD3 components against it.

### 7.3 State layers / ripple
No new dependency. `Scene::push_layer(clip_path, ...)` with a per-ripple `Animated<f64>` radius and `Animated<f64>` opacity, using the same central tick as everything else.

Ripple state lives on `Node` itself (`interaction: Option<InteractionState>`, §5) — not folded into `PaintProperties` or a `NodeKind` payload, since ripple applies across many otherwise-unrelated `NodeKind` variants (buttons, chips, FABs, list items, icon buttons, ...) rather than belonging to any one of them:

```rust
pub struct InteractionState {
    pub ripples: SmallVec<[RippleState; 4]>, // small bound — MD3 doesn't expect many concurrent ripples
}

pub struct RippleState {
    pub origin: kurbo::Point,   // press location, clip-path center
    pub radius: Animated<f64>,
    pub opacity: Animated<f64>,
}
```

Each press pushes a new `RippleState`; each animates independently through the same central tick, and a finished ripple is removed from the `SmallVec` via the same completion-queue mechanism §5 already defines for `on_complete` — reused per-ripple rather than per-property. This is what lets rapid taps produce MD3's real overlapping-ripple look instead of one ripple snapping/restarting per node.

> **Review note (decision recorded):** the original draft modeled ripple as exactly one `Animated<f64>` radius + one `Animated<f64>` opacity, which only supports a single ripple per node at a time. Real MD3 shows overlapping ripples under rapid taps; resolved above with a small bounded collection per node instead of a single scalar pair.

### 7.4 Shape morphing
No turnkey crate exists for this. Technique: equalize point/segment counts between the start and end `kurbo::BezPath`s (insert zero-length or subdivided segments into whichever has fewer), then linearly interpolate corresponding point positions per frame — the standard approach used by shape-morphing tools generally, ported onto `kurbo` primitives. Build as a small internal module in `engine-md3`. **No library to lean on here — budget real implementation time.**

> **Review note (from the TRE archive):** "equalize count, then lerp" is only half the standard technique, and the missing half is the half that actually determines whether the morph looks good. Naive per-index interpolation assumes point *N* on the start path visually corresponds to point *N* on the end path — for two arbitrary shapes (a circle morphing into a star, a FAB morphing into an extended FAB with a different point count and rotation) that assumption is usually false, and the result is self-intersecting or wildly-rotating geometry mid-morph rather than a clean transition. Real shape-morphing tools add a correspondence/alignment search first: try several starting-point offsets (and possibly winding-direction flips) between the two point sets and pick the one that minimizes total point-travel distance, *then* lerp. Budget real implementation time for that search step specifically, not just the interpolation loop — it's the harder half.

### 7.5 Motion tokens
MD3 named easing curves (Standard, Emphasized, etc.) are cubic-bezier control points — evaluate with `kurbo`'s curve math or a direct implementation. Durations and curves live as a static data table in `engine-md3`, not as a separate tweening dependency.

### 7.6 Container transform

MD3's container-transform pattern (a FAB expanding into a bottom sheet, a card expanding into a detail view) is in scope for v1 — but it is **not a new core mechanism**. It's `engine-md3` choreographing several `ActiveAnimation`s already defined in §5 across two participating nodes, with correlated timing. No navigation/router/screen-stack subsystem needs to exist anywhere in this framework for it to work, and none is being added here.

The choreography, concretely:

1. **Capture.** Just before the transition starts, read the trigger node's *computed* layout (resolved position + size from Taffy's last layout pass — not its `taffy::Style` input) and current `PaintProperties` (`corner_radius`, `background`, `transform`). This is the transition's `from` state. It requires one small, genuinely new addition to the Node API: `engine-core` must expose computed layout output per node as a queryable value (e.g. `Node::computed_layout() -> taffy::Layout`), not just consume it internally during the paint pass (§6) as it does today.
2. **Insert the destination container.** A new node — the expanded surface — is added to the tree (e.g. as a top-level child of the root, above everything else in paint order) with `PaintProperties` initialized to *exactly* the trigger's captured bounds/corner_radius/background. Nothing visually changes yet.
3. **Drive one synchronized animation set.** The destination's `transform`, `corner_radius`, `background`, and `elevation` each get an `ActiveAnimation` toward their real target values, all sharing one `start: Instant` and one MD3 motion curve/duration (§7.5, typically "Emphasized"). This is the same `Animated<T>` + central tick mechanism as any other property animation on any other node — the tick system still never special-cases this.
4. **Content cross-fade.** The trigger's own content (icon/label) gets an `Animated<f64>` opacity animation toward `0.0` starting immediately; the destination's real content gets an `Animated<f64>` opacity animation toward `1.0` with a *staggered* `start: Instant` (set to "now + N ms," not "now"). `ActiveAnimation<T>` (§5) already carries a `start: Instant` field for exactly this — no new struct field, just `engine-md3` choosing to construct one with a future start time.
5. **Teardown.** On completion (via §5's queue-drain mechanism), the trigger node is hidden or removed and the destination container becomes the persistent subtree going forward — an ordinary tree mutation, not special-cased machinery.

**What this deliberately does *not* require:** a navigation/router/screen-stack subsystem. "Showing a new screen" in this tree model is already just "add or reveal a subtree" — whatever pattern the Python app author uses for that (a single persistent root that swaps its child, an app-level stack the app itself manages) is orthogonal to container-transform, which only ever cares about two nodes' bounds and paint state at one point in time. `engine-md3` exposes the choreography above as a single helper function; it does not own *what screens exist* or *how you got there* — keeping this addition scoped to what §7.4's morphing module and §5's animation core already make possible, not a new navigation architecture bolted on to justify it. (§11.2's `AppShell` later names the "single persistent root that swaps its child" pattern explicitly, as a reusable composition — it still isn't a router, and this section's reasoning is why one was never needed.)

---

## 8. Python/Rust FFI Boundary (`engine-py`)

```rust
#[pyclass(unsendable)]
pub struct PyNode {
    handle: NodeId,
    tree: Rc<RefCell<Tree>>, // main-thread-only (§9) — not Arc<Mutex<>>; PyO3 panics if ever touched from another thread
}

#[pymethods]
impl PyNode {
    fn animate(&self, py: Python, property: &str, to: PyObject, duration_ms: u64, curve: &str) -> PyResult<()> { /* registers an ActiveAnimation, returns immediately */ }
    fn add_child(&self, child: &PyNode) -> PyResult<()> { /* tree mutation; rejects a child that is an ancestor of self with a PyValueError, not a silent cycle */ }
    fn set_on_click(&self, callback: PyObject) -> PyResult<()> { /* stored in the shared Tree's callback map (below), invoked via Python::with_gil on the matching winit pointer event */ }
}

/// One `#[pyclass(gc)]` per OS window (§11.1), not a single process-wide
/// instance — each window owns an independent `Tree`, so each needs its
/// own callback-map traversal. Every stored Python callback — click
/// handlers, animation on_complete handles (§5), and virtualized-list
/// materializers (§11.7) alike — lives in `Tree`'s own maps, not scattered
/// across individual `PyNode`s, so exactly one type per window needs to
/// implement PyO3's cyclic-GC protocol.
#[pyclass(gc, unsendable)]
pub struct PyWindow {
    tree: Rc<RefCell<Tree>>, // this window's own Tree; every PyNode created in it shares this handle
}

#[pymethods]
impl PyWindow {
    fn __traverse__(&self, visit: pyo3::PyVisit<'_>) -> Result<(), pyo3::PyTraverseError> {
        let tree = self.tree.borrow();
        for cb in tree.on_click.values() { visit.call(cb)?; }
        for cb in tree.on_complete.values() { visit.call(cb)?; }
        for cb in tree.materializers.values() { visit.call(cb)?; }
        Ok(())
    }
    fn __clear__(&mut self) {
        let mut tree = self.tree.borrow_mut();
        tree.on_click.clear();
        tree.on_complete.clear();
        tree.materializers.clear();
    }
}

/// The single process-wide entry point (`app.run()`, Design Principle 1,
/// §2) — this is what owns the `winit::EventLoop`, not any per-window
/// state. `PyApp` creates `PyWindow`s (§11.1: one per OS window) but holds
/// no `Tree` of its own and needs no GC participation itself, since it
/// never stores a Python callback directly.
#[pyclass(unsendable)]
pub struct PyApp;

#[pymethods]
impl PyApp {
    fn create_window(&self, py: Python) -> PyResult<PyWindow> { /* opens a new winit::Window via engine-platform, returns its PyWindow */ }
    fn run(&self, py: Python) -> PyResult<()> { /* the one blocking call — Design Principle 1 */ }
}
```

> **Consistency-pass finding:** the original `PyApp` (a single struct holding one `Tree` directly) predates §11.1's multi-window decision and became wrong once that landed — a single `Tree` field can't represent "one independent `Tree` per OS window." Split above into `PyWindow` (one per window, owns that window's `Tree` and is the `#[pyclass(gc)]`) and `PyApp` (the actual `app.run()` singleton, owns no `Tree`, needs no GC participation). `PyNode`'s shape is unaffected — it already only ever held a `Tree` handle, never cared how many other `Tree`s existed elsewhere.

Design rules for this crate specifically:

- Every setter/mutator is a direct, synchronous call — no async surface at this layer.
- `animate()` registers work and returns; it never blocks waiting for the animation to finish.
- Callback invocation (`on_click`, animation `on_complete`) is the *only* place `Python::with_gil` is taken from inside the render/event loop.
- No Vello/kurbo/peniko/taffy/accesskit type crosses this boundary — translate to plain Python-friendly types (floats, strings, tuples, small dataclasses) at the edge.
- Errors funnel through one `EngineError` (via `thiserror`, §3) with a single `From<EngineError> for PyErr`, not ad hoc `PyErr::new_err` scattered per call site:

```rust
#[derive(thiserror::Error, Debug)]
pub enum EngineError {
    #[error("{kind} has no property '{property}'")]
    UnknownProperty { kind: &'static str, property: String },
    #[error("property '{property}' expects {expected}, got {actual}")]
    TypeMismatch { property: String, expected: &'static str, actual: String },
    #[error("cannot add a node as a child of its own descendant")]
    CycleRejected,
}

impl From<EngineError> for PyErr {
    fn from(e: EngineError) -> PyErr {
        match e {
            EngineError::UnknownProperty { .. } => PyValueError::new_err(e.to_string()),
            EngineError::TypeMismatch { .. } => PyTypeError::new_err(e.to_string()),
            EngineError::CycleRejected => PyValueError::new_err(e.to_string()),
        }
    }
}
```

> **Review note (from the TRE archive):** `animate(property: &str, to: PyObject, ...)` needs an explicit type-dispatch design, not just a signature. Something has to map `property`'s string value to the correct `Animated<T>` field, downcast `to` into that field's concrete type, and — critically — surface a clear `PyTypeError`/`PyValueError` (e.g. "opacity expects a float, got a str") when a caller gets it wrong, rather than panicking across the FFI boundary or silently coercing. This is exactly the class of "caller-supplied input reaching native code" surface TRE's own security review process treated seriously (see the archive's finding on custom-shader input hardening) — worth designing the validation and error path deliberately here too.
>
> **Sharpened by the §1/§5 node-model decision:** the dispatch is now concretely two-level — try `property` against `PaintProperties`' own field names first (universal, always present), then against the node's `NodeKind` payload's field names if it has one (e.g. `"thumb_position"` only resolves on a `Slider`). A property name that matches neither should name the node's actual kind in the error ("Checkbox has no property 'thumb_position'"), not just say "unknown property."
>
> **Decision recorded:** `animate()` stays the single generic, string-keyed entry point — no per-property typed methods. Adding one typed method (or one wrapper Python class) per animatable property would, for `NodeKind`-specific properties, effectively require a Python class per MD3 component to be fully realized — multiplying the FFI surface by the component count, exactly the growth cost §7's closed-`NodeKind` decision already chose to accept once rather than pay again here. `EngineError` above is the concrete error path this note asked for.
>
> **Review note (from the TRE archive):** storing a long-lived `PyObject` callback in a Rust struct (`set_on_click`) is a risk class TRE never had to deal with — it never held Python callbacks across frames the way a click-handler system fundamentally requires. If a stored closure ever captures the widget it's attached to (a very natural pattern — "on_click: lambda: self.set_state(...)"), that's a reference cycle CPython's own GC can't see through unless the `#[pyclass]` participates in Python's cyclic GC via `__traverse__`/`__clear__` (PyO3's `#[pyclass(gc)]` support).
>
> **Decision recorded:** implemented now, not deferred — `PyWindow` above. This project's own target (long-running desktop apps with dynamically created/destroyed widgets — dialogs, list items) is exactly the profile where an uncollected cycle accumulates during normal operation, not just at process exit, unlike a short-lived script where it wouldn't matter.

### MVVM data binding

A `bind(view_model, attr, node, property)` helper — smaller than a full binding subsystem, because it's pure-Python wiring onto the `animate()` path that already exists, not a new Rust mutation mechanism:

```python
# shipped in the framework's own Python package, not engine-py's Rust surface
class Bindable:
    def __set_name__(self, owner, name):
        self._name = f"_bindable_{name}"
    def __get__(self, obj, objtype=None):
        return getattr(obj, self._name, None)
    def __set__(self, obj, value):
        setattr(obj, self._name, value)
        for callback in obj.__bindings__.get(self._name, []):
            callback(value)

def bind(view_model, attr: str, node: PyNode, property: str, duration_ms: int = 0, curve: str = "linear"):
    view_model.__bindings__.setdefault(f"_bindable_{attr}", []).append(
        lambda value: node.animate(property, value, duration_ms, curve)
    )
```

A `ViewModel` author declares state with `is_visible = Bindable()` instead of a plain attribute; every write goes through `Bindable.__set__`, which fans out to every `node.animate(...)` call registered against it — the existing FFI call, unmodified. **"Commands"** (a ViewModel method invoked by user interaction) need no new mechanism at all: `set_on_click(view_model.on_save)` already works today, since a bound Python method is just another callable. Default is an instant snap (`duration_ms=0`); a caller passes a real duration/curve to animate a bound update instead.

**GC, resolved for free, not by extension:** the binding closure above is a plain Python object living in `view_model.__bindings__` — an ordinary Python `dict` on an ordinary Python object, already inside CPython's own cyclic-GC graph with no Rust-side involvement. The only place a genuine cycle can hide is exactly the one §8 already solved: a stored callback (`on_click`, `on_complete`) capturing a `ViewModel` that itself holds a binding back to that same node. `PyWindow`'s existing `__traverse__`/`__clear__` (above) already accounts for that half of the graph; the binding's own half needs no new GC protocol, since it never leaves Python's object graph in the first place.

---

## 9. Threading & Event Loop Model

- **winit owns the main thread**, unconditionally. This is a hard platform requirement on macOS and the simplest correct choice everywhere else.
- Python never drives the loop. It's reached only via `Python::with_gil` from inside a Rust-side callback invocation (pointer event → registered click handler; animation completion → registered callback).
- `Python::allow_threads` wraps layout/paint/render so per-frame Rust work never serializes behind the GIL. **This is currently a no-op-cost safety habit, not a live requirement:** with `Tree` as `Rc<RefCell<>>` (below) there is, for now, no second thread ever contending for the GIL during a frame. Keep the wrapping anyway — it costs nothing today and is the one thing that won't need revisiting if a future GIL-holding thread (see the asyncio note below) is ever added.
- If the framework needs asyncio-native app code, bridge via `pyo3-async-runtimes` — keep this strictly separate from the render loop; asyncio drives app logic, winit/engine-core drives rendering, they meet only at the callback boundary. **Deferred, not designed:** this is explicitly optional/future (§3) and its exact threading shape (an interleaved event loop cooperatively stepped from inside winit's own loop, vs. a genuinely separate executor thread) is undecided — see the `Tree` ownership decision below for why that undecided shape doesn't block anything today.

**`Tree` is `Rc<RefCell<Tree>>`, not `Arc<Mutex<Tree>>` — single-threaded by construction, not by convention.** `PyNode`, `PyWindow`, and `PyApp` (§8) are all `#[pyclass(unsendable)]`: PyO3 enforces main-thread-only access at runtime, panicking immediately if the object is ever touched from a second thread, rather than this being an unenforced assumption. This matches what v1 actually needs — asyncio bridging is optional/future and its threading shape isn't even decided yet (above) — and avoids paying a real, permanent per-frame lock-acquisition cost (§6's frame budget) plus a genuine two-lock-ordering deadlock hazard (GIL vs. a `Tree` mutex, acquired in opposite orders by the render thread and a hypothetical callback-invoking async thread) for a capability that isn't confirmed to ship. **If/when the asyncio bridge is actually built, revisit this decision first** — depending on the threading shape chosen then, `Tree` may need to migrate to `Arc<Mutex<>>` at that point, as a scoped, well-understood follow-up rather than something paid for speculatively now.

**Unhandled exceptions from a callback are caught, logged, and non-fatal.** A `Python::with_gil` call to `on_click`/`on_complete` wraps the call so a raised Python exception is caught, its full traceback logged via `tracing::error!` (§3), and the render loop continues — the callback's effects are simply incomplete, not the whole application. This matches how Tkinter/Qt/GTK all treat a callback exception: one broken handler shouldn't take the whole window down for an end user of a shipped app, even though it means a bug can degrade a running app silently until someone reads the log.

> **Review note (from the TRE archive):** worth stating explicitly rather than leaving implicit — since callbacks run on winit's main thread, a slow Python click handler (or `on_complete` handler) will visibly stall the whole render loop; there's no isolation between "app logic taking a while" and "frames drop." Most main-thread GUI toolkits (Tkinter, PyQt without explicit threading) accept exactly this constraint, so it's likely fine as a stated, deliberate limitation — just make it a stated one, so framework users know not to do slow work in a click handler, rather than an implicit trap they discover by hitting it.
>
> **Decision recorded:** accepted as stated above, with no mitigation beyond documentation — matches every mainstream main-thread-owned GUI toolkit's own accepted trade-off.

---

## 10. Accessibility

`accesskit` + `accesskit_winit`, wired directly (no Masonry intermediary). `AccessNodeData` on each `Node` is the source of truth; the `TreeUpdate` sent to AccessKit is built fresh from the current `Node` tree every frame, not maintained as a separate parallel structure that can drift out of sync.

**`engine-core` builds the `TreeUpdate`, not `engine-render` (§4).** `accesskit` (the plain data crate — `Node`, `TreeUpdate`, `Role`, `Action` — not `accesskit_winit`, which stays in `engine-platform`) is a small, OS-agnostic dependency, architecturally the same class as `taffy`/`parley`, both already core dependencies. `Tree` exposes `build_access_update(&self) -> accesskit::TreeUpdate`; `engine-platform` — which already depends on `engine-core` (§4) — calls it once per frame and feeds the result to its own `accesskit_winit` adapter. This keeps a11y-semantic interpretation (what a `Role`/state actually means) with the crate that already owns `AccessNodeData`'s meaning, rather than teaching `engine-render` — whose only other job is GPU rendering — a second domain it has no other reason to know.

```rust
pub struct AccessNodeData {
    pub role: accesskit::Role,
    pub label: Option<String>,
    pub description: Option<String>,
    pub states: AccessStates,       // checked, expanded, disabled, selected, ... — plain bitflags-style data
    pub actions: Vec<accesskit::Action>, // which AT-SPI/UIA/NSAccessibility actions this node responds to
}
```

**A minimal keyboard focus model lives in `engine-core`, not left unspecified.** `Tree` tracks `focused: Option<NodeId>`; Tab/Shift-Tab moves focus in tree order (the same order the `TreeUpdate` already walks); `Enter`/`Space` on the focused node dispatches `accesskit::Action::Default`, and platform-driven focus requests (a screen reader focusing a node directly) dispatch `accesskit::Action::Focus` — both routed through the same `AppHandler`/`InputEvent` inversion already wired for pointer events (§4), so keyboard input isn't a second, parallel dispatch mechanism. **Explicitly out of this minimal model:** component-specific keyboard semantics — arrow keys moving between options in a radio group, a slider's arrow-key increments — are deferred to per-component design in `engine-md3`, exactly when each such component is actually built, the same way `NodeKind` payloads already are (§7). Keyboard operability (WCAG 2.1's baseline requirement, not a nice-to-have) ships from day one; the component-specific *extent* of it grows incrementally with the component catalog.

> **Review note (from the TRE archive), first-hand:** `accesskit` had real breaking changes across the version range I worked through directly on TRE this session (0.17→0.25): `Tree` became a deprecated alias for `TreeInfo` and lost its `app_name` field entirely (the Linux adapter now derives the app name from the running executable instead); `TreeUpdate` gained a new required `tree_id` field; and the AT-SPI2 object-path encoding changed shape (a node's id moved from being the literal last path segment to being packed into the high 64 bits of a 128-bit value alongside a tree index) — none of which I could have predicted from documentation alone; each was found by reading the new version's actual source. Pin an exact `accesskit`/`accesskit_winit` version early, and re-verify the real current API against the pinned version's own source at implementation time rather than assuming this document's description still matches whatever version ends up resolved.

---

## 11. Desktop Shell & Workspace

Material Design 3 is a mobile/web-first design language — it has no concept of a menu bar, docking, splitters, or multiple windows. This section covers what a real desktop app needs that MD3 doesn't specify, resolved against everything already locked in §1–§10 rather than as a bolted-on afterthought.

### 11.1 Multiple windows

One `Tree` per OS window (§5, §9) — each `winit::Window` owns an independent node arena, root, and `focused: Option<NodeId>` state; `engine-platform` manages a `HashMap<WindowId, Tree>` and dispatches each window's events to its own tree, unchanged from the single-window model already designed. `winit` already tags every event with the `WindowId` it belongs to, so routing an event to the right `Tree` before translating it into `engine-core`'s `InputEvent` (§4) is a lookup, not new dispatch machinery — `InputEvent` itself stays window-agnostic, since by the time it reaches `AppHandler` it's already scoped to the correct `Tree`. A window closing just drops its `Tree`. **Accepted gap:** there's no cross-window node transfer — a `NodeId` is only meaningful within the generational slotmap (§5) of the `Tree` that issued it. "Detach this panel into its own window" (a common docking feature, §11.4) therefore means *recreating* the panel's subtree in a new `Tree`, not moving existing `Node`s across the boundary — a real, if narrow, limitation to revisit only if a docking UX specifically needs true node migration rather than teardown-and-rebuild.

### 11.2 App shell & single-page navigation

One `Tree` may optionally be composed as an **`AppShell`** — a persistent root layout with named regions (menu bar, toolbar, dock zones, status bar, and exactly one **content** region) — rather than every app hand-composing shell layout from scratch. `content`'s `NodeId` is stable — it names one `Container` node that never changes identity — so "navigating" means replacing *that node's children*, an ordinary, already-supported tree mutation (§5, §8's `add_child`/removal), optionally choreographed through container-transform (§7.6); it is not a general multi-screen router and does not require one, matching the single-page-app model this framework targets. `AppShell` is a composition convenience, not mandatory: a secondary window opened per §11.1 can be a bare content tree with no shell regions at all (a tool palette, an about box) — only windows that want the standard chrome use it.

```rust
pub struct AppShell {
    pub menu_bar: Option<NodeId>,
    pub toolbar: Option<NodeId>,
    pub dock: DockLayout,           // §11.4 — may be empty
    pub status_bar: Option<NodeId>,
    pub content: NodeId,            // exactly one — the swappable region
}
```

Each present region is a completely ordinary node subtree (a flex `Container` for a toolbar, ordinary MD3 components inside) positioned by the shell's own root `taffy::Style` (a standard header/content/footer column — flexbox already does this natively, no new layout mechanism needed). Only `content`'s designation as *the* swap target, and the region names themselves, are new.

### 11.3 Menus, popups & dialogs — one overlay mechanism, not three

`§3`'s "native menus deferred" decision (avoiding `muda`'s GTK-on-Linux dependency) stands — reinforced, not reopened. Menu bars, dropdown menus, context menus, tooltips, and MD3 dialogs are all the same missing primitive: a node rendered above normal paint order, positioned relative to a trigger, dismissed on outside-click or Escape. This is the identical "insert a node above the root, outside normal paint order" trick container-transform's destination container already uses (§7.6) — not a second mechanism, and deliberately **tree-resident, not a parallel structure**: an overlay's root is an ordinary child of `Tree`'s root, using Taffy's existing `Position::Absolute` (positioned relative to its anchor's computed bounds, not flex-flowed alongside normal siblings — no new layout mechanism), and *appended* to the root's `children` (opening an overlay pushes it onto the end of the list). **Also previously unstated, now explicit:** paint (§6) draws a node's children in `children`-list order, so an appended overlay paints on top with no separate z-order concept needed — the same convention hit-testing's reverse-order walk (§11.10) relies on. Because it's a real tree node, paint (§6), hit-testing (§11.10), the focus model (§10), and `build_access_update()` (§10) all already walk it with zero special-casing — none of those four subsystems need to learn about a separate "overlay" concept at all. Only its anchor/dismissal *behavior* needs new bookkeeping:

```rust
pub struct OverlayMeta {
    pub anchor: NodeId,             // positions the overlay's root relative to this node's computed bounds
    pub dismiss_on_outside_click: bool,
    pub dismiss_on_escape: bool,
}
```

`Tree` holds a small `HashMap<NodeId, OverlayMeta>` (keyed by the overlay root's own `NodeId`) purely for this metadata — not the node itself, which already lives in the ordinary tree. Each overlay is an ordinary subtree using every existing mechanism (`Animated<T>` for enter/exit fades, the focus model for Tab-navigating menu items, `AccessNodeData` for role `MenuItem`/`Dialog`). A menu bar (File/Edit/View/Help) is `NodeKind::MenuBar` containing `NodeKind::MenuItem`s, where activating one adds a `NodeKind::Menu` dropdown as a root child with an `OverlayMeta` entry; a dialog is the same pattern with a root whose content is an MD3 scrim + centered surface, exactly matching MD3's own dialog spec (which is already an overlay pattern, not a new-window pattern). Keyboard mnemonics (Alt+F for File) extend the minimal focus model (§10) with a letter-keyed jump table scoped to whichever overlay is currently open, rather than a second input-dispatch path.

**Accepted platform-fit compromise:** on macOS, users expect the OS-level global menu bar; an in-window custom-rendered one is unconventional there specifically, even though it's normal on Linux and Windows. Revisit only if that specific platform gap becomes a real, reported problem — not preemptively, since building a *third* menu representation (native on macOS, custom elsewhere) now would be exactly the kind of speculative, unexercised complexity §1's own "no second design language" reasoning already argues against building.

### 11.4 Docking

In scope for v1 — a deliberate choice to build real, load-bearing new architecture here despite no MD3 precedent to draw from, because IDE-style rearrangeable workspaces are a stated requirement, not an optional nicety.

```rust
pub struct DockLayout {
    pub zones: [Option<DockZone>; 5],   // Left, Right, Top, Bottom, Center — a fixed, non-nested set for v1
}

pub struct DockZone {
    pub panels: SmallVec<[NodeId; 4]>,  // tabbed together when more than one
    pub active_tab: usize,
    pub size: Animated<f64>,            // this zone's extent — layout-affecting, same §6 named exception as Splitter.position (§11.5)
}
```

- **Drag-to-rearrange** reuses the overlay mechanism (§11.3) for drop-zone indicators (translucent highlight regions shown over candidate `DockZone`s while dragging a panel's header) and the same pointer-event dispatch already wired for everything else (§9) — not a new input-handling path.
- **Resizing** between zones is exactly one `NodeKind::Splitter` (§11.5) per zone boundary — docking is a *consumer* of splitters, not a second resize mechanism.
- **Tabbed grouping** (`panels` + `active_tab`) is a plain index switch, no different from any other single-active-child UI pattern.
- **Persisted layout:** `DockLayout` is deliberately POD-shaped (indices and small numbers, no live node references beyond `NodeId`s already stable within one `Tree`'s session) so an app can serialize/restore it — a real requirement for "remember my window layout" UX — without engine-py needing a bespoke serialization format.
- **Deliberately bounded scope:** exactly five fixed zones (`Left`/`Right`/`Top`/`Bottom`/`Center`), no arbitrary recursive splits. A fully general nested-splits docking model (arbitrary trees of horizontal/vertical splits, the way some professional IDEs work) is explicitly *not* what's being built — this fixed-zone model covers the common case at a fraction of the complexity, and is the right scope to ship rather than the largest thing docking could theoretically be.

### 11.5 Splitters

`NodeKind::Splitter` — a draggable divider between two sibling regions:

```rust
pub struct SplitterState {
    pub position: Animated<f64>,   // 0.0..=1.0 along the split axis, or an absolute size — implementation detail
}
```

On drag, `position`'s tick handler mutates its two adjacent siblings' `layout_style` (flex-basis or absolute size, respecting each side's own min/max constraints from `taffy::Style`) directly — this is §6's named exception: a genuinely layout-affecting `Animated<T>`, not a paint-only one, so it explicitly calls `mark_dirty()` on those two siblings rather than relying on the structural-disjointness trick every other component uses. Taffy's cache still scopes the resulting recompute to just those two siblings, not the whole tree. Used both standalone (any resizable-pane layout) and by docking (§11.4) for zone resizing — one mechanism, two call sites.

### 11.6 Toolbars & status bars

No new mechanism. Both are ordinary flex `Container`s (a fixed-height row of icon buttons; a fixed-height row of status text/indicators) placed in `AppShell`'s named regions (§11.2) — standard header/footer flexbox, which Taffy already does natively.

### 11.7 Virtualization

A framework-level `NodeKind::VirtualList` — the alternative (requiring 100,000 real `Node`s for a 100,000-row list) would fail §6's frame budget on layout/paint tree-walk cost alone, with zero animations even running:

```rust
pub struct VirtualListState {
    pub item_count: usize,             // the logical count — most items never become real Nodes
    pub item_extent: ItemExtent,       // fixed, or a size-hint callback for variable-height items
    pub materialized: BTreeMap<usize, NodeId>, // only the visible window (+ small overscan)
}
```

`materialized`'s values are exactly the `VirtualList` node's own `children` (§5) — the map just adds "which logical index" on top of the ordinary parent/children relationship, not a second, separately-tracked child set. Only this small windowed subset are real `Node`s at any time; scrolling recycles `NodeId` slots (via §5's generational index — an old slot's generation increments on reuse, so any stray reference to a scrolled-away item's `NodeId` fails safely) rather than allocating fresh nodes per item. `taffy::Style` for the scroll container uses `item_count × item_extent` as its estimated content size, so scrollbar sizing is correct without every item existing. **New FFI shape (§8):** unlike `on_click`/`on_complete`'s event-driven callbacks, this needs an ad hoc "materialize item N" callback invoked during layout/scroll — a genuinely different callback pattern, stored in `Tree.materializers` alongside `on_click`/`on_complete` and (like every other stored `PyObject`, §8) traversed by that window's `PyWindow::__traverse__`.

### 11.8 Culling

The paint pass (§6) skips Vello scene-encoding entirely for any subtree whose computed layout bounds — transformed into viewport space (§11.9) — don't intersect the current visible/clip region. This is unconditional, not a v1-vs-later decision: painting fully off-screen content was always wasted work, and it composes directly with virtualization (§11.7) rather than duplicating it — virtualization avoids *creating* off-screen items at all; culling skips painting whatever's off-screen for any other reason (scrolled slightly past the overscan buffer, clipped by a parent `Container`, off-screen in a pan/zoom canvas).

### 11.9 Transform composition & pan/zoom

**Previously unstated, now explicit:** `PaintProperties.transform` (§5) composes down the tree — a node's effective transform is its parent's effective transform composed with its own, exactly like nested `<g transform>` in SVG or any standard 2D scene graph. `kurbo::Affine` is a composable matrix specifically so this falls out of ordinary matrix multiplication during the paint walk, not a special case. A pannable/zoomable canvas is therefore not a new mechanism: animate a `Container`'s own `transform` (pan offset × zoom scale) and every descendant inherits it for free.

### 11.10 Hit-testing

**Also previously unstated:** pointer-to-node resolution walks the tree in reverse paint order (topmost first — this naturally reaches overlays, §11.3, before normal content, with no special-casing needed since they're ordinary trailing root children), transforming the pointer position into each candidate's local space via the inverse of its composed transform (§11.9), then testing containment against its computed layout bounds. `NodeKind::Canvas` may additionally supply a custom hit-test callback (a closer test than its bounding rect allows — a bezier curve within N pixels of the point, a specific plotted data point) that overrides the default rect test for that node only; nodes with no custom hit-test use the rect default. Without this, custom-drawn content (§11.11) would only ever be clickable across its whole bounding box, which is wrong for exactly the nodes that need precise hit-testing most.

### 11.11 Node graphs & charts

No new framework mechanism — both compose entirely from what's already specified: `NodeKind::Canvas` for custom-drawn content, pan/zoom via transform composition (§11.9), `kurbo` path drawing for links/chart geometry, and custom hit-testing (§11.10) for clicking a curved link or a specific data point. Large datasets (a graph with thousands of nodes, a chart with dense series) can reuse virtualization/culling (§11.7/§11.8) for which items get real draw calls at all — full level-of-detail/decimation logic beyond that is an application or library concern, not something this framework needs to own.

---

## 12. Project Structure

```
project-root/
├── Cargo.toml                    # workspace root
├── crates/
│   ├── engine-core/               # Node tree, Animated<T>, layout/text wiring, InputEvent/AppHandler, accesskit TreeUpdate building, focus model (§10), AppShell/Overlay/DockLayout + Splitter/VirtualList NodeKinds (§11) — no pyo3, no winit, no engine-md3
│   ├── engine-md3/                  # MD3 theming: color scheme, shadow/ripple helpers, shape morph, motion-curve presets, MD3-styled defaults for shell/dock/menu components (§11) — depends on engine-core (§1, §4)
│   ├── engine-render/              # vello_hybrid + wgpu + kurbo + peniko + parley wiring, scene building — no winit dependency (§4)
│   ├── engine-platform/              # winit EventLoop/ApplicationHandler + accesskit_winit adapter — the only crate depending on winit (§1, §4)
│   └── engine-py/                    # PyO3 bindings — the ONLY crate depending on pyo3
│       └── src/lib.rs
├── python/
│   └── <package_name>/
│       ├── __init__.py
│       └── widgets/
├── pyproject.toml                 # maturin build config
└── examples/
    └── standalone_render/          # cargo-run examples that bypass Python entirely — see §14
```

> **Review note (from the TRE archive):** design `standalone_render`'s examples as `cargo test`-runnable from day one, not as `cargo run --example` binaries you fold into CI later. TRE didn't do this until its very last finding (#261), after ~40 examples had accumulated with no `cargo test` coverage at all. One real constraint to design around: if any of these build a real `winit` event loop (likely, for anything exercising `engine-render` against a real window/surface), a plain `#[test]` function won't work — `winit` permits constructing an `EventLoop` only on a process's actual main thread and only once per process, ever, and `cargo test` runs each `#[test]` body on a worker thread. The fix that worked for TRE: register these as `[[test]]` targets in `Cargo.toml` with `harness = false`, so each gets its own process with a real main thread, and have each gracefully exit 0 (not panic) when no display/GPU is reachable, so `cargo test` still passes on a contributor machine with no display server.

`pyproject.toml`:

```toml
[build-system]
requires = ["maturin>=1.7,<2.0"]
build-backend = "maturin"

[project]
name = "yourframework"
requires-python = ">=3.9"

[tool.maturin]
module-name = "yourframework._core"
python-source = "python"
features = ["pyo3/extension-module"]
```

---

## 13. Environment Setup

### Prerequisites

| Tool | Notes |
|---|---|
| Rust (via `rustup`) | Stable channel. Verify MSRV against current Vello/Parley requirements at setup time — several of these crates bump MSRV on minor releases, don't assume a fixed number stays current. |
| Python | 3.9+ (match your framework's minimum supported version) |
| `maturin` | `pip install maturin` |
| Linux only | `libxkbcommon-dev`, Wayland or X11 dev headers, a Vulkan loader/driver |
| macOS only | Xcode command line tools (Metal available by default) |
| Windows only | MSVC build tools (DX12 available by default) |

### Bootstrap

```bash
# 1. Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable

# 2. Python environment
python -m venv .venv
source .venv/bin/activate        # .venv\Scripts\activate on Windows
pip install maturin

# 3. Build the extension and install it into the venv (editable/dev mode)
maturin develop

# 4. Smoke test
python -c "import yourframework; print(yourframework.__version__)"
```

### Dev loop

| Command | Use |
|---|---|
| `cargo check -p engine-core` | Fast iteration on tree/animation/layout logic — no Python rebuild |
| `cargo test -p engine-core` | Unit tests for animation/layout, independent of FFI |
| `cargo run -p engine-render --example smoke` | Standalone Rust rendering, bypasses Python entirely — first milestone, see §14 |
| `maturin develop` | Rebuild + reinstall the Python extension after touching `engine-py` |

### Packaging (later, not needed for early development)

Cross-platform wheel builds: `maturin-action` in GitHub Actions, matrixed across Linux/macOS/Windows × supported Python versions. Don't set this up until the API surface has stabilized past the spike stage.

> **Review note (from the TRE archive):** the "later" framing here is worth pushing back on for one specific piece — not the full packaging matrix, but a bare CI smoke test. TRE ran with **zero** CI coverage of its Python bindings for roughly three-quarters of the project's life; the first real run, added very late, found three genuine, previously-undetected bugs in a single pass (a packaging default silently duplicating system libraries — see the next note — a demo hardcoding a core count the CI runner didn't have, and a real GPU-resource teardown use-after-free). A minimal job — `maturin develop` succeeds, `python -c "import yourframework"` succeeds — costs almost nothing and should exist as soon as `engine-py` exists (build-order step 5), not deferred until the API stabilizes. Save the full cross-platform release matrix for later; don't save *all* CI for later.
>
> **Decision recorded (§1 Locked Decisions):** primary-OS CI starts at step 5 as recommended above. Windows/macOS CI is deliberately deferred rather than run from day one — but with a named trigger (no later than step 6) rather than an open-ended "later," specifically to avoid repeating the TRE pattern this note describes.
>
> **Review note (from the TRE archive), concrete and costly:** `maturin`'s default wheel build (`--auditwheel repair`, implied by not passing `--auditwheel skip`) **vendors system libraries into the wheel**, renaming their sonames in the process. TRE's real production bug: on the CI runner, the built wheel bundled its own copies of `libgdk-3`/`libgtk-3`/`libxkbcommon` alongside the system copies that `winit` (via `xkbcommon-dl`) and GTK dlopen at runtime — two copies of each in one process — which broke `winit`'s X11 keyboard-extension setup (`XKBNotFound`) and made GLib abort with a double type-registration segfault. It never showed up locally (a plain `maturin develop` doesn't repair the wheel) and took a full `LD_DEBUG=libs` trace under CI's exact conditions to diagnose. This project links the identical class of libraries on Linux (`winit`, `accesskit_winit`, `tray-icon`, `rfd`/`arboard`), so the same failure mode is live here. Decide deliberately: a manylinux-repaired portable wheel (test the *exact* built artifact end-to-end, not just `maturin develop`, before trusting it) or a system-linked wheel for known targets (`--auditwheel skip --compatibility linux`, correct on the CI/target distro, not portable to arbitrary end-user Linux systems). Don't let the default silently decide this for you.

---

## 14. Suggested Build Order

De-risk unknowns before building on them, per Design Principle 5.

1. `engine-render`: one static rounded rect through `vello_hybrid`, presented into a real window `engine-render`'s own example opens via `engine-platform` (a dev-dependency of the example only — `engine-render` the library still takes a generic window-handle parameter, per §4). No layout, no text, no Python.
2. Add `Animated<T>` + the central tick; animate that rect's color/elevation. Validates the animation core in isolation.
3. Wire `taffy` for layout of multiple static nodes. Add the frame-time CI benchmark here (§6 Locked Decisions) — this is the earliest point a real render+layout+tick pipeline exists to measure against the stated 16.6ms/8.3ms target.
4. Wire `parley` for text — don't stop at one static label. Render a small sample of MD3's real type scale (at least two type roles, e.g. Body and Headline, at their real weights/sizes) plus one non-trivial string (mixed-direction or a non-Latin script, if the framework needs to support one) to get real signal on `parley`'s current line-breaking/BiDi/font-fallback behavior before component work depends on it. Treat this as a lightweight spike, not just plumbing verification — `parley` is the same young Linebender family already flagged as a risk in §3, and text is the single most universally-visible thing the framework renders.
5. Wire `engine-py`: expose node creation + one property setter to Python; drive step 2's animation from a `.py` script.
6. Wire `accesskit`: confirm one button is correctly exposed to a screen reader.
7. **Spike:** MD3 shadow via `fill_blurred_rounded_rect`, standalone, against the exact pinned Vello version (§7.2 risk).
8. Ripple/state-layer via `push_layer` + animated alpha.
9. Shape morph module (§7.4) — the one component with no library to lean on.
10. Wire `material-colors` for a full dynamic color theme.

---

## 15. Risk Register

| Risk | Detail | Mitigation |
|---|---|---|
| Vello API churn | Pre-1.0; `vello`/`vello_cpu`/`vello_hybrid` split still settling, no stability guarantees | Pin exact versions; confine all Vello calls to `engine-render` |
| Shadow/blur feature parity | `fill_blurred_rounded_rect`/`DropShadowOnly` early-stage, uneven across variants | Standalone spike (§14 step 7) before MD3 components depend on it |
| `vello_svg` gaps | No text, clipping, masking, filters, patterns, group opacity | Pre-rasterize affected icons or patch upstream; don't assume full SVG fidelity |
| Shape morphing | No existing crate | In-house module against `kurbo::BezPath`; budget real time |
| GIL / event loop conflict | Two possible "loop owners" (winit, Python) | winit owns the main thread unconditionally; Python reached only via callback |
| No prior art | No existing project pairs this exact combination (hand-rolled retained tree + Vello + PyO3, MD3-targeted) | De-risk via the spike order in §14 before deeper investment |
| `maturin` wheel packaging *(from TRE archive)* | Default `--auditwheel repair` vendors system libraries (GTK, xkbcommon, etc.) into the wheel with mangled sonames — caused TRE a real duplicate-library segfault on Linux, undetectable via `maturin develop` alone | Decide manylinux-portable vs. system-linked wheels deliberately (§13); test the actual built wheel end-to-end under CI's real conditions before trusting it |
| `accesskit` API churn *(from TRE archive)* | Real breaking changes hit directly this session (0.17→0.25): `Tree`→`TreeInfo` losing `app_name`, a new required `TreeUpdate.tree_id`, a changed AT-SPI object-path encoding | Pin an exact version early; re-verify the real current API against the pinned source at implementation time, not against this document |

---

## ADR-001: Hand-rolled vs. Masonry

**Decision:** hand-roll the tree/animation substrate on top of `vello_hybrid` + `taffy` + `parley` + `accesskit` directly. Do not depend on Masonry.

**Why:** Masonry's value is centralizing focus/pointer/lifecycle/accessibility plumbing across independently-implemented Rust widget types — valuable if using its defaults. Every MD3 component here needs fully custom paint code regardless of the tree substrate, so that value doesn't apply. What actually matters — a uniform, centrally-ticked animation system across every widget, driven cleanly from Python — doesn't fit Masonry's per-widget-trait model without retrofitting, and is architecturally cleaner to bake into a purpose-built node type from the start. Masonry's focus/pointer/AccessKit *patterns* are still worth studying; the crate itself isn't adopted.

**Revisited after §5–§10 — the decision holds, and for the reason originally stated, not despite it.** By the time §8–§10 were designed, this project had ended up hand-building a real slice of exactly the plumbing Masonry centralizes: a keyboard focus model, click/pointer dispatch, PyO3 cyclic-GC handling for stored callbacks, and an `accesskit::TreeUpdate` builder. That could read as evidence against this ADR — except every one of those pieces was shaped by the animation model this project is actually built around (the completion-queue mechanism, §5; the `AppHandler` inversion, §4; the central tick touching focus/paint state uniformly). Sitting them on top of Masonry's own per-widget-trait plumbing wouldn't have avoided building them — it would have meant fighting Masonry's model to bolt in a uniform animation system it wasn't designed for, on top of still writing every MD3 component's paint code from scratch either way. The amount of custom plumbing turned out real; the reason it's justified is unchanged.

**Worth verifying at implementation time, not deciding here:** whether Linebender has factored any low-level pieces out of the full Masonry widget framework as independently reusable crates (a standalone tree-arena, or accesskit-integration helpers, separate from adopting Masonry's widget-trait model wholesale). This document's knowledge of Masonry's current crate structure isn't current — the same caution already applied to `accesskit`/`parley` elsewhere (§3, §10). If such extracted pieces exist and don't fight the central-tick model, reusing them for parts of §8–§10's plumbing could cut implementation cost without reversing this ADR, which is specifically about the *widget/animation model*, not about refusing every external crate on principle.

**Revisit trigger:** reopen this ADR only if Masonry ships (a) a documented, centrally-ticked animation system compatible with arbitrary per-widget state, or (b) an official MD3 (or general Material) widget set — either would remove the specific gap this ADR identifies. Absent either, re-confirming this decision costs nothing; reversing it after component work has started would cost a full tree/widget-model migration, so the bar for revisiting is this concrete, not "reconsider periodically."
