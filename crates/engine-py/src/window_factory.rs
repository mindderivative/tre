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
    AccessNodeData, Action, Animated, ContentFit, ImageState, NodeId, NodeKind, PaintProperties,
    Role, TextAlign, TextFieldState, TextState, Tree,
};
use engine_render::MONOSPACE_FONT_FAMILY;
use peniko::Color;
use pyo3::prelude::*;
use taffy::prelude::{Size, length};

use crate::error::EngineError;
use crate::node::{Node, validate_rgba_frame_len};
use crate::terminal::TerminalSession;
use crate::window::{PyWindow, positioned_style};

/// M90: the one `orientation=` vocabulary shared by `add_divider`,
/// `add_scroll_view`, and `add_toolbar` -- returns whether it's
/// vertical. Replaces the old `vertical=`/`horizontal=` booleans.
fn parse_orientation(method: &str, orientation: &str) -> PyResult<bool> {
    match orientation {
        "horizontal" => Ok(false),
        "vertical" => Ok(true),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "{method}: unknown orientation {other:?} -- expected \"horizontal\" or \"vertical\""
        ))),
    }
}

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

/// M82: inserts an `Image` node from a ready `ImageState` --
/// `add_image_from_bytes` builds on it.
fn insert_image_node(
    tree: &mut Tree,
    root: NodeId,
    image_state: ImageState,
    width: f32,
    height: f32,
    x: Option<f32>,
    y: Option<f32>,
) -> NodeId {
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
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.add_child(root, id);
    id
}

/// M30 Phase 1 (§5, §7): a fully transparent fill -- `Rect`'s own real
/// "paint nothing" value (`border_paint.rs`'s own proof that `alpha:
/// 0` genuinely paints no pixels applies identically to `background`),
/// used by `Outlined`/`Text`'s real MD3 anatomy: neither variant has a
/// filled container at all, only `Outlined`'s real 1dp stroke or (for
/// `Text`) nothing but the label itself.
const TRANSPARENT: Color = Color::from_rgba8(0, 0, 0, 0);

#[pymethods]
impl PyWindow {
    /// §14 step 6's own "node creation" -- one shape (a colored rect, a
    /// child of this window's implicit root row) is the real minimal
    /// slice; unchanged by the `PyWindow` split, just moved here with
    /// `App` itself.
    /// M48 (§5, §7): `border_color`/`border_width` -- real `PaintProperties`
    /// fields since M30 Phase 1 -- were never settable anywhere in the
    /// imperative API, confirmed via grep before this change (only
    /// specific MD3 component factories, e.g. Outlined Button, ever
    /// baked a border in internally). `add_rect` is this file's one
    /// genuinely generic "colored box" factory (no `add_container`
    /// exists in the imperative API at all), so it's the representative
    /// case for this milestone; extending border kwargs to every other
    /// factory is a natural, separate follow-up.
    #[pyo3(signature = (background, width, height, x=None, y=None, border_color=None, border_width=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_rect(
        &self,
        background: (u8, u8, u8, u8),
        width: f32,
        height: f32,
        x: Option<f32>,
        y: Option<f32>,
        border_color: Option<(u8, u8, u8, u8)>,
        border_width: Option<f64>,
    ) -> Node {
        let (r, g, b, a) = background;
        let mut tree = self.tree.borrow_mut();
        let mut paint = PaintProperties::new(Color::from_rgba8(r, g, b, a), 0.0, 1.0);
        if let Some((br, bg, bb, ba)) = border_color {
            paint.border_color = Animated::new(Color::from_rgba8(br, bg, bb, ba));
        }
        if let Some(border_width) = border_width {
            paint.border_width = Animated::new(border_width);
        }
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
            paint,
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
    /// are required, the same as every other `add_*` method -- no
    /// measure-function/intrinsic-sizing wiring exists for `Text` to
    /// lean on instead, confirmed before choosing this shape rather than
    /// assumed.
    #[pyo3(signature = (content, foreground, width, height, font_family=None, font_weight=None, font_size=None, line_height=None, x=None, y=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_text(
        &self,
        content: &str,
        foreground: (u8, u8, u8, u8),
        width: f32,
        height: f32,
        font_family: Option<&str>,
        font_weight: Option<f32>,
        font_size: Option<f32>,
        line_height: Option<f32>,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        let background = foreground;
        // M99: the MD3 type-scale role (`typography_role`) went with
        // `engine-md3`; unset fields fall back to the old defaults.
        let resolved_family = font_family.unwrap_or("Roboto").to_string();
        let resolved_weight = font_weight.unwrap_or(400.0);
        let resolved_size = font_size.unwrap_or(16.0);
        let resolved_line_height = line_height;

        let (r, g, b, a) = background;
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Text(TextState {
                content: content.to_string(),
                font_family: resolved_family,
                font_weight: resolved_weight,
                font_size: resolved_size,
                align: TextAlign::Start,
                line_height: resolved_line_height,
                options: Default::default(),
            }),
            positioned_style(
                Size {
                    width: length(width),
                    height: length(height),
                },
                x,
                y,
            ),
            PaintProperties::new(Color::from_rgba8(r, g, b, a), 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        Ok(self.wrap_node(id))
    }

    /// M82: the real, decode-free image primitive -- takes already-decoded, straight
    /// -alpha RGBA8 pixels directly, the identical contract and
    /// validation `Node.push_frame` already established (that method's
    /// own doc comment: "the app decodes however it likes ... PyAV,
    /// OpenCV, a camera driver, frames generated on the fly"), just
    /// reachable at construction time under an honest `Image` name
    /// instead of the `add_video(...)` + `push_frame(...)` two-call
    /// workaround this replaces. `width`/`height` are the node's own
    /// fixed display box, `add_image`'s own identical existing contract;
    /// `pixel_width`/`pixel_height` describe `rgba` itself -- `fit`
    /// (`content_fit`) resolves any mismatch between the two at paint
    /// time, the identical real mechanism a pushed video frame of a
    /// different resolution than its node's box already relies on.
    /// M99 (D6): the only image constructor -- `add_image(path)` and the
    /// `image` crate went; the framework decodes.
    #[pyo3(signature = (rgba, pixel_width, pixel_height, width, height, fit="fill", x=None, y=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_image_from_bytes(
        &self,
        rgba: Vec<u8>,
        pixel_width: u32,
        pixel_height: u32,
        width: f32,
        height: f32,
        fit: &str,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        validate_rgba_frame_len(
            "add_image_from_bytes",
            rgba.len(),
            pixel_width,
            pixel_height,
        )?;
        let content_fit = parse_content_fit(fit)?;
        let image_data = peniko::ImageData {
            data: peniko::Blob::from(rgba),
            format: peniko::ImageFormat::Rgba8,
            alpha_type: peniko::ImageAlphaType::Alpha,
            width: pixel_width,
            height: pixel_height,
        };
        let mut image_state = ImageState::new(image_data);
        image_state.content_fit = content_fit;

        let mut tree = self.tree.borrow_mut();
        let id = insert_image_node(&mut tree, self.root, image_state, width, height, x, y);
        Ok(self.wrap_node(id))
    }

    /// M30 Phase 9 Step 1 (§5): creates a real `NodeKind::Image` node
    /// meant to be updated live via `Node.push_frame` -- see that
    /// method's own doc comment for the full real design (a "frame
    /// sink," not a decoder, directly grounded in the sibling
    /// `pyCopper` project's own real `Video` widget). No official MD3
    /// page exists for Video (confirmed via the same real directory-
    /// listing technique this milestone already uses throughout), and
    /// unlike `add_image` there is no file to load or decode here at
    /// all -- `width`/`height` are the node's own real, fixed box
    /// (exactly `add_image`'s own contract), initialized with a single
    /// fully-transparent placeholder pixel so the node paints as
    /// genuinely empty until the app's own first real `push_frame`
    /// call, the same real "nothing to show yet" contract `add_image`
    /// would have for pixel data if it allowed loading nothing. `fit`
    /// (`"cover"`/`"contain"`/`"fill"`, default `"fill"`) is `add_image`'s
    /// own identical real `ContentFit` parameter, reused verbatim --
    /// a pushed frame's own resolution is resolved against this node's
    /// fixed box the exact same way a loaded image's is.
    #[pyo3(signature = (width, height, fit="fill", x=None, y=None))]
    fn add_video(
        &self,
        width: f32,
        height: f32,
        fit: &str,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        let content_fit = parse_content_fit(fit)?;
        let mut image_state = ImageState::blank();
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
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
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
    // M71 (§5, §8): `multiline`/`show_whitespace` added -- the exact
    // two real `TextFieldState` fields `add_code_editor` already sets
    // internally (unconditionally, with no Python-facing equivalent),
    // confirmed the single real blocker to composing an equivalent
    // widget from Python (the sibling `Tesserae` project's own real
    // next milestone): every other real `add_code_editor` field
    // (`content`/`background`/`width`/`height`/`font_weight`/
    // `font_size`/`x`/`y`) was already a plain `add_text_field`
    // parameter. Both default `false`, the real, pre-existing
    // `add_text_field` behavior for every caller that doesn't pass
    // them -- a true no-op widening.
    // 0.3.1 review finding (architecture, High): `multiline`/`show_
    // whitespace` originally landed *before* the pre-existing trailing
    // `x`/`y` params, breaking positional-call compatibility with
    // every other 0.3.0 caller -- the one real inconsistency with this
    // branch's own established convention of always appending new
    // params at the end (`View::new`'s `source`/`spec`, `Node.set_
    // layout`'s `flex_direction`, both `instantiate`'s `source`, all
    // appended). Moved to the end to match.
    #[pyo3(signature = (background, width, height, content="", font_family="Roboto", font_weight=400.0, font_size=16.0, x=None, y=None, multiline=false, show_whitespace=false))]
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
        multiline: bool,
        show_whitespace: bool,
    ) -> Node {
        let (r, g, b, a) = background;
        // `TextFieldState`'s default text color is dark (`0x1C1B1F`); M99
        // removed theming, so nothing overrides it here.
        let mut text_field_state =
            TextFieldState::new(content, font_family, font_weight, font_size);
        text_field_state.multiline = multiline;
        text_field_state.show_whitespace = show_whitespace;
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
            PaintProperties::new(Color::from_rgba8(r, g, b, a), 0.0, 1.0),
        );
        tree.set_access(
            id,
            AccessNodeData::new(Role::TextInput).with_action(Action::Focus),
        );
        tree.add_child(self.root, id);
        self.wrap_node(id)
    }

    /// M30 Phase 9 Step 3 (§5, §8, §10): `Code Editor`, a real
    /// multiline `NodeKind::TextField` -- reuses `add_text_field`'s
    /// own exact real shape verbatim, differing only in `TextField
    /// State.multiline = true` (this step's own new field). No
    /// official MD3 page exists (confirmed via the same directory-
    /// listing technique this milestone already uses); designed from
    /// pyCopper's own real `CodeEditor` widget, checked directly, not
    /// assumed -- its own doc comment names the real reasons a genuine
    /// code editor is built alongside `TextField`/`TextFieldElement`
    /// rather than as a thin subclass of it (M3-specific chrome a code
    /// surface has none of), the same real split this step's own
    /// `multiline` flag honors instead by staying inside the one
    /// shared `NodeKind`, since `engine-core`'s own real editing model
    /// (cursor/selection/undo-free keyboard mutation) is already
    /// exactly what both need, unlike pyCopper's own M3-styled paint
    /// chrome, which this codebase's `TextField` doesn't have either.
    ///
    /// **Real, honestly-scoped v1 at this step -- since substantially
    /// widened, see M31 immediately below, not a full IDE-grade editor
    /// even now:** closed, at this step, the one genuinely load-bearing
    /// gap (real multiline editing: `Enter` inserts `\n`, `Home`/`End`
    /// operate per-line, `ArrowUp`/`ArrowDown` navigate by line
    /// preserving column -- `TextFieldState.multiline`'s own doc
    /// comment, `Tree::dispatch_text_field_key`'s own real logic).
    ///
    /// **M31 (§5, §8, §10), all 6 phases: real syntax highlighting, a
    /// real composed line-number gutter, and real Tab-key indentation
    /// capture all closed the three real gaps this doc comment used to
    /// name here as deliberately deferred -- corrected directly rather
    /// than left stale, confirmed by direct re-check of the live source
    /// before writing this correction, not assumed from the tracker
    /// alone.** Real per-token syntax coloring: `Node.set_syntax_spans`
    /// (`TextFieldState.syntax_spans`, app-side tokenization only --
    /// Design Principle 6, `engine-core` never interprets the ranges
    /// itself, the identical "no engine-bundled lexer" split pyCopper's
    /// own optional-Pygments design already established). Real
    /// line-number gutter: composed from existing primitives, not a
    /// new engine capability -- an ordinary sibling `Text` node with
    /// the editor's own identical `font_family`/`font_weight`/
    /// `font_size` lines up with the editor's own real per-line Y
    /// positions *by construction*, both built through the identical
    /// real `shaped_layout` path; `examples/code_editor_gutter.py` has
    /// the real, working, live-updating pattern to copy. Real Tab-key
    /// indentation capture: a focused *multiline* field now claims
    /// `Tab` before ordinary focus traversal and inserts a literal
    /// `\t` (`Tree::dispatch_text_field_key`'s own real "first refusal"
    /// contract, widened) -- a single-line `TextField` keeps its prior
    /// real behavior byte-for-byte, `Tab` still moves focus there.
    /// M31 also added real visible space/tab glyphs (`show_whitespace`,
    /// set `true` below) and real code folding (`Node.set_folded_
    /// ranges`) -- see each field's own doc comment for the full real
    /// design. **Still real, stated, deliberately deferred, matching
    /// pyCopper's own identical v1 scope for the identical reasons:**
    /// multi-cursor editing, a minimap, bracket auto-closing/matching,
    /// and any language-server (LSP) integration -- "an LSP client is
    /// an *application* concern, never a widget's own hard dependency"
    /// (`BUILD_TRACKER.md`'s own M31 closing note).
    ///
    /// **M38 Phase 7 (§5, §8): real vertical scroll+clip for content
    /// taller than the box, with real caret-follow, closes the gap
    /// this doc comment used to name here.** `TextFieldState.scroll_
    /// offset`'s own doc comment has the full real design (a dedicated
    /// mechanism, not `ScrollView` wrapping -- scoped via
    /// `AskUserQuestion` since that would need real `taffy` measure-
    /// function integration this codebase has no precedent for).
    /// **M39 Phase 1 (§5, §8): real horizontal scroll+clip for a line
    /// wider than the box, with real horizontal caret-follow, closes
    /// the real, separate gap this doc comment used to name here
    /// too.** `TextFieldState.horizontal_scroll_offset`'s own doc
    /// comment has the full real design -- the identical dedicated-
    /// mechanism shape, parallel to `scroll_offset`'s own vertical one.
    ///
    /// **M32 Phase 1 (§5, §8, §10):** always shapes with the real
    /// bundled monospace face (`engine_render::MONOSPACE_FONT_FAMILY`,
    /// "Hack Nerd Font Mono") rather than the general-purpose `Roboto`
    /// this step originally had to fall back to -- a genuinely monospace
    /// editor at last, not an approximation. `font_family` still isn't
    /// exposed as a param here (unlike `add_text_field`): a code editor
    /// composed from a proportional face would defeat the whole point
    /// of this widget class, so there is still only one real correct
    /// choice, now a bundled one rather than an absent one.
    #[pyo3(signature = (content, background, width, height, font_weight=400.0, font_size=14.0, x=None, y=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_code_editor(
        &self,
        content: &str,
        background: (u8, u8, u8, u8),
        width: f32,
        height: f32,
        font_weight: f32,
        font_size: f32,
        x: Option<f32>,
        y: Option<f32>,
    ) -> Node {
        let (r, g, b, a) = background;
        let mut text_field_state =
            TextFieldState::new(content, MONOSPACE_FONT_FAMILY, font_weight, font_size);
        text_field_state.multiline = true;
        // M31 Phase 3 (§5, §8): a real Code Editor shows space/tab as
        // visible glyphs by default -- the same real convention every
        // real code editor (VS Code, Sublime Text) already has for
        // this exact widget class, unlike an ordinary `add_text_field`
        // form input, where a visible indicator would be real, unasked-
        // for visual noise.
        text_field_state.show_whitespace = true;
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
            PaintProperties::new(Color::from_rgba8(r, g, b, a), 0.0, 1.0),
        );
        tree.set_access(
            id,
            AccessNodeData::new(Role::TextInput).with_action(Action::Focus),
        );
        tree.add_child(self.root, id);
        self.wrap_node(id)
    }

    /// M30 Phase 9 Step 4 (§5, §8, §10): `Terminal`, a real, live
    /// pseudo-terminal -- spawns `shell` on a real PTY (`portable_pty`)
    /// and parses its real byte stream with a real VT100 parser
    /// (`vt100`), the identical real "spawn a real pseudo-terminal is
    /// OS-specific process management; interpreting its byte stream is
    /// the VT/ANSI state machine every real terminal emulator
    /// implements identically -- neither is this widget's own concern
    /// to reinvent" split the sibling `pyCopper` project's own real
    /// `Terminal` widget already established (`crates/engine-py/src/
    /// terminal.rs`'s own module doc comment has the full real design).
    /// No official MD3 page exists (confirmed via the same directory-
    /// listing technique this milestone already uses).
    ///
    /// `width`/`height` are computed from `cols`/`rows` via the real
    /// bundled monospace face's own measured cell size (M32 Phase 1,
    /// §5, §8, §10: `TextRenderer::monospace_cell_size`, replacing the
    /// old `engine_core::terminal_cell_size` analytic estimate) -- the
    /// identical real per-cell grid `engine-render`'s own `draw_
    /// terminal` positions every cell on, so the node's own box always
    /// exactly fits its own real grid, no manual size bookkeeping for
    /// the app. **Real, confirmed POSIX-only v1**, the identical real
    /// scope pyCopper's own `Terminal` already chose for the same
    /// stated reason (`terminal.rs`'s own doc comment).
    ///
    /// M32 Phase 5 (§4, §8): `scrollback_lines` is a real, retained
    /// history length (`0` for none) -- `Node.scroll_terminal`/a real
    /// mouse wheel over a focused terminal move the viewport into it.
    #[pyo3(signature = (shell, cols, rows, background, font_size=14.0, scrollback_lines=1000, x=None, y=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_terminal(
        &self,
        shell: &str,
        cols: u16,
        rows: u16,
        background: (u8, u8, u8, u8),
        font_size: f32,
        scrollback_lines: usize,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        let session =
            TerminalSession::spawn(shell, cols, rows, scrollback_lines).map_err(|reason| {
                EngineError::TerminalSpawnFailed {
                    shell: shell.to_string(),
                    reason,
                }
            })?;

        let (r, g, b, a) = background;
        // M32 Phase 1 (§5, §8, §10): measures the real bundled monospace
        // face -- M96: on the thread's shared shaper (`shaper.rs`), not a
        // fresh `TextRenderer` per call.
        let (cell_width, cell_height) = crate::shaper::with(|shaper| {
            shaper.monospace_cell_size(MONOSPACE_FONT_FAMILY, font_size)
        });
        let width = cell_width * f32::from(cols);
        let height = cell_height * f32::from(rows);

        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Terminal(engine_core::TerminalState::new(
                cols,
                rows,
                MONOSPACE_FONT_FAMILY,
                font_size,
            )),
            positioned_style(
                Size {
                    width: length(width),
                    height: length(height),
                },
                x,
                y,
            ),
            PaintProperties::new(Color::from_rgba8(r, g, b, a), 0.0, 1.0),
        );
        tree.set_access(
            id,
            AccessNodeData::new(Role::TextInput).with_action(Action::Focus),
        );
        tree.add_child(self.root, id);
        drop(tree);

        self.terminals.borrow_mut().insert(id, session);
        Ok(self.wrap_node(id))
    }

    /// M32 Phase 1 (§5, §8, §10): the real per-`font_size` cell size
    /// (`width`, `height`) of the bundled monospace face (`Hack Nerd
    /// Font Mono`) that `add_terminal`/`add_code_editor` themselves now
    /// use internally -- exposed here so app-level layout code (a
    /// gutter's own per-line click target, `examples/code_editor_
    /// folding.py`'s own toggle affordance) can size against the exact
    /// real value actually painted, rather than reaching for its own
    /// approximation the way `LINE_HEIGHT = FONT_SIZE * 1.3` (that
    /// example's own prior real placeholder, `terminal_cell_size`'s own
    /// now-removed doc comment) had to before a real font existed to
    /// measure.
    fn get_monospace_cell_size(&self, font_size: f32) -> (f32, f32) {
        crate::shaper::with(|shaper| shaper.monospace_cell_size(MONOSPACE_FONT_FAMILY, font_size))
    }

    /// M33 Phase 1 (§4, §5, §8): resizes a real, live `Terminal`'s own
    /// grid -- the real capability `TerminalSession::resize` (spawned
    /// at M30 Phase 9 Step 4, kept `#[allow(dead_code)]` until this
    /// phase actually needed it) always had, finally given a real
    /// caller. Resizes the real kernel-level PTY and the `vt100`
    /// parser's own screen buffer (`TerminalSession::resize`'s own doc
    /// comment has the full real reasoning, including the one real,
    /// honest shell-redraw caveat), then recomputes this node's own
    /// real box from `cols`/`rows` the identical way `add_terminal`
    /// itself does at construction time, via `Tree::set_layout_style`
    /// (the one real, correct way to push a style update back into
    /// `taffy`, confirmed the hard way by M32 Phase 2's own real bug:
    /// mutating `layout_style` directly desyncs taffy's own internal
    /// copy). Raises `ValueError` if `node` isn't a real `Terminal`
    /// this `Window` created.
    fn resize_terminal(&self, node: PyRef<'_, Node>, cols: u16, rows: u16) -> PyResult<()> {
        if !Rc::ptr_eq(&self.tree, &node.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        let (font_family, font_size) = {
            let tree = self.tree.borrow();
            match tree.get(node.id).map(|n| &n.kind) {
                Some(NodeKind::Terminal(state)) => (state.font_family.clone(), state.font_size),
                _ => return Err(EngineError::NotATerminal.into()),
            }
        };
        if !self.terminals.borrow().contains_key(&node.id) {
            return Err(EngineError::NotATerminal.into());
        }

        let (cell_width, cell_height) =
            crate::shaper::with(|shaper| shaper.monospace_cell_size(&font_family, font_size));
        let width = cell_width * f32::from(cols);
        let height = cell_height * f32::from(rows);
        {
            let mut tree = self.tree.borrow_mut();
            let mut style = tree.get(node.id).unwrap().layout_style.clone();
            style.size = Size {
                width: length(width),
                height: length(height),
            };
            tree.set_layout_style(node.id, style);
        }

        if let Some(session) = self.terminals.borrow_mut().get_mut(&node.id) {
            session.resize(&mut self.tree.borrow_mut(), node.id, cols, rows);
        }
        Ok(())
    }

    /// M36 Phase 1 (§5, §7, §11.7): a real, general scrollable viewport
    /// over exactly one child -- see `ScrollViewState`'s own doc
    /// comment for the full real design (grounded directly in the
    /// sibling `pyCopper` project's own `ScrollViewElement`). Returns
    /// a plain container `Node`; the caller composes their own real
    /// content in via the existing, generic `Node.add_child` (M6
    /// Phase 1), the identical real "engine provides the primitive,
    /// app composes" split `add_toolbar` (M35 Phase 1) already
    /// established for the same real reason -- a real content node
    /// (often a flex column of many rows) needs its own real, explicit
    /// size on the scroll axis matching its own true content extent,
    /// the same "caller supplies a real explicit size" convention
    /// every other `add_*` factory in this codebase already has; this
    /// call has no opinion about what that content actually is.
    /// `horizontal=false` (the default) scrolls vertically; `true`
    /// scrolls horizontally -- never both at once, the identical
    /// single-axis-at-a-time real scope `ScrollViewState`'s own doc
    /// comment already states. Real wheel scrolling and `Window.
    /// scroll` both already work with zero other changes: `ScrollView`
    /// joins `Tree::dispatch`'s existing "walk up to the nearest
    /// scrollable ancestor" mechanism the same way `VirtualList`/
    /// `Carousel` already do.
    #[pyo3(signature = (width, height, orientation="vertical", x=None, y=None))]
    fn add_scroll_view(
        &self,
        width: f32,
        height: f32,
        orientation: &str,
        x: Option<f32>,
        y: Option<f32>,
    ) -> PyResult<Node> {
        let horizontal = !parse_orientation("add_scroll_view", orientation)?;
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::ScrollView(engine_core::ScrollViewState::new(horizontal)),
            positioned_style(
                Size {
                    width: length(width),
                    height: length(height),
                },
                x,
                y,
            ),
            PaintProperties::new(TRANSPARENT, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        Ok(self.wrap_node(id))
    }
}
