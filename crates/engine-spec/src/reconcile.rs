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
use std::path::Path;

use engine_core::{NodeId, Tree};
use engine_md3::ColorScheme;

use crate::build::{SpecError, build_tree, patch_node};
use crate::cascade::Stylesheet;
use crate::include::parse_view_with_includes;
use crate::spec::WidgetSpec;

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
    #[allow(clippy::too_many_arguments)]
    pub fn load(
        tree: &mut Tree,
        yaml: &str,
        default_theme: Option<&Stylesheet>,
        custom_theme: Option<&Stylesheet>,
        sheet: Option<&Stylesheet>,
        scheme: Option<&ColorScheme>,
        base_dir: Option<&Path>,
    ) -> Result<Self, SpecError> {
        let spec = parse_view_with_includes(yaml, base_dir)?;
        Self::load_spec(
            tree,
            spec,
            default_theme,
            custom_theme,
            sheet,
            scheme,
            base_dir,
        )
    }

    /// tre issue #3, Part B: `load`'s own real building/id-recording
    /// logic, factored out so a caller that already has a `WidgetSpec`
    /// -- constructed programmatically (`engine-py`'s own `pythonize`-
    /// based Tier 1), or via one of the JSON siblings (Tier 2) -- can
    /// build/reconcile from it directly, without a YAML-text round
    /// trip. `load` above is now just `parse_view_with_includes` then
    /// this, byte-for-byte the same external behavior it always had.
    #[allow(clippy::too_many_arguments)]
    pub fn load_spec(
        tree: &mut Tree,
        spec: WidgetSpec,
        default_theme: Option<&Stylesheet>,
        custom_theme: Option<&Stylesheet>,
        sheet: Option<&Stylesheet>,
        scheme: Option<&ColorScheme>,
        base_dir: Option<&Path>,
    ) -> Result<Self, SpecError> {
        let root = build_tree(
            tree,
            &spec,
            default_theme,
            custom_theme,
            sheet,
            scheme,
            base_dir,
        )?;
        let mut ids = HashMap::new();
        record_ids(tree, root, &spec, &mut ids);
        Ok(Self { root, spec, ids })
    }

    pub fn root(&self) -> NodeId {
        self.root
    }

    /// M51: re-resolves *every* node's `PaintProperties`/`layout_style`
    /// against a new set of theme layers -- the real, live-re-theme
    /// counterpart to `reconcile` above, for the case where the spec
    /// itself hasn't changed at all (a theme change, not a content
    /// change), so there's nothing to diff. Calls `patch_node`
    /// unconditionally for every node in `self.spec`, skipping
    /// `reconcile`'s own `node_props_equal` fast path entirely -- that
    /// check exists to avoid needless work when *most* nodes are
    /// unchanged across a content reload; here, by definition, *no*
    /// node's spec changed, only the theme layers being resolved
    /// against it, so every node's own static cascade genuinely needs
    /// re-running. `&self`, not `&mut self` -- only reads `self.spec`/
    /// `self.ids`, mutates only the passed-in `tree`.
    pub fn retheme(
        &self,
        tree: &mut Tree,
        default_theme: Option<&Stylesheet>,
        custom_theme: Option<&Stylesheet>,
        sheet: Option<&Stylesheet>,
        scheme: Option<&ColorScheme>,
        base_dir: Option<&Path>,
    ) -> Result<(), SpecError> {
        retheme_node(
            tree,
            &self.spec,
            &self.ids,
            default_theme,
            custom_theme,
            sheet,
            scheme,
            base_dir,
        )
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
    #[allow(clippy::too_many_arguments)]
    pub fn reconcile(
        &mut self,
        tree: &mut Tree,
        yaml: &str,
        default_theme: Option<&Stylesheet>,
        custom_theme: Option<&Stylesheet>,
        sheet: Option<&Stylesheet>,
        scheme: Option<&ColorScheme>,
        base_dir: Option<&Path>,
    ) -> Result<(), SpecError> {
        let new_spec = parse_view_with_includes(yaml, base_dir)?;
        self.reconcile_spec(
            tree,
            new_spec,
            default_theme,
            custom_theme,
            sheet,
            scheme,
            base_dir,
        )
    }

    /// tre issue #3, Part B: `reconcile`'s own real diffing/patching
    /// logic, factored out so a caller that already has a `WidgetSpec`
    /// can reconcile against it directly -- see `load_spec`'s own doc
    /// comment for the real motivation, identical here. `reconcile`
    /// above is now just `parse_view_with_includes` then this,
    /// byte-for-byte the same external behavior it always had.
    #[allow(clippy::too_many_arguments)]
    pub fn reconcile_spec(
        &mut self,
        tree: &mut Tree,
        new_spec: WidgetSpec,
        default_theme: Option<&Stylesheet>,
        custom_theme: Option<&Stylesheet>,
        sheet: Option<&Stylesheet>,
        scheme: Option<&ColorScheme>,
        base_dir: Option<&Path>,
    ) -> Result<(), SpecError> {
        if new_spec.id != self.spec.id || new_spec.kind != self.spec.kind {
            tree.remove(self.root);
            let new_root = build_tree(
                tree,
                &new_spec,
                default_theme,
                custom_theme,
                sheet,
                scheme,
                base_dir,
            )?;
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
            default_theme,
            custom_theme,
            sheet,
            scheme,
            base_dir,
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

/// M51: `record_ids`'s own recursive-walk shape, but calling
/// `patch_node` unconditionally at every node instead of just mapping
/// ids -- `self.ids` is already known-complete and known-correct here
/// (the spec never changed), so unlike `reconcile_node` there's no
/// insert/remove/match-by-kind logic needed at all, just "patch this
/// node, then patch every child."
#[allow(clippy::too_many_arguments)]
fn retheme_node(
    tree: &mut Tree,
    spec: &WidgetSpec,
    ids: &HashMap<String, NodeId>,
    default_theme: Option<&Stylesheet>,
    custom_theme: Option<&Stylesheet>,
    sheet: Option<&Stylesheet>,
    scheme: Option<&ColorScheme>,
    base_dir: Option<&Path>,
) -> Result<(), SpecError> {
    let id = *ids
        .get(&spec.id)
        .expect("retheme_node: every spec id must already be tracked in self.ids");
    patch_node(
        tree,
        id,
        spec,
        default_theme,
        custom_theme,
        sheet,
        scheme,
        base_dir,
    )?;
    for child in &spec.children {
        retheme_node(
            tree,
            child,
            ids,
            default_theme,
            custom_theme,
            sheet,
            scheme,
            base_dir,
        )?;
    }
    Ok(())
}

/// A node's own properties, ignoring `id` (already matched by the
/// caller) and `children` (diffed separately, below). M22 Phase 2
/// (§16.1): `image` compared too -- a real `image.src:`/`fit:` change
/// across a reload, with everything else unchanged, must still trigger
/// a real `patch_node` call; omitting it here would silently leave the
/// old image on screen after a real reload, the same real bug class
/// this function's own comparison already prevents for `style`/`text`.
fn node_props_equal(a: &WidgetSpec, b: &WidgetSpec) -> bool {
    a.kind == b.kind
        && a.classes == b.classes
        && a.style == b.style
        && a.text == b.text
        && a.image == b.image
}

#[allow(clippy::too_many_arguments)]
fn reconcile_node(
    tree: &mut Tree,
    tree_id: NodeId,
    old_spec: &WidgetSpec,
    new_spec: &WidgetSpec,
    default_theme: Option<&Stylesheet>,
    custom_theme: Option<&Stylesheet>,
    sheet: Option<&Stylesheet>,
    scheme: Option<&ColorScheme>,
    base_dir: Option<&Path>,
    ids: &mut HashMap<String, NodeId>,
) -> Result<(), SpecError> {
    if !node_props_equal(old_spec, new_spec) {
        patch_node(
            tree,
            tree_id,
            new_spec,
            default_theme,
            custom_theme,
            sheet,
            scheme,
            base_dir,
        )?;
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
                default_theme,
                custom_theme,
                sheet,
                scheme,
                base_dir,
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

            let new_tree_id = build_tree(
                tree,
                new_child,
                default_theme,
                custom_theme,
                sheet,
                scheme,
                base_dir,
            )?;
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
    fn load_spec_and_reconcile_spec_build_and_patch_from_an_already_parsed_widgetspec() {
        // tre issue #3, Part B: a caller that already has a real
        // `WidgetSpec` (constructed programmatically, or via one of the
        // JSON siblings) can build/reconcile from it directly, with the
        // identical real behavior `load`/`reconcile`'s own YAML-text
        // path already has -- proven here by driving both through
        // `crate::spec::parse_view` first, then handing the resulting
        // `WidgetSpec` values straight to `load_spec`/`reconcile_spec`.
        let before = crate::spec::parse_view(
            r##"
id: root
kind: Container
children:
  - id: swatch
    kind: Rect
    style: {width: 10, height: 10, background: "#112233"}
"##,
        )
        .unwrap();
        let after = crate::spec::parse_view(
            r##"
id: root
kind: Container
children:
  - id: swatch
    kind: Rect
    style: {width: 10, height: 10, background: "#445566"}
"##,
        )
        .unwrap();

        let mut tree = Tree::new();
        let mut reconciler =
            Reconciler::load_spec(&mut tree, before, None, None, None, None, None).unwrap();
        let swatch_id = reconciler.id_of("swatch").unwrap();

        reconciler
            .reconcile_spec(&mut tree, after, None, None, None, None, None)
            .unwrap();

        assert_eq!(
            reconciler.id_of("swatch"),
            Some(swatch_id),
            "an unchanged widget id must keep the exact same NodeId across reconcile_spec"
        );
        let node = tree.get(swatch_id).unwrap();
        let NodeKind::Rect = node.kind else {
            panic!("expected a Rect node");
        };
        assert_eq!(
            node.paint.background.current,
            peniko::Color::from_rgba8(0x44, 0x55, 0x66, 0xFF)
        );
    }

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
        let mut reconciler =
            Reconciler::load(&mut tree, yaml, None, None, None, None, None).unwrap();
        let swatch_id = reconciler.id_of("swatch").unwrap();

        // Reconcile against byte-identical YAML -- nothing changed at all.
        reconciler
            .reconcile(&mut tree, yaml, None, None, None, None, None)
            .unwrap();

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
        let mut reconciler =
            Reconciler::load(&mut tree, before, None, None, None, None, None).unwrap();
        let swatch_id = reconciler.id_of("swatch").unwrap();

        reconciler
            .reconcile(&mut tree, after, None, None, None, None, None)
            .unwrap();

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

    // --- M51: retheme ---

    #[test]
    fn retheme_applies_a_new_theme_layers_corner_radius_to_every_matching_node() {
        let yaml = r##"
id: root
kind: Container
children:
  - id: a
    kind: Rect
    style: {width: 10, height: 10, background: "#112233"}
  - id: b
    kind: Rect
    style: {width: 10, height: 10, background: "#112233"}
"##;
        let mut tree = Tree::new();
        let reconciler = Reconciler::load(&mut tree, yaml, None, None, None, None, None).unwrap();
        let a_id = reconciler.id_of("a").unwrap();
        let b_id = reconciler.id_of("b").unwrap();
        assert_eq!(tree.get(a_id).unwrap().paint.corner_radius.current, 0.0);

        let theme = crate::cascade::parse_stylesheet(
            "styles:\n  - kind: Rect\n    style: {corner_radius: 8}\n",
        )
        .unwrap();
        reconciler
            .retheme(&mut tree, Some(&theme), None, None, None, None)
            .unwrap();

        assert_eq!(tree.get(a_id).unwrap().paint.corner_radius.current, 8.0);
        assert_eq!(tree.get(b_id).unwrap().paint.corner_radius.current, 8.0);
        // NodeIds must survive -- retheme patches in place, never
        // removes/reinserts, the same real contract `reconcile` itself
        // already establishes for a content-only change.
        assert_eq!(reconciler.id_of("a"), Some(a_id));
        assert_eq!(reconciler.id_of("b"), Some(b_id));
    }

    #[test]
    fn retheme_leaves_a_widgets_own_inline_style_untouched() {
        let yaml = r##"
id: root
kind: Container
children:
  - id: themed
    kind: Rect
    style: {width: 10, height: 10, background: "#112233"}
  - id: inline
    kind: Rect
    style: {width: 10, height: 10, background: "#112233", corner_radius: 99}
"##;
        let mut tree = Tree::new();
        let reconciler = Reconciler::load(&mut tree, yaml, None, None, None, None, None).unwrap();
        let themed_id = reconciler.id_of("themed").unwrap();
        let inline_id = reconciler.id_of("inline").unwrap();

        let theme = crate::cascade::parse_stylesheet(
            "styles:\n  - kind: Rect\n    style: {corner_radius: 8}\n",
        )
        .unwrap();
        reconciler
            .retheme(&mut tree, Some(&theme), None, None, None, None)
            .unwrap();

        assert_eq!(
            tree.get(themed_id).unwrap().paint.corner_radius.current,
            8.0
        );
        assert_eq!(
            tree.get(inline_id).unwrap().paint.corner_radius.current,
            99.0,
            "a widget's own inline style must still win over the new theme layer"
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
        let mut reconciler =
            Reconciler::load(&mut tree, before, None, None, None, None, None).unwrap();
        let a_id = reconciler.id_of("a").unwrap();

        reconciler
            .reconcile(&mut tree, after, None, None, None, None, None)
            .unwrap();

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
        let mut reconciler =
            Reconciler::load(&mut tree, before, None, None, None, None, None).unwrap();
        let old_id = reconciler.id_of("w").unwrap();
        assert!(matches!(tree.get(old_id).unwrap().kind, NodeKind::Rect));

        reconciler
            .reconcile(&mut tree, after, None, None, None, None, None)
            .unwrap();

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
        let mut reconciler =
            Reconciler::load(&mut tree, before, None, None, None, None, None).unwrap();
        let old_root = reconciler.root();

        reconciler
            .reconcile(&mut tree, after, None, None, None, None, None)
            .unwrap();

        assert!(tree.get(old_root).is_none());
        assert_ne!(reconciler.root(), old_root);
        assert_eq!(reconciler.id_of("different-root"), Some(reconciler.root()));
    }
}
