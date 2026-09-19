# PLAN — M38 Phase 7: Real Scroll+Clip for Code Editor (closes M38)

## Goal
Give Code Editor (a plain, fixed-box, multiline `TextField`) real
vertical scroll+clip for content taller than its own box, with the
caret auto-scrolling into view as it moves -- the last of the seven
gaps M38's own investigation found, and the one its own scoping note
flagged as most likely to need a design pause.

## Steps
1. Investigated the current state: `add_code_editor` is a plain
   `NodeKind::TextField`, fixed `width`/`height`, no clip, no scroll
   offset at all -- overflowing content simply painted past the box
   (confirmed by direct source read: the `NodeKind::TextField` paint
   arm had zero clip logic anywhere).
2. Paused via `AskUserQuestion`: the milestone's own original scoping
   note preferred composing the already-built `ScrollView` around
   `TextField`, but that needs `TextField` to report a real, unbounded
   intrinsic content height to `taffy` for `ScrollView`'s own child-
   measurement to work -- a real `taffy` measure-function integration,
   confirmed via direct grep to have zero precedent anywhere in this
   codebase. User chose a dedicated `TextFieldState.scroll_offset`
   mechanism instead: self-contained, no `taffy` changes, mirroring
   `VirtualList`/`Carousel`'s own established "per-`NodeKind` scroll,
   not `ScrollView` reuse" pattern.
3. New `TextFieldState.scroll_offset: Animated<f64>` (`node.rs`) --
   real vertical pixel scroll, driven directly (not through
   `animate_field`), the identical precedent `ScrollViewState.scroll`/
   `VirtualListState.scroll_offset` already establish. `TextFieldState`
   lost its `Clone`/`Debug`/`PartialEq` derive once it gained a real
   `Animated<f64>` field (the same constraint every other `Animated<T>`
   -holding state struct in this codebase already has) -- this broke
   one real downstream `.clone()` in `engine-py::app.rs`'s own `text_
   field_hit_offset` click-to-position helper, fixed by holding the
   `RefCell` borrow for the whole helper instead of cloning state out
   of it.
4. New `Tree::scroll_text_field_caret_into_view` (`tree.rs`) -- real
   caret-follow: if the caret's own real line would sit outside the
   current viewport, scrolls just enough to reveal it. Uses a real,
   cited line-height approximation (VS Code's own real default,
   `fontSize * 1.35`, non-macOS) since `engine-core` has no font-
   shaping access to measure an exact value (§4) -- a real, honest
   heuristic imprecision, not a correctness bug, since the same
   `scroll_offset` value drives both the real clip and the real glyph
   shift at paint time, keeping the rendered result internally
   consistent regardless of how precisely the heuristic guessed.
   Wired into every real cursor-moving dispatch chokepoint: the
   `dispatch_text_field_key` caller site, `set_text_field_cursor`,
   `extend_text_field_selection`, and the `TextInput` dispatch arm --
   the same four sites Phase 2/3's own `goal_column` reset already
   used, reused here for the identical reason (guaranteed to run
   regardless of which of `dispatch_text_field_key`'s own ~15 early-
   return arms fired).
5. Real clip + scroll wired into `engine-render`'s own `NodeKind::
   TextField` paint arm: a real `push_layer`/`pop_layer` clip when
   `multiline`, `TextPlacement.y = -scroll_offset.current`. **Real,
   previously-uncached box fill found and fixed along the way:** direct
   source read found `TextField`'s own box fill was *still* a fresh,
   uncached `RoundedRect::to_path` every frame, despite M38 Phase 1's
   own completion note claiming it had already been fixed -- a real,
   honest correction of a prior write-up (caught by checking the
   actual current source before writing this phase's own completion
   note, not by trusting the earlier one). Fixed to route through
   `GeometryCache::rounded_rect_fill`, reused directly for the clip.
6. Real tests: three new `tree.rs` unit tests for `scroll_text_field_
   caret_into_view` (scroll-down math, scroll-back-up math, single-
   line true-no-op), all hand-verified against the real 1.35 ratio
   before running. Two new pixel-level tests in `crates/engine-render/
   tests/text_field_paint.rs` (a genuinely overflowing field's own
   bottom edge shows only the plain fill color, proving real clip; a
   nonzero `scroll_offset` paints genuinely different pixels than
   unscrolled). One new pytest test exercising the real, full FFI
   surface with genuinely overflowing content -- no Python getter
   exists for `scroll_offset` itself, the same verification-surface
   limit already established repeatedly this milestone.
7. Corrected four stale doc comments claiming this gap was still open:
   `add_code_editor`'s own Rust doc comment and its `python/tre/
   _core.pyi` stub, `TextFieldState.scroll_offset`'s own new doc
   comment states the real design directly.
8. Full verification chain: `cargo check`/`clippy -D warnings`/`fmt`/
   `test --workspace --release`, `maturin develop --release`, full
   `pytest tests/`, all 75 examples, showcase demo.
9. `BUILD_TRACKER.md` Phase 7 flipped to done, **closing M38 entirely
   (all 7 phases)** -- milestone status line, Top Metrics row, and a
   full "Just closed" trailer all updated; artifact regenerated
   (38/122/212, unchanged) and republished.

## Status
Complete. Full verification chain green (`cargo test --workspace
--release`: `engine-core` 196 passed (+3), `engine-render`'s own
`text_field_paint` suite 15 passed (+2); `pytest tests/`: 563 passed/1
skipped, up from 562, +1 new test; all 75 examples + showcase demo
clean). **M38 Phase 7 -- Real Scroll+Clip for Code Editor is now
complete. M38 -- Hardening II: Closing the Remaining Stated v1 Gaps is
now complete, all 7 phases.**
