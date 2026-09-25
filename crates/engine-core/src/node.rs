//! §5's core data model: `NodeId`, `Node`, `NodeKind`, `PaintProperties`.
//!
//! Deliberately narrower than ARCHITECTURE.md §5's full struct shapes, for
//! this build-order step (§14 step 3, "layout of multiple static nodes"):
//!
//! - `Node` originally omitted `access: AccessNodeData` (AccessKit
//!   wiring, step 7) and `interaction: Option<InteractionState>`
//!   (ripple/state-layer, §7.3, step 9); both fields have since landed
//!   at their own steps, per Design Principle 5's "add it when its own
//!   step needs it" discipline.
//! - `PaintProperties` originally omitted `transform: Animated<kurbo::
//!   Affine>` (needed a non-trivial `Interpolate` impl); it landed at
//!   M5 Phase 1 (§11.9) as a plain componentwise coefficient lerp, not
//!   a rotation-aware decomposition -- see `animation.rs`'s own
//!   `Interpolate for Affine` doc comment for why that's still correct
//!   for this framework's actual pan/zoom scope. `shape:
//!   Animated<ShapeKey>` (the §7.4 shape-correspondence-then-lerp
//!   technique) landed at M7 Phase 4, once `ShapeKey` itself moved here
//!   from `engine-md3` (see `shape_morph.rs`'s own doc comment).
//! - `NodeKind` carried only `Rect`/`Container` through step 3;
//!   `Text(TextState)` is added at step 4 (§14 step 4, the typography
//!   spike). `Image`/`Slider`/`Checkbox`/`Canvas` still land with their
//!   own later build-order steps.
//!
//! Each omission is additive to restore later, per Design Principle 5 and
//! the same "don't build ahead of need" discipline already applied to
//! `MotionCurve` (step 2, `Linear`-only) and here again to `NodeKind`.
//! `TextState` itself is narrower than a real future MD3 text component
//! will need: no `Animated` fields (no MD3 component in scope yet
//! animates a text property -- cursor blink, reveal-on-scroll, etc. --
//! so none is manufactured here ahead of a step that needs one), no
//! wrapping/overflow policy (this step measures `parley`'s own
//! line-breaking against a fixed box width, it doesn't design a CSS-like
//! overflow model).

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
/// No longer derives `Clone`/`Debug`/`PartialEq` now that `Splitter`
/// carries an `Animated<f64>` -- `Animated<T>` implements none of those
/// (same reason `PaintProperties`, also full of `Animated` fields,
/// never derived them either); nothing in this codebase actually
/// cloned, printed, or compared a `NodeKind` value directly (checked
/// directly, not assumed), so this costs nothing real.
pub enum NodeKind {
    Rect,
    Container,
    Text(TextState),
    /// §14 step 15 (§11.5): a draggable divider between two sibling
    /// regions. Carries no paint of its own beyond `PaintProperties`
    /// (a real splitter typically just wants a `background` for its
    /// own grip/handle) -- `SplitterState` is purely the mechanism's
    /// own animatable position, not appearance.
    Splitter(SplitterState),
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
    /// M14 Phase 1 (§5, §7.3): a real MD3 checkbox. `checked` is plain,
    /// app-owned state (Design Principle 6 -- "selection/checked-state
    /// ... depend on what the app's data means," not anything the
    /// engine determines on its own); `check_progress` is the engine-
    /// driven visual consequence, animated toward `1.0`/`0.0` whenever
    /// the app sets `checked`. The engine deliberately does not toggle
    /// `checked` on click itself -- the already-generic `Click`
    /// dispatch/`Node.enable_interaction()` ripple mechanism (works for
    /// any `NodeKind` already) is what this phase reuses unchanged; an
    /// app's own `on_click` handler is what actually flips `checked`.
    Checkbox(CheckboxState),
    /// M14 Phase 2 (§5, §7.3): a real MD3 slider. `thumb_position` *is*
    /// the real value (0.0..=1.0 along the track, the identical shape
    /// `SplitterState.position` already has) -- no separate field, the
    /// same "the animated field is the value" precedent. A real drag
    /// (`Tree::set_slider_position`) sets it the same instant, `Duration
    /// ::ZERO` + immediate-manual-tick way `SplitterState.position`
    /// already does, live-following the pointer. Unlike `SplitterState.
    /// position` (never exposed to `Node.animate()` at all -- confirmed
    /// via direct read, so it never needs central ticking), `thumb_
    /// position` *is* exposed (`"thumb_position"`, this phase's own
    /// second real kind-payload `animate()` arm) for a real, app-
    /// triggered eased move (e.g. a keyboard nudge, not a drag) -- real
    /// finding while designing this: that path needs `Tree::tick_all`
    /// to actually tick it centrally, or a nonzero-duration `animate()`
    /// call would set an active animation that never progresses. Ticked
    /// there, unconditionally, alongside `CheckboxState.check_progress`.
    Slider(SliderState),
    /// M15 Phase 1 (§5, §16.7): a real, single-line editable text
    /// field. `content`/`cursor`/`selection_anchor` are plain, engine-
    /// core-native byte-offset state -- Design Principle 6's own
    /// "app-owned meaning" shape `CheckboxState.checked` already
    /// established, except here the *engine* is the one real mutator
    /// (via `Tree::dispatch`'s own keyboard-editing arm, M15 Phase 2),
    /// since typing is mechanical, not app-defined meaning the way a
    /// checkbox's "checked" is. Deliberately does *not* carry a
    /// `parley::Layout`/`Selection` directly: `engine-core` has no
    /// `parley` dependency at all (§4's crate-boundary rule) --
    /// `engine-render` reconstructs both, each frame, purely to compute
    /// real caret/highlight paint geometry, the same "engine-core holds
    /// inert data, engine-render re-derives it" split `TextState`
    /// itself already uses.
    TextField(TextFieldState),
    /// M22 Phase 1 (§5): a real, file-backed image, painted through
    /// `vello_hybrid`'s own real `PaintType::Image` mechanism -- see
    /// `ImageState`'s own doc comment for the real crate-boundary
    /// reasoning (decoding lives in `engine-py`, this holds only the
    /// already-decoded result).
    Image(ImageState),
    /// M23 Phase 1 (§1, §3): a real MD3 vector icon, painted as a
    /// solid-`tint` `BezPath` fill -- see `IconState`'s own doc
    /// comment for the real crate-boundary reasoning.
    Icon(IconState),
    /// M95 (D4): any vector path -- fill, stroke, trim, morph.
    Path(crate::path::PathState),
    /// M30 Phase 2 Step 1 (§5, §7.3): a real MD3 radio button.
    /// `selected` is plain, app-owned state -- the identical Design
    /// Principle 6 shape `CheckboxState.checked` already establishes
    /// (the engine never toggles it on click itself); `select_
    /// progress` is the engine-driven visual consequence, animated
    /// toward `1.0`/`0.0` whenever the app sets `selected`, the same
    /// real "the animated field is the value" precedent `check_
    /// progress` already is. See `RadioButtonState`'s own doc comment
    /// for why the ring itself is animated color, not just the dot.
    RadioButton(RadioButtonState),
    /// M30 Phase 2 Step 2 (§5, §7.3): a real MD3 switch. `on` is plain,
    /// app-owned state, `toggle_progress` the engine-driven visual
    /// consequence -- the identical `Checkbox`/`RadioButton` shape a
    /// third time. See `SwitchState`'s own doc comment for its real,
    /// verified anatomy (a track plus a handle that both slides *and*
    /// grows as it toggles).
    Switch(SwitchState),
    /// M30 Phase 3 Step 2 (§5, §7): a real MD3 linear progress
    /// indicator. `value` is plain, app-owned state (`0.0..=1.0`,
    /// `SliderState.thumb_position`'s own real shape, except never
    /// draggable -- a progress indicator only ever displays a value an
    /// app computes elsewhere, it's never a real input control) --
    /// `Animated<f64>` directly, the same "the animated field is the
    /// value" precedent every progress-like field in this codebase
    /// already uses, so a real app-triggered eased update (`Node.
    /// animate("value", ...)`) works exactly like `thumb_position`'s
    /// own real eased-move path.
    LinearProgress(LinearProgressState),
    /// M30 Phase 3 Step 2 (§5, §7): `LinearProgress`'s own real
    /// circular sibling -- same real `value` shape, painted as a
    /// stroked arc (`engine-render`'s own paint arm) instead of a
    /// filled bar. Real MD3 anatomy has no separate background track
    /// ring for the circular indicator (confirmed from Material Web's
    /// own token source -- no `track-color` token exists for it,
    /// unlike the linear indicator's real, separate `track-color`).
    CircularProgress(CircularProgressState),
    /// M30 Phase 8 Step 2 (§5, §7): a real, standalone clickable label
    /// -- MD3's own `Link`. Fulfills a real, explicit commitment this
    /// codebase already made to itself (Phase 1's own `Tree::hit_test_
    /// at` fix, `NodeKind::Text(_) => false`'s own doc comment): "a
    /// future standalone clickable label... gets its own dedicated
    /// `NodeKind`... not a handler bolted onto bare `Text`." A bare
    /// `Text` node deliberately never independently claims a hit (it
    /// always defers to whatever real interactive container it sits
    /// inside) -- a genuinely new variant, by simply not matching that
    /// arm, is real, minimal hit-testing independence "for free" via
    /// `hit_test_at`'s own existing `_ => rect_contains(...)`
    /// catch-all, no new hit-test logic needed at all. Reuses
    /// `TextState` verbatim as its own payload -- `Link`'s own real
    /// content/font shape is identical to `Text`'s, the only real
    /// difference is which `NodeKind` variant it is.
    Link(TextState),
    /// M30 Phase 9 Step 4 (§5, §8, §10): a real, live terminal -- see
    /// `TerminalState`'s own doc comment for the real crate-boundary
    /// reasoning (the identical "engine-core holds inert data,
    /// engine-render re-derives paint geometry" split `TextFieldState`
    /// already established, extended to a whole cell grid instead of
    /// one string).
    Terminal(TerminalState),
    /// M30 Phase 9 Step 5 (§5, §7, §11.7): a real MD3 carousel -- see
    /// `CarouselState`'s own doc comment for its real, distinctive
    /// "an animated value that also invalidates layout, not just
    /// paint" shape, and `Tree::sync_carousel_layouts` for how that's
    /// actually made real against `taffy`.
    Carousel(CarouselState),
    /// M36 Phase 1 (§5, §7, §11.7): a real, general scrollable
    /// viewport over one oversized child -- see `ScrollViewState`'s
    /// own doc comment for the real design (grounded directly in the
    /// sibling `pyCopper` project's own `ScrollViewElement`) and
    /// `Tree::sync_scroll_view_layouts` for how the real scroll offset
    /// is made real against `taffy` without the hit-test-after-scroll
    /// gap this phase's own investigation found in `VirtualList`.
    ScrollView(ScrollViewState),
    /// M39 Phase 2 (§5, §7): a real, perpetually-looping MD3
    /// Expressive-style loading spinner -- see `LoadingIndicatorState`'s
    /// own doc comment for the full real design.
    LoadingIndicator(LoadingIndicatorState),
    /// M39 Phase 2 Step 2 (§5, §7): a real MD3 Time Picker's own
    /// circular clock-face drag control -- see `TimePickerDialState`'s
    /// own doc comment for the full real design and its stated v1
    /// scope limits.
    TimePickerDial(TimePickerDialState),
}

/// M39 Phase 2 Step 2 (§5, §7): MD3's real Time Picker dial -- a
/// circular clock face with an hour hand and a minute hand, where
/// dragging around the circle picks a value by angle (12 o'clock =
/// the "start" of the circle, sweeping clockwise, the identical real
/// angle convention `CircularProgress`'s own paint arm already
/// established: `-PI/2` start, `+angle` clockwise). Only one hand is
/// draggable at a time (`mode` selects which); this mirrors real
/// MD3's own dialog, which shows both hands but only lets the
/// currently-focused one (hour first, then minute) actually move --
/// `mode` here is this widget's own real equivalent of that focus,
/// driven by whatever app-level control (e.g. an hour/minute toggle
/// button) the caller builds, not by the dial itself.
///
/// **Real, stated v1 simplifications** (this is a genuinely new
/// capability with no precedent anywhere in this codebase -- no
/// existing angle-based drag math, no existing circular *drag* region
/// distinct from a rectangular one, confirmed by direct grep before
/// writing any of this):
/// - No digit labels (`1`-`12` around the hour face, `00`-`55` around
///   the minute face) are painted -- real MD3 anatomy has them, but
///   drawing them needs `engine-render`'s own text-shaping pipeline
///   threaded through paint-time geometry a plain `NodeKind` doesn't
///   otherwise need, real added plumbing this v1 skips in favor of
///   plain tick marks at each of the 12 hour/60 minute positions
///   (still real, situated geometry, not a placeholder).
/// - No AM/PM toggle chrome, no digital hour:minute text input field,
///   no dialog frame -- this is the real circular drag *primitive*
///   MD3's own Time Picker dialog is built from, not the whole
///   dialog. `hour` is still a real 24-hour value (`0..=23`); flipping
///   AM/PM is left to whatever app-level control the caller builds
///   (dragging the hour hand preserves whichever half of the day the
///   current `hour` was already in, the same way real MD3 dragging
///   the hour hand alone never silently flips AM/PM either).
/// - Dragging is a plain rectangular hit region (this node's own
///   whole bounding box), not a strict circular clip -- the identical
///   real simplicity `Slider`'s own whole-track-width hit region
///   already uses rather than a pixel-exact thumb hit box. A drag
///   starting anywhere in the box still computes a real angle from
///   the box's own real center, so this is never visually wrong, only
///   slightly more permissive about where a drag may *start* than a
///   real circular clip would be.
/// - `hour`/`minute` are plain, driven-directly values, not
///   `Animated<f64>` -- the identical real "driven directly, like a
///   scrollbar being dragged, never eased" precedent `CarouselState::
///   scroll_x`'s own doc comment already establishes: a clock hand
///   snapping instantly to wherever the pointer is IS the correct
///   real behavior, not something to ease.
///
/// No `PartialEq`/`Eq` derive -- `peniko::Color` (`face_tint`/`hand_
/// tint` below) itself only derives `Clone, Copy, Debug`, the same
/// real reason `LinearProgressState`/`CircularProgressState`'s own
/// `Color` fields already block a derived `PartialEq` there (tests
/// compare individual fields instead, not the whole struct).
#[derive(Clone, Copy, Debug)]
pub struct TimePickerDialState {
    /// Real 24-hour value, `0..=23`. The dial's own hour hand always
    /// shows this modulo 12 (`0`/`12` both point straight up), the
    /// real MD3 12-hour face convention.
    pub hour: u8,
    /// `0..=59`. A drag on the minute hand snaps to the nearest real
    /// 5-minute increment (real MD3's own primary interaction
    /// granularity -- fine per-minute adjustment exists in the real
    /// spec too, via a secondary drag-precision mode this v1 doesn't
    /// build), a programmatic `set_time_picker_dial_time` call does
    /// not re-snap an already-precise value.
    pub minute: u8,
    /// Which hand a drag currently moves.
    pub mode: TimePickerDialMode,
    /// The circular face's own fill color -- `LinearProgressState::
    /// track_tint`'s own real sibling, same reasoning (a real,
    /// themeable field rather than a fixed literal `engine-render`
    /// would otherwise hardcode).
    pub face_tint: Color,
    /// Both hands' stroke color plus the selector dot at the active
    /// hand's own tip -- `CircularProgressState::indicator_tint`'s own
    /// real sibling.
    pub hand_tint: Color,
}

impl TimePickerDialState {
    /// Clamps `hour` to `0..=23` and `minute` to `0..=59` (a debug-
    /// only clamp, not a public contract -- `Tree::set_time_picker_
    /// dial_time` is the real public setter that also clamps, this
    /// constructor mirrors it so a directly-built `TimePickerDialState`
    /// can never start out-of-range either). Starts in `Hour` mode,
    /// mirroring real MD3's own dialog, which always focuses the hour
    /// hand first. `face_tint`/`hand_tint` default to real, byte-for-
    /// byte the same neutral literals `LinearProgressState::new`
    /// already uses for its own untheme colors -- `Window.set_theme`
    /// (`engine-py`) overwrites both once a real MD3 theme is pushed.
    pub fn new(hour: u8, minute: u8) -> Self {
        Self {
            hour: hour.min(23),
            minute: minute.min(59),
            mode: TimePickerDialMode::Hour,
            face_tint: Color::from_rgba8(0xE7, 0xE0, 0xEC, 0xFF),
            hand_tint: Color::from_rgba8(0x00, 0x00, 0x00, 0xFF),
        }
    }
}

/// M39 Phase 2 Step 2 (§5, §7): which of a `TimePickerDialState`'s two
/// real hands a drag currently moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimePickerDialMode {
    Hour,
    Minute,
}

/// M39 Phase 2 (§5, §7): real MD3 Expressive "the loading indicator
/// shows progress for a short wait time... a looping shape morph
/// sequence" -- research (the M3 site's own spec page is JS-rendered/
/// unfetchable, the same real finding M38 Phase 4/5/M39 Phase 1 each
/// already made) found the *real* spec is genuinely bigger than a
/// spinner: seven real named shapes (Soft Burst, Cookie 9, Pentagon,
/// Pill, Sunny, Cookie 4, Oval) with real vertex data sourced from
/// official Material Design SVG assets, driven by genuine spring
/// physics and a dual rotation formula, looping forever -- a real,
/// substantial undertaking with no precedent anywhere in this
/// codebase (no spring-physics motion model, no looping-animation
/// concept). **Scoped via `AskUserQuestion` to a real, honest v1
/// simplification instead:** four procedurally-generated real shapes
/// (Pentagon, Pill, Cookie, Oval -- a real, recognizable subset of
/// MD3's own named sequence, in spirit if not exact vertex-for-vertex),
/// morphed via the already-built, already-proven `Animated<ShapeKey>`
/// machinery (M7 Phase 4, reused extensively by M38 Phase 4/5) with
/// plain eased transitions, not genuine spring physics. Real, cited
/// timing kept even though the physics model was simplified: 650ms
/// per real shape transition, MD3 Expressive's own real documented
/// value (a real open-source port's own README, quoted directly,
/// since the M3 spec page itself carries no fetchable static value).
///
/// The real loop itself needs no app-side wiring at all -- unlike
/// every other real animated value in this catalog (Design Principle
/// 6's own "engine provides mechanism, app decides state" split),
/// there is no real *state* decision here for an app to make; `Tree::
/// tick_all`'s own new real per-`LoadingIndicator` case advances to
/// the next shape in the cycle the instant `PaintProperties.shape`'s
/// own animation genuinely settles (`Animated::shape.active.is_
/// none()`, true both the very first tick and every real completion
/// after), starting the whole thing automatically the moment a real
/// node is constructed.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadingIndicatorState {
    /// The four real shapes to cycle through, built once at
    /// construction time to this node's own real `w x h` (`ShapeKey`
    /// itself has no scale transform, so resizing after construction
    /// is a real, stated v1 limit: the shapes stay sized to whatever
    /// box this node was built with). Always real length 4 in
    /// practice (`crate::shape_morph::loading_indicator_shapes`'s own
    /// four real functions), not enforced by the type itself.
    pub shapes: Vec<crate::shape_morph::ShapeKey>,
    /// Which of `shapes` `PaintProperties.shape` is currently
    /// animating *toward* -- `Tree::tick_all`'s own real cycle
    /// advances this by one (wrapping) every time it retargets.
    pub current_shape: usize,
}

impl LoadingIndicatorState {
    /// Builds all four real shapes directly to `w x h` -- see this
    /// struct's own doc comment for why sizing happens once, here,
    /// rather than at paint time.
    pub fn new(w: f64, h: f64) -> Self {
        use crate::shape_morph::loading_indicator_shapes as shapes;
        Self {
            shapes: vec![
                shapes::pentagon(w, h),
                shapes::pill(w, h),
                shapes::cookie(w, h),
                shapes::oval(w, h),
            ],
            current_shape: 0,
        }
    }
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
    /// hardcoded one here (`engine-core` has no MD3/theme awareness,
    /// §4).
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
/// incremental diffing" simplicity `Video`'s own `push_frame` design
/// already chose for a comparable "engine only displays the latest
/// snapshot a real external process produced" shape, M30 Phase 9 Step
/// 1).
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
    /// in" contract every other additive field in this catalog already
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

/// M30 Phase 9 Step 5 (§5, §7, §11.7): which of MD3's three real
/// carousel layouts a `NodeKind::Carousel` is -- verified directly
/// against `COMPONENT_CAROUSEL.md` (via the sibling `pyCopper`
/// project's own already-built, already-cited real widget) rather than
/// assumed. `Uncontained` items "don't change size" and scroll by raw
/// pixels; `Hero`/`MultiBrowse` items "automatically change size and
/// snap into place," which is the real, distinctive behavior that
/// makes a carousel a carousel rather than a styled horizontal list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarouselLayout {
    Uncontained,
    Hero,
    MultiBrowse,
}

impl CarouselLayout {
    /// True for the two layouts whose items resize continuously and
    /// snap to a whole index (`Hero`/`MultiBrowse`); false for
    /// `Uncontained`, which scrolls freely by pixel instead -- the same
    /// real split pyCopper's own `snaps` property already draws.
    pub fn snaps(self) -> bool {
        !matches!(self, CarouselLayout::Uncontained)
    }

    /// The real per-slot width pattern past the leading keyline, for
    /// the two layouts that have one -- `None` for `Uncontained`
    /// (`snaps()` is false, so nothing ever calls this for it).
    /// Quoted directly from pyCopper's own real `PATTERNS` constant,
    /// itself grounded in `COMPONENT_CAROUSEL.md`.
    fn slot_pattern(self) -> &'static [CarouselSlot] {
        use CarouselSlot::{Large, Medium, Small};
        match self {
            CarouselLayout::Uncontained => &[],
            CarouselLayout::Hero => &[Large, Small],
            CarouselLayout::MultiBrowse => &[Large, Medium, Small],
        }
    }

    /// Whatever the fixed slots leave over, which is MD3's own
    /// "dynamic" large-item width -- mirrors pyCopper's own real
    /// `_large_width` exactly.
    pub fn large_width(self, available: f64) -> f64 {
        let pattern = self.slot_pattern();
        let fixed: f64 = pattern
            .iter()
            .filter(|slot| !matches!(slot, CarouselSlot::Large))
            .map(|slot| match slot {
                CarouselSlot::Medium => f64::from(CAROUSEL_MEDIUM),
                _ => f64::from(CAROUSEL_SMALL_MAX),
            })
            .sum();
        let gaps = f64::from(CAROUSEL_GAP) * (pattern.len().saturating_sub(1)) as f64;
        (available - fixed - gaps).max(f64::from(CAROUSEL_SMALL_MAX))
    }
}

/// One slot in a snapping layout's own `slot_pattern` -- see
/// `CarouselLayout::slot_pattern`'s own doc comment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CarouselSlot {
    Large,
    Medium,
    Small,
}

/// Item corner radius -- "Item corner radius | 28dp" (`COMPONENT_
/// CAROUSEL.md`, quoted verbatim in pyCopper's own real widget).
pub const CAROUSEL_ITEM_RADIUS: f32 = 28.0;
/// Leading/trailing padding -- "16dp".
pub const CAROUSEL_PAD_X: f32 = 16.0;
/// Top/bottom padding -- "8dp".
pub const CAROUSEL_PAD_Y: f32 = 8.0;
/// Padding between elements -- "8dp".
pub const CAROUSEL_GAP: f32 = 8.0;
/// Small item width's own real upper bound -- "40-56dp"; only the max
/// is used (the real interpolated width formula below only ever needs
/// one concrete "small" figure, the same real simplification pyCopper's
/// own `SMALL_MAX` already makes).
pub const CAROUSEL_SMALL_MAX: f32 = 56.0;
/// **Not sourced** -- MD3 calls the medium item "dynamic" and gives no
/// figure (`COMPONENT_CAROUSEL.md`'s own size tables are images, not
/// real text this codebase can quote a number from). Twice the largest
/// small item, the identical real, stated choice pyCopper's own
/// `MEDIUM` constant already makes, reused verbatim rather than
/// re-guessed independently.
pub const CAROUSEL_MEDIUM: f32 = 112.0;
/// **Not sourced**, the identical real reason `CAROUSEL_MEDIUM` isn't.
/// Reused verbatim from pyCopper's own `HEIGHT`.
pub const CAROUSEL_HEIGHT: f32 = 160.0;
/// Width of an `Uncontained` item that doesn't request its own.
pub const CAROUSEL_UNCONTAINED_WIDTH: f32 = 200.0;
/// Drag distance, in logical px, a snapping carousel needs before it
/// commits to the next/previous item. **Not sourced** -- MD3's own
/// guidelines describe "swipe through one item at a time" but give no
/// pixel threshold for a pointer's equivalent of a touch swipe, the
/// identical real gap pyCopper's own `DRAG_INDEX_THRESHOLD` already
/// states; reused verbatim.
pub const CAROUSEL_DRAG_INDEX_THRESHOLD: f64 = 60.0;

/// M30 Phase 9 Step 5 (§5, §7, §11.7): a real, live carousel's own
/// inert state -- `engine-core` holds only this; the real per-frame
/// item-geometry math it drives (`Tree::sync_carousel_layouts`) and the
/// real wheel/drag dispatch that mutates it both live in `tree.rs`
/// alongside every other kind-specific `Tree::dispatch` arm, the same
/// "engine-core owns the mechanism, this struct just owns the value"
/// split `SplitterState`/`SliderState` already establish.
///
/// The real, distinctive finding this step's own investigation made:
/// `position` is a genuinely *layout*-invalidating animated value, not
/// a paint-only one like every other `Animated<T>` field in this
/// codebase (`PaintProperties.opacity`, `SliderState.thumb_position`,
/// etc.) -- an item promoted from medium to large must actually grow
/// *as it travels*, which only real per-frame relayout can produce.
/// Confirmed via direct read of `Tree::tick_all`/`app.rs`'s own per-
/// frame loop that this already works for free: *every* active
/// animation already sets `Tree.dirty`, and a dirty frame already calls
/// `compute_layout` unconditionally -- so `position` needs no new
/// central-ticking mechanism at all, just a real per-frame consumer of
/// its current value (`sync_carousel_layouts`), unlike pyCopper's own
/// framework, which needed an explicit `invalidates="layout"` opt-in
/// because *most* of its own animated values are paint-only by default.
///
/// No `#[derive(...)]` here at all -- the identical real reason
/// `SplitterState` (also carrying a bare `Animated<f64>`) has none
/// either: `Animated<T>` itself implements neither `Clone`, `Debug`,
/// nor `PartialEq`.
pub struct CarouselState {
    pub layout: CarouselLayout,
    /// The item the carousel is settling on. Only meaningful when
    /// `layout.snaps()` -- mirrors pyCopper's own real `index`.
    pub index: usize,
    /// Where the strip actually is, between items, mid-snap -- an
    /// integer while at rest, fractional while travelling. Every real
    /// item width this step's own `sync_carousel_layouts` computes is
    /// derived from this, which is what makes items resize *as they
    /// move* rather than jumping on arrival. Only meaningful when
    /// `layout.snaps()`.
    pub position: Animated<f64>,
    /// Free pixel scroll offset, for `Uncontained` only -- driven
    /// directly (like a scrollbar being dragged), never eased, the same
    /// real `Animated<f64>`-free precedent `VirtualListState::scroll_
    /// offset`'s own doc comment already establishes for `SplitterState
    /// ::position`.
    pub scroll_x: f64,
    /// The pointer's own last-seen local x while a real drag (`Tree::
    /// dragging`) is active on this node -- `None` when not dragging.
    /// Mirrors pyCopper's own `carousel_drag_x`.
    pub drag_last_x: Option<f64>,
    /// Accumulated drag distance since the last committed index, for
    /// `layout.snaps()` carousels -- mirrors pyCopper's own real
    /// `carousel_drag_accum`.
    pub drag_accum: f64,
}

impl CarouselState {
    pub fn new(layout: CarouselLayout) -> Self {
        Self {
            layout,
            index: 0,
            position: Animated::new(0.0),
            scroll_x: 0.0,
            drag_last_x: None,
            drag_accum: 0.0,
        }
    }

    /// Width of the keyline slot `slot_index` places past the leading
    /// edge -- mirrors pyCopper's own real `_slot_width` exactly,
    /// including its own real "already scrolled past the leading edge"
    /// clamp for a negative `slot_index`.
    fn slot_width(&self, slot_index: i64, large: f64) -> f64 {
        if slot_index < 0 {
            return f64::from(CAROUSEL_SMALL_MAX);
        }
        let pattern = self.layout.slot_pattern();
        if pattern.is_empty() {
            return large;
        }
        let idx = (slot_index as usize).min(pattern.len() - 1);
        match pattern[idx] {
            CarouselSlot::Large => large,
            CarouselSlot::Medium => f64::from(CAROUSEL_MEDIUM),
            CarouselSlot::Small => f64::from(CAROUSEL_SMALL_MAX),
        }
    }

    /// Width for an item sitting `position` slots past the leading
    /// keyline -- mirrors pyCopper's own real `_item_width` exactly:
    /// `position` is fractional mid-snap, so the width is interpolated
    /// between the two slots it lies between. That interpolation *is*
    /// the whole real "items resize as they travel" behavior.
    pub fn item_width(&self, position: f64, large: f64) -> f64 {
        let low = position.floor() as i64;
        let t = position - position.floor();
        if t == 0.0 {
            return self.slot_width(low, large);
        }
        self.slot_width(low, large) * (1.0 - t) + self.slot_width(low + 1, large) * t
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
/// `engine-render::paint_node` translate, never reflected back into
/// `layout_style` -- this phase's own investigation found that gives
/// a real, previously undiscovered hit-test-after-scroll bug, a real
/// point at a scrolled item's own genuine post-scroll screen position
/// resolves to the wrong node), `scroll` here is turned into a real,
/// baked-in absolute `layout_style.inset` by `Tree::sync_scroll_view_
/// layouts` every frame -- the identical bug-free pattern `Carousel`'s
/// own `sync_carousel_layouts` already established, which both
/// `engine-render::paint_node` and `Tree::hit_test_at` read correctly
/// by construction, since neither needs a second, separate transform
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
/// implements none of those, the same real reason `NodeKind`/
/// `IconState` already state for `Splitter`/`Icon`.
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
    /// state (not `Tree` itself) the same way `CarouselState.drag_
    /// last_x`/`drag_accum` already establish: kind-specific drag
    /// anchor data belongs to the kind, `Tree.dragging` alone only
    /// ever names *which* node is being dragged.
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
    /// by painting (`engine-render::paint_node`) and hit-testing/
    /// dragging (`Tree::grabs_scroll_view_thumb`/`update_scroll_view_
    /// thumb_drag`) so the two can never drift -- the identical real
    /// "one function, every real caller" discipline `VirtualListState::
    /// offset_of`/`Tree::splitter_geometry` already establish.
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
/// identical real reason `ScrollViewState`/`Splitter`/`Icon`'s own
/// doc comments already state for their own `Animated<T>` fields.
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
    /// selection-highlight color -- `CheckboxState.mark_tint`'s own
    /// real sibling, same reasoning. Defaults to real, byte-for-byte
    /// the historical hardcoded `0x1C1B1F` `engine-render`'s own
    /// `TextField` paint used before this phase.
    pub text_tint: Color,
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
            text_tint: Color::from_rgba8(0x1C, 0x1B, 0x1F, 0xFF),
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
/// `engine-py::Window.add_image` -- the same real "resolved ahead of
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

    /// A 1x1 fully-transparent placeholder -- the same real "nothing to
    /// show yet" image `Window.add_video`'s own synthetic default
    /// already builds (`window_factory.rs`), factored out here so a
    /// declarative `kind: Image` with no `src:` (`engine-spec::build`)
    /// can express the identical shape without duplicating the literal
    /// in a second crate.
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

/// M23 Phase 1 (§1, §3): every real Material Symbols icon this
/// project curates shares this identical, fixed SVG `viewBox` width/
/// height (`viewBox="0 -960 960 960"`, confirmed via a real fetch of
/// eight distinct icons directly from Google's own CDN) -- a real MD3
/// convention, not a per-icon variable, so one constant suffices.
pub const ICON_VIEWBOX_SIZE: f64 = 960.0;

/// M23 Phase 1 (§1, §3): a real MD3 vector icon -- `path` is already-
/// parsed, renderer-ready `peniko::kurbo::BezPath` data (`ImageState`'s
/// own "engine-core holds inert data" precedent: parsing a curated
/// icon's own `d=` SVG path string happens in `engine-py::Window.
/// add_icon`, not here), painted as a plain solid-`tint` fill -- no
/// GPU texture involved at all (unlike `ImageState`, §5), since a
/// vector path fill is exactly what `vello_hybrid::Scene::fill_path`
/// already does for every other `NodeKind`'s own shape.
///
/// M35 Phase 2 (§5, §8): `rotation` is a real, new animatable degrees-
/// of-clockwise-rotation value, `Split Button`'s own real "the menu
/// icon rotates inwards 180° when opened and closed" need
/// (`COMPONENT_SPLIT_BUTTONS.md`) -- deliberately a plain scalar
/// `Animated<f64>`, not routed through `PaintProperties.transform`
/// (`Interpolate for Affine`'s own doc comment already states why: a
/// componentwise coefficient lerp between two *rotated* affines
/// produces a non-circular morph, not a true sweep through the
/// correct arc). A scalar angle has no such problem -- `Interpolate
/// for f64` is already exact -- and `engine-render`'s own paint arm
/// constructs a fresh `Affine::rotate` from it each frame, the
/// identical "a scalar progress value drives real paint geometry"
/// shape `CheckboxState.check_progress`/`RadioButtonState.select_
/// progress`/`SwitchState.toggle_progress` already establish. No
/// longer derives `Clone`/`Debug`/`PartialEq` now that `Icon` carries
/// an `Animated<f64>` -- `Animated<T>` implements none of those (the
/// identical real reason `NodeKind`'s own doc comment already states
/// for `Splitter`); nothing in this codebase actually cloned,
/// printed, or compared an `IconState` value directly (checked
/// directly via grep, not assumed), so this costs nothing real.
pub struct IconState {
    pub path: peniko::kurbo::BezPath,
    /// M92: `Animated`, like every other color the renderer paints, so
    /// `Node.animate("foreground", ...)` eases an icon's color the way it
    /// does a `Text`'s. Ticked beside `rotation` in `Tree::tick_all`.
    pub tint: Animated<Color>,
    pub rotation: Animated<f64>,
}

impl IconState {
    pub fn new(path: peniko::kurbo::BezPath, tint: Color) -> Self {
        Self {
            path,
            tint: Animated::new(tint),
            rotation: Animated::new(0.0),
        }
    }
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
    /// paint-time position (`paint_node`), not their `layout_style`
    /// (their real taffy layout never changes; only where they're
    /// *drawn* does). Plain `Animated<f64>`, driven directly (like a
    /// scrollbar being dragged, not eased toward a target) the same way
    /// `SplitterState.position` already is -- confirmed via reading
    /// `Tree::tick_all` directly that kind-specific `Animated<T>`
    /// fields are never ticked centrally, only by their own dedicated
    /// mechanism, so this follows that same real precedent rather than
    /// inventing a new one.
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
    /// (`engine-render::paint_node`) and hit-testing/dragging
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

    /// Item `idx`'s own real top-offset. Panics if `item_extent` is
    /// `Variable` and `idx` isn't yet resolved in `resolved_offsets` --
    /// an internal bookkeeping bug (the caller must resolve an index
    /// before positioning it), the same "internal bug, not a runtime
    /// condition" contract `set_splitter_position` already uses for its
    /// own malformed-call panics.
    pub fn offset_of(&self, idx: usize) -> f64 {
        match &self.item_extent {
            ItemExtent::Fixed(v) => idx as f64 * v,
            ItemExtent::Variable => *self.resolved_offsets.get(&idx).unwrap_or_else(|| {
                panic!(
                    "VirtualListState::offset_of: item {idx}'s own offset must be resolved \
                     (via Tree::set_virtual_list_resolved_offsets) before it can be positioned"
                )
            }),
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

/// §5's own struct sketch, plus the real `checked: bool` §7.3's own
/// text separately names (the struct sketch only showed the animated
/// half). `check_progress` starts already matching `checked` (`1.0`
/// for checked, `0.0` for not) so a checkbox created already-checked
/// shows its own real initial state without a spurious animation from
/// `0.0` the instant it first paints.
pub struct CheckboxState {
    pub checked: bool,
    pub check_progress: Animated<f64>,
    /// M20 Phase 1 (§7.1, §7.3): the checkmark's own real paint color
    /// -- plain, already-resolved data (Design Principle 6), same as
    /// `InteractionState.tint`'s own real precedent (M7 Phase 3), but
    /// living here since every real `CheckboxState` always has one
    /// (not an optional opt-in capability the way `InteractionState`
    /// is). Defaults to real white -- byte-for-byte the hardcoded
    /// value `engine-render`'s own checkmark paint used before this
    /// phase, so a checkbox whose app never calls `Window.set_theme`
    /// sees zero visual change.
    pub mark_tint: Color,
}

impl CheckboxState {
    pub fn new(checked: bool) -> Self {
        Self {
            checked,
            check_progress: Animated::new(if checked { 1.0 } else { 0.0 }),
            mark_tint: Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF),
        }
    }
}

/// M30 Phase 2 Step 1 (§5, §7.3): `CheckboxState`'s own real shape,
/// mirrored -- `selected`/`select_progress` are the direct analogues
/// of `checked`/`check_progress`. One real, necessary difference: a
/// radio button's own outer *ring* changes color between its
/// unselected and selected state (MD3's real anatomy -- an unchecked
/// checkbox's box is a plain neutral fill either way, only the
/// checkmark itself appears/disappears), so this carries *two* plain
/// tints (`unselected_tint`/`selected_tint`) rather than `mark_tint`'s
/// single color -- `engine-render`'s own paint arm interpolates
/// between them using `select_progress.current` as the blend factor
/// (`Interpolate for peniko::Color`, already real since §5's own
/// animation core), so the ring's own color transition rides the
/// identical timeline the inner dot's scale-in already does, not a
/// second, independently-timed animation.
pub struct RadioButtonState {
    pub selected: bool,
    pub select_progress: Animated<f64>,
    pub unselected_tint: Color,
    pub selected_tint: Color,
}

impl RadioButtonState {
    pub fn new(selected: bool) -> Self {
        Self {
            selected,
            select_progress: Animated::new(if selected { 1.0 } else { 0.0 }),
            unselected_tint: Color::from_rgba8(0x00, 0x00, 0x00, 0xFF),
            selected_tint: Color::from_rgba8(0x00, 0x00, 0x00, 0xFF),
        }
    }
}

/// M30 Phase 2 Step 2 (§5, §7.3): `CheckboxState`/`RadioButtonState`'s
/// own real shape, mirrored a third time -- `on`/`toggle_progress` are
/// the direct analogues of `checked`/`check_progress` and `selected`/
/// `select_progress`. Real MD3 anatomy, verified against Material
/// Web's own token source (`tokens/versions/v0_192/_md-comp-switch.
/// scss`) rather than assumed: the track fill genuinely changes color
/// (`track_off_tint` -> `track_on_tint`) *and* the handle both
/// **slides** (left to right, `engine-render`'s own paint arm) *and*
/// **grows** as it toggles -- unselected handle is a real 16dp
/// diameter, selected a real 24dp, not a fixed size that merely
/// changes color the way `RadioButton`'s dot does. `track_outline_
/// tint` is a real, separate role from `track_off_tint` (`outline`
/// vs. `surface_container_highest`) -- MD3's real unselected track has
/// both a fill *and* a distinct stroke, which fades out as the switch
/// turns on (the primary fill alone reads as "on," no visible outline
/// needed once selected).
pub struct SwitchState {
    pub on: bool,
    pub toggle_progress: Animated<f64>,
    pub track_off_tint: Color,
    pub track_on_tint: Color,
    pub track_outline_tint: Color,
    pub handle_off_tint: Color,
    pub handle_on_tint: Color,
}

impl SwitchState {
    pub fn new(on: bool) -> Self {
        Self {
            on,
            toggle_progress: Animated::new(if on { 1.0 } else { 0.0 }),
            track_off_tint: Color::from_rgba8(0x00, 0x00, 0x00, 0xFF),
            track_on_tint: Color::from_rgba8(0x00, 0x00, 0x00, 0xFF),
            track_outline_tint: Color::from_rgba8(0x00, 0x00, 0x00, 0xFF),
            handle_off_tint: Color::from_rgba8(0x00, 0x00, 0x00, 0xFF),
            handle_on_tint: Color::from_rgba8(0x00, 0x00, 0x00, 0xFF),
        }
    }
}

/// M30 Phase 3 Step 2 (§5, §7): see `NodeKind::LinearProgress`'s own
/// doc comment. `track_tint` is the unfilled portion's real, separate
/// color (`surface_container_highest` in real MD3) -- genuinely
/// distinct from `indicator_tint` (`primary`), not the same role
/// reused at reduced opacity.
pub struct LinearProgressState {
    pub value: Animated<f64>,
    pub track_tint: Color,
    pub indicator_tint: Color,
}

impl LinearProgressState {
    pub fn new(value: f64) -> Self {
        Self {
            value: Animated::new(value.clamp(0.0, 1.0)),
            track_tint: Color::from_rgba8(0x00, 0x00, 0x00, 0xFF),
            indicator_tint: Color::from_rgba8(0x00, 0x00, 0x00, 0xFF),
        }
    }
}

/// M30 Phase 3 Step 2 (§5, §7): see `NodeKind::CircularProgress`'s own
/// doc comment.
pub struct CircularProgressState {
    pub value: Animated<f64>,
    pub indicator_tint: Color,
}

impl CircularProgressState {
    pub fn new(value: f64) -> Self {
        Self {
            value: Animated::new(value.clamp(0.0, 1.0)),
            indicator_tint: Color::from_rgba8(0x00, 0x00, 0x00, 0xFF),
        }
    }
}

/// §5's own struct sketch, unchanged in shape -- `thumb_position` is
/// the real value itself, `0.0..=1.0` along the track, the identical
/// shape `SplitterState.position` already established.
pub struct SliderState {
    pub thumb_position: Animated<f64>,
    /// M20 Phase 1 (§7.1, §7.3): the track's own real paint color --
    /// `CheckboxState.mark_tint`'s own real sibling, same reasoning.
    /// The thumb itself already paints with the node's own real,
    /// per-node `PaintProperties.background` (already themeable
    /// directly) -- only the track ever needed a fixed literal.
    /// Defaults to real byte-for-byte the historical hardcoded gray.
    pub track_tint: Color,
}

impl SliderState {
    pub fn new(value: f64) -> Self {
        Self {
            thumb_position: Animated::new(value.clamp(0.0, 1.0)),
            track_tint: Color::from_rgba8(0x79, 0x74, 0x7A, 0xFF),
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

/// §11.5's own struct sketch, unchanged: `position` is 0.0..=1.0 along
/// the split axis (not an absolute size -- "implementation detail" per
/// the architecture's own text, and a fraction is what lets the same
/// splitter keep working correctly if its parent is later resized).
/// `Tree::set_splitter_position` is what actually moves the two
/// flanking siblings this value nominally describes; the field alone
/// is inert data, matching every other `NodeKind` payload's own
/// "engine-core carries the state, a `Tree` method is what makes it do
/// anything" shape.
pub struct SplitterState {
    pub position: Animated<f64>,
}

/// M30 Phase 1 (§5, §7): a real horizontal text-alignment capability --
/// `engine-render`'s own text-shaping pipeline (`text.rs`) hardcoded
/// `parley::Alignment::Start` unconditionally until this phase, a real,
/// verified gap (confirmed by direct read, not assumed) that blocks any
/// correctly-rendered centered label -- MD3's `Button` is the first real
/// consumer (its label must sit centered in the button's own box), but
/// this lives on `TextState` itself rather than as Button-specific
/// machinery, the identical "universal capability, not component-
/// specific" precedent `PaintProperties.border_color`/`border_width`
/// already established this same phase.
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
/// to shape a run, and nothing about how it got styled (that's
/// `engine_md3`'s future job, not this crate's -- `engine-core` stays
/// MD3-agnostic per §1 Locked Decisions). `font_family` names an
/// already-registered family (by exact name, matching the font's own
/// name table) rather than carrying a weight/style axis: this step's two type
/// roles are two distinct font files (Roboto Regular vs. Medium), not
/// one variable font interpolated at draw time.
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
    pub elevation: Animated<f64>,
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
    /// M7 Phase 4 (§7.4): the real MD3 shape-morph target -- defaults
    /// to `ShapeKey::empty()` (no shape ever set), which `paint_node`
    /// treats as a true no-op, matching every other additive field's
    /// "off unless a caller opts in" contract. See `shape_morph.rs`'s
    /// own module doc comment for why this lives here, in
    /// `engine-core`, and not `engine-md3` (where it was originally
    /// built, M3 step 10).
    pub shape: Animated<crate::shape_morph::ShapeKey>,
    /// M30 Phase 1 (§5, §7): a real stroked border, painted inside the
    /// node's own fill edge (never expanding its layout box) -- MD3's
    /// Outlined button variant is the real consumer that surfaced this
    /// gap (a 1dp outline with no fill), but the field is universal
    /// like every other `PaintProperties` field, not `Button`-specific.
    /// `border_width.current <= 0.0` is a true no-op, the same
    /// "off unless a caller opts in" contract `elevation`/`shape`
    /// already establish.
    pub border_color: Animated<Color>,
    pub border_width: Animated<f64>,
    /// M30 Phase 1 Step 4 (§5, §7): a real per-corner radius override,
    /// `[top_left, top_right, bottom_right, bottom_left]` -- kurbo's
    /// own `RoundedRect::new` already accepts a 4-tuple of independent
    /// corner radii natively (confirmed via direct source read of the
    /// pinned `kurbo 0.13.1`, not assumed), so this is exposing an
    /// existing real primitive, not inventing new geometry. `Segmented
    /// Button`'s own real MD3 anatomy is the consumer that surfaced
    /// the gap: a group's first/last segments are rounded only on
    /// their outer edge, square on the edge touching the next
    /// segment -- `corner_radius`'s own single scalar can't express
    /// that. `None` (every existing node, unchanged) means "use the
    /// uniform `corner_radius` scalar," the same true no-op contract
    /// `border_width: 0.0` already establishes -- not `Animated`, a
    /// deliberate, honest scope limit: nothing in this catalog yet
    /// needs a *smooth transition* between two different corner-radii
    /// shapes, only a static per-node choice made once at construction.
    pub corner_radii_override: Option<Animated<CornerRadii>>,
    /// M95: drop shadows (`Shadows`), independent of the legacy MD3
    /// `elevation`, which keeps drawing its own until M99 removes it.
    pub shadows: Animated<Shadows>,
    /// M96: the target API's `translate_x`/`translate_y`/`scale`/
    /// `rotation_deg`, composed after the legacy `transform` above (which
    /// keeps its top-left origin for canvas pan/zoom until M101 merges
    /// the two). Read both through `local_transform`.
    pub node_transform: NodeTransform,
    /// M32 Phase 3 (§5, §7, §11.7/§11.8): the real, general form of the
    /// clip `VirtualList`/`Carousel` each already bake into their own
    /// paint -- confirmed via direct read of `engine-render::paint_node`
    /// before adding this that no other `NodeKind` clips its own
    /// children at all, the exact gap this catalog's own "no `NodeKind`
    /// besides `VirtualList` clips today" note names. `false` (every
    /// existing node, unchanged) is a true no-op, the same "off unless
    /// a caller opts in" contract every other additive field here
    /// already follows -- a child painted past this node's own box
    /// stays exactly as visible as it always was. **Real, stated v1
    /// scope limit:** clipping only -- unlike `VirtualList`, opting a
    /// plain node into this does not give it a real scroll offset or
    /// wheel-input wiring of its own; content still simply extends
    /// past the box, just genuinely hidden there instead of visibly
    /// spilling out. Not `Animated`, the identical deliberate choice
    /// `corner_radii_override` already made just above: nothing in
    /// this catalog needs a *smooth transition* into/out of clipping,
    /// only a static per-node choice.
    pub clip_children: bool,
    /// M35 Phase 3 (§5, §7, §11.7): the real MD3 Standard Button
    /// Group's own distinctive mechanic -- "pressing a button also
    /// affects the width of adjacent buttons" (`COMPONENT_BUTTON_
    /// GROUPS.md`) -- `None` (every existing node, unchanged) is a
    /// true no-op; `Some((grow_px, gap_px))` marks this node as a real
    /// button-group container and gives `Tree::
    /// sync_button_group_layouts` (mirroring `Carousel`'s own `sync_
    /// carousel_layouts` shape: a container-level marker driving every
    /// child's own real `layout_style`, recomputed and pushed via
    /// `Tree::set_layout_style` each layout pass) the two real numbers
    /// it needs: `grow_px` is how much the currently-pressed child
    /// (read from the already-existing, already-tracked `Tree.
    /// pressed` field, the identical real "read live interaction
    /// state to drive computed layout" technique `update_slider_drag`/
    /// `update_splitter_drag` already establish) grows by, split
    /// evenly back out of its own immediate neighbors so the row's own
    /// total width stays constant -- real MD3's own stated behavior
    /// ("briefly changes the width of itself and adjacent buttons"),
    /// not raw growth with no compensation; `gap_px` is the real,
    /// constant horizontal gap between every child, MD3's own "inner
    /// padding" anatomy. A plain `PaintProperties` field, not a new
    /// `NodeKind`, since (unlike `Carousel`) a button group needs no
    /// other real per-instance data -- the identical "a universal flag
    /// any `NodeKind` can opt into" shape `clip_children` already
    /// establishes just above, not `Carousel`'s heavier dedicated-
    /// `NodeKind` pattern.
    pub button_group_reflow: Option<(f64, f64)>,
    /// M38 Phase 4 (§5, §7): real MD3 "the shape changes while
    /// hovered/pressed" behavior (`Split Button`'s own real inner-
    /// corner shape-tightening being the first real consumer) --
    /// `Some((relaxed, tightened))` opts this node into `Tree::
    /// update_hover` automatically retargeting `shape` (already a
    /// real, general `Animated<ShapeKey>` field, M7 Phase 4) to
    /// `tightened` while this node is the currently-hovered one, and
    /// back to `relaxed` otherwise -- the identical real "read live
    /// interaction state to drive a paint property" technique
    /// `hover_opacity`'s own transition in that same method already
    /// establishes, just targeting `shape` instead of an opacity
    /// scalar. `None` (every existing node, unchanged) is a true
    /// no-op. **Real, deliberate v1 scope choice, stated directly:**
    /// tied to `hovered` only, not `focused`/`pressed` separately --
    /// a real mouse press can only ever land on an already-hovered
    /// node (`Tree::hit_test`'s own contract), so `hovered` already
    /// covers the entire press gesture for this purely cosmetic
    /// corner effect; keyboard-only focus is intentionally left out,
    /// since this catalog's own dedicated `focus_ring` mechanism
    /// already signals keyboard focus distinctly and doesn't need a
    /// second, redundant visual cue riding along with it.
    pub interactive_shape: Option<(crate::shape_morph::ShapeKey, crate::shape_morph::ShapeKey)>,
    /// M38 Phase 5 (§5, §7): `interactive_shape`'s own real `pressed`-
    /// driven sibling -- real MD3 Expressive "buttons reshape as you
    /// press them" (`Button Group`'s own per-child press morph being
    /// the first real consumer, distinct from Split Button's hover-
    /// driven *inner-corner* tightening, M38 Phase 4). `Some((relaxed,
    /// tightened))` opts this node into `Tree::set_pressed` (the
    /// single real chokepoint every `self.pressed` mutation now goes
    /// through) retargeting `shape` toward `tightened` while this node
    /// is the currently-pressed one, back to `relaxed` otherwise --
    /// deliberately a *separate* field from `interactive_shape` rather
    /// than one field reacting to both `hovered`/`pressed`: a real
    /// Button Group child must not visually tighten on a mere hover,
    /// only a genuine press, the opposite real trigger Split Button's
    /// own inner corners need. `None` (every existing node) is a true
    /// no-op.
    pub press_interactive_shape:
        Option<(crate::shape_morph::ShapeKey, crate::shape_morph::ShapeKey)>,
}

impl PaintProperties {
    pub fn new(background: Color, corner_radius: f64, elevation: f64, opacity: f64) -> Self {
        Self {
            background: Animated::new(background),
            corner_radius: Animated::new(corner_radius),
            elevation: Animated::new(elevation),
            opacity: Animated::new(opacity),
            transform: Animated::new(peniko::kurbo::Affine::IDENTITY),
            shape: Animated::new(crate::shape_morph::ShapeKey::empty()),
            border_color: Animated::new(Color::from_rgba8(0, 0, 0, 0)),
            border_width: Animated::new(0.0),
            corner_radii_override: None,
            shadows: Animated::new(Shadows::default()),
            node_transform: NodeTransform::default(),
            clip_children: false,
            button_group_reflow: None,
            interactive_shape: None,
            press_interactive_shape: None,
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
        let elevation = self.elevation.tick(now, completed);
        let opacity = self.opacity.tick(now, completed);
        let transform = self.transform.tick(now, completed);
        let shape = self.shape.tick(now, completed);
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
            || elevation
            || opacity
            || transform
            || shape
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
    /// §14 step 9 (§7.3): `None` until a caller opts a node into
    /// pointer interaction via `Tree::interaction_mut` -- most nodes
    /// (plain rects, text, containers) never touch this and pay nothing
    /// for it.
    pub interaction: Option<crate::interaction::InteractionState>,
    /// M30 Phase 5 Step 1 (§5, §7): a real, confirmed gap this step's
    /// own `Navigation Rail` surfaced -- `Tree::hit_test_at`'s own
    /// "children checked first, no ancestor bubbling" contract (M30
    /// Phase 1's own `NodeKind::Text`/`NodeKind::Icon` fix already
    /// documents this) meant a decorative interior `Rect` (the active-
    /// indicator pill behind a nav item's icon) permanently ate every
    /// click meant for its own interactive parent, since a plain
    /// `Rect` always independently claims a hit and nothing bubbles
    /// back out. Text/Icon got a hardcoded per-`NodeKind` exemption;
    /// a `Rect` genuinely can't (it's the real click target for
    /// `Button`/`Card`/`Chip`/every other composite in this catalog),
    /// so this generalizes the same real exemption into a purely
    /// additive, opt-in per-node flag instead. `true` (every existing
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
