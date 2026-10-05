//! 0.5.4 (#106): how big a node is. `cargo test -p engine-core --release --test node_size -- --nocapture`
use engine_core::*;
use peniko::Color;
use std::mem::size_of;

#[test]
fn sizes() {
    println!("Node {}", size_of::<Node>());
    println!("PaintProperties {}", size_of::<PaintProperties>());
    println!("NodeKind {}", size_of::<NodeKind>());
    println!("taffy Style {}", size_of::<taffy::Style>());
    println!("Animated<f64> {}", size_of::<Animated<f64>>());
    println!("Animated<Color> {}", size_of::<Animated<Color>>());
    println!("TextState {}", size_of::<TextState>());
    println!("TextFieldState {}", size_of::<TextFieldState>());
    println!("VirtualListState {}", size_of::<VirtualListState>());
    println!("CanvasState {}", size_of::<CanvasState>());
    println!("ImageState {}", size_of::<ImageState>());
    println!("PathState {}", size_of::<PathState>());
    println!("TerminalState {}", size_of::<TerminalState>());
    println!("ScrollViewState {}", size_of::<ScrollViewState>());
    println!("AccessNodeData {}", size_of::<AccessNodeData>());
}
