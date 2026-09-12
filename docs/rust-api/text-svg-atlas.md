# Text, SVG & Atlas

Three crates that supply content the Canvas API renders but doesn't itself parse, shape, or pack: `tre-text` (typography), `tre-svg` (vector art), and `tre-atlas` (the shared dynamic texture atlas both glyphs and icons live in). All three are pure-Rust, all three `#![forbid(unsafe_code)]`, and none of them depend on `tre-engine`'s rendering loop directly.

## `tre-atlas`

2D Guillotine bin-packing plus the real multi-window atlas concurrency model built on top of it. Shared by MSDF glyph entries and plain-color icon/vector-decal entries alike -- deliberately content-agnostic (it has no idea a glyph key and an icon key are packed differently), which is why it's its own crate rather than living inside `tre-text`. No `unsafe` code lives here at all -- the concurrency primitives that need it (`MpscRingBuffer`, `SwmrSlotTable`) live in `tre-memory` (see [Math & Memory](math-and-memory.md)).

```rust
pub struct PackedRect { pub x: u32, pub y: u32, pub width: u32, pub height: u32 }
impl PackedRect { pub fn overlaps(&self, other: &PackedRect) -> bool; }

pub struct AtlasPacker { /* private */ }
impl AtlasPacker {
    pub fn new(width: u32, height: u32) -> Self;
    pub fn insert(&mut self, width: u32, height: u32) -> Option<PackedRect>;  // None = doesn't fit, never panics
    pub fn remove(&mut self, rect: PackedRect);  // rect must be one this packer actually returned
    pub fn used_fraction(&self) -> f64;
}

pub struct AtlasKey(/* private u64 */);
impl AtlasKey { pub fn from_glyph(font_id: u32, glyph_id: u32) -> Self; }
// impl From<AtlasKey> for u64, impl From<u64> for AtlasKey

pub fn pack_slot_value(rect: PackedRect, generation: u16) -> u64;   // panics if any field overflows its bits
pub fn unpack_slot_value(value: u64) -> (PackedRect, u16);
```

`AtlasPacker` uses **Best-Area-Fit selection** with a real Guillotine split -- when a placement leaves an "L-shaped" leftover region, the split always partitions it into exactly two non-overlapping rectangles (never the overlapping-candidate set a MaxRects packer would keep), always choosing whichever cut leaves the single larger resulting piece (not approximated from raw leftover dimensions alone -- the smaller *leftover width* doesn't always produce the larger resulting piece). `remove` does no coalescing against adjacent free rectangles -- real fragmentation-reduction is deferred until a concrete problem shows up in practice, not built speculatively.

### The atlas owner: real multi-thread concurrency

```rust
pub trait RasterSource: Send {
    fn size(&self) -> (u32, u32);
    fn rasterize(&self) -> Vec<u8>;   // RGBA8, width * height * 4 bytes, row-major
}

pub struct AtlasInsertRequest {
    pub key: AtlasKey,
    pub raster_source: Box<dyn RasterSource>,
    pub current_frame: u64,
}

pub struct AtlasOwner { /* private */ }
impl AtlasOwner {
    pub fn spawn(atlas_width: u32, atlas_height: u32, request_capacity: usize, slot_capacity: usize) -> Self;
    pub fn handle(&self) -> AtlasOwnerHandle;
    pub fn join(self) -> Vec<u8>;   // stops the background thread for good, returns the finished buffer
}

#[derive(Clone)]
pub struct AtlasOwnerHandle { /* private */ }
impl AtlasOwnerHandle {
    pub fn request_insert(&self, key: AtlasKey, raster_source: Box<dyn RasterSource>, current_frame: u64) -> bool;
    pub fn lookup(&self, key: AtlasKey, current_frame: u64) -> Option<(PackedRect, u16)>;
    pub fn snapshot(&self) -> Vec<u8>;   // full-buffer clone -- see generation() for a cheaper check
    pub fn generation(&self) -> u64;     // bumped once per real pixel-writing insertion; one atomic load
}
```

`AtlasOwner::spawn` starts a **dedicated background thread** -- the only code in the whole engine ever allowed to touch `AtlasPacker`'s free-rectangle list -- draining `AtlasInsertRequest`s via a `tre_memory::MpscRingBuffer` from any number of producer threads, and publishing results into a `tre_memory::SwmrSlotTable` any reader can consult lock-free.

- **`request_insert`** never blocks: returns `false` if the bounded request queue is full, or if `join` has already been called on any clone of this handle's owner (without that second check, a request racing shutdown could be enqueued yet never processed, silently leaving `lookup` returning `None` forever with no way to tell that apart from "still pending").
- **`lookup`** returning `None` is deliberately indistinguishable across three real cases: never requested, requested-but-not-yet-processed, or evicted -- a caller's placeholder-glyph fallback responds to all three identically (render a placeholder this frame, re-check later).
- **`current_frame`** on both methods is the only way the owner's background thread -- which otherwise has no concept of frames -- learns what frame number to weigh its own LRU eviction check against, and (via `lookup`) records recency so a glyph rendered every frame never goes stale enough to evict.
- **`snapshot`** locks and clones the *entire* shared pixel buffer every call (the owner's own writes are scattered small per-glyph copies with no dirty-region tracking) -- use `generation()`'s cheap atomic-load check first to avoid calling it when nothing changed.
- Eviction policy (a private implementation detail, `maybe_evict_stale_entries`): triggers once atlas occupancy crosses **85%** used, evicting entries idle for at least **600 frames** -- directly implementing DESIGN.md's own documented policy language.

!!! note "A disclosed, unfixed performance limitation"
    `maybe_evict_stale_entries` runs unconditionally at the top of every `process_insert` call, and once occupancy is at or above the 85% threshold, `SwmrSlotTable::scan_older_than`'s full `O(capacity)` linear scan repeats on **every subsequent insert** while occupancy stays elevated -- not just once per threshold-crossing. Documented as a known limitation, not yet fixed.

## `tre-svg`

SVG ingestion, tessellation, and morphing. Parses real SVG documents via [`usvg`](https://docs.rs/usvg) (resolves the DOM -- `<use>`/`<g>`/CSS -- into absolute-coordinate path data, performs no rasterization itself), flattens curves, then tessellates via [`lyon`](https://github.com/nical/lyon) -- "the industry-standard Rust 2D tessellation library." A prior hand-rolled ear-clipping triangulator (and the GPU-side stencil-and-cover fallback it needed for self-intersecting input) has been fully retired: lyon's real sweep-line fill tessellator handles self-intersection, compound shapes with holes, and both winding rules directly, as one algorithm.

```rust
pub struct Polygon { pub points: Vec<[f32; 2]> }  // one closed contour, last point implicitly closes to first

pub enum SvgError {
    TooLarge { size: usize, max: usize },
    TooManyPoints { count: usize, max: usize },
    Parse(String),
    TessellationFailed,
    TopologyMismatch { from_points: usize, to_points: usize },
    MalformedXml(String),
}

pub fn parse_svg(source: &[u8], max_bytes: usize, max_points: usize) -> Result<Vec<Polygon>, SvgError>;
```

`parse_svg` checks `max_bytes` before `usvg` ever sees the data, and `max_points` (the *total* point count across every path, checked incrementally while walking the tree, not only after fully resolving a pathological document) since `usvg` itself enforces no such cap -- a depth/element-count-bounded document can still resolve to an unbounded number of points. `usvg` brings its own hardening too: a 1024-deep nesting/`<use>`-chain cap, a 1,000,000-element cap, and `<use>` reference cycle detection. `max_points` bounds peak memory but **not** worst-case CPU time -- a single adversarially-shaped path within budget can still be far more expensive to tessellate than a well-behaved one of the same point count. The parser is fuzz-tested (`proptest`, not `cargo-fuzz`) against malformed and adversarial documents, asserting bounded time/memory regardless of input.

```rust
pub fn flatten_cubic(p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], p3: [f32; 2], out: &mut Vec<[f32; 2]>);
pub fn flatten_quad(p0: [f32; 2], control: [f32; 2], p1: [f32; 2], out: &mut Vec<[f32; 2]>);
```

Cubic/quadratic Bezier flattening into polylines, backed by `lyon_geom`'s tolerance-based curve flattening. `pub`, not `pub(crate)`, specifically so `tre-text` can reuse the exact same functions for glyph outline geometry (a font glyph's outline is cubic/quadratic Béziers too) -- the contract (excludes the start point, includes the end point) matches `lyon_geom::Flattened`'s own documented behavior exactly.

```rust
pub fn morph(from: &Polygon, to: &Polygon, t: f32) -> Result<Polygon, SvgError>;
pub fn morph_into(from: &Polygon, to: &Polygon, t: f32, out: &mut Vec<[f32; 2]>) -> Result<(), SvgError>;
```

SIMD keyframe interpolation (via `tre_math::lerp_points_batch`) between two already-flattened polygons at parameter `t`. "Topological equivalence" means equal vertex counts -- a mismatch is a real `Err`, never auto-resampled. `morph` allocates a fresh `Vec` every call (fine for one-shot use); `morph_into` reuses a caller-supplied buffer and is the zero-allocation path a real per-frame animation loop should use.

```rust
pub enum FillRule { NonZero, EvenOdd }
pub fn tessellate_fill(contours: &[Polygon], fill_rule: FillRule, rgba: u32)
    -> Result<(Vec<tre_engine::UiVertex>, Vec<u32>), SvgError>;
```

Tessellates one or more already-flattened contours (together forming one compound fill region) directly into flat-colored vertex/index buffers -- one real tessellation pass, not "triangulate, then separately convert": lyon's tessellator can introduce genuinely new vertices at a real self-intersection it resolves, so a result shaped as "indices into the caller's own point array" isn't something a general tessellator can honestly promise.

### Real SMIL animation parsing

```rust
pub struct SmilAnimate { pub attribute_name: String, pub keyframes: Vec<f32>, pub duration_seconds: f32 }
pub struct SmilAnimateTranslate { pub keyframes: Vec<[f32; 2]>, pub duration_seconds: f32 }
pub struct ParsedSmil { pub animates: Vec<SmilAnimate>, pub animate_translates: Vec<SmilAnimateTranslate> }

pub fn parse_smil(svg_source: &str) -> Result<ParsedSmil, SvgError>;
```

`usvg` is a static-resolution parser by design (confirmed by reading its own source) -- it does not process SMIL `<animate>`/`<animateTransform>` elements at all, so real SMIL support needs a separate direct XML pass via `roxmltree`. **Real, disclosed v1 scope**: `<animate>` (a single scalar attribute) and `<animateTransform type="translate">` only, each with `values="a;b;c"` or `from`/`to`, plus `dur`. `begin`, `repeatCount`, `calcMode`, `type="scale"/"rotate"`, and `<animateMotion>` are disclosed gaps, not attempted. A malformed individual element is silently skipped rather than rejecting the whole document. This module only *extracts* keyframes -- a caller drives them through `tre-animation`'s own `Timeline`/`Tween` (see [Animation](animation.md)).

## `tre-text`

Dynamic typography: bidi + script run segmentation and shaping via [`rustybuzz`](https://docs.rs/rustybuzz) (a HarfBuzz binding), glyph outline extraction via [`skrifa`](https://docs.rs/skrifa), Linux-only `fontconfig`-driven font fallback, and MSDF rasterization via [`fdsm`](https://docs.rs/fdsm) -- a real pure-Rust reimplementation of `msdfgen`'s own published algorithm. A deliberate all-pure-Rust font stack, so this workspace's only C ABI boundary stays Vulkan.

### Shaping

```rust
pub struct ShapedGlyph {
    pub glyph_id: u32,
    pub cluster: u32,     // byte offset into the original string this glyph's grapheme cluster starts at
    pub x_advance: i32, pub y_advance: i32,  // font design units, not pixels
    pub x_offset: i32, pub y_offset: i32,
}
pub struct ShapedRun { pub text_range: Range<usize>, pub direction: rustybuzz::Direction, pub glyphs: Vec<ShapedGlyph> }
pub struct TextRun { pub text_range: Range<usize>, pub level: unicode_bidi::Level, pub script: unicode_script::Script }

pub fn segment_runs(text: &str) -> Vec<TextRun>;
pub fn shape_text(face: &Face, text: &str) -> Result<Vec<ShapedRun>, TextError>;
```

`segment_runs` splits `text` into bidi- and script-uniform runs in **visual** order: `unicode_bidi::BidiInfo::visual_runs` already reorders same-level runs within a paragraph, and this function additionally splits each level-run further wherever the resolved script changes (a single bidi level can still span more than one script, e.g. an RTL paragraph mixing Arabic and Hebrew). `shape_text` shapes each run and returns them already in correct left-to-right concatenation order for rendering, including a mixed LTR/RTL paragraph.

### Errors

```rust
pub enum TextError {
    InvalidFontForShaping,
    InvalidFontForOutlines,
    OutlineDrawFailed,
    FontDiscoveryUnavailable,
}
```

### Outline extraction & MSDF generation

```rust
pub enum OutlineSegment {
    MoveTo([f32; 2]), LineTo([f32; 2]),
    QuadTo { control: [f32; 2], end: [f32; 2] },
    CubicTo { control1: [f32; 2], control2: [f32; 2], end: [f32; 2] },
    Close,
}
pub type Contour = Vec<OutlineSegment>;
pub fn glyph_outline(font: &FontRef, glyph_id: GlyphId) -> Result<Vec<Contour>, TextError>;
```

The pure-Rust replacement for `FT_Outline_Decompose`, returning raw unscaled (font design-unit) contours -- deliberately mirroring `tre-svg`'s own `OutlineSegment`/path-segment shape, so a future step feeding these into `tre-svg::flatten_cubic`/`flatten_quad` needs no translation layer.

```rust
pub struct MsdfBitmap { pub width: u32, pub height: u32, pub pixels: Vec<u8> }  // RGB8, row-major
pub fn generate_msdf(contours: &[Contour], size: u32, range_px: f64) -> Option<MsdfBitmap>;
pub fn has_real_ink(contours: &[Contour]) -> bool;
```

"MSDF" = Multi-channel Signed Distance Field. The glyph is fit into the target box with a single **uniform** scale (never anisotropic -- that would distort corners and defeat MSDF's whole "preserve sharp corners" purpose), and the Y axis is flipped (font design space is Y-up; this crate's pixel output is Y-down like every other raster image in the project).

!!! warning "Two real, found-and-fixed panics"
    `generate_msdf` returns `None` (not a panic) when `contours` has no real ink -- either genuinely empty (e.g. U+0020 SPACE legitimately has zero contours) or every point is coincident/non-finite. It used to `panic!()` via an `.expect()` on the premise "a real glyph always has real extent" -- false for space, and false for malformed/corrupted font data (a realistic threat given this crate's attacker-influenceable-font-bytes threat model). **`has_real_ink`** exists because of a related, separately-found bug (REVIEW.md finding #139): `Canvas::draw_text` used to gate rasterization on the narrower, wrong condition `!contours.is_empty()` -- a non-empty-but-degenerate contour (e.g. a single `MoveTo`/`Close` pair, which an ordinary font's ordinary single-point contour can produce) still fails `generate_msdf`'s real degeneracy check, so calling code that only checked emptiness could construct a raster request that panics on the atlas owner's own shared background thread. `has_real_ink` exposes the exact correct check.

### Word boundaries, wrapping, and caret placement

```rust
pub fn next_word_end(text: &str, from: usize) -> Option<usize>;
pub fn prev_word_start(text: &str, from: usize) -> Option<usize>;
```

Pure string functions over **UAX #29** ("Unicode Text Segmentation", via `unicode-segmentation`) -- no font/shaping dependency at all, since word boundaries are a property of the text itself. Uses `unicode_word_indices` (not `split_word_bound_indices`) deliberately: it already filters out pure whitespace/punctuation segments, landing exactly where a Ctrl+arrow jump should.

```rust
pub struct WrappedLine { pub byte_range: Range<usize>, pub start_glyph: usize, pub end_glyph: usize, pub width: f32 }
pub fn wrap_lines(text: &str, runs: &[ShapedRun], px_size: f32, units_per_em: u16, max_width: Option<f32>) -> Vec<WrappedLine>;
```

Real line-breaking: `\n` always hard-breaks; break *opportunities* within a segment come from [`unicode-linebreak`](https://docs.rs/unicode-linebreak)'s real implementation of **[UAX #14](https://www.unicode.org/reports/tr14/)** -- covering real hyphen/punctuation/CJK-ideograph break points, not just whitespace. `max_width: None` degrades cleanly to hard-wrap-only. **Real, disclosed v1 scope**: LTR/single-script text only, a single unbreakable token wider than `max_width` still gets its own line (no hyphenation), left-aligned only. Trailing whitespace immediately before any break is consumed into the closing line as an unrendered tail (covered by that line's `byte_range` so every byte still maps to exactly one line, but contributing no width/glyphs) -- matching how a real word processor renders a wrap point, not a byte-loss bug.

```rust
pub struct CaretPosition { pub byte_offset: usize, pub x: f32 }
pub fn caret_positions(runs: &[ShapedRun], text_len: usize, px_size: f32, units_per_em: u16) -> Vec<CaretPosition>;
pub fn hit_test(positions: &[CaretPosition], x: f32) -> usize;  // panics if positions is empty

pub struct MultiLineCaretPosition { pub byte_offset: usize, pub line: usize, pub x: f32 }
pub fn multiline_caret_positions(lines: &[WrappedLine], runs: &[ShapedRun], px_size: f32, units_per_em: u16) -> Vec<MultiLineCaretPosition>;
pub fn hit_test_2d(positions: &[MultiLineCaretPosition], line_height: f32, x: f32, y: f32) -> usize;  // panics if empty
pub fn line_of(positions: &[MultiLineCaretPosition], byte_offset: usize) -> usize;  // panics if empty
```

`caret_positions` mirrors `tre_engine::text::flatten_text`'s own pen-advance formula exactly (confirmed by reading that function's source), so a caret computed here lands exactly where the matching glyph actually renders. A byte offset sitting exactly at a line boundary gets two valid stops (end of the earlier line, start of the next) -- `line_of` always prefers the earlier line, the same ambiguity every real text editor resolves with caret "affinity" (not attempted further here).

!!! note "A real, found bug in hit_test_2d"
    An earlier version located the containing line via `(y / line_height).round()`, which finds the line whose *top* is nearest rather than the line whose *span* actually contains `y` -- silently misattributing roughly the bottom half of every line's own vertical extent to the line below it. Fixed to `.floor()` (clamped to a valid index).

### Font fallback (Linux only)

```rust
pub struct FontCascade { pub entries: Vec<PathBuf> }
impl FontCascade { pub fn discover() -> Result<Self, TextError>; }

pub fn covers(font_bytes: &[u8], text: &str) -> Result<bool, TextError>;
pub fn resolve_font_index(font_candidates: &[&[u8]], run_text: &str) -> Result<usize, TextError>;
pub fn resolve_run(font_candidates: &[&[u8]], text: &str, run: &TextRun) -> Result<(usize, ShapedRun), TextError>;
```

Only compiled on `target_os = "linux"` -- Windows (DirectWrite)/macOS (Core Text) system font discovery is disclosed, deferred future work. `discover()` queries `fontconfig` for `["sans-serif", "Noto Sans", "emoji"]` in priority order (primary UI sans, broad-coverage fallback, color emoji), deduping by canonicalized path since two queries can resolve to the same underlying font file via two different filesystem paths. `resolve_font_index` falls back to index `0` if no candidate fully covers the run text -- a run degrading to `.notdef` tofu glyphs in the primary font is still a defined, visible outcome, never a panic or a silently dropped run.

### `GlyphRasterSource` -- the `tre_atlas::RasterSource` glue

```rust
pub struct GlyphRasterSource { pub contours: Vec<Contour>, pub size: u32, pub range_px: f64 }
// impl RasterSource: size() -> (u32, u32); rasterize() -> Vec<u8>  (panics if contours has no real ink)
```

Deliberately lives in `tre-text`, not `tre-atlas` -- `tre-atlas` stays content-agnostic, so the content-aware glue (turning glyph contours into RGBA8 pixels) belongs on this side of the `RasterSource` trait-object boundary. Callers must only construct this for a glyph already confirmed to have real ink (via `has_real_ink`) -- see the MSDF warning above for why.
