//! `PyWindow`'s node-factory methods (review follow-through, M28 Phase
//! 2, §4/§8): every `add_*`/`build_shell` method that creates a new
//! node and hands back a real `Node` wrapping it. Split out of
//! `window.rs` itself, which used to hold this together with synthetic
//! input dispatch, docking delegation, and virtual-list/canvas
//! plumbing -- four largely independent responsibilities the review's
//! own architecture lens flagged as sharing one ~1700-line file only
//! because that's where each was added at the time, not by design.
//! `PyWindow`'s real `#[pymethods]` now spans this file plus `window.rs`
//! (construction/theming/GC), `window_input.rs`, `window_docking.rs`,
//! and `window_virtual_canvas.rs` -- enabled by pyo3's own
//! `multiple-pymethods` feature (`Cargo.toml`), no behavior change.

use std::rc::Rc;

use engine_core::{
    AccessNodeData, Action, Animated, CheckboxState, ContentFit, IconState, ImageState, NodeKind,
    PaintProperties, Role, SliderState, SplitterState, TextFieldState, TextState,
};
use peniko::Color;
use pyo3::prelude::*;
use taffy::prelude::{Size, Style, length};

use crate::error::EngineError;
use crate::node::Node;
use crate::window::{PyWindow, positioned_style};

/// M22 Phase 2 (§16.1): `Window.add_image`'s own real `fit:` string
/// vocabulary -- `parse_dock_side`'s own established pattern
/// (`dock.rs`), applied to `ContentFit`'s three real variants.
fn parse_content_fit(fit: &str) -> PyResult<ContentFit> {
    match fit {
        "cover" => Ok(ContentFit::Cover),
        "contain" => Ok(ContentFit::Contain),
        "fill" => Ok(ContentFit::Fill),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown content fit {other:?} -- expected one of \"cover\", \"contain\", \"fill\""
        ))),
    }
}

#[pymethods]
impl PyWindow {
    /// §14 step 6's own "node creation" -- one shape (a colored rect, a
    /// child of this window's implicit root row) is the real minimal
    /// slice; unchanged by the `PyWindow` split, just moved here with
    /// `App` itself.
    #[pyo3(signature = (background, width, height, x=None, y=None))]
    fn add_rect(
        &self,
        background: (u8, u8, u8, u8),
        width: f32,
        height: f32,
        x: Option<f32>,
        y: Option<f32>,
    ) -> Node {
        let (r, g, b, a) = background;
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Rect,
            positioned_style(
                Size {
                    width: length(width),
                    height: length(height),
                },
                x,
                y,
            ),
            PaintProperties::new(Color::from_rgba8(r, g, b, a), 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        self.wrap_node(id)
    }

    /// M27 Phase 2 (§5): a real, genuine gap found while building the
    /// showcase demo's component gallery screen -- `NodeKind::Text` has
    /// been fully real and renderable since §14 step 4 (`TextRenderer`,
    /// `engine-render`), and declarative `kind: Text` in a `view.yaml`
    /// has built it since §14 step 5, but `Window` (the imperative path)
    /// had no way to create one at all, confirmed via grep before this
    /// method existed. Mirrors `add_rect`'s own real shape exactly --
    /// `background` is repurposed as the glyph color for a plain
    /// `NodeKind::Text` (no visible box of its own), the identical real
    /// convention `paint_node`'s own `NodeKind::Text` arm and the
    /// declarative `required_background(..., "Text")` path both already
    /// establish -- not a new convention invented here. `width`/`height`
    /// are required, the same as every other `add_*` method except
    /// `add_icon` (a single `size`) -- no measure-function/intrinsic-
    /// sizing wiring exists for `Text` to lean on instead, confirmed
    /// before choosing this shape rather than assumed.
    #[pyo3(signature = (content, background, width, height, font_family="Roboto", font_weight=400.0, font_size=16.0, x=None, y=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_text(
        &self,
        content: &str,
        background: (u8, u8, u8, u8),
        width: f32,
        height: f32,
        font_family: &str,
        font_weight: f32,
        font_size: f32,
        x: Option<f32>,
        y: Option<f32>,
    ) -> Node {
        let (r, g, b, a) = background;
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Text(TextState {
                content: content.to_string(),
                font_family: font_family.to_string(),
                font_weight,
                font_size,
            }),
            positioned_style(
                Size {
                    width: length(width),
                    height: length(height),
                },
                x,
                y,
            ),
            PaintProperties::new(Color::from_rgba8(r, g, b, a), 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        self.wrap_node(id)
    }

    /// M14 Phase 1 (§5, §7.3): creates a real `NodeKind::Checkbox`,
    /// mirroring `add_rect`'s own real shape exactly -- `background`
    /// is the box's own real fill color (universal `PaintProperties`,
    /// same as any other node), `checked` seeds `CheckboxState`'s own
    /// initial state (and its `check_progress` starting already at the
    /// matching `1.0`/`0.0`, `CheckboxState::new`'s own real contract).
    /// The already-generic `set_on_click`/`enable_interaction()` work
    /// on this exactly like any other node -- no new interaction wiring
    /// needed here.
    #[pyo3(signature = (background, width, height, checked=false, x=None, y=None))]
    fn add_checkbox(
        &self,
        background: (u8, u8, u8, u8),
        width: f32,
        height: f32,
        checked: bool,
        x: Option<f32>,
        y: Option<f32>,
    ) -> Node {
        let (r, g, b, a) = background;
        // M20 Phase 1 (§7.1, §7.3): a Checkbox created *after* `Window.
        // set_theme` must start genuinely themed, not stuck with the
        // plain white default until another `set_theme` call happens
        // to re-push it -- the same real intent `Node.enable_
        // interaction`'s own construction-time read already has. Real,
        // necessary difference from that precedent: `ThemeState::
        // on_surface()`'s own no-theme-set default is black, but
        // `CheckboxState`'s own real default is white -- reading it
        // unconditionally would silently replace an un-themed
        // checkbox's real white mark with black. Gated on `theme.
        // is_set()` so the real historical default survives untouched
        // until an app genuinely calls `set_theme`.
        let mut checkbox_state = CheckboxState::new(checked);
        {
            let theme = self.theme.borrow();
            if theme.is_set() {
                checkbox_state.mark_tint = theme.on_surface();
            }
        }
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Checkbox(checkbox_state),
            positioned_style(
                Size {
                    width: length(width),
                    height: length(height),
                },
                x,
                y,
            ),
            PaintProperties::new(Color::from_rgba8(r, g, b, a), 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        self.wrap_node(id)
    }

    /// M14 Phase 2 (§5, §7.3): creates a real `NodeKind::Slider`,
    /// mirroring `add_checkbox`'s own real shape exactly -- `background`
    /// is the thumb's own real fill color (universal `PaintProperties`,
    /// same as any other node); `value` seeds `SliderState`'s own
    /// initial `thumb_position` (clamped `0.0..=1.0`, `SliderState::
    /// new`'s own real contract). The real drag-to-set interaction is
    /// entirely internal to `Tree::dispatch` (M14 Phase 2's own real
    /// finding, mirroring how `Splitter` dragging already works) -- no
    /// Python-facing wiring needed for that half at all.
    #[pyo3(signature = (background, width, height, value=0.0, x=None, y=None))]
    fn add_slider(
        &self,
        background: (u8, u8, u8, u8),
        width: f32,
        height: f32,
        value: f64,
        x: Option<f32>,
        y: Option<f32>,
    ) -> Node {
        let (r, g, b, a) = background;
        // M20 Phase 1 (§7.1, §7.3): same real "themed-at-construction,
        // gated on a real theme actually being set" reasoning as
        // `add_checkbox`, above -- `SliderState`'s own real default
        // track color is gray, not `on_surface()`'s own black default.
        let mut slider_state = SliderState::new(value);
        {
            let theme = self.theme.borrow();
            if theme.is_set() {
                slider_state.track_tint = theme.on_surface();
            }
        }
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Slider(slider_state),
            positioned_style(
                Size {
                    width: length(width),
                    height: length(height),
                },
                x,
                y,
            ),
            PaintProperties::new(Color::from_rgba8(r, g, b, a), 0.0, 0.0, 1.0),
        );
        // M24 Phase 1 (§10): a real, necessary connected fix, found
        // only by actually trying the new arrow-key increment end to
        // end from Python, not assumed -- `Tree::dispatch`'s own new
        // `dispatch_slider_key` requires a real focused slider to ever
        // reach it at all, but before this a `Slider` had no `access.
        // actions` set anywhere, so `collect_interactive`'s own real
        // Tab-order predicate never included one (`enable_interaction`
        // only ever touched ripple/hover tint, not focusability). A
        // real `Slider` now opts into keyboard focus at construction
        // the identical way `add_text_field` already does -- §10's own
        // "keyboard operability ships from day one" text, applied to
        // the one real component this milestone's own scope covers.
        tree.set_access(
            id,
            AccessNodeData::new(Role::Slider).with_action(Action::Focus),
        );
        tree.add_child(self.root, id);
        self.wrap_node(id)
    }

    /// M22 Phase 1 (§5): creates a real `NodeKind::Image`, loaded from
    /// a real file on disk. Unlike `add_rect`/`add_checkbox`/
    /// `add_slider`, deliberately does *not* take a `background` param
    /// -- mirrors `add_canvas`'s own real precedent instead (a
    /// hardcoded transparent `PaintProperties` fill), since there's no
    /// meaningful "behind the content" color this phase scopes for a
    /// node whose entire content is a loaded image, the same "fully
    /// custom-drawn kind doesn't expose a separate background" reasoning
    /// `add_canvas` already established.
    ///
    /// Decoding is the real crate-boundary work this method does that
    /// `engine-core` deliberately never does itself (`ImageState`'s own
    /// doc comment) -- `image::open` reads and decodes the file
    /// (whatever real format its own magic-byte sniffing detects among
    /// this crate's enabled `png`/`jpeg` features), `.to_rgba8()` gives
    /// real straight-alpha (unpremultiplied) 8-bit RGBA pixels, and
    /// those raw bytes become a `peniko::ImageData` via `peniko::Blob`'s
    /// own real `From<Vec<u8>>` impl -- zero copying beyond what
    /// `to_rgba8()` itself already allocates.
    ///
    /// M22 Phase 2 (§16.1): `fit` (`"cover"`/`"contain"`/`"fill"`,
    /// default `"fill"` -- byte-for-byte Phase 1's own only behavior)
    /// sets `ImageState.content_fit`, the identical field `kind: Image`
    /// in a real `view.yaml`'s own `image.fit:` sets -- kept symmetric
    /// with the declarative path rather than leaving this imperative
    /// entry point stuck at `Fill` forever.
    #[pyo3(signature = (path, width, height, fit="fill", x=None, y=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_image(
        &self,
        path: &str,
        width: f32,
        height: f32,
        fit: &str,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        let content_fit = parse_content_fit(fit)?;
        let decoded = image::open(path)
            .map_err(|e| EngineError::ImageLoadFailed {
                path: path.to_string(),
                reason: e.to_string(),
            })?
            .to_rgba8();
        let (img_width, img_height) = decoded.dimensions();
        let image_data = peniko::ImageData {
            data: peniko::Blob::from(decoded.into_raw()),
            format: peniko::ImageFormat::Rgba8,
            alpha_type: peniko::ImageAlphaType::Alpha,
            width: img_width,
            height: img_height,
        };
        let mut image_state = ImageState::new(image_data);
        image_state.content_fit = content_fit;

        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Image(image_state),
            positioned_style(
                Size {
                    width: length(width),
                    height: length(height),
                },
                x,
                y,
            ),
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        Ok(self.wrap_node(id))
    }

    /// M23 Phase 1 (§1, §3): creates a real `NodeKind::Icon` from one
    /// of this project's own real curated Material Symbols icons
    /// (`engine_md3::icons::path_for`) -- deliberately takes one
    /// square `size`, not `width`+`height` the way every other
    /// `add_*` method does: Material Symbols icons are a real,
    /// uniformly square icon system by design (every fetched icon's
    /// own SVG `width`/`height` attributes are identical), so a
    /// single size parameter is a genuine ergonomic fit, not an
    /// invented shortcut. `color` is the icon's own real, plain fill
    /// tint -- MD3 icons have no separate "background" the way a
    /// boxed component does, so unlike `add_rect`/`add_checkbox` this
    /// takes no `background` param at all (mirroring `add_canvas`/
    /// `add_image`'s own real precedent for a kind with no meaningful
    /// separate background). An unknown `name` is a real, clear
    /// `PyValueError` -- `parse_dock_side`/`parse_content_fit`'s own
    /// established "fail loudly at the boundary" pattern, not routed
    /// through `EngineError` since this is a pure name-lookup failure
    /// with no I/O involved, the same reason those two live directly
    /// here rather than in `error.rs`.
    #[pyo3(signature = (name, color, size, x=None, y=None))]
    fn add_icon(
        &self,
        name: &str,
        color: (u8, u8, u8, u8),
        size: f32,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        let d = engine_md3::icons::path_for(name).ok_or_else(|| {
            let known: Vec<&str> = engine_md3::icons::names().collect();
            pyo3::exceptions::PyValueError::new_err(format!(
                "unknown icon {name:?} -- expected one of {known:?}"
            ))
        })?;
        let path = peniko::kurbo::BezPath::from_svg(d).unwrap_or_else(|e| {
            panic!("engine_md3::icons's own curated path data for {name:?} must parse: {e}")
        });
        let (r, g, b, a) = color;

        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Icon(IconState {
                path,
                tint: Color::from_rgba8(r, g, b, a),
            }),
            positioned_style(
                Size {
                    width: length(size),
                    height: length(size),
                },
                x,
                y,
            ),
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        Ok(self.wrap_node(id))
    }

    /// M15 Phase 1 (§5, §16.7): creates a real `NodeKind::TextField`,
    /// mirroring `add_checkbox`/`add_slider`'s own real shape --
    /// `background` is the field's own real box fill (universal
    /// `PaintProperties`, same as any other node); `content`/
    /// `font_family`/`font_weight`/`font_size` seed `TextFieldState`
    /// directly (`TextFieldState::new`'s own real contract: `cursor`
    /// starts at `content`'s own end). **Real finding (see `PLAN.md`):**
    /// this is the first real `engine-py` caller of `Tree::set_access`
    /// anywhere -- every other `add_*` method leaves a node at the
    /// default `Role::Unknown`/no actions, confirmed via grep before
    /// this method. A `TextField` is inherently interactive (unlike a
    /// plain `Rect`, which only becomes Tab-reachable as a side effect
    /// of `set_on_click`), so it opts into `Role::TextInput` +
    /// `Action::Focus` right here at construction, not deferred to a
    /// later opt-in call.
    #[pyo3(signature = (background, width, height, content="", font_family="Roboto", font_weight=400.0, font_size=16.0, x=None, y=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_text_field(
        &self,
        background: (u8, u8, u8, u8),
        width: f32,
        height: f32,
        content: &str,
        font_family: &str,
        font_weight: f32,
        font_size: f32,
        x: Option<f32>,
        y: Option<f32>,
    ) -> Node {
        let (r, g, b, a) = background;
        // M20 Phase 2 (§7.1, §7.3): same real "themed-at-construction,
        // gated on a real theme actually being set" reasoning as
        // `add_checkbox`/`add_slider` (M20 Phase 1) -- `TextFieldState`
        // 's own real default text color is dark (`0x1C1B1F`), not
        // `on_surface()`'s own black no-theme default.
        let mut text_field_state =
            TextFieldState::new(content, font_family, font_weight, font_size);
        {
            let theme = self.theme.borrow();
            if theme.is_set() {
                text_field_state.text_tint = theme.on_surface();
            }
        }
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::TextField(text_field_state),
            positioned_style(
                Size {
                    width: length(width),
                    height: length(height),
                },
                x,
                y,
            ),
            PaintProperties::new(Color::from_rgba8(r, g, b, a), 0.0, 0.0, 1.0),
        );
        tree.set_access(
            id,
            AccessNodeData::new(Role::TextInput).with_action(Action::Focus),
        );
        tree.add_child(self.root, id);
        self.wrap_node(id)
    }

    /// M13 Phase 1 (§11.2): a real, one-call way to build `AppShell`'s
    /// own named regions -- `self.root` itself stays a plain `Flex Row`
    /// (every other `add_*` method's own implicit flow depends on that,
    /// confirmed by direct read of `PyWindow::new`), so this creates one
    /// new dedicated `Container` child of `self.root`, `Flex Column`,
    /// sized to the window's own real width/height -- the one new
    /// structural node this phase adds. `menu_bar`/`toolbar`/`status_
    /// bar` are already-built `Node`s the app supplies (this method is
    /// a composition convenience, not a content-authoring one, matching
    /// `AppShell`'s own struct sketch: it names *which* node serves
    /// which chrome role, it doesn't build that node's own content) --
    /// each is re-parented into the shell container via `Tree::try_add_
    /// child`, not the cheap `Tree::add_child` the freshly-inserted
    /// `shell`/`content` nodes below use -- **real finding while writing
    /// this phase's own example:** every `add_*` method already
    /// attaches its result to `self.root` immediately, so `menu_bar`/
    /// `toolbar`/`status_bar` always already have a real parent by the
    /// time this runs; the cheap `add_child` only ever detaches nothing,
    /// leaving a node listed as a child of *both* its old parent and the
    /// shell -- real tree corruption `examples/app_shell.py`'s own live
    /// `accesskit` validation caught as a duplicate-child panic, not any
    /// pytest test (none render a real frame). `try_add_child` is the
    /// real, checked counterpart that detaches first, the same mechanism
    /// `Node.add_child`'s own pyo3 wrapper already uses. A new, empty
    /// `content` `Container` is created here, `flex_grow: 1.0` so it
    /// fills whatever vertical space the given chrome regions don't take
    /// -- the one handle the caller needs to keep, for Phase 2's own
    /// real navigation.
    #[pyo3(signature = (menu_bar=None, toolbar=None, status_bar=None))]
    fn build_shell(
        &mut self,
        menu_bar: Option<PyRef<'_, Node>>,
        toolbar: Option<PyRef<'_, Node>>,
        status_bar: Option<PyRef<'_, Node>>,
    ) -> PyResult<Node> {
        for region in [&menu_bar, &toolbar, &status_bar].into_iter().flatten() {
            if !Rc::ptr_eq(&self.tree, &region.tree) {
                return Err(EngineError::ForeignNode.into());
            }
        }

        let mut tree = self.tree.borrow_mut();
        let shell = tree.insert(
            NodeKind::Container,
            Style {
                display: taffy::Display::Flex,
                flex_direction: taffy::FlexDirection::Column,
                size: Size {
                    width: length(self.width as f32),
                    height: length(self.height as f32),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, shell);

        // menu_bar/toolbar/status_bar are pre-existing nodes -- every
        // add_* method already attaches its result to self.root
        // immediately, so each already has a real parent here. The
        // cheap, unchecked `Tree::add_child` above is only ever correct
        // for a freshly-inserted node with no parent yet (confirmed via
        // direct read of its own doc comment) -- reusing it for an
        // already-attached node would leave it listed as a child of
        // *both* its old parent and the shell, real tree corruption
        // caught live by accesskit's own duplicate-child panic while
        // writing this example, not by any pytest test (none render a
        // real frame). `try_add_child` is the real, checked counterpart
        // that detaches first, the same mechanism `Node.add_child`'s
        // own pyo3 wrapper already uses.
        if let Some(menu_bar) = &menu_bar {
            tree.try_add_child(shell, menu_bar.id);
        }
        if let Some(toolbar) = &toolbar {
            tree.try_add_child(shell, toolbar.id);
        }

        let content = tree.insert(
            NodeKind::Container,
            Style {
                flex_grow: 1.0,
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(shell, content);

        if let Some(status_bar) = &status_bar {
            tree.try_add_child(shell, status_bar.id);
        }

        drop(tree);
        Ok(self.wrap_node(content))
    }

    /// M4 Phase 3, step 2 (§11.5): the missing Python-facing half of
    /// step 1's already-real drag mechanism -- until now, nothing created a
    /// `NodeKind::Splitter` from Python at all, so `Tree::dispatch`'s
    /// real drag handling (`Tree::update_drag`/`set_splitter_position`,
    /// reachable from a real mouse the moment such a node exists) had no
    /// way to actually be exercised by a Python app.
    ///
    /// Adds a child of this window's own root row, the same append-only
    /// way `add_rect` does -- called between two `add_rect` calls (left
    /// pane, splitter, right pane, in that order), it produces exactly
    /// the resizable-pane layout §11.5's own architecture text
    /// describes, with no separate "pane container" concept needed: the
    /// window's root row already *is* the flex parent `Tree::
    /// splitter_geometry` expects, holding the splitter directly between
    /// its two real flanking siblings.
    ///
    /// `background` matches `add_rect`'s own parameter shape exactly --
    /// a real splitter typically just wants a background for its own
    /// grip/handle (§11.5's own text: `SplitterState` carries no
    /// separate appearance data). `initial_position` (0.0..=1.0 along
    /// the split axis) defaults to an even 0.5 split.
    #[pyo3(signature = (background, width, height, initial_position=0.5))]
    fn add_splitter(
        &mut self,
        background: (u8, u8, u8, u8),
        width: f32,
        height: f32,
        initial_position: f64,
    ) -> Node {
        let (r, g, b, a) = background;
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Splitter(SplitterState {
                position: Animated::new(initial_position),
            }),
            Style {
                size: Size {
                    width: length(width),
                    height: length(height),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(r, g, b, a), 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        self.wrap_node(id)
    }
}
