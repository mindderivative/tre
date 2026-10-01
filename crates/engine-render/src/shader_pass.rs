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
//! Effects are a later milestone: such a shader is skipped here, and paints
//! as if it had none.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use engine_core::{NodeId, NodeKind, Shader, ShaderMode, Tree, frame_block, node_id_as_u64};
use peniko::kurbo::Rect;
use vello_gpu::{TextureBindings, TextureId};

use crate::image_cache::{ImageInputs, ImageTextureCache, MAX_IMAGE_DIMENSION};
use crate::walk;

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
    ready: HashMap<NodeId, (u32, u32)>,
}

impl ShaderTextures {
    /// No shader textures: nothing to draw.
    pub fn none() -> Self {
        Self::default()
    }

    /// The texture's size when `id`'s shader painted this frame.
    pub(crate) fn size_of(&self, id: NodeId) -> Option<(u32, u32)> {
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
    view: wgpu::TextureView,
    version: u64,
}

/// Whether this milestone's pass draws `shader`: a fill.
fn drawn_here(shader: &Shader) -> bool {
    shader.mode() == ShaderMode::Fill
}

/// The pass list: every visible shader node, from the paint walk.
struct Collect {
    nodes: Vec<NodeId>,
}

impl<'t> walk::Visitor<'t> for Collect {
    fn enter(&mut self, v: &walk::Visit<'t>) -> bool {
        if v.node.shader.as_deref().is_some_and(drawn_here) {
            self.nodes.push(v.id);
        }
        true
    }
}

/// One frame's working state.
struct Frame<'a> {
    tree: &'a Tree,
    time: f32,
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    encoder: &'a mut wgpu::CommandEncoder,
    bindings: &'a mut TextureBindings,
    images: ImageInputs<'a>,
    /// Shader nodes already brought up to date this frame.
    done: HashMap<NodeId, Option<Ready>>,
    /// Shader nodes being resolved now, to cut a cycle.
    active: Vec<NodeId>,
}

/// A window's shader textures, pipelines, and the passes that fill them.
#[derive(Default)]
pub struct ShaderPasses {
    targets: HashMap<NodeId, Target>,
    /// Pipelines by assembled source; `None` for a module the GPU refused
    /// (logged once, never retried).
    pipelines: HashMap<Arc<str>, Option<Pipeline>>,
    /// Nodes whose texture or inputs were refused, already logged.
    refused: HashSet<NodeId>,
    sampler: Option<wgpu::Sampler>,
    generation: u64,
    /// How many passes the last `run` recorded, for tests and tracing.
    last_passes: usize,
}

impl ShaderPasses {
    pub fn new() -> Self {
        Self::default()
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
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        images: &mut ImageTextureCache,
    ) -> ShaderTextures {
        self.last_passes = 0;
        let (bindings, image_inputs) = images.split();
        self.evict(tree, bindings);
        let mut ready = ShaderTextures::none();
        if !tree.has_shaders() {
            return ready;
        }
        let visible = Rect::new(0.0, 0.0, f64::from(width), f64::from(height));
        let mut collect = Collect { nodes: Vec::new() };
        walk::walk(tree, root, visible, &mut collect);
        let mut frame = Frame {
            tree,
            time,
            device,
            queue,
            encoder,
            bindings,
            images: image_inputs,
            done: HashMap::new(),
            active: Vec::new(),
        };
        for id in collect.nodes {
            if let Some(r) = self.ensure(&mut frame, id) {
                ready.ready.insert(id, r.size);
            }
        }
        ready
    }

    /// Frees the texture of a node that was removed or lost its shader.
    fn evict(&mut self, tree: &Tree, bindings: &mut TextureBindings) {
        let gone: Vec<NodeId> = self
            .targets
            .keys()
            .chain(self.refused.iter())
            .filter(|id| {
                !tree
                    .get(**id)
                    .is_some_and(|n| n.shader.as_deref().is_some_and(drawn_here))
            })
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
        let shader = node.shader.clone().filter(|s| drawn_here(s))?;
        let layout = f.tree.layout(id).size;
        let size = (layout.width.ceil() as u32, layout.height.ceil() as u32);
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
        self.run_pass(f, id, &shader, size, &views, versions)
    }

    /// Runs one node's pass if it is stale.
    fn run_pass(
        &mut self,
        f: &mut Frame<'_>,
        id: NodeId,
        shader: &Arc<Shader>,
        size: (u32, u32),
        views: &[wgpu::TextureView],
        versions: Vec<u64>,
    ) -> Option<Ready> {
        let source = shader.assembled();
        let uniform_bytes = shader.uniform_bytes();
        if !self.pipelines.contains_key(&source) {
            let built = build_pipeline(f.device, &source, uniform_bytes.len(), views.len());
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

        let uniform_len = uniform_bytes.len() as u64;
        let reuse = self
            .targets
            .get(&id)
            .is_some_and(|t| t.size == size && t.uniforms.size() == uniform_len);
        if !reuse {
            let target = new_target(f.device, size, uniform_len);
            f.bindings.insert(texture_id_for(id), target.view.clone());
            self.targets.insert(id, target);
        }
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
                let mut rpass = f.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("tre fill shader"),
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
            view: target.view.clone(),
            version: target.generation,
        })
    }
}

fn new_target(device: &wgpu::Device, size: (u32, u32), uniform_len: u64) -> Target {
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
    Target {
        _texture: texture,
        view,
        size,
        frame: buffer("tre shader frame", engine_core::FRAME_BLOCK_SIZE as u64),
        uniforms: buffer("tre shader uniforms", uniform_len),
        key: None,
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
) -> Option<Pipeline> {
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("tre fill shader"),
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
