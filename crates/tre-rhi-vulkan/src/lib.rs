//! Vulkan 1.2+ RHI backend (`RhiDevice`/`RhiCommandBuffer` impls,
//! ARCHITECTURE.md Section 6), built on the `ash` raw-bindings crate
//! (IMPLEMENTATION.md Step 2.1). Cross-platform wherever Vulkan is
//! available -- unlike the DX12/Metal backends, not target-gated to one OS.
//!
//! One of the three crates permitted to contain `unsafe`
//! (TECHNICAL.md Section 9.1), for raw Vulkan FFI.
#![deny(unsafe_op_in_unsafe_fn)]

mod headless;

pub use headless::{HeadlessSwapchain, HEADLESS_FORMAT};

use std::collections::HashMap;
use std::ffi::{c_char, CStr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use ash::vk;
use ash::vk::Handle;
use tre_engine::{
    AcquiredImage, EngineError, RhiBuffer, RhiCommandBuffer, RhiDevice, RhiDynamicRingBuffer,
    RhiPipelineState, RhiSwapchain, RhiTexture, ScissorRect, TextureFormat, UiVertex,
};

const REQUIRED_DEVICE_EXTENSIONS: &[&CStr] =
    &[ash::khr::swapchain::NAME, ash::khr::dynamic_rendering::NAME];

/// TECHNICAL.md Section 3.1's triple-buffered ring: 3 logical segments,
/// one per frame-in-flight slot.
const FRAMES_IN_FLIGHT: usize = 3;

/// TECHNICAL.md Section 3.1's "256-byte minimum alignment for RHI dynamic
/// offsets."
const RING_BUFFER_ALIGNMENT: usize = 256;

fn align_up(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

fn texture_format_to_vk(format: TextureFormat) -> vk::Format {
    match format {
        TextureFormat::Bgra8Srgb => vk::Format::B8G8R8A8_SRGB,
        TextureFormat::Rgba16Float => vk::Format::R16G16B16A16_SFLOAT,
    }
}

/// Shared frame-completion fences (TECHNICAL.md Section 3.1's 3-deep
/// ring), owned jointly by `VulkanDevice` and every
/// `VulkanRingBuffer` created from it -- so a ring buffer's segment
/// selection is tied to the EXACT signal the device's own frame
/// submission produces, not a separate, never-actually-wired fence. This
/// is currently redundant with `VulkanDevice::begin_frame`'s own wait
/// (Phase 2 Step 1 keeps submission fully synchronous, one frame at a
/// time -- see `planning/archive/PLAN_PHASE2_STEP1.md`'s scope decision),
/// but stays correct once real overlapping submission is introduced
/// later, since nothing about this structure assumes synchronous
/// submission.
struct FrameSync {
    /// The SAME single fence Phase 0 built (`in_flight_fence`): waited on
    /// and reset at the start of every `begin_frame`, signaled by every
    /// `submit_and_present`. There is only one -- NOT one per ring-buffer
    /// segment -- because `VulkanDevice` reuses a single persistent
    /// `command_buffer` across every frame regardless of which ring-buffer
    /// segment is current; gating that one command buffer's reuse with a
    /// *rotating* fence (waiting on a fresh, trivially-already-signaled
    /// fence instead of the one its own last submission actually
    /// signaled) would NOT prove the GPU is done with it. An earlier
    /// version of this file made exactly that mistake -- three fences,
    /// indexed by `frame_index` -- and the Vulkan validation layer caught
    /// it immediately (`walking_skeleton`/`multi_window` both threw
    /// command-buffer-still-in-use errors) once actually run, not just
    /// compiled. See `planning/archive/LOG_PHASE2_STEP1.md`.
    fence: vk::Fence,
    /// A rotating counter (0, 1, 2, 0, 1, 2, ...), advanced by
    /// `VulkanDevice::submit_and_present` after every frame. Used ONLY by
    /// `VulkanRingBuffer` to pick which of its 3 segments is "current" --
    /// safe without its own per-segment fence precisely because `fence`
    /// above already fully synchronizes every single frame, so by the
    /// time this counter cycles back to a given value, at least two other
    /// fully-synchronous frames have completed since that segment was
    /// last written.
    frame_index: AtomicUsize,
}

/// Shared Vulkan device state (ARCHITECTURE.md Section 2.1's "Global
/// `RhiDevice`"). Frame submission itself stays fully synchronous (one
/// frame in flight, `begin_frame` fully waits before recording) -- see
/// `frame_sync`'s doc comment for the real, once-broken-then-fixed reason
/// it still tracks a rotating index despite that.
pub struct VulkanDevice {
    entry: ash::Entry,
    pub instance: ash::Instance,
    pub physical_device: vk::PhysicalDevice,
    pub device: ash::Device,
    pub queue_family_index: u32,
    graphics_queue: vk::Queue,
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    dynamic_rendering: ash::khr::dynamic_rendering::Device,
    frame_sync: Arc<FrameSync>,
    /// Transient render target pool (TECHNICAL.md Section 3.2), keyed by
    /// `(width, height, format)` after power-of-two bucket rounding.
    /// `Mutex`-guarded (not `RefCell`) so `VulkanDevice` stays genuinely
    /// `Sync`-shareable across threads later, matching the same
    /// forward-looking reasoning as `tre_memory::SpscRingBuffer`.
    transient_pool: Mutex<TransientPool>,
}

impl VulkanDevice {
    /// Creates the Vulkan instance and a temporary probe surface (needed
    /// only to query present support while picking a physical device),
    /// then the logical device and queue. Returns the surface loader and
    /// probe surface too, since `VulkanSwapchain::new` reuses both rather
    /// than creating a second surface.
    pub fn new(
        display_handle: raw_window_handle::RawDisplayHandle,
        window_handle: raw_window_handle::RawWindowHandle,
    ) -> Result<(Self, ash::khr::surface::Instance, vk::SurfaceKHR), EngineError> {
        // SAFETY: dynamically loads the system Vulkan loader; this is the
        // first Vulkan call the crate makes, and the resulting `Entry` is
        // kept alive on `Self` for as long as any function pointers loaded
        // through it (instance/device calls below) are used.
        let entry = unsafe { ash::Entry::load() }.map_err(|_| EngineError::DeviceLost)?;

        let app_info = vk::ApplicationInfo::default()
            .application_name(c"tre-walking-skeleton")
            .api_version(vk::API_VERSION_1_2);

        let mut required_extensions = ash_window::enumerate_required_extensions(display_handle)
            .map_err(|_| EngineError::DeviceLost)?
            .to_vec();
        required_extensions.push(ash::khr::get_physical_device_properties2::NAME.as_ptr());

        let instance_create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&required_extensions);

        // SAFETY: `entry` was just loaded above and is valid; `app_info`
        // and `required_extensions` are locals borrowed only for the
        // duration of this call. The returned `VkInstance` is destroyed
        // exactly once in `Drop for VulkanDevice` below.
        let instance = unsafe { entry.create_instance(&instance_create_info, None) }
            .map_err(|_| EngineError::DeviceLost)?;

        let surface_loader = ash::khr::surface::Instance::new(&entry, &instance);
        let surface = Self::create_surface_raw(&entry, &instance, display_handle, window_handle)?;

        // SAFETY: `instance` was just successfully created above and is
        // still valid.
        let physical_devices = unsafe { instance.enumerate_physical_devices() }
            .map_err(|_| EngineError::DeviceLost)?;

        let (physical_device, queue_family_index) = physical_devices
            .into_iter()
            .find_map(|pd| {
                // SAFETY: `pd` comes from `enumerate_physical_devices` on
                // this same still-valid `instance`, so it is a valid
                // physical device handle.
                let queue_families =
                    unsafe { instance.get_physical_device_queue_family_properties(pd) };
                queue_families.iter().enumerate().find_map(|(i, family)| {
                    let i = i as u32;
                    let graphics_capable = family.queue_flags.contains(vk::QueueFlags::GRAPHICS);
                    // SAFETY: `pd` and `i` are valid (queried from this
                    // instance immediately above), and `surface` was just
                    // created by `create_surface_raw` and is still alive
                    // for the duration of this call.
                    let present_capable = unsafe {
                        surface_loader.get_physical_device_surface_support(pd, i, surface)
                    }
                    .unwrap_or(false);
                    (graphics_capable && present_capable).then_some((pd, i))
                })
            })
            .ok_or(EngineError::DeviceLost)?;

        let queue_priorities = [1.0f32];
        let queue_create_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family_index)
            .queue_priorities(&queue_priorities);
        let queue_create_infos = [queue_create_info];

        let device_extension_names: Vec<*const c_char> = REQUIRED_DEVICE_EXTENSIONS
            .iter()
            .map(|e| e.as_ptr())
            .collect();

        let mut dynamic_rendering_feature =
            vk::PhysicalDeviceDynamicRenderingFeatures::default().dynamic_rendering(true);

        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&device_extension_names)
            .push_next(&mut dynamic_rendering_feature);

        // SAFETY: `physical_device` was chosen above from this instance's
        // own enumeration, and `device_create_info`'s borrowed
        // `queue_create_infos`/`device_extension_names`/
        // `dynamic_rendering_feature` are all locals that outlive this
        // call.
        let device = unsafe { instance.create_device(physical_device, &device_create_info, None) }
            .map_err(|_| EngineError::DeviceLost)?;

        // SAFETY: `device` was just successfully created above, and
        // `queue_family_index`/index `0` are exactly the family and single
        // queue priority `device_create_info` requested.
        let graphics_queue = unsafe { device.get_device_queue(queue_family_index, 0) };

        // SAFETY: `device` is the just-created, still-valid logical
        // device, and `queue_family_index` is the same family it was
        // created with.
        let command_pool = unsafe {
            device.create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(queue_family_index)
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )
        }
        .map_err(|_| EngineError::DeviceLost)?;

        // SAFETY: `command_pool` was just created above on this same
        // `device` and is still valid.
        let command_buffer = unsafe {
            device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
        }
        .map_err(|_| EngineError::DeviceLost)?[0];

        let dynamic_rendering = ash::khr::dynamic_rendering::Device::new(&instance, &device);

        // SAFETY: `device` is valid (created above).
        let fence = unsafe {
            device.create_fence(
                &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                None,
            )
        }
        .map_err(|_| EngineError::DeviceLost)?;
        let frame_sync = Arc::new(FrameSync {
            fence,
            frame_index: AtomicUsize::new(0),
        });

        Ok((
            Self {
                entry,
                instance,
                physical_device,
                device,
                queue_family_index,
                graphics_queue,
                command_pool,
                command_buffer,
                dynamic_rendering,
                frame_sync,
                transient_pool: Mutex::new(TransientPool::default()),
            },
            surface_loader,
            surface,
        ))
    }

    pub fn graphics_queue(&self) -> vk::Queue {
        self.graphics_queue
    }

    /// Snapshot of the transient render target pool's hit/miss counters
    /// (TECHNICAL.md Section 3.2), for demos/tests to prove steady-state
    /// reuse without reaching into private pool state.
    #[must_use]
    pub fn transient_pool_stats(&self) -> TransientPoolStats {
        self.transient_pool
            .lock()
            .expect("transient pool poisoned")
            .stats
    }

    /// Creates a new Vulkan surface for another window against this
    /// already-selected device -- the multi-window path (Phase 1 Step 1).
    /// `VulkanDevice::new` uses the same underlying call for its initial
    /// probe surface; this is the version any *additional* window uses,
    /// since re-running physical device selection per window would be
    /// wrong (all windows share the one device chosen at startup, per
    /// ARCHITECTURE.md Section 2.1's "Global RhiDevice").
    ///
    /// # Errors
    /// Returns [`EngineError::DeviceLost`] if surface creation fails.
    pub fn create_surface(
        &self,
        display_handle: raw_window_handle::RawDisplayHandle,
        window_handle: raw_window_handle::RawWindowHandle,
    ) -> Result<(ash::khr::surface::Instance, vk::SurfaceKHR), EngineError> {
        let surface_loader = ash::khr::surface::Instance::new(&self.entry, &self.instance);
        let surface =
            Self::create_surface_raw(&self.entry, &self.instance, display_handle, window_handle)?;
        Ok((surface_loader, surface))
    }

    fn create_surface_raw(
        entry: &ash::Entry,
        instance: &ash::Instance,
        display_handle: raw_window_handle::RawDisplayHandle,
        window_handle: raw_window_handle::RawWindowHandle,
    ) -> Result<vk::SurfaceKHR, EngineError> {
        // SAFETY: `entry`/`instance` are valid for the duration of this
        // call, and `display_handle`/`window_handle` are valid raw handles
        // for a live window for the duration of this call, which is all
        // `ash_window::create_surface` requires (it does not retain them).
        unsafe { ash_window::create_surface(entry, instance, display_handle, window_handle, None) }
            .map_err(|_| EngineError::DeviceLost)
    }

    pub fn create_pipeline(
        &self,
        vertex_spv: &[u8],
        fragment_spv: &[u8],
        color_format: vk::Format,
    ) -> Result<VulkanPipelineState, EngineError> {
        let vertex_module = self.create_shader_module(vertex_spv)?;
        let fragment_module = self.create_shader_module(fragment_spv)?;

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

        let binding_description = vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(std::mem::size_of::<UiVertex>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX);
        let attribute_descriptions = [
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
        ];
        let bindings = [binding_description];
        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&bindings)
            .vertex_attribute_descriptions(&attribute_descriptions);

        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);

        // ARCHITECTURE.md Section 6.1: depth test/write disabled, culling
        // disabled, premultiplied-alpha blending in linear space.
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
        let color_blend =
            vk::PipelineColorBlendStateCreateInfo::default().attachments(&attachments);

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        // SAFETY: `self.device` is the valid logical device owned by this
        // `VulkanDevice`, and the `push_constant_ranges` slice is a local
        // temporary that outlives this call.
        let layout = unsafe {
            self.device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().push_constant_ranges(&[
                    vk::PushConstantRange::default()
                        .stage_flags(vk::ShaderStageFlags::VERTEX)
                        .offset(0)
                        .size(8), // vec2 screen_size
                ]),
                None,
            )
        }
        .map_err(|_| EngineError::PipelineCreationFailed)?;

        let color_formats = [color_format];
        let mut rendering_info =
            vk::PipelineRenderingCreateInfo::default().color_attachment_formats(&color_formats);

        let pipeline_create_info = vk::GraphicsPipelineCreateInfo::default()
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

        // SAFETY: `self.device` is valid; `pipeline_create_info` and
        // everything it borrows (`stages`, `vertex_input`, `attachments`
        // via `color_blend`, `dynamic_states`, and `rendering_info` via
        // `push_next`) are locals that outlive this call; `layout` was
        // just created above on this same device, and
        // `vk::PipelineCache::null()` is a valid null handle meaning "no
        // cache".
        let pipeline = unsafe {
            self.device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[pipeline_create_info],
                None,
            )
        }
        .map_err(|_| EngineError::PipelineCreationFailed)?[0];

        // SAFETY: `vertex_module`/`fragment_module` were created by this
        // same device above and are no longer needed once
        // `create_graphics_pipelines` has consumed them into `pipeline`.
        unsafe {
            self.device.destroy_shader_module(vertex_module, None);
            self.device.destroy_shader_module(fragment_module, None);
        }

        Ok(VulkanPipelineState {
            pipeline,
            layout,
            device: self.device.clone(),
        })
    }

    fn create_shader_module(&self, spv: &[u8]) -> Result<vk::ShaderModule, EngineError> {
        let words = ash::util::read_spv(&mut std::io::Cursor::new(spv))
            .map_err(|_| EngineError::PipelineCreationFailed)?;
        // SAFETY: `self.device` is valid, and `words` is a local `Vec` of
        // complete, word-aligned SPIR-V (parsed by `ash::util::read_spv`
        // above) that outlives this call.
        unsafe {
            self.device
                .create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None)
        }
        .map_err(|_| EngineError::PipelineCreationFailed)
    }

    /// Uploads vertex/index data into a single host-visible, host-coherent
    /// buffer. Phase 0 only -- TECHNICAL.md Section 3.1's mapped ring
    /// buffers replace this ad hoc allocation in Phase 2.
    pub fn upload_buffer(
        &self,
        bytes: &[u8],
        usage: vk::BufferUsageFlags,
    ) -> Result<VulkanBuffer, EngineError> {
        // SAFETY: `self.device` is valid, and `bytes.len()` is used
        // directly as `size` so the create info describes exactly this
        // buffer's contents.
        let buffer = unsafe {
            self.device.create_buffer(
                &vk::BufferCreateInfo::default()
                    .size(bytes.len() as u64)
                    .usage(usage)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            )
        }
        .map_err(|_| EngineError::DeviceLost)?;

        // SAFETY: `buffer` was just created above on this device.
        let requirements = unsafe { self.device.get_buffer_memory_requirements(buffer) };
        // SAFETY: `self.physical_device` is the device selected in
        // `VulkanDevice::new` and is valid for as long as `self.instance`
        // (also alive here) is.
        let memory_properties = unsafe {
            self.instance
                .get_physical_device_memory_properties(self.physical_device)
        };
        let wanted = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
        let memory_type_index = (0..memory_properties.memory_type_count)
            .find(|&i| {
                (requirements.memory_type_bits & (1 << i)) != 0
                    && memory_properties.memory_types[i as usize]
                        .property_flags
                        .contains(wanted)
            })
            .ok_or(EngineError::DeviceLost)?;

        // SAFETY: `self.device` is valid, `requirements.size` comes
        // directly from `get_buffer_memory_requirements` above, and
        // `memory_type_index` was selected from the `find` above so it is
        // one of the bits set in `requirements.memory_type_bits`.
        let memory = unsafe {
            self.device.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(requirements.size)
                    .memory_type_index(memory_type_index),
                None,
            )
        }
        .map_err(|_| EngineError::DeviceLost)?;

        // SAFETY: `buffer` and `memory` were both just created above on
        // this device, `buffer` has not been bound to memory before now,
        // and `memory` was allocated as host-visible/host-coherent
        // (selected via `wanted` above), so mapping it is valid. `dst` is
        // therefore writable for at least `bytes.len()` bytes (the same
        // length passed to `map_memory`), matching `copy_nonoverlapping`'s
        // write, and `unmap_memory` is called exactly once right after to
        // end the mapping.
        unsafe {
            self.device
                .bind_buffer_memory(buffer, memory, 0)
                .map_err(|_| EngineError::DeviceLost)?;
            let dst = self
                .device
                .map_memory(memory, 0, bytes.len() as u64, vk::MemoryMapFlags::empty())
                .map_err(|_| EngineError::DeviceLost)?;
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst as *mut u8, bytes.len());
            self.device.unmap_memory(memory);
        }

        Ok(VulkanBuffer {
            buffer,
            memory,
            device: self.device.clone(),
        })
    }

    /// Allocates any transient-target buckets a prior frame's
    /// `acquire_transient_target` miss queued (TECHNICAL.md Section 3.2,
    /// DESIGN.md Section 2.6's "grown into the pool asynchronously for
    /// subsequent frames"). Called at the very start of `begin_frame`,
    /// before that frame's render tick begins -- allocating here, not
    /// mid-frame, is what "asynchronously" means for this step's scope
    /// (see `planning/archive/PLAN_PHASE2_STEP1.md`'s scope decision;
    /// no background thread is involved).
    fn grow_pending_transient_targets(&self) {
        let pending = {
            let mut pool = self.transient_pool.lock().expect("transient pool poisoned");
            std::mem::take(&mut pool.pending_growth)
        };
        for (width, height, format) in pending {
            if let Ok(texture) = VulkanTexture::new(self, width, height, format) {
                let mut pool = self.transient_pool.lock().expect("transient pool poisoned");
                pool.free
                    .entry((width, height, format))
                    .or_default()
                    .push(texture);
            }
        }
    }
}

impl Drop for VulkanDevice {
    fn drop(&mut self) {
        // Explicitly drop every texture still checked into the transient
        // pool BEFORE destroying the device below. Rust drops a struct's
        // OTHER fields (including `transient_pool`) only after this
        // `drop` function returns, which would run each pooled
        // `VulkanTexture`'s own `Drop` (destroying its image/view/memory)
        // AFTER `destroy_device` below already ran -- a real
        // use-after-free the Vulkan validation layer caught as 6 leaked
        // objects during this step's own demo run, since without this
        // clear the objects were never destroyed at all (this fixes both
        // the leak and the ordering hazard a naive fix would introduce).
        if let Ok(mut pool) = self.transient_pool.lock() {
            pool.free.clear();
        }
        // SAFETY: `self` is being dropped, so no other code holds
        // references to these handles afterward; destroying the fences and
        // command pool (children of the device) before the device, and
        // the device before the instance, follows Vulkan's required
        // child-before-parent destruction order. Any `VulkanRingBuffer`s
        // created from this device hold a clone of `self.frame_sync`'s
        // `Arc`, not this fence independently, so they don't outlive this
        // destruction in a way that would use-after-free it --
        // `VulkanRingBuffer` never touches `frame_sync.fence` at all (see
        // its own doc comment).
        unsafe {
            self.device.destroy_fence(self.frame_sync.fence, None);
            self.device.destroy_command_pool(self.command_pool, None);
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

impl RhiDevice for VulkanDevice {
    fn create_dynamic_ring_buffer(&self, capacity: usize) -> Box<dyn RhiDynamicRingBuffer> {
        Box::new(
            VulkanRingBuffer::new(self, capacity).expect("failed to create dynamic ring buffer"),
        )
    }

    fn acquire_transient_target(
        &self,
        width: u32,
        height: u32,
        format: TextureFormat,
    ) -> Box<dyn RhiTexture> {
        let bucket = (
            width.next_power_of_two(),
            height.next_power_of_two(),
            format,
        );
        let mut pool = self.transient_pool.lock().expect("transient pool poisoned");

        if let Some(texture) = pool.free.get_mut(&bucket).and_then(Vec::pop) {
            pool.stats.hits += 1;
            return Box::new(texture);
        }

        // Genuine miss: DESIGN.md Section 2.6 forbids a dynamic RHI
        // allocation inside the render tick, so borrow the next-larger
        // already-pooled bucket for this frame (any texture at least as
        // large as requested is usable -- the caller renders into a
        // sub-rect if it's bigger) and queue the exactly-right bucket to
        // be grown in at the start of the next frame.
        pool.stats.misses += 1;
        if !pool.pending_growth.contains(&bucket) {
            pool.pending_growth.push(bucket);
        }
        if let Some(((_, _, _), texture)) = pool
            .free
            .iter_mut()
            .filter(|((w, h, f), textures)| {
                *f == format && *w >= bucket.0 && *h >= bucket.1 && !textures.is_empty()
            })
            .min_by_key(|((w, h, _), _)| u64::from(*w) * u64::from(*h))
            .map(|(key, textures)| (*key, textures.pop().expect("checked non-empty above")))
        {
            return Box::new(texture);
        }

        // No existing bucket at all (even oversized) is free -- this is
        // the very first request of this size/format combination this
        // process has ever seen. Allocating here is the one case Phase 2
        // Step 1 accepts a synchronous allocation for (there is nothing
        // smaller to borrow), matching Phase 0/1's "walking skeleton
        // first" precedent: a cold-start allocation is unavoidable
        // somewhere, and DESIGN.md Section 2.6's own wording ("a first-
        // ever window size... isn't already resident in the pool") is
        // explicit that this exact case can occur.
        drop(pool);
        Box::new(
            VulkanTexture::new(self, width, height, format)
                .expect("failed to create transient render target"),
        )
    }

    fn release_transient_target(&self, texture: Box<dyn RhiTexture>) {
        let (width, height) = texture.dimensions();
        let format = texture.format();
        let bucket = (
            width.next_power_of_two(),
            height.next_power_of_two(),
            format,
        );
        // Reconstructs a `VulkanTexture` from `texture`'s opaque handles
        // rather than downcasting -- `texture` is a `Box<dyn RhiTexture>`
        // this same `VulkanDevice` produced moments ago via
        // `acquire_transient_target`/`VulkanTexture::new`, so every handle
        // it exposes is one of this device's own live Vulkan objects.
        let reclaimed = VulkanTexture {
            view: vk::ImageView::from_raw(texture.raw_handle()),
            image: vk::Image::from_raw(texture.image_handle()),
            memory: vk::DeviceMemory::from_raw(texture.memory_handle()),
            width,
            height,
            format,
            device: self.device.clone(),
        };
        // `texture` (the original box) must not also run its `Drop` and
        // destroy these same handles out from under `reclaimed`.
        std::mem::forget(texture);

        let mut pool = self.transient_pool.lock().expect("transient pool poisoned");
        pool.free.entry(bucket).or_default().push(reclaimed);
    }

    fn begin_frame(
        &self,
        swapchain: &dyn RhiSwapchain,
    ) -> Result<(Box<dyn RhiCommandBuffer>, AcquiredImage), EngineError> {
        // SAFETY: `self.device` is valid and `self.frame_sync.fence` was
        // created signaled in `new`; under the single-frame-in-flight
        // model it is only ever waited on and reset here, once per frame.
        unsafe {
            self.device
                .wait_for_fences(&[self.frame_sync.fence], true, u64::MAX)
                .map_err(|_| EngineError::DeviceLost)?;
            self.device
                .reset_fences(&[self.frame_sync.fence])
                .map_err(|_| EngineError::DeviceLost)?;
        }

        // Genuine transient-pool growth queued by a prior frame's miss
        // (see `acquire_transient_target`) happens here, before this
        // frame's render tick begins -- DESIGN.md Section 2.6 requires it
        // not happen mid-frame, and "the start of the next frame" is
        // exactly that boundary.
        self.grow_pending_transient_targets();

        let image = swapchain.acquire_next_image()?;

        // Reuse the one persistent command buffer (allocated once in
        // `new`) rather than allocate-then-free every frame: the fence
        // wait above already guarantees the GPU is done with whatever it
        // last recorded, so resetting it here is safe.
        let command_buffer = self.command_buffer;
        // SAFETY: `command_buffer` is the persistent buffer allocated once
        // in `new`; the fence wait immediately above already guarantees
        // the GPU is done with whatever it last recorded, so resetting
        // and beginning a fresh recording on it now does not race the
        // GPU.
        unsafe {
            self.device
                .reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())
                .map_err(|_| EngineError::DeviceLost)?;
            self.device
                .begin_command_buffer(command_buffer, &vk::CommandBufferBeginInfo::default())
                .map_err(|_| EngineError::DeviceLost)?;
        }

        let target_view = vk::ImageView::from_raw(image.target_view_handle);
        let target_image = vk::Image::from_raw(image.target_image_handle);
        let (width, height) = swapchain.extent();

        // Undefined -> COLOR_ATTACHMENT_OPTIMAL: dynamic rendering has no
        // render pass to do this transition implicitly.
        let barrier = vk::ImageMemoryBarrier::default()
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .image(target_image)
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .level_count(1)
                    .layer_count(1),
            );
        // SAFETY: `command_buffer` is in the recording state (just begun
        // above), and `target_image` is the swapchain image acquired this
        // frame, whose layout is being transitioned before any rendering
        // uses it.
        unsafe {
            self.device.cmd_pipeline_barrier(
                command_buffer,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
        }

        let color_attachment = vk::RenderingAttachmentInfo::default()
            .image_view(target_view)
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.05, 0.05, 0.08, 1.0],
                },
            });
        let color_attachments = [color_attachment];
        let rendering_info = vk::RenderingInfo::default()
            .render_area(vk::Rect2D {
                offset: vk::Offset2D::default(),
                extent: vk::Extent2D { width, height },
            })
            .layer_count(1)
            .color_attachments(&color_attachments);

        // SAFETY: `command_buffer` is still recording, and `target_view`
        // (via `color_attachment`/`rendering_info`) is the same acquired
        // image the barrier above just transitioned to
        // `COLOR_ATTACHMENT_OPTIMAL`; `width`/`height` match the
        // swapchain's own reported extent.
        unsafe {
            self.dynamic_rendering
                .cmd_begin_rendering(command_buffer, &rendering_info);
            self.device.cmd_set_viewport(
                command_buffer,
                0,
                &[vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: width as f32,
                    height: height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            );
            self.device.cmd_set_scissor(
                command_buffer,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D::default(),
                    extent: vk::Extent2D { width, height },
                }],
            );
        }

        Ok((
            Box::new(VulkanCommandBuffer {
                device: self.device.clone(),
                command_buffer,
                width,
                height,
                pipeline_layout: None,
            }),
            image,
        ))
    }

    fn submit_and_present(
        &self,
        cmd_buffer: Box<dyn RhiCommandBuffer>,
        swapchain: &dyn RhiSwapchain,
        image: AcquiredImage,
    ) -> Result<(), EngineError> {
        let raw_cmd = vk::CommandBuffer::from_raw(cmd_buffer.raw_handle());
        let target_image = vk::Image::from_raw(image.target_image_handle);

        let barrier = vk::ImageMemoryBarrier::default()
            .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_access_mask(vk::AccessFlags::empty())
            .image(target_image)
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .level_count(1)
                    .layer_count(1),
            );

        // SAFETY: `raw_cmd` is the command buffer `begin_frame` began
        // rendering into this same frame (a `cmd_begin_rendering` without
        // a matching `cmd_end_rendering` yet), and `target_image` is the
        // same acquired image that rendering targeted, so ending
        // rendering, transitioning the image, and ending the buffer here
        // are all well-ordered and happen exactly once per frame.
        unsafe {
            self.dynamic_rendering.cmd_end_rendering(raw_cmd);
            self.device.cmd_pipeline_barrier(
                raw_cmd,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
            self.device
                .end_command_buffer(raw_cmd)
                .map_err(|_| EngineError::DeviceLost)?;
        }

        let wait_semaphore = vk::Semaphore::from_raw(image.image_available_semaphore_handle);
        let wait_semaphores = [wait_semaphore];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let signal_semaphore = vk::Semaphore::from_raw(image.render_finished_semaphore_handle);
        let signal_semaphores = [signal_semaphore];
        let command_buffers = [raw_cmd];
        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&command_buffers)
            .signal_semaphores(&signal_semaphores);

        // SAFETY: `raw_cmd` was just ended above; `wait_semaphores`/
        // `signal_semaphores` come from the `AcquiredImage` returned by
        // `begin_frame` this same frame and are valid; `self.frame_sync
        // .fence` is the same fence `begin_frame` waited on and reset for
        // this frame, so signaling it here is the matching half of that
        // handshake.
        unsafe {
            self.device
                .queue_submit(self.graphics_queue, &[submit_info], self.frame_sync.fence)
                .map_err(|_| EngineError::DeviceLost)?;
        }
        // Advances the ring-buffer segment-selection counter -- see
        // `FrameSync::frame_index`'s doc comment for why this is sound
        // without its own per-segment fence. A plain load-then-store (not
        // `fetch_add`) is correct here: `submit_and_present` is only ever
        // called from the single render-loop thread under this step's
        // synchronous-submission scope, so there is no concurrent writer
        // to race against, and `fetch_add` would not have wrapped the
        // *stored* value into `0..FRAMES_IN_FLIGHT` anyway (only its
        // discarded return value).
        let next_frame_index =
            (self.frame_sync.frame_index.load(Ordering::Acquire) + 1) % FRAMES_IN_FLIGHT;
        self.frame_sync
            .frame_index
            .store(next_frame_index, Ordering::Release);

        swapchain.present(image)
    }
}

/// A per-window presentation surface (ARCHITECTURE.md Section 6's
/// referenced-but-undefined `RhiSwapchain`). Owns the "image available"
/// semaphore (Phase 0: one, reused every frame under the fully-synchronous
/// single-frame-in-flight model `VulkanDevice::begin_frame` enforces via
/// its fence wait).
pub struct VulkanSwapchain {
    surface_loader: ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
    swapchain_loader: ash::khr::swapchain::Device,
    swapchain: vk::SwapchainKHR,
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    format: vk::Format,
    width: u32,
    height: u32,
    image_available_semaphore: vk::Semaphore,
    /// One per swapchain image, indexed by acquired image index -- see
    /// `AcquiredImage::render_finished_semaphore_handle`'s doc comment in
    /// tre-engine for why this can't be a single shared instance.
    render_finished_semaphores: Vec<vk::Semaphore>,
    device: ash::Device,
    present_queue: vk::Queue,
}

impl VulkanSwapchain {
    pub fn new(
        device: &VulkanDevice,
        surface_loader: ash::khr::surface::Instance,
        surface: vk::SurfaceKHR,
        width: u32,
        height: u32,
    ) -> Result<Self, EngineError> {
        // SAFETY: `device.physical_device` and `surface` were both
        // selected/created during `VulkanDevice::new` (or, for additional
        // windows, `create_surface`) and are both still valid.
        let capabilities = unsafe {
            surface_loader.get_physical_device_surface_capabilities(device.physical_device, surface)
        }
        .map_err(|_| EngineError::DeviceLost)?;
        // SAFETY: same as above -- `device.physical_device`/`surface` are
        // a valid, still-alive pair.
        let formats = unsafe {
            surface_loader.get_physical_device_surface_formats(device.physical_device, surface)
        }
        .map_err(|_| EngineError::DeviceLost)?;
        let surface_format = formats
            .iter()
            .find(|f| f.format == vk::Format::B8G8R8A8_SRGB)
            .copied()
            .unwrap_or(formats[0]);

        let image_count =
            (capabilities.min_image_count + 1).min(if capabilities.max_image_count == 0 {
                u32::MAX
            } else {
                capabilities.max_image_count
            });

        let extent = vk::Extent2D { width, height };

        let swapchain_loader = ash::khr::swapchain::Device::new(&device.instance, &device.device);
        // SAFETY: `device.instance`/`device.device` (backing
        // `swapchain_loader`) are valid, `surface` is the same live
        // surface queried above, and `capabilities`/`surface_format` were
        // just queried against this exact physical device/surface pair,
        // so `image_count`/`image_format`/`pre_transform` etc. are all
        // values that pair validly with `surface`.
        let swapchain = unsafe {
            swapchain_loader.create_swapchain(
                &vk::SwapchainCreateInfoKHR::default()
                    .surface(surface)
                    .min_image_count(image_count)
                    .image_format(surface_format.format)
                    .image_color_space(surface_format.color_space)
                    .image_extent(extent)
                    .image_array_layers(1)
                    .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                    .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .pre_transform(capabilities.current_transform)
                    .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                    .present_mode(vk::PresentModeKHR::FIFO)
                    .clipped(true),
                None,
            )
        }
        .map_err(|_| EngineError::DeviceLost)?;

        // SAFETY: `swapchain` was just created above on this same loader.
        let images = unsafe { swapchain_loader.get_swapchain_images(swapchain) }
            .map_err(|_| EngineError::DeviceLost)?;

        let image_views = images
            .iter()
            .map(|&image| {
                // SAFETY: `device.device` is valid, and `image` comes from
                // `get_swapchain_images` above, so it is a live image
                // owned by this swapchain.
                unsafe {
                    device.device.create_image_view(
                        &vk::ImageViewCreateInfo::default()
                            .image(image)
                            .view_type(vk::ImageViewType::TYPE_2D)
                            .format(surface_format.format)
                            .subresource_range(
                                vk::ImageSubresourceRange::default()
                                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                                    .level_count(1)
                                    .layer_count(1),
                            ),
                        None,
                    )
                }
                .map_err(|_| EngineError::DeviceLost)
            })
            .collect::<Result<Vec<_>, _>>()?;

        // SAFETY: `device.device` is valid.
        let image_available_semaphore = unsafe {
            device
                .device
                .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
        }
        .map_err(|_| EngineError::DeviceLost)?;
        let render_finished_semaphores = images
            .iter()
            .map(|_| {
                // SAFETY: `device.device` is valid; one semaphore is
                // created per swapchain image so their indices line up
                // with acquired image indices (see the field doc comment
                // on `render_finished_semaphores`).
                unsafe {
                    device
                        .device
                        .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                }
                .map_err(|_| EngineError::DeviceLost)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            surface_loader,
            surface,
            swapchain_loader,
            swapchain,
            images,
            image_views,
            format: surface_format.format,
            width,
            height,
            image_available_semaphore,
            render_finished_semaphores,
            device: device.device.clone(),
            present_queue: device.graphics_queue(),
        })
    }

    pub fn format(&self) -> vk::Format {
        self.format
    }
}

impl RhiSwapchain for VulkanSwapchain {
    fn extent(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn acquire_next_image(&self) -> Result<AcquiredImage, EngineError> {
        // SAFETY: `self.swapchain` is valid, and `self.image_available_semaphore`
        // is not currently pending a wait -- under the single-frame-in-flight
        // model, `VulkanDevice::begin_frame`'s fence wait ensures the prior
        // frame's wait on this same semaphore has already completed before
        // a new frame acquires and signals it again.
        let (index, _suboptimal) = unsafe {
            self.swapchain_loader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                self.image_available_semaphore,
                vk::Fence::null(),
            )
        }
        .map_err(|e| {
            if e == vk::Result::ERROR_OUT_OF_DATE_KHR {
                EngineError::SwapchainOutOfDate
            } else {
                EngineError::DeviceLost
            }
        })?;

        Ok(AcquiredImage {
            index,
            target_view_handle: self.image_views[index as usize].as_raw(),
            target_image_handle: self.images[index as usize].as_raw(),
            image_available_semaphore_handle: self.image_available_semaphore.as_raw(),
            render_finished_semaphore_handle: self.render_finished_semaphores[index as usize]
                .as_raw(),
        })
    }

    fn present(&self, image: AcquiredImage) -> Result<(), EngineError> {
        let wait_semaphore = vk::Semaphore::from_raw(image.render_finished_semaphore_handle);
        let wait_semaphores = [wait_semaphore];
        let swapchains = [self.swapchain];
        let indices = [image.index];
        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&indices);

        // Present queue == graphics queue for Phase 0 (queried as both
        // graphics- and present-capable in `VulkanDevice::new`).
        // SAFETY: `self.present_queue` and `self.swapchain` are valid, and
        // `wait_semaphores`/`indices` come from the `AcquiredImage` this
        // same frame's `acquire_next_image` returned.
        match unsafe {
            self.swapchain_loader
                .queue_present(self.present_queue, &present_info)
        } {
            Ok(_) => Ok(()),
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {
                Err(EngineError::SwapchainOutOfDate)
            }
            Err(_) => Err(EngineError::DeviceLost),
        }
    }
}

impl Drop for VulkanSwapchain {
    fn drop(&mut self) {
        // SAFETY: `self` is being dropped, so no other code holds
        // references to these handles afterward; destroying the
        // semaphores and image views (children of the swapchain) before
        // the swapchain, and the swapchain before the surface, follows
        // Vulkan's required child-before-parent destruction order.
        unsafe {
            self.device
                .destroy_semaphore(self.image_available_semaphore, None);
            for &sem in &self.render_finished_semaphores {
                self.device.destroy_semaphore(sem, None);
            }
            for &view in &self.image_views {
                self.device.destroy_image_view(view, None);
            }
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
            self.surface_loader.destroy_surface(self.surface, None);
        }
    }
}

pub struct VulkanCommandBuffer {
    device: ash::Device,
    command_buffer: vk::CommandBuffer,
    width: u32,
    height: u32,
    pipeline_layout: Option<vk::PipelineLayout>,
}

impl RhiCommandBuffer for VulkanCommandBuffer {
    fn set_pipeline(&mut self, pipeline: &dyn RhiPipelineState) {
        let raw = vk::Pipeline::from_raw(pipeline.raw_handle());
        self.pipeline_layout = Some(vk::PipelineLayout::from_raw(pipeline.layout_handle()));
        // SAFETY: `self.command_buffer` is recording (allocated once and
        // reset/begun per frame by `VulkanDevice::begin_frame`), and `raw`
        // is a pipeline handle the `RhiPipelineState` trait contract
        // guarantees was created by this same device and is still alive.
        unsafe {
            self.device.cmd_bind_pipeline(
                self.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                raw,
            );
        }
    }

    fn set_scissor(&mut self, rect: &ScissorRect) {
        // SAFETY: `self.command_buffer` is recording, consistent with the
        // rest of this frame's commands.
        unsafe {
            self.device.cmd_set_scissor(
                self.command_buffer,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D {
                        x: rect.x,
                        y: rect.y,
                    },
                    extent: vk::Extent2D {
                        width: rect.width,
                        height: rect.height,
                    },
                }],
            );
        }
    }

    fn bind_vertex_buffer(&mut self, buffer: &dyn RhiBuffer, offset: u32) {
        let raw = vk::Buffer::from_raw(buffer.raw_handle());
        // SAFETY: `self.command_buffer` is recording, and `raw` is a
        // buffer handle the `RhiBuffer` trait contract guarantees was
        // created by this device and is still alive.
        unsafe {
            self.device
                .cmd_bind_vertex_buffers(self.command_buffer, 0, &[raw], &[offset as u64]);
        }
    }

    fn bind_index_buffer(&mut self, buffer: &dyn RhiBuffer, offset: u32) {
        let raw = vk::Buffer::from_raw(buffer.raw_handle());
        // SAFETY: `self.command_buffer` is recording, and `raw` is a
        // buffer handle the `RhiBuffer` trait contract guarantees is
        // valid and alive; `UINT32` matches how index buffers are
        // uploaded via `VulkanDevice::upload_buffer`.
        unsafe {
            self.device.cmd_bind_index_buffer(
                self.command_buffer,
                raw,
                offset as u64,
                vk::IndexType::UINT32,
            );
        }
    }

    fn bind_texture(&mut self, _slot: u32, _bindless_index: u32) {
        unimplemented!("Phase 4 (bindless atlas textures) -- out of Phase 0's scope")
    }

    fn draw_indexed(&mut self, index_count: u32, start_index: u32, base_vertex: i32) {
        // Phase 0 has no transform stack yet (IMPLEMENTATION.md Phase 3);
        // push the screen size the vertex shader needs to map pixel-space
        // positions to NDC.
        let push = [self.width as f32, self.height as f32];
        // SAFETY: `self.command_buffer` is recording; `self.pipeline_layout`
        // was set by `set_pipeline` (asserted via `.expect` above) and
        // matches the layout `create_pipeline` declared its push constant
        // range against, and `push`'s 8-byte size matches the 8-byte
        // range reserved there.
        unsafe {
            self.device.cmd_push_constants(
                self.command_buffer,
                self.pipeline_layout
                    .expect("set_pipeline must be called before draw_indexed"),
                vk::ShaderStageFlags::VERTEX,
                0,
                bytemuck::bytes_of(&push),
            );
            self.device.cmd_draw_indexed(
                self.command_buffer,
                index_count,
                1,
                start_index,
                base_vertex,
                0,
            );
        }
    }

    fn raw_handle(&self) -> u64 {
        self.command_buffer.as_raw()
    }
}

pub struct VulkanBuffer {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    device: ash::Device,
}

impl RhiBuffer for VulkanBuffer {
    fn raw_handle(&self) -> u64 {
        self.buffer.as_raw()
    }
}

impl Drop for VulkanBuffer {
    fn drop(&mut self) {
        // SAFETY: `self` is being dropped, so no other code holds
        // references to `self.buffer`/`self.memory` afterward; destroying
        // the buffer before freeing the memory it was bound to follows
        // Vulkan's required order.
        unsafe {
            self.device.destroy_buffer(self.buffer, None);
            self.device.free_memory(self.memory, None);
        }
    }
}

pub struct VulkanPipelineState {
    pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    device: ash::Device,
}

impl RhiPipelineState for VulkanPipelineState {
    fn raw_handle(&self) -> u64 {
        self.pipeline.as_raw()
    }

    fn layout_handle(&self) -> u64 {
        self.layout.as_raw()
    }
}

impl Drop for VulkanPipelineState {
    fn drop(&mut self) {
        // SAFETY: `self` is being dropped, so no other code holds
        // references to `self.pipeline`/`self.layout` afterward.
        unsafe {
            self.device.destroy_pipeline(self.pipeline, None);
            self.device.destroy_pipeline_layout(self.layout, None);
        }
    }
}

/// Diagnostic counters for `VulkanDevice`'s transient render target pool
/// (TECHNICAL.md Section 3.2), exposed so demos/tests can prove
/// steady-state pool reuse (a `hits`-only-growing, `misses`-flat pattern
/// after warmup) rather than asserting on internal state.
#[derive(Debug, Default, Clone, Copy)]
pub struct TransientPoolStats {
    pub hits: u64,
    pub misses: u64,
}

#[derive(Default)]
struct TransientPool {
    /// Checked-in (available) textures, bucketed by power-of-two
    /// `(width, height)` plus format.
    free: HashMap<(u32, u32, TextureFormat), Vec<VulkanTexture>>,
    /// Exact buckets a miss needs grown at the start of the next frame
    /// (deduplicated -- see the `contains` check at the push site).
    pending_growth: Vec<(u32, u32, TextureFormat)>,
    stats: TransientPoolStats,
}

/// A GPU render target (TECHNICAL.md Section 3.2's transient pool
/// entries). See `RhiTexture`'s doc comment for why it exposes three
/// separate opaque handles rather than one.
pub struct VulkanTexture {
    image: vk::Image,
    view: vk::ImageView,
    memory: vk::DeviceMemory,
    width: u32,
    height: u32,
    format: TextureFormat,
    device: ash::Device,
}

impl VulkanTexture {
    fn new(
        device: &VulkanDevice,
        width: u32,
        height: u32,
        format: TextureFormat,
    ) -> Result<Self, EngineError> {
        let vk_format = texture_format_to_vk(format);

        // SAFETY: `device.device` is valid, and `width`/`height` are
        // non-zero (`next_power_of_two` of any caller-supplied dimension
        // is at least 1) as `VkImageCreateInfo` requires.
        let image = unsafe {
            device.device.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk_format)
                    .extent(vk::Extent3D {
                        width,
                        height,
                        depth: 1,
                    })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .initial_layout(vk::ImageLayout::UNDEFINED),
                None,
            )
        }
        .map_err(|_| EngineError::DeviceLost)?;

        // SAFETY: `image` was just created above on this device.
        let requirements = unsafe { device.device.get_image_memory_requirements(image) };
        // SAFETY: `device.physical_device` is the device selected in
        // `VulkanDevice::new` and is valid for as long as `device.instance`
        // (also alive here) is.
        let memory_properties = unsafe {
            device
                .instance
                .get_physical_device_memory_properties(device.physical_device)
        };
        let memory_type_index = (0..memory_properties.memory_type_count)
            .find(|&i| {
                (requirements.memory_type_bits & (1 << i)) != 0
                    && memory_properties.memory_types[i as usize]
                        .property_flags
                        .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
            })
            .ok_or(EngineError::DeviceLost)?;

        // SAFETY: `device.device` is valid, `requirements.size` comes
        // directly from `get_image_memory_requirements` above, and
        // `memory_type_index` was selected from the `find` above so it is
        // one of the bits set in `requirements.memory_type_bits`.
        let memory = unsafe {
            device.device.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(requirements.size)
                    .memory_type_index(memory_type_index),
                None,
            )
        }
        .map_err(|_| EngineError::DeviceLost)?;

        // SAFETY: `image` and `memory` were both just created above on
        // this device, and `image` has not been bound to memory before
        // now.
        unsafe {
            device
                .device
                .bind_image_memory(image, memory, 0)
                .map_err(|_| EngineError::DeviceLost)?;
        }

        // SAFETY: `device.device` is valid, and `image` was just bound to
        // `memory` immediately above, so creating a view of it now is
        // valid.
        let view = unsafe {
            device.device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(vk_format)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .level_count(1)
                            .layer_count(1),
                    ),
                None,
            )
        }
        .map_err(|_| EngineError::DeviceLost)?;

        Ok(Self {
            image,
            view,
            memory,
            width,
            height,
            format,
            device: device.device.clone(),
        })
    }
}

impl RhiTexture for VulkanTexture {
    fn raw_handle(&self) -> u64 {
        self.view.as_raw()
    }

    fn image_handle(&self) -> u64 {
        self.image.as_raw()
    }

    fn memory_handle(&self) -> u64 {
        self.memory.as_raw()
    }

    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn format(&self) -> TextureFormat {
        self.format
    }
}

impl Drop for VulkanTexture {
    fn drop(&mut self) {
        // SAFETY: `self` is being dropped, so no other code holds
        // references to `self.view`/`self.image`/`self.memory` afterward;
        // destroying the view before the image, and the image before
        // freeing the memory it was bound to, follows Vulkan's required
        // child-before-parent destruction order.
        unsafe {
            self.device.destroy_image_view(self.view, None);
            self.device.destroy_image(self.image, None);
            self.device.free_memory(self.memory, None);
        }
    }
}

struct RingBufferState {
    /// The `FrameSync::frame_index` value this segment's `cursor` was
    /// last reset for. When `write` observes a different current index,
    /// that means a new frame has begun (per the "`VulkanDevice::
    /// begin_frame` is always called before this buffer's `write` each
    /// frame" calling convention -- see `VulkanRingBuffer`'s doc comment)
    /// and the new segment's cursor starts over at 0.
    last_seen_frame_index: usize,
    cursor: usize,
}

/// TECHNICAL.md Section 3.1's triple-buffered dynamic ring buffer: one
/// host-coherent `VkBuffer`, persistently mapped once at construction,
/// divided into `FRAMES_IN_FLIGHT` equal segments.
///
/// Calling convention: call `VulkanDevice::begin_frame` before any
/// `write` calls for a given frame. This buffer has no fence-wait of its
/// own -- it shares `VulkanDevice`'s `FrameSync` purely to read which
/// segment is current, trusting that `begin_frame`'s own wait already
/// guaranteed that segment's prior GPU usage is complete (waiting on the
/// same fence a second time here would deadlock, since `begin_frame`
/// already reset it to unsignaled in preparation for this frame's own
/// submission).
pub struct VulkanRingBuffer {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    mapped_ptr: *mut u8,
    segment_size: usize,
    frame_sync: Arc<FrameSync>,
    state: Mutex<RingBufferState>,
    device: ash::Device,
}

// SAFETY: `mapped_ptr` is only ever dereferenced inside `write`, which is
// guarded by `state`'s `Mutex`, and the pointer stays valid for this
// buffer's entire lifetime (mapped once in `new`, unmapped only in
// `Drop`) -- there is no unsynchronized access to the raw pointer this
// auto-trait would otherwise (correctly) forbid.
unsafe impl Send for VulkanRingBuffer {}
unsafe impl Sync for VulkanRingBuffer {}

impl VulkanRingBuffer {
    fn new(device: &VulkanDevice, capacity: usize) -> Result<Self, EngineError> {
        let segment_size = align_up(capacity.div_ceil(FRAMES_IN_FLIGHT), RING_BUFFER_ALIGNMENT);
        let total_size = segment_size * FRAMES_IN_FLIGHT;

        // SAFETY: `device.device` is valid, and `total_size` is used
        // directly as `size` so the create info describes exactly this
        // buffer's full triple-segment span.
        let buffer = unsafe {
            device.device.create_buffer(
                &vk::BufferCreateInfo::default()
                    .size(total_size as u64)
                    .usage(
                        vk::BufferUsageFlags::VERTEX_BUFFER
                            | vk::BufferUsageFlags::INDEX_BUFFER
                            | vk::BufferUsageFlags::UNIFORM_BUFFER,
                    )
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            )
        }
        .map_err(|_| EngineError::DeviceLost)?;

        // SAFETY: `buffer` was just created above on this device.
        let requirements = unsafe { device.device.get_buffer_memory_requirements(buffer) };
        // SAFETY: `device.physical_device` is valid for as long as
        // `device.instance` (also alive here) is.
        let memory_properties = unsafe {
            device
                .instance
                .get_physical_device_memory_properties(device.physical_device)
        };
        let wanted = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
        let memory_type_index = (0..memory_properties.memory_type_count)
            .find(|&i| {
                (requirements.memory_type_bits & (1 << i)) != 0
                    && memory_properties.memory_types[i as usize]
                        .property_flags
                        .contains(wanted)
            })
            .ok_or(EngineError::DeviceLost)?;

        // SAFETY: `device.device` is valid, `requirements.size` comes
        // directly from `get_buffer_memory_requirements` above, and
        // `memory_type_index` was selected from the `find` above so it is
        // one of the bits set in `requirements.memory_type_bits`.
        let memory = unsafe {
            device.device.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(requirements.size)
                    .memory_type_index(memory_type_index),
                None,
            )
        }
        .map_err(|_| EngineError::DeviceLost)?;

        // SAFETY: `buffer`/`memory` were both just created above on this
        // device, `buffer` has not been bound to memory before now, and
        // `memory` was allocated as host-visible/host-coherent (selected
        // via `wanted` above), so mapping it is valid. The returned
        // pointer is kept for this struct's entire lifetime (persistent
        // mapping, matching TECHNICAL.md Section 3.1's design rather than
        // `upload_buffer`'s Phase-0 map-write-unmap-once pattern) and
        // unmapped exactly once, in `Drop`.
        let mapped_ptr = unsafe {
            device
                .device
                .bind_buffer_memory(buffer, memory, 0)
                .map_err(|_| EngineError::DeviceLost)?;
            device
                .device
                .map_memory(memory, 0, total_size as u64, vk::MemoryMapFlags::empty())
                .map_err(|_| EngineError::DeviceLost)? as *mut u8
        };

        Ok(Self {
            buffer,
            memory,
            mapped_ptr,
            segment_size,
            frame_sync: device.frame_sync.clone(),
            state: Mutex::new(RingBufferState {
                last_seen_frame_index: usize::MAX,
                cursor: 0,
            }),
            device: device.device.clone(),
        })
    }
}

impl RhiBuffer for VulkanRingBuffer {
    fn raw_handle(&self) -> u64 {
        self.buffer.as_raw()
    }
}

impl RhiDynamicRingBuffer for VulkanRingBuffer {
    fn write(&self, bytes: &[u8]) -> Option<u32> {
        let frame_index = self.frame_sync.frame_index.load(Ordering::Acquire);
        let mut state = self.state.lock().expect("ring buffer state poisoned");
        if state.last_seen_frame_index != frame_index {
            state.last_seen_frame_index = frame_index;
            state.cursor = 0;
        }

        let aligned_len = align_up(bytes.len(), RING_BUFFER_ALIGNMENT);
        if state.cursor + aligned_len > self.segment_size {
            return None; // DESIGN.md Section 2.6: starvation is reported, not grown mid-frame.
        }

        let segment_base = frame_index * self.segment_size;
        let offset = segment_base + state.cursor;
        // SAFETY: `self.mapped_ptr` is valid for `segment_size *
        // FRAMES_IN_FLIGHT` bytes for this struct's whole lifetime; `offset
        // + bytes.len() <= offset + aligned_len <= segment_base +
        // self.segment_size <= total mapped size` (checked above), and
        // `state`'s `MutexGuard` makes this the only writer touching
        // `mapped_ptr` at a time.
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), self.mapped_ptr.add(offset), bytes.len());
        }
        state.cursor += aligned_len;

        u32::try_from(offset).ok()
    }
}

impl Drop for VulkanRingBuffer {
    fn drop(&mut self) {
        // SAFETY: `self` is being dropped, so no other code holds
        // references to `self.mapped_ptr` afterward; unmapping before
        // destroying the buffer and freeing its memory follows Vulkan's
        // required order.
        unsafe {
            self.device.unmap_memory(self.memory);
            self.device.destroy_buffer(self.buffer, None);
            self.device.free_memory(self.memory, None);
        }
    }
}
