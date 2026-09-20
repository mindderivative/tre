# LOG — M40: Smooth Window Resize (Coalesced Swapchain Reconfigure)

- User's own explicit instruction: "Start" continuing straight from
  M40's own scoping turn ("Yes scope window resizing, this one is
  really important"), the user's own prior context: "TRE v1 and
  pyCopper had some issues with window trailing behind cursor and
  stuttering. I believe v1 ended up implementing google chrome's
  method."
- Real winit investigation before design: direct source read of `winit
  = "0.30.13"`'s own public `WindowEvent` enum (`event.rs:152-438`)
  confirmed `Resized(PhysicalSize<u32>)` is the *only* public,
  cross-platform resize-related event -- no enter/exit-live-resize
  signal exists anywhere (the Windows-internal `WM_ENTERSIZEMOVE`/
  `WM_EXITSIZEMOVE` handling I'd read earlier only sets a private
  `MARKER_IN_SIZE_MOVE` flag, never surfaced to the app). This resolved
  the scoping note's own explicitly-left-open design question: a
  frame-count-based mechanism is the only portable option, not a
  platform-native "still dragging" event.
- **Real, load-bearing correction, made before writing a single line of
  production code -- the exact discipline this whole project has
  applied repeatedly to *other* people's prior notes, now applied to my
  own scoping note from one turn earlier.** The scoping section
  committed to porting both TRE v1's and pyCopper's own real technique:
  hold the swapchain at a coarse, oversized "bucketed" size during a
  drag, relying on the compositor to scale it down to fit. Before
  building that, I re-read pyCopper's own `_pin_surface` doc comment
  more carefully (`engine.py:292-313`) and found it says "content is
  drawn across the *whole* oversized buffer," while my own summary of
  TRE v1's report said content was "projected at the true logical
  size" -- two different, mutually exclusive mechanics. Rather than
  guess which (if either) applies to TRE v2's own architecture, built a
  real, throwaway `wgpu`+`winit` scratch probe (outside the repo
  entirely, in the session scratchpad) and ran it against this exact
  session's own real KWin/Wayland compositor (confirmed via `loginctl`/
  `ps`: `XDG_CURRENT_DESKTOP=KDE`, `kwin_wayland` running -- the same
  real compositor TRE v1's own report was grounded in). The probe: a
  fixed 300x300 window, a wgpu surface deliberately configured at
  600x600 (2x oversized), painting the whole buffer red except a
  scissor-restricted 100x100 blue marker in the top-left corner (1/6 of
  each buffer dimension). Screenshotted the real, composited result via
  `spectacle -b -a` and read the image directly. **Real, decisive
  result: the blue marker appeared at its own full, native 100x100
  size within the visible window -- not shrunk to ~1/6 as the
  scale-to-fit model would predict.** This compositor crops an
  oversized `wl_surface` buffer to the surface's own declared window
  geometry by default; it does not scale it. Both prior projects' own
  real, *working* implementations of this technique needed `wp_
  viewporter`-level explicit scaling support to get real scale-to-fit
  behavior -- TRE v1 via a real, confirmed winit fork (`WindowExtWayland
  ::set_viewport_source_crop`), pyCopper likely via something
  equivalent inside its own GLFW/rendercanvas stack, neither of which
  this milestone's own scoping had planned to build (the fork was
  already, separately, deferred as a follow-up). Had I built the
  originally-scoped design without this check, it would have shipped a
  real, visibly broken crop during every drag -- worse than the
  original bug, not a fix for it.
- A second real scratch probe (`configure_timing.rs`, same throwaway
  project) measured `wgpu::Surface::configure()`'s own real cost on
  this machine's actual adapter: **AMD Radeon 890M Graphics (RADV
  STRIX1), Vulkan backend -- ~600µs average, ~941µs max across 100 real
  reconfigure calls**, alternating sizes each time to force a genuine
  rebuild rather than a same-size no-op. This is neither TRE v1's own
  300-450ms figure (a different, Wayland-specific `vkAcquireNextImageKHR`
  compositor-blocking stall, not the `configure()` call itself) nor
  pyCopper's own 1.35-1.88ms (a different real backend) -- a real,
  machine-specific number, not assumed to transfer from either prior
  project.
- **Real, corrected design, informed by both findings:** since (a) the
  compositor-scaling trick doesn't work here without a fork this
  milestone already declined, and (b) the real reconfigure cost on this
  machine is modest (sub-millisecond, not hundreds of milliseconds),
  the simplest real fix is architectural, not a bucket/settle
  mechanism: **reconfigure the surface at most once per real frame,
  always at the true current size**, instead of inline on every raw
  `Resized` event. `InputEvent::Resized`'s handler (`app.rs`) now only
  updates the real, live `runtime.width`/`height` `SharedSize` cells
  (unchanged -- zero added lag, still immediately visible to
  Python-facing `PyWindow` fields per M33 Phase 2's own real sharing).
  New `GpuState::needs_resize(width, height) -> bool`, comparing against
  the surface's own currently-configured size. The per-frame
  `RedrawRequested` closure's existing `take_dirty()` early-return is
  widened to also check it -- a pending resize touches no `Tree` state
  at all (pure GPU/window sizing), so `take_dirty` alone would never
  see it and the frame would wrongly skip real work. The existing
  `GpuState::resize` is now called from exactly one place: right before
  acquiring the surface texture, once per frame, only when `needs_
  resize` is true. Any burst of `Resized` events arriving between two
  real frames -- exactly what a live drag produces -- now collapses
  into a single real reconfigure at whatever the window's true size is
  at that moment, eliminating the redundant per-event cost with zero
  compositor-scaling assumption and no fork. This turned out simpler
  than either prior project's own more elaborate mechanism, precisely
  *because* this machine's own real measured cost didn't demand more.
- Cleaned up both scratch probes (deleted, never committed, matching
  M34/M39 Phase 5's own identical "scratch, not committed" precedent)
  once their real findings were captured and written into the code's
  own doc comments.
- Full verification chain, all green: `cargo check --workspace --all-
  targets`; `cargo clippy --workspace --all-targets -- -D warnings`;
  `cargo fmt` + `cargo fmt --check`; `cargo test --workspace --release`
  (unchanged counts across every crate -- a pure internal render-loop
  refactor with no new pure-logic surface a headless unit test could
  exercise); `maturin develop --release` rebuilt; `pytest tests/` (581
  passed, 1 skipped, unchanged, including `test_resize.py`'s own 5
  tests -- confirmed these exercise the *synthetic*, `Tree::dispatch`-
  based `Window.resize()` path, not the real winit-driven one this
  phase touched, so their being unaffected is the expected, correct
  outcome, not a false negative); all 77 examples + showcase demo
  clean.
- `BUILD_TRACKER.md`: both phases flipped to done with the full real
  investigative story (the crop finding, the measured cost, the design
  correction) written directly into the milestone's own top-level
  section, not just the phase bullets; milestone status line and Top
  Metrics row both flipped to Complete; a new "Just closed" trailer
  added above the pre-existing M39 one. Parser re-confirmed balanced
  (40 milestones, 129 phases, 220 items, unchanged -- only status flips
  and prose edits, no new real line items); artifact regenerated and
  republished.
- **Real, honestly-stated limit, matching M33 Phase 2's own already-
  established precedent for this exact class of capability:** whether
  the fix actually *feels* smoother during a genuine, human-driven live
  OS resize drag cannot be verified from this environment at all -- no
  `xdotool`/`wmctrl`-equivalent window-manipulation tool is available
  to simulate one programmatically, and "feels smooth" is inherently a
  real-time, hands-on-the-mouse judgment no amount of code review or
  scripted measurement substitutes for. Left for the user to try
  directly against a real window (any example run with `app.run(max_
  frames=None)` instead of a bounded frame count) and report back.

**M40 — Smooth Window Resize (Coalesced Swapchain Reconfigure) — is
now fully complete, both phases.** This closes the milestone — per the
standing "push only after a full milestone closes" convention, a `git
push` is now appropriate.
