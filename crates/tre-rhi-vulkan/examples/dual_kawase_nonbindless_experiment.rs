//! REVIEW.md finding #130, second research session: an isolated
//! experiment testing whether the Dual-Kawase blur's all-zero-read bug
//! is specific to sampling through the persistent bindless texture
//! array, by replacing exactly the downsample hop (L0 -> L1) and the
//! final composite with a single, plain, conventionally-bound
//! `sampler2D` -- bypassing `bindless_textures[]`/`register_bindless`
//! entirely -- instead of `dual_kawase_blur_demo.rs`'s own bindless
//! path. Everything else (render-to-texture lifecycle, real barriers,
//! transient target acquisition) is unchanged, already-proven RHI
//! machinery; only the one variable under test differs.
//!
//! Motivation: most real-time blur implementations outside this engine
//! (game-engine post-process chains in particular) bind a just-rendered
//! offscreen target as a plain, dedicated sampler for that one pass,
//! rather than through a persistent, `UPDATE_AFTER_BIND` bindless array
//! shared with every other texture in the engine.
//!
//! **Update (2026-09-08): this experiment's own read genuinely worked,
//! but not for the reason originally assumed.** Converting the real
//! `dual_kawase_blur_demo.rs` to this same non-bindless approach still
//! failed past its second hop -- a shape this single-hop experiment
//! never exercised. The real defect (`dual_kawase_blur_demo.rs`'s own
//! updated header has the full account) is in `RhiCommandBuffer::
//! draw_indexed` silently clobbering a manually-pushed `screen_size`
//! push constant with the render target's own real dimensions, which
//! only diverge from the intended per-hop size when `acquire_
//! transient_target`'s "oversized borrow" fallback hands back a larger
//! texture than requested. This experiment's own two non-bindless draws
//! (below) still call the wrapper's `draw_indexed`, carrying the exact
//! same latent hazard -- it simply never triggers here, because this
//! file's single-hop, single-hand-off shape never acquires a smaller
//! bucket size after releasing a larger one first. Bindless vs.
//! non-bindless was never actually the variable that mattered; this
//! experiment's success was real, but the isolation it seemed to prove
//! was not.
//!
//! Hand-rolled Vulkan throughout (raw `ash::Device` access via
//! `VulkanDevice::device`, `RhiCommandBuffer::raw_handle()` to recover
//! the real `vk::CommandBuffer`) -- necessary because `RhiCommandBuffer::
//! set_pipeline` always rebinds the bindless descriptor set
//! unconditionally, which would be wrong for this experiment's own,
//! deliberately different pipeline layout. `RhiCommandBuffer::begin_
//! render_to_texture`/`end_render_to_texture`/`begin_render_to_texture_
//! no_end`/`resume_swapchain_rendering` are still used unchanged for the
//! render-target lifecycle -- those never touch descriptor sets.

use ash::vk;
use ash::vk::Handle;
use tre_engine::{rgba8, submit_frame, RenderingCanvas, RhiDevice, TextureFormat, UiVertex};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const SWAPCHAIN_WIDTH: u32 = 128;
const SWAPCHAIN_HEIGHT: u32 = 64;
const SIZE_FULL: (u32, u32) = (SWAPCHAIN_WIDTH, SWAPCHAIN_HEIGHT);
const SIZE_HALF: (u32, u32) = (SWAPCHAIN_WIDTH / 2, SWAPCHAIN_HEIGHT / 2);

const SQUARE_SIZE: f32 = 24.0;
const SQUARE_X: f32 = (SWAPCHAIN_WIDTH as f32 - SQUARE_SIZE) / 2.0;
const SQUARE_Y: f32 = (SWAPCHAIN_HEIGHT as f32 - SQUARE_SIZE) / 2.0;

fn textured_quad(width: f32, height: f32) -> [UiVertex; 4] {
    let white = rgba8(255, 255, 255, 255);
    [
        UiVertex {
            position: [0.0, 0.0],
            uv: [0.0, 0.0],
            color: white,
            params: [0.0; 3],
        },
        UiVertex {
            position: [width, 0.0],
            uv: [1.0, 0.0],
            color: white,
            params: [0.0; 3],
        },
        UiVertex {
            position: [width, height],
            uv: [1.0, 1.0],
            color: white,
            params: [0.0; 3],
        },
        UiVertex {
            position: [0.0, height],
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
/// instead of always getting the universal bindless one.
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

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre non-bindless kawase experiment (never shown)", 1, 1)
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

    // --- The one variable under test: a plain, conventional descriptor
    // set layout (a single COMBINED_IMAGE_SAMPLER), NOT the persistent
    // bindless array. No UPDATE_AFTER_BIND flags at all -- this is the
    // ordinary, textbook binding shape most engines use for a
    // post-process input. ---
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

    let pool_sizes = [vk::DescriptorPoolSize::default()
        .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .descriptor_count(2)];
    let descriptor_pool = unsafe {
        device.device.create_descriptor_pool(
            &vk::DescriptorPoolCreateInfo::default()
                .pool_sizes(&pool_sizes)
                .max_sets(2),
            None,
        )
    }
    .expect("failed to create descriptor pool");
    let set_layouts = [descriptor_set_layout, descriptor_set_layout];
    let descriptor_sets = unsafe {
        device.device.allocate_descriptor_sets(
            &vk::DescriptorSetAllocateInfo::default()
                .descriptor_pool(descriptor_pool)
                .set_layouts(&set_layouts),
        )
    }
    .expect("failed to allocate descriptor sets");
    let set_for_l0 = descriptor_sets[0]; // bound to L0 for the downsample pass
    let set_for_l1 = descriptor_sets[1]; // bound to L1 for the composite pass

    // Same real sampler parameters as the engine's own bindless_sampler
    // (crates/tre-rhi-vulkan/src/lib.rs) -- an apples-to-apples
    // comparison, not a more forgiving one.
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

    let out_dir = env!("OUT_DIR");
    let read_spv = |name: &str| {
        std::fs::read(format!("{out_dir}/{name}.spv"))
            .unwrap_or_else(|e| panic!("failed to read compiled shader {name}: {e}"))
    };

    // The square's own draw uses the engine's real, unmodified bindless
    // pipeline (sdf_rounded_rect doesn't sample any texture at all, so
    // the bindless descriptor set being bound alongside it is inert).
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
    let composite_pipeline = create_custom_pipeline(
        &device.device,
        &read_spv("bindless_textured.vert"),
        &read_spv("passthrough_nonbindless.frag"),
        pipeline_layout,
        tre_rhi_vulkan::HEADLESS_FORMAT,
        device.stencil_format,
    );

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

    let upload_quad = |width: u32, height: u32| {
        let quad = textured_quad(width as f32, height as f32);
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
    let (half_quad_vb, half_quad_ib) = upload_quad(SIZE_HALF.0, SIZE_HALF.1);
    let (full_quad_vb, full_quad_ib) = upload_quad(SIZE_FULL.0, SIZE_FULL.1);

    submit_frame(&device, &swapchain, |cmd_buffer| {
        let raw_cmd_buffer = vk::CommandBuffer::from_raw(cmd_buffer.raw_handle());

        // L0: the square, rendered into a full-size transient target -- real,
        // unmodified RHI render-to-texture, identical to the bindless demo.
        let l0 = device
            .acquire_transient_target(SIZE_FULL.0, SIZE_FULL.1, TextureFormat::Rgba16Float)
            .expect("failed to acquire L0");
        cmd_buffer.begin_render_to_texture(&*l0, SIZE_FULL.0, SIZE_FULL.1);
        cmd_buffer.set_pipeline(&rect_pipeline);
        cmd_buffer.bind_vertex_buffer(&square_vertex_buffer, 0);
        cmd_buffer.bind_index_buffer(&square_index_buffer, 0);
        cmd_buffer.draw_indexed(square_frame.indices.len() as u32, 0, 0);

        // L0 -> L1: downsample, via the plain non-bindless descriptor set --
        // the one variable under test. `end_render_to_texture` transitions L0
        // to SHADER_READ_ONLY_OPTIMAL with the real, already-proven barrier;
        // this just points a conventional descriptor at that same real view
        // instead of registering it into the bindless array.
        cmd_buffer.end_render_to_texture(&*l0);
        let l0_view = vk::ImageView::from_raw(l0.raw_handle());
        let l0_image_info = vk::DescriptorImageInfo::default()
            .image_view(l0_view)
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .sampler(sampler);
        unsafe {
            device.device.update_descriptor_sets(
                &[vk::WriteDescriptorSet::default()
                    .dst_set(set_for_l0)
                    .dst_binding(0)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(std::slice::from_ref(&l0_image_info))],
                &[],
            );
        }

        let l1 = device
            .acquire_transient_target(SIZE_HALF.0, SIZE_HALF.1, TextureFormat::Rgba16Float)
            .expect("failed to acquire L1");
        cmd_buffer.begin_render_to_texture_no_end(&*l1, SIZE_HALF.0, SIZE_HALF.1);
        // Raw bind: `RhiCommandBuffer::set_pipeline` would unconditionally
        // rebind the bindless descriptor set, which is wrong for this
        // pipeline's own, deliberately different layout.
        let push_constants: [f32; 2] = [SIZE_HALF.0 as f32, SIZE_HALF.1 as f32];
        unsafe {
            device.device.cmd_bind_pipeline(
                raw_cmd_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                downsample_pipeline,
            );
            device.device.cmd_bind_descriptor_sets(
                raw_cmd_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline_layout,
                0,
                &[set_for_l0],
                &[],
            );
            device.device.cmd_push_constants(
                raw_cmd_buffer,
                pipeline_layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                bytemuck::cast_slice(&push_constants),
            );
        }
        cmd_buffer.bind_vertex_buffer(&half_quad_vb, 0);
        cmd_buffer.bind_index_buffer(&half_quad_ib, 0);
        cmd_buffer.draw_indexed(6, 0, 0);
        device.release_transient_target(l0);

        // L1 -> swapchain: composite, again via a plain descriptor -- L1 was
        // ALSO this same frame's own render target a moment ago, so this
        // step alone reproduces the exact scenario under test for the final
        // composite too, not just the downsample hop.
        cmd_buffer.end_render_to_texture(&*l1);
        let l1_view = vk::ImageView::from_raw(l1.raw_handle());
        let l1_image_info = vk::DescriptorImageInfo::default()
            .image_view(l1_view)
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .sampler(sampler);
        unsafe {
            device.device.update_descriptor_sets(
                &[vk::WriteDescriptorSet::default()
                    .dst_set(set_for_l1)
                    .dst_binding(0)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(std::slice::from_ref(&l1_image_info))],
                &[],
            );
        }

        cmd_buffer.resume_swapchain_rendering();
        unsafe {
            device.device.cmd_bind_pipeline(
                raw_cmd_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                composite_pipeline,
            );
            device.device.cmd_bind_descriptor_sets(
                raw_cmd_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline_layout,
                0,
                &[set_for_l1],
                &[],
            );
        }
        cmd_buffer.bind_vertex_buffer(&full_quad_vb, 0);
        cmd_buffer.bind_index_buffer(&full_quad_ib, 0);
        cmd_buffer.draw_indexed(6, 0, 0);
        device.release_transient_target(l1);
    })
    .expect("submit_frame failed");

    let bgra = swapchain
        .read_pixels_bgra8()
        .expect("failed to read back pixels");
    let pixel_at =
        |x: u32, y: u32| -> [u8; 4] { pixel_helpers::bgra_pixel_at(&bgra, SWAPCHAIN_WIDTH, x, y) };

    let background = pixel_at(2, 2);
    eprintln!("background (clear color): {background:?}");

    let center = (
        (SQUARE_X + SQUARE_SIZE / 2.0) as u32,
        (SQUARE_Y + SQUARE_SIZE / 2.0) as u32,
    );
    let interior = pixel_at(center.0, center.1);
    eprintln!("composited square center: {interior:?}");

    if interior == background {
        eprintln!(
            "RESULT: NON-BINDLESS BINDING STILL READS ALL-ZERO -- the bindless array is NOT \
             the root cause; this bug is not specific to it."
        );
        panic!(
            "the composited square's center reads exactly the background clear color -- the \
             same all-zero-read symptom REVIEW.md finding #130 describes, reproduced WITHOUT \
             the bindless array"
        );
    }

    assert!(
        interior[0] > background[0] || interior[1] > background[1] || interior[2] > background[2],
        "composited center must be brighter than background if the square's own white content \
         survived the downsample+composite round trip -- got {interior:?} vs background \
         {background:?}"
    );
    eprintln!(
        "RESULT: NON-BINDLESS BINDING WORKS -- real, non-background content was sampled and \
         composited successfully. The bindless texture array is implicated as the real root \
         cause of REVIEW.md finding #130."
    );

    let mut rgba = bgra.clone();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_NONBINDLESS_EXPERIMENT_OUTPUT")
        .unwrap_or_else(|_| "dual_kawase_nonbindless_experiment_output.png".to_string());
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
    eprintln!("wrote {SWAPCHAIN_WIDTH}x{SWAPCHAIN_HEIGHT} render to {out_path}");

    // Real cleanup, matching every other demo's own discipline -- wait
    // for the GPU to finish the one submitted frame, then destroy every
    // object this experiment created directly (not through a Drop impl,
    // since these are hand-rolled raw `ash` objects with no RHI wrapper
    // type of their own).
    unsafe {
        device
            .device
            .device_wait_idle()
            .expect("device_wait_idle failed");
        device.device.destroy_pipeline(downsample_pipeline, None);
        device.device.destroy_pipeline(composite_pipeline, None);
        device.device.destroy_pipeline_layout(pipeline_layout, None);
        device.device.destroy_sampler(sampler, None);
        device.device.destroy_descriptor_pool(descriptor_pool, None);
        device
            .device
            .destroy_descriptor_set_layout(descriptor_set_layout, None);
    }
}
