//! `RhiCommandBuffer::apply_layer_blur`'s Dual-Kawase blur subsystem
//! (IMPLEMENTATION.md Step 7.2.2) -- one self-contained algorithm's
//! worth of setup (`create_blur_resources` and its own private helpers)
//! plus the small POD types (`PushConstants`/`BlurResources`) both
//! `VulkanDevice` and `VulkanCommandBuffer` need a copy of. Split out of
//! `lib.rs` as one of its ten separable concerns (Architecture review
//! finding) -- this is also the one subsystem the Security review found
//! missing its `// SAFETY:` comments, since fixed.

use ash::vk;
use tre_engine::UiVertex;

/// The push-constant layout `create_pipeline`'s universal pipeline layout
/// declares (IMPLEMENTATION.md Step 2.1): 12 bytes total, no padding
/// (`[f32; 2]` then `u32`, both 4-byte aligned).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct PushConstants {
    pub(crate) screen_size: [f32; 2],
    pub(crate) texture_index: u32,
}

/// `RhiCommandBuffer::apply_layer_blur`'s own real, non-bindless Dual-
/// Kawase machinery (IMPLEMENTATION.md Step 7.2.2), graduated from
/// `dual_kawase_blur_demo.rs`'s own proven design (REVIEW.md finding
/// #130's real fix) into real, reusable engine capability -- lazily
/// created once (`VulkanDevice::blur_resources`), then reused for every
/// call this process ever makes. One plain `COMBINED_IMAGE_SAMPLER`
/// descriptor set (repointed at each hop's own source view via
/// `vkUpdateDescriptorSets` before that hop's draw, matching
/// `point_descriptor_at`'s own proven pattern) -- one set suffices since
/// a chain only ever needs "the set for the current source" at any one
/// moment, never several alive at once. `unit_quad_vertex_buffer`/
/// `unit_quad_index_buffer` are a single, fixed, NDC-authored quad
/// (`-1..1`, paired with `fullscreen_quad_nonbindless.vert`'s own
/// dedicated vertex shader) reused unchanged for every hop of every
/// call regardless of the caller's real width/height -- no per-call
/// vertex-buffer upload, keeping this whole operation free of dynamic
/// RHI allocation inside the render tick (DESIGN.md Section 2.6) beyond
/// its own one-time setup cost.
#[derive(Clone, Copy)]
pub(crate) struct BlurResources {
    pub(crate) descriptor_set_layout: vk::DescriptorSetLayout,
    pub(crate) descriptor_pool: vk::DescriptorPool,
    /// One descriptor set per real hop (4: L0-read, L1-read, L2-read,
    /// U1-read), matching `dual_kawase_blur_demo.rs`'s own proven
    /// design exactly -- each is `vkUpdateDescriptorSets`-written exactly
    /// once, before its own first use, then never touched again.
    /// Reusing a *single* set across hops (updating it between binds)
    /// was tried first and rejected: this descriptor set layout has no
    /// `UPDATE_AFTER_BIND` flag (deliberately -- matching the same,
    /// already-proven-sufficient plain layout the demo itself uses), so
    /// updating a set already bound to a still-recording command buffer
    /// is invalid per the Vulkan spec, caught immediately by validation
    /// (`vkCmdPipelineBarrier(): ... invalid state ... VkDescriptorSet
    /// ... was destroyed or updated without UPDATE_AFTER_BIND`) the
    /// first time this code actually ran.
    pub(crate) descriptor_sets: [vk::DescriptorSet; 4],
    pub(crate) sampler: vk::Sampler,
    pub(crate) pipeline_layout: vk::PipelineLayout,
    pub(crate) downsample_pipeline: vk::Pipeline,
    pub(crate) upsample_pipeline: vk::Pipeline,
    pub(crate) unit_quad_vertex_buffer: vk::Buffer,
    pub(crate) unit_quad_vertex_memory: vk::DeviceMemory,
    pub(crate) unit_quad_index_buffer: vk::Buffer,
    pub(crate) unit_quad_index_memory: vk::DeviceMemory,
}

fn create_blur_shader_module(device: &ash::Device, spv_bytes: &[u8]) -> vk::ShaderModule {
    let mut cursor = std::io::Cursor::new(spv_bytes);
    let code = ash::util::read_spv(&mut cursor).expect("invalid SPIR-V bytecode");
    let info = vk::ShaderModuleCreateInfo::default().code(&code);
    // SAFETY: `device` is the caller's own still-valid logical device;
    // `info` (and the `code` word buffer it borrows, just parsed as real
    // SPIR-V by `read_spv` above) is a local borrowed only for the
    // duration of this call.
    unsafe { device.create_shader_module(&info, None) }
        .expect("failed to create blur shader module")
}

/// Mirrors `dual_kawase_blur_demo.rs`'s own `create_custom_pipeline`
/// exactly (proven correct there across many real runs) -- vertex
/// input/blend/dynamic-viewport-scissor state identical to
/// `VulkanDevice::create_pipeline`'s own, only `layout` differs.
fn create_blur_pipeline(
    device: &ash::Device,
    vertex_spv: &[u8],
    fragment_spv: &[u8],
    layout: vk::PipelineLayout,
    color_format: vk::Format,
    stencil_format: vk::Format,
) -> vk::Pipeline {
    let vertex_module = create_blur_shader_module(device, vertex_spv);
    let fragment_module = create_blur_shader_module(device, fragment_spv);
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

    // SAFETY: `device` is the caller's own still-valid logical device;
    // `vertex_module`/`fragment_module` were both just created above from
    // this same device and are still valid; `create_info` (and every
    // create-info it chains -- `stages`/`vertex_input`/`rendering_info`/
    // etc.) is a local borrowed only for the duration of this call.
    let pipeline = unsafe {
        device.create_graphics_pipelines(vk::PipelineCache::null(), &[create_info], None)
    }
    .expect("failed to create blur graphics pipeline")[0];

    // SAFETY: `device` is still the same valid logical device; both
    // modules were created from it above and are safe to destroy now --
    // `create_graphics_pipelines` just consumed their contents into
    // `pipeline`, and the Vulkan spec explicitly permits destroying a
    // shader module immediately after it's been baked into a pipeline.
    unsafe {
        device.destroy_shader_module(vertex_module, None);
        device.destroy_shader_module(fragment_module, None);
    }
    pipeline
}

/// One-time setup for `RhiCommandBuffer::apply_layer_blur`
/// (IMPLEMENTATION.md Step 7.2.2) -- called at most once per process,
/// behind `VulkanDevice::blur_resources`'s own lazy-init check.
pub(crate) fn create_blur_resources(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: &ash::Device,
    stencil_format: vk::Format,
) -> BlurResources {
    let descriptor_set_layout_bindings = [vk::DescriptorSetLayoutBinding::default()
        .binding(0)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .descriptor_count(1)
        .stage_flags(vk::ShaderStageFlags::FRAGMENT)];
    // SAFETY: `device` is the caller's own still-valid logical device;
    // the create-info (and the `descriptor_set_layout_bindings` slice it
    // borrows) is a local valid only for the duration of this call.
    let descriptor_set_layout = unsafe {
        device.create_descriptor_set_layout(
            &vk::DescriptorSetLayoutCreateInfo::default().bindings(&descriptor_set_layout_bindings),
            None,
        )
    }
    .expect("failed to create blur descriptor set layout");

    const HOP_COUNT: u32 = 4; // L0-read, L1-read, L2-read, U1-read
    let pool_sizes = [vk::DescriptorPoolSize::default()
        .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .descriptor_count(HOP_COUNT)];
    // SAFETY: `device` is still the same valid logical device; the
    // create-info (and the `pool_sizes` slice it borrows) is a local
    // valid only for the duration of this call.
    let descriptor_pool = unsafe {
        device.create_descriptor_pool(
            &vk::DescriptorPoolCreateInfo::default()
                .pool_sizes(&pool_sizes)
                .max_sets(HOP_COUNT),
            None,
        )
    }
    .expect("failed to create blur descriptor pool");
    let set_layouts = vec![descriptor_set_layout; HOP_COUNT as usize];
    // SAFETY: `device` is still valid; `descriptor_pool` was just created
    // above from this same device and is still valid; `set_layouts`
    // (every entry a copy of the `descriptor_set_layout` also created
    // above and still valid) is a local borrowed only for this call.
    let allocated_sets = unsafe {
        device.allocate_descriptor_sets(
            &vk::DescriptorSetAllocateInfo::default()
                .descriptor_pool(descriptor_pool)
                .set_layouts(&set_layouts),
        )
    }
    .expect("failed to allocate blur descriptor sets");
    let descriptor_sets: [vk::DescriptorSet; 4] = allocated_sets
        .try_into()
        .expect("allocate_descriptor_sets returned HOP_COUNT sets");

    // SAFETY: `device` is still the same valid logical device; the
    // create-info is a local valid only for the duration of this call.
    let sampler = unsafe {
        device.create_sampler(
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
    .expect("failed to create blur sampler");

    // SAFETY: `device` is still valid; `descriptor_set_layout` was
    // created above from this same device and is still valid; the
    // create-info (and the push-constant-range slice it borrows) is a
    // local valid only for the duration of this call.
    let pipeline_layout = unsafe {
        device.create_pipeline_layout(
            &vk::PipelineLayoutCreateInfo::default()
                .set_layouts(std::slice::from_ref(&descriptor_set_layout))
                .push_constant_ranges(&[vk::PushConstantRange::default()
                    .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                    .offset(0)
                    .size(12)]),
            None,
        )
    }
    .expect("failed to create blur pipeline layout");

    let vertex_spv = include_bytes!(concat!(
        env!("OUT_DIR"),
        "/fullscreen_quad_nonbindless.vert.spv"
    ));
    let downsample_spv = include_bytes!(concat!(
        env!("OUT_DIR"),
        "/kawase_downsample_nonbindless.frag.spv"
    ));
    let upsample_spv = include_bytes!(concat!(
        env!("OUT_DIR"),
        "/kawase_upsample_nonbindless.frag.spv"
    ));
    let downsample_pipeline = create_blur_pipeline(
        device,
        vertex_spv,
        downsample_spv,
        pipeline_layout,
        vk::Format::R16G16B16A16_SFLOAT,
        stencil_format,
    );
    let upsample_pipeline = create_blur_pipeline(
        device,
        vertex_spv,
        upsample_spv,
        pipeline_layout,
        vk::Format::R16G16B16A16_SFLOAT,
        stencil_format,
    );

    // A single, fixed, NDC-authored (-1..1) quad -- see this struct's
    // own doc comment for why this needs no per-call/per-hop variant.
    let white = 0xFFFF_FFFFu32;
    let quad_vertices = [
        UiVertex {
            position: [-1.0, -1.0],
            uv: [0.0, 0.0],
            color: white,
            params: [0.0; 3],
        },
        UiVertex {
            position: [1.0, -1.0],
            uv: [1.0, 0.0],
            color: white,
            params: [0.0; 3],
        },
        UiVertex {
            position: [1.0, 1.0],
            uv: [1.0, 1.0],
            color: white,
            params: [0.0; 3],
        },
        UiVertex {
            position: [-1.0, 1.0],
            uv: [0.0, 1.0],
            color: white,
            params: [0.0; 3],
        },
    ];
    let quad_indices: [u32; 6] = [0, 1, 2, 0, 2, 3];
    // Mirrors `VulkanDevice::upload_buffer`'s own exact allocation
    // pattern for a small, one-time, host-visible buffer -- that method
    // is only reachable through a concrete `&VulkanDevice`, not the
    // `&dyn RhiDevice` trait object `apply_layer_blur`'s own signature
    // is given, so this crate's own memory-type-selection logic is
    // duplicated here rather than reused directly.
    let (unit_quad_vertex_buffer, unit_quad_vertex_memory) = create_blur_static_buffer(
        instance,
        physical_device,
        device,
        bytemuck::cast_slice(&quad_vertices),
        vk::BufferUsageFlags::VERTEX_BUFFER,
    );
    let (unit_quad_index_buffer, unit_quad_index_memory) = create_blur_static_buffer(
        instance,
        physical_device,
        device,
        bytemuck::cast_slice(&quad_indices),
        vk::BufferUsageFlags::INDEX_BUFFER,
    );

    BlurResources {
        descriptor_set_layout,
        descriptor_pool,
        descriptor_sets,
        sampler,
        pipeline_layout,
        downsample_pipeline,
        upsample_pipeline,
        unit_quad_vertex_buffer,
        unit_quad_vertex_memory,
        unit_quad_index_buffer,
        unit_quad_index_memory,
    }
}

/// Mirrors `VulkanDevice::upload_buffer`'s own memory-type-selection and
/// host-visible-map-and-copy logic exactly, for the one case that method
/// itself is unreachable from (`create_blur_resources`'s own callers
/// only have `&ash::Instance`/`vk::PhysicalDevice`/`&ash::Device`
/// individually, never a concrete `&VulkanDevice`). Returns the raw
/// buffer and its backing memory, both owned by the caller from here on
/// -- `create_blur_resources`'s own result never wraps these in a
/// `VulkanBuffer` (whose `Drop` would destroy them the moment it went
/// out of scope; these are meant to outlive that scope, cached in
/// `BlurResources` for the rest of the process).
fn create_blur_static_buffer(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: &ash::Device,
    bytes: &[u8],
    usage: vk::BufferUsageFlags,
) -> (vk::Buffer, vk::DeviceMemory) {
    // SAFETY: `device` is a valid, live logical device; `bytes.len()` is
    // used directly as `size`, matching `VulkanDevice::upload_buffer`'s
    // own reasoning exactly.
    let buffer = unsafe {
        device.create_buffer(
            &vk::BufferCreateInfo::default()
                .size(bytes.len() as u64)
                .usage(usage)
                .sharing_mode(vk::SharingMode::EXCLUSIVE),
            None,
        )
    }
    .expect("failed to create blur static buffer");

    // SAFETY: `buffer` was just created above on this device.
    let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
    // SAFETY: `physical_device` is the device `VulkanDevice::new` itself
    // selected, valid for as long as `instance` (also alive here) is.
    let memory_properties =
        unsafe { instance.get_physical_device_memory_properties(physical_device) };
    let wanted = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
    let memory_type_index = (0..memory_properties.memory_type_count)
        .find(|&i| {
            (requirements.memory_type_bits & (1 << i)) != 0
                && memory_properties.memory_types[i as usize]
                    .property_flags
                    .contains(wanted)
        })
        .expect("no host-visible/host-coherent memory type available for blur static buffer");

    // SAFETY: `device` is valid, `requirements.size` comes directly from
    // `get_buffer_memory_requirements` above, and `memory_type_index` was
    // selected from the `find` above so it is one of the bits set in
    // `requirements.memory_type_bits`.
    let memory = unsafe {
        device.allocate_memory(
            &vk::MemoryAllocateInfo::default()
                .allocation_size(requirements.size)
                .memory_type_index(memory_type_index),
            None,
        )
    }
    .expect("failed to allocate blur static buffer memory");

    // SAFETY: `buffer`/`memory` were both just created above on this
    // device, `buffer` has not been bound to memory before now, and
    // `memory` was allocated host-visible/host-coherent (selected via
    // `wanted` above), so mapping it is valid; `dst` is writable for at
    // least `bytes.len()` bytes, matching `copy_nonoverlapping`'s write,
    // and `unmap_memory` is called exactly once right after.
    unsafe {
        device
            .bind_buffer_memory(buffer, memory, 0)
            .expect("failed to bind blur static buffer memory");
        let dst = device
            .map_memory(memory, 0, bytes.len() as u64, vk::MemoryMapFlags::empty())
            .expect("failed to map blur static buffer memory");
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst.cast::<u8>(), bytes.len());
        device.unmap_memory(memory);
    }

    (buffer, memory)
}
