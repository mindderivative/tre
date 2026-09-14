//! `VulkanTexture` -- the `RhiTexture` implementation backing both
//! `RhiDevice::create_texture` (bindless-registered, real pixel data)
//! and `RhiDevice::acquire_transient_target` (pooled render targets,
//! TECHNICAL.md Section 3.2). `PendingImage`/`PendingCommandBuffer` are
//! its own construction-time drop guards (Phase 2 Code Review finding
//! #68). Split out of `lib.rs` as one of its ten separable concerns
//! (Architecture review finding).

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use ash::vk;
use ash::vk::Handle;
use tre_engine::{EngineError, RhiTexture, TextureFormat};

use crate::device::DeviceOwner;
use crate::transient_pool::BindlessRegistry;
use crate::{bytes_per_pixel, texture_format_to_vk, VulkanDevice};

/// A GPU render target (TECHNICAL.md Section 3.2's transient pool
/// entries). See `RhiTexture`'s doc comment for why it exposes three
/// separate opaque handles rather than one.
pub struct VulkanTexture {
    pub(crate) image: vk::Image,
    pub(crate) view: vk::ImageView,
    pub(crate) memory: vk::DeviceMemory,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) format: TextureFormat,
    pub(crate) device: ash::Device,
    /// Keep-alive for the owning device (REVIEW.md finding #259). This
    /// texture's `Drop` frees its GPU image through the cloned `device`
    /// handle above, which dangles the instant the real `VkDevice` is
    /// destroyed. A handed-out texture (bindless, from `create_texture`)
    /// can outlive the `VulkanDevice` at teardown, so holding the device's
    /// `Arc<DeviceOwner>` defers `vkDestroyDevice` until this texture is
    /// gone too. Never read.
    #[allow(dead_code, reason = "keep-alive only; see doc comment")]
    pub(crate) _owner: Arc<DeviceOwner>,
    /// This texture's slot in the bindless array (IMPLEMENTATION.md
    /// Step 2.1), if it has one. `None` for a transient render target
    /// (`VulkanTexture::new`) -- only `VulkanTexture::from_pixels`
    /// (`RhiDevice::create_texture`'s backing) registers one.
    pub(crate) bindless_index: Option<u32>,
    /// A clone of the owning `VulkanDevice`'s registry `Arc`, so `Drop` can
    /// release `bindless_index` back to the free list without holding a
    /// reference to the whole device. `None` exactly when `bindless_index`
    /// is `None`.
    pub(crate) bindless_registry: Option<Arc<Mutex<BindlessRegistry>>>,
    /// IMPLEMENTATION.md Step 2.3: the `FrameSync::total_frame_count` value
    /// as of this texture's creation, or (for a pooled texture) its last
    /// `release_transient_target` check-in -- what the GC thread compares
    /// against the current frame count to judge staleness. Meaningless for
    /// a bindless texture (`from_pixels`'s output), which never enters
    /// `TransientPool::free` and so is never scanned.
    pub(crate) last_used_frame: u64,
    /// This texture's own `VkMemoryRequirements::size` -- what
    /// `TransientPool::total_free_bytes` sums to decide whether the pool
    /// has crossed IMPLEMENTATION.md Step 2.3's 85%-of-budget GC trigger.
    pub(crate) size_bytes: u64,
}

/// Guards an in-progress sampled image's `image`/`memory`/`view` between
/// creation and `VulkanTexture::from_pixels`'s success -- destroying
/// whichever of them exist if dropped early (any of that function's
/// several fallible steps returning via `?`) instead of leaking GPU memory
/// (Phase 2 Code Review finding #68).
struct PendingImage {
    device: ash::Device,
    image: vk::Image,
    memory: Option<vk::DeviceMemory>,
    view: Option<vk::ImageView>,
}

impl PendingImage {
    /// Claims the three handles without running `Drop` -- call only once
    /// nothing further in `from_pixels` can fail.
    fn into_parts(self) -> (vk::Image, vk::ImageView, vk::DeviceMemory) {
        let parts = (
            self.image,
            self.view
                .expect("view assigned before into_parts is called"),
            self.memory
                .expect("memory assigned before into_parts is called"),
        );
        std::mem::forget(self);
        parts
    }
}

impl Drop for PendingImage {
    fn drop(&mut self) {
        // SAFETY: only reached when `from_pixels` abandons this image
        // before `into_parts` claims it, so nothing else references these
        // handles; `self.view`/`self.memory` are `None` only if abandoned
        // before that step ran, hence the guards below.
        unsafe {
            if let Some(view) = self.view {
                self.device.destroy_image_view(view, None);
            }
            self.device.destroy_image(self.image, None);
            if let Some(memory) = self.memory {
                self.device.free_memory(memory, None);
            }
        }
    }
}

/// Guards a one-time upload command buffer between allocation and
/// `VulkanTexture::from_pixels`'s successful submission -- freeing it if
/// dropped early instead of leaking it from the (limited-capacity) command
/// pool it was allocated from (Phase 2 Code Review finding #68).
struct PendingCommandBuffer {
    device: ash::Device,
    pool: vk::CommandPool,
    buffer: vk::CommandBuffer,
}

impl PendingCommandBuffer {
    /// Claims the command buffer without running `Drop` -- call only once
    /// it has been successfully submitted and waited on.
    fn into_inner(self) -> vk::CommandBuffer {
        let buffer = self.buffer;
        std::mem::forget(self);
        buffer
    }
}

impl Drop for PendingCommandBuffer {
    fn drop(&mut self) {
        // SAFETY: only reached when `from_pixels` abandons this command
        // buffer before `into_inner` claims it; `self.pool` is the same
        // pool it was allocated from and is still valid.
        unsafe {
            self.device.free_command_buffers(self.pool, &[self.buffer]);
        }
    }
}

impl VulkanTexture {
    pub(crate) fn new(
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
            _owner: Arc::clone(&device.owner),
            bindless_index: None,
            bindless_registry: None,
            // IMPLEMENTATION.md Step 2.3: a freshly cold-allocated texture
            // is "used" right now, not stale from the moment it's born.
            last_used_frame: device.frame_sync.total_frame_count.load(Ordering::Acquire),
            size_bytes: requirements.size,
        })
    }

    /// Uploads `pixels` as a new `SAMPLED | TRANSFER_DST` image and
    /// registers it into `device`'s bindless array (`RhiDevice::
    /// create_texture`'s backing, IMPLEMENTATION.md Step 2.1). Blocking:
    /// submits a one-time command buffer and waits on a fence before
    /// returning, matching this step's synchronous-upload scope decision
    /// (see `planning/archive/PLAN_PHASE2_STEP2.1.md`).
    pub(crate) fn from_pixels(
        device: &VulkanDevice,
        width: u32,
        height: u32,
        format: TextureFormat,
        pixels: &[u8],
    ) -> Result<Self, EngineError> {
        // Phase 2 Code Review finding #66: validated BEFORE any GPU call.
        // The `vkCmdCopyBufferToImage` region built further down is sized
        // purely from `width`/`height`/`format`, independent of the
        // staging buffer's actual size (`upload_buffer` sizes it from
        // `pixels.len()`) -- nothing else in this function protects
        // against a `pixels` slice shorter than that implies (including
        // empty), which would otherwise instruct the GPU to read past the
        // end of an undersized staging buffer.
        if width == 0 || height == 0 {
            return Err(EngineError::InvalidTextureData);
        }
        // Security-review finding: plain `u64` multiplication here can
        // wrap around (this workspace's release profile has no
        // `overflow-checks`), which for extreme attacker/caller-supplied
        // width/height could silently produce a small `expected_len` that
        // a small, real `pixels` buffer then passes against -- defeating
        // the exact OOB-read protection this check (Phase 2 finding #66)
        // exists to provide. `u128` has ample headroom for
        // `u32::MAX * u32::MAX * 8` and is compared back against a `u64`
        // pixel length via `try_from`, which fails (correctly rejecting
        // the input) rather than wrapping if it ever doesn't fit.
        let expected_len =
            u128::from(width) * u128::from(height) * u128::from(bytes_per_pixel(format));
        if pixels.len() as u128 != expected_len {
            return Err(EngineError::InvalidTextureData);
        }

        let vk_format = texture_format_to_vk(format);

        // SAFETY: `device.device` is valid, and `width`/`height` are
        // non-zero, validated above.
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
                    .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .initial_layout(vk::ImageLayout::UNDEFINED),
                None,
            )
        }
        .map_err(|_| EngineError::DeviceLost)?;

        // Phase 2 Code Review finding #68: from here on, `pending_image`
        // destroys `image` (and `memory`/`view` once assigned below) if
        // any later fallible step in this function returns early via `?`,
        // instead of leaking them. `image`/`view`/`memory` are cheap
        // `Copy` handles, so using them directly below (rather than
        // through `pending_image`) and relying on this guard purely for
        // its `Drop`/`into_parts` behavior is equivalent and simpler.
        let mut pending_image = PendingImage {
            device: device.device.clone(),
            image,
            memory: None,
            view: None,
        };

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
        // `memory_type_index` was selected from the `find` above.
        let memory = unsafe {
            device.device.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(requirements.size)
                    .memory_type_index(memory_type_index),
                None,
            )
        }
        .map_err(|_| EngineError::DeviceLost)?;
        pending_image.memory = Some(memory);

        // SAFETY: `image` and `memory` were both just created above on
        // this device, and `image` has not been bound to memory before now.
        unsafe {
            device
                .device
                .bind_image_memory(image, memory, 0)
                .map_err(|_| EngineError::DeviceLost)?;
        }

        // SAFETY: `device.device` is valid, and `image` was just bound to
        // `memory` immediately above.
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
        pending_image.view = Some(view);

        // Stage `pixels` and copy them into `image` via a one-time command
        // buffer, blocking on a fence before returning -- this crate's
        // established synchronous-only scope (Phase 2 Step 1's frame
        // submission, this same file's `upload_buffer`).
        let staging = device.upload_buffer(pixels, vk::BufferUsageFlags::TRANSFER_SRC)?;

        // Phase 2 Code Review finding #72 (reopened and actually closed by
        // the Phase 1-4 review): allocated from `upload_command_pool`, NOT
        // the frame loop's `command_pool` -- see that field's doc comment
        // on `VulkanDevice` for why sharing one pool between the two would
        // be an unsynchronized Vulkan spec violation. The review found
        // that the previous version of this code (`let upload_pool =
        // *device.upload_command_pool.lock().expect(...);`) bound only the
        // dereferenced `Copy` handle value, letting the `MutexGuard`
        // temporary drop at the end of that statement -- so the lock was
        // released *before* `allocate_command_buffers` below and long
        // before `free_command_buffers` further down, leaving both
        // spec-mandated-synchronized pool operations completely
        // unguarded. The fix is to keep the guard itself alive (bound to
        // `upload_pool_guard`, not just `_`) across the entire
        // allocate -> record -> submit -> free sequence, dropping it
        // explicitly right after the final `free_command_buffers` call.
        //
        // SAFETY: `upload_pool` is the valid, still-alive pool created in
        // `VulkanDevice::new`.
        let upload_pool_guard = device
            .upload_command_pool
            .lock()
            .expect("upload command pool poisoned");
        let upload_pool = *upload_pool_guard;
        let upload_cmd = unsafe {
            device.device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(upload_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
        }
        .map_err(|_| EngineError::DeviceLost)?[0];

        // Phase 2 Code Review finding #68: frees `upload_cmd` if any step
        // between here and its successful submission (below) returns
        // early via `?`.
        let pending_cmd = PendingCommandBuffer {
            device: device.device.clone(),
            pool: upload_pool,
            buffer: upload_cmd,
        };

        // SAFETY: `upload_cmd` was just allocated above and is in the
        // initial state; `image` and `staging.buffer` were both just
        // created on this same device and are still valid for the
        // duration of this recording.
        unsafe {
            device
                .device
                .begin_command_buffer(
                    upload_cmd,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .map_err(|_| EngineError::DeviceLost)?;

            let to_transfer_dst = vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .src_access_mask(vk::AccessFlags::empty())
                .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .image(image)
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .level_count(1)
                        .layer_count(1),
                );
            device.device.cmd_pipeline_barrier(
                upload_cmd,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[to_transfer_dst],
            );

            let region = vk::BufferImageCopy::default()
                .image_subresource(
                    vk::ImageSubresourceLayers::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .layer_count(1),
                )
                .image_extent(vk::Extent3D {
                    width,
                    height,
                    depth: 1,
                });
            device.device.cmd_copy_buffer_to_image(
                upload_cmd,
                staging.buffer,
                image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[region],
            );

            let to_shader_read = vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)
                .image(image)
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .level_count(1)
                        .layer_count(1),
                );
            device.device.cmd_pipeline_barrier(
                upload_cmd,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[to_shader_read],
            );

            device
                .device
                .end_command_buffer(upload_cmd)
                .map_err(|_| EngineError::DeviceLost)?;
        }

        // SAFETY: `device.device` is valid; a plain (unsignaled) fence is
        // correct since it is only ever waited on once, immediately below.
        // Not guarded by a `Pending*`-style drop guard: a `queue_submit`/
        // `wait_for_fences` failure here is effectively an unrecoverable
        // device-lost condition either way, at which point a leaked fence
        // handle is moot (Phase 2 Code Review finding #68's accepted
        // scope boundary).
        let upload_fence = unsafe {
            device
                .device
                .create_fence(&vk::FenceCreateInfo::default(), None)
        }
        .map_err(|_| EngineError::DeviceLost)?;
        let cmd_buffers = [upload_cmd];
        let submit_info = vk::SubmitInfo::default().command_buffers(&cmd_buffers);
        // SAFETY: `device.graphics_queue` is valid; `upload_cmd` just
        // finished recording above; `upload_fence` was just created above
        // and is waited on and destroyed only once, right here -- nothing
        // else references it afterward.
        unsafe {
            device
                .device
                .queue_submit(device.graphics_queue, &[submit_info], upload_fence)
                .map_err(|_| EngineError::DeviceLost)?;
            device
                .device
                .wait_for_fences(&[upload_fence], true, u64::MAX)
                .map_err(|_| EngineError::DeviceLost)?;
            device.device.destroy_fence(upload_fence, None);
        }
        // The GPU has confirmed it's done with `upload_cmd` (the fence
        // wait above), so free it for real now -- claiming it from the
        // guard first so `pending_cmd`'s own `Drop` doesn't also try.
        let upload_cmd = pending_cmd.into_inner();
        // SAFETY: `upload_pool` is the same pool `upload_cmd` was
        // allocated from above, and the fence wait just confirmed the GPU
        // is done with it.
        unsafe {
            device
                .device
                .free_command_buffers(upload_pool, &[upload_cmd]);
        }
        // Only now, after the last operation that needed the pool
        // externally synchronized, is it safe to let another thread's
        // concurrent `create_texture` call proceed past its own `lock()`.
        drop(upload_pool_guard);

        // Register into the bindless array -- factored into `VulkanDevice::
        // allocate_bindless_slot` (IMPLEMENTATION.md Phase 6 Step 6.4.1),
        // shared with the new `RhiDevice::register_bindless`, which needs
        // this same allocate-and-write-descriptor logic for a texture
        // that's already GPU-resident rather than being freshly uploaded.
        let bindless_index = device.allocate_bindless_slot(view)?;

        // Every fallible step is behind us -- claim the handles out of the
        // guard without running its `Drop`.
        let (image, view, memory) = pending_image.into_parts();

        Ok(Self {
            image,
            view,
            memory,
            width,
            height,
            format,
            device: device.device.clone(),
            _owner: Arc::clone(&device.owner),
            bindless_index: Some(bindless_index),
            bindless_registry: Some(Arc::clone(&device.bindless_registry)),
            // IMPLEMENTATION.md Step 2.3: unread for a bindless texture
            // (it never enters `TransientPool::free`), but every
            // `VulkanTexture` carries the field, so it's set for
            // struct-completeness rather than defaulted to a meaningless
            // value.
            last_used_frame: device.frame_sync.total_frame_count.load(Ordering::Acquire),
            size_bytes: requirements.size,
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

    fn bindless_index(&self) -> Option<u32> {
        self.bindless_index
    }

    fn size_bytes(&self) -> u64 {
        self.size_bytes
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
        // Release the bindless slot back to the free list, if this texture
        // ever had one -- transient render targets (`bindless_index: None`)
        // skip this entirely.
        if let (Some(index), Some(registry)) = (self.bindless_index, &self.bindless_registry) {
            registry
                .lock()
                .expect("bindless registry poisoned")
                .release(index);
        }
    }
}
