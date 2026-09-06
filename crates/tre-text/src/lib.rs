//! Dynamic typography: bidi + script run segmentation and shaping via
//! `rustybuzz`, glyph outline extraction via `skrifa`, (Linux-only this
//! step) a real `fontconfig`-driven font fallback cascade
//! (IMPLEMENTATION.md Step 4.1), and MSDF glyph-outline rasterization via
//! `fdsm` (Step 4.2.2). A deliberate all-pure-Rust font stack -- see
//! `planning/archive/PLAN_PHASE4_STEP4_1.md` -- so this workspace's only
//! C ABI boundary stays Vulkan. Produces the shaped-glyph data and raw
//! RGB8 MSDF bitmaps Step 4.2.3's GPU shader and Step 4.2.1's atlas
//! packer will consume; this crate does no GPU work of its own.
#![forbid(unsafe_code)]

mod error;
#[cfg(target_os = "linux")]
mod fallback;
mod msdf;
mod outline;
mod shape;

pub use error::TextError;
#[cfg(target_os = "linux")]
pub use fallback::{covers, resolve_font_index, resolve_run, FontCascade};
pub use msdf::{generate_msdf, MsdfBitmap};
pub use outline::{glyph_outline, Contour, OutlineSegment};
pub use shape::{segment_runs, shape_text, ShapedGlyph, ShapedRun, TextRun};
