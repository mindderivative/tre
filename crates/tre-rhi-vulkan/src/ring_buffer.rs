//! `VulkanRingBuffer` -- TECHNICAL.md Section 3.1's triple-buffered
//! dynamic ring buffer, used for both `VulkanDevice::create_dynamic_
//! ring_buffer`'s external callers and the device's own internal
//! `shape_style_buffer`. Split out of `lib.rs` as one of its ten
//! separable concerns (Architecture review finding).

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use ash::vk;
use ash::vk::Handle;
use tre_engine::{EngineError, RhiBuffer, RhiDynamicRingBuffer};

use crate::{align_up, FrameSync, FRAMES_IN_FLIGHT, RING_BUFFER_ALIGNMENT};

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
    pub(crate) buffer: vk::Buffer,
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
    /// Takes the raw pieces (rather than `&VulkanDevice`) so this can be
    /// called from inside `VulkanDevice::new` itself -- Phase 10 Step
    /// 10.2's `shape_style_buffer` field needs to exist before `Self` does
    /// (it must be fully built to go in the constructor's own `Self { .. }`
    /// literal), which a `&VulkanDevice`-taking constructor could never
    /// support. `RhiDevice::create_dynamic_ring_buffer` (the original,
    /// still the only *external* caller) just forwards its own device's
    /// fields through.
    pub(crate) fn new(
        device: &ash::Device,
        physical_device: vk::PhysicalDevice,
        instance: &ash::Instance,
        frame_sync: Arc<FrameSync>,
        capacity: usize,
    ) -> Result<Self, EngineError> {
        let segment_size = align_up(capacity.div_ceil(FRAMES_IN_FLIGHT), RING_BUFFER_ALIGNMENT);
        let total_size = segment_size * FRAMES_IN_FLIGHT;

        // SAFETY: `device` is valid, and `total_size` is used directly as
        // `size` so the create info describes exactly this buffer's full
        // triple-segment span. `STORAGE_BUFFER` (Phase 10 Step 10.2, on
        // top of the pre-existing three usages) lets any ring buffer this
        // constructor builds double as an SSBO -- harmless for the
        // vertex/index ring buffers that never use it, and exactly what
        // `VulkanDevice::shape_style_buffer` needs to be bindable at the
        // bindless set's binding 2.
        let buffer = unsafe {
            device.create_buffer(
                &vk::BufferCreateInfo::default()
                    .size(total_size as u64)
                    .usage(
                        vk::BufferUsageFlags::VERTEX_BUFFER
                            | vk::BufferUsageFlags::INDEX_BUFFER
                            | vk::BufferUsageFlags::UNIFORM_BUFFER
                            | vk::BufferUsageFlags::STORAGE_BUFFER,
                    )
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            )
        }
        .map_err(|_| EngineError::DeviceLost)?;

        // SAFETY: `buffer` was just created above on this device.
        let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
        // SAFETY: `physical_device` is valid for as long as `instance`
        // (also alive here) is.
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
            .ok_or(EngineError::DeviceLost)?;

        // SAFETY: `device` is valid, `requirements.size` comes directly
        // from `get_buffer_memory_requirements` above, and
        // `memory_type_index` was selected from the `find` above so it is
        // one of the bits set in `requirements.memory_type_bits`.
        let memory = unsafe {
            device.allocate_memory(
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
                .bind_buffer_memory(buffer, memory, 0)
                .map_err(|_| EngineError::DeviceLost)?;
            device
                .map_memory(memory, 0, total_size as u64, vk::MemoryMapFlags::empty())
                .map_err(|_| EngineError::DeviceLost)? as *mut u8
        };

        Ok(Self {
            buffer,
            memory,
            mapped_ptr,
            segment_size,
            frame_sync,
            state: Mutex::new(RingBufferState {
                last_seen_frame_index: usize::MAX,
                cursor: 0,
            }),
            device: device.clone(),
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
