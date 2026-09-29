//! §5's core data model: `NodeId`, `Node`, `NodeKind`, `PaintProperties`.
//!
//! `NodeKind` is the engine's small set of primitives -- a plain
//! `Container` plus what `window.create` names "box", "text",
//! "text_input", "image", "path", "canvas", "scroll_view",
//! "virtual_list", and "terminal". Anything richer (a checkbox, a menu,
//! a dialog) is a framework's to build from these; `engine-core` has no
//! design system of its own.

use std::time::Instant;

use crate::canvas::CanvasState;

use peniko::Color;
use taffy::Style;

use crate::animation::{Animated, Interpolate};

slotmap::new_key_type! {
    /// A generational index (slot index + reuse generation), matching
    /// §5's own `NodeId { index: u32, generation: u32 }` doc comment
    /// exactly in spirit: a stale `NodeId` from a removed node fails a
    /// checked generation comparison instead of silently resolving to
    /// whatever now occupies that slot (§1 Locked Decisions).
    ///
    /// Built on `slotmap`'s key type rather than a hand-rolled struct:
    /// `taffy` (this crate's own dependency) already pulls in `slotmap`
    /// transitively at the exact same version, so this costs no new
    /// entry in the dependency graph, and `SlotMap`/`SecondaryMap`'s
    /// generation-checked `get`/`remove` are exactly the semantics §5
    /// asks for.
    pub struct NodeId;
}

/// Component-specific animatable state lives here, per kind -- §1 Locked
/// Decisions ("common core + per-kind payload"). `Rect`/`Container`
/// carry no payload beyond `PaintProperties`; `Text` carries `TextState`
/// (§14 step 4).
/// Doesn't derive `Clone`/`Debug`/`PartialEq`: several kinds carry an
/// `Animated<f64>` -- `Animated<T>` implements none of those
/// (same reason `PaintProperties`, also full of `Animated` fields,
/// never derived them either); nothing in this codebase actually
/// cloned, printed, or compared a `NodeKind` value directly (checked
/// directly, not assumed), so this costs nothing real.
pub enum NodeKind {
    Rect,
    Container,
    Text(TextState),
    /// §14 step 15 (§11.7): a windowed logical list -- only the small
    /// visible-window subset in `VirtualListState::materialized` are
    /// ever real `Node`s, regardless of `item_count`.
    VirtualList(VirtualListState),
    /// M5 Phase 3 (§11.10, §11.11): custom-drawn content plus an
    /// optional custom hit-test override. `CanvasState` is plain, inert
    /// data -- no `Py<PyAny>` -- resolved ahead of time by
    /// `Tree::set_canvas_content`, not computed live during paint or
    /// hit-testing (see `canvas.rs`'s own module doc comment for why).
    Canvas(CanvasState),
    /// M15 Phase 1 (§5, §16.7): a real, single-line editable text
    /// field. `content`/`cursor`/`selection_anchor` are plain, engine-
    /// core-native byte-offset state, and the *engine* is the one real
    /// mutator (via `Tree::dispatch`'s own keyboard-editing arm, M15
    /// Phase 2), since typing is mechanical, not app-defined meaning
    /// (Design Principle 6). Deliberately does *not* carry a
    /// `parley::Layout`/`Selection` directly: `engine-core` has no
    /// `parley` dependency at all (§4's crate-boundary rule) --
    /// `engine-render` reconstructs both, each frame, purely to compute
    /// real caret/highlight paint geometry, the same "engine-core holds
    /// inert data, engine-render re-derives it" split `TextState`
    /// itself already uses.
    TextField(TextFieldState),
    /// M22 Phase 1 (§5): decoded pixels, painted as an external GPU
    /// texture (`engine-render`'s `image_cache`) -- see
    /// `ImageState`'s own doc comment for the real crate-boundary
    /// reasoning (decoding lives in `engine-py`, this holds only the
    /// already-decoded result).
    Image(ImageState),
    /// M95 (D4): any vector path -- fill, stroke, trim, morph.
    Path(crate::path::PathState),
    /// M30 Phase 9 Step 4 (§5, §8, §10): a real, live terminal -- see
    /// `TerminalState`'s own doc comment for the real crate-boundary
    /// reasoning (the identical "engine-core holds inert data,
    /// engine-render re-derives paint geometry" split `TextFieldState`
    /// already established, extended to a whole cell grid instead of
    /// one string).
    Terminal(TerminalState),
    /// M36 Phase 1 (§5, §7, §11.7): a real, general scrollable
    /// viewport over one oversized child -- see `ScrollViewState`'s
    /// own doc comment for the real design (grounded directly in the
    /// sibling `pyCopper` project's own `ScrollViewElement`) and
    /// `Tree::sync_scroll_view_layouts` for how the real scroll offset
    /// is made real against `taffy` without the hit-test-after-scroll
    /// gap this phase's own investigation found in `VirtualList`.
    ScrollView(ScrollViewState),
}

/// M30 Phase 9 Step 4 (§5, §8, §10): one real, already-VT-interpreted
/// terminal cell -- a single real character plus its own real
/// foreground/background color and bold attribute, the identical real
/// "what a genuine VT/ANSI parser hands back" shape the sibling
/// `pyCopper` project's own real `Terminal` widget (grounded in
/// `bittty`'s own `Cell` type) already established.
///
/// **M39 Phase 4 (§5, §7): widened with `dim`/`italic`/`underline`/
/// `inverse`** -- direct source read of the vendored `vt100 = "0.16.2"`
/// crate confirmed `vt100::Cell` already exposes all four as real,
/// already-parsed booleans (`bold()`/`dim()`/`italic()`/`underline()`/
/// `inverse()`), so this is pure plumbing: `engine-py::terminal.rs`'s
/// own real VT-to-`TerminalCell` translation now reads them too.
/// **Strikethrough stays a real, stated v1 omission, not an oversight**
/// -- confirmed via grep that `vt100`'s own source has zero real
/// strikethrough support anywhere (no such attribute is even parsed
/// from the byte stream), a genuine constraint of the vendored VT
/// parser itself, not an engine-side choice; the identical real scope
/// pyCopper's own `Terminal` already chose for the same real reason
/// ("underline and strikethrough rendering... out of scope for this
/// pass" -- pyCopper's own comment predates this project's own direct
/// confirmation that strikethrough specifically has no real source
/// data to render in the first place).
/// M95: a terminal cell's color as the program set it -- the palette's
/// own foreground/background, one of its 256 indexed colors, or an exact
/// RGB value. Resolved against `TerminalState::palette` when painted, so
/// changing the palette recolors what's already on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CellColor {
    Default,
    Indexed(u8),
    Rgb(Color),
}

/// M95: every color a terminal paints -- the 16 ANSI colors (the rest of
/// the 256 are the standard cube and grey ramp), the default foreground
/// and background, the cursor, and the selection highlight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalPalette {
    pub ansi: [Color; 16],
    pub foreground: Color,
    pub background: Color,
    pub cursor: Color,
    pub selection: Color,
}

impl Default for TerminalPalette {
    fn default() -> Self {
        const ANSI_16: [(u8, u8, u8); 16] = [
            (28, 28, 33),
            (222, 89, 89),
            (140, 191, 102),
            (217, 178, 89),
            (102, 153, 230),
            (191, 128, 217),
            (102, 191, 204),
            (204, 204, 209),
            (102, 107, 117),
            (242, 115, 115),
            (166, 217, 128),
            (242, 204, 115),
            (140, 178, 242),
            (217, 153, 242),
            (140, 217, 230),
            (242, 242, 247),
        ];
        Self {
            ansi: ANSI_16.map(|(r, g, b)| Color::from_rgba8(r, g, b, 0xFF)),
            foreground: Color::from_rgba8(0x1C, 0x1B, 0x1F, 0xFF),
            background: Color::TRANSPARENT,
            cursor: Color::from_rgba8(0x1C, 0x1B, 0x1F, 0x80),
            selection: Color::from_rgba8(0x1C, 0x1B, 0x1F, 0x4D),
        }
    }
}

impl TerminalPalette {
    /// One of the 256 indexed colors: the palette's 16, then the 6x6x6
    /// color cube, then the 24-step grey ramp.
    pub fn indexed(&self, index: u8) -> Color {
        if let Some(&color) = self.ansi.get(usize::from(index)) {
            return color;
        }
        if index < 232 {
            let n = index - 16;
            let levels = [0u8, 95, 135, 175, 215, 255];
            return Color::from_rgba8(
                levels[usize::from(n / 36)],
                levels[usize::from((n / 6) % 6)],
                levels[usize::from(n % 6)],
                0xFF,
            );
        }
        let level = (8 + u16::from(index - 232) * 10).min(255) as u8;
        Color::from_rgba8(level, level, level, 0xFF)
    }

    pub fn foreground_of(&self, color: CellColor) -> Color {
        match color {
            CellColor::Default => self.foreground,
            CellColor::Indexed(index) => self.indexed(index),
            CellColor::Rgb(color) => color,
        }
    }

    pub fn background_of(&self, color: CellColor) -> Color {
        match color {
            CellColor::Default => self.background,
            CellColor::Indexed(index) => self.indexed(index),
            CellColor::Rgb(color) => color,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalCell {
    pub ch: char,
    pub fg: CellColor,
    pub bg: CellColor,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
}

impl TerminalCell {
    /// A real blank cell -- a literal space, transparent background
    /// (so `engine-render`'s own paint arm can skip drawing a
    /// background rect for it entirely, the same "skip painting a
    /// real default" convention other `NodeKind`s already use), the
    /// default foreground `engine-render`'s own paint code resolves
    /// against the terminal's own real base ink color instead of a
    /// hardcoded one here (`engine-core` has no design-system
    /// awareness, §4).
    pub fn blank() -> Self {
        Self {
            ch: ' ',
            fg: CellColor::Default,
            bg: CellColor::Default,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            inverse: false,
        }
    }
}

/// M30 Phase 9 Step 4 (§5, §8, §10): a real, live terminal's own
/// current cell-grid snapshot -- engine-core holds only this inert
/// data (`cells`, row-major, `cols * rows` long, the real "already-
/// VT-interpreted" state a real PTY/VT100 pipeline produces), the
/// same real crate-boundary split `TextFieldState`'s own doc comment
/// already established for a single editable string: real PTY process
/// spawning and real VT/ANSI byte-stream parsing are `engine-py`'s own
/// real concern (§4 -- neither belongs in a pure, OS-and-parser-
/// agnostic `engine-core`), which rebuilds this whole struct's own
/// `cells`/`cursor_*` fields wholesale whenever the real terminal
/// screen changes (the identical real "wholesale replacement, not
/// incremental diffing" simplicity an image node's `rgba` frames
/// already use for a comparable "engine only displays the latest
/// snapshot a real external process produced" shape).
#[derive(Clone, Debug, PartialEq)]
pub struct TerminalState {
    pub cols: u16,
    pub rows: u16,
    pub cells: Vec<TerminalCell>,
    pub cursor_col: u16,
    pub cursor_row: u16,
    pub cursor_visible: bool,
    pub font_family: String,
    pub font_size: f32,
    /// M32 Phase 6 (§4, §5, §8): a real mouse-drag text selection over
    /// this terminal's own cell grid -- `(row, col)`, the anchor where
    /// a real press started. `None` (every terminal, until a real
    /// press) means no selection, the same "off unless a caller opts
    /// in" contract every other additive field here already
    /// follows. `engine-core` never interprets these coordinates itself
    /// beyond clamping/normalizing (`Tree::terminal_selected_text`) --
    /// deciding *where* a real pointer press landed needs real font
    /// metrics only `engine-render` has (§4), so `engine-py` is the one
    /// place that ever writes a real value here.
    pub selection_start: Option<(u16, u16)>,
    /// The other real endpoint of the drag -- moves with every real
    /// `PointerMoved` while the drag is active; stays put once a real
    /// `PointerReleased` ends it, so the selection visibly persists
    /// until a new press starts one (or clears it).
    pub selection_end: Option<(u16, u16)>,
    /// M95: the colors this terminal paints.
    pub palette: TerminalPalette,
}

impl TerminalState {
    /// M96: resizes the grid to `cols` x `rows`, keeping the cells that
    /// still fit where they were and blanking the rest -- the terminal's
    /// session catches its PTY up at its next drain.
    pub fn resize_grid(&mut self, cols: u16, rows: u16) {
        let mut cells = vec![TerminalCell::blank(); usize::from(cols) * usize::from(rows)];
        for row in 0..rows.min(self.rows) {
            for col in 0..cols.min(self.cols) {
                cells[usize::from(row) * usize::from(cols) + usize::from(col)] =
                    self.cells[usize::from(row) * usize::from(self.cols) + usize::from(col)];
            }
        }
        self.cells = cells;
        self.cols = cols;
        self.rows = rows;
        self.cursor_col = self.cursor_col.min(cols.saturating_sub(1));
        self.cursor_row = self.cursor_row.min(rows.saturating_sub(1));
    }

    /// Seeds a real, fully blank `cols * rows` grid -- the real
    /// "nothing to show yet" state before the app's own first real PTY
    /// bytes ever arrive, the identical real placeholder-first-frame
    /// contract `Video`'s own `ImageState` placeholder already
    /// establishes for a comparable "displays whatever a real external
    /// process produced" shape.
    pub fn new(cols: u16, rows: u16, font_family: impl Into<String>, font_size: f32) -> Self {
        let cell_count = usize::from(cols) * usize::from(rows);
        Self {
            cols,
            rows,
            cells: vec![TerminalCell::blank(); cell_count],
            cursor_col: 0,
            cursor_row: 0,
            cursor_visible: true,
            font_family: font_family.into(),
            font_size,
            selection_start: None,
            selection_end: None,
            palette: TerminalPalette::default(),
        }
    }

    /// The real cell at `(row, col)`, `0`-indexed -- panics on an
    /// out-of-bounds `row`/`col`, the same real "internal bookkeeping
    /// bug, not a recoverable runtime condition" contract every other
    /// direct-index accessor in this crate already has (`Tree::get_
    /// mut`'s own doc comment, for one).
    pub fn cell(&self, row: u16, col: u16) -> &TerminalCell {
        &self.cells[usize::from(row) * usize::from(self.cols) + usize::from(col)]
    }
}

/// M36 Phase 1 (§5, §7, §11.7): a real, general scrollable viewport
/// over exactly one child, grounded directly in the sibling `pyCopper`
/// project's own real `ScrollViewElement` (`widgets/scroll.py`) --
/// single-axis (never simultaneous 2D scroll, the identical real
/// scope pyCopper's own design already settled on), the child
/// measured against unbounded space on that one axis so it reports
/// its own true content extent, and the real scroll offset applied as
/// a pure "where does the content start" value.
///
/// **Real, deliberate design choice, not accidental:** unlike
/// `VirtualListState::scroll_offset` (applied only as an extra
/// `engine-render's paint walk` translate, never reflected back into
/// `layout_style` -- this phase's own investigation found that gives
/// a real, previously undiscovered hit-test-after-scroll bug, a real
/// point at a scrolled item's own genuine post-scroll screen position
/// resolves to the wrong node), `scroll` here is turned into a real,
/// baked-in absolute `layout_style.inset` by `Tree::sync_scroll_view_
/// layouts` every frame, which both `engine-render's paint walk` and
/// `Tree::hit_test_at` read correctly by construction, since neither needs a second, separate transform
/// to agree with. Plain `Animated<f64>`, driven directly (never
/// through `animate_field`/central ticking), the identical real
/// precedent `VirtualListState::scroll_offset`'s own doc comment
/// already establishes -- confirmed there via a direct read of `Tree::
/// tick_all`: no kind-specific `Animated<T>` field is ever ticked
/// centrally, only by its own dedicated mechanism (`Tree::
/// scroll_scroll_view_by`, mirroring `scroll_virtual_list_by`), so
/// this follows that same real precedent rather than inventing a new
/// one; `Animated<f64>` is used here purely for its own `.current`/
/// `Interpolate` convenience, not because this value is ever eased.
/// No `#[derive(Clone, Debug, PartialEq)]` -- `Animated<T>`
/// implements none of those, the same real reason `NodeKind`'s own
/// doc comment states.
pub struct ScrollViewState {
    pub scroll: Animated<f64>,
    /// `false` (the default) scrolls vertically; `true` scrolls
    /// horizontally. Never both at once -- the identical real
    /// single-axis scope pyCopper's own `ScrollViewElement.axis`
    /// already settled on, not a limitation this phase introduces.
    pub horizontal: bool,
    /// M38 Phase 6 (§5, §7, §11.7): `Some((pointer_coord, scroll_at_
    /// start))`, set the instant a real thumb drag begins and cleared
    /// when it ends -- `Tree::update_scroll_view_thumb_drag`'s own
    /// real anchor, ported directly from pyCopper's own `state.data
    /// ["drag_from"]`/`["drag_scroll"]` (`widgets/scroll.py`'s own
    /// `on_pointer_down`/`on_pointer_move`). A *relative*-delta anchor,
    /// not an absolute pointer-to-scroll mapping: grabbing the thumb
    /// anywhere along its own length must not snap it so that point
    /// jumps under the pointer, the identical real UX pyCopper's own
    /// design already chose and this ports verbatim. Lives on this
    /// state (not `Tree` itself): kind-specific drag anchor data belongs
    /// to the kind, `Tree.dragging` alone only ever names *which* node
    /// is being dragged.
    pub thumb_drag_anchor: Option<(f64, f64)>,
    /// M95: the scrollbar thumb's color; `None` paints the default.
    pub scrollbar_fill: Option<Color>,
    /// M95: the scrollbar thumb's thickness -- painted and hit-tested.
    pub scrollbar_width: f64,
}

impl ScrollViewState {
    pub fn new(horizontal: bool) -> Self {
        Self {
            scroll: Animated::new(0.0),
            horizontal,
            thumb_drag_anchor: None,
            scrollbar_fill: None,
            scrollbar_width: SCROLLBAR_THICKNESS,
        }
    }

    /// M38 Phase 6 (§5, §7, §11.7): real thumb geometry `(track, thumb,
    /// along)` -- ported directly from pyCopper's own real
    /// `ScrollViewElement.thumb_geometry` (`widgets/scroll.py`), shared
    /// by painting (`engine-render's paint walk`) and hit-testing/
    /// dragging (`Tree::grabs_scroll_view_thumb`/`update_scroll_view_
    /// thumb_drag`) so the two can never drift -- the identical real
    /// "one function, every real caller" discipline `VirtualListState::
    /// offset_of` already establishes.
    /// `viewport_extent`/`content_extent` are the real, live measured
    /// sizes along this view's own scroll axis (`Tree::sync_scroll_
    /// view_layouts`'s own already-computed values, recomputed here
    /// too rather than cached, since neither crate can hold the
    /// other's own cross-frame state).
    pub fn thumb_geometry(&self, viewport_extent: f64, content_extent: f64) -> (f64, f64, f64) {
        let track = viewport_extent - SCROLLBAR_MARGIN * 2.0;
        if track <= 0.0 {
            return (0.0, 0.0, 0.0);
        }
        let max_scroll = (content_extent - viewport_extent).max(0.0);
        let thumb = if content_extent > 0.0 {
            (track * (viewport_extent / content_extent)).max(SCROLLBAR_MIN_LENGTH)
        } else {
            track
        };
        let thumb = thumb.min(track);
        let progress = if max_scroll > 0.0 {
            self.scroll.current / max_scroll
        } else {
            0.0
        };
        let along = SCROLLBAR_MARGIN + (track - thumb) * progress;
        (track, thumb, along)
    }
}

/// M38 Phase 6 (§5, §7, §11.7): real scrollbar geometry tokens -- M3
/// has no real scrollbar spec at all (pyCopper's own `scroll.py`
/// module doc comment states this directly: "the catalogue mentions
/// only that a scrolling menu 'shows a persistent scrollbar'"), so
/// these are pyCopper's own real, cited values, ported verbatim rather
/// than presented as an MD3 token. `SCROLLBAR_THICKNESS`/`SCROLLBAR_
/// GRAB_SLOP` live here (not `engine-render`) because `Tree::grabs_
/// scroll_view_thumb`'s own real hit-test needs them too, not just
/// paint -- `SCROLLBAR_MARGIN`/`SCROLLBAR_MIN_LENGTH` are used by
/// `thumb_geometry` above directly, the shared real geometry both
/// painting and hit-testing read.
pub const SCROLLBAR_THICKNESS: f64 = 4.0;
pub const SCROLLBAR_MARGIN: f64 = 2.0;
pub const SCROLLBAR_MIN_LENGTH: f64 = 32.0;
/// How far either side of the real thumb still counts as grabbing it
/// -- a 4dp target is unusable with a real mouse, let alone a
/// trackpad (pyCopper's own real reasoning, `THUMB_GRAB_SLOP`'s own
/// doc comment, quoted directly).
pub const SCROLLBAR_GRAB_SLOP: f64 = 6.0;

/// M15 Phase 1 (§5, §16.7): mirrors `TextState`'s own four font/content
/// fields exactly (so `engine-render`'s own layout-building code can be
/// shared between the two), plus real editable-field state. `cursor`/
/// `selection_anchor` are plain UTF-8 *byte* offsets into `content`,
/// not char or grapheme-cluster indices -- `String` slicing/`char_
/// indices` are what M15 Phase 2's own real keyboard-editing mutation
/// uses to keep every offset on a real UTF-8 boundary; `engine-core`
/// itself never validates this beyond what those std APIs already
/// guarantee, since `content` is never sliced at an arbitrary offset
/// here, only ever at boundaries `char_indices` itself produced.
///
/// M38 Phase 7 (§5, §8): no longer derives `Clone`/`Debug`/`PartialEq`
/// -- `scroll_offset: Animated<f64>` implements none of those, the
/// identical real reason `ScrollViewState`'s own doc comment already
/// states for its own `Animated<T>` field.
pub struct TextFieldState {
    pub content: String,
    pub font_family: String,
    pub font_weight: f32,
    pub font_size: f32,
    /// A real UTF-8 byte offset into `content`, `0..=content.len()`.
    pub cursor: usize,
    /// `Some(byte_offset)` when a real selection is active (`cursor`
    /// is the selection's own "focus" end, this is its "anchor" end,
    /// the identical two-endpoint shape `parley::editing::Selection`
    /// itself uses) -- `None` (the default) means no selection, the
    /// overwhelmingly common case for a freshly-created field.
    pub selection_anchor: Option<usize>,
    /// M17 Phase 2 (§8): a real, *uncommitted* IME composition preview
    /// -- `Some` while an input method is composing (e.g. pinyin
    /// candidates before the user picks one), `None` otherwise.
    /// Deliberately not spliced into `content` itself: nothing is
    /// really "typed" until a real `winit::event::Ime::Commit`, which
    /// reaches the exact same `TextInput` mechanism a plain keypress
    /// already uses (M15 Phase 2) -- this field exists purely so
    /// `engine-render` can paint the real, visible in-progress preview
    /// (with a real underline) without ever mutating real content for
    /// text that might still be revised or cancelled mid-composition.
    /// Drops `winit::event::Ime::Preedit`'s own real sub-cursor range
    /// (`Option<(usize, usize)>`, where *within* the preedit text the
    /// composition cursor sits) -- real, but strictly more detail than
    /// "a preedit underline" needs; a real, stated simplification.
    pub preedit: Option<String>,
    /// M20 Phase 2 (§7.1, §7.3): the field's own real text/caret/
    /// selection-highlight color. Defaults to real, byte-for-byte
    /// the historical hardcoded `0x1C1B1F` `engine-render`'s own
    /// `TextField` paint used before this phase.
    pub text_tint: Animated<Color>,
    /// M30 Phase 9 Step 3 (§8, §10): `false` (the default, every
    /// existing construction site's own byte-for-byte unchanged
    /// behavior) is the original real, stated single-line scope --
    /// `Enter` is consumed but never inserts a newline. `true` (set
    /// directly, a plain `pub` field write -- no `new()` signature
    /// change needed, so every existing caller stays untouched) is
    /// `Code Editor`'s own real need: `Enter` inserts `\n`, `Home`/
    /// `End` operate on the current line rather than the whole
    /// buffer, and `ArrowUp`/`ArrowDown` move by line -- see `Tree::
    /// dispatch_text_field_key`'s own real logic for each.
    pub multiline: bool,
    /// M31 Phase 3 (§5, §8): `false` (the default, every existing
    /// construction site's own byte-for-byte unchanged behavior) never
    /// substitutes anything. `true` (set directly, the identical plain
    /// `pub` field write `multiline` already established -- no `new()`
    /// signature change needed) is `Code Editor`'s own real need: a
    /// real space/tab in `content` paints as a visible middle-dot/
    /// arrow glyph instead (`engine-render`'s own `draw_field`/`hit_
    /// test_position`), purely at paint time -- `content` itself is
    /// never touched, the identical "engine-core holds the real value,
    /// engine-render decides how it looks" split every other paint-
    /// only field in this codebase already has.
    pub show_whitespace: bool,
    /// M31 Phase 4 (§5, §8): the app's own real per-byte-range syntax
    /// coloring, supplied by tokenization the app performs itself
    /// (Design Principle 6, the same real "app-side tokenization
    /// only, no engine-bundled lexer" split pyCopper's own optional-
    /// Pygments design already established) -- `engine-core` never
    /// interprets these ranges itself, and literal RGBA is the real,
    /// deliberate color representation (not a palette-token name):
    /// MD3 only has four real color roles (primary/secondary/
    /// tertiary/error) against the roughly ten categories a real
    /// syntax theme needs, so no semantic role exists to map the rest
    /// onto (pyCopper's own real, stated reasoning, quoted directly).
    /// Empty (the default) paints every existing field exactly as
    /// before this phase; ranges may freely overlap `show_whitespace`
    /// substitution -- `engine-render`'s own `draw_field` remaps both
    /// through the identical real byte-offset map.
    pub syntax_spans: Vec<(std::ops::Range<usize>, Color)>,
    /// M31 Phase 5 (§5, §8): real, paint-only content folding -- each
    /// `Range<usize>` names real bytes in `content` (the user's own
    /// explicit "full real folding" scope choice) collapsed into one
    /// visible "⋯" marker (`engine-render`'s own `elide_folded_
    /// ranges`). `content` itself is never touched; `engine-core`
    /// never *validates* these ranges (the identical real "app's own
    /// concern" split `syntax_spans` already has -- an app decides
    /// *which* real lines are foldable/currently folded, this
    /// codebase has no code-structure awareness of its own to decide
    /// that itself, and a malformed range is defensively skipped
    /// rather than trusted). Empty (the default) paints every
    /// existing field exactly as before this phase.
    ///
    /// M38 Phase 3 (§5, §8): `engine-core` *does* now interpret these
    /// ranges for one real purpose -- `Tree::dispatch_text_field_key`'s
    /// own `Home`/`End`/`ArrowUp`/`ArrowDown` snap a cursor that would
    /// otherwise land strictly inside a real folded range forward to
    /// right after that fold's own real marker (`Tree::snap_out_of_
    /// fold`), closing the real gap this comment used to name -- a
    /// cursor can no longer move into a folded region and end up
    /// somewhere genuinely invisible. Mirrors `engine-render::text::
    /// to_display_offset_folded`'s own identical "resolves to right
    /// after the fold" convention, so navigation and paint now agree.
    pub folded_ranges: Vec<std::ops::Range<usize>>,
    /// M38 Phase 2 (§5, §8): real "goal column" memory for consecutive
    /// `ArrowUp`/`ArrowDown` moves -- `Some(column)` while such a
    /// sequence is in progress, set to the cursor's own real column
    /// the *first* time either key fires after any other cursor-
    /// moving action, then left untouched by further `ArrowUp`/
    /// `ArrowDown` in the same sequence (even through a shorter line
    /// that clamps the real cursor to a smaller column) so a later
    /// hop back onto a long-enough line lands back at the original
    /// column -- the same real behavior every desktop text editor
    /// already has. `None` (the default, and what every other cursor-
    /// moving action resets it to -- `ArrowLeft`/`Right`, `Home`/
    /// `End`, a click, typing, a delete) means "no goal yet, derive it
    /// fresh from wherever the cursor currently sits."
    pub goal_column: Option<usize>,
    /// M38 Phase 7 (§5, §8): real vertical scroll for a genuinely
    /// overflowing `multiline` field (`Code Editor`'s own real need,
    /// scoped via `AskUserQuestion` to a dedicated mechanism rather
    /// than wrapping `TextField` in a real `ScrollView`, since that
    /// would need real `taffy` measure-function integration -- a
    /// genuinely new capability with no precedent anywhere in this
    /// codebase, confirmed by direct grep before choosing this path).
    /// A real pixel offset, driven directly by `Tree::scroll_text_
    /// field_caret_into_view` and `engine-render`'s own paint code --
    /// not through `animate_field`/central ticking, the identical
    /// real precedent `ScrollViewState.scroll`/`VirtualListState.
    /// scroll_offset`'s own doc comments already establish for every
    /// other real per-`NodeKind` scroll value. `Animated<f64>` purely
    /// for its own `.current` convenience, not because this value is
    /// ever eased. `0.0` (the default) is a true no-op for every
    /// existing single-line/non-overflowing field, unchanged.
    pub scroll_offset: Animated<f64>,
    /// M39 Phase 1 (§5, §8): `scroll_offset`'s own real horizontal
    /// sibling -- a genuinely overflowing real line (`engine-render::
    /// text::field_max_width` shapes every `multiline` field at
    /// `f32::MAX`, confirmed by direct read: no line ever wraps, it
    /// simply extends right, clipped since M38 Phase 7 but not
    /// scrollable until this field). Identical real contract to
    /// `scroll_offset`: a real pixel offset, driven directly by
    /// `Tree::scroll_text_field_caret_into_view` and `engine-render`'s
    /// own paint code, never through `animate_field`/central ticking.
    /// `0.0` (the default) is a true no-op for every existing single-
    /// line field and every multiline field whose own longest real
    /// line still fits the box, unchanged.
    pub horizontal_scroll_offset: Animated<f64>,
    /// M95: the hint shown while `content` is empty; empty shows none.
    pub placeholder: String,
    /// M95: the placeholder's color; `None` is `text_tint` at 60% alpha.
    pub placeholder_fill: Option<Color>,
    /// M95: the caret's color; `None` is `text_tint`.
    pub caret_color: Option<Color>,
    /// M95: the selection highlight's color; `None` is `text_tint` at 30%
    /// alpha.
    pub selection_fill: Option<Color>,
    /// M95: a password field -- every character paints as a bullet, and
    /// its text never leaves through copy or cut.
    pub obscured: bool,
}

impl TextFieldState {
    /// Seeds `cursor` at `content`'s own real end -- a real text
    /// field's own real, expected initial-cursor-at-end convention
    /// (every desktop toolkit's own default), not `0`.
    pub fn new(
        content: impl Into<String>,
        font_family: impl Into<String>,
        font_weight: f32,
        font_size: f32,
    ) -> Self {
        let content = content.into();
        let cursor = content.len();
        Self {
            content,
            font_family: font_family.into(),
            font_weight,
            font_size,
            cursor,
            selection_anchor: None,
            preedit: None,
            text_tint: Animated::new(Color::from_rgba8(0x1C, 0x1B, 0x1F, 0xFF)),
            multiline: false,
            show_whitespace: false,
            syntax_spans: Vec::new(),
            folded_ranges: Vec::new(),
            goal_column: None,
            scroll_offset: Animated::new(0.0),
            horizontal_scroll_offset: Animated::new(0.0),
            placeholder: String::new(),
            placeholder_fill: None,
            caret_color: None,
            selection_fill: None,
            obscured: false,
        }
    }
}

/// M22 Phase 1 (§5): a real, file-backed image. `image` is already-
/// decoded, renderer-agnostic pixel data -- `peniko::ImageData` is
/// exactly `TextFieldState`'s own "`engine-core` holds inert data"
/// precedent, except here there's no separate engine-core-native
/// representation to invent at all (unlike `TextState`'s deliberate
/// avoidance of a raw `parley::Layout`, §4's crate-boundary rule):
/// `peniko` is already a real, direct `engine-core` dependency (used
/// for `Color` throughout this file), and `peniko::ImageData` is
/// already exactly the shape a renderer needs (`Blob<u8>` pixel data
/// plus format/alpha-type/width/height), so storing it directly costs
/// zero new dependency-graph edge here. Decoding an actual image file
/// (the `image` crate, PNG/JPEG bytes -> raw RGBA8) happens in
/// `engine-py` (an image node's `src`) -- the same real "resolved ahead of
/// time, not computed live" split `NodeKind::Canvas`'s own module doc
/// comment already established for its Python draw callback.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageState {
    pub image: peniko::ImageData,
    /// M22 Phase 2 (§16.1): how the image's own real pixel content
    /// fits a node whose box doesn't share its aspect ratio -- real,
    /// standard CSS `object-fit` semantics (`Cover` crops to fill with
    /// no letterboxing, `Contain` scales down to fit entirely, leaving
    /// the node's own `background` visible on the uncovered sides,
    /// `Fill` stretches non-uniformly to the box exactly). Defaults to
    /// `Fill` -- Phase 1's own real, only behavior, so an `ImageState`
    /// built through `ImageState::new` (every Phase 1 call site) keeps
    /// byte-for-byte the same paint as before this phase.
    pub content_fit: ContentFit,
}

impl ImageState {
    pub fn new(image: peniko::ImageData) -> Self {
        Self {
            image,
            content_fit: ContentFit::Fill,
        }
    }

    /// A 1x1 fully-transparent placeholder -- the "nothing to show yet"
    /// image an image node starts with before it has a `src` or
    /// `rgba` frame.
    pub fn blank() -> Self {
        Self::new(peniko::ImageData {
            data: peniko::Blob::from(vec![0u8, 0, 0, 0]),
            format: peniko::ImageFormat::Rgba8,
            alpha_type: peniko::ImageAlphaType::Alpha,
            width: 1,
            height: 1,
        })
    }
}

/// M22 Phase 2 (§16.1): see `ImageState.content_fit`'s own doc comment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ContentFit {
    /// Scales up (never down) just enough to cover the node's box
    /// entirely, cropping whichever axis overflows; centered.
    Cover,
    /// Scales to fit entirely inside the node's box, letterboxing
    /// (leaving `background` visible) on whichever axis has slack;
    /// centered.
    Contain,
    /// Stretches non-uniformly to the box exactly, ignoring the
    /// image's own real aspect ratio -- Phase 1's own original, only
    /// behavior.
    #[default]
    Fill,
}

/// §11.7's own struct sketch, unchanged in shape (`item_count`,
/// `item_extent`, `materialized`). `materialized`'s values are exactly
/// this node's own `children` (§5) with "which logical index" attached
/// on top -- not a second, separately-tracked child set; `Tree::
/// set_virtual_list_window` is what keeps the two in lockstep, the same
/// "engine-core carries inert state, a `Tree` method is what makes it do
/// anything" shape every other `NodeKind` payload already uses.
pub struct VirtualListState {
    pub item_count: usize,
    pub item_extent: ItemExtent,
    pub materialized: std::collections::BTreeMap<usize, NodeId>,
    /// M8 Phase 2 (§11.7): the real vertical scroll position, in local
    /// pixels -- composed into materialized children's own effective
    /// paint-time position (`draw_own`), not their `layout_style`
    /// (their real taffy layout never changes; only where they're
    /// *drawn* does). Plain `Animated<f64>`, driven directly (like a
    /// scrollbar being dragged, not eased toward a target) -- never
    /// ticked centrally, only by its own dedicated mechanism.
    pub scroll_offset: Animated<f64>,
    /// M12 Phase 1 (§11.7): real, resolved cumulative offsets for
    /// `Variable`-extent lists -- index `idx` maps to item `idx`'s own
    /// real top-offset (not its height). Always empty for `Fixed`
    /// (whose offsets are computed directly, `idx * item_extent`). The
    /// key `item_count` (one past the last real item, the same "one
    /// past the end" convention a `Range` already uses) holds the real
    /// total content extent. `engine-core` never computes a cumulative
    /// sum itself here -- only looks a resolved value up (`offset_of`/
    /// `total_extent` below); §4's own pyo3-agnostic boundary is why
    /// the actual per-item size-hint resolution lives in `engine-py`
    /// (populated via `Tree::set_virtual_list_resolved_offsets`), the
    /// same real reason `materialize` itself lives there too, not here.
    pub resolved_offsets: std::collections::BTreeMap<usize, f64>,
    /// M47 (§5, §7, §11.7): `Some((pointer_y, scroll_at_drag_start))`,
    /// set the instant a real thumb drag begins and cleared when it
    /// ends -- byte-for-byte the same real anchor shape `ScrollViewState
    /// ::thumb_drag_anchor` (M38 Phase 6) already established, ported
    /// here rather than reinvented. A *relative*-delta anchor, not an
    /// absolute pointer-to-scroll mapping, for the identical real reason
    /// `ScrollView`'s own doc comment already gives: grabbing the thumb
    /// anywhere along its own length must not snap it so that point
    /// jumps under the pointer.
    pub thumb_drag_anchor: Option<(f64, f64)>,
}

impl VirtualListState {
    pub fn new(item_count: usize, item_extent: ItemExtent) -> Self {
        Self {
            item_count,
            item_extent,
            materialized: std::collections::BTreeMap::new(),
            scroll_offset: Animated::new(0.0),
            resolved_offsets: std::collections::BTreeMap::new(),
            thumb_drag_anchor: None,
        }
    }

    /// M47 (§5, §7, §11.7): real thumb geometry `(track, thumb, along)`
    /// -- the identical real shape `ScrollViewState::thumb_geometry`
    /// (M38 Phase 6) already established, shared by painting
    /// (`engine-render's paint walk`) and hit-testing/dragging
    /// (`Tree::grabs_virtual_list_thumb`/`update_virtual_list_thumb_
    /// drag`) so the two can never drift -- the same "one function,
    /// every real caller" discipline this codebase already applies
    /// throughout. Vertical-only (no `horizontal` parameter): `Virtual
    /// List` itself has no horizontal-scroll variant today (`Tree::
    /// scroll_virtual_list_by`'s own signature is vertical-only),
    /// unlike `ScrollView`, so this is a real, narrower single-axis
    /// version, not a second horizontal-capable copy. Reads `self.
    /// total_extent()` for content extent directly (`VirtualList` has
    /// no single measured child to pass one in from, unlike
    /// `ScrollView`) -- the same real primitive `Tree::scroll_virtual_
    /// list_by`'s own clamping already uses, so thumb geometry and
    /// wheel-scroll clamping can never disagree about the real content
    /// extent.
    pub fn thumb_geometry(&self, viewport_extent: f64) -> (f64, f64, f64) {
        let track = viewport_extent - SCROLLBAR_MARGIN * 2.0;
        if track <= 0.0 {
            return (0.0, 0.0, 0.0);
        }
        let content_extent = self.total_extent();
        let max_scroll = (content_extent - viewport_extent).max(0.0);
        let thumb = if content_extent > 0.0 {
            (track * (viewport_extent / content_extent)).max(SCROLLBAR_MIN_LENGTH)
        } else {
            track
        };
        let thumb = thumb.min(track);
        let progress = if max_scroll > 0.0 {
            self.scroll_offset.current / max_scroll
        } else {
            0.0
        };
        let along = SCROLLBAR_MARGIN + (track - thumb) * progress;
        (track, thumb, along)
    }

    /// Item `idx`'s own real top-offset. M100: `0.0` when `item_extent`
    /// is `Variable` and `idx` isn't resolved yet -- a list whose
    /// `size_hint` hasn't run (no layout yet) or raised (logged) has no
    /// extent, so it neither positions rows apart nor scrolls. This used
    /// to panic, which a raising `size_hint` and a wheel event could
    /// reach from Python.
    pub fn offset_of(&self, idx: usize) -> f64 {
        match &self.item_extent {
            ItemExtent::Fixed(v) => idx as f64 * v,
            ItemExtent::Variable => self.resolved_offsets.get(&idx).copied().unwrap_or(0.0),
        }
    }

    /// The real total content extent across every item -- `item_count *
    /// item_extent` for `Fixed`, or `offset_of(item_count)` (the
    /// resolved offset "one past the last item") for `Variable`.
    pub fn total_extent(&self) -> f64 {
        match &self.item_extent {
            ItemExtent::Fixed(v) => self.item_count as f64 * v,
            ItemExtent::Variable => self.offset_of(self.item_count),
        }
    }
}

/// §11.7's own text: "fixed, or a size-hint callback for variable-height
/// items." M12 Phase 1 (§11.7): `Variable` is real now -- a fieldless
/// marker, deliberately; the actual per-item data lives on `VirtualList
/// State::resolved_offsets`, not this enum itself, since only `Virtual
/// ListState` has a `Tree`-mutation path (`Tree::set_virtual_list_
/// resolved_offsets`) to populate it. No callback lives in `engine-core`
/// itself -- §4's own pyo3-agnostic boundary rules that out, the same
/// real reason `materialize` lives in `engine-py`, not here; resolving
/// a real per-item size-hint from Python is Phase 2's own concern.
pub enum ItemExtent {
    Fixed(f64),
    Variable,
}

/// M30 Phase 1 (§5, §7): a real horizontal text-alignment capability --
/// `engine-render`'s own text-shaping pipeline (`text.rs`) hardcoded
/// `parley::Alignment::Start` unconditionally until this phase, a real,
/// verified gap (confirmed by direct read, not assumed) that blocks any
/// correctly-rendered centered label (a button's, say). It lives on
/// `TextState` itself, a universal capability rather than
/// component-specific machinery.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextAlign {
    /// Left for LTR text, right for RTL -- byte-for-byte the same
    /// direction-aware behavior every existing text node already had
    /// before this field existed, so this is a true no-op default.
    #[default]
    Start,
    Center,
    /// Right for LTR text, left for RTL.
    End,
}

/// A text node's content and shaping inputs -- everything `parley` needs
/// to shape a run, and nothing about how it got styled (that's a
/// framework's job, not this crate's -- `engine-core` has no design
/// system, §1 Locked Decisions). `font_family` names an
/// already-registered family (by exact name, matching the font's own
/// name table) rather than carrying a weight/style axis: this step's two type
/// roles are two distinct font files (Roboto Regular vs. Medium), not
/// one variable font interpolated at draw time.
/// M96: how a text node lays out its lines, beyond its font -- italics,
/// letter spacing, wrapping, a line limit, and whether an overflowing last
/// line ends in an ellipsis. The default is plain wrapped text.
#[derive(Clone, Debug, PartialEq)]
pub struct TextOptions {
    /// Italic. Synthesized by slanting the glyphs when the family has no
    /// italic face.
    pub italic: bool,
    /// Extra space after each character, in pixels.
    pub letter_spacing: f32,
    /// `true` wraps lines at word boundaries within the node's width;
    /// `false` keeps each paragraph on one line.
    pub wrap: bool,
    /// The most lines shown; the rest are cut.
    pub max_lines: Option<usize>,
    /// Ends a cut last line -- by `max_lines`, or by the width when not
    /// wrapping -- with "…".
    pub ellipsis: bool,
}

impl Default for TextOptions {
    fn default() -> Self {
        Self {
            italic: false,
            letter_spacing: 0.0,
            wrap: true,
            max_lines: None,
            ellipsis: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextState {
    pub content: String,
    pub font_family: String,
    /// OpenType weight class (100.0..=950.0, matching CSS `font-weight`'s
    /// numeric range; 400.0 is normal). A real, non-obvious finding from
    /// wiring this up: distinct static weights of the same type family
    /// (e.g. Roboto Regular vs. Medium) commonly register under the
    /// *same* family name -- their typographic family name (OpenType
    /// name ID 16) is shared, only the subfamily (ID 17) differs -- so
    /// weight cannot be selected by family name alone. Confirmed
    /// directly against Roboto's own name table, not assumed.
    pub font_weight: f32,
    pub font_size: f32,
    /// M30 Phase 1 (§5, §7): see `TextAlign`'s own doc comment.
    pub align: TextAlign,
    /// M62 Phase 1 (§7.1, §16.3): a font-size-relative line-height
    /// multiplier (`parley::LineHeight::FontSizeRelative`'s own real
    /// shape -- verified against `parley` 0.11.1's own vendored source,
    /// not assumed), matching MD3's own published type-scale convention
    /// of stating line-height as `size` × ratio. `None` is a real,
    /// distinct value, not a stand-in for some concrete default: it
    /// means "use the font's own natural metrics" (`parley::LineHeight
    /// ::MetricsRelative(1.0)`, the library's own real default, and
    /// this codebase's exact real behavior for every `TextState` ever
    /// built before this field existed) -- `Some(1.0)` is a different,
    /// real, explicit choice (exactly the font size, no leading at
    /// all), not the same thing spelled two ways.
    pub line_height: Option<f32>,
    /// M96: line layout beyond the font (`TextOptions`).
    pub options: TextOptions,
}

/// M95: four corner radii -- `[top_left, top_right, bottom_right,
/// bottom_left]` -- animatable as one value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CornerRadii(pub [f64; 4]);

impl Interpolate for CornerRadii {
    fn interpolate(&self, other: &Self, t: f64) -> Self {
        let mut out = [0.0; 4];
        for (i, corner) in out.iter_mut().enumerate() {
            *corner = self.0[i].interpolate(&other.0[i], t);
        }
        Self(out)
    }
}

/// M95: one drop shadow, as CSS `box-shadow` draws it -- the node's own
/// rounded box, offset, grown by `spread`, and blurred by `blur` (a CSS
/// blur radius in pixels).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub color: Color,
    pub offset_x: f64,
    pub offset_y: f64,
    pub blur: f64,
    pub spread: f64,
}

impl Interpolate for Shadow {
    fn interpolate(&self, other: &Self, t: f64) -> Self {
        Self {
            color: self.color.interpolate(&other.color, t),
            offset_x: self.offset_x.interpolate(&other.offset_x, t),
            offset_y: self.offset_y.interpolate(&other.offset_y, t),
            blur: self.blur.interpolate(&other.blur, t),
            spread: self.spread.interpolate(&other.spread, t),
        }
    }
}

impl Shadow {
    /// The same shadow, fully transparent -- what a shadow fades from or
    /// to when two lists of different lengths animate.
    fn invisible(self) -> Self {
        Self {
            color: self.color.with_alpha(0.0),
            ..self
        }
    }
}

/// M96: a node's own transform, as four independently animatable parts
/// applied about the center of its box, like CSS's default
/// `transform-origin`: scale and rotation first, then translation. Each
/// part has its own animation, so easing one never retargets another, and
/// a rotation sweeps through its arc.
pub struct NodeTransform {
    pub translate_x: Animated<f64>,
    pub translate_y: Animated<f64>,
    pub scale: Animated<f64>,
    pub rotation_deg: Animated<f64>,
}

impl Default for NodeTransform {
    fn default() -> Self {
        Self {
            translate_x: Animated::new(0.0),
            translate_y: Animated::new(0.0),
            scale: Animated::new(1.0),
            rotation_deg: Animated::new(0.0),
        }
    }
}

impl NodeTransform {
    /// The current affine for a `width` x `height` box.
    pub fn to_affine(&self, width: f64, height: f64) -> peniko::kurbo::Affine {
        use peniko::kurbo::Affine;
        let (tx, ty) = (self.translate_x.current, self.translate_y.current);
        let (scale, degrees) = (self.scale.current, self.rotation_deg.current);
        if scale == 1.0 && degrees == 0.0 {
            return Affine::translate((tx, ty));
        }
        let center = (width / 2.0, height / 2.0);
        Affine::translate((tx + center.0, ty + center.1))
            * Affine::rotate(degrees.to_radians())
            * Affine::scale(scale)
            * Affine::translate((-center.0, -center.1))
    }

    fn tick(&mut self, now: Instant, completed: &mut Vec<crate::CompletionHandle>) -> bool {
        let x = self.translate_x.tick(now, completed);
        let y = self.translate_y.tick(now, completed);
        let scale = self.scale.tick(now, completed);
        let rotation = self.rotation_deg.tick(now, completed);
        x || y || scale || rotation
    }
}

/// M95: a node's drop shadows, the first painted on top, like CSS.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shadows(pub Vec<Shadow>);

impl Interpolate for Shadows {
    /// Pairwise; a shadow with no partner fades in or out in place.
    fn interpolate(&self, other: &Self, t: f64) -> Self {
        if t >= 1.0 {
            return other.clone();
        }
        let len = self.0.len().max(other.0.len());
        let shadows = (0..len)
            .map(|i| {
                let from = self.0.get(i).copied();
                let to = other.0.get(i).copied();
                match (from, to) {
                    (Some(a), Some(b)) => a.interpolate(&b, t),
                    (Some(a), None) => a.interpolate(&a.invisible(), t),
                    (None, Some(b)) => b.invisible().interpolate(&b, t),
                    (None, None) => unreachable!("i < the longer list's length"),
                }
            })
            .collect();
        Self(shadows)
    }
}

/// Universal paint state every node has, regardless of `NodeKind`.
pub struct PaintProperties {
    pub background: Animated<Color>,
    pub corner_radius: Animated<f64>,
    pub opacity: Animated<f64>,
    /// §11.9 (M5 Phase 1): composed down the tree during the paint walk
    /// -- a node's effective transform is its parent's effective
    /// transform composed with its own, exactly like nested `<g
    /// transform>` in SVG. Defaults to `Affine::IDENTITY`, so every
    /// node that never sets this paints exactly where its taffy layout
    /// already places it -- purely additive, matching every other
    /// `PaintProperties` field's own "off unless a caller opts in"
    /// shape.
    pub transform: Animated<peniko::kurbo::Affine>,
    /// M30 Phase 1 (§5, §7): a real stroked border, painted inside the
    /// node's own fill edge (never expanding its layout box) -- the
    /// Python API's `stroke_color`/`stroke_width`.
    /// `border_width.current <= 0.0` is a true no-op, the same
    /// "off unless a caller opts in" contract every other
    /// `PaintProperties` field keeps.
    pub border_color: Animated<Color>,
    pub border_width: Animated<f64>,
    /// M30 Phase 1 Step 4 (§5, §7): a real per-corner radius override,
    /// `[top_left, top_right, bottom_right, bottom_left]` -- kurbo's
    /// own `RoundedRect::new` already accepts a 4-tuple of independent
    /// corner radii natively (confirmed via direct source read of the
    /// pinned `kurbo 0.13.1`, not assumed), so this is exposing an
    /// existing real primitive, not inventing new geometry -- a
    /// segmented group's first/last segments, say, rounded only on
    /// their outer edge, which `corner_radius`'s own single scalar
    /// can't express. `None` (the default) means "use the uniform
    /// `corner_radius` scalar," the same true no-op contract
    /// `border_width: 0.0` already establishes.
    pub corner_radii_override: Option<Animated<CornerRadii>>,
    /// M95: drop shadows (`Shadows`).
    pub shadows: Animated<Shadows>,
    /// M96: the target API's `translate_x`/`translate_y`/`scale`/
    /// `rotation_deg`, composed after `transform` above (a plain
    /// top-left-origin affine the Python API doesn't set). Read both
    /// through `local_transform`.
    pub node_transform: NodeTransform,
    /// M32 Phase 3 (§5, §7, §11.7/§11.8): the real, general form of the
    /// clip `VirtualList` bakes into its own paint. `false` (every
    /// existing node, unchanged) is a true no-op, the same "off unless
    /// a caller opts in" contract every other additive field here
    /// already follows -- a child painted past this node's own box
    /// stays exactly as visible as it always was. **Real, stated v1
    /// scope limit:** clipping only -- unlike `VirtualList`, opting a
    /// plain node into this does not give it a real scroll offset or
    /// wheel-input wiring of its own; content still simply extends
    /// past the box, just genuinely hidden there instead of visibly
    /// spilling out. Not `Animated`: nothing needs a *smooth
    /// transition* into/out of clipping, only a static per-node choice.
    pub clip_children: bool,
}

impl PaintProperties {
    pub fn new(background: Color, corner_radius: f64, opacity: f64) -> Self {
        Self {
            background: Animated::new(background),
            corner_radius: Animated::new(corner_radius),
            opacity: Animated::new(opacity),
            transform: Animated::new(peniko::kurbo::Affine::IDENTITY),
            border_color: Animated::new(Color::from_rgba8(0, 0, 0, 0)),
            border_width: Animated::new(0.0),
            corner_radii_override: None,
            shadows: Animated::new(Shadows::default()),
            node_transform: NodeTransform::default(),
            clip_children: false,
        }
    }

    /// The per-node half of the central tick (§5): advances every owned
    /// `Animated<T>`, returning `true` if any is still mid-animation.
    /// `Tree::tick_all` calls this for every node -- a naive whole-tree
    /// walk, not the "walks only the active set" scoped version §5
    /// describes. That scoping is real dirty-tracking machinery that
    /// belongs to §6's per-frame pipeline design, not manufactured here
    /// ahead of a step that profiles it as actually necessary; the
    /// frame-time CI benchmark this same step adds is exactly what would
    /// catch it if a naive walk ever stopped meeting the 16.6ms budget.
    /// M96: this node's whole transform relative to its layout position,
    /// for a `width` x `height` box -- what paint and hit-testing apply.
    pub fn local_transform(&self, width: f64, height: f64) -> peniko::kurbo::Affine {
        self.transform.current * self.node_transform.to_affine(width, height)
    }

    pub fn tick(&mut self, now: Instant, completed: &mut Vec<crate::CompletionHandle>) -> bool {
        let background = self.background.tick(now, completed);
        let corner_radius = self.corner_radius.tick(now, completed);
        let opacity = self.opacity.tick(now, completed);
        let transform = self.transform.tick(now, completed);
        let border_color = self.border_color.tick(now, completed);
        let border_width = self.border_width.tick(now, completed);
        let radii = self
            .corner_radii_override
            .as_mut()
            .is_some_and(|radii| radii.tick(now, completed));
        let shadows = self.shadows.tick(now, completed);
        let node_transform = self.node_transform.tick(now, completed);
        radii
            || shadows
            || node_transform
            || background
            || corner_radius
            || opacity
            || transform
            || border_color
            || border_width
    }
}

pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    /// M96: `false` hides the node and its subtree -- not painted, not
    /// hit, not in the accessibility tree or tab order, and (through
    /// `Display::None`, set alongside) taking no layout space.
    pub visible: bool,
    /// M96: paint and hit-test order among siblings -- higher paints later,
    /// on top; equal values keep child order.
    pub z_index: i32,
    pub kind: NodeKind,
    pub layout_style: Style,
    pub paint: PaintProperties,
    /// §14 step 7: `AccessNodeData::default()` (`Role::Unknown`, no
    /// label/actions) unless a caller opts a node in via
    /// `Tree::set_access`.
    pub access: crate::access::AccessNodeData,
    /// M30 Phase 5 Step 1 (§5, §7): whether this node itself claims a
    /// hit in `Tree::hit_test_at` -- `false` lets a decorative interior
    /// `Rect` (an indicator pill behind an icon, say) pass the point
    /// through to its parent. `Text` gets the same exemption
    /// unconditionally; a `Rect` can't (it's often the real click
    /// target), so this is an opt-in per-node flag. `true` (every existing
    /// node, via `Tree::insert`'s own single real construction site)
    /// is a true no-op -- only `Tree::set_hit_testable(id, false)`
    /// changes anything.
    pub hit_testable: bool,
    /// M94: the pointer shape shown over this node; `None` inherits the
    /// nearest ancestor's, and the default arrow when none sets one.
    pub cursor: Option<Cursor>,
}

/// M94: the pointer shapes a node can ask for -- CSS's own vocabulary, in
/// snake_case, so a framework author already knows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cursor {
    Default,
    Pointer,
    Text,
    Grab,
    Grabbing,
    Move,
    NotAllowed,
    Wait,
    Progress,
    Crosshair,
    Help,
    ColResize,
    RowResize,
    EwResize,
    NsResize,
    NeswResize,
    NwseResize,
    Copy,
    Cell,
    ContextMenu,
    ZoomIn,
    ZoomOut,
    AllScroll,
}

impl Cursor {
    pub const ALL: [Cursor; 23] = [
        Self::Default,
        Self::Pointer,
        Self::Text,
        Self::Grab,
        Self::Grabbing,
        Self::Move,
        Self::NotAllowed,
        Self::Wait,
        Self::Progress,
        Self::Crosshair,
        Self::Help,
        Self::ColResize,
        Self::RowResize,
        Self::EwResize,
        Self::NsResize,
        Self::NeswResize,
        Self::NwseResize,
        Self::Copy,
        Self::Cell,
        Self::ContextMenu,
        Self::ZoomIn,
        Self::ZoomOut,
        Self::AllScroll,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Pointer => "pointer",
            Self::Text => "text",
            Self::Grab => "grab",
            Self::Grabbing => "grabbing",
            Self::Move => "move",
            Self::NotAllowed => "not_allowed",
            Self::Wait => "wait",
            Self::Progress => "progress",
            Self::Crosshair => "crosshair",
            Self::Help => "help",
            Self::ColResize => "col_resize",
            Self::RowResize => "row_resize",
            Self::EwResize => "ew_resize",
            Self::NsResize => "ns_resize",
            Self::NeswResize => "nesw_resize",
            Self::NwseResize => "nwse_resize",
            Self::Copy => "copy",
            Self::Cell => "cell",
            Self::ContextMenu => "context_menu",
            Self::ZoomIn => "zoom_in",
            Self::ZoomOut => "zoom_out",
            Self::AllScroll => "all_scroll",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|cursor| cursor.name() == name)
    }
}
