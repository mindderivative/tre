# Shapes

`tre-engine`'s `shapes` module is the retained-mode convenience layer external UI frameworks build on: `ShapeRegistry` and a handful of concrete shape types (`Rectangle`, `Circle`, `Polygon`, `Path`, `Text`, `Svg`, `CustomShaded`). Every shape compiles down into the exact same immediate-mode `RenderingCanvas` calls any other caller would issue -- this module is a convenience, retained-store layer over the existing IR/sort/batch/RHI pipeline, never a second rendering path. (Caret/selection *editing* logic lives in the separate `tre-text` crate -- see [Text, SVG & Atlas](text-svg-atlas.md).)

## Shared building blocks

```rust
pub type Vec2 = [f32; 2];   // a plain 2D point/extent
pub type Color = u32;       // UiVertex::color's packed-RGBA8 convention

pub struct Transform2D {
    pub position: Vec2,
    pub scale: Vec2,
    pub rotation: f32,   // radians
}
impl Transform2D {
    pub const IDENTITY: Self;
    pub fn to_affine2(&self) -> tre_math::Affine2;  // translate * rotate * scale composition
}

pub enum BlendMode { Normal, Multiply, Screen, Overlay, SoftLight, ColorDodge }
pub enum Visibility { Visible, Hidden, Collapsed }

pub struct PrimitiveCommon {
    pub transform: Transform2D,
    pub opacity: f32,
    pub blend_mode: BlendMode,
    pub visibility: Visibility,
    pub hit_testable: bool,
}

pub trait Primitive {
    fn common(&self) -> &PrimitiveCommon;
    fn common_mut(&mut self) -> &mut PrimitiveCommon;
}
```

`Transform2D` is deliberately **not** `tre_math::Affine2` itself -- a raw matrix's rotation component can't be cleanly interpolated for animation, so shapes store the decomposed position/scale/rotation and resolve to a real `Affine2` once per frame during flattening. `BlendMode` is real for `Polygon`/`Path` solid fill only (gated behind `RhiDevice::local_read_blend_supported`, falling back to `Normal` on hardware without it) -- `Rectangle`/`Circle` and non-solid fills don't read it yet. `Visibility::Hidden` and `Collapsed` render identically today; the distinction exists for a future layout system to use without a breaking data-model change.

`Primitive` is an ordinary object-safe accessor trait, not a marker for dynamic dispatch -- every real call site uses the `ShapePrimitive` enum, never `dyn Primitive`, per the workspace's "no dynamic type inspection in hot paths" rule.

## Fill

```rust
pub enum FillStyle { Solid(Color), Gradient(GradientId), Texture(u32) }

pub struct GradientId(pub u32);
pub struct GradientDef { pub kind: GradientKind, pub stops: Vec<GradientStop> }
pub enum GradientKind {
    Linear { start: Vec2, end: Vec2 },
    Radial { center: Vec2, radius: f32 },
}
pub struct GradientStop { pub position: f32, pub color: Color }

pub enum GradientError {
    NoStops,
    TooManyStops { count: usize, max: usize },
    StopPositionOutOfRange { index: usize, position: f32 },
    StopsNotAscending { index: usize },
    NonPositiveRadius(f32),
}
```

Every `FillStyle` variant is real for `Rectangle`/`Circle`/`Polygon`/`Path` (`Text`/`Svg` are solid-fill-only, a disclosed scope boundary). A `GradientId` is scoped to the `ShapeRegistry` that created it via `create_gradient` -- the gradient table is append-only with no generational reuse (no real UI use case creates/discards gradients at shape-churn rates). All gradient points/positions are in the *same local, untransformed space* the shape's own geometry is defined in -- a gradient moves and rotates rigidly with its shape. `GradientError` is deliberately **not** `EngineError`: it's caller-input validation checked entirely on the CPU before any GPU call, the same category `tre_svg::SvgError` occupies.

## Shape primitives

Every shape embeds `common: PrimitiveCommon` by value (not inherited) and has a `new(...)` constructor covering the common case, with every other field defaulting "off."

```rust
pub struct CornerRadii { pub top_left: f32, pub top_right: f32, pub bottom_right: f32, pub bottom_left: f32 }
impl CornerRadii {
    pub const fn uniform(radius: f32) -> Self;
    pub fn is_uniform(&self) -> bool;
}

pub struct Rectangle {
    pub common: PrimitiveCommon, pub size: Vec2, pub fill: FillStyle,
    pub border_color: Color, pub border_thickness: f32, pub border_enabled: bool,
    pub corner_radius: CornerRadii,
    pub corner_smoothing: f32,  // 0.0 = circular-arc rounding, 1.0 = full squircle
}
impl Rectangle { pub fn new(size: Vec2, color: Color) -> Self; }

pub struct Circle {
    pub common: PrimitiveCommon, pub radius: Vec2, pub fill: FillStyle,
    pub border_color: Color, pub border_thickness: f32, pub border_enabled: bool,
    pub arc_length: f32,  // degrees, 0.0..=360.0, partial sweep from 12 o'clock, clockwise
}
impl Circle { pub fn new(radius: Vec2, color: Color) -> Self; }
// radius[0] == radius[1] is a circle, otherwise an ellipse -- one struct covers both.

pub struct Polygon {
    pub common: PrimitiveCommon, pub sides: u32, pub radius: f32, pub vertex_radius: f32,
    pub star_points: Option<u32>,  // None: regular N-gon; Some(k): 2k-vertex star
    pub fill: FillStyle, pub border_color: Color, pub border_thickness: f32, pub border_enabled: bool,
}
impl Polygon { pub fn new(sides: u32, radius: f32, color: Color) -> Self; }

pub enum PathCommand {
    MoveTo(Vec2), LineTo(Vec2),
    QuadraticTo { control: Vec2, to: Vec2 },
    CubicTo { control1: Vec2, control2: Vec2, to: Vec2 },
    Close,
}
pub enum LineCap { Butt, Round, Square }
pub enum LineJoin { Miter, Round, Bevel }
pub struct Path {
    pub common: PrimitiveCommon, pub commands: Vec<PathCommand>, pub fill: FillStyle,
    pub border_color: Color, pub border_thickness: f32, pub border_enabled: bool,
    pub stroke_line_cap: LineCap, pub stroke_line_join: LineJoin,
}
impl Path { pub fn new(commands: Vec<PathCommand>, color: Color) -> Self; }

pub struct Svg { pub common: PrimitiveCommon, pub positions: Vec<Vec2>, pub triangles: Vec<[u32; 3]>, pub fill_color: Color }
impl Svg { pub fn new(positions: Vec<Vec2>, triangles: Vec<[u32; 3]>, fill_color: Color) -> Self; }

pub const CUSTOM_PIPELINE_ID_BASE: u16 = 1000;
pub struct CustomShaded {
    pub common: PrimitiveCommon, pub size: Vec2, pub pipeline_id: u16,
    pub fill_color: Color, pub params: [f32; 3],
}
impl CustomShaded { pub fn new(size: Vec2, pipeline_id: u16, fill_color: Color) -> Self; }
```

Notes:

- **`Polygon`/`Circle`/`Rectangle`/`Path`** all fully render, including borders, real fill (`Path`/`Polygon` handle compound shapes with holes and self-intersection via `lyon`), and every `FillStyle` variant.
- **`Svg`** stores **already-tessellated** geometry (real fill-rule resolution -- nonzero or even-odd -- already happened once, at construction, not at every flatten), since SVG content is typically static across many frames. `tre-engine` deliberately does **not** depend on `tre-svg` (which itself depends on `tre-engine` for `UiVertex` -- the reverse would be a real dependency cycle); real SVG parsing/tessellation happens in `tre-python` instead, which has no such cycle. Solid fill only.
- **`CustomShaded`** carries no shader source itself -- only a `pipeline_id` referencing a pipeline the caller must already have registered via `PipelineRegistry` (see [RHI Trait & Backends](rhi-and-backends.md)). `CUSTOM_PIPELINE_ID_BASE = 1000` sits comfortably above `PipelineKind`'s 8 fixed ids so custom pipelines can never collide with a built-in one.
- Every non-uniform-radius/border/smoothing/fill field beyond the trivial case (`Rectangle`/`Circle`) is backed by a `gpu_style` GPU-side style record -- see [Canvas & Intermediate Representation](canvas-and-ir.md#gpu-side-style-records-gpu_style).

### `Text` -- the retained-mode text shape

```rust
pub struct FontId(pub u32);
pub enum FontError { InvalidFont }

pub struct FontRegistry { /* private */ }
impl FontRegistry {
    pub fn new() -> Self;
    pub fn load_bytes(&mut self, bytes: Vec<u8>) -> Result<FontId, FontError>;
    pub fn font_ref(&self, id: FontId) -> Option<skrifa::FontRef<'_>>;
    pub fn face(&self, id: FontId) -> Option<rustybuzz::Face<'_>>;
}

pub struct TextFlattenContext<'a> { pub fonts: &'a FontRegistry, pub atlas: &'a GlyphAtlasContext<'a> }

pub struct Text {
    pub common: PrimitiveCommon,
    pub text: String,
    pub font: FontId,
    pub px_size: f32,
    pub fill_color: Color,
    pub wrap_width: Option<f32>,  // None = single-line (pre-Phase-15 path, byte-identical)
}
impl Text { pub fn new(text: impl Into<String>, font: FontId, px_size: f32, color: Color) -> Self; }
```

`load_bytes` validates the font against **both** real consumers (`skrifa` for outline/metrics, `rustybuzz` for shaping) before storing it, so a caller gets a real `Err` immediately rather than a panic deep inside a future flatten call. `font_ref`/`face` return fresh, cheap views over already-validated bytes (a real table-directory parse, not a full font parse -- safe every frame). `Text::common.transform.position` is the text's own top-left corner, not its baseline; the flattening logic offsets the real pen position by the font's own scaled ascent internally. `wrap_width: Some(width)` enables real multi-line rendering (`\n` always breaks a line; words greedily wrap to fit `width` -- pass `f32::INFINITY` for hard-wrap-only); `None` keeps the exact single-line path.

## `ShapePrimitive` and `ShapeRegistry`

```rust
pub enum ShapePrimitive { Rectangle(Rectangle), Circle(Circle), Polygon(Polygon), Path(Path), Text(Text), Svg(Svg), CustomShaded(CustomShaded) }

pub struct ShapeId { /* private: index, generation */ }
pub struct AnimationId(pub u64);
pub struct ShapeSlot {
    pub shape: ShapePrimitive,
    pub active_animations: Vec<AnimationId>,
    pub layout_dirty: bool,
    pub clip_bounds: Option<ScissorRect>,
}

pub struct ShapeRegistry { /* private: a hand-built generational slot arena */ }
```

`ShapePrimitive` is plain `enum` dispatch, not `Box<dyn Primitive>` -- matching the workspace's "no dynamic type inspection in hot paths" rule. `ShapeId` carries a `generation` counter so a stale handle (its slot was removed and reused) is detectable rather than silently resolving to a different, unrelated shape.

```rust
impl ShapeRegistry {
    pub fn new() -> Self;
    pub fn insert(&mut self, shape: ShapePrimitive) -> ShapeId;
    pub fn remove(&mut self, id: ShapeId) -> bool;          // false (not panic) on a stale id
    pub fn get(&self, id: ShapeId) -> Option<&ShapeSlot>;
    pub fn get_mut(&mut self, id: ShapeId) -> Option<&mut ShapeSlot>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;

    pub fn create_gradient(&mut self, def: GradientDef) -> Result<GradientId, GradientError>;
    pub fn gradient(&self, id: GradientId) -> Option<&GradientDef>;
    pub fn gradient_mut(&mut self, id: GradientId) -> Option<&mut GradientDef>;

    pub fn mark_all_dirty(&mut self);
    pub fn flatten_into(&mut self, canvas: &mut RenderingCanvas, device: &dyn RhiDevice,
        text_context: Option<&TextFlattenContext<'_>>);
    pub fn hit_test(&self, point: Vec2) -> Option<ShapeId>;
}
```

- **`insert`/`remove`** reuse a freed slot's backing storage via a free list rather than always growing (the same "grow once, reuse after" discipline `FrameArena` uses). `remove` returns `false` on a stale id, matching the engine's "report, don't panic" convention for caller-facing errors.
- **`flatten_into`** is the per-frame flattening pass: for every live slot that is `layout_dirty` or has active animations, it resolves the shape's transform and records it into `canvas` via the same immediate-mode calls any other caller would use, then clears `layout_dirty`. A hidden/collapsed shape is skipped (but still has `layout_dirty` cleared, so it doesn't attempt to re-flatten every frame while invisible). `device` is needed because `Rectangle`/`Circle`'s richer rendering paths write a style record into the device's live shape-style buffer at flatten time. `text_context` is `None` for a registry that never inserts a `Text` shape (every pre-existing Rust example passes `None`); a registry that does insert `Text` must pass `Some`, or `flatten_into` panics. Also panics on a `FillStyle::Gradient` naming a `GradientId` this registry never issued -- a real programmer error, not a normal runtime condition.
- **`mark_all_dirty`** forces every live slot's `layout_dirty` back to `true`, so the next `flatten_into` re-records the entire registry regardless of what was already flattened. This exists specifically for `tre-python`'s `HeadlessRenderer::render`: a Python caller re-rendering an unchanged registry has no dirty-flag protocol visible to it, so "flatten everything now" is the only sound semantics for every `render()` call. Fixed a real bug (a second `render()` on an unmutated registry used to produce an empty frame, since every shape was already non-dirty, which then crashed on the resulting zero-length vertex buffer).
- **`hit_test`** returns the topmost (highest slot index -- later `insert` calls paint over earlier ones) shape whose real geometry contains `point`, in the same world space `flatten_into`'s output uses. Skips non-`hit_testable` and hidden/collapsed shapes, and any shape whose transform has collapsed to non-invertible (zero scale on some axis -- correctly "no point can hit it," not a panic). Uses exact analytic tests per shape kind: the same rounded-box SDF formula the GPU shader evaluates for `Rectangle`, an exact point-in-ellipse test for `Circle` (not the shader's scaled-circle SDF *approximation*), and the standard even-odd ray-casting test (PNPOLY) for `Polygon`/`Path`. `Text`/`Svg` are not standalone-hit-testable (real text/icons are normally hit-tested via their containing control's own background shape) -- disclosed future work, not silently approximated.

### `flatten_path`

```rust
pub fn flatten_path(commands: &[PathCommand]) -> Vec<Vec<Vec2>>
```

Flattens a `Path`'s curve commands (quadratic/cubic Béziers) into polylines -- one `Vec<Vec2>` per subpath (split on `PathCommand::Close`/`MoveTo`). Used by `hit_test`'s own `Path` case even though `flatten_into`'s rendering path for `Path` uses `lyon` directly -- hit-testing only needs CPU-side geometry, not the GPU tessellation plumbing.
