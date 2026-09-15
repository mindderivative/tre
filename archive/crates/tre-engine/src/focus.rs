//! In-app *widget* keyboard focus and Tab-order traversal
//! (IMPLEMENTATION.md Phase 19, DESIGN.md Section 5's "Spatial Hit
//! Testing & Scene Tree Node Focus Manager" box) -- distinct from
//! OS-level *window* focus (`InputEvent::WindowFocused`, Phase 19 Step
//! 19.1), which fires even for an app with no concept of a focused
//! widget at all.
//!
//! Split into its own module (rather than folded into `lib.rs` next to
//! `AccessibilityNode`) because, unlike that plain data type, this one
//! also owns real stateful traversal logic -- matching why `input.rs`/
//! `canvas.rs` are each their own module too.

use crate::AccessibilityNodeId;

/// One per-frame-tagged focusable widget (`RenderingCanvas::
/// tag_focusable`), mirroring `AccessibilityNode`'s own real,
/// transform-correct world-space bounds. Reuses `AccessibilityNodeId`
/// as its identity rather than inventing a second parallel node-id
/// system for the same widget tree -- that type's own doc comment
/// already establishes the precedent: "The UI framework already owns
/// the real widget tree and its hierarchy; this engine only reports
/// each tagged node's *rendered* spatial position back, keyed by
/// whatever id the framework itself already tracks."
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FocusableNode {
    pub node_id: AccessibilityNodeId,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// HTML `tabindex` convention (WHATWG HTML Standard Section 6.6.7,
    /// "The tabindex attribute") -- see [`tab_order`]'s own doc comment
    /// for the full rule. `None` behaves identically to `Some(0)`
    /// (natural/geometry order); `Some(positive)` is visited earlier,
    /// in ascending order; `Some(negative)` is excluded from
    /// [`FocusManager::focus_next`]/[`FocusManager::focus_previous`]
    /// but remains directly settable via [`FocusManager::set_focus`].
    pub tab_index: Option<i32>,
}

/// Persistent (NOT per-frame-cleared, unlike [`FocusableNode`]'s own
/// list on `RenderingCanvas`) in-app widget focus state. Pure in-memory
/// logic, no OS handle, no thread affinity -- unlike `tre-python`'s
/// `unsendable` platform-connection wrappers (`Clipboard`/`TrayIcon`/
/// `A11yBridge`), this is plain data, like `EditableText`.
///
/// Deliberately does not parse raw `key_code`s or track modifier state
/// (Shift/Ctrl/Alt) itself: the caller recognizes Tab/Shift+Tab (and
/// tracks Shift state) and calls [`FocusManager::focus_next`]/
/// [`FocusManager::focus_previous`] once it has decided a tab
/// navigation should happen -- matching the established boundary
/// (`InputEvent::KeyboardKey`'s own doc comment: "layout-aware
/// translation is a UI framework concern, out of scope here"; Phase
/// 15's `EditableText::move_caret_left(extend: bool)` precedent has the
/// *caller* track Shift state and pass a plain bool). Duplicating that
/// tracking here would be a second, redundant source of truth.
#[derive(Debug, Default)]
pub struct FocusManager {
    focused: Option<AccessibilityNodeId>,
}

impl FocusManager {
    #[must_use]
    pub fn new() -> Self {
        Self { focused: None }
    }

    #[must_use]
    pub fn focused(&self) -> Option<AccessibilityNodeId> {
        self.focused
    }

    /// Programmatic/click-to-focus: sets focus directly, bypassing tab
    /// order entirely -- this is also the only way to focus a node
    /// whose `tab_index` is negative (HTML convention: still focusable,
    /// just excluded from *sequential* Tab navigation). `None` clears
    /// focus (e.g. the previously-focused widget was removed).
    pub fn set_focus(&mut self, node_id: Option<AccessibilityNodeId>) {
        self.focused = node_id;
    }

    /// Advances to the next node in `nodes`' tab order (see
    /// [`tab_order`]'s own doc comment for the sort rule), wrapping from
    /// the last back to the first. If nothing is currently focused, or
    /// the currently focused node id no longer appears in `nodes`
    /// (removed/unmounted since it was focused), focuses the first node
    /// in tab order. Returns the newly-focused id, or `None` if `nodes`
    /// contains no node eligible for sequential navigation (every node
    /// has a negative `tab_index`, or `nodes` is empty).
    pub fn focus_next(&mut self, nodes: &[FocusableNode]) -> Option<AccessibilityNodeId> {
        self.step(nodes, Direction::Forward)
    }

    /// The `focus_previous` mirror of [`FocusManager::focus_next`] --
    /// wraps from the first back to the last.
    pub fn focus_previous(&mut self, nodes: &[FocusableNode]) -> Option<AccessibilityNodeId> {
        self.step(nodes, Direction::Backward)
    }

    fn step(
        &mut self,
        nodes: &[FocusableNode],
        direction: Direction,
    ) -> Option<AccessibilityNodeId> {
        let order = tab_order(nodes);
        if order.is_empty() {
            self.focused = None;
            return None;
        }
        let next = match self
            .focused
            .and_then(|id| order.iter().position(|&candidate| candidate == id))
        {
            // Plain `usize` modular arithmetic (no signed-cast round trip,
            // which clippy's `cast_possible_wrap`/`cast_possible_truncation`
            // pedantic lints flag on a 32-bit target): stepping backward
            // from index 0 adds `len - 1` rather than subtracting 1, which
            // would underflow.
            Some(current_index) => {
                let len = order.len();
                let new_index = match direction {
                    Direction::Forward => (current_index + 1) % len,
                    Direction::Backward => (current_index + len - 1) % len,
                };
                order[new_index]
            }
            None => order[0],
        };
        self.focused = Some(next);
        Some(next)
    }
}

/// [`FocusManager::step`]'s own traversal direction -- a plain enum
/// instead of a signed `i64`/`isize` offset, so the wraparound
/// arithmetic in `step` stays entirely within `usize`.
#[derive(Clone, Copy)]
enum Direction {
    Forward,
    Backward,
}

/// Sequential Tab-navigation order for `nodes`, per the well-known HTML
/// `tabindex` convention (WHATWG HTML Standard Section 6.6.7, "The
/// tabindex attribute") rather than an invented rule: nodes with a
/// positive `tab_index` come first, ascending by value, ties broken by
/// `nodes`' own input order (stable sort) as the "document order"
/// stand-in -- this engine has no widget tree of its own to derive a
/// real document order from, the same "flat list, not a tree" boundary
/// `AccessibilityNode`'s own doc comment already draws. Nodes with
/// `tab_index` of `None` or `Some(0)` follow, in the same input-order
/// tie-break ("geometry order" in practice, since a real caller tags
/// nodes in their own on-screen/traversal order). Nodes with a negative
/// `tab_index` are excluded entirely -- HTML's own convention for
/// "focusable, but skip me during sequential Tab navigation" -- though
/// [`FocusManager::set_focus`] can still target them directly
/// (programmatic/click-to-focus is a different operation from Tab
/// traversal). Citing an existing, external, non-tre-specific standard
/// beats inventing bespoke tie-break semantics nobody outside this
/// codebase would recognize.
fn tab_order(nodes: &[FocusableNode]) -> Vec<AccessibilityNodeId> {
    let mut positive: Vec<&FocusableNode> = nodes
        .iter()
        .filter(|n| matches!(n.tab_index, Some(t) if t > 0))
        .collect();
    positive.sort_by_key(|n| n.tab_index.unwrap());
    let natural = nodes
        .iter()
        .filter(|n| !matches!(n.tab_index, Some(t) if t != 0));
    positive
        .into_iter()
        .chain(natural)
        .map(|n| n.node_id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: u64, tab_index: Option<i32>) -> FocusableNode {
        FocusableNode {
            node_id: AccessibilityNodeId(id),
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            tab_index,
        }
    }

    #[test]
    fn focus_next_on_an_empty_list_returns_none() {
        let mut manager = FocusManager::new();
        assert_eq!(manager.focus_next(&[]), None);
        assert_eq!(manager.focused(), None);
    }

    #[test]
    fn tab_order_follows_the_html_tabindex_convention() {
        // [None, Some(2), Some(-1), Some(1), None] -> positive tab_index
        // nodes first ascending (id 3 then id 1), then None/Some(0) in
        // input order (id 0 then id 4), excluding the negative one (id 2).
        let nodes = [
            node(0, None),
            node(1, Some(2)),
            node(2, Some(-1)),
            node(3, Some(1)),
            node(4, None),
        ];
        let order = tab_order(&nodes);
        assert_eq!(
            order,
            vec![
                AccessibilityNodeId(3),
                AccessibilityNodeId(1),
                AccessibilityNodeId(0),
                AccessibilityNodeId(4),
            ]
        );
    }

    #[test]
    fn focus_next_walks_the_tab_order_and_wraps_around() {
        let nodes = [node(0, None), node(1, Some(1)), node(2, None)];
        let mut manager = FocusManager::new();

        assert_eq!(manager.focus_next(&nodes), Some(AccessibilityNodeId(1)));
        assert_eq!(manager.focus_next(&nodes), Some(AccessibilityNodeId(0)));
        assert_eq!(manager.focus_next(&nodes), Some(AccessibilityNodeId(2)));
        // Wraps back to the first entry in tab order.
        assert_eq!(manager.focus_next(&nodes), Some(AccessibilityNodeId(1)));
    }

    #[test]
    fn focus_previous_walks_the_tab_order_backwards_and_wraps_around() {
        let nodes = [node(0, None), node(1, Some(1)), node(2, None)];
        let mut manager = FocusManager::new();

        // Nothing focused yet: `focus_previous` still starts at the
        // first entry in tab order, same as `focus_next` would.
        assert_eq!(manager.focus_previous(&nodes), Some(AccessibilityNodeId(1)));
        // From the first entry, stepping backward wraps to the last.
        assert_eq!(manager.focus_previous(&nodes), Some(AccessibilityNodeId(2)));
        assert_eq!(manager.focus_previous(&nodes), Some(AccessibilityNodeId(0)));
        assert_eq!(manager.focus_previous(&nodes), Some(AccessibilityNodeId(1)));
    }

    #[test]
    fn negative_tab_index_is_excluded_from_sequential_navigation_but_still_directly_settable() {
        let nodes = [node(0, None), node(1, Some(-1))];
        let mut manager = FocusManager::new();

        // `focus_next` never lands on node 1 (tab_index = -1).
        assert_eq!(manager.focus_next(&nodes), Some(AccessibilityNodeId(0)));
        assert_eq!(manager.focus_next(&nodes), Some(AccessibilityNodeId(0)));

        // But it's still directly focusable via `set_focus`.
        manager.set_focus(Some(AccessibilityNodeId(1)));
        assert_eq!(manager.focused(), Some(AccessibilityNodeId(1)));
    }

    #[test]
    fn set_focus_none_clears_focus() {
        let nodes = [node(0, None)];
        let mut manager = FocusManager::new();
        manager.focus_next(&nodes);
        assert!(manager.focused().is_some());

        manager.set_focus(None);
        assert_eq!(manager.focused(), None);
    }

    #[test]
    fn focus_next_recovers_when_the_focused_node_was_removed() {
        let mut manager = FocusManager::new();
        manager.set_focus(Some(AccessibilityNodeId(99))); // not in `nodes` below
        let nodes = [node(0, None), node(1, None)];
        // Falls back to the first node in tab order rather than panicking
        // or getting stuck on a stale id.
        assert_eq!(manager.focus_next(&nodes), Some(AccessibilityNodeId(0)));
    }
}
