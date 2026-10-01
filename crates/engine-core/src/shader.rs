//! 0.5.1 (#54, #66): a shader on a node -- the app's WGSL, checked on the CPU.
//!
//! The app writes one function, `fn shade(p: Pixel) -> vec4<f32>`. tre writes
//! the rest (the design is `docs/design/wgsl.md`): a vertex stage, `Pixel`, a
//! `frame` block, the app's uniforms as a generated struct `u`, `content(uv)`
//! for an effect, and `input_<name>(uv)` per named input. [`Shader`] holds the
//! app's source, assembles it after that prelude, and validates the result
//! with `naga`, so a mistake is reported when the shader is created, with
//! the line and column in the app's *own* source (the prelude's lines are
//! subtracted), and no GPU is needed.
//!
//! Nothing here draws. The renderer (a later milestone) takes
//! [`Shader::assembled`] for its pipeline, [`Shader::uniform_bytes`] for the
//! uniform buffer, and [`frame_block`] for the `frame` buffer.
//!
//! **Colour convention.** `shade` returns straight-alpha RGBA, like an
//! `image`'s pixels, and an effect's `content(uv)` returns the node's rendered
//! pixels the same way; tre converts at the edges.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use naga::valid::{Capabilities, ValidationFlags, Validator};

use crate::NodeId;

/// How a shader applies to its node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShaderMode {
    /// Paints the node's box, behind its content: a procedural fill.
    Fill,
    /// Transforms the node's own rendered content, which `content(uv)` reads.
    Effect,
}

impl ShaderMode {
    /// The name Python uses.
    pub fn name(self) -> &'static str {
        match self {
            Self::Fill => "fill",
            Self::Effect => "effect",
        }
    }
}

/// The type of a uniform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UniformKind {
    F32,
    Vec2,
    Vec3,
    Vec4,
}

impl UniformKind {
    fn wgsl(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::Vec2 => "vec2<f32>",
            Self::Vec3 => "vec3<f32>",
            Self::Vec4 => "vec4<f32>",
        }
    }

    /// WGSL's alignment and size of the type in a uniform buffer.
    fn align_and_size(self) -> (usize, usize) {
        match self {
            Self::F32 => (4, 4),
            Self::Vec2 => (8, 8),
            Self::Vec3 => (16, 12),
            Self::Vec4 => (16, 16),
        }
    }
}

/// A uniform's value: a number is an `f32`, and two to four numbers a vector.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UniformValue {
    F32(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
}

impl UniformValue {
    pub fn kind(&self) -> UniformKind {
        match self {
            Self::F32(_) => UniformKind::F32,
            Self::Vec2(_) => UniformKind::Vec2,
            Self::Vec3(_) => UniformKind::Vec3,
            Self::Vec4(_) => UniformKind::Vec4,
        }
    }

    fn components(&self) -> &[f32] {
        match self {
            Self::F32(v) => std::slice::from_ref(v),
            Self::Vec2(v) => v,
            Self::Vec3(v) => v,
            Self::Vec4(v) => v,
        }
    }
}

/// A problem with a shader, positioned in the app's own source when it has a
/// position.
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderError {
    pub message: String,
    /// 1-based, in the app's source.
    pub line: Option<u32>,
    /// 1-based, in bytes.
    pub column: Option<u32>,
    /// The app's source line the error is on.
    pub source_line: Option<String>,
}

impl ShaderError {
    fn plain(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            line: None,
            column: None,
            source_line: None,
        }
    }
}

impl fmt::Display for ShaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        if let (Some(line), Some(column)) = (self.line, self.column) {
            write!(f, " (line {line}, column {column})")?;
            if let Some(source) = &self.source_line {
                let caret = " ".repeat(column.saturating_sub(1) as usize);
                write!(f, "\n    {source}\n    {caret}^")?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for ShaderError {}

/// The size of the `frame` uniform block in bytes: `size: vec2<f32>` at 0,
/// `time: f32` at 8, and padding to 16.
pub const FRAME_BLOCK_SIZE: usize = 16;

/// The bytes of the `frame` block for a node of `size` pixels at `time` seconds.
pub fn frame_block(size: (f32, f32), time: f32) -> [u8; FRAME_BLOCK_SIZE] {
    let mut out = [0u8; FRAME_BLOCK_SIZE];
    out[0..4].copy_from_slice(&size.0.to_le_bytes());
    out[4..8].copy_from_slice(&size.1.to_le_bytes());
    out[8..12].copy_from_slice(&time.to_le_bytes());
    out
}

/// Names tre's prelude declares at module scope: an app declaration with one
/// of these is a clash.
const PRELUDE_NAMES: &[&str] = &[
    "Pixel",
    "Frame",
    "Uniforms",
    "VertexOutput",
    "frame",
    "u",
    "content",
    "content_texture",
    "content_sampler",
    "tre_vertex",
    "tre_fragment",
];

/// Whether `name` can name a uniform or an input: a WGSL identifier that is
/// not reserved (WGSL keeps `_` alone, anything starting with `__`, and a
/// list of keywords for itself).
fn identifier_problem(name: &str) -> Option<&'static str> {
    let mut chars = name.chars();
    let first_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    if !first_ok || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Some(
            "isn't a WGSL identifier (letters, digits and underscores, not starting with a digit)",
        );
    }
    if name == "_" || name.starts_with("__") {
        return Some("is reserved by WGSL (`_` alone, or a name starting with `__`)");
    }
    if naga::keywords::wgsl::RESERVED.contains(&name) {
        return Some("is a WGSL keyword or reserved word");
    }
    None
}

fn check_name(what: &str, name: &str) -> Result<(), ShaderError> {
    match identifier_problem(name) {
        Some(why) => Err(ShaderError::plain(format!("{what} name `{name}` {why}"))),
        None => Ok(()),
    }
}

/// The generated WGSL that comes before the app's source. It ends with a
/// newline, so the app's first line is the line after the prelude's last.
fn prelude(mode: ShaderMode, uniforms: &[(String, UniformKind)], inputs: &[String]) -> String {
    let mut s = String::new();
    s.push_str("// ---- tre's generated prelude: the app's source follows ----\n");
    s.push_str("struct Pixel {\n    uv: vec2<f32>,\n    px: vec2<f32>,\n}\n");
    s.push_str("struct Frame {\n    size: vec2<f32>,\n    time: f32,\n    pad: f32,\n}\n");
    s.push_str("struct Uniforms {\n");
    if uniforms.is_empty() {
        // A WGSL struct needs a member.
        s.push_str("    unused: vec4<f32>,\n");
    }
    for (name, kind) in uniforms {
        s.push_str(&format!("    {name}: {},\n", kind.wgsl()));
    }
    s.push_str("}\n");
    s.push_str("@group(0) @binding(0) var<uniform> frame: Frame;\n");
    s.push_str("@group(0) @binding(1) var<uniform> u: Uniforms;\n");
    if mode == ShaderMode::Effect {
        s.push_str("@group(0) @binding(2) var content_texture: texture_2d<f32>;\n");
        s.push_str("@group(0) @binding(3) var content_sampler: sampler;\n");
        s.push_str(
            "fn content(uv: vec2<f32>) -> vec4<f32> {\n    \
             return textureSampleLevel(content_texture, content_sampler, uv, 0.0);\n}\n",
        );
    }
    for (i, name) in inputs.iter().enumerate() {
        let (texture, sampler) = (4 + 2 * i, 5 + 2 * i);
        s.push_str(&format!(
            "@group(0) @binding({texture}) var input_{name}_texture: texture_2d<f32>;\n\
             @group(0) @binding({sampler}) var input_{name}_sampler: sampler;\n\
             fn input_{name}(uv: vec2<f32>) -> vec4<f32> {{\n    \
             return textureSampleLevel(input_{name}_texture, input_{name}_sampler, uv, 0.0);\n}}\n"
        ));
    }
    s.push_str(
        "struct VertexOutput {\n    @builtin(position) position: vec4<f32>,\n    @location(0) uv: vec2<f32>,\n}\n",
    );
    s.push_str(
        "@vertex\nfn tre_vertex(@builtin(vertex_index) index: u32) -> VertexOutput {\n    \
         var corners = array<vec2<f32>, 3>(\n        \
         vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));\n    \
         let p = corners[index];\n    \
         var out: VertexOutput;\n    \
         out.position = vec4<f32>(p, 0.0, 1.0);\n    \
         out.uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);\n    \
         return out;\n}\n",
    );
    s.push_str(
        "@fragment\nfn tre_fragment(in: VertexOutput) -> @location(0) vec4<f32> {\n    \
         return shade(Pixel(in.uv, in.uv * frame.size));\n}\n",
    );
    s.push_str("// ---- the app's source ----\n");
    s
}

/// The assembled module: the prelude, then the app's source.
struct Assembled {
    source: String,
    prelude_lines: u32,
}

/// A `naga` error's text: the error and what caused it, joined.
fn error_chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut next = error.source();
    while let Some(cause) = next {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        next = cause.source();
    }
    text
}

/// A positioned error: `location` is in the assembled source, and is mapped
/// back to the app's own.
fn positioned(
    message: String,
    location: Option<naga::SourceLocation>,
    wgsl: &str,
    prelude_lines: u32,
) -> ShaderError {
    let Some(location) = location else {
        return ShaderError::plain(message);
    };
    // naga gives some errors no real span (a default one at offset 0).
    if location.offset == 0 && location.length == 0 {
        return ShaderError::plain(message);
    }
    if location.line_number <= prelude_lines {
        // Only tre's own `tre_fragment` calls `shade`, so a problem there is
        // the app's `shade` missing or misshapen; anything else in the
        // prelude is a name tre generated being clashed with.
        if !wgsl.contains("shade") {
            return ShaderError::plain(SHADE_WANTED);
        }
        if message.contains("shade") || message.contains("convert") {
            return ShaderError::plain(format!(
                "`shade` has the wrong signature: {SHADE_WANTED} ({message})"
            ));
        }
        return ShaderError::plain(format!(
            "{message} (in tre's generated code: a uniform or input name probably \
             clashes with a name tre provides)"
        ));
    }
    let line = location.line_number - prelude_lines;
    ShaderError {
        message,
        line: Some(line),
        column: Some(location.line_position),
        source_line: wgsl.lines().nth(line as usize - 1).map(str::to_owned),
    }
}

/// Assembles and validates `wgsl` with this mode, these uniforms and inputs.
fn assemble(
    wgsl: &str,
    mode: ShaderMode,
    uniforms: &[(String, UniformKind)],
    inputs: &[String],
) -> Result<Assembled, ShaderError> {
    let head = prelude(mode, uniforms, inputs);
    let prelude_lines = head.matches('\n').count() as u32;
    let source = format!("{head}{wgsl}");
    let module = match naga::front::wgsl::parse_str(&source) {
        Ok(module) => module,
        Err(error) => {
            let mut message = error.message().to_owned();
            if message.contains("redefinition") {
                message.push_str(&format!(
                    "; tre provides these names: {}",
                    PRELUDE_NAMES.join(", ")
                ));
            }
            return Err(positioned(
                message,
                error.location(&source),
                wgsl,
                prelude_lines,
            ));
        }
    };

    // The app writes `shade`, and no entry points of its own.
    for entry in &module.entry_points {
        let name = entry.function.name.as_deref();
        if name != Some("tre_vertex") && name != Some("tre_fragment") {
            return Err(ShaderError::plain(format!(
                "a shader defines `fn shade(p: Pixel) -> vec4<f32>`, not entry points; \
                 remove `{}`",
                name.unwrap_or("?")
            )));
        }
    }
    check_shade(&module)?;

    if let Err(error) =
        Validator::new(ValidationFlags::all(), Capabilities::empty()).validate(&module)
    {
        return Err(positioned(
            error_chain(error.as_inner()),
            error.location(&source),
            wgsl,
            prelude_lines,
        ));
    }
    Ok(Assembled {
        source,
        prelude_lines,
    })
}

const SHADE_WANTED: &str = "a shader must define `fn shade(p: Pixel) -> vec4<f32>`";

/// `shade` exists, takes one `Pixel`, and returns a `vec4<f32>`.
fn check_shade(module: &naga::Module) -> Result<(), ShaderError> {
    let Some((_, shade)) = module
        .functions
        .iter()
        .find(|(_, f)| f.name.as_deref() == Some("shade"))
    else {
        return Err(ShaderError::plain(SHADE_WANTED));
    };
    let takes_a_pixel = shade.arguments.len() == 1
        && matches!(
            &module.types[shade.arguments[0].ty],
            naga::Type {
                name: Some(name),
                inner: naga::TypeInner::Struct { .. },
            } if name == "Pixel"
        );
    let returns_a_vec4 = shade.result.as_ref().is_some_and(|result| {
        matches!(
            module.types[result.ty].inner,
            naga::TypeInner::Vector {
                size: naga::VectorSize::Quad,
                scalar: naga::Scalar {
                    kind: naga::ScalarKind::Float,
                    width: 4,
                },
            }
        )
    });
    if takes_a_pixel && returns_a_vec4 {
        Ok(())
    } else {
        Err(ShaderError::plain(format!(
            "`shade` has the wrong signature: {SHADE_WANTED}"
        )))
    }
}

/// WGSL's layout of the `Uniforms` struct: each member's byte offset, and the
/// struct's size.
fn uniform_layout(uniforms: &[(String, UniformKind)]) -> (Vec<usize>, usize) {
    if uniforms.is_empty() {
        return (Vec::new(), 16); // the placeholder `vec4<f32>`
    }
    let (mut offsets, mut end, mut struct_align) = (Vec::new(), 0usize, 1usize);
    for (_, kind) in uniforms {
        let (align, size) = kind.align_and_size();
        let offset = end.next_multiple_of(align);
        offsets.push(offset);
        end = offset + size;
        struct_align = struct_align.max(align);
    }
    (offsets, end.next_multiple_of(struct_align))
}

struct State {
    uniforms: Vec<(String, UniformValue)>,
    assembled: Arc<str>,
    prelude_lines: u32,
}

impl State {
    fn kinds(&self) -> Vec<(String, UniformKind)> {
        self.uniforms
            .iter()
            .map(|(name, value)| (name.clone(), value.kind()))
            .collect()
    }
}

/// A validated shader. Shared (`Arc`) between the nodes it is set on, so
/// changing its uniforms updates all of them.
pub struct Shader {
    wgsl: String,
    mode: ShaderMode,
    animated: bool,
    inputs: Vec<(String, NodeId)>,
    state: Mutex<State>,
    /// Bumped when a uniform's value changes: the buffer needs rewriting.
    value_version: AtomicU64,
    /// Bumped when the uniforms' names or types change: the pipeline needs
    /// rebuilding.
    layout_version: AtomicU64,
}

impl Shader {
    /// Validates `wgsl` and builds the shader. A mistake in the source, a bad
    /// uniform or input name, or a missing `shade` is a [`ShaderError`].
    pub fn new(
        wgsl: String,
        uniforms: Vec<(String, UniformValue)>,
        inputs: Vec<(String, NodeId)>,
        mode: ShaderMode,
        animated: bool,
    ) -> Result<Arc<Self>, ShaderError> {
        for (name, _) in &uniforms {
            check_name("uniform", name)?;
        }
        let mut seen = std::collections::HashSet::new();
        for (name, _) in &inputs {
            check_name("input", name)?;
            if !seen.insert(name.as_str()) {
                return Err(ShaderError::plain(format!(
                    "input name `{name}` is used twice"
                )));
            }
        }
        let kinds: Vec<_> = uniforms
            .iter()
            .map(|(n, v)| (n.clone(), v.kind()))
            .collect();
        let names: Vec<_> = inputs.iter().map(|(n, _)| n.clone()).collect();
        let assembled = assemble(&wgsl, mode, &kinds, &names)?;
        Ok(Arc::new(Self {
            wgsl,
            mode,
            animated,
            inputs,
            state: Mutex::new(State {
                uniforms,
                assembled: Arc::from(assembled.source),
                prelude_lines: assembled.prelude_lines,
            }),
            value_version: AtomicU64::new(0),
            layout_version: AtomicU64::new(0),
        }))
    }

    /// Replaces the uniforms, checked first and applied all at once. A change
    /// of names or types is revalidated against the source (it might use a
    /// uniform that is gone); a change of values only is just stored.
    pub fn set_uniforms(&self, uniforms: Vec<(String, UniformValue)>) -> Result<(), ShaderError> {
        for (name, _) in &uniforms {
            check_name("uniform", name)?;
        }
        let new_kinds: Vec<_> = uniforms
            .iter()
            .map(|(n, v)| (n.clone(), v.kind()))
            .collect();
        let mut state = self.state.lock().unwrap();
        if new_kinds == state.kinds() {
            state.uniforms = uniforms;
            drop(state);
            self.value_version.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }
        let names: Vec<_> = self.inputs.iter().map(|(n, _)| n.clone()).collect();
        let assembled = assemble(&self.wgsl, self.mode, &new_kinds, &names)?;
        state.uniforms = uniforms;
        state.assembled = Arc::from(assembled.source);
        state.prelude_lines = assembled.prelude_lines;
        drop(state);
        self.layout_version.fetch_add(1, Ordering::Relaxed);
        self.value_version.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// The app's own source, as given.
    pub fn wgsl(&self) -> &str {
        &self.wgsl
    }

    pub fn mode(&self) -> ShaderMode {
        self.mode
    }

    pub fn animated(&self) -> bool {
        self.animated
    }

    /// The named inputs, in the order their bindings are numbered.
    pub fn inputs(&self) -> &[(String, NodeId)] {
        &self.inputs
    }

    /// The uniforms and their current values, in the order given.
    pub fn uniforms(&self) -> Vec<(String, UniformValue)> {
        self.state.lock().unwrap().uniforms.clone()
    }

    /// The complete WGSL module for the renderer: tre's prelude, then the
    /// app's source. Its entry points are `tre_vertex` and `tre_fragment`.
    pub fn assembled(&self) -> Arc<str> {
        self.state.lock().unwrap().assembled.clone()
    }

    /// How many lines of generated code come before the app's first line.
    pub fn prelude_lines(&self) -> u32 {
        self.state.lock().unwrap().prelude_lines
    }

    /// The `Uniforms` buffer's bytes, laid out as WGSL lays the struct out.
    pub fn uniform_bytes(&self) -> Vec<u8> {
        let state = self.state.lock().unwrap();
        let kinds = state.kinds();
        let (offsets, size) = uniform_layout(&kinds);
        let mut bytes = vec![0u8; size];
        for (offset, (_, value)) in offsets.iter().zip(&state.uniforms) {
            for (i, component) in value.components().iter().enumerate() {
                let at = offset + 4 * i;
                bytes[at..at + 4].copy_from_slice(&component.to_le_bytes());
            }
        }
        bytes
    }

    /// `(values, layout)`: what changed since the caller last looked, for the
    /// damage tracker and the renderer's caches.
    pub fn versions(&self) -> (u64, u64) {
        (
            self.value_version.load(Ordering::Relaxed),
            self.layout_version.load(Ordering::Relaxed),
        )
    }
}

impl fmt::Debug for Shader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Shader")
            .field("mode", &self.mode)
            .field("animated", &self.animated)
            .field(
                "inputs",
                &self.inputs.iter().map(|(n, _)| n).collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str =
        "fn shade(p: Pixel) -> vec4<f32> {\n    return vec4<f32>(p.uv, 0.0, 1.0);\n}\n";

    fn make(wgsl: &str) -> Result<Arc<Shader>, ShaderError> {
        Shader::new(wgsl.to_owned(), vec![], vec![], ShaderMode::Fill, false)
    }

    #[test]
    fn a_valid_shader_is_accepted_in_both_modes() {
        assert!(make(GOOD).is_ok());
        let effect = "fn shade(p: Pixel) -> vec4<f32> {\n    return content(p.uv);\n}\n";
        assert!(Shader::new(effect.into(), vec![], vec![], ShaderMode::Effect, false).is_ok());
        // `content` exists only for an effect.
        assert!(Shader::new(effect.into(), vec![], vec![], ShaderMode::Fill, false).is_err());
    }

    #[test]
    fn an_error_is_positioned_in_the_apps_own_source() {
        let source =
            "fn shade(p: Pixel) -> vec4<f32> {\n    let x = ;\n    return vec4<f32>(1.0);\n}\n";
        let error = make(source).unwrap_err();
        assert_eq!(error.line, Some(2), "{error}");
        assert_eq!(error.source_line.as_deref(), Some("    let x = ;"));
        assert!(error.column.is_some_and(|c| c >= 5), "{error}");
        let shown = error.to_string();
        assert!(shown.contains("(line 2, column"), "{shown}");
        assert!(shown.contains("    let x = ;\n"), "{shown}");
    }

    #[test]
    fn a_type_error_is_positioned_too() {
        let source = "fn shade(p: Pixel) -> vec4<f32> {\n    return 1.0;\n}\n";
        let error = make(source).unwrap_err();
        assert!(error.message.contains("convert"), "{error}");
    }

    #[test]
    fn a_missing_or_misshapen_shade_is_reported() {
        let none = make("fn other() {}\n").unwrap_err();
        assert!(none.message.contains("must define `fn shade"), "{none}");
        let wrong_return = make("fn shade(p: Pixel) -> f32 {\n    return 1.0;\n}\n").unwrap_err();
        assert!(
            wrong_return.message.contains("wrong signature"),
            "{wrong_return}"
        );
        let wrong_arg =
            make("fn shade(x: f32) -> vec4<f32> {\n    return vec4<f32>(x);\n}\n").unwrap_err();
        assert!(wrong_arg.message.contains("wrong signature"), "{wrong_arg}");
    }

    #[test]
    fn the_app_defines_no_entry_points_of_its_own() {
        let source = format!(
            "{GOOD}@fragment\nfn mine() -> @location(0) vec4<f32> {{\n    return vec4<f32>(1.0);\n}}\n"
        );
        let error = make(&source).unwrap_err();
        assert!(error.message.contains("not entry points"), "{error}");
        assert!(error.message.contains("mine"), "{error}");
    }

    #[test]
    fn a_name_tre_provides_is_a_clash_listing_what_tre_provides() {
        let source = format!("{GOOD}struct Pixel {{\n    a: f32,\n}}\n");
        let error = make(&source).unwrap_err();
        assert!(
            error.message.contains("tre provides these names"),
            "{error}"
        );
        assert!(error.message.contains("Pixel"), "{error}");
    }

    fn uniforms(list: &[(&str, UniformValue)]) -> Vec<(String, UniformValue)> {
        list.iter().map(|(n, v)| ((*n).to_owned(), *v)).collect()
    }

    #[test]
    fn uniforms_are_in_the_source_and_laid_out_as_wgsl_lays_them_out() {
        let list = uniforms(&[
            ("amount", UniformValue::F32(0.5)),
            ("tint", UniformValue::Vec4([1.0, 0.5, 0.25, 1.0])),
            ("centre", UniformValue::Vec2([3.0, 4.0])),
            ("axis", UniformValue::Vec3([0.0, 1.0, 0.0])),
            ("last", UniformValue::F32(9.0)),
        ]);
        let source = "fn shade(p: Pixel) -> vec4<f32> {\n    return u.tint * u.amount + vec4<f32>(u.centre, u.axis.y, u.last);\n}\n";
        let shader =
            Shader::new(source.into(), list.clone(), vec![], ShaderMode::Fill, false).unwrap();

        // The layout the module computes for itself must match ours.
        let module = naga::front::wgsl::parse_str(&shader.assembled()).unwrap();
        let members = module
            .types
            .iter()
            .find_map(|(_, t)| match (&t.name, &t.inner) {
                (Some(name), naga::TypeInner::Struct { members, span }) if name == "Uniforms" => {
                    Some((
                        members
                            .iter()
                            .map(|m| m.offset as usize)
                            .collect::<Vec<_>>(),
                        *span as usize,
                    ))
                }
                _ => None,
            })
            .expect("a Uniforms struct");
        let kinds: Vec<_> = list.iter().map(|(n, v)| (n.clone(), v.kind())).collect();
        assert_eq!((members.0.clone(), members.1), uniform_layout(&kinds));
        assert_eq!(members.0, vec![0, 16, 32, 48, 60]);
        assert_eq!(members.1, 64);

        let bytes = shader.uniform_bytes();
        assert_eq!(bytes.len(), 64);
        let f = |at: usize| f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(
            (f(0), f(16), f(20), f(24), f(28)),
            (0.5, 1.0, 0.5, 0.25, 1.0)
        );
        assert_eq!(
            (f(32), f(36), f(48), f(52), f(56), f(60)),
            (3.0, 4.0, 0.0, 1.0, 0.0, 9.0)
        );
    }

    #[test]
    fn a_shader_with_no_uniforms_has_a_placeholder_buffer() {
        let shader = make(GOOD).unwrap();
        assert_eq!(shader.uniform_bytes(), vec![0u8; 16]);
    }

    #[test]
    fn a_bad_name_is_refused_before_the_source_is_looked_at() {
        let attempt = |name: &str| {
            Shader::new(
                GOOD.into(),
                uniforms(&[(name, UniformValue::F32(1.0))]),
                vec![],
                ShaderMode::Fill,
                false,
            )
            .unwrap_err()
            .message
        };
        assert!(attempt("1abc").contains("isn't a WGSL identifier"));
        assert!(attempt("has space").contains("isn't a WGSL identifier"));
        assert!(attempt("").contains("isn't a WGSL identifier"));
        assert!(attempt("__x").contains("reserved by WGSL"));
        assert!(attempt("_").contains("reserved by WGSL"));
        assert!(attempt("var").contains("keyword or reserved"));
    }

    #[test]
    fn setting_values_only_keeps_the_pipeline_and_bumps_the_value_version() {
        let source = "fn shade(p: Pixel) -> vec4<f32> {\n    return vec4<f32>(u.amount);\n}\n";
        let shader = Shader::new(
            source.into(),
            uniforms(&[("amount", UniformValue::F32(0.25))]),
            vec![],
            ShaderMode::Fill,
            false,
        )
        .unwrap();
        let module_before = shader.assembled();
        shader
            .set_uniforms(uniforms(&[("amount", UniformValue::F32(0.75))]))
            .unwrap();
        assert!(
            Arc::ptr_eq(&module_before, &shader.assembled()),
            "the module is unchanged"
        );
        assert_eq!(shader.versions(), (1, 0));
        assert_eq!(shader.uniforms()[0].1, UniformValue::F32(0.75));
    }

    #[test]
    fn changing_a_type_revalidates_and_a_failure_changes_nothing() {
        let source = "fn shade(p: Pixel) -> vec4<f32> {\n    return vec4<f32>(u.amount);\n}\n";
        let shader = Shader::new(
            source.into(),
            uniforms(&[("amount", UniformValue::F32(0.25))]),
            vec![],
            ShaderMode::Fill,
            false,
        )
        .unwrap();
        // `u.amount` is a vec4 now, which `vec4<f32>(...)` still accepts: fine.
        shader
            .set_uniforms(uniforms(&[(
                "amount",
                UniformValue::Vec4([1.0, 2.0, 3.0, 4.0]),
            )]))
            .unwrap();
        assert_eq!(shader.versions(), (1, 1), "a new pipeline is needed");
        // Dropping `amount` breaks the source: refused, and nothing changed.
        let error = shader
            .set_uniforms(uniforms(&[("other", UniformValue::F32(1.0))]))
            .unwrap_err();
        assert!(error.line.is_some(), "{error}");
        assert_eq!(shader.uniforms()[0].0, "amount");
        assert_eq!(shader.versions(), (1, 1));
    }

    #[test]
    fn inputs_get_a_function_each_and_are_checked() {
        let mut ids = slotmap::SlotMap::<NodeId, ()>::with_key();
        let (a, b) = (ids.insert(()), ids.insert(()));
        let source = "fn shade(p: Pixel) -> vec4<f32> {\n    return input_photo(p.uv) * input_mask(p.uv);\n}\n";
        let shader = Shader::new(
            source.into(),
            vec![],
            vec![("photo".into(), a), ("mask".into(), b)],
            ShaderMode::Fill,
            false,
        )
        .unwrap();
        assert_eq!(shader.inputs().len(), 2);
        let twice = Shader::new(
            GOOD.into(),
            vec![],
            vec![("x".into(), a), ("x".into(), b)],
            ShaderMode::Fill,
            false,
        );
        assert!(twice.unwrap_err().message.contains("used twice"));
        let bad = Shader::new(
            GOOD.into(),
            vec![],
            vec![("1x".into(), a)],
            ShaderMode::Fill,
            false,
        );
        assert!(bad.unwrap_err().message.contains("input name"));
    }

    #[test]
    fn the_frame_block_holds_the_size_and_the_time() {
        let bytes = frame_block((120.0, 80.0), 2.5);
        let f = |at: usize| f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!((f(0), f(4), f(8), f(12)), (120.0, 80.0, 2.5, 0.0));
    }
}
