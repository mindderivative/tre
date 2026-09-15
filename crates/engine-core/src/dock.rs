//! §11.4's docking data model (§14 step 15). Deliberately bounded scope,
//! per the architecture's own text: "exactly five fixed zones... no
//! arbitrary recursive splits" -- the §15 Risk Register's own named
//! mitigation for "Docking scope creep, the single largest net-new v1
//! subsystem."
//!
//! `DockLayout`/`DockZone` are plain data -- "deliberately POD-shaped...
//! so an app can serialize/restore it" (the architecture's own stated
//! reason). Neither derives `Clone`/`Debug`/`PartialEq`: `DockZone.size`
//! is an `Animated<f64>`, which implements none of those (the same
//! reason `NodeKind`/`PaintProperties` don't either -- checked directly
//! that nothing needs them before leaving them off).
//!
//! **What actually makes a `DockLayout` do anything lives on `Tree`, not
//! here** -- `Tree::apply_active_tab` (tabbed grouping) and Stage A's
//! own `Tree::set_splitter_position` (zone resizing, reused verbatim,
//! not a second mechanism, per §11.4's own "docking is a *consumer* of
//! splitters" text) -- matching every other `NodeKind` payload's "the
//! struct is inert, a `Tree` method is what makes it do anything" shape
//! this crate already uses throughout.

use smallvec::SmallVec;

use crate::animation::Animated;
use crate::node::NodeId;

/// §11.4's own fixed, non-nested zone set -- explicitly not arbitrary
/// nested splits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockSide {
    Left,
    Right,
    Top,
    Bottom,
    Center,
}

impl DockSide {
    fn index(self) -> usize {
        match self {
            DockSide::Left => 0,
            DockSide::Right => 1,
            DockSide::Top => 2,
            DockSide::Bottom => 3,
            DockSide::Center => 4,
        }
    }
}

/// §11.4's own struct sketch, unchanged. `panels` are tabbed together
/// when more than one; `active_tab` indexes into `panels` (which one is
/// currently attached to this zone's container, via `Tree::
/// apply_active_tab`); `size` is this zone's own extent, kept in sync
/// with whatever the shared splitter mechanism (Stage A) actually
/// applies to the zone's container -- syncing that is the caller's own
/// small helper, not something the splitter mechanism reaches in and
/// mutates itself (keeping "one generic resize mechanism, independent
/// call sites" a real, decoupled fact, not just an architectural
/// framing).
pub struct DockZone {
    pub panels: SmallVec<[NodeId; 4]>,
    pub active_tab: usize,
    pub size: Animated<f64>,
}

impl DockZone {
    pub fn new(size: f64) -> Self {
        Self {
            panels: SmallVec::new(),
            active_tab: 0,
            size: Animated::new(size),
        }
    }
}

/// §11.4's own struct sketch, unchanged.
pub struct DockLayout {
    pub zones: [Option<DockZone>; 5],
}

impl DockLayout {
    pub fn new() -> Self {
        Self {
            zones: [None, None, None, None, None],
        }
    }

    pub fn zone(&self, side: DockSide) -> Option<&DockZone> {
        self.zones[side.index()].as_ref()
    }

    pub fn zone_mut(&mut self, side: DockSide) -> Option<&mut DockZone> {
        self.zones[side.index()].as_mut()
    }

    pub fn set_zone(&mut self, side: DockSide, zone: DockZone) {
        self.zones[side.index()] = Some(zone);
    }
}

impl Default for DockLayout {
    fn default() -> Self {
        Self::new()
    }
}
