//! Material Design 3 theming: color scheme (dynamic-color science, the
//! one real MD3 subsystem that genuinely needs `material_colors`).
//! Depends only on `engine-core` (§1 Locked Decisions: "keep `engine-
//! core` MD3-agnostic"), never on `vello_hybrid`/`wgpu` -- those stay
//! confined to `engine-render` (§15's own Risk Register mitigation),
//! which is why steps 8 (shadow spike) and 9 (ripple/state-layer) both
//! landed there instead of here despite being MD3 mechanisms.
//!
//! Scaffolded empty in M3 Phase 1 (workspace scaffold); real content
//! started at step 10 (§14, originally this crate's own `shape_morph`
//! module, §7.4). **Moved to `engine-core` at M7 Phase 4**, once real
//! integration (`PaintProperties.shape`) needed it there: `shape_morph`
//! turned out to have no actual MD3-specific content at all (pure
//! `Interpolate`/`peniko::kurbo` geometry), and `engine-core` can never
//! depend on this crate (§4) -- the identical resolution M7 Phase 1
//! already made for `MotionCurve`. This crate's only real content now
//! is `color` -- true MD3 color science (`material_colors`), the one
//! piece that couldn't move the same way.

mod color;

pub use color::{ColorScheme, DynamicTheme};
