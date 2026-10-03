//! 0.5.1 (#67, #68): the GPU pass for fill shaders.
//!
//! A node with a `mode="fill"` [`Shader`] paints its box from the app's
//! `shade` function. Each frame, before the scene is rendered, this module
//! runs one render pass per visible shader node into a texture of the node's
//! box size, and binds the texture (premultiplied, as `vello_gpu`'s external
//! textures must be) so the scene draws it in the node's place -- the same
//! seam images use (`image_cache`), under a `TextureId` of its own.
//!
//! The pass list comes from the paint walk (`walk`), so it follows what
//! painting follows: a hidden, fully transparent, or off-screen node is not
//! reached, and runs no pass and keeps no new texture (#62). A pass runs only
//! when its inputs changed -- the shader, its uniforms, the node's size, an
//! input's content, or the frame's time for an animated shader -- so an idle
//! shader costs nothing.
//!
//! Inputs (#68). A shader names nodes it samples. An `image` or `video`
//! node is its image pixels; any other node is its own shader's output, so a
//! shader reads another shader. Dependencies run first, in the order the
//! recursion reaches them, which is also the order the passes are recorded
//! in; an input shader runs even when its node is off-screen, since it is
//! needed. A cycle is refused when a shader is set (`Tree::shader_cycle`),
//! and skipped here regardless.
//!
//! A problem only the GPU finds -- the pipeline won't build, the texture is
//! too large, an input has no texture -- is logged once and the node paints
//! as if it had no shader; it never takes down the frame (#60).
//!
//! Effects (#69). A `mode="effect"` shader receives the node's own rendered
//! content -- its paint and descendants, with `vello`, as if the node were
//! the whole scene -- as `content(uv)`, and its result replaces the node and
//! its subtree in the main scene. The content render is offscreen, into a
//! texture of the node's box (overflow past the box is not part of it), and
//! only when something in the subtree changed (`damage`'s fingerprint). The
//! renderer writes its buffers when work is *submitted*, so each offscreen
//! render is submitted on its own, in dependency order, before the frame's
//! own encoder: every shader pass and content render here goes to the queue
//! ahead of the main scene.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use engine_core::{NodeId, NodeKind, Shader, ShaderMode, Tree, frame_block, node_id_as_u64};
use peniko::kurbo::{Affine, Rect};
use vello_gpu::{
    ClearSettings, RenderSize, Renderer, Resources, TargetInit, TextureBindings, TextureId,
};

use crate::image_cache::{ImageInputs, ImageTextureCache, MAX_IMAGE_DIMENSION};
use crate::{GeometryCache, TextRenderer, build_effect_content, walk};

/// The format shader textures are rendered in: what `image_cache` uses, so
/// `vello_gpu` samples both alike.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// How deep a chain of shaders reading shaders may go before it is cut off
/// (a defence: a cycle is refused earlier).
const MAX_DEPTH: usize = 16;

/// The `TextureId` a node's shader texture is bound under: the node's id
/// with the top bit set, so it never meets the same node's image texture.
pub(crate) fn texture_id_for(id: NodeId) -> TextureId {
    TextureId(node_id_as_u64(id) | (1 << 63))
}

/// The nodes whose shader texture is current this frame, by texture size --
/// what the scene may draw.
#[derive(Default)]
pub struct ShaderTextures {
    /// Texture size, and whether it is an effect's result (which replaces
    /// the node's subtree) rather than a fill (which paints behind it).
    ready: HashMap<NodeId, ((u32, u32), bool)>,
}

impl ShaderTextures {
    /// No shader textures: nothing to draw.
    pub fn none() -> Self {
        Self::default()
    }

    /// The texture's size, and whether it is an effect, when `id`'s shader
    /// ran this frame.
    pub(crate) fn texture_of(&self, id: NodeId) -> Option<((u32, u32), bool)> {
        self.ready.get(&id).copied()
    }

    /// Whether any shader texture is ready.
    pub fn is_empty(&self) -> bool {
        self.ready.is_empty()
    }
}

/// What a pass was last run with: the pass runs again when this changes.
#[derive(Clone, PartialEq)]
struct Key {
    shader: usize,
    values: u64,
    layout: u64,
    size: (u32, u32),
    /// The time an animated shader ran at, as bits; `0` for one that isn't.
    time: u32,
    /// Each input's content version, in input order.
    inputs: Vec<u64>,
    /// An effect's subtree fingerprint; `0` for a fill.
    content: u64,
}

/// An effect node's offscreen content texture.
struct Content {
    // Held so the texture outlives its view.
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// The subtree fingerprint it was last rendered for.
    key: Option<u64>,
}

/// One node's texture and the buffers its pass writes.
struct Target {
    // Held so the texture outlives its view in `bindings`.
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: (u32, u32),
    frame: wgpu::Buffer,
    uniforms: wgpu::Buffer,
    key: Option<Key>,
    content: Option<Content>,
    /// Bumped every time the pass runs: what a shader reading this one sees
    /// change.
    generation: u64,
}

/// A built pipeline for one assembled module.
struct Pipeline {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

/// A node's shader texture, current this frame.
#[derive(Clone)]
struct Ready {
    size: (u32, u32),
    effect: bool,
    view: wgpu::TextureView,
    version: u64,
}

/// The pass list: every visible shader node, from the paint walk.
struct Collect {
    nodes: Vec<NodeId>,
    /// A node to leave out: the effect root of an offscreen walk.
    skip: Option<NodeId>,
}

impl<'t> walk::Visitor<'t> for Collect {
    fn enter(&mut self, v: &walk::Visit<'t>) -> bool {
        if v.node.shader.is_some() && self.skip != Some(v.id) {
            self.nodes.push(v.id);
        }
        true
    }
}

/// The renderer state a frame's shader work uses: the `vello` renderer for an
/// effect's offscreen content, and what builds its scene.
pub(crate) struct Gpu<'a> {
    pub renderer: &'a mut Renderer,
    pub resources: &'a mut Resources,
    pub images: &'a mut ImageTextureCache,
    pub text: &'a mut TextRenderer,
    pub geometry: &'a mut GeometryCache,
}

/// One frame's working state.
struct Frame<'a> {
    tree: &'a Tree,
    time: f32,
    /// The display scale: textures are sized in physical pixels, a node's
    /// layout in logical ones (0.5.4, #102).
    scale: f64,
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    /// Pass work not yet submitted.
    encoder: Option<wgpu::CommandEncoder>,
    renderer: &'a mut Renderer,
    resources: &'a mut Resources,
    text: &'a mut TextRenderer,
    geometry: &'a mut GeometryCache,
    bindings: &'a mut TextureBindings,
    images: ImageInputs<'a>,
    /// Shader nodes already brought up to date this frame.
    done: HashMap<NodeId, Option<Ready>>,
    /// Shader nodes being resolved now, to cut a cycle.
    active: Vec<NodeId>,
}

/// A window's shader textures, pipelines, and the passes that fill them.
pub struct ShaderPasses {
    targets: HashMap<NodeId, Target>,
    /// Pipelines by assembled source; `None` for a module the GPU refused
    /// (logged once, never retried).
    pipelines: HashMap<Arc<str>, Option<Pipeline>>,
    /// Nodes whose texture or inputs were refused, already logged.
    refused: HashSet<NodeId>,
    sampler: Option<wgpu::Sampler>,
    /// The window surface's format, which `vello` renders an effect's
    /// content in.
    format: wgpu::TextureFormat,
    generation: u64,
    /// How many passes the last `run` recorded, for tests and tracing.
    last_passes: usize,
    /// Whether a shader that ran in the last `run` is `animated`.
    animated: bool,
}

impl Frame<'_> {
    /// The encoder for pass work, made on first use.
    fn encoder(&mut self) -> &mut wgpu::CommandEncoder {
        let device = self.device;
        self.encoder.get_or_insert_with(|| {
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tre shader passes"),
            })
        })
    }

    /// Submits the pass work recorded so far.
    fn flush(&mut self) {
        if let Some(encoder) = self.encoder.take() {
            self.queue.submit([encoder.finish()]);
        }
    }

    /// The shader textures current so far this frame: what an effect's
    /// content scene may draw.
    fn ready(&self) -> ShaderTextures {
        ShaderTextures {
            ready: self
                .done
                .iter()
                .filter_map(|(id, r)| r.as_ref().map(|r| (*id, (r.size, r.effect))))
                .collect(),
        }
    }
}

impl ShaderPasses {
    /// Shader passes for a window whose surface is `format`.
    pub fn new(format: wgpu::TextureFormat) -> Self {
        Self {
            targets: HashMap::new(),
            pipelines: HashMap::new(),
            refused: HashSet::new(),
            sampler: None,
            format,
            generation: 0,
            last_passes: 0,
            animated: false,
        }
    }

    /// Whether a shader that ran in the last `run` is animated -- the loop
    /// must keep drawing frames while one does.
    pub fn animated(&self) -> bool {
        self.animated
    }

    /// A frame with nothing to draw runs no pass.
    pub(crate) fn skipped(&mut self) {
        self.last_passes = 0;
    }

    /// How many passes the last [`run`](Self::run) recorded.
    pub fn last_pass_count(&self) -> usize {
        self.last_passes
    }

    /// Records this frame's passes into `encoder`, binds their textures, and
    /// says which nodes' textures are ready to draw. Call before the scene
    /// is rendered, with the same viewport as the scene.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn run(
        &mut self,
        tree: &Tree,
        root: NodeId,
        width: u16,
        height: u16,
        time: f32,
        scale: f64,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        gpu: Gpu<'_>,
    ) -> ShaderTextures {
        self.last_passes = 0;
        self.animated = false;
        let Gpu {
            renderer,
            resources,
            images,
            text,
            geometry,
        } = gpu;
        let (bindings, image_inputs) = images.split();
        self.evict(tree, bindings);
        let mut ready = ShaderTextures::none();
        if !tree.has_shaders() {
            return ready;
        }
        let visible = Rect::new(0.0, 0.0, f64::from(width), f64::from(height));
        let mut collect = Collect {
            nodes: Vec::new(),
            skip: None,
        };
        walk::walk(tree, root, Affine::scale(scale), visible, &mut collect);
        let mut frame = Frame {
            tree,
            time,
            scale,
            device,
            queue,
            encoder: None,
            renderer,
            resources,
            text,
            geometry,
            bindings,
            images: image_inputs,
            done: HashMap::new(),
            active: Vec::new(),
        };
        for id in collect.nodes {
            if let Some(r) = self.ensure(&mut frame, id) {
                ready.ready.insert(id, (r.size, r.effect));
            }
        }
        frame.flush();
        ready
    }

    /// Frees the texture of a node that was removed or lost its shader.
    fn evict(&mut self, tree: &Tree, bindings: &mut TextureBindings) {
        let gone: Vec<NodeId> = self
            .targets
            .keys()
            .chain(self.refused.iter())
            .filter(|id| !tree.get(**id).is_some_and(|n| n.shader.is_some()))
            .copied()
            .collect();
        for id in gone {
            if self.targets.remove(&id).is_some() {
                bindings.remove(texture_id_for(id));
            }
            self.refused.remove(&id);
        }
    }

    /// Brings `id`'s shader texture up to date, running its inputs' passes
    /// first; `None` if it can't be drawn.
    fn ensure(&mut self, f: &mut Frame<'_>, id: NodeId) -> Option<Ready> {
        if let Some(done) = f.done.get(&id) {
            return done.clone();
        }
        if f.active.contains(&id) || f.active.len() >= MAX_DEPTH {
            return None;
        }
        f.active.push(id);
        let result = self.resolve(f, id);
        f.active.pop();
        f.done.insert(id, result.clone());
        result
    }

    /// One input's texture: an image node's pixels, or another shader's
    /// output.
    fn input(&mut self, f: &mut Frame<'_>, id: NodeId) -> Option<(wgpu::TextureView, u64)> {
        if matches!(f.tree.get(id)?.kind, NodeKind::Image(_)) {
            return f.images.get(id);
        }
        self.ensure(f, id).map(|r| (r.view, r.version))
    }

    fn resolve(&mut self, f: &mut Frame<'_>, id: NodeId) -> Option<Ready> {
        let node = f.tree.get(id)?;
        let shader = node.shader.clone()?;
        let effect = shader.mode() == ShaderMode::Effect;
        self.animated |= shader.animated();
        let layout = f.tree.layout(id).size;
        let size = (
            (f64::from(layout.width) * f.scale).ceil() as u32,
            (f64::from(layout.height) * f.scale).ceil() as u32,
        );
        if size.0 == 0 || size.1 == 0 {
            return None;
        }
        if size.0 > MAX_IMAGE_DIMENSION || size.1 > MAX_IMAGE_DIMENSION {
            if self.refused.insert(id) {
                tracing::warn!(
                    width = size.0,
                    height = size.1,
                    max = MAX_IMAGE_DIMENSION,
                    "a shader node is larger than the GPU's textures can be; painted without its shader"
                );
            }
            return None;
        }
        let mut views = Vec::new();
        let mut versions = Vec::new();
        for (name, input) in shader.inputs() {
            let Some((view, version)) = self.input(f, *input) else {
                if self.refused.insert(id) {
                    tracing::warn!(
                        input = %name,
                        "a shader's input has no texture (not an image, or a shader that can't be drawn); painted without the shader"
                    );
                }
                return None;
            };
            views.push(view);
            versions.push(version);
        }
        self.refused.remove(&id);

        // An effect's content is its subtree's own render, redone when the
        // subtree changed (or the texture is new).
        let uniform_len = shader.uniform_bytes().len() as u64;
        let mut content = 0;
        if effect {
            content = crate::damage::effect_content_fingerprint(f.tree, f.time, id);
            let reusable = self.reusable(id, size, uniform_len, true);
            let stale = !reusable
                || self
                    .targets
                    .get(&id)
                    .and_then(|t| t.content.as_ref())
                    .is_none_or(|c| c.key != Some(content));
            if stale && !self.render_content(f, id, size, uniform_len, content) {
                return None;
            }
        } else {
            self.prepare_target(f, id, size, uniform_len, false);
        }
        self.run_pass(f, id, &shader, size, &views, versions, content)
    }

    /// Whether `id`'s texture and buffers already fit this size.
    fn reusable(&self, id: NodeId, size: (u32, u32), uniform_len: u64, effect: bool) -> bool {
        self.targets.get(&id).is_some_and(|t| {
            t.size == size && t.uniforms.size() == uniform_len && t.content.is_some() == effect
        })
    }

    /// Makes `id`'s texture, buffers and (for an effect) content texture fit.
    fn prepare_target(
        &mut self,
        f: &mut Frame<'_>,
        id: NodeId,
        size: (u32, u32),
        uniform_len: u64,
        effect: bool,
    ) {
        if self.reusable(id, size, uniform_len, effect) {
            return;
        }
        let target = new_target(f.device, size, uniform_len, effect.then_some(self.format));
        f.bindings.insert(texture_id_for(id), target.view.clone());
        self.targets.insert(id, target);
    }

    /// Renders an effect node's own content into its content texture, first
    /// bringing up to date every shader in its subtree, and submitting each
    /// step so the next sees it. `false` if the render failed.
    fn render_content(
        &mut self,
        f: &mut Frame<'_>,
        id: NodeId,
        size: (u32, u32),
        uniform_len: u64,
        content: u64,
    ) -> bool {
        // The shaders inside this node paint into its content.
        let mut inner = Collect {
            nodes: Vec::new(),
            skip: Some(id),
        };
        walk::walk_root(f.tree, id, Affine::scale(f.scale), &mut inner);
        for node in inner.nodes {
            self.ensure(f, node);
        }
        self.prepare_target(f, id, size, uniform_len, true);
        let Some(view) = self
            .targets
            .get(&id)
            .and_then(|t| t.content.as_ref())
            .map(|c| c.view.clone())
        else {
            return false;
        };
        f.flush();
        let ready = f.ready();
        let scene = build_effect_content(
            f.tree,
            id,
            size.0 as u16,
            size.1 as u16,
            &ready,
            f.scale,
            f.resources,
            f.text,
            f.geometry,
        );
        let mut encoder = f
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tre effect content"),
            });
        let rendered = f.renderer.render(
            &scene,
            f.resources,
            f.device,
            f.queue,
            &mut encoder,
            &RenderSize {
                width: size.0 as u16,
                height: size.1 as u16,
            },
            &view,
            None,
            f.bindings,
            TargetInit::Clear(ClearSettings::default()),
        );
        if let Err(error) = rendered {
            if self.refused.insert(id) {
                tracing::warn!(%error, "an effect's content could not be rendered; painted without its shader");
            }
            return false;
        }
        f.queue.submit([encoder.finish()]);
        if let Some(c) = self.targets.get_mut(&id).and_then(|t| t.content.as_mut()) {
            c.key = Some(content);
        }
        true
    }

    /// Runs one node's pass if it is stale.
    #[allow(clippy::too_many_arguments)]
    fn run_pass(
        &mut self,
        f: &mut Frame<'_>,
        id: NodeId,
        shader: &Arc<Shader>,
        size: (u32, u32),
        views: &[wgpu::TextureView],
        versions: Vec<u64>,
        content: u64,
    ) -> Option<Ready> {
        let effect = shader.mode() == ShaderMode::Effect;
        let source = shader.assembled();
        let uniform_bytes = shader.uniform_bytes();
        if !self.pipelines.contains_key(&source) {
            let built = build_pipeline(f.device, &source, uniform_bytes.len(), views.len(), effect);
            self.pipelines.insert(source.clone(), built);
        }
        let sampler = self
            .sampler
            .get_or_insert_with(|| {
                f.device.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some("tre shader input sampler"),
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    ..Default::default()
                })
            })
            .clone();
        let pipeline = self.pipelines.get(&source)?.as_ref()?;
        let target = self.targets.get_mut(&id)?;

        let (values, layout) = shader.versions();
        let key = Key {
            shader: Arc::as_ptr(shader) as usize,
            values,
            layout,
            size,
            time: if shader.animated() {
                f.time.to_bits()
            } else {
                0
            },
            inputs: versions,
            content,
        };
        if target.key.as_ref() != Some(&key) {
            f.queue.write_buffer(
                &target.frame,
                0,
                &frame_block((size.0 as f32, size.1 as f32), f.time),
            );
            f.queue.write_buffer(&target.uniforms, 0, &uniform_bytes);
            let mut entries = vec![
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: target.frame.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: target.uniforms.as_entire_binding(),
                },
            ];
            if effect {
                let content_view = &target.content.as_ref()?.view;
                entries.push(wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(content_view),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                });
            }
            for (i, view) in views.iter().enumerate() {
                entries.push(wgpu::BindGroupEntry {
                    binding: 4 + 2 * i as u32,
                    resource: wgpu::BindingResource::TextureView(view),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 5 + 2 * i as u32,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                });
            }
            let group = f.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("tre shader bind group"),
                layout: &pipeline.layout,
                entries: &entries,
            });
            {
                let mut rpass = f.encoder().begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("tre shader"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target.view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                rpass.set_pipeline(&pipeline.pipeline);
                rpass.set_bind_group(0, &group, &[]);
                rpass.draw(0..3, 0..1);
            }
            self.generation += 1;
            target.generation = self.generation;
            target.key = Some(key);
            self.last_passes += 1;
        }
        Some(Ready {
            size,
            effect,
            view: target.view.clone(),
            version: target.generation,
        })
    }
}

fn new_target(
    device: &wgpu::Device,
    size: (u32, u32),
    uniform_len: u64,
    content_format: Option<wgpu::TextureFormat>,
) -> Target {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("tre shader node texture"),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let buffer = |label, size| {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    };
    let content = content_format.map(|format| {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tre effect content texture"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Content {
            _texture: texture,
            view,
            key: None,
        }
    });
    Target {
        _texture: texture,
        view,
        size,
        frame: buffer("tre shader frame", engine_core::FRAME_BLOCK_SIZE as u64),
        uniforms: buffer("tre shader uniforms", uniform_len),
        key: None,
        content,
        generation: 0,
    }
}

/// Builds the pipeline for `source`, with any validation error caught and
/// logged once rather than reaching the device's error handler.
fn build_pipeline(
    device: &wgpu::Device,
    source: &Arc<str>,
    uniform_len: usize,
    inputs: usize,
    effect: bool,
) -> Option<Pipeline> {
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("tre shader"),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(source)),
    });
    let uniform = |binding, min| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(min),
        },
        count: None,
    };
    let mut entries = vec![
        uniform(0, engine_core::FRAME_BLOCK_SIZE as u64),
        uniform(1, uniform_len as u64),
    ];
    if effect {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        });
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 3,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
    }
    for i in 0..inputs as u32 {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 4 + 2 * i,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        });
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 5 + 2 * i,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
    }
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("tre shader bind group layout"),
        entries: &entries,
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("tre shader pipeline layout"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("tre fill shader pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("tre_vertex"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("tre_fragment"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    match pop(scope) {
        None => Some(Pipeline { pipeline, layout }),
        Some(error) => {
            tracing::warn!(%error, "a shader was refused by the GPU; its nodes paint without it");
            None
        }
    }
}

/// The error a scope caught. A scope's future is ready at once on a native
/// device; one that isn't is taken as no error.
fn pop(scope: wgpu::ErrorScopeGuard) -> Option<wgpu::Error> {
    let mut future = std::pin::pin!(scope.pop());
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(error) => error,
        Poll::Pending => None,
    }
}
