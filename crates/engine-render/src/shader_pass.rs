//! 0.5.1 (#67): the GPU pass for fill shaders.
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
//! when its inputs changed -- the shader, its uniforms, the node's size, or
//! the frame's time for an animated shader -- so an idle shader costs
//! nothing.
//!
//! A problem only the GPU finds -- the pipeline won't build, the texture is
//! too large -- is logged once and the node paints as if it had no shader;
//! it never takes down the frame (#60).
//!
//! Effects and inputs are later milestones: a shader with either is skipped
//! here, and paints as if it had none.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use engine_core::{NodeId, Shader, ShaderMode, Tree, frame_block, node_id_as_u64};
use peniko::kurbo::Rect;
use vello_gpu::{TextureBindings, TextureId};

use crate::image_cache::MAX_IMAGE_DIMENSION;
use crate::walk;

/// The format shader textures are rendered in: what `image_cache` uses, so
/// `vello_gpu` samples both alike.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

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
}

/// A built pipeline for one assembled module.
struct Pipeline {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

/// One entry of a frame's pass list.
struct Pass {
    id: NodeId,
    shader: Arc<Shader>,
    size: (u32, u32),
}

/// Whether this milestone's pass draws `shader`: a fill with no inputs.
fn drawn_here(shader: &Shader) -> bool {
    shader.mode() == ShaderMode::Fill && shader.inputs().is_empty()
}

/// The pass list: every visible, drawable shader node, from the paint walk.
struct Collect {
    passes: Vec<Pass>,
}

impl<'t> walk::Visitor<'t> for Collect {
    fn enter(&mut self, v: &walk::Visit<'t>) -> bool {
        if let Some(shader) = &v.node.shader
            && drawn_here(shader)
        {
            self.passes.push(Pass {
                id: v.id,
                shader: shader.clone(),
                size: (v.w.ceil() as u32, v.h.ceil() as u32),
            });
        }
        true
    }
}

/// A window's shader textures, pipelines, and the passes that fill them.
#[derive(Default)]
pub struct ShaderPasses {
    targets: HashMap<NodeId, Target>,
    /// Pipelines by assembled source; `None` for a module the GPU refused
    /// (logged once, never retried).
    pipelines: HashMap<Arc<str>, Option<Pipeline>>,
    /// Nodes whose texture was refused, already logged.
    refused: HashSet<NodeId>,
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

    /// Records this frame's passes into `encoder`, binds their textures in
    /// `bindings`, and says which nodes' textures are ready to draw. Call
    /// before the scene is rendered, with the same viewport as the scene.
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
        bindings: &mut TextureBindings,
    ) -> ShaderTextures {
        self.last_passes = 0;
        self.evict(tree, bindings);
        let mut ready = ShaderTextures::none();
        if !tree.has_shaders() {
            return ready;
        }
        let visible = Rect::new(0.0, 0.0, f64::from(width), f64::from(height));
        let mut collect = Collect { passes: Vec::new() };
        walk::walk(tree, root, visible, &mut collect);
        for pass in collect.passes {
            if let Some(size) = self.run_pass(&pass, time, device, queue, encoder, bindings) {
                ready.ready.insert(pass.id, size);
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
                    .is_some_and(|n| n.shader.as_ref().is_some_and(|s| drawn_here(s)))
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

    /// Runs one node's pass if it is stale; the texture's size if it is now
    /// current, `None` if the GPU refused it.
    fn run_pass(
        &mut self,
        pass: &Pass,
        time: f32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        bindings: &mut TextureBindings,
    ) -> Option<(u32, u32)> {
        let (w, h) = pass.size;
        if w == 0 || h == 0 {
            return None;
        }
        if w > MAX_IMAGE_DIMENSION || h > MAX_IMAGE_DIMENSION {
            if self.refused.insert(pass.id) {
                tracing::warn!(
                    width = w,
                    height = h,
                    max = MAX_IMAGE_DIMENSION,
                    "a shader node is larger than the GPU's textures can be; painted without its shader"
                );
            }
            return None;
        }
        self.refused.remove(&pass.id);
        let source = pass.shader.assembled();
        let uniform_bytes = pass.shader.uniform_bytes();
        if !self.pipelines.contains_key(&source) {
            let built = build_pipeline(device, &source, uniform_bytes.len());
            self.pipelines.insert(source.clone(), built);
        }
        let pipeline = self.pipelines.get(&source)?.as_ref()?;

        let uniform_len = uniform_bytes.len() as u64;
        let reuse = self
            .targets
            .get(&pass.id)
            .is_some_and(|t| t.size == pass.size && t.uniforms.size() == uniform_len);
        if !reuse {
            let target = new_target(device, pass.size, uniform_len);
            bindings.insert(texture_id_for(pass.id), target.view.clone());
            self.targets.insert(pass.id, target);
        }
        let target = self.targets.get_mut(&pass.id)?;

        let (values, layout) = pass.shader.versions();
        let key = Key {
            shader: Arc::as_ptr(&pass.shader) as usize,
            values,
            layout,
            size: pass.size,
            time: if pass.shader.animated() {
                time.to_bits()
            } else {
                0
            },
        };
        if target.key.as_ref() == Some(&key) {
            return Some(pass.size);
        }

        queue.write_buffer(&target.frame, 0, &frame_block((w as f32, h as f32), time));
        queue.write_buffer(&target.uniforms, 0, &uniform_bytes);
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tre shader bind group"),
            layout: &pipeline.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: target.frame.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: target.uniforms.as_entire_binding(),
                },
            ],
        });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
        target.key = Some(key);
        self.last_passes += 1;
        Some(pass.size)
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
    }
}

/// Builds the pipeline for `source`, with any validation error caught and
/// logged once rather than reaching the device's error handler.
fn build_pipeline(
    device: &wgpu::Device,
    source: &Arc<str>,
    uniform_len: usize,
) -> Option<Pipeline> {
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("tre fill shader"),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(source)),
    });
    let entry = |binding, min| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(min),
        },
        count: None,
    };
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("tre shader bind group layout"),
        entries: &[
            entry(0, engine_core::FRAME_BLOCK_SIZE as u64),
            entry(1, uniform_len as u64),
        ],
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
