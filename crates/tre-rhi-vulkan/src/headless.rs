//! Headless (zero-window) rendering (DESIGN.md Section 4.3, IMPLEMENTATION.md
//! Step 1.1 task 4): a `RhiSwapchain` backed by a plain `VkImage` with
//! `TRANSFER_SRC` usage instead of a real `VkSwapchainKHR`, whose `present`
//! reads the rendered image back to a host-visible staging buffer instead
//! of calling `vkQueuePresentKHR`. Proves the Phase 0 trait design
//! generalizes to a fundamentally different backing with zero trait
//! changes -- the whole point of defining `RhiSwapchain` as a trait.

use ash::vk;
use ash::vk::Handle;
use tre_engine::{AcquiredImage, EngineError, RhiSwapchain};

use crate::VulkanDevice;

pub struct HeadlessSwapchain {
    device: ash::Device,
    queue: vk::Queue,
    width: u32,
    height: u32,
    image: vk::Image,
    image_view: vk::ImageView,
    image_memory: vk::DeviceMemory,
    /// This swapchain's own stencil image (IMPLEMENTATION.md Step 3.3.3),
    /// sized to `width`/`height` like `image` above -- owned per-swapchain
    /// rather than once on `VulkanDevice`, since different swapchains
    /// (e.g. `multi_window`'s two windows) can have different extents.
    stencil_image: vk::Image,
    stencil_image_view: vk::ImageView,
    stencil_image_memory: vk::DeviceMemory,
    staging_buffer: vk::Buffer,
    staging_memory: vk::DeviceMemory,
    staging_size: u64,
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    image_available_semaphore: vk::Semaphore,
    render_finished_semaphore: vk::Semaphore,
    readback_fence: vk::Fence,
}

/// Same format the real swapchain prefers (`VulkanSwapchain::new`) so a
/// scene renders identically whether windowed or headless.
pub const HEADLESS_FORMAT: vk::Format = vk::Format::B8G8R8A8_SRGB;

impl HeadlessSwapchain {
    pub fn new(device: &VulkanDevice, width: u32, height: u32) -> Result<Self, EngineError> {
        let raw_device = &device.device;

        // SAFETY: `raw_device` is `device.device`, valid for the life of
        // `device`, and the `ImageCreateInfo` only references locals
        // (`width`/`height`) that outlive this call.
        let image = unsafe {
            raw_device.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(HEADLESS_FORMAT)
                    .extent(vk::Extent3D {
                        width,
                        height,
                        depth: 1,
                    })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(
                        vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC,
                    )
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .initial_layout(vk::ImageLayout::UNDEFINED),
                None,
            )
        }
        .map_err(|_| EngineError::DeviceLost)?;

        let image_memory = allocate_and_bind_image(device, raw_device, image)?;

        // SAFETY: `raw_device` is valid, and `image` was just created and
        // bound to memory above (`allocate_and_bind_image`), so it is a
        // valid, memory-backed image.
        let image_view = unsafe {
            raw_device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(HEADLESS_FORMAT)
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

        // IMPLEMENTATION.md Step 3.3.3: this swapchain's own stencil
        // image, sized to match its own extent -- mirrors the color
        // image's creation above exactly, just with `device.stencil_format`
        // and `DEPTH_STENCIL_ATTACHMENT` usage instead.
        //
        // SAFETY: `raw_device` is valid, and `ImageCreateInfo` only
        // references locals (`device.stencil_format`, `width`/`height`)
        // that outlive this call.
        let stencil_image = unsafe {
            raw_device.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(device.stencil_format)
                    .extent(vk::Extent3D {
                        width,
                        height,
                        depth: 1,
                    })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .initial_layout(vk::ImageLayout::UNDEFINED),
                None,
            )
        }
        .map_err(|_| EngineError::DeviceLost)?;
        let stencil_image_memory = allocate_and_bind_image(device, raw_device, stencil_image)?;
        // SAFETY: `raw_device` is valid, and `stencil_image` was just
        // created and bound to memory above.
        let stencil_image_view = unsafe {
            raw_device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(stencil_image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(device.stencil_format)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::STENCIL)
                            .level_count(1)
                            .layer_count(1),
                    ),
                None,
            )
        }
        .map_err(|_| EngineError::DeviceLost)?;

        let staging_size = u64::from(width) * u64::from(height) * 4;
        let (staging_buffer, staging_memory) =
            create_staging_buffer(device, raw_device, staging_size)?;

        // SAFETY: `raw_device` is valid, and `device.queue_family_index`
        // is the same graphics-capable family selected in
        // `VulkanDevice::new`.
        let command_pool = unsafe {
            raw_device.create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(device.queue_family_index)
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )
        }
        .map_err(|_| EngineError::DeviceLost)?;
        // SAFETY: `command_pool` was just created above on this same
        // `raw_device`.
        let command_buffer = unsafe {
            raw_device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
        }
        .map_err(|_| EngineError::DeviceLost)?[0];

        // SAFETY: `raw_device` is valid.
        let image_available_semaphore =
            unsafe { raw_device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) }
                .map_err(|_| EngineError::DeviceLost)?;
        // SAFETY: `raw_device` is valid.
        let render_finished_semaphore =
            unsafe { raw_device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) }
                .map_err(|_| EngineError::DeviceLost)?;
        // SAFETY: `raw_device` is valid.
        let readback_fence =
            unsafe { raw_device.create_fence(&vk::FenceCreateInfo::default(), None) }
                .map_err(|_| EngineError::DeviceLost)?;

        Ok(Self {
            device: raw_device.clone(),
            queue: device.graphics_queue(),
            width,
            height,
            image,
            image_view,
            image_memory,
            stencil_image,
            stencil_image_view,
            stencil_image_memory,
            staging_buffer,
            staging_memory,
            staging_size,
            command_pool,
            command_buffer,
            image_available_semaphore,
            render_finished_semaphore,
            readback_fence,
        })
    }

    /// Reads back the last presented frame as tightly-packed `B8G8R8A8`
    /// bytes (matching [`HEADLESS_FORMAT`]). Call after
    /// `RhiDevice::submit_and_present` for the frame you want to inspect.
    ///
    /// # Errors
    /// Returns [`EngineError::DeviceLost`] if the memory map fails.
    pub fn read_pixels_bgra8(&self) -> Result<Vec<u8>, EngineError> {
        // SAFETY: `self.staging_memory` was allocated as host-visible/
        // host-coherent and sized to `self.staging_size` bytes
        // (`create_staging_buffer`) and is not mapped elsewhere; `ptr` is
        // therefore valid for `self.staging_size` bytes, matching `out`'s
        // length, so `copy_nonoverlapping`'s read/write stays in bounds,
        // and `unmap_memory` is called exactly once right after to end the
        // mapping.
        unsafe {
            let ptr = self
                .device
                .map_memory(
                    self.staging_memory,
                    0,
                    self.staging_size,
                    vk::MemoryMapFlags::empty(),
                )
                .map_err(|_| EngineError::DeviceLost)?;
            let mut out = vec![0u8; self.staging_size as usize];
            std::ptr::copy_nonoverlapping(ptr.cast::<u8>(), out.as_mut_ptr(), out.len());
            self.device.unmap_memory(self.staging_memory);
            Ok(out)
        }
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }
}

impl RhiSwapchain for HeadlessSwapchain {
    fn extent(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn stencil_view_handle(&self) -> u64 {
        self.stencil_image_view.as_raw()
    }

    fn stencil_image_handle(&self) -> u64 {
        self.stencil_image.as_raw()
    }

    fn acquire_next_image(&self) -> Result<AcquiredImage, EngineError> {
        // Headless has no async acquire -- the one image is always ready.
        // Re-signal `image_available_semaphore` via a trivial empty submit
        // so `RhiDevice::submit_and_present`'s wait on it behaves exactly
        // as it would for a real swapchain (semaphores are consumed on
        // wait, so this must happen fresh every frame).
        let signal_semaphores = [self.image_available_semaphore];
        let submit = vk::SubmitInfo::default().signal_semaphores(&signal_semaphores);
        // SAFETY: `self.queue` is valid, and `signal_semaphores` contains
        // only `self.image_available_semaphore`, which is not currently
        // pending a wait -- headless has no concurrent frame in flight
        // (mirroring the windowed swapchain's single-frame-in-flight
        // model), and `vk::Fence::null()` is a valid null handle meaning
        // "no fence".
        unsafe {
            self.device
                .queue_submit(self.queue, &[submit], vk::Fence::null())
        }
        .map_err(|_| EngineError::DeviceLost)?;

        Ok(AcquiredImage {
            index: 0,
            target_view_handle: self.image_view.as_raw(),
            target_image_handle: self.image.as_raw(),
            image_available_semaphore_handle: self.image_available_semaphore.as_raw(),
            render_finished_semaphore_handle: self.render_finished_semaphore.as_raw(),
        })
    }

    fn present(&self, _image: AcquiredImage) -> Result<(), EngineError> {
        // No presentation engine to hand off to -- instead, copy the
        // rendered image to the host-visible staging buffer so
        // `read_pixels_bgra8` can return it.
        //
        // SAFETY: `self.command_buffer`/`self.command_pool` were allocated
        // once in `new` and this type is only ever driven from one thread
        // at a time, so reset/record/submit/wait below cannot race another
        // use of the same command buffer. `region`'s extent matches
        // `self.width`/`self.height`, which is also what `self.staging_buffer`
        // was sized for in `new`, so the buffer-to-image copy stays in
        // bounds. The whole sequence -- record, submit, and wait on
        // `self.readback_fence` -- completes before this call returns, so
        // no command buffer or fence is left in a state a later call could
        // race with.
        unsafe {
            self.device
                .reset_command_buffer(self.command_buffer, vk::CommandBufferResetFlags::empty())
                .map_err(|_| EngineError::DeviceLost)?;
            self.device
                .begin_command_buffer(self.command_buffer, &vk::CommandBufferBeginInfo::default())
                .map_err(|_| EngineError::DeviceLost)?;

            // `RhiDevice::submit_and_present`'s shared code (in
            // `VulkanDevice::submit_and_present`) always transitions the
            // rendered image to `PRESENT_SRC_KHR` before ending its command
            // buffer, regardless of which concrete `RhiSwapchain` is in
            // use -- that transition is correct for a real presentable
            // swapchain, but by the time control reaches here the image is
            // actually in `PRESENT_SRC_KHR`, not `COLOR_ATTACHMENT_OPTIMAL`.
            // This barrier must start from what the image really is, not
            // what a windowed swapchain would leave it as; using
            // `PRESENT_SRC_KHR` as a transient layout for a plain
            // (non-swapchain) image is unusual but valid -- it is still
            // just a layout tag, and no swapchain-specific object identity
            // is implied by it. A cleaner fix -- letting each concrete
            // `RhiSwapchain` control its own post-render transition instead
            // of hardcoding one in `submit_and_present` -- is a real
            // interface refinement worth making before Phase 2 builds
            // more swapchain variants on top of this; see
            // documentation/REVIEW.md's Phase 1 Step 1 entry.
            let to_transfer_src = vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .src_access_mask(vk::AccessFlags::empty())
                .dst_access_mask(vk::AccessFlags::TRANSFER_READ)
                .image(self.image)
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .level_count(1)
                        .layer_count(1),
                );
            self.device.cmd_pipeline_barrier(
                self.command_buffer,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[to_transfer_src],
            );

            let region = vk::BufferImageCopy::default()
                .image_subresource(
                    vk::ImageSubresourceLayers::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .layer_count(1),
                )
                .image_extent(vk::Extent3D {
                    width: self.width,
                    height: self.height,
                    depth: 1,
                });
            self.device.cmd_copy_image_to_buffer(
                self.command_buffer,
                self.image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                self.staging_buffer,
                &[region],
            );

            self.device
                .end_command_buffer(self.command_buffer)
                .map_err(|_| EngineError::DeviceLost)?;

            let wait_semaphores = [self.render_finished_semaphore];
            let wait_stages = [vk::PipelineStageFlags::TRANSFER];
            let command_buffers = [self.command_buffer];
            let submit = vk::SubmitInfo::default()
                .wait_semaphores(&wait_semaphores)
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(&command_buffers);
            self.device
                .queue_submit(self.queue, &[submit], self.readback_fence)
                .map_err(|_| EngineError::DeviceLost)?;
            self.device
                .wait_for_fences(&[self.readback_fence], true, u64::MAX)
                .map_err(|_| EngineError::DeviceLost)?;
            self.device
                .reset_fences(&[self.readback_fence])
                .map_err(|_| EngineError::DeviceLost)?;
        }
        Ok(())
    }
}

impl Drop for HeadlessSwapchain {
    fn drop(&mut self) {
        // SAFETY: `self` is being dropped; `device_wait_idle` below
        // ensures no GPU work still references these handles before
        // they're destroyed, and destroying the image view/buffer before
        // freeing their bound memory, and the command pool after all
        // commands using it have completed, follows Vulkan's required
        // order.
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_fence(self.readback_fence, None);
            self.device
                .destroy_semaphore(self.render_finished_semaphore, None);
            self.device
                .destroy_semaphore(self.image_available_semaphore, None);
            self.device.destroy_command_pool(self.command_pool, None);
            self.device.destroy_buffer(self.staging_buffer, None);
            self.device.free_memory(self.staging_memory, None);
            self.device.destroy_image_view(self.image_view, None);
            self.device.destroy_image(self.image, None);
            self.device.free_memory(self.image_memory, None);
            self.device
                .destroy_image_view(self.stencil_image_view, None);
            self.device.destroy_image(self.stencil_image, None);
            self.device.free_memory(self.stencil_image_memory, None);
        }
    }
}

/// `pub(crate)` (not just private) so `VulkanSwapchain::new` (lib.rs) can
/// reuse this exact same allocate-and-bind logic for its own stencil
/// image (IMPLEMENTATION.md Step 3.3.3) instead of duplicating it a
/// third time.
pub(crate) fn allocate_and_bind_image(
    device: &VulkanDevice,
    raw_device: &ash::Device,
    image: vk::Image,
) -> Result<vk::DeviceMemory, EngineError> {
    // SAFETY: `image` was just created by the caller
    // (`HeadlessSwapchain::new`) and passed in still unbound, so its
    // handle is valid on `raw_device`.
    let requirements = unsafe { raw_device.get_image_memory_requirements(image) };
    let memory_type_index = find_memory_type(
        device,
        requirements.memory_type_bits,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )?;
    // SAFETY: `raw_device` is valid, `requirements.size` comes directly
    // from `get_image_memory_requirements` above, and `memory_type_index`
    // was selected from `requirements.memory_type_bits` by
    // `find_memory_type`.
    let memory = unsafe {
        raw_device.allocate_memory(
            &vk::MemoryAllocateInfo::default()
                .allocation_size(requirements.size)
                .memory_type_index(memory_type_index),
            None,
        )
    }
    .map_err(|_| EngineError::DeviceLost)?;
    // SAFETY: `image` and `memory` were both just created above on this
    // same `raw_device`, and `image` has not been bound to memory before
    // now.
    unsafe { raw_device.bind_image_memory(image, memory, 0) }
        .map_err(|_| EngineError::DeviceLost)?;
    Ok(memory)
}

fn create_staging_buffer(
    device: &VulkanDevice,
    raw_device: &ash::Device,
    size: u64,
) -> Result<(vk::Buffer, vk::DeviceMemory), EngineError> {
    // SAFETY: `raw_device` is valid, and `size` is used directly as the
    // create info's `size`.
    let buffer = unsafe {
        raw_device.create_buffer(
            &vk::BufferCreateInfo::default()
                .size(size)
                .usage(vk::BufferUsageFlags::TRANSFER_DST)
                .sharing_mode(vk::SharingMode::EXCLUSIVE),
            None,
        )
    }
    .map_err(|_| EngineError::DeviceLost)?;
    // SAFETY: `buffer` was just created above on this `raw_device`.
    let requirements = unsafe { raw_device.get_buffer_memory_requirements(buffer) };
    let wanted = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
    let memory_type_index = find_memory_type(device, requirements.memory_type_bits, wanted)?;
    // SAFETY: `raw_device` is valid, `requirements.size` comes directly
    // from `get_buffer_memory_requirements` above, and `memory_type_index`
    // was selected from `requirements.memory_type_bits` by
    // `find_memory_type`.
    let memory = unsafe {
        raw_device.allocate_memory(
            &vk::MemoryAllocateInfo::default()
                .allocation_size(requirements.size)
                .memory_type_index(memory_type_index),
            None,
        )
    }
    .map_err(|_| EngineError::DeviceLost)?;
    // SAFETY: `buffer` and `memory` were both just created above on this
    // same `raw_device`, and `buffer` has not been bound to memory before
    // now.
    unsafe { raw_device.bind_buffer_memory(buffer, memory, 0) }
        .map_err(|_| EngineError::DeviceLost)?;
    Ok((buffer, memory))
}

fn find_memory_type(
    device: &VulkanDevice,
    type_bits: u32,
    wanted: vk::MemoryPropertyFlags,
) -> Result<u32, EngineError> {
    // SAFETY: `device.physical_device` is the device selected in
    // `VulkanDevice::new`, and `device.instance` (which owns it) is still
    // alive here.
    let memory_properties = unsafe {
        device
            .instance
            .get_physical_device_memory_properties(device.physical_device)
    };
    (0..memory_properties.memory_type_count)
        .find(|&i| {
            (type_bits & (1 << i)) != 0
                && memory_properties.memory_types[i as usize]
                    .property_flags
                    .contains(wanted)
        })
        .ok_or(EngineError::DeviceLost)
}
