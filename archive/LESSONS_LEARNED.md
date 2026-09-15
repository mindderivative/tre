# Lessons Learned — TRE v0.1.0

A distillation for the next iteration, not a restatement of
`documentation/REVIEW.md`'s 261 findings. Organized by theme; each point
names the real finding(s) it comes from so the full account is one grep
away.

## 1. The core architectural bet was never actually tested

TRE built its own RHI trait abstraction (`RhiDevice`/`RhiSwapchain`/
`RhiCommandBuffer`/`RhiTexture`) specifically so `tre-engine` would stay
backend-agnostic, with `tre-rhi-dx12`/`tre-rhi-metal` reserved for
Windows/macOS. Those two crates stayed **empty stubs for the entire
project**. That means the abstraction's central promise — "swap backends
without touching the core" — was designed for but never once exercised.
An untested abstraction boundary is not a real boundary; it's a hope.

**For next time:** if cross-platform is a real goal, prove the seam
early with a second, even-toy backend (a software rasterizer counts)
before investing further in the first one. If cross-platform is *not*
actually a near-term goal, don't build the abstraction layer at all —
it adds indirection and constrains API choices (see §2) for a promise
that was never going to be cashed in this project's lifetime anyway.

**The harder, more honest version of this question** — raw Vulkan vs.
wgpu — was asked explicitly near the end of this project (see the
session that produced finding #260/#261) and the answer was genuinely
contested, not a clear win either way: wgpu would have delivered
Windows/macOS/Web today instead of as stubs, and its Arc-based resource
model would have prevented the exact use-after-free class in §3 by
construction. Raw Vulkan bought two real things wgpu can't easily match:
single-pass framebuffer-fetch blend modes (`VK_KHR_dynamic_rendering_
local_read`) and fully hand-tuned always-resident bindless textures at
the zero-allocation frame budget this project chased. **Decide this
trade-off explicitly, in writing, before writing the first render call**
— don't let it be decided implicitly by which stub crates never get
filled in.

## 2. Raw GPU resource lifetime management is a recurring, expensive bug class

Finding #259: a `VulkanTexture` frees its GPU image through a *cloned*
`ash::Device` handle in `Drop`. That handle dangles the instant the real
`VulkanDevice` calls `vkDestroyDevice`. Any texture that outlives its
device — trivially possible once textures are handed across a language
boundary (Python) where the garbage collector, not the programmer,
decides finalization order — is a use-after-free. It was invisible on
the real GPU driver (RADV silently tolerated it) and only surfaced as
heap corruption on the software driver CI actually uses (lavapipe).
Three findings (#258, #259, and the CI diagnosis work in #254/#257) went
into finding and fixing this one bug class.

**For next time:** if the architecture holds raw native handles with
manual teardown ordering (GPU resources, file descriptors, anything with
a C-style "destroy the parent last" contract), and any of those handles
can cross a boundary with automatic memory management on the other side
(Python, JS, a GC'd host language), **make the lifetime dependency
structural from day one** — refcount the owning resource (`Arc`) so
teardown order is enforced by the type system, not by convention and
comments. Retrofitting this (as this project had to) means finding every
call site that assumed manual ordering was safe.

## 3. A validation layer is not a substitute for testing on the actual target driver

Multiple long investigations (findings #206–208, #217, #223, #226, #237)
trace back to the same root cause: CI ran on a hosted runner with no
real GPU, using a software driver (lavapipe) with its own validation
layer quirks, and the gap between "works on the developer's real GPU"
and "works in CI" repeatedly cost multiple round trips to close. The
worst instance: a validation-layer message that looked like a real
error was actually a known false positive specific to running the
`VK_KHR_dynamic_rendering_local_read` extension probe under lavapipe,
and distinguishing "real error, abort" from "known false positive,
continue" needed its own hand-written message-parsing logic
(`is_local_read_rejected_by_layer`/`is_known_false_positive`).

**For next time:** get the *exact* CI driver stack (software ICD,
validation layer version, virtual display) running locally, on day one,
not discovered iteratively through failed CI runs. A local `xvfb-run` +
`VK_DRIVER_FILES=<software-icd>` rehearsal loop (eventually built in
this project, used heavily for findings #254–#261) should be the
*starting* tool, not something assembled under pressure two-thirds of
the way through.

## 4. A binding surface with no CI coverage will drift silently

The Python bindings (`tre-python`, arguably the project's actual
GUI-framework-facing deliverable) had **zero CI coverage** until finding
#254 — three-quarters of the way through the project's life. Every
prior dependency bump, every Rust-side refactor, was verified against
Python demos *manually, locally*. When CI coverage was finally added,
the very first real run found three genuine bugs that had been sitting
undetected: a wheel-packaging default that silently duplicated system
GUI libraries and crashed most Vulkan operations, a demo that hardcoded
a core count the CI runner didn't have, and the use-after-free from §2.
All three were real, all three had presumably been latent for a while.

**For next time:** the binding layer that end users actually touch
needs CI from the moment it exists, not after the "real" (Rust/native)
layer is judged stable. "It works when I run it locally" is not
evidence for a binding that gets built with a different toolchain, on a
different OS, by a packaging tool (here: `maturin`/`auditwheel`) whose
default behavior (vendoring system libraries into the wheel) was never
actually inspected until it broke something.

## 5. Pinned constraints need an enforcing CI job, or they silently become fiction

`rust-version = "1.75"` was declared and cited as the reason six
dependencies were frozen at old versions — and was **false** for an
unknown period before finding #248 caught it (`cargo +1.75.0 check`
failed immediately; the real floor was 1.88). No CI job ever built at
the declared floor; every job used the pinned *development* toolchain
(1.98.0), which hid the gap completely. The same pattern showed up with
GitHub Actions pinned by floating major tag (a real, if smaller,
supply-chain gap, finding #256) — a constraint stated in a comment but
never mechanically checked.

**For next time:** any claim of the form "we require X" (a language
version, a dependency floor, an extension being optional) needs a CI job
that actually tests the boundary condition, added in the same commit
that makes the claim. A comment is not enforcement.

## 6. An append-only findings ledger is a good audit trail and a bad working list

`documentation/REVIEW.md` grew to 261 numbered findings and roughly
4,000 lines. This was genuinely valuable as a *why did we do this*
reference (it's the first thing this document points at) — but by the
end, loading enough of it to know what was still open required reading
a long top-of-file preamble that itself needed periodic re-summarizing
just to stay honest, and finding "what's still actually unresolved" took
real digging.

**For next time:** separate the **audit trail** (append-only, exactly
what this project has) from the **live worklist** (a short, always-
current file listing only open items, each linking to its full audit
entry once resolved and removed from the worklist). Keep both; don't let
one file try to be both a history book and a to-do list.

## 7. Depth was chased hard in one dimension while basics lagged in another

Genuinely excellent engineering went into things a typical 2D renderer
never bothers with: a real, mechanically-enforced zero-heap-allocation
steady-state render loop (finding-driven, `tre_memory::DebugAllocGuard`),
bindless textures with runtime-clamped capacity, single-pass hardware
blend-mode compositing, MSDF text with bidi/script shaping and font
fallback, lock-free SWMR/MPSC primitives with real concurrent stress
tests. At the same time: no Windows/macOS support (stub crates), no
Python CI until very late, an MSRV claim that was quietly false, GitHub
Actions pinned insecurely, and a security review of the one
user-facing-untrusted-code surface (custom shaders) that didn't happen
until explicitly requested near the end (findings #247(b)/#260).

**For next time:** the project's own stated identity ("to support GUI
frameworks," implying breadth) and its actual engineering effort
(overwhelmingly single-platform depth) diverged without anyone deciding
that on purpose. Write the actual target platform list and the actual
performance budget down together, early, and revisit both explicitly
before doubling down on either axis — don't let architecture stub crates
quietly become the record of a scope decision nobody made deliberately.

## 8. Security review of a "power user" surface (custom shaders) should not be an afterthought

Custom, caller-supplied GLSL compiled at runtime and linked against the
engine's shared bindless resources (finding #247 item b, closed as
finding #260) is exactly the kind of feature that looks like "advanced
API for power users" and is actually "arbitrary code with GPU memory
access, from whatever calls this API." It went unreviewed from a
security standpoint until a dedicated pass asked for it explicitly.

**For next time:** any surface that compiles or loads caller-supplied
code (shaders, plugins, scripts) gets a security pass at the time it's
built, with the same seriousness as a network-facing input-parsing
surface — not deferred to "we'll harden it later" as a separate,
optional finding.

## What worked and is worth keeping

- **The phase-by-phase plan/log/demo discipline** (a `PLAN_PHASEn_
  STEPn.md` + `LOG_PHASEn_STEPn.md` + a runnable, self-asserting
  `demo/phaseN_stepN/demo.py` for every real step) produced a genuinely
  complete, checkable record of what was built and why, and made the
  eventual CI-coverage retrofit (finding #254) tractable specifically
  *because* every demo already asserted its own correctness.
- **Cross-checking new findings against the existing ledger before
  reporting them** (every multi-lens review pass this project ran was
  instructed to grep prior findings first) kept the signal-to-noise
  ratio of later review passes high instead of re-litigating settled
  questions.
- **Exact-pin-with-a-recorded-reason** for version-sensitive
  dependencies (`cargo add --dry-run` confirmed, comment left in place)
  is a good pattern *when the reason is re-verified whenever the
  constraint that produced it changes* — see §5 for what happens when
  it isn't.
- **Rehearsing CI's exact environment locally before pushing** (built
  out of necessity in this project's final phase) turned several
  would-be multi-round-trip CI failures into single-shot fixes once it
  existed. Build this tooling on day one of the next project, not the
  last week.
