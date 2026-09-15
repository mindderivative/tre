//! `VulkanBuffer`/`VulkanPipelineState` -- small, opaque `RhiBuffer`/
//! `RhiPipelineState` wrapper types with no logic of their own beyond
//! "hold a handle, destroy it on drop." Grouped together in one file
//! since each is a handful of lines; every larger, self-contained
//! subsystem (the device, the swapchain, the command buffer, the blur
//! pipeline, the transient pool, textures, the ring buffer) gets its own
//! file instead (Architecture review finding: this crate's own main file
//! had grown to over 5,000 lines with no internal module boundaries at
//! all, unlike `tre-engine`'s own already-split `lib.rs`).

use ash::vk;
use ash::vk::Handle;
use tre_engine::{RhiBuffer, RhiPipelineState};

/// A single host-visible, host-coherent GPU buffer (`VulkanDevice::
/// upload_buffer`'s own return type) -- the `RhiBuffer` this crate hands
/// back for one-shot vertex/index uploads. For a reusable, per-frame
/// dynamic buffer, see [`crate::VulkanRingBuffer`] instead.
pub struct VulkanBuffer {
    pub(crate) buffer: vk::Buffer,
    pub(crate) memory: vk::DeviceMemory,
    pub(crate) device: ash::Device,
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
    pub(crate) pipeline: vk::Pipeline,
    /// This pipeline's own `VkPipelineLayout` -- not shared across
    /// pipelines, even where two pipelines' layouts are structurally
    /// identical (`create_universal_pipeline_layout`'s own doc comment).
    pub layout: vk::PipelineLayout,
    pub(crate) device: ash::Device,
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
