//! M7 Phase 5 (§7.6): container transform, the real choreography
//! `ARCHITECTURE.md` §7.6 describes as "not a new core mechanism" --
//! several `Animated<T>`/`ActiveAnimation` instances (§5, already real)
//! coordinated across two nodes with correlated timing. No navigation/
//! router subsystem, no new `PaintProperties` field: this module only
//! ever reads real, already-existing `Tree` state and calls `animate_
//! to` the same way any other caller would.
//!
//! **§7.6's own text names one "genuinely new" `engine-core` addition:
//! `Node::computed_layout() -> taffy::Layout`, to expose computed
//! layout as a queryable value outside the paint pass.** Investigated
//! directly before writing this module: `Tree::layout(id) -> &Layout`
//! (real position+size from Taffy's last pass) and `Tree::
//! absolute_position(id) -> (f64, f64)` (M6 Phase 4, transform-aware)
//! already both exist and are already `pub`, already used externally
//! by `engine-py`. Nothing new needed building for that half of this
//! phase -- this module simply uses them.
//!
//! `PaintProperties` has no raw position/size fields (only `background`/
//! `corner_radius`/`elevation`/`opacity`/`transform`/`shape`) -- the
//! trigger's captured bounds can only be represented through
//! `transform`, so `begin` computes the one non-uniform scale+translate
//! `Affine` that makes the destination's own real box visually coincide
//! with the trigger's captured box, then animates it back to `Affine::
//! IDENTITY`. `Interpolate for Affine`'s own doc comment (`animation.
//! rs`) confirms this is safe: a plain componentwise coefficient lerp
//! between two zero-shear (diagonal) affines stays diagonal at every
//! `t`, not just for the uniform-scale case this codebase has used so
//! far (M6 Phase 2's `translate x uniform-scale`).
//!
//! "The trigger's own content (icon/label)" / "the destination's real
//! content" (§7.6's own step 4 phrasing) is modeled the only way this
//! engine can know what "content" means generically: each container's
//! own direct children (`Node.children`) -- not a new concept.
//!
//! **§7.6's own step 5 names "§5's queue-drain mechanism"
//! (`CompletionHandle`) for automatic teardown-on-completion.** Real and
//! wired end to end as of M9 Phase 3: `begin`'s own `on_complete`
//! parameter attaches a real handle to the destination's own driven
//! `transform` animation, and `engine-py::Window.begin_container_
//! transform`'s own `on_complete` Python parameter is what a real app
//! uses to fire `end_container_transform` automatically. `teardown`
//! itself stays a plain, explicit function regardless -- "an ordinary
//! tree mutation" exactly as §7.6's own text says, called by the app's
//! own callback (or manually, `on_complete` omitted), not invoked by
//! this module on its own.

use std::time::{Duration, Instant};

use engine_core::{MotionCurve, NodeId, Tree};
use peniko::kurbo::Affine;

/// The shared timing every property this choreography drives animates
/// under -- one `duration`/`curve` for the synchronized set (§7.6 step
/// 3), plus how far into the future the destination's own content fade
/// starts relative to the trigger's (step 4's own "staggered start").
pub struct ContainerTransformConfig {
    pub duration: Duration,
    pub curve: MotionCurve,
    pub content_stagger: Duration,
}

/// §7.6's own 4-step "start the transition" choreography (capture,
/// insert/initialize, drive the synchronized set, content cross-fade).
/// `destination` must already be attached to the tree, laid out, and
/// carry its own real target `corner_radius`/`background`/`elevation`
/// -- this function captures those as the animation's real targets
/// before overwriting them to the trigger's captured from-state, then
/// animates back.
///
/// M9 Phase 3 (§5): `on_complete`, when given, is attached to the
/// destination's own driven `transform` animation only -- all four
/// driven properties share one `start`/`duration`/`curve` (step 3,
/// below), so any one of them completing means the whole transition is
/// genuinely done; no real need to attach it to more than one. This is
/// what finally lets a caller fire `teardown` automatically instead of
/// only ever manually (the exact gap this module's own doc comment
/// named as confirmed-still-unwired before this phase).
pub fn begin(
    tree: &mut Tree,
    trigger: NodeId,
    destination: NodeId,
    config: &ContainerTransformConfig,
    now: Instant,
    on_complete: Option<engine_core::CompletionHandle>,
) {
    // Step 1: Capture -- the trigger's real, computed bounds (position
    // via the transform-aware `absolute_position`, size via `layout`,
    // unaffected by paint-time transform) and its current appearance.
    let (trigger_x, trigger_y) = tree.absolute_position(trigger);
    let trigger_size = tree.layout(trigger).size;
    let (trigger_w, trigger_h) = (
        f64::from(trigger_size.width),
        f64::from(trigger_size.height),
    );
    let trigger_node = tree
        .get(trigger)
        .expect("begin: trigger NodeId not found in this Tree");
    let from_radius = trigger_node.paint.corner_radius.current;
    let from_color = trigger_node.paint.background.current;

    // Destination's own real target appearance, and its own natural
    // bounds -- read *before* this function ever touches its
    // `transform`, which still holds `Affine::IDENTITY` (`Paint
    // Properties::new`'s own default) at this point, so `absolute_
    // position`/`layout` report its real, untransformed position/size.
    let (dest_x, dest_y) = tree.absolute_position(destination);
    let dest_size = tree.layout(destination).size;
    let (dest_w, dest_h) = (f64::from(dest_size.width), f64::from(dest_size.height));
    let dest_node = tree
        .get(destination)
        .expect("begin: destination NodeId not found in this Tree");
    let to_radius = dest_node.paint.corner_radius.current;
    let to_color = dest_node.paint.background.current;
    let to_elevation = dest_node.paint.elevation.current;

    // Step 2: Insert/initialize -- the affine that maps destination's
    // own natural box onto the trigger's captured box: independent x/y
    // scale (a zero-shear/zero-rotation diagonal affine, matching
    // `Interpolate for Affine`'s own safe subspace) plus the translation
    // needed so its origin lands on the trigger's own captured origin,
    // not its own.
    let scale_x = if dest_w > 0.0 {
        trigger_w / dest_w
    } else {
        1.0
    };
    let scale_y = if dest_h > 0.0 {
        trigger_h / dest_h
    } else {
        1.0
    };
    let start_transform = Affine::new([
        scale_x,
        0.0,
        0.0,
        scale_y,
        trigger_x - dest_x,
        trigger_y - dest_y,
    ]);

    {
        let dest_node = tree
            .get_mut(destination)
            .expect("begin: destination NodeId not found in this Tree");
        dest_node.paint.transform.current = start_transform;
        dest_node.paint.corner_radius.current = from_radius;
        dest_node.paint.background.current = from_color;
        dest_node.paint.elevation.current = 0.0;
    }

    // Step 3: Drive one synchronized animation set -- transform,
    // corner_radius, background, elevation, all sharing one start/
    // duration/curve.
    {
        let dest_node = tree
            .get_mut(destination)
            .expect("begin: destination NodeId not found in this Tree");
        match on_complete {
            Some(handle) => dest_node.paint.transform.animate_to_with_completion(
                Affine::IDENTITY,
                config.duration,
                config.curve,
                now,
                handle,
            ),
            None => dest_node.paint.transform.animate_to(
                Affine::IDENTITY,
                config.duration,
                config.curve,
                now,
            ),
        }
        dest_node
            .paint
            .corner_radius
            .animate_to(to_radius, config.duration, config.curve, now);
        dest_node
            .paint
            .background
            .animate_to(to_color, config.duration, config.curve, now);
        dest_node
            .paint
            .elevation
            .animate_to(to_elevation, config.duration, config.curve, now);
    }

    // Step 4: Content cross-fade -- the trigger's own children fade out
    // immediately; the destination's own children start from `0.0` and
    // fade in with a staggered start time (a real future `Instant` --
    // `Animated::tick`'s own `saturating_duration_since` correctly holds
    // at the from-value for as long as `now < start`, confirmed by
    // reading `animation.rs` directly -- no new mechanism needed).
    let trigger_children: Vec<NodeId> = tree
        .get(trigger)
        .expect("begin: trigger NodeId not found in this Tree")
        .children
        .clone();
    for child in trigger_children {
        if let Some(child_node) = tree.get_mut(child) {
            child_node
                .paint
                .opacity
                .animate_to(0.0, config.duration, config.curve, now);
        }
    }

    let staggered_start = now + config.content_stagger;
    let dest_children: Vec<NodeId> = tree
        .get(destination)
        .expect("begin: destination NodeId not found in this Tree")
        .children
        .clone();
    for child in dest_children {
        if let Some(child_node) = tree.get_mut(child) {
            child_node.paint.opacity.current = 0.0;
            child_node.paint.opacity.animate_to(
                1.0,
                config.duration,
                config.curve,
                staggered_start,
            );
        }
    }
}

/// §7.6's own step 5: "the trigger node is hidden or removed... an
/// ordinary tree mutation, not special-cased machinery." Detaches
/// `trigger` from its own parent via the already-real `Tree::detach` --
/// the caller invokes this once it knows the transition has finished
/// (e.g. after `config.duration` has elapsed), since this codebase has
/// no real completion-queue wiring to fire it automatically (a
/// confirmed, stated gap -- see this module's own doc comment).
pub fn teardown(tree: &mut Tree, trigger: NodeId) {
    if let Some(parent) = tree
        .get(trigger)
        .expect("teardown: trigger NodeId not found in this Tree")
        .parent
    {
        tree.detach(parent, trigger);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::{NodeKind, PaintProperties};
    use peniko::Color;
    use taffy::prelude::{AvailableSpace, Position, Rect as TaffyRect, Size, Style, auto, length};

    fn absolute(left: f32, top: f32, width: f32, height: f32) -> Style {
        Style {
            position: Position::Absolute,
            inset: TaffyRect {
                left: length(left),
                top: length(top),
                right: auto(),
                bottom: auto(),
            },
            size: Size {
                width: length(width),
                height: length(height),
            },
            ..Default::default()
        }
    }

    /// A root containing a small trigger (with one child, its "icon")
    /// at one position, and a large destination (with one child, its
    /// "label") at a different position/appearance -- the real shape
    /// `begin` is meant to choreograph between.
    fn scene() -> (Tree, NodeId, NodeId, NodeId, NodeId) {
        let mut tree = Tree::new();
        let root = tree.insert(
            NodeKind::Container,
            Style {
                size: Size {
                    width: length(400.0),
                    height: length(400.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );

        let trigger = tree.insert(
            NodeKind::Rect,
            absolute(20.0, 20.0, 60.0, 40.0),
            PaintProperties::new(Color::from_rgba8(0xFF, 0x00, 0x00, 0xFF), 8.0, 0.0, 1.0),
        );
        tree.add_child(root, trigger);
        let trigger_icon = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(10.0),
                    height: length(10.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF), 0.0, 0.0, 1.0),
        );
        tree.add_child(trigger, trigger_icon);

        let destination = tree.insert(
            NodeKind::Rect,
            absolute(100.0, 150.0, 300.0, 200.0),
            PaintProperties::new(Color::from_rgba8(0x00, 0x00, 0xFF, 0xFF), 24.0, 6.0, 1.0),
        );
        tree.add_child(root, destination);
        let dest_label = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(50.0),
                    height: length(10.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF), 0.0, 0.0, 1.0),
        );
        tree.add_child(destination, dest_label);

        let available = Size {
            width: AvailableSpace::Definite(400.0),
            height: AvailableSpace::Definite(400.0),
        };
        tree.compute_layout(root, available);
        (tree, trigger, destination, trigger_icon, dest_label)
    }

    fn config() -> ContainerTransformConfig {
        ContainerTransformConfig {
            duration: Duration::from_millis(300),
            curve: MotionCurve::Emphasized,
            content_stagger: Duration::from_millis(90),
        }
    }

    #[test]
    fn begin_initializes_the_destination_to_the_triggers_captured_appearance_not_its_own() {
        let (mut tree, trigger, destination, _, _) = scene();
        let now = Instant::now();

        let trigger_radius = tree.get(trigger).unwrap().paint.corner_radius.current;
        let trigger_color = tree.get(trigger).unwrap().paint.background.current;
        let dest_real_radius = tree.get(destination).unwrap().paint.corner_radius.current;
        let dest_real_color = tree.get(destination).unwrap().paint.background.current;

        begin(&mut tree, trigger, destination, &config(), now, None);

        let dest = tree.get(destination).unwrap();
        assert_eq!(
            dest.paint.corner_radius.current, trigger_radius,
            "immediately after begin, the destination must show the trigger's own captured \
             corner_radius, not its own real target"
        );
        assert_eq!(
            dest.paint.background.current, trigger_color,
            "immediately after begin, the destination must show the trigger's own captured \
             background, not its own real target"
        );
        assert_eq!(
            dest.paint.elevation.current, 0.0,
            "the destination must start flat (no shadow) before the elevation animation runs"
        );
        assert_ne!(
            dest.paint.transform.current,
            Affine::IDENTITY,
            "the destination's initial transform must not be identity -- it needs to visually \
             occupy the trigger's own captured box, not its own real one"
        );

        // Real targets are still reachable via the animation this
        // started (not lost/overwritten) -- ticking to completion must
        // land on them, not on whatever was captured.
        tree.tick_all(now + Duration::from_secs(1));
        let dest = tree.get(destination).unwrap();
        assert_eq!(dest.paint.corner_radius.current, dest_real_radius);
        assert_eq!(dest.paint.background.current, dest_real_color);
        assert_eq!(dest.paint.transform.current, Affine::IDENTITY);
        assert_ne!(
            trigger_color, dest_real_color,
            "sanity: the two colors must differ for this test to mean anything"
        );
    }

    #[test]
    fn content_cross_fade_hides_trigger_children_immediately_and_staggers_destination_children() {
        let (mut tree, trigger, destination, trigger_icon, dest_label) = scene();
        let now = Instant::now();
        let cfg = config();

        begin(&mut tree, trigger, destination, &cfg, now, None);

        // Immediately: the destination's own child is pinned at 0.0
        // (its stagger hasn't started yet).
        assert_eq!(
            tree.get(dest_label).unwrap().paint.opacity.current,
            0.0,
            "the destination's own content must start fully transparent before its staggered \
             fade-in begins"
        );

        tree.tick_all(now + Duration::from_millis(50));
        assert!(
            tree.get(trigger_icon).unwrap().paint.opacity.current < 1.0,
            "the trigger's own content must already be fading out by 50ms in (immediate start)"
        );
        assert!(
            tree.get(dest_label).unwrap().paint.opacity.current.abs() < 1e-9,
            "the destination's own content must still be (~exactly) fully transparent before \
             its own staggered start time (90ms) arrives -- a real MotionCurve's own ease(0.0) \
             is only ~0.0, not always bit-exact, since it's solved via bisection (M7 Phase 1)"
        );

        tree.tick_all(now + Duration::from_secs(1));
        assert_eq!(tree.get(trigger_icon).unwrap().paint.opacity.current, 0.0);
        assert_eq!(tree.get(dest_label).unwrap().paint.opacity.current, 1.0);
    }

    #[test]
    fn teardown_detaches_the_trigger_from_its_own_parent() {
        let (mut tree, trigger, destination, _, _) = scene();
        begin(
            &mut tree,
            trigger,
            destination,
            &config(),
            Instant::now(),
            None,
        );

        let root = tree.get(trigger).unwrap().parent.unwrap();
        assert!(tree.get(root).unwrap().children.contains(&trigger));

        teardown(&mut tree, trigger);

        assert!(
            !tree.get(root).unwrap().children.contains(&trigger),
            "teardown must remove the trigger from its own parent's children"
        );
        assert!(
            tree.get(destination).unwrap().parent.is_some(),
            "the destination must remain in the tree, untouched by teardown"
        );
    }

    /// M9 Phase 3 (§5): the real proof this phase exists to make -- a
    /// real `on_complete` handle given to `begin` genuinely reaches
    /// `Tree::tick_all`'s own real drain once the transition finishes,
    /// not just that the new parameter compiles and is silently
    /// dropped somewhere along the way.
    #[test]
    fn a_real_on_complete_handle_reaches_tick_alls_own_drain_when_the_transition_finishes() {
        let (mut tree, trigger, destination, _, _) = scene();
        let now = Instant::now();
        let handle = engine_core::CompletionHandle(99);

        begin(
            &mut tree,
            trigger,
            destination,
            &config(),
            now,
            Some(handle),
        );

        // Not yet -- still mid-flight.
        let (_, completed) = tree.tick_all(now + Duration::from_millis(50));
        assert!(
            completed.is_empty(),
            "must not report completion before the transition genuinely finishes"
        );

        // Past the shared duration: the real handle reports exactly
        // once, the same tick every driven property finishes on.
        let (_, completed) = tree.tick_all(now + Duration::from_secs(1));
        assert_eq!(completed, vec![handle]);
    }

    /// The real regression counterpart -- `on_complete: None` (this
    /// module's own default, and every prior test's own real usage)
    /// must never report a completion, even on the exact tick the
    /// transition finishes.
    #[test]
    fn no_on_complete_given_never_reports_a_completion() {
        let (mut tree, trigger, destination, _, _) = scene();
        let now = Instant::now();

        begin(&mut tree, trigger, destination, &config(), now, None);

        let (_, completed) = tree.tick_all(now + Duration::from_secs(1));
        assert!(completed.is_empty());
    }
}
