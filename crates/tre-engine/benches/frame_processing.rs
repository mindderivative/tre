//! Phase 9 Step 9.2: TECHNICAL.md Section 9.2's own "cargo bench via
//! criterion, verifying the <=0.50ms CPU frame processing budget" gate
//! -- never previously built (no `criterion` dependency, no `benches/`
//! anywhere in this workspace before this step). A minimal, real gate:
//! one representative frame's worth of real work (recording, radix
//! sort/batch), at the Architectural Decision Matrix's own stated
//! ">10,000 active nodes" scale, benchmarked end to end via
//! `RenderingCanvas::flatten()`. `ci.yml` parses criterion's own
//! reported per-iteration time and fails the build if it exceeds the
//! documented budget -- a real, if minimal, hard gate, not a number
//! nobody checks.
//!
//! Deliberately narrow: this is not the full per-demo-scene benchmark
//! suite TECHNICAL.md Section 9.2 itself separately owns -- one
//! representative scene shape is what this step's own task list names.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tre_engine::{rgba8, RenderingCanvas};

/// ARCHITECTURE.md's own Architectural Decision Matrix: "Guarantees
/// deterministic sub-millisecond sorting times even when UI trees
/// contain over 10,000 active nodes" -- the real scale this budget is
/// specified against, not an arbitrary round number.
const NODE_COUNT: usize = 10_000;

fn record_and_flatten_10k_nodes(c: &mut Criterion) {
    c.bench_function("record_and_flatten_10k_nodes", |b| {
        b.iter(|| {
            let mut canvas = RenderingCanvas::new();
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
            black_box(canvas.flatten())
        });
    });
}

criterion_group!(benches, record_and_flatten_10k_nodes);
criterion_main!(benches);
