# Demo: Phase 1, Step 2 -- Decoupled Event & Signal Pipeline

```bash
./demo/phase1_step2/run_input_demo.sh
```

Opens **two** native windows -- A (amber) and B (blue) -- on **one shared `PlatformConnection`** (Wayland or X11, whichever this session uses), the same connection-consolidation this step introduced. Move the mouse, click, and press keys in each window; every translated event prints to the terminal tagged with the window it came from:

```
[A] pointer moved to (240.0, 180.0)
[A] pointer button Left Pressed
[A] pointer button Left Released
[A] key 30 Pressed
[A] key 30 Released
[B] pointer moved to (200.0, 159.0)
...
```

This is the demo Step 1.1's couldn't run at all: `walking_skeleton`, `multi_window`, and `headless` proved windowing and rendering, but none of them could react to input. This one proves the real pipeline IMPLEMENTATION.md Step 1.2 and TECHNICAL.md Section 8 describe -- OS pointer/keyboard events translated into `tre_engine::InputEvent`s, coalesced (a burst of mouse motion collapses to the latest position -- move the mouse quickly and you'll see far fewer `pointer moved` lines than actual physical motion), and drained non-blockingly once per frame -- and that events are routed to the **correct window** when more than one is open (move/click in A, then in B, and the `[A]`/`[B]` tags should never cross).

Close both windows (or wait for the frame budget, `${TRE_INPUT_DEMO_FRAMES:-600}` frames by default) to exit.

**Note on window placement:** like Step 1.1's `multi_window` demo, the two windows may open stacked exactly on top of each other (neither Wayland nor, in practice, this session's X11/KWin window management gives a client control over its initial position) -- drag one aside if you want to see both at once. This does not affect the proof: `[A]`/`[B]` tagging in the terminal output is the actual routing check, not visual window position.

**Verify with the Vulkan validation layer:**
```bash
VK_LOADER_LAYERS_ENABLE=VK_LAYER_KHRONOS_validation cargo run -p tre-rhi-vulkan --example input_demo
```

**What's actually new under the hood** (see `documentation/REVIEW.md`'s "Phase 1 Step 2 Implementation" section and `LOG.md` for the full detail):
- `tre-platform` consolidated from one connection per window (Step 1.1) to one shared `PlatformConnection` per backend, owning multiple windows via an opaque `WindowId`.
- A real, generically-typed `tre_memory::SpscRingBuffer<T>` -- fixed-capacity, genuinely atomic/lock-free, allocated once at construction.
- `tre_engine::InputEventQueue`, wrapping that ring buffer with pointer-move coalescing that stays safe even once a real second consumer thread is introduced (the pending value is staged in producer-exclusive memory, never mutated after publication).
- Pointer/keyboard binding on both backends: Wayland via `wl_seat` -> `wl_pointer`/`wl_keyboard`; X11 via the window's extended event mask (`ButtonPress`/`ButtonRelease`/`PointerMotion`/`KeyPress`/`KeyRelease`).
