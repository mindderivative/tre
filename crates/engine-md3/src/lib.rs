//! Material Design 3 theming: color scheme, shadow/ripple helpers, shape
//! morph, motion-curve presets. Depends only on `engine-core` (§1
//! Locked Decisions: "keep `engine-core` MD3-agnostic"), never on
//! `vello_hybrid`/`wgpu` -- those stay confined to `engine-render`
//! (§15's own Risk Register mitigation), which is why steps 8
//! (shadow spike) and 9 (ripple/state-layer) both landed there instead
//! of here despite being MD3 mechanisms.
//!
//! Scaffolded empty in M3 Phase 1 (workspace scaffold); real content
//! starts at step 10 (§14, this crate's `shape_morph` module, §7.4) --
//! the first step whose mechanism (`kurbo::BezPath` geometry, no GPU
//! calls) actually belongs in this crate rather than `engine-render`.

mod color;
mod shape_morph;

pub use color::{ColorScheme, DynamicTheme};
pub use shape_morph::ShapeKey;
