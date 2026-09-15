//! Phase 9 Step 9.2: TECHNICAL.md Section 9.2's own "cargo bench via
//! criterion, verifying the <=0.50ms CPU frame processing budget" gate
//! -- never previously built (no `criterion` dependency, no `benches/`
//! anywhere in this workspace before this step). A minimal, real gate:
//! one representative frame's worth of real work (recording, radix
//! sort/batch), at the Architectural Decision Matrix's own stated
//! ">10,000 active nodes" scale, benchmarked end to end. `ci.yml` parses
//! criterion's own reported per-iteration time and fails the build if it
//! exceeds the documented budget -- a real, if minimal, hard gate, not a
//! number nobody checks.
//!
//! Deliberately narrow: this is not the full per-demo-scene benchmark
//! suite TECHNICAL.md Section 9.2 itself separately owns -- one
//! representative scene shape is what this step's own task list names.
//!
//! REVIEW.md finding #224: this benchmark originally built a fresh
//! `RenderingCanvas::new()` and called the consuming `RenderingCanvas::
//! flatten(self)` inside every `b.iter()` closure, so every sample paid
//! full cold-allocation cost (`vertices`/`indices`/`commands` growing
//! from empty up to this scene's real size, plus `flatten`'s own
//! `segment_and_flatten` allocating fresh sort scratch every call) on
//! top of the actual per-frame processing work TECHNICAL.md Section
//! 9.2's budget is about -- the opposite of this project's own
//! established, documented zero-allocation-reuse convention. A real
//! running application never rebuilds these buffers every frame, so
//! measuring cold-start allocation on every sample was never actually
//! measuring the steady-state cost the budget describes. Rewritten to
//! build the canvas/output once, outside the timed loop, and call
//! `reset()` then the new non-consuming `RenderingCanvas::flatten_into`
//! (this same finding: a real, reusable single-canvas sibling to
//! `flatten`, avoiding `FrameArena`'s own copy overhead that only
//! benefits *multi*-canvas stitching) per iteration -- the real
//! steady-state cost of one canvas's own per-frame work, with nothing
//! left to combine.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tre_engine::{rgba8, FlattenedFrame, RenderingCanvas};

/// ARCHITECTURE.md's own Architectural Decision Matrix: "Guarantees
/// deterministic sub-millisecond sorting times even when UI trees
/// contain over 10,000 active nodes" -- the real scale this budget is
/// specified against, not an arbitrary round number.
const NODE_COUNT: usize = 10_000;

fn record_and_flatten_10k_nodes(c: &mut Criterion) {
    let mut canvas = RenderingCanvas::new();
    let mut flattened = FlattenedFrame::default();

    c.bench_function("record_and_flatten_10k_nodes", |b| {
        b.iter(|| {
            canvas.reset();
            for i in 0..NODE_COUNT {
                let (col, row) = (i % 100, i / 100);
                #[allow(
                    clippy::cast_precision_loss,
                    reason = "NODE_COUNT is a small compile-time constant, far below f32's \
                               exact-integer range"
                )]
                let x = col as f32 * 8.0;
                #[allow(
                    clippy::cast_precision_loss,
                    reason = "same reasoning as the col->x conversion above"
                )]
                let y = row as f32 * 8.0;
                canvas.draw_rounded_rect(x, y, 6.0, 6.0, 0.0, rgba8(255, 255, 255, 255));
            }
            canvas.flatten_into(&mut flattened);
            black_box(flattened.commands.len());
        });
    });
}

criterion_group!(benches, record_and_flatten_10k_nodes);
criterion_main!(benches);
