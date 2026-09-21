//! YAML view/stylesheet parsing, reconciliation, and the
//! `BindingResolver` trait.
//!
//! §14 step 5 (minimal): `WidgetSpec` parsing and `Tree`-building for a
//! static view -- no `bindings:`/`handlers:`, no MD3 token resolution,
//! no reconciliation/hot-reload, no `pyo3` involvement at all. Each
//! lands at its own later build-order step (§14 step 12) once something
//! actually depends on it working.

mod binding;
mod build;
mod cascade;
mod include;
mod reconcile;
mod spec;
mod theme;
mod watch;

pub use binding::{
    BinOp, BindingResolver, Expression, ExpressionError, ResolveError, Value, evaluate,
    parse_binding,
};
pub use build::{SpecError, build_tree, load_styled_view, load_view};
pub use cascade::{StyleRule, Stylesheet, parse_stylesheet, resolve_style, resolve_style_layered};
pub use include::parse_view_with_includes;
pub use reconcile::Reconciler;
pub use spec::{FlexDirectionSpec, NodeKindSpec, StyleSpec, TextSpec, WidgetSpec, parse_view};
pub use theme::{ComponentOverride, ThemeSpec, parse_theme};
pub use watch::ViewWatcher;
