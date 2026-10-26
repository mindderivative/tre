# Why a small window holds about 210 MB of video memory

Research for issue #207, 2026-10-25. The question: a 200 x 100 window draws one coloured box, and
the process holds about 210 MB of VRAM (and 73 MB of GTT) before it does anything else. Why?

**The short answer: tre itself uses under 1 MiB for that window. The rest is `wgpu`'s memory
allocator reserving big blocks when each window creates its GPU device.** One setting
(`MemoryHints::MemoryUsage`, which tre does not set) brings the same window to 31 MiB, with no
measurable cost in frame time.

## Hardware and method

One machine: an AMD Radeon 890M (an integrated GPU, RADV, Mesa 26.2.5), `wgpu` 30.0.1 on Vulkan,
hosted on a headless sway (wlroots) and, for the first reading, KDE's KWin. The numbers are per
process, from `/proc/<pid>/fdinfo` (`drm-total-vram`, `drm-total-gtt`, once per DRM client), so the
compositor and other applications do not disturb them; the system-wide `mem_info_vram_used` is
only a cross-check. `tools/vram_probe.py` reproduces the first table. Not measured: a discrete
GPU, NVIDIA, Intel, Windows (DX12), macOS (Metal).

## What a window holds

| Scenario (as shipped) | VRAM | GTT |
|---|---|---|
| one 200 x 100 window | 211 MiB | 73 MiB |
| two such windows (one process) | 404 MiB | 143 MiB |
| a 1280 x 32 bar (`LayerWindow`) | 198 MiB | 72 MiB |
| a full-screen overlay at 1280 x 720 | 211 MiB | 72 MiB |
| a full-screen overlay at 3840 x 2160 | 325 MiB | 72 MiB |

Everything is allocated when the window opens and stays flat: no growth over five seconds, none
across 80 resizes. Each window adds about 193 MiB: it is a per-window fixed cost, not a per-pixel
one.

## Where it goes

A stand-alone probe (a throwaway program, reverted) walks the same steps a window takes and reads
both the process's memory and `wgpu`'s own allocator report after each:

| Step | VRAM | `wgpu` used | `wgpu` reserved |
|---|---|---|---|
| `Instance::default()` | 2.8 MiB | | |
| `request_adapter` | 2.8 MiB | | |
| **`request_device`** | **195.0 MiB** | 0.5 MiB | **192 MiB** (blocks of 128 + 64) |
| `GpuTimer::new` | 195.0 MiB (GTT 5 to 69 MiB) | 0.5 MiB | 256 MiB (+ a 64 MiB host block) |
| `WindowRenderer::new` 200 x 100 | 195.1 MiB | 0.9 MiB | 256 MiB |
| one frame rendered | 195.8 MiB | 0.9 MiB | 256 MiB |

**Everything tre allocates for that window is under 1 MiB** (the allocator's own list: a 0.5 MiB
zero-initialisation buffer that `wgpu` makes itself, the 0.17 MiB persistent render target, and
about 0.2 MiB of `vello_gpu` textures; the largest of the rest is 64 KiB). The 192 MiB
appears the moment the device exists. `wgpu`'s allocator, asked for hints with the default
`MemoryHints::Performance`, creates device memory blocks of at least **128 MiB** (host-visible
ones at least 64 MiB) the first time anything needs a memory type, whatever is then placed in
them (`wgpu-hal` 30.0.1, `AllocationSizes::from_memory_hints`: Performance 128 to 256 MiB device and
64 to 128 MiB host blocks; `MemoryUsage` 8 to 64 MiB device and 4 to 32 MiB host). `GpuTimer`'s
small read-back buffers then pull a 64 MiB host block into GTT.

tre makes one `Instance`, adapter and device **per window** (`GpuState::new`), so every window pays
this again.

## The same windows with `MemoryHints::MemoryUsage`

A throwaway switch in `GpuState::new` (reverted) asked for the small blocks:

| Scenario | VRAM, as shipped to with the hint | GTT, as shipped to with the hint |
|---|---|---|
| one 200 x 100 window | 211 to **31** MiB | 73 to 13 MiB |
| two windows | 404 to **44** MiB | 143 to 24 MiB |
| a 1280 x 32 bar | 198 to **18** MiB | 72 to 12 MiB |
| full-screen overlay, 720p | 211 to 31 MiB | 72 to 12 MiB |
| full-screen overlay, 4K | 325 to **177** MiB | 72 to 12 MiB |

- **Frame cost: none measurable.** A scene of 900 rounded boxes with text and animations, full
  window, three runs per setting: total frame time mean 15.2 to 15.3 ms either way, p95 16.2 to
  16.5 ms, CPU 2.85 to 3.0 ms, draw about 2.0 to 2.1 ms (most of the frame is the wait for the
  display). One 27 ms outlier appeared in each setting.
- **Real needs are still met.** A 4096 x 4096 image (64 MiB) adds 64 MiB in both settings (263 to
  83 MiB with the hint, so the 180 MiB gap is unchanged): large resources get the memory they
  ask for.
- **A resize storm** (80 resizes between 300 x 200 and 1000 x 700): flat at about 198 MiB as shipped;
  with the hint 17 to 18 MiB, then one step to 36 MiB at the last, largest size (a new block), so
  the memory grows in small steps instead of one reservation. Whether it ever shrinks was not
  measured.

## The part that scales with pixels

What is left at 4K (177 MiB) is the surface: the probe's allocator shows one big thing, tre's
persistent render target (32 MiB at 3840 x 2160, 4 bytes a pixel), and the rest of the gap to the
real process (about 129 MiB) is the swapchain: the driver gives four images of 32 MiB. tre cannot
choose that count: `desired_maximum_frame_latency` set to 1, 2 or 3 and the present modes `vsync`
and `low_latency` all gave exactly 176.6 MiB. So a full-screen surface costs about 19 bytes per
pixel on this driver (four swapchain images plus the persistent target), which is why the overlay
menu pattern creates its surface only while a menu is open.

## Ranked findings

Filed as Backlog issues; nothing is built until approved.

1. **Ask `wgpu` for small memory blocks** (`MemoryHints::MemoryUsage`, or `Manual` with chosen
   sizes). Saves about 180 MiB of VRAM and 60 MiB of GTT per window here; the frame-time cost
   measured is nil. One line, plus a way to choose (an environment variable or a setting) for
   anyone who wants the old reservation. Effort: very small. Risk: low on what was measured;
   unmeasured on other GPUs and backends (the same block sizes are used on DX12; Metal ignores the
   hint), and on a discrete GPU the host-visible blocks are in system memory, so the VRAM saving
   there is about 128 MiB, not 180 (inferred from the allocator's behaviour, not measured).
2. **Share one device across windows.** *Done (0.5.9, #220).* After finding 1 a further window still
   cost about 13 MiB of VRAM and 11 MiB of GTT, plus the time to create an instance and device. Now
   the first window's instance, adapter, device and queue are shared (`gpu_share.rs`), the device's
   lost and error handlers are installed once with a watch per window (`GpuHub`), and a window the
   shared adapter cannot present to gets a device of its own. Measured on the same machine
   (`tools/vram_probe.py`, AMD RADV, a private headless sway): 1 window 31.0 MiB VRAM / 13.4 GTT,
   2 windows 31.0 / 14.8, 5 windows 31.0 / 16.0 (before: 44.1 / 24.2 and 83.2 / 51.2).
   `TRE_GPU_DEVICES=per-window` reproduces the old numbers exactly. Time until five windows have
   each drawn a frame: about 285 ms against 334 ms (three runs each), so roughly 12 ms a window;
   the rest of a window's start-up is its own renderer (pipelines and caches), which is not shared.
   That is the next step if many windows matter: share the `vello_gpu` pipelines between windows.
3. **Cut the per-pixel cost of full-screen surfaces** (4K: about 160 MiB). Options: skip the
   persistent target for a surface that never redraws partially (saves 32 MiB at 4K); keep
   overlay surfaces menu-sized where an input grab does not need the whole output. Effort and risk
   vary with the option; needs a design.
4. **Show GPU memory to the developer.** `frame_stats()` could include the allocator's used and
   reserved totals, which would have found this at once. Effort: small. A feature, not a fix.
5. **Measure other hardware** (NVIDIA, Intel, a discrete AMD GPU, Windows, macOS): folded into the
   existing hardware issues, #187 and #188, with this probe.

## What was done afterwards (#219)

Finding 1 is built: tre asks for small memory blocks by default (`gpu_memory.rs`), with
`TRE_GPU_MEMORY=performance` as the escape hatch, at both device sites (a window's device and the
headless snapshot device). A live test reads a window's own video memory from `fdinfo` and checks it
stays under 100 MiB and that the variable brings the large reservation back. Findings 2 to 5 stay
in the Backlog (#220, #221, #222, and the hardware issues #187 and #188).

## What was done afterwards (#221)

The persistent render target is made only for a window that redraws partially. A window opened
with `partial_redraw=False` keeps no copy of its last frame: measured on a 2400 x 1400 window,
54.4 MiB with the copy and 38.4 MiB without (16 MiB less, a little over the 12.8 MiB of pixels).
`set(partial_redraw=...)` works while the window is open too: on makes the copy and draws that
frame in full, off drops it (`WindowRenderer::set_persistent`, pixel-checked in
`partial_redraw.rs`). Dropping it live did not lower the process's video memory on the AMD
driver tested in four seconds, even with the device polled, so the saving is reliable only for a
window opened without it; why the driver keeps it is not known. The other option in the issue
(a menu-sized surface instead of a full-screen overlay) is `PopupWindow` (#223). The four
swapchain images (about 16 bytes a pixel at 4K) stay out of tre's reach.

## What was changed and reverted

Two throwaway edits in `GpuState::new` (a memory-hint switch and a swapchain-latency switch) and
a stand-alone probe program were used and **reverted**; the engine code on this branch is
unchanged. Kept: `tools/vram_probe.py` and this page.
