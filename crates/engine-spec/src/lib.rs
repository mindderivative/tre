//! YAML view/stylesheet parsing, reconciliation, and the
//! `BindingResolver` trait.
//!
//! §14 step 5 (minimal): `WidgetSpec` parsing and `Tree`-building for a
//! static view -- no `bindings:`/`handlers:`, no MD3 token resolution,
//! no reconciliation/hot-reload, no `pyo3` involvement at all. Each
//! lands at its own later build-order step (§14 step 12) once something
//! actually depends on it working.

mod build;
mod spec;

pub use build::{SpecError, build_tree, load_view};
pub use spec::{FlexDirectionSpec, NodeKindSpec, StyleSpec, TextSpec, WidgetSpec, parse_view};
