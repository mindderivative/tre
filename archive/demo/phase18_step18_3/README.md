# Demo: Phase 18 -- Wire Accessibility Into tre-python

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cargo build -p tre-rhi-vulkan --example canvas_accessibility_single_process_check_verify
cd demo/phase18_step18_3
../../.venv/bin/python demo.py
```

**What this proves.** GUI Readiness recommendation 7 -- the last real
architectural gap: `tre-a11y` (a real, working AT-SPI2 bridge via
`accesskit`/`accesskit_unix`) existed and was tested, but neither
`tre-engine` nor `tre-python` depended on it. This phase wires it in
for real, and corrects a stale theory this project's own docs had
carried forward unexamined for several phases along the way.

## Step 18.1: the real bug, identified but never fixed

The GUI Readiness artifact's own recommendation 7 said accessibility
needed "a real process-split," because `accesskit_unix`'s background
registration thread supposedly "never completes inside a process that
also links real Vulkan/X11 shared libraries." Reading this project's
own real investigation history (REVIEW.md finding #126, thirteen real
CI pushes) before touching anything showed that theory was directly
disproved partway through: a genuinely Vulkan/X11-free binary hit the
identical CI failure. The investigation continued through further
disproven hypotheses to the real, final root cause, which
`IMPLEMENTATION.md`'s own Step 5.3.3 entry states outright was
"identified but not implemented or pushed": `ensure_accessibility_enabled`
(in both `crates/tre-a11y/tests/round_trip.rs` and
`crates/tre-rhi-vulkan/examples/canvas_accessibility_verify.rs`) built
its `org.a11y.Status` proxy on the **a11y bus**, but `at-spi-bus-launcher`
owns `org.a11y.Bus`/`IsEnabled` on the **session bus**
(`g_bus_own_name(G_BUS_TYPE_SESSION, ...)`, confirmed by reading its
real source) -- every `get_property` call silently failed against a
nonexistent destination, and `.unwrap_or(false)` swallowed it as an
indistinguishable "not enabled yet."

**Fixed**: both functions now build `status` on a real, separate
session-bus connection. Real effect on this machine: `tre-a11y`'s own
round-trip test went from racing a 10s timeout (previously ~10.05s,
right at the edge) to completing in **0.11s** -- `IsEnabled` was true
instantly once queried on the right bus. The stale "Vulkan/X11
same-process" theory was also corrected in the doc comments of
`canvas_accessibility_demo.rs`/`canvas_accessibility_verify.rs`, which
still stated it as settled fact.

## Step 18.2: the real test this project had never actually run

Nobody had ever tested whether a real Vulkan-linked process publishing
its own AT-SPI2 tree actually works, because the wrong-bus bug fully
explained every CI symptom on its own. Two new, throwaway example
binaries answered this directly: `canvas_accessibility_single_process_check`
(since removed -- superseded by this demo) did real headless Vulkan
rendering AND a real `A11yBridge::connect`/`publish` in **one process**;
`canvas_accessibility_single_process_check_verify` (kept -- reused
below) queried it from a genuinely separate OS process. Run together on
this real desktop:

```
real Vulkan render complete in this process (ash/x11rb genuinely linked) -- now
publishing real AT-SPI2 accessibility data from the SAME process
A11yBridge::connect succeeded in a Vulkan-linked process ...
```
```
found our app in the real registry: bus=":1.74" ...
SINGLE-PROCESS FEASIBILITY CONFIRMED: ... extents=(10, 10, 60, 40)
```

**Single-process feasibility, confirmed for the first time.** The
"process split" this project's own GUI Readiness assessment had assumed
was needed for years was never a real requirement for an
*application* -- the role `tre-python` fills, not the AT's. This
project's own earlier `canvas_accessibility_demo`/
`canvas_accessibility_verify` split remains real and correct, but for
an unrelated reason `demo/phase5_step5_3_3/README.md` already
disclosed: it matches real AT-SPI2 deployment practice (a real screen
reader is always a separate process from the app it inspects), not a
genuine same-process technical limitation.

## Step 18.3: `tre.A11yBridge`, wired in for real

- `crates/tre-engine/src/canvas.rs`: `RenderingCanvas::accessibility_nodes()`
  -- a plain getter over the field `tag_accessibility_node` already
  populated; nothing outside the engine's own internal `FlattenedFrame`
  path read it before this.
- `crates/tre-python/src/canvas.rs`: `PyAccessibilityNode` (a plain
  data mirror) and `Canvas.accessibility_nodes()`.
- New `crates/tre-python/src/a11y.rs`: `tre.A11yBridge(app_name,
  toolkit_name, toolkit_version)`, wrapping `tre_a11y::A11yBridge::
  connect`/`.publish(nodes)` directly -- `unsendable`, matching
  `PyTrayIcon`/`PyClipboard`'s own established precedent for
  platform-connection state.

**A real, disclosed constraint found while building the demo**:
`A11yBridge` being `unsendable` means `publish()` must be called from
the same Python thread that constructed it -- an earlier draft of this
demo tried publishing from a background `threading.Thread` and hit a
real PyO3 panic (`assertion left == right failed: ... is unsendable,
but sent to another thread`). Fixed by keeping `publish()` calls on the
main thread and running the real, separate AT-SPI2 verifier as a
genuinely separate OS **process** instead (`subprocess.Popen`, real
concurrency at the OS level) -- which is exactly the shape a real
deployment needs anyway: the app publishes on its own thread, a real AT
queries it from its own separate process.

**A second real ordering bug found while building the demo**:
`renderer.render_canvas(canvas)` deliberately consumes the canvas's own
recorded content (so the same `Canvas` object can be reused next
frame) -- calling `canvas.accessibility_nodes()` *after* `render_canvas`
returned an empty list. Fixed by reading it before rendering, matching
the real pattern a caller must follow: tag nodes, read them, publish,
*then* render (or read them fresh each frame before that frame's own
render call).

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo build --workspace --all-targets`/
`cargo test --workspace` all clean in debug. `--release` clean apart
from the same 5 pre-existing, already-disclosed `debug_assert!`
failures. `demo/phase5_step5_3_3/run_canvas_accessibility_demo.sh`
(the original two-process demo) still passes unchanged -- a real
regression check.

`demo.py`, run via `maturin develop --release`: a real Vulkan render
confirmed at the tagged rect's own center pixel; `canvas.
accessibility_nodes()` reports the exact tagged bounds and role;
`tre.A11yBridge` connects in this same Vulkan-linked Python process;
and a genuinely separate verifier process, spawned mid-publish,
independently confirms the real, live AT-SPI2 `Component.GetExtents`
matches exactly -- the first time this project's own accessibility
work has been proven end to end through the actual Python binding.

**Real, disclosed remaining scope**: Linux only (matching every other
`tre-a11y` disclosure); no real focus tracking (the synthesized root
always reports focused); no tree hierarchy beyond the one synthesized
root; no incremental/diffed `TreeUpdate`s (`publish()` rebuilds the
full node list every call, matching `tre_a11y::A11yBridge`'s own
existing, unchanged scope). Windows UIA/macOS NSAccessibility remain
out of scope, as they have been since Phase 5.
