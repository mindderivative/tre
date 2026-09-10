//! Phase 7 Step 7.2.1 -- **Fixed (2026-09-08).** REVIEW.md finding #130
//! has the full account: sampling a texture while the active render
//! target was an offscreen texture (not the swapchain) read back
//! all-zero/background data on real hardware, with no validation error,
//! across thirteen independently tested and ruled-out hypotheses over
//! two research sessions -- all of them focused on the bindless texture
//! array, since every failing case happened to sample through it.
//!
//! **The bindless array was never the real cause.** A third session
//! built `dual_kawase_nonbindless_experiment.rs`, a single-hop test that
//! sampled through a plain, conventional descriptor instead of the
//! bindless array -- and it worked, seemingly confirming the bindless
//! theory. But converting this file's real 5-hop chain to that same
//! non-bindless approach *still failed* past the second hop, which the
//! single-hop experiment had never actually exercised. Bisecting hop by
//! hop isolated the real defect to `RhiCommandBuffer::draw_indexed`
//! (`tre-rhi-vulkan/src/lib.rs`): it unconditionally issues its own
//! second `cmd_push_constants` call using `self.width`/`self.height`
//! (this render target's own dimensions, as last set by `begin_render_
//! to_texture`/`begin_render_to_texture_no_end`), which silently
//! clobbers any `screen_size` push constant a caller already made for a
//! custom, non-bindless pipeline. This is normally invisible because
//! `self.width`/`self.height` usually do match what the caller intended
//! -- but `RhiDevice::acquire_transient_target`'s own documented
//! "oversized borrow" fallback can return a texture *larger* than
//! requested when no free bucket of the exact size exists yet (which is
//! exactly what happened here: L2/U1/U0's bucket sizes are each
//! requested for the first time only after L0/L1 have already been
//! released back into the pool). When that happens, the wrapper's
//! redundant push silently substitutes the render target's real
//! (oversized) dimensions for the intended per-hop `screen_size`,
//! confining the actual draw to a small corner of the larger backing
//! image while every later hop keeps sampling/rendering at the real,
//! larger extent -- reading back untouched clear-color everywhere except
//! that corner. Bindless vs. non-bindless made no difference to this;
//! the original bindless failures likely hit the exact same mechanism,
//! just never diagnosed past "the bindless read fails."
//!
//! **The fix**: every non-bindless downsample/upsample/composite pass in
//! this file issues its draw via a raw `cmd_draw_indexed` call instead
//! of `RhiCommandBuffer::draw_indexed`, so the wrapper's own redundant,
//! potentially-stale push constant call never executes for these
//! passes. `register_bindless`/`deregister_bindless` are still gone from
//! this file (the non-bindless conversion itself -- a plain
//! `COMBINED_IMAGE_SAMPLER` per hop instead of the bindless array -- is
//! kept, since it's a real, independently-reasonable design matching how
//! other engines implement this exact kind of same-frame offscreen
//! backdrop blur, even though it turned out not to be what was actually
//! broken).
//!
//! **Update (2026-09-08, REVIEW.md finding #152):** the same underlying
//! defect (a render target's own *real* dimensions silently diverging
//! from what a caller's vertex data assumed) also broke the standard,
//! bindless `PushLayer`/`PopLayer` compositing path, via `RhiDevice::
//! acquire_transient_target`'s own "oversized borrow" fallback rather
//! than a stale `draw_indexed` push. The real, general fix landed at the
//! shared `begin_render_to_texture`/`begin_render_to_texture_no_end`
//! trait level (an explicit `logical_width`/`logical_height` parameter),
//! making this file's own raw-`cmd_draw_indexed` workaround technically
//! redundant for *new* code -- kept here anyway, unchanged, since
//! reverting an already-shipped, already-verified step to prove that
//! point is no part of what finding #152 needed fixed.
//!
//! Real, unmodified RHI machinery, unaffected by this fix: the
//! render-to-texture lifecycle (`begin_render_to_texture`/`end_render_
//! to_texture`/`begin_render_to_texture_no_end`/`resume_swapchain_
//! rendering`, including the real barriers each one performs) and the
//! transient-target pool (`acquire_transient_target`/`release_
//! transient_target`) -- both are working exactly as documented; this
//! file just wasn't accounting for what `draw_indexed` does with the
//! sizes they report.
//!
//! Hand-rolled Vulkan calls throughout for the downsample/upsample/
//! composite passes (raw `ash::Device` access via `VulkanDevice::
//! device`, `RhiCommandBuffer::raw_handle()` to recover the real
//! `vk::CommandBuffer`) -- necessary since `RhiCommandBuffer::
//! set_pipeline` always rebinds the bindless descriptor set
//! unconditionally, which would be wrong for this file's own,
//! deliberately different pipeline layout, and since `draw_indexed`
//! itself is unsafe to use here for the reason explained above. The
//! square's own initial draw into L0 still uses the normal, unmodified
//! bindless pipeline via the ordinary `RhiCommandBuffer` trait methods
//! (including its own `draw_indexed` call) -- L0 is always freshly
//! allocated at its own exact requested size (nothing has been released
//! into the pool yet at that point), so the hazard this file works
//! around never applies to that one draw.
//!
//! 1. Draw a small white square into a full-size transient target (L0),
//!    via the existing, unmodified `sdf_rounded_rect` pipeline.
//! 2. Downsample L0 -> L1 (half size) -> L2 (quarter size), each pass
//!    the real 5-tap `kawase_downsample_nonbindless` filter.
//! 3. Upsample L2 -> U1 (half size) -> U0 (full size), each pass the
//!    real 8-tap `kawase_upsample_nonbindless` filter.
//! 4. Composite U0 back onto the swapchain via `passthrough_
//!    nonbindless`, a plain textured-quad sample -- functionally
//!    identical to `bindless_textured.frag`'s own textured branch.
//!
//! `create_pipeline`'s shared push-constant layout has no room for a
//! dedicated `half_pixel` field, so each Kawase shader derives it from
//! `screen_size` (its own destination's real dimensions) alone --
//! correct only because this demo always sizes an adjacent level at
//! exactly 2x/0.5x, which it does by construction (see each shader's own
//! comment for the derivation).

use ash::vk;
use ash::vk::Handle;
use tre_engine::{
    rgba8, submit_frame, RenderingCanvas, RhiBuffer, RhiDevice, TextureFormat, UiVertex,
};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const SWAPCHAIN_WIDTH: u32 = 256;
const SWAPCHAIN_HEIGHT: u32 = 128;

// Three distinct real sizes in the chain: full, half, quarter -- each
// level after L0 exactly half its predecessor, matching what every
// Kawase shader's own `half_pixel` derivation requires by construction.
const SIZE_FULL: (u32, u32) = (SWAPCHAIN_WIDTH, SWAPCHAIN_HEIGHT);
const SIZE_HALF: (u32, u32) = (SWAPCHAIN_WIDTH / 2, SWAPCHAIN_HEIGHT / 2);
const SIZE_QUARTER: (u32, u32) = (SWAPCHAIN_WIDTH / 4, SWAPCHAIN_HEIGHT / 4);

// A small square, well clear of every edge, so the blur's own spread has
// real background on all sides to bleed into.
const SQUARE_SIZE: f32 = 20.0;
const SQUARE_X: f32 = (SWAPCHAIN_WIDTH as f32 - SQUARE_SIZE) / 2.0;
const SQUARE_Y: f32 = (SWAPCHAIN_HEIGHT as f32 - SQUARE_SIZE) / 2.0;

/// A plain textured quad covering `(x, y)`-`(x + width, y + height)`,
/// sampling its bound texture across its full `(0,0)`-`(1,1)` extent --
/// the same shape `render_to_texture_demo.rs`'s own `textured_quad`
/// already established (Step 6.4.1), reused unchanged here for every
/// downsample/upsample/composite pass's own full-target quad.
fn textured_quad(x: f32, y: f32, width: f32, height: f32) -> [UiVertex; 4] {
    let white = rgba8(255, 255, 255, 255);
    [
        UiVertex {
            position: [x, y],
            uv: [0.0, 0.0],
            color: white,
            params: [0.0; 3],
        },
        UiVertex {
            position: [x + width, y],
            uv: [1.0, 0.0],
            color: white,
            params: [0.0; 3],
        },
        UiVertex {
            position: [x + width, y + height],
            uv: [1.0, 1.0],
            color: white,
            params: [0.0; 3],
        },
        UiVertex {
            position: [x, y + height],
            uv: [0.0, 1.0],
            color: white,
            params: [0.0; 3],
        },
    ]
}

fn create_shader_module(device: &ash::Device, spv_bytes: &[u8]) -> vk::ShaderModule {
    let mut cursor = std::io::Cursor::new(spv_bytes);
    let code = ash::util::read_spv(&mut cursor).expect("invalid SPIR-V bytecode");
    let info = vk::ShaderModuleCreateInfo::default().code(&code);
    unsafe { device.create_shader_module(&info, None) }.expect("failed to create shader module")
}

/// Hand-rolled pipeline creation, matching `VulkanDevice::create_pipeline`'s
/// own real state (vertex input, blend, dynamic viewport/scissor) exactly
/// -- the only real difference is `layout`, which the caller supplies
/// instead of always getting the universal bindless one. Proven correct
/// by `dual_kawase_nonbindless_experiment.rs`.
fn create_custom_pipeline(
    device: &ash::Device,
    vertex_spv: &[u8],
    fragment_spv: &[u8],
    layout: vk::PipelineLayout,
    color_format: vk::Format,
    stencil_format: vk::Format,
) -> vk::Pipeline {
    let vertex_module = create_shader_module(device, vertex_spv);
    let fragment_module = create_shader_module(device, fragment_spv);
    let entry_point = c"main";
    let stages = [
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vertex_module)
            .name(entry_point),
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(fragment_module)
            .name(entry_point),
    ];

    let bindings = [vk::VertexInputBindingDescription::default()
        .binding(0)
        .stride(std::mem::size_of::<UiVertex>() as u32)
        .input_rate(vk::VertexInputRate::VERTEX)];
    let attributes = [
        vk::VertexInputAttributeDescription::default()
            .location(0)
            .binding(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(0),
        vk::VertexInputAttributeDescription::default()
            .location(1)
            .binding(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(8),
        vk::VertexInputAttributeDescription::default()
            .location(2)
            .binding(0)
            .format(vk::Format::R8G8B8A8_UNORM)
            .offset(16),
        vk::VertexInputAttributeDescription::default()
            .location(3)
            .binding(0)
            .format(vk::Format::R32G32B32_SFLOAT)
            .offset(20),
    ];
    let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
        .vertex_binding_descriptions(&bindings)
        .vertex_attribute_descriptions(&attributes);

    let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
    let viewport_state = vk::PipelineViewportStateCreateInfo::default()
        .viewport_count(1)
        .scissor_count(1);
    let rasterization = vk::PipelineRasterizationStateCreateInfo::default()
        .polygon_mode(vk::PolygonMode::FILL)
        .cull_mode(vk::CullModeFlags::NONE)
        .line_width(1.0);
    let multisample = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1);
    let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
        .depth_test_enable(false)
        .depth_write_enable(false);

    let color_blend_attachment = vk::PipelineColorBlendAttachmentState::default()
        .blend_enable(true)
        .src_color_blend_factor(vk::BlendFactor::ONE)
        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(vk::BlendFactor::ONE)
        .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .alpha_blend_op(vk::BlendOp::ADD)
        .color_write_mask(vk::ColorComponentFlags::RGBA);
    let attachments = [color_blend_attachment];
    let color_blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&attachments);

    let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dynamic_state =
        vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

    let color_formats = [color_format];
    let mut rendering_info = vk::PipelineRenderingCreateInfo::default()
        .color_attachment_formats(&color_formats)
        .stencil_attachment_format(stencil_format);

    let create_info = vk::GraphicsPipelineCreateInfo::default()
        .stages(&stages)
        .vertex_input_state(&vertex_input)
        .input_assembly_state(&input_assembly)
        .viewport_state(&viewport_state)
        .rasterization_state(&rasterization)
        .multisample_state(&multisample)
        .depth_stencil_state(&depth_stencil)
        .color_blend_state(&color_blend)
        .dynamic_state(&dynamic_state)
        .layout(layout)
        .push_next(&mut rendering_info);

    let pipeline = unsafe {
        device.create_graphics_pipelines(vk::PipelineCache::null(), &[create_info], None)
    }
    .expect("failed to create graphics pipeline")[0];

    unsafe {
        device.destroy_shader_module(vertex_module, None);
        device.destroy_shader_module(fragment_module, None);
    }
    pipeline
}

/// Points `set`'s own binding 0 at `view`, ready to sample -- the
/// non-bindless equivalent of `RhiDevice::register_bindless`, proven by
/// `dual_kawase_nonbindless_experiment.rs`.
fn point_descriptor_at(
    device: &ash::Device,
    set: vk::DescriptorSet,
    view: vk::ImageView,
    sampler: vk::Sampler,
) {
    let image_info = vk::DescriptorImageInfo::default()
        .image_view(view)
        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
        .sampler(sampler);
    unsafe {
        device.update_descriptor_sets(
            &[vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(std::slice::from_ref(&image_info))],
            &[],
        );
    }
}

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre dual-kawase blur probe (never shown)", 1, 1)
        .expect("failed to open probe window");
    use raw_window_handle::HasDisplayHandle;
    let display_handle = probe_connection.display_handle().unwrap().as_raw();
    let window_handle = probe_connection
        .window_handle(probe_window)
        .unwrap()
        .as_raw();
    let (device, surface_loader, surface) =
        VulkanDevice::new(display_handle, window_handle).expect("failed to create VulkanDevice");
    unsafe {
        surface_loader.destroy_surface(surface, None);
    }

    let swapchain = HeadlessSwapchain::new(&device, SWAPCHAIN_WIDTH, SWAPCHAIN_HEIGHT)
        .expect("failed to create HeadlessSwapchain");

    // --- The real fix: a plain, conventional descriptor set layout (a
    // single COMBINED_IMAGE_SAMPLER), NOT the persistent bindless array
    // -- proven by dual_kawase_nonbindless_experiment.rs. One set per
    // real hop (5 total), allocated up front. ---
    let descriptor_set_layout_bindings = [vk::DescriptorSetLayoutBinding::default()
        .binding(0)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .descriptor_count(1)
        .stage_flags(vk::ShaderStageFlags::FRAGMENT)];
    let descriptor_set_layout = unsafe {
        device.device.create_descriptor_set_layout(
            &vk::DescriptorSetLayoutCreateInfo::default().bindings(&descriptor_set_layout_bindings),
            None,
        )
    }
    .expect("failed to create non-bindless descriptor set layout");

    const HOP_COUNT: u32 = 5; // L0, L1, L2, U1, U0
    let pool_sizes = [vk::DescriptorPoolSize::default()
        .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .descriptor_count(HOP_COUNT)];
    let descriptor_pool = unsafe {
        device.device.create_descriptor_pool(
            &vk::DescriptorPoolCreateInfo::default()
                .pool_sizes(&pool_sizes)
                .max_sets(HOP_COUNT),
            None,
        )
    }
    .expect("failed to create descriptor pool");
    let set_layouts = vec![descriptor_set_layout; HOP_COUNT as usize];
    let descriptor_sets = unsafe {
        device.device.allocate_descriptor_sets(
            &vk::DescriptorSetAllocateInfo::default()
                .descriptor_pool(descriptor_pool)
                .set_layouts(&set_layouts),
        )
    }
    .expect("failed to allocate descriptor sets");
    let (set_l0, set_l1, set_l2, set_u1, set_u0) = (
        descriptor_sets[0],
        descriptor_sets[1],
        descriptor_sets[2],
        descriptor_sets[3],
        descriptor_sets[4],
    );

    // Same real sampler parameters as the engine's own bindless_sampler.
    let sampler = unsafe {
        device.device.create_sampler(
            &vk::SamplerCreateInfo::default()
                .mag_filter(vk::Filter::LINEAR)
                .min_filter(vk::Filter::LINEAR)
                .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE),
            None,
        )
    }
    .expect("failed to create sampler");

    let pipeline_layout = unsafe {
        device.device.create_pipeline_layout(
            &vk::PipelineLayoutCreateInfo::default()
                .set_layouts(&[descriptor_set_layout])
                .push_constant_ranges(&[vk::PushConstantRange::default()
                    .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                    .offset(0)
                    .size(12)]),
            None,
        )
    }
    .expect("failed to create pipeline layout");

    // --- Four pipelines: the square (into L0, real unmodified bindless
    // pipeline), the downsample/upsample filters (every intermediate
    // level, all Rgba16Float, now via the plain descriptor layout above),
    // and the final composite (onto the swapchain's own HEADLESS_FORMAT,
    // also via the plain descriptor layout). ---
    let out_dir = env!("OUT_DIR");
    let read_spv = |name: &str| {
        std::fs::read(format!("{out_dir}/{name}.spv"))
            .unwrap_or_else(|e| panic!("failed to read compiled shader {name}: {e}"))
    };

    let rect_pipeline = device
        .create_pipeline(
            &read_spv("sdf_rounded_rect.vert"),
            &read_spv("sdf_rounded_rect.frag"),
            vk::Format::R16G16B16A16_SFLOAT,
        )
        .expect("failed to create rect pipeline");
    let downsample_pipeline = create_custom_pipeline(
        &device.device,
        &read_spv("bindless_textured.vert"),
        &read_spv("kawase_downsample_nonbindless.frag"),
        pipeline_layout,
        vk::Format::R16G16B16A16_SFLOAT,
        device.stencil_format,
    );
    let upsample_pipeline = create_custom_pipeline(
        &device.device,
        &read_spv("bindless_textured.vert"),
        &read_spv("kawase_upsample_nonbindless.frag"),
        pipeline_layout,
        vk::Format::R16G16B16A16_SFLOAT,
        device.stencil_format,
    );
    let composite_pipeline = create_custom_pipeline(
        &device.device,
        &read_spv("bindless_textured.vert"),
        &read_spv("passthrough_nonbindless.frag"),
        pipeline_layout,
        tre_rhi_vulkan::HEADLESS_FORMAT,
        device.stencil_format,
    );

    // --- The square's own geometry, via RenderingCanvas purely as a
    // convenient vertex-data builder -- the same non-Canvas-flow use
    // `render_to_texture_demo.rs` already established. ---
    let white = rgba8(255, 255, 255, 255);
    let mut square_canvas = RenderingCanvas::new();
    square_canvas.draw_rounded_rect(SQUARE_X, SQUARE_Y, SQUARE_SIZE, SQUARE_SIZE, 0.0, white);
    let square_frame = square_canvas.flatten();
    let square_vertex_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&square_frame.vertices),
            vk::BufferUsageFlags::VERTEX_BUFFER,
        )
        .expect("failed to upload square vertex buffer");
    let square_index_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&square_frame.indices),
            vk::BufferUsageFlags::INDEX_BUFFER,
        )
        .expect("failed to upload square index buffer");

    // --- One full-target quad buffer per distinct real size in the
    // chain -- reused across every pass that renders into a target of
    // that same size, regardless of which pipeline/texture is active. ---
    let upload_quad = |width: u32, height: u32| {
        let quad = textured_quad(0.0, 0.0, width as f32, height as f32);
        let indices: [u32; 6] = [0, 1, 2, 2, 3, 0];
        let vertex_buffer = device
            .upload_buffer(
                bytemuck::cast_slice(&quad),
                vk::BufferUsageFlags::VERTEX_BUFFER,
            )
            .expect("failed to upload quad vertex buffer");
        let index_buffer = device
            .upload_buffer(
                bytemuck::cast_slice(&indices),
                vk::BufferUsageFlags::INDEX_BUFFER,
            )
            .expect("failed to upload quad index buffer");
        (vertex_buffer, index_buffer)
    };
    let (full_quad_vb, full_quad_ib) = upload_quad(SIZE_FULL.0, SIZE_FULL.1);
    let (half_quad_vb, half_quad_ib) = upload_quad(SIZE_HALF.0, SIZE_HALF.1);
    let (quarter_quad_vb, quarter_quad_ib) = upload_quad(SIZE_QUARTER.0, SIZE_QUARTER.1);

    // --- The real chain. ---
    submit_frame(&device, &swapchain, |cmd_buffer| {
        let raw_cmd_buffer = vk::CommandBuffer::from_raw(cmd_buffer.raw_handle());

        // Raw-binds a non-bindless pipeline + its one descriptor set + this
        // pass's own `screen_size` push constant, then issues the draw call
        // itself via a raw `cmd_draw_indexed` -- never `RhiCommandBuffer::
        // draw_indexed`. That wrapper method unconditionally performs its own
        // *second* `cmd_push_constants` call using `self.width`/`self.height`
        // (this render target's own real dimensions, per whatever `begin_
        // render_to_texture[_no_end]` last set them to), which silently
        // clobbers the correct `size` just pushed below. This isn't
        // theoretical: `acquire_transient_target`'s own documented "oversized
        // borrow" fallback (`RhiDevice::acquire_transient_target`'s doc
        // comment) can and does hand back a differently-sized (larger)
        // texture than requested whenever no free bucket of the exact
        // requested size exists yet but a larger one does -- exactly the case
        // here the first time L2/U1/U0's own bucket sizes are ever requested,
        // after L0/L1 have already been released. When that happens, `self.
        // width`/`self.height` reflect the REAL (oversized) size, not this
        // hop's intended one, and the wrapper's redundant push would silently
        // substitute the wrong `screen_size` into the vertex shader's NDC
        // mapping right before the draw -- confining the actual draw to a
        // small corner of the oversized backing image while every later hop
        // (and the final composite) keeps sampling/rendering at the real,
        // larger extent, reading back untouched clear-color everywhere except
        // that corner. This raw draw path sidesteps the whole hazard: the
        // manually-pushed `screen_size` above is the only one ever pushed for
        // these draws, and downstream UV math is scale-invariant regardless
        // of the backing image's real physical size.
        let draw_nonbindless_pass = |pipeline: vk::Pipeline,
                                     set: vk::DescriptorSet,
                                     size: (u32, u32),
                                     vertex_buffer: &dyn RhiBuffer,
                                     index_buffer: &dyn RhiBuffer,
                                     index_count: u32| {
            let push_constants: [f32; 2] = [size.0 as f32, size.1 as f32];
            let vb_raw = vk::Buffer::from_raw(vertex_buffer.raw_handle());
            let ib_raw = vk::Buffer::from_raw(index_buffer.raw_handle());
            unsafe {
                device.device.cmd_bind_pipeline(
                    raw_cmd_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipeline,
                );
                device.device.cmd_bind_descriptor_sets(
                    raw_cmd_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipeline_layout,
                    0,
                    &[set],
                    &[],
                );
                device.device.cmd_push_constants(
                    raw_cmd_buffer,
                    pipeline_layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    bytemuck::cast_slice(&push_constants),
                );
                device
                    .device
                    .cmd_bind_vertex_buffers(raw_cmd_buffer, 0, &[vb_raw], &[0]);
                device.device.cmd_bind_index_buffer(
                    raw_cmd_buffer,
                    ib_raw,
                    0,
                    vk::IndexType::UINT32,
                );
                device
                    .device
                    .cmd_draw_indexed(raw_cmd_buffer, index_count, 1, 0, 0, 0);
            }
        };

        // L0: the square, rendered into a full-size transient target.
        let l0 = device
            .acquire_transient_target(SIZE_FULL.0, SIZE_FULL.1, TextureFormat::Rgba16Float)
            .expect("failed to acquire L0");
        cmd_buffer.begin_render_to_texture(&*l0, SIZE_FULL.0, SIZE_FULL.1);
        cmd_buffer.set_pipeline(&rect_pipeline);
        cmd_buffer.bind_vertex_buffer(&square_vertex_buffer, 0);
        cmd_buffer.bind_index_buffer(&square_index_buffer, 0);
        cmd_buffer.draw_indexed(square_frame.indices.len() as u32, 0, 0);

        // L0 -> L1: downsample to half size, via the plain descriptor set --
        // `end_render_to_texture` transitions L0 to SHADER_READ_ONLY_OPTIMAL
        // with the real, already-proven barrier; `point_descriptor_at` just
        // points a conventional descriptor at that same real view instead of
        // registering it into the bindless array.
        cmd_buffer.end_render_to_texture(&*l0);
        point_descriptor_at(
            &device.device,
            set_l0,
            vk::ImageView::from_raw(l0.raw_handle()),
            sampler,
        );
        let l1 = device
            .acquire_transient_target(SIZE_HALF.0, SIZE_HALF.1, TextureFormat::Rgba16Float)
            .expect("failed to acquire L1");
        cmd_buffer.begin_render_to_texture_no_end(&*l1, SIZE_HALF.0, SIZE_HALF.1);
        draw_nonbindless_pass(
            downsample_pipeline,
            set_l0,
            SIZE_HALF,
            &half_quad_vb,
            &half_quad_ib,
            6,
        );
        device.release_transient_target(l0);

        // L1 -> L2: downsample to quarter size.
        cmd_buffer.end_render_to_texture(&*l1);
        point_descriptor_at(
            &device.device,
            set_l1,
            vk::ImageView::from_raw(l1.raw_handle()),
            sampler,
        );
        let l2 = device
            .acquire_transient_target(SIZE_QUARTER.0, SIZE_QUARTER.1, TextureFormat::Rgba16Float)
            .expect("failed to acquire L2");
        cmd_buffer.begin_render_to_texture_no_end(&*l2, SIZE_QUARTER.0, SIZE_QUARTER.1);
        draw_nonbindless_pass(
            downsample_pipeline,
            set_l1,
            SIZE_QUARTER,
            &quarter_quad_vb,
            &quarter_quad_ib,
            6,
        );
        device.release_transient_target(l1);

        // L2 -> U1: upsample back to half size.
        cmd_buffer.end_render_to_texture(&*l2);
        point_descriptor_at(
            &device.device,
            set_l2,
            vk::ImageView::from_raw(l2.raw_handle()),
            sampler,
        );
        let u1 = device
            .acquire_transient_target(SIZE_HALF.0, SIZE_HALF.1, TextureFormat::Rgba16Float)
            .expect("failed to acquire U1");
        cmd_buffer.begin_render_to_texture_no_end(&*u1, SIZE_HALF.0, SIZE_HALF.1);
        draw_nonbindless_pass(
            upsample_pipeline,
            set_l2,
            SIZE_HALF,
            &half_quad_vb,
            &half_quad_ib,
            6,
        );
        device.release_transient_target(l2);

        // U1 -> U0: upsample back to full size.
        cmd_buffer.end_render_to_texture(&*u1);
        point_descriptor_at(
            &device.device,
            set_u1,
            vk::ImageView::from_raw(u1.raw_handle()),
            sampler,
        );
        let u0 = device
            .acquire_transient_target(SIZE_FULL.0, SIZE_FULL.1, TextureFormat::Rgba16Float)
            .expect("failed to acquire U0");
        cmd_buffer.begin_render_to_texture_no_end(&*u0, SIZE_FULL.0, SIZE_FULL.1);
        draw_nonbindless_pass(
            upsample_pipeline,
            set_u1,
            SIZE_FULL,
            &full_quad_vb,
            &full_quad_ib,
            6,
        );
        device.release_transient_target(u1);

        // U0 is the chain's final texture -- what follows is `resume_
        // swapchain_rendering`, which never calls `cmd_end_rendering` itself
        // (Step 6.4.1's own original single-level pairing, unchanged).
        cmd_buffer.end_render_to_texture(&*u0);
        point_descriptor_at(
            &device.device,
            set_u0,
            vk::ImageView::from_raw(u0.raw_handle()),
            sampler,
        );

        // Composite the final blurred result back onto the swapchain --
        // `passthrough_nonbindless.frag` reads no push constants, but this
        // still goes through the same raw draw path as every other
        // non-bindless pass for consistency (its push is simply unused).
        cmd_buffer.resume_swapchain_rendering();
        draw_nonbindless_pass(
            composite_pipeline,
            set_u0,
            SIZE_FULL,
            &full_quad_vb,
            &full_quad_ib,
            6,
        );
        device.release_transient_target(u0);
    })
    .expect("submit_frame failed");

    let bgra = swapchain
        .read_pixels_bgra8()
        .expect("failed to read back pixels");
    let pixel_at =
        |x: u32, y: u32| -> [u8; 4] { pixel_helpers::bgra_pixel_at(&bgra, SWAPCHAIN_WIDTH, x, y) };

    let background = pixel_at(5, 5);
    eprintln!("background (clear color): {background:?}");

    // Deep in the square's own center: coverage is 1.0 pre-blur, and the
    // blur at this depth must not wash it out to uniform gray.
    let center = (
        (SQUARE_X + SQUARE_SIZE / 2.0) as u32,
        (SQUARE_Y + SQUARE_SIZE / 2.0) as u32,
    );
    let interior = pixel_at(center.0, center.1);
    assert!(
        interior[0] > 150 && interior[1] > 150 && interior[2] > 150,
        "the square's own deep interior must stay mostly foreground after blur, not be washed \
         to uniform gray -- got {interior:?}"
    );
    eprintln!("bounded interior: OK (still mostly foreground, {interior:?})");

    // Just outside the square's own original hard left edge -- pure
    // background before any blur -- must now show a genuine partial
    // blend, proving real blur spread outward.
    let edge_x = (SQUARE_X - 6.0) as u32;
    let bled = pixel_at(edge_x, center.1);
    assert_ne!(
        bled, background,
        "a point just outside the square's own original boundary must show real blur bleed, \
         not pure background -- got {bled:?}"
    );
    assert!(
        bled[0] < 250,
        "a point just outside the square's own original boundary must be a genuine partial \
         blend, not pure foreground either -- got {bled:?}"
    );
    eprintln!("edge bleed: OK (real partial blend outside the original hard edge, {bled:?})");

    let mut rgba = bgra.clone();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_DUAL_KAWASE_BLUR_OUTPUT")
        .unwrap_or_else(|_| "dual_kawase_blur_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(file),
        SWAPCHAIN_WIDTH,
        SWAPCHAIN_HEIGHT,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba)
        .expect("failed to write PNG image data");

    eprintln!("wrote {SWAPCHAIN_WIDTH}x{SWAPCHAIN_HEIGHT} dual-kawase blur render to {out_path}");
    eprintln!("all dual-kawase blur assertions passed");

    // Real cleanup, matching every other demo's own discipline -- wait
    // for the GPU to finish the one submitted frame, then destroy every
    // hand-rolled object this file created directly (no RHI wrapper type
    // of their own to Drop them).
    unsafe {
        device
            .device
            .device_wait_idle()
            .expect("device_wait_idle failed");
        device.device.destroy_pipeline(downsample_pipeline, None);
        device.device.destroy_pipeline(upsample_pipeline, None);
        device.device.destroy_pipeline(composite_pipeline, None);
        device.device.destroy_pipeline_layout(pipeline_layout, None);
        device.device.destroy_sampler(sampler, None);
        device.device.destroy_descriptor_pool(descriptor_pool, None);
        device
            .device
            .destroy_descriptor_set_layout(descriptor_set_layout, None);
    }
}
