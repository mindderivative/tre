//! §16.4's reconciliation: re-parsing a changed view and patching the
//! persistent `Tree` in place instead of rebuilding it. "Matched by
//! `id` plus `NodeKind` variant, the same keyed-diffing idea React
//! popularized" (§16.4's own words) -- a matched, unchanged node keeps
//! its real `NodeId` untouched (so focus, scroll, and any in-flight
//! `ActiveAnimation` on it survive); a matched, changed node gets
//! patched via `build::patch_node`; a node with no match in the old
//! tree is inserted fresh; an old node with no match in the new tree is
//! removed via `Tree::remove` (§14 step 12, Stage B).
//!
//! **Stated scope limit, not a silent gap:** a widget whose `id` is
//! present in both the old and new child lists, but at a different
//! position, is matched and patched in place -- but its position within
//! `Tree`'s own `children` list (and therefore `taffy`'s child order,
//! which drives flex layout) is *not* reordered to match. Reordering
//! needs a `taffy`-child-list-mutation API this step hasn't verified
//! and no test scenario here needs; every other diff outcome (patch,
//! insert, remove) is fully real.

use std::collections::{HashMap, HashSet};

use engine_core::{NodeId, Tree};
use engine_md3::ColorScheme;

use crate::build::{SpecError, build_tree, patch_node};
use crate::cascade::Stylesheet;
use crate::spec::{WidgetSpec, parse_view};

/// Owns the live mapping from a view's own author-assigned `id`s (§16.1)
/// to their current `NodeId`s, plus the last-loaded `WidgetSpec` tree to
/// diff the next reload against. Created once via [`Reconciler::load`];
/// [`Reconciler::reconcile`] re-parses a changed `yaml` string and
/// patches `tree` to match.
pub struct Reconciler {
    root: NodeId,
    spec: WidgetSpec,
    ids: HashMap<String, NodeId>,
}

impl Reconciler {
    /// Parses and builds `yaml` into `tree` for the first time, the
    /// same way [`crate::load_styled_view`] would, additionally
    /// recording every widget `id`'s resulting `NodeId` so a later
    /// [`Reconciler::reconcile`] call can find them again.
    pub fn load(
        tree: &mut Tree,
        yaml: &str,
        sheet: Option<&Stylesheet>,
        scheme: Option<&ColorScheme>,
    ) -> Result<Self, SpecError> {
        let spec = parse_view(yaml)?;
        let root = build_tree(tree, &spec, sheet, scheme)?;
        let mut ids = HashMap::new();
        record_ids(tree, root, &spec, &mut ids);
        Ok(Self { root, spec, ids })
    }

    pub fn root(&self) -> NodeId {
        self.root
    }

    /// The current `NodeId` for a widget's author-assigned `id`, if it
    /// exists in the most recently loaded/reconciled tree.
    pub fn id_of(&self, widget_id: &str) -> Option<NodeId> {
        self.ids.get(widget_id).copied()
    }

    /// Re-parses `yaml` and patches `tree` to match it, per this
    /// module's own doc comment. If the root widget's own `id` or
    /// `NodeKindSpec` changed, no partial match is meaningful (§16.1's
    /// own matching rule requires both) -- the whole old subtree is
    /// removed and the new one built fresh instead.
    pub fn reconcile(
        &mut self,
        tree: &mut Tree,
        yaml: &str,
        sheet: Option<&Stylesheet>,
        scheme: Option<&ColorScheme>,
    ) -> Result<(), SpecError> {
        let new_spec = parse_view(yaml)?;

        if new_spec.id != self.spec.id || new_spec.kind != self.spec.kind {
            tree.remove(self.root);
            let new_root = build_tree(tree, &new_spec, sheet, scheme)?;
            let mut ids = HashMap::new();
            record_ids(tree, new_root, &new_spec, &mut ids);
            self.root = new_root;
            self.spec = new_spec;
            self.ids = ids;
            return Ok(());
        }

        reconcile_node(
            tree,
            self.root,
            &self.spec,
            &new_spec,
            sheet,
            scheme,
            &mut self.ids,
        )?;
        self.spec = new_spec;
        Ok(())
    }
}

/// Records `spec.id -> id` and recurses into children, walking `tree`'s
/// own `children` list and `spec.children` in lockstep -- valid
/// immediately after `build_tree` built this exact subtree, since it
/// inserts and links children in the same order `spec.children` lists
/// them.
fn record_ids(tree: &Tree, id: NodeId, spec: &WidgetSpec, ids: &mut HashMap<String, NodeId>) {
    ids.insert(spec.id.clone(), id);
    let node = tree
        .get(id)
        .expect("record_ids: NodeId just built must exist in this Tree");
    for (&child_id, child_spec) in node.children.iter().zip(spec.children.iter()) {
        record_ids(tree, child_id, child_spec, ids);
    }
}

fn remove_ids(spec: &WidgetSpec, ids: &mut HashMap<String, NodeId>) {
    ids.remove(&spec.id);
    for child in &spec.children {
        remove_ids(child, ids);
    }
}

/// A node's own properties, ignoring `id` (already matched by the
/// caller) and `children` (diffed separately, below).
fn node_props_equal(a: &WidgetSpec, b: &WidgetSpec) -> bool {
    a.kind == b.kind && a.classes == b.classes && a.style == b.style && a.text == b.text
}

fn reconcile_node(
    tree: &mut Tree,
    tree_id: NodeId,
    old_spec: &WidgetSpec,
    new_spec: &WidgetSpec,
    sheet: Option<&Stylesheet>,
    scheme: Option<&ColorScheme>,
    ids: &mut HashMap<String, NodeId>,
) -> Result<(), SpecError> {
    if !node_props_equal(old_spec, new_spec) {
        patch_node(tree, tree_id, new_spec, sheet, scheme)?;
    }

    let old_by_id: HashMap<&str, &WidgetSpec> = old_spec
        .children
        .iter()
        .map(|c| (c.id.as_str(), c))
        .collect();
    let mut consumed: HashSet<&str> = HashSet::new();

    for new_child in &new_spec.children {
        if let Some(&old_child) = old_by_id.get(new_child.id.as_str())
            && old_child.kind == new_child.kind
        {
            consumed.insert(new_child.id.as_str());
            let child_tree_id = *ids
                .get(new_child.id.as_str())
                .expect("a matched child's id must already be tracked");
            reconcile_node(
                tree,
                child_tree_id,
                old_child,
                new_child,
                sheet,
                scheme,
                ids,
            )?;
        } else {
            // Either a genuinely new id, or the same id reused for a
            // different NodeKind -- either way, §16.1's own matching
            // rule ("id plus NodeKind variant") says this isn't the
            // same widget, so it's removed (if an old node under this
            // id exists at all) and inserted as a fresh subtree.
            //
            // The old node must be removed *here*, before `record_ids`
            // below overwrites `ids[new_child.id]` with the new node's
            // id -- otherwise the old id -> NodeId mapping is lost
            // before the old node itself is ever actually removed, and
            // the later "leftover old children" cleanup pass would
            // remove the wrong (freshly-inserted) node instead, having
            // only the new mapping left to look up.
            if let Some(&old_child) = old_by_id.get(new_child.id.as_str()) {
                if let Some(&old_tree_id) = ids.get(old_child.id.as_str()) {
                    tree.remove(old_tree_id);
                }
                remove_ids(old_child, ids);
                consumed.insert(new_child.id.as_str());
            }

            let new_tree_id = build_tree(tree, new_child, sheet, scheme)?;
            tree.add_child(tree_id, new_tree_id);
            record_ids(tree, new_tree_id, new_child, ids);
        }
    }

    for old_child in &old_spec.children {
        if !consumed.contains(old_child.id.as_str())
            && let Some(&old_tree_id) = ids.get(old_child.id.as_str())
        {
            tree.remove(old_tree_id);
            remove_ids(old_child, ids);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::NodeKind;

    #[test]
    fn unchanged_node_keeps_its_nodeid_and_is_never_patched() {
        let yaml = r##"
id: root
kind: Container
children:
  - id: swatch
    kind: Rect
    style: {width: 10, height: 10, background: "#112233"}
"##;
        let mut tree = Tree::new();
        let mut reconciler = Reconciler::load(&mut tree, yaml, None, None).unwrap();
        let swatch_id = reconciler.id_of("swatch").unwrap();

        // Reconcile against byte-identical YAML -- nothing changed at all.
        reconciler.reconcile(&mut tree, yaml, None, None).unwrap();

        assert_eq!(
            reconciler.id_of("swatch"),
            Some(swatch_id),
            "an unchanged widget must keep the exact same NodeId across a reconcile"
        );
    }

    #[test]
    fn changed_style_patches_paint_properties_in_place_same_nodeid() {
        let before = r##"
id: root
kind: Container
children:
  - id: swatch
    kind: Rect
    style: {width: 10, height: 10, background: "#112233"}
"##;
        let after = r##"
id: root
kind: Container
children:
  - id: swatch
    kind: Rect
    style: {width: 10, height: 10, background: "#445566"}
"##;
        let mut tree = Tree::new();
        let mut reconciler = Reconciler::load(&mut tree, before, None, None).unwrap();
        let swatch_id = reconciler.id_of("swatch").unwrap();

        reconciler.reconcile(&mut tree, after, None, None).unwrap();

        assert_eq!(
            reconciler.id_of("swatch"),
            Some(swatch_id),
            "a style-only change must patch in place, not remove+reinsert"
        );
        let node = tree.get(swatch_id).unwrap();
        assert_eq!(
            node.paint.background.current,
            peniko::Color::from_rgba8(0x44, 0x55, 0x66, 0xFF)
        );
    }

    #[test]
    fn a_new_child_is_inserted_and_a_removed_child_actually_leaves_the_tree() {
        let before = r##"
id: root
kind: Container
children:
  - id: a
    kind: Rect
    style: {width: 10, height: 10, background: "#112233"}
"##;
        let after = r##"
id: root
kind: Container
children:
  - id: b
    kind: Rect
    style: {width: 10, height: 10, background: "#445566"}
"##;
        let mut tree = Tree::new();
        let mut reconciler = Reconciler::load(&mut tree, before, None, None).unwrap();
        let a_id = reconciler.id_of("a").unwrap();

        reconciler.reconcile(&mut tree, after, None, None).unwrap();

        assert!(
            tree.get(a_id).is_none(),
            "a disappeared widget must actually be removed from the Tree, not just untracked"
        );
        assert!(reconciler.id_of("a").is_none());
        let b_id = reconciler
            .id_of("b")
            .expect("a genuinely new widget id must be inserted");
        assert!(tree.get(b_id).is_some());

        let root_node = tree.get(reconciler.root()).unwrap();
        assert_eq!(root_node.children, vec![b_id]);
    }

    #[test]
    fn same_id_different_kind_is_treated_as_remove_plus_insert() {
        let before = r##"
id: root
kind: Container
children:
  - id: w
    kind: Rect
    style: {width: 10, height: 10, background: "#112233"}
"##;
        let after = r##"
id: root
kind: Container
children:
  - id: w
    kind: Text
    text: {content: "Hi", font_family: Roboto, font_size: 16}
    style: {width: 10, height: 10, background: "#112233"}
"##;
        let mut tree = Tree::new();
        let mut reconciler = Reconciler::load(&mut tree, before, None, None).unwrap();
        let old_id = reconciler.id_of("w").unwrap();
        assert!(matches!(tree.get(old_id).unwrap().kind, NodeKind::Rect));

        reconciler.reconcile(&mut tree, after, None, None).unwrap();

        assert!(
            tree.get(old_id).is_none(),
            "the old Rect node must be gone, not reused for a different NodeKind"
        );
        let new_id = reconciler.id_of("w").unwrap();
        assert_ne!(
            new_id, old_id,
            "a kind change must mint a genuinely new NodeId"
        );
        assert!(matches!(tree.get(new_id).unwrap().kind, NodeKind::Text(_)));
    }

    #[test]
    fn root_identity_change_tears_down_and_rebuilds_everything() {
        let before = "id: root\nkind: Container\n";
        let after = "id: different-root\nkind: Container\n";

        let mut tree = Tree::new();
        let mut reconciler = Reconciler::load(&mut tree, before, None, None).unwrap();
        let old_root = reconciler.root();

        reconciler.reconcile(&mut tree, after, None, None).unwrap();

        assert!(tree.get(old_root).is_none());
        assert_ne!(reconciler.root(), old_root);
        assert_eq!(reconciler.id_of("different-root"), Some(reconciler.root()));
    }
}
