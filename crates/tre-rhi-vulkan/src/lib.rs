//! Vulkan 1.2+ RHI backend (`RhiDevice`/`RhiCommandBuffer` impls,
//! ARCHITECTURE.md Section 6), built on the `ash` raw-bindings crate
//! (IMPLEMENTATION.md Step 2.1). Cross-platform wherever Vulkan is
//! available -- unlike the DX12/Metal backends, not target-gated to one OS.
//!
//! One of the three crates permitted to contain `unsafe`
//! (TECHNICAL.md Section 9.1), for raw Vulkan FFI.
#![deny(unsafe_op_in_unsafe_fn)]

mod blur;
mod buffer;
mod command_buffer;
mod device;
mod headless;
mod ring_buffer;
mod shape_pipelines;
mod swapchain;
mod texture;
mod transient_pool;

pub use buffer::{VulkanBuffer, VulkanPipelineState};
pub use command_buffer::VulkanCommandBuffer;
pub(crate) use device::FrameSync;
pub use device::VulkanDevice;
pub use headless::{HeadlessSwapchain, HEADLESS_FORMAT};
pub use ring_buffer::VulkanRingBuffer;
pub use shape_pipelines::register_shape_pipelines;
pub use swapchain::VulkanSwapchain;
pub use texture::VulkanTexture;
pub use transient_pool::TransientPoolStats;

use std::ffi::CStr;
use std::time::Duration;

use ash::vk;
use tre_engine::TextureFormat;

const REQUIRED_DEVICE_EXTENSIONS: &[&CStr] = &[
    ash::khr::swapchain::NAME,
    ash::khr::dynamic_rendering::NAME,
    ash::ext::descriptor_indexing::NAME,
];

/// ARCHITECTURE.md Section 4.1's sort key commits to a 12-bit (4,096-slot)
/// texture ID field -- this is the array size requested for the bindless
/// descriptor array, clamped down at runtime (see `VulkanDevice::new`)
/// against whatever the real device's
/// `maxDescriptorSetUpdateAfterBindSampledImages` limit actually is, since
/// `VK_EXT_descriptor_indexing`'s `VARIABLE_DESCRIPTOR_COUNT` machinery
/// still requires declaring a maximum at layout-creation time.
const BINDLESS_TEXTURE_CAPACITY_TARGET: u32 = 4096;

/// The push-constant/shader convention for "no texture bound, use the
/// vertex's own color" (IMPLEMENTATION.md Step 2.1's per-draw-call texture
/// index has to mean something when `RhiCommandBuffer::bind_texture` was
/// never called for a given draw -- Phase 0's flat-color path must keep
/// working unchanged by default).
const BINDLESS_TEXTURE_SENTINEL: u32 = u32::MAX;

/// TECHNICAL.md Section 1's "Dynamic VRAM Footprint" target -- the budget
/// IMPLEMENTATION.md Step 2.3's GC trigger is a percentage of. Deliberately
/// a fixed target, not a fraction of the real device's total VRAM: a
/// modern desktop GPU has gigabytes of headroom, so a device-relative
/// trigger would almost never fire, defeating the point of a budget the
/// engine itself is supposed to police.
const DYNAMIC_VRAM_BUDGET_BYTES: u64 = 128 * 1024 * 1024;

/// IMPLEMENTATION.md Step 2.3 task 2: "scans resource pools when VRAM
/// capacity hits 85%."
const GC_TRIGGER_THRESHOLD_BYTES: u64 = DYNAMIC_VRAM_BUDGET_BYTES * 85 / 100;

/// IMPLEMENTATION.md Step 2.3 task 3: "resources older than N = 600
/// frames," compared against `FrameSync::total_frame_count`.
const GC_EVICTION_AGE_FRAMES: u64 = 600;

/// IMPLEMENTATION.md Step 2.3 task 4: "destroy hardware resources only if
/// N_current - N_evicted > 3 frames" -- the grace period `begin_frame`'s
/// deferred-release drain waits out before actually destroying anything
/// the GC thread evicted, so a resource the GPU might still be reading
/// from a just-finished frame is never destroyed out from under it.
const DEFERRED_RELEASE_GRACE_FRAMES: u64 = 3;

/// How often the background GC thread wakes to check
/// `TransientPool::total_free_bytes` against `GC_TRIGGER_THRESHOLD_BYTES`.
/// Not specified by TECHNICAL.md; chosen to be responsive relative to
/// `GC_EVICTION_AGE_FRAMES` (600 frames is multiple seconds even at
/// 240 Hz) without busy-looping a whole CPU core doing nothing between
/// real triggers.
const GC_SCAN_INTERVAL: Duration = Duration::from_millis(100);

/// Caps how many entries a single GC scan evicts before releasing
/// `transient_pool`'s lock (Phase 2 Step 2.3 Code Review finding #81).
/// Without a cap, one scan pass evicts every eligible entry in a single
/// critical section -- cheap at today's scale (an age comparison per
/// entry), but the per-eviction work (a `Vec` push, later a queue-lock and
/// push) is real cost that grows without bound alongside the pool's total
/// entry count as more of the engine comes to share this pool (the
/// atlas/SVG cache this step's literal wording targets). Capping the
/// per-scan eviction count bounds both the render thread's worst-case
/// contention on `transient_pool` and `Drop for VulkanDevice`'s shutdown
/// latency to a small, constant amount of work -- any backlog beyond the
/// cap simply finishes on the next scan, `GC_SCAN_INTERVAL` later, which
/// is harmless since eviction was never time-critical to begin with.
const GC_MAX_EVICTIONS_PER_SCAN: usize = 64;

/// TECHNICAL.md Section 3.1's triple-buffered ring: 3 logical segments,
/// one per frame-in-flight slot.
pub(crate) const FRAMES_IN_FLIGHT: usize = 3;

/// TECHNICAL.md Section 3.1's "256-byte minimum alignment for RHI dynamic
/// offsets."
pub(crate) const RING_BUFFER_ALIGNMENT: usize = 256;

/// Phase 10 Step 10.2: total capacity (across all `FRAMES_IN_FLIGHT`
/// segments) of `VulkanDevice::shape_style_buffer`, the bindless
/// binding-2 storage buffer `GpuShapeStyle` records are bump-allocated
/// into. 1 MiB / 3 segments / 192 bytes-per-record (`GpuShapeStyle`'s
/// real size) is a little under 1,800 styled shapes per frame -- far more
/// than one real UI frame draws today, with headroom to grow.
const SHAPE_STYLE_BUFFER_CAPACITY: usize = 1 << 20;

pub(crate) fn align_up(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

fn texture_format_to_vk(format: TextureFormat) -> vk::Format {
    match format {
        TextureFormat::Bgra8Srgb => vk::Format::B8G8R8A8_SRGB,
        TextureFormat::Rgba16Float => vk::Format::R16G16B16A16_SFLOAT,
        TextureFormat::Rgba8Unorm => vk::Format::R8G8B8A8_UNORM,
    }
}

/// Bytes per texel for `format`, tightly packed -- the same layout
/// `VulkanTexture::from_pixels` requires of its `pixels` argument. Used to
/// validate an uploaded buffer's length before it ever reaches a GPU call
/// (Phase 2 Code Review finding #66).
fn bytes_per_pixel(format: TextureFormat) -> u64 {
    match format {
        TextureFormat::Bgra8Srgb | TextureFormat::Rgba8Unorm => 4,
        TextureFormat::Rgba16Float => 8,
    }
}
