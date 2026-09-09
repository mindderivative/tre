//! Publishes `tre-engine`'s tagged accessibility nodes to the real Linux
//! AT-SPI2 accessibility bus, via `accesskit`/`accesskit_unix`
//! (IMPLEMENTATION.md Step 5.3.2). Decoupled from rendering (DESIGN.md
//! Section 5's own framing) -- this crate has no dependency on any RHI
//! backend and no rendering-loop hook of its own.
#![forbid(unsafe_code)]

use std::sync::{Arc, Mutex};

use accesskit::{
    ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, Node, NodeId, Rect, Role,
    Tree, TreeUpdate,
};
use tre_engine::{AccessibilityNode, AccessibilityNodeId, AccessibilityRole};

/// Reserved root `NodeId`. `accesskit::TreeUpdate` requires exactly one
/// root, but Step 5.3.1's tagged nodes are a flat list with no such
/// concept -- every published frame's nodes become children of this one,
/// engine-external, synthesized root. `AccessibilityNodeId(u64::MAX)`
/// colliding with this sentinel is a real, disclosed constraint
/// (PLAN_PHASE5_STEP5_3_2.md), not a silently-assumed impossibility.
const ROOT_ID: NodeId = NodeId(u64::MAX);

fn map_role(role: AccessibilityRole) -> Role {
    match role {
        // NOT `Role::GenericContainer`: `accesskit_consumer::common_filter`
        // (used by `accesskit_atspi_common`) hard-codes `GenericContainer`
        // as always excluded from the platform tree entirely -- its real
        // semantics are ARIA's `role="none"`/`"presentation"` (hide this
        // from assistive technology), the opposite of what a caller
        // tagging a real, generic element wants. `Role::Unknown` (this
        // enum's own `#[default]`) is the correct real match for "a
        // taggable element with no more specific role" -- confirmed via
        // `accesskit_consumer`'s own filter source, which excludes only
        // `GenericContainer`/`TextRun` and includes everything else.
        // Found by Step 5.3.3's own real, live end-to-end query: a
        // `Generic`-tagged node was silently absent from every AT-SPI2
        // query, a real regression since this crate first shipped.
        AccessibilityRole::Generic => Role::Unknown,
        AccessibilityRole::Button => Role::Button,
        AccessibilityRole::TextLabel => Role::Label,
        AccessibilityRole::Image => Role::Image,
    }
}

fn node_id(id: AccessibilityNodeId) -> NodeId {
    NodeId(id.0)
}

fn bounds_rect(node: &AccessibilityNode) -> Rect {
    Rect {
        x0: f64::from(node.x),
        y0: f64::from(node.y),
        x1: f64::from(node.x + node.width),
        y1: f64::from(node.y + node.height),
    }
}

/// The real axis-aligned union of every published node's bounds, so the
/// synthesized root itself reports a real, always-correct
/// `Component.GetExtents` rather than nothing at all. `None` when no
/// nodes are published yet -- a real absence, not a placeholder zero
/// rect.
fn union_bounds(nodes: &[AccessibilityNode]) -> Option<Rect> {
    nodes.iter().map(bounds_rect).reduce(|a, b| Rect {
        x0: a.x0.min(b.x0),
        y0: a.y0.min(b.y0),
        x1: a.x1.max(b.x1),
        y1: a.y1.max(b.y1),
    })
}

fn to_accesskit_node(node: &AccessibilityNode) -> Node {
    let mut out = Node::new(map_role(node.role));
    out.set_bounds(bounds_rect(node));
    out
}

/// Builds the full `TreeUpdate` for the current set of published nodes.
/// Every call rebuilds the full list rather than diffing against the
/// previous one -- `accesskit`'s own docs note incremental updates are a
/// performance nicety, not a correctness requirement, and per-node
/// diffing is explicitly out of scope for this sub-step
/// (PLAN_PHASE5_STEP5_3_2.md).
fn build_tree_update(tree: &Tree, nodes: &[AccessibilityNode]) -> TreeUpdate {
    let mut root = Node::new(Role::Window);
    if let Some(bounds) = union_bounds(nodes) {
        root.set_bounds(bounds);
    }
    root.set_children(nodes.iter().map(|n| node_id(n.node_id)).collect::<Vec<_>>());

    let mut entries: Vec<(NodeId, Node)> = Vec::with_capacity(nodes.len() + 1);
    entries.push((ROOT_ID, root));
    entries.extend(
        nodes
            .iter()
            .map(|n| (node_id(n.node_id), to_accesskit_node(n))),
    );

    TreeUpdate {
        nodes: entries,
        tree: Some(tree.clone()),
        // No real focus tracking this sub-step (a disclosed
        // simplification, PLAN_PHASE5_STEP5_3_2.md) -- the root itself
        // is reported focused, matching `TreeUpdate::focus`'s own
        // documented fallback ("if no specific node ... has keyboard
        // focus, this must be set to the root").
        focus: ROOT_ID,
    }
}

struct SharedState {
    nodes: Mutex<Vec<AccessibilityNode>>,
    tree: Tree,
}

struct Handler(Arc<SharedState>);

impl ActivationHandler for Handler {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        let nodes = self.0.nodes.lock().unwrap();
        Some(build_tree_update(&self.0.tree, &nodes))
    }
}

struct NoopActionHandler;

impl ActionHandler for NoopActionHandler {
    fn do_action(&mut self, _request: ActionRequest) {
        // `tag_accessibility_node`'s own signature never asked for
        // actionability -- accepted, but deliberately a no-op
        // (PLAN_PHASE5_STEP5_3_2.md's own "Explicitly out of scope").
    }
}

struct NoopDeactivationHandler;

impl DeactivationHandler for NoopDeactivationHandler {
    fn deactivate_accessibility(&mut self) {}
}

/// Bridges `tre-engine`'s tagged accessibility nodes onto the real Linux
/// AT-SPI2 bus. See `planning/archive/PLAN_PHASE5_STEP5_3_2.md` for the
/// full design.
pub struct A11yBridge {
    adapter: Mutex<accesskit_unix::Adapter>,
    state: Arc<SharedState>,
}

impl A11yBridge {
    /// Connects to the real Linux accessibility bus. Infallible:
    /// `accesskit_unix::Adapter::new` never returns a `Result` -- reading
    /// its real source (not assuming) confirms it gracefully degrades to
    /// a permanently-inactive adapter when no AT-SPI2 registry is
    /// reachable, with `publish` becoming a real no-op rather than an
    /// error in that case.
    #[must_use]
    pub fn connect(
        app_name: impl Into<String>,
        toolkit_name: impl Into<String>,
        toolkit_version: impl Into<String>,
    ) -> Self {
        let state = Arc::new(SharedState {
            nodes: Mutex::new(Vec::new()),
            tree: Tree {
                root: ROOT_ID,
                app_name: Some(app_name.into()),
                toolkit_name: Some(toolkit_name.into()),
                toolkit_version: Some(toolkit_version.into()),
            },
        });
        let adapter = accesskit_unix::Adapter::new(
            Handler(Arc::clone(&state)),
            NoopActionHandler,
            NoopDeactivationHandler,
        );
        Self {
            adapter: Mutex::new(adapter),
            state,
        }
    }

    /// Publishes one frame's tagged accessibility nodes. Never blocks on
    /// D-Bus I/O: `accesskit_unix::Adapter` already owns a real,
    /// dedicated background thread and an unbounded internal channel for
    /// that (confirmed by reading its source, not assumed) -- this only
    /// mutates in-process state and, if a real AT is already connected,
    /// enqueues an update via that thread's own channel.
    ///
    /// # Concurrency (REVIEW.md finding #133)
    /// Single-writer only: `self.state.nodes` and the live AT-SPI2 tree
    /// (`self.adapter`) are updated via two separate lock acquisitions,
    /// not one atomic step, so two threads calling `publish` concurrently
    /// with different node sets could leave the two out of sync with each
    /// other (whichever call's `nodes` store landed last vs. whichever
    /// call's tree update ran last). Every real caller today publishes
    /// from one thread only, so this is latent, not exercised -- but
    /// unlike this crate's other cross-thread-shared types
    /// (`SwmrSlotTable`, `MpscRingBuffer`), nothing enforced this
    /// contract before now. A future multi-window/multi-render-thread
    /// caller must serialize its own `publish` calls.
    pub fn publish(&self, nodes: &[AccessibilityNode]) {
        *self.state.nodes.lock().unwrap() = nodes.to_vec();
        let tree = &self.state.tree;
        self.adapter
            .lock()
            .unwrap()
            .update_if_active(|| build_tree_update(tree, nodes));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_role_covers_every_accessibility_role_variant() {
        assert_eq!(map_role(AccessibilityRole::Generic), Role::Unknown);
        assert_eq!(map_role(AccessibilityRole::Button), Role::Button);
        assert_eq!(map_role(AccessibilityRole::TextLabel), Role::Label);
        assert_eq!(map_role(AccessibilityRole::Image), Role::Image);
    }

    #[test]
    fn union_bounds_of_no_nodes_is_none() {
        assert_eq!(union_bounds(&[]), None);
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s (whole-number positions, no rounding), same \
                   reasoning as tre-engine's own exact-arithmetic tests"
    )]
    fn union_bounds_of_two_nodes_is_their_real_axis_aligned_union() {
        let a = AccessibilityNode {
            node_id: AccessibilityNodeId(1),
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            role: AccessibilityRole::Generic,
        };
        let b = AccessibilityNode {
            node_id: AccessibilityNodeId(2),
            x: 20.0,
            y: -5.0,
            width: 5.0,
            height: 5.0,
            role: AccessibilityRole::Generic,
        };
        let union = union_bounds(&[a, b]).expect("two nodes were provided");
        assert_eq!(
            (union.x0, union.y0, union.x1, union.y1),
            (0.0, -5.0, 25.0, 10.0)
        );
    }

    #[test]
    fn connect_then_immediately_drop_causes_no_panic_or_hang() {
        let bridge = A11yBridge::connect("tre-a11y-test", "tre-a11y-test-toolkit", "0.0.0");
        drop(bridge);
    }

    #[test]
    fn publish_returns_promptly_even_with_no_real_at_connected() {
        // Whether or not a real AT-SPI2 registry ends up activating this
        // adapter, `publish` must never hang the caller -- confirmed
        // with a real wall-clock bound, not just reasoned about.
        let bridge = A11yBridge::connect("tre-a11y-test", "tre-a11y-test-toolkit", "0.0.0");
        let node = AccessibilityNode {
            node_id: AccessibilityNodeId(1),
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
            role: AccessibilityRole::Generic,
        };
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            bridge.publish(&[node]);
        }
        assert!(
            start.elapsed() < std::time::Duration::from_secs(5),
            "1000 publish() calls took {:?} -- publish must never block on D-Bus I/O",
            start.elapsed()
        );
    }
}
