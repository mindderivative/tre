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
use std::collections::VecDeque;
use std::ffi::{c_char, CStr};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use ash::vk;
use ash::vk::Handle;
use tre_engine::{
    AcquiredImage, EngineError, RhiBuffer, RhiCommandBuffer, RhiDevice, RhiDynamicRingBuffer,
    RhiPipelineState, RhiSwapchain, RhiTexture, ScissorRect, TextureFormat, UiVertex,
};

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

/// Bytes per texel for `format`, tightly packed -- the same layout
/// `VulkanTexture::from_pixels` requires of its `pixels` argument. Used to
/// validate an uploaded buffer's length before it ever reaches a GPU call
/// (Phase 2 Code Review finding #66).
fn bytes_per_pixel(format: TextureFormat) -> u64 {
    match format {
        TextureFormat::Bgra8Srgb => 4,
        TextureFormat::Rgba16Float => 8,
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
    /// A genuinely monotonic, ever-increasing frame counter (IMPLEMENTATION.md
    /// Step 2.3), advanced alongside `frame_index` by `submit_and_present`
    /// but never wrapping -- `frame_index`'s 0..3 rotation answers "which
    /// ring-buffer segment," this answers "how many frames old is this
    /// resource." Read by both the main thread (grace-period checks in
    /// `begin_frame`'s deferred-release drain) and the background GC
    /// thread (staleness checks against `VulkanTexture::last_used_frame`).
    total_frame_count: AtomicU64,
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
    /// `Arc`-wrapped (IMPLEMENTATION.md Step 2.3) so the background GC
    /// thread can hold its own clone -- the "later" the doc comment above
    /// refers to has arrived.
    transient_pool: Arc<Mutex<TransientPool>>,
    /// A single shared sampler used by every bindless-array texture
    /// (IMPLEMENTATION.md Step 2.1) -- baked into
    /// `bindless_descriptor_set_layout` as an immutable sampler, so it is
    /// never itself written via `vkUpdateDescriptorSets`.
    bindless_sampler: vk::Sampler,
    bindless_descriptor_pool: vk::DescriptorPool,
    bindless_descriptor_set_layout: vk::DescriptorSetLayout,
    /// The one persistent descriptor set every pipeline binds (see
    /// `create_pipeline`/`VulkanCommandBuffer::set_pipeline`) -- bindless
    /// means this is bound exactly once and never rebound between draws
    /// that reference different textures, unlike traditional per-texture
    /// descriptor sets.
    bindless_descriptor_set: vk::DescriptorSet,
    /// Which of `bindless_descriptor_set`'s array slots (binding 0) are
    /// currently assigned to a live texture. `Mutex`-guarded for the same
    /// forward-looking reason as `transient_pool`; `Arc`-wrapped (like
    /// `frame_sync`) so every `VulkanTexture` created via `create_texture`
    /// can hold a clone and release its own slot on `Drop` without needing
    /// to reach back through a whole `VulkanDevice`.
    bindless_registry: Arc<Mutex<BindlessRegistry>>,
    /// The real, runtime-clamped size of the bindless array (`min(4096,
    /// maxDescriptorSetUpdateAfterBindSampledImages)`), cached here for
    /// `VulkanCommandBuffer::bind_texture` to bounds-check against without
    /// locking `bindless_registry` (Phase 2 Code Review finding #69).
    bindless_capacity: u32,
    /// A command pool dedicated to `VulkanTexture::from_pixels`'s one-time
    /// upload command buffers -- deliberately SEPARATE from `command_pool`
    /// above (the per-frame render loop's pool). Vulkan requires external
    /// synchronization on a command pool for `vkAllocateCommandBuffers`/
    /// `vkFreeCommandBuffers`; sharing one pool between the frame loop and
    /// texture uploads would need its own synchronization, which nothing
    /// provided (Phase 2 Code Review finding #72). `Mutex`-guarded so
    /// concurrent `create_texture` calls from multiple threads serialize
    /// safely instead of racing each other.
    upload_command_pool: Mutex<vk::CommandPool>,
    /// Evicted transient textures awaiting their 3-frame grace period
    /// (IMPLEMENTATION.md Step 2.3) before the main thread actually
    /// destroys them in `begin_frame`. A plain `Mutex<VecDeque<_>>`, not a
    /// lock-free structure -- see PLAN.md's "deliberate deviation from
    /// lock-free queue" for why (peeking the front without consuming it is
    /// needed here, and contention at this call frequency is negligible).
    /// `Arc`-wrapped so the GC thread (sole producer) and the main thread
    /// (sole consumer) each hold their own clone.
    deferred_release: Arc<Mutex<VecDeque<DeferredRelease>>>,
    /// Set to `false` by `Drop for VulkanDevice` to tell the GC thread to
    /// exit its scan loop; joined immediately after.
    gc_running: Arc<AtomicBool>,
    /// The background GC thread's handle (IMPLEMENTATION.md Step 2.3) --
    /// the engine's first genuine OS thread. `Option` only so `Drop` can
    /// `.take()` it to call `.join()`, which consumes the handle.
    gc_thread: Option<JoinHandle<()>>,
    /// `VK_EXT_debug_utils` messenger (TECHNICAL.md Section 9.2,
    /// IMPLEMENTATION.md Step 2.4), `None` if the validation
    /// layer/extension weren't both available at instance-creation time.
    /// The field itself doesn't exist in release builds -- compiled out
    /// entirely, matching TECHNICAL.md Section 3.4's zero-allocation
    /// guard's own release-build behavior, so there is no cost (not even
    /// an unused `Option`) in a shipped binary.
    #[cfg(debug_assertions)]
    debug_utils: Option<(ash::ext::debug_utils::Instance, vk::DebugUtilsMessengerEXT)>,
}

/// `VK_EXT_debug_utils` messenger callback (IMPLEMENTATION.md Step 2.4).
/// Called BY the Vulkan loader/driver (non-Rust code) -- an
/// `extern "system" fn`, not a Rust closure, so it must never let a panic
/// unwind past it (unwinding across a non-Rust ABI boundary is undefined
/// behavior).
///
/// `std::process::abort()`, not `std::process::exit()`, on an
/// error-severity message: this was verified by actually triggering it
/// (a deliberately invalid Vulkan call during this step's own CI-gate
/// verification, `documentation/REVIEW.md`'s Phase 2 Step 2 entry), not
/// assumed from reading the docs. `std::process::exit()` runs registered
/// `atexit` handlers before terminating -- if the driver has registered
/// one that tries to reacquire a lock the still-on-the-stack Vulkan call
/// that triggered this very callback is holding, `exit()` deadlocks
/// instead of terminating (confirmed: it hung indefinitely under real
/// hardware/drivers). `abort()` raises `SIGABRT` directly, skipping
/// `atexit` entirely, and reliably terminates the process with a nonzero
/// exit status (enough to fail a CI job) instead.
#[cfg(debug_assertions)]
unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::ffi::c_void,
) -> vk::Bool32 {
    // SAFETY: `callback_data` is supplied by the Vulkan loader for the
    // duration of this call only, per `VK_EXT_debug_utils`'s contract,
    // and its `p_message` is always a valid, NUL-terminated C string when
    // this callback fires.
    let message = unsafe { CStr::from_ptr((*callback_data).p_message) }.to_string_lossy();
    eprintln!("[Vulkan {message_severity:?} {message_type:?}] {message}");
    if message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
        std::process::abort();
    }
    vk::FALSE
}

/// Checks whether both `VK_LAYER_KHRONOS_validation` and
/// `VK_EXT_debug_utils` are actually installed, rather than unconditionally
/// requesting them -- requesting an unavailable layer/extension would fail
/// `vkCreateInstance` outright, breaking `cargo run` for any contributor
/// who hasn't installed the Vulkan validation layers package locally.
/// Debug-build-only: validation is meant to be free in release builds.
#[cfg(debug_assertions)]
fn debug_validation_available(entry: &ash::Entry) -> bool {
    // SAFETY: `entry` was just loaded by the caller and is valid; this is
    // a query with no preconditions beyond that.
    let layers = unsafe { entry.enumerate_instance_layer_properties() }.unwrap_or_default();
    let layer_available = layers.iter().any(|layer| {
        // SAFETY: `layer.layer_name` is a fixed-size buffer the Vulkan
        // implementation NUL-terminates.
        (unsafe { CStr::from_ptr(layer.layer_name.as_ptr()) }) == c"VK_LAYER_KHRONOS_validation"
    });

    // SAFETY: `entry` is valid; `None` queries the base Vulkan
    // implementation's extensions rather than a specific layer's.
    let extensions =
        unsafe { entry.enumerate_instance_extension_properties(None) }.unwrap_or_default();
    let debug_utils_available = extensions.iter().any(|ext| {
        // SAFETY: same as `layer.layer_name` above.
        (unsafe { CStr::from_ptr(ext.extension_name.as_ptr()) }) == ash::ext::debug_utils::NAME
    });

    layer_available && debug_utils_available
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

        let mut enabled_layers: Vec<*const c_char> = Vec::new();
        #[cfg(debug_assertions)]
        let validation_requested = if debug_validation_available(&entry) {
            enabled_layers.push(c"VK_LAYER_KHRONOS_validation".as_ptr());
            required_extensions.push(ash::ext::debug_utils::NAME.as_ptr());
            true
        } else {
            eprintln!(
                "tre-rhi-vulkan: VK_LAYER_KHRONOS_validation/VK_EXT_debug_utils not both \
                 available (install the Vulkan validation layers package for debug-build GPU \
                 validation) -- continuing without them"
            );
            false
        };

        let instance_create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_layer_names(&enabled_layers)
            .enabled_extension_names(&required_extensions);

        // SAFETY: `entry` was just loaded above and is valid; `app_info`,
        // `enabled_layers`, and `required_extensions` are locals borrowed
        // only for the duration of this call. The returned `VkInstance` is
        // destroyed exactly once in `Drop for VulkanDevice` below.
        let instance = unsafe { entry.create_instance(&instance_create_info, None) }
            .map_err(|_| EngineError::DeviceLost)?;

        #[cfg(debug_assertions)]
        let debug_utils = validation_requested
            .then(|| {
                let debug_utils_loader = ash::ext::debug_utils::Instance::new(&entry, &instance);
                let messenger_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
                    .message_severity(
                        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                            | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
                    )
                    .message_type(
                        vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                            | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                            | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                    )
                    .pfn_user_callback(Some(vulkan_debug_callback));
                // SAFETY: `debug_utils_loader` was just created from this
                // valid `instance`/`entry`; `messenger_info` (and the
                // `'static` callback function it references) is a local
                // borrowed only for the duration of this call, which is all
                // `create_debug_utils_messenger` requires.
                unsafe { debug_utils_loader.create_debug_utils_messenger(&messenger_info, None) }
                    .ok()
                    .map(|messenger| (debug_utils_loader, messenger))
            })
            .flatten();

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

        // TECHNICAL.md Section 2.1 requires `VK_EXT_descriptor_indexing` as
        // a hard requirement (unlike the gracefully-degraded validation
        // layer), but `VARIABLE_DESCRIPTOR_COUNT`'s array size still has to
        // be clamped to what this real device actually supports --
        // ARCHITECTURE.md Section 4.1's 4,096-slot target is a ceiling, not
        // a guarantee, and a software rasterizer in particular has no
        // reason to advertise a generous limit.
        //
        // SAFETY: `physical_device` was chosen above from this instance's
        // own enumeration and is still valid; `descriptor_indexing_properties`
        // is a local that outlives this call, referenced only via
        // `properties2`'s `push_next` chain.
        let mut descriptor_indexing_properties =
            vk::PhysicalDeviceDescriptorIndexingProperties::default();
        let mut properties2 =
            vk::PhysicalDeviceProperties2::default().push_next(&mut descriptor_indexing_properties);
        unsafe { instance.get_physical_device_properties2(physical_device, &mut properties2) };
        let bindless_capacity = BINDLESS_TEXTURE_CAPACITY_TARGET
            .min(descriptor_indexing_properties.max_descriptor_set_update_after_bind_sampled_images)
            .max(1);

        let mut descriptor_indexing_feature =
            vk::PhysicalDeviceDescriptorIndexingFeatures::default()
                .shader_sampled_image_array_non_uniform_indexing(true)
                .descriptor_binding_sampled_image_update_after_bind(true)
                .descriptor_binding_partially_bound(true)
                .descriptor_binding_variable_descriptor_count(true)
                .descriptor_binding_update_unused_while_pending(true)
                .runtime_descriptor_array(true);

        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&device_extension_names)
            .push_next(&mut dynamic_rendering_feature)
            .push_next(&mut descriptor_indexing_feature);

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

        // A separate pool from `command_pool` above, dedicated to
        // `VulkanTexture::from_pixels`'s one-time upload command buffers
        // (Phase 2 Code Review finding #72) -- `TRANSIENT` since every
        // buffer allocated from it is recorded once, submitted once, and
        // freed immediately.
        //
        // SAFETY: `device` is the just-created, still-valid logical
        // device, and `queue_family_index` is the same family it was
        // created with.
        let upload_command_pool = unsafe {
            device.create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(queue_family_index)
                    .flags(vk::CommandPoolCreateFlags::TRANSIENT),
                None,
            )
        }
        .map_err(|_| EngineError::DeviceLost)?;

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
            total_frame_count: AtomicU64::new(0),
        });

        // IMPLEMENTATION.md Step 2.1: one persistent bindless descriptor
        // set, created once here and bound once per pipeline
        // (`VulkanCommandBuffer::set_pipeline`) rather than rebuilt or
        // rebound per texture.
        //
        // SAFETY: `device` is valid (created above).
        let bindless_sampler = unsafe {
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
        .map_err(|_| EngineError::DeviceLost)?;

        // Binding 1 (the HIGHEST-numbered binding -- required, per spec,
        // since `VARIABLE_DESCRIPTOR_COUNT` may only be set on the binding
        // with the highest binding number in the layout): the unbounded
        // `texture2D textures[]` array IMPLEMENTATION.md Step 2.1 describes
        // -- `SAMPLED_IMAGE`, not `COMBINED_IMAGE_SAMPLER`, per that same
        // wording (a separate, single shared sampler at binding 0 instead).
        // Binding 0's `immutable_samplers` bakes `bindless_sampler` into
        // the layout itself, so that binding is never written via
        // `vkUpdateDescriptorSets`.
        let bindless_layout_bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT)
                .immutable_samplers(std::slice::from_ref(&bindless_sampler)),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(bindless_capacity)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        // Binding 1 (the texture array) needs all four flags:
        // `UPDATE_AFTER_BIND` (textures are registered after the set is
        // bound elsewhere in a frame's lifetime), `PARTIALLY_BOUND` (most
        // of a 4,096-slot array is unused at any given moment),
        // `VARIABLE_DESCRIPTOR_COUNT` (the array's real size is
        // `bindless_capacity`, decided at runtime, not
        // `BINDLESS_TEXTURE_CAPACITY_TARGET` unconditionally), and
        // `UPDATE_UNUSED_WHILE_PENDING` (registering a new texture must not
        // require waiting for in-flight draws that don't reference it).
        // Binding 0's immutable sampler needs none of them.
        let bindless_binding_flags = [
            vk::DescriptorBindingFlags::empty(),
            vk::DescriptorBindingFlags::UPDATE_AFTER_BIND
                | vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::VARIABLE_DESCRIPTOR_COUNT
                | vk::DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING,
        ];
        let mut bindless_binding_flags_info =
            vk::DescriptorSetLayoutBindingFlagsCreateInfo::default()
                .binding_flags(&bindless_binding_flags);
        // SAFETY: `device` is valid; `bindless_layout_bindings` (including
        // the `bindless_sampler` handle it borrows) and
        // `bindless_binding_flags_info` are locals that outlive this call.
        let bindless_descriptor_set_layout = unsafe {
            device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default()
                    .bindings(&bindless_layout_bindings)
                    .flags(vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL)
                    .push_next(&mut bindless_binding_flags_info),
                None,
            )
        }
        .map_err(|_| EngineError::DeviceLost)?;

        let bindless_pool_sizes = [
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(bindless_capacity),
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::SAMPLER)
                .descriptor_count(1),
        ];
        // SAFETY: `device` is valid, and `bindless_pool_sizes` is a local
        // that outlives this call.
        let bindless_descriptor_pool = unsafe {
            device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .flags(vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND)
                    .max_sets(1)
                    .pool_sizes(&bindless_pool_sizes),
                None,
            )
        }
        .map_err(|_| EngineError::DeviceLost)?;

        let bindless_set_layouts = [bindless_descriptor_set_layout];
        let bindless_variable_counts = [bindless_capacity];
        let mut bindless_variable_count_info =
            vk::DescriptorSetVariableDescriptorCountAllocateInfo::default()
                .descriptor_counts(&bindless_variable_counts);
        // SAFETY: `device` is valid; `bindless_descriptor_pool` and
        // `bindless_descriptor_set_layout` were both just created above on
        // this same device; `bindless_set_layouts`/
        // `bindless_variable_count_info` are locals that outlive this call.
        let bindless_descriptor_set = unsafe {
            device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(bindless_descriptor_pool)
                    .set_layouts(&bindless_set_layouts)
                    .push_next(&mut bindless_variable_count_info),
            )
        }
        .map_err(|_| EngineError::DeviceLost)?[0];

        // IMPLEMENTATION.md Step 2.3: the transient pool and the
        // deferred-release queue are constructed as locals first (not
        // directly inside the `Self { .. }` literal below) specifically so
        // the background GC thread, spawned next, can hold its own `Arc`
        // clone of each before they're moved into `Self`.
        let transient_pool = Arc::new(Mutex::new(TransientPool::default()));
        let deferred_release: Arc<Mutex<VecDeque<DeferredRelease>>> =
            Arc::new(Mutex::new(VecDeque::new()));
        let gc_running = Arc::new(AtomicBool::new(true));
        let gc_thread = std::thread::spawn({
            let transient_pool = Arc::clone(&transient_pool);
            let frame_sync = Arc::clone(&frame_sync);
            let deferred_release = Arc::clone(&deferred_release);
            let gc_running = Arc::clone(&gc_running);
            move || gc_thread_loop(transient_pool, frame_sync, deferred_release, gc_running)
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
                upload_command_pool: Mutex::new(upload_command_pool),
                dynamic_rendering,
                frame_sync,
                transient_pool,
                deferred_release,
                gc_running,
                gc_thread: Some(gc_thread),
                bindless_sampler,
                bindless_descriptor_pool,
                bindless_descriptor_set_layout,
                bindless_descriptor_set,
                bindless_registry: Arc::new(Mutex::new(BindlessRegistry::new(bindless_capacity))),
                bindless_capacity,
                #[cfg(debug_assertions)]
                debug_utils,
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

        // IMPLEMENTATION.md Step 2.1: this is the ONE universal pipeline
        // layout every pipeline gets, always including the bindless
        // descriptor set and the 4-byte `texture_index` push constant --
        // regardless of whether a given shader actually declares/consumes
        // them. A pipeline layout may expose resources a shader doesn't
        // use, so `walking_skeleton.vert`/`.frag` (and every other
        // pre-existing shader) keep compiling and running completely
        // unmodified against this extended layout.
        let bindless_set_layouts = [self.bindless_descriptor_set_layout];
        // SAFETY: `self.device` is the valid logical device owned by this
        // `VulkanDevice`; `bindless_set_layouts` (referencing this device's
        // own `bindless_descriptor_set_layout`, created in `new`) and
        // `push_constant_ranges`'s slice are local temporaries that outlive
        // this call.
        let layout = unsafe {
            self.device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(&bindless_set_layouts)
                    .push_constant_ranges(&[
                        vk::PushConstantRange::default()
                            .stage_flags(
                                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            )
                            .offset(0)
                            .size(12), // vec2 screen_size, uint texture_index
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
                pool.total_free_bytes += texture.size_bytes;
                pool.free
                    .entry((width, height, format))
                    .or_default()
                    .push(texture);
            }
        }
    }

    /// Physically destroys every deferred-release entry that has served
    /// its `DEFERRED_RELEASE_GRACE_FRAMES` (IMPLEMENTATION.md Step 2.3
    /// task 4) -- the ONLY place in this crate that ever destroys a
    /// GC-evicted texture, deliberately on the main thread, not the GC
    /// thread that decided to evict it (see `gc_thread_loop`'s doc
    /// comment). Called at the start of `begin_frame`, alongside
    /// `grow_pending_transient_targets`.
    fn drain_deferred_release_queue(&self) {
        let current_frame = self.frame_sync.total_frame_count.load(Ordering::Acquire);
        let mut queue = self
            .deferred_release
            .lock()
            .expect("deferred release queue poisoned");
        // The queue is FIFO-ordered by a monotonically non-decreasing
        // `evicted_at_frame` (the GC thread only ever reads an
        // ever-increasing counter), so the moment the front entry hasn't
        // served its grace period, nothing behind it has either.
        while let Some(front) = queue.front() {
            if current_frame.saturating_sub(front.evicted_at_frame) <= DEFERRED_RELEASE_GRACE_FRAMES
            {
                break;
            }
            let entry = queue.pop_front().expect("front() just confirmed Some");
            // Dropping `entry.texture` here runs `Drop for VulkanTexture`,
            // which does the real `vkDestroy*` teardown.
            drop(entry);
            self.transient_pool
                .lock()
                .expect("transient pool poisoned")
                .stats
                .destroyed += 1;
        }
    }
}

impl Drop for VulkanDevice {
    fn drop(&mut self) {
        // IMPLEMENTATION.md Step 2.3: stop and join the background GC
        // thread FIRST, before anything else -- it only ever touches
        // `transient_pool`'s `Mutex` and plain data (never a Vulkan call,
        // see `gc_thread_loop`'s doc comment), but the pool-clear step
        // just below this would otherwise race a scan still in progress.
        // `gc_thread_loop` re-checks `running` immediately after waking
        // from its sleep, so shutdown latency is bounded by
        // `GC_SCAN_INTERVAL`, not indefinite.
        self.gc_running.store(false, Ordering::Release);
        if let Some(handle) = self.gc_thread.take() {
            let _ = handle.join();
        }

        // Phase 2 Code Review finding #71: wait for the GPU to finish all
        // outstanding work before destroying anything below. Every
        // windowed example happened to call `device_wait_idle()` itself at
        // the end of `main()` first, but nothing enforced that -- this
        // phase's growing teardown list (the whole bindless descriptor
        // apparatus, on top of the transient pool) raised the stakes of
        // relying on caller convention. Ignoring the result: if the device
        // is already lost, there is nothing further to usefully wait for,
        // and panicking inside `Drop` is itself undesirable.
        //
        // SAFETY: `self.device` is still a valid handle at this point.
        let _ = unsafe { self.device.device_wait_idle() };

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
        //
        // Deliberately silent on a poisoned lock (finding #74), unlike
        // every other lock site in this file: a prior panic while holding
        // this mutex means we're already unwinding, and calling
        // `.expect()` here (panicking again, inside `Drop`, during an
        // unwind already in progress) would abort the process instead of
        // completing that unwind -- worse than skipping this cleanup step.
        if let Ok(mut pool) = self.transient_pool.lock() {
            pool.free.clear();
        }
        // SAFETY: `self` is being dropped, so no other code holds
        // references to these handles afterward; destroying the fences and
        // command pools (children of the device) before the device, and
        // the device before the instance, follows Vulkan's required
        // child-before-parent destruction order. Any `VulkanRingBuffer`s
        // created from this device hold a clone of `self.frame_sync`'s
        // `Arc`, not this fence independently, so they don't outlive this
        // destruction in a way that would use-after-free it --
        // `VulkanRingBuffer` never touches `frame_sync.fence` at all (see
        // its own doc comment).
        unsafe {
            self.device
                .destroy_descriptor_pool(self.bindless_descriptor_pool, None);
            self.device
                .destroy_descriptor_set_layout(self.bindless_descriptor_set_layout, None);
            self.device.destroy_sampler(self.bindless_sampler, None);
            self.device.destroy_fence(self.frame_sync.fence, None);
            self.device.destroy_command_pool(self.command_pool, None);
            if let Ok(pool) = self.upload_command_pool.lock() {
                self.device.destroy_command_pool(*pool, None);
            }
            self.device.destroy_device(None);
        }
        // Destroyed after the device but before the instance: the
        // messenger is a child of the INSTANCE (created via an
        // `ash::ext::debug_utils::Instance` loader), not the device, so it
        // must not outlive `destroy_instance` below.
        #[cfg(debug_assertions)]
        if let Some((debug_utils_loader, messenger)) = self.debug_utils.take() {
            // SAFETY: `messenger` was created from `debug_utils_loader` on
            // this same `self.instance`, both still valid at this point;
            // `self` is being dropped, so nothing else can reference
            // `messenger` afterward.
            unsafe {
                debug_utils_loader.destroy_debug_utils_messenger(messenger, None);
            }
        }
        // SAFETY: `self.instance` is valid and every child object
        // (device, messenger) has been destroyed above.
        unsafe {
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
        // Phase 2 Code Review finding #73: clamped before rounding up, so
        // this is unconditionally safe regardless of `width`/`height`
        // (see `release_transient_target`'s matching clamp for why).
        let bucket = (
            width.min(1 << 30).next_power_of_two(),
            height.min(1 << 30).next_power_of_two(),
            format,
        );
        let mut pool = self.transient_pool.lock().expect("transient pool poisoned");

        if let Some(texture) = pool.free.get_mut(&bucket).and_then(Vec::pop) {
            pool.stats.hits += 1;
            // IMPLEMENTATION.md Step 2.3: leaving the free list means this
            // texture is in active use again, not idle -- it no longer
            // counts toward the GC trigger.
            pool.total_free_bytes -= texture.size_bytes;
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
            // IMPLEMENTATION.md Step 2.3: same accounting as the exact-hit
            // path above -- this texture is leaving the free list too.
            pool.total_free_bytes -= texture.size_bytes;
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
        // Phase 2 Code Review finding #70: this function's raw-handle
        // reconstruction below assumes `texture` came from
        // `acquire_transient_target`, which never assigns a bindless
        // index. Nothing in the `RhiTexture`/`RhiDevice` trait boundary
        // actually prevents a caller from passing a `create_texture`-
        // sourced (bindless) texture here instead -- if that happened, the
        // naive reconstruction would silently strand its bindless slot
        // (never returned to `BindlessRegistry`'s free list) and pool a
        // `SAMPLED | TRANSFER_DST` image as if it were a `COLOR_ATTACHMENT`
        // render target. Detect that misuse and let `texture` drop
        // normally instead -- its own `Drop` (`impl Drop for
        // VulkanTexture`) correctly destroys its GPU resources AND
        // releases its bindless slot, which is exactly the right behavior
        // for a texture that was never meant to be pooled.
        if texture.bindless_index().is_some() {
            debug_assert!(
                false,
                "release_transient_target called with a bindless (create_texture) texture; \
                 dropping it instead of pooling it"
            );
            return;
        }

        let (width, height) = texture.dimensions();
        let format = texture.format();
        let bucket = (
            // Phase 2 Code Review finding #73: `next_power_of_two` panics
            // (debug) or silently wraps to 0 (release) for inputs above
            // `2^31 - 1`. Clamping first is a no-op for every realistic
            // texture request and makes the call unconditionally safe.
            width.min(1 << 30).next_power_of_two(),
            height.min(1 << 30).next_power_of_two(),
            format,
        );
        // Captured before `texture` is forgotten below: `size_bytes()`
        // (IMPLEMENTATION.md Step 2.3) round-trips the allocation size
        // `VulkanTexture::new` already computed, so check-in doesn't need
        // to re-query `vkGetImageMemoryRequirements`.
        let size_bytes = texture.size_bytes();
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
            // Transient render targets never enter the bindless array
            // (IMPLEMENTATION.md Step 2.1's scope decision) -- confirmed
            // above (the misuse guard already returned otherwise), so
            // there is nothing to reconstruct here.
            bindless_index: None,
            bindless_registry: None,
            // IMPLEMENTATION.md Step 2.3: "used" right now, at the moment
            // of check-in -- what the GC thread's staleness check measures
            // age from.
            last_used_frame: self.frame_sync.total_frame_count.load(Ordering::Acquire),
            size_bytes,
        };
        // `texture` (the original box) must not also run its `Drop` and
        // destroy these same handles out from under `reclaimed`.
        std::mem::forget(texture);

        let mut pool = self.transient_pool.lock().expect("transient pool poisoned");
        pool.total_free_bytes += size_bytes;
        pool.free.entry(bucket).or_default().push(reclaimed);
    }

    fn create_texture(
        &self,
        width: u32,
        height: u32,
        format: TextureFormat,
        pixels: &[u8],
    ) -> Result<Box<dyn RhiTexture>, EngineError> {
        Ok(Box::new(VulkanTexture::from_pixels(
            self, width, height, format, pixels,
        )?))
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
        // IMPLEMENTATION.md Step 2.3: same frame-boundary rationale as
        // `grow_pending_transient_targets` above -- destroying GC-evicted
        // resources happens here, once per frame, not mid-frame.
        self.drain_deferred_release_queue();

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
                bindless_descriptor_set: self.bindless_descriptor_set,
                bindless_capacity: self.bindless_capacity,
                texture_index: BINDLESS_TEXTURE_SENTINEL,
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
        // IMPLEMENTATION.md Step 2.3: the genuinely monotonic counter the
        // GC thread and `begin_frame`'s deferred-release drain use to
        // judge resource age -- distinct from `frame_index` above, which
        // only ever counts 0..FRAMES_IN_FLIGHT. `fetch_add` (not
        // load-then-store) is fine here even though nothing currently
        // reads the returned value, since this field is never rotated
        // (no modulus to get wrong).
        self.frame_sync
            .total_frame_count
            .fetch_add(1, Ordering::Release);

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

/// The push-constant layout `create_pipeline`'s universal pipeline layout
/// declares (IMPLEMENTATION.md Step 2.1): 12 bytes total, no padding
/// (`[f32; 2]` then `u32`, both 4-byte aligned).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PushConstants {
    screen_size: [f32; 2],
    texture_index: u32,
}

pub struct VulkanCommandBuffer {
    device: ash::Device,
    command_buffer: vk::CommandBuffer,
    width: u32,
    height: u32,
    pipeline_layout: Option<vk::PipelineLayout>,
    /// The one persistent bindless descriptor set (`VulkanDevice::
    /// bindless_descriptor_set`), bound once per `set_pipeline` call.
    bindless_descriptor_set: vk::DescriptorSet,
    /// The real, runtime-clamped bindless array size (`VulkanDevice::
    /// bindless_capacity`), used by `bind_texture` to bounds-check its
    /// `bindless_index` argument (Phase 2 Code Review finding #69) before
    /// it can ever reach the GPU as an out-of-range descriptor index.
    bindless_capacity: u32,
    /// The bindless array index `draw_indexed` will push next, set by
    /// `bind_texture`. Starts at `BINDLESS_TEXTURE_SENTINEL` ("no texture,
    /// use vertex color") so a draw that never calls `bind_texture` keeps
    /// behaving exactly like Phase 0's flat-color path.
    texture_index: u32,
}

impl RhiCommandBuffer for VulkanCommandBuffer {
    fn set_pipeline(&mut self, pipeline: &dyn RhiPipelineState) {
        let raw = vk::Pipeline::from_raw(pipeline.raw_handle());
        let layout = vk::PipelineLayout::from_raw(pipeline.layout_handle());
        self.pipeline_layout = Some(layout);
        // SAFETY: `self.command_buffer` is recording (allocated once and
        // reset/begun per frame by `VulkanDevice::begin_frame`); `raw` is a
        // pipeline handle the `RhiPipelineState` trait contract guarantees
        // was created by this same device and is still alive; `layout` is
        // that same pipeline's own layout, which `create_pipeline` always
        // builds against `self.bindless_descriptor_set`'s layout at set 0,
        // so binding `self.bindless_descriptor_set` here is always
        // compatible with whatever pipeline was just bound.
        unsafe {
            self.device.cmd_bind_pipeline(
                self.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                raw,
            );
            // IMPLEMENTATION.md Step 2.1: bound exactly once per pipeline
            // bind, never rebound between draws that sample different
            // textures -- selecting a texture is purely the push-constant
            // write in `draw_indexed` below (via `bind_texture`), which is
            // the entire performance point of a bindless array.
            self.device.cmd_bind_descriptor_sets(
                self.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                layout,
                0,
                &[self.bindless_descriptor_set],
                &[],
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

    fn bind_texture(&mut self, slot: u32, bindless_index: u32) {
        // Only one bindless array/slot exists this step (IMPLEMENTATION.md
        // Step 2.1's explicit scope -- a second slot, e.g. a separate
        // mask-atlas array, is future work, not built speculatively here).
        // Phase 2 Code Review finding #75: loud in debug builds, but a
        // safe no-op (not silent misbinding into slot 0) in release --
        // `bind_texture` has no `Result` to report this through.
        debug_assert_eq!(
            slot, 0,
            "slot 0 is the only bindless array this step supports"
        );
        if slot != 0 {
            return;
        }

        // Phase 2 Code Review finding #69: `bindless_index` is an
        // arbitrary caller-supplied `u32` with nothing upstream validating
        // it against the real (runtime-clamped) array size. An in-range
        // check here, not just a debug assertion, keeps an out-of-range
        // value from ever reaching the GPU as a descriptor-array index
        // (driver-defined behavior the validation layer's static checks
        // cannot catch, since the index is a fully dynamic per-draw
        // value) -- falling back to the safe "no texture" sentinel instead
        // of passing it through.
        let in_range =
            bindless_index == BINDLESS_TEXTURE_SENTINEL || bindless_index < self.bindless_capacity;
        debug_assert!(
            in_range,
            "bindless_index {bindless_index} is out of range (capacity {})",
            self.bindless_capacity
        );
        self.texture_index = if in_range {
            bindless_index
        } else {
            BINDLESS_TEXTURE_SENTINEL
        };
    }

    fn draw_indexed(&mut self, index_count: u32, start_index: u32, base_vertex: i32) {
        // Phase 0 has no transform stack yet (IMPLEMENTATION.md Phase 3);
        // push the screen size the vertex shader needs to map pixel-space
        // positions to NDC, plus (IMPLEMENTATION.md Step 2.1) which
        // bindless array slot, if any, this draw samples from.
        let push = PushConstants {
            screen_size: [self.width as f32, self.height as f32],
            texture_index: self.texture_index,
        };
        // SAFETY: `self.command_buffer` is recording; `self.pipeline_layout`
        // was set by `set_pipeline` (asserted via `.expect` above) and
        // matches the layout `create_pipeline` declared its push constant
        // range against, and `push`'s 12-byte size matches the 12-byte
        // range reserved there.
        unsafe {
            self.device.cmd_push_constants(
                self.command_buffer,
                self.pipeline_layout
                    .expect("set_pipeline must be called before draw_indexed"),
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
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
    /// Entries the background GC thread has moved from the free list into
    /// the deferred-release queue (IMPLEMENTATION.md Step 2.3) -- not yet
    /// physically destroyed, just no longer available for reuse. Distinct
    /// from `destroyed` so a demo/test can observe both phases of the
    /// deferred-release design, not just an aggregate.
    pub evictions: u64,
    /// Entries the main thread has actually destroyed after their 3-frame
    /// grace period elapsed (IMPLEMENTATION.md Step 2.3).
    pub destroyed: u64,
}

/// An evicted transient texture awaiting `DEFERRED_RELEASE_GRACE_FRAMES`
/// before the main thread physically destroys it (IMPLEMENTATION.md
/// Step 2.3). Holds the real `VulkanTexture` -- moving it here (rather
/// than, say, just its raw handles) means its own `Drop` does the correct
/// image/view/memory teardown once this struct is finally dropped for
/// real, with nothing further to reconstruct.
struct DeferredRelease {
    // Never read by field name -- its only purpose is to be dropped, at
    // the right moment, running `VulkanTexture`'s own real teardown.
    // `dead_code` can't see a field-access-free drop as a "use," so this
    // is a deliberate `allow`, not an oversight.
    #[allow(dead_code)]
    texture: VulkanTexture,
    /// `FrameSync::total_frame_count` at the moment the GC thread evicted
    /// this entry -- what the main thread's grace-period check compares
    /// against the current frame count.
    evicted_at_frame: u64,
}

/// The background GC thread's entire loop body (IMPLEMENTATION.md
/// Step 2.3) -- the engine's first genuine OS thread. Deliberately never
/// calls a single Vulkan function: it only locks `transient_pool` (plain
/// Rust `HashMap`/`Vec` manipulation) and moves evicted `VulkanTexture`
/// values into `deferred_release`. The actual `vkDestroy*` calls happen
/// later, on the main thread, in `VulkanDevice::begin_frame`'s deferred-
/// release drain -- see PLAN_PHASE2_STEP2_3.md's "why this is safe despite
/// being genuinely concurrent" for the full reasoning. Exits when
/// `running` is set to `false` by `Drop for VulkanDevice`.
fn gc_thread_loop(
    transient_pool: Arc<Mutex<TransientPool>>,
    frame_sync: Arc<FrameSync>,
    deferred_release: Arc<Mutex<VecDeque<DeferredRelease>>>,
    running: Arc<AtomicBool>,
) {
    while running.load(Ordering::Acquire) {
        std::thread::sleep(GC_SCAN_INTERVAL);
        // Re-check immediately after waking: `Drop for VulkanDevice` may
        // have requested shutdown while this thread was asleep, and
        // `transient_pool` may already be mid-teardown by the time a
        // sleep started before shutdown finishes.
        if !running.load(Ordering::Acquire) {
            break;
        }

        let mut pool = transient_pool.lock().expect("transient pool poisoned");
        if pool.total_free_bytes < GC_TRIGGER_THRESHOLD_BYTES {
            continue;
        }

        // IMPLEMENTATION.md Step 2.3 task 3: evict every free-list entry
        // older than `GC_EVICTION_AGE_FRAMES`, once triggered -- not just
        // enough to get back under budget, matching the task's literal
        // wording ("identify resources older than N = 600 frames").
        let current_frame = frame_sync.total_frame_count.load(Ordering::Acquire);
        let mut evicted = Vec::new();
        for textures in pool.free.values_mut() {
            let mut i = 0;
            while i < textures.len() {
                let age = current_frame.saturating_sub(textures[i].last_used_frame);
                if age > GC_EVICTION_AGE_FRAMES {
                    // `swap_remove`, not `remove`: free-list order is
                    // irrelevant, and this avoids an O(n) shift per
                    // eviction. Does NOT increment `i` -- the element
                    // swapped into this position hasn't been checked yet.
                    // (Byte accounting updated after this loop, once
                    // `pool.free`'s borrow above has ended -- mutating
                    // `pool.total_free_bytes` here too would borrow `pool`
                    // mutably a second time while `values_mut()`'s
                    // iterator is still live.)
                    evicted.push(textures.swap_remove(i));
                } else {
                    i += 1;
                }
            }
        }
        pool.total_free_bytes -= evicted.iter().map(|t| t.size_bytes).sum::<u64>();
        if evicted.is_empty() {
            continue;
        }
        pool.stats.evictions += evicted.len() as u64;
        // Released before taking `deferred_release`'s lock: nothing below
        // needs `transient_pool` any further this iteration, and the main
        // thread should never have to wait on the GC thread's scan just to
        // acquire/release a transient target.
        drop(pool);

        let mut queue = deferred_release
            .lock()
            .expect("deferred release queue poisoned");
        for texture in evicted {
            queue.push_back(DeferredRelease {
                texture,
                evicted_at_frame: current_frame,
            });
        }
    }
}

/// Free-list + bump allocator for `VulkanDevice::bindless_descriptor_set`'s
/// array slots (IMPLEMENTATION.md Step 2.1). Same shape as `TransientPool`'s
/// checked-in/checked-out bookkeeping, for the same reason: a small,
/// `Mutex`-guarded piece of device state that many textures' lifetimes touch
/// independently.
struct BindlessRegistry {
    /// Indices below this bound have been assigned at least once.
    next: u32,
    /// The real ceiling -- `VulkanDevice::new`'s runtime-clamped
    /// `bindless_capacity`, not `BINDLESS_TEXTURE_CAPACITY_TARGET`
    /// unconditionally, since a real device (a software rasterizer, most
    /// plausibly) may support fewer update-after-bind sampled images than
    /// the sort key's 4,096-slot target.
    capacity: u32,
    /// Indices released by a dropped `VulkanTexture`, available for reuse
    /// before bumping `next`.
    free: Vec<u32>,
}

impl BindlessRegistry {
    fn new(capacity: u32) -> Self {
        Self {
            next: 0,
            capacity,
            free: Vec::new(),
        }
    }

    /// Returns `None` if every slot up to `capacity` is currently live --
    /// exhausting the bindless array is a real, reportable condition, not
    /// something to paper over with an unbounded `Vec`.
    fn allocate(&mut self) -> Option<u32> {
        if let Some(index) = self.free.pop() {
            return Some(index);
        }
        if self.next < self.capacity {
            let index = self.next;
            self.next += 1;
            return Some(index);
        }
        None
    }

    fn release(&mut self, index: u32) {
        self.free.push(index);
    }
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
    /// Sum of `size_bytes` across every entry currently in `free`
    /// (IMPLEMENTATION.md Step 2.3) -- what the GC thread compares against
    /// `GC_TRIGGER_THRESHOLD_BYTES`. Maintained incrementally (added to on
    /// check-in, subtracted on check-out/eviction) rather than recomputed
    /// by summing `free` on every scan, since the GC thread's scan
    /// interval is independent of how often the pool itself changes.
    total_free_bytes: u64,
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
    /// This texture's slot in the bindless array (IMPLEMENTATION.md
    /// Step 2.1), if it has one. `None` for a transient render target
    /// (`VulkanTexture::new`) -- only `VulkanTexture::from_pixels`
    /// (`RhiDevice::create_texture`'s backing) registers one.
    bindless_index: Option<u32>,
    /// A clone of the owning `VulkanDevice`'s registry `Arc`, so `Drop` can
    /// release `bindless_index` back to the free list without holding a
    /// reference to the whole device. `None` exactly when `bindless_index`
    /// is `None`.
    bindless_registry: Option<Arc<Mutex<BindlessRegistry>>>,
    /// IMPLEMENTATION.md Step 2.3: the `FrameSync::total_frame_count` value
    /// as of this texture's creation, or (for a pooled texture) its last
    /// `release_transient_target` check-in -- what the GC thread compares
    /// against the current frame count to judge staleness. Meaningless for
    /// a bindless texture (`from_pixels`'s output), which never enters
    /// `TransientPool::free` and so is never scanned.
    last_used_frame: u64,
    /// This texture's own `VkMemoryRequirements::size` -- what
    /// `TransientPool::total_free_bytes` sums to decide whether the pool
    /// has crossed IMPLEMENTATION.md Step 2.3's 85%-of-budget GC trigger.
    size_bytes: u64,
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
    fn from_pixels(
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
        let expected_len = u64::from(width) * u64::from(height) * bytes_per_pixel(format);
        if pixels.len() as u64 != expected_len {
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

        // Phase 2 Code Review finding #72: allocated from
        // `upload_command_pool`, NOT the frame loop's `command_pool` --
        // see that field's doc comment on `VulkanDevice` for why sharing
        // one pool between the two would be an unsynchronized Vulkan spec
        // violation.
        //
        // SAFETY: `upload_pool` is the valid, still-alive pool created in
        // `VulkanDevice::new`.
        let upload_pool = *device
            .upload_command_pool
            .lock()
            .expect("upload command pool poisoned");
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

        // Register into the bindless array: a free slot, assigned once,
        // written via a single `vkUpdateDescriptorSets` call. Exhausting
        // `bindless_capacity` is a real, reportable condition (Phase 2
        // Code Review finding #67 -- `RhiDevice::create_texture` now
        // actually propagates this `Result` instead of `.expect()`-ing it
        // away).
        let bindless_index = device
            .bindless_registry
            .lock()
            .expect("bindless registry poisoned")
            .allocate()
            .ok_or(EngineError::BindlessArrayExhausted)?;

        let image_info = vk::DescriptorImageInfo::default()
            .image_view(view)
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        let write = vk::WriteDescriptorSet::default()
            .dst_set(device.bindless_descriptor_set)
            .dst_binding(1)
            .dst_array_element(bindless_index)
            .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
            .image_info(std::slice::from_ref(&image_info));
        // SAFETY: `device.device` is valid; `device.bindless_descriptor_set`
        // was allocated in `VulkanDevice::new` from a layout whose binding 0
        // has `UPDATE_AFTER_BIND`, so writing to it here (potentially while
        // other draws using this same set are in flight, though this
        // step's scope keeps submission fully synchronous anyway) is
        // explicitly permitted; `bindless_index` was just allocated above
        // so it is `< bindless_capacity`, and `view` was just created on
        // this same device.
        unsafe {
            device.device.update_descriptor_sets(&[write], &[]);
        }

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
