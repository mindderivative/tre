//! `VulkanDevice` -- the `RhiDevice` implementation and the crate's
//! central shared-state type (ARCHITECTURE.md Section 2.1's "Global
//! `RhiDevice`"): instance/physical-device/logical-device setup, the
//! bindless texture array, the transient render-target pool's frame-loop
//! side (its background GC thread lives in `crate::transient_pool`), and
//! per-frame submission (`begin_frame`/`submit_and_present`). Split out
//! of `lib.rs` as one of its ten separable concerns (Architecture review
//! finding) -- the last and largest of the ten, kept as one file since
//! (like `tre-engine`'s own `canvas.rs`/`shapes.rs`) it is genuinely one
//! cohesive concern, not several independent ones bundled together.

use std::collections::VecDeque;
use std::ffi::{c_char, CStr};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use ash::vk;
use ash::vk::Handle;
use tre_engine::{
    AcquiredImage, BeginFrameOptions, EngineError, RhiCommandBuffer, RhiDevice,
    RhiDynamicRingBuffer, RhiPipelineState, RhiSwapchain, RhiTexture, TextureFormat, UiVertex,
};

use crate::blur::BlurResources;
use crate::transient_pool::{gc_thread_loop, BindlessRegistry, DeferredRelease, TransientPool};
use crate::{
    TransientPoolStats, VulkanBuffer, VulkanCommandBuffer, VulkanPipelineState, VulkanRingBuffer,
    VulkanTexture, BINDLESS_TEXTURE_CAPACITY_TARGET, BINDLESS_TEXTURE_SENTINEL,
    DEFERRED_RELEASE_GRACE_FRAMES, DYNAMIC_VRAM_BUDGET_BYTES, FRAMES_IN_FLIGHT,
    REQUIRED_DEVICE_EXTENSIONS, SHAPE_STYLE_BUFFER_CAPACITY,
};

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
pub(crate) struct FrameSync {
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
    pub(crate) fence: vk::Fence,
    /// A rotating counter (0, 1, 2, 0, 1, 2, ...), advanced by
    /// `VulkanDevice::submit_and_present` after every frame. Used ONLY by
    /// `VulkanRingBuffer` to pick which of its 3 segments is "current" --
    /// safe without its own per-segment fence precisely because `fence`
    /// above already fully synchronizes every single frame, so by the
    /// time this counter cycles back to a given value, at least two other
    /// fully-synchronous frames have completed since that segment was
    /// last written.
    pub(crate) frame_index: AtomicUsize,
    /// A genuinely monotonic, ever-increasing frame counter (IMPLEMENTATION.md
    /// Step 2.3), advanced alongside `frame_index` by `submit_and_present`
    /// but never wrapping -- `frame_index`'s 0..3 rotation answers "which
    /// ring-buffer segment," this answers "how many frames old is this
    /// resource." Read by both the main thread (grace-period checks in
    /// `begin_frame`'s deferred-release drain) and the background GC
    /// thread (staleness checks against `VulkanTexture::last_used_frame`).
    pub(crate) total_frame_count: AtomicU64,
}

/// Shared Vulkan device state (ARCHITECTURE.md Section 2.1's "Global
/// `RhiDevice`"). Frame submission itself stays fully synchronous (one
/// frame in flight, `begin_frame` fully waits before recording) -- see
/// `frame_sync`'s doc comment for the real, once-broken-then-fixed reason
/// it still tracks a rotating index despite that.
pub struct VulkanDevice {
    pub(crate) entry: ash::Entry,
    /// The real Vulkan instance this device was created against.
    pub instance: ash::Instance,
    /// The physical device (GPU) `new` selected.
    pub physical_device: vk::PhysicalDevice,
    /// A combined depth/stencil format this physical device actually
    /// supports for `DEPTH_STENCIL_ATTACHMENT_OPTIMAL` tiling (queried
    /// once in `new`, IMPLEMENTATION.md Step 3.3.3) -- every swapchain's
    /// stencil image, and every pipeline's declared
    /// `stencilAttachmentFormat`, uses this same format.
    pub stencil_format: vk::Format,
    /// The real logical device every other Vulkan call in this crate is
    /// issued against.
    pub device: ash::Device,
    /// The graphics+present-capable queue family index `new` selected;
    /// `graphics_queue()` is the actual `VkQueue` handle from this family.
    pub queue_family_index: u32,
    pub(crate) graphics_queue: vk::Queue,
    pub(crate) command_pool: vk::CommandPool,
    pub(crate) command_buffer: vk::CommandBuffer,
    pub(crate) dynamic_rendering: ash::khr::dynamic_rendering::Device,
    pub(crate) frame_sync: Arc<FrameSync>,
    /// Transient render target pool (TECHNICAL.md Section 3.2), keyed by
    /// `(width, height, format)` after power-of-two bucket rounding.
    /// `Mutex`-guarded (not `RefCell`) so `VulkanDevice` stays genuinely
    /// `Sync`-shareable across threads later, matching the same
    /// forward-looking reasoning as `tre_memory::SpscRingBuffer`.
    /// `Arc`-wrapped (IMPLEMENTATION.md Step 2.3) so the background GC
    /// thread can hold its own clone -- the "later" the doc comment above
    /// refers to has arrived.
    pub(crate) transient_pool: Arc<Mutex<TransientPool>>,
    /// `RhiCommandBuffer::apply_layer_blur`'s own non-bindless descriptor
    /// set/pool/sampler/pipeline-layout/2-pipeline resources
    /// (IMPLEMENTATION.md Step 7.2.2), lazily created on the first real
    /// call -- `None` until then. `Arc<Mutex<..>>` for the same reason
    /// `transient_pool` is: `VulkanCommandBuffer` holds its own clone
    /// (set in `begin_frame`, mirroring `bindless_descriptor_set`'s own
    /// copied-at-construction precedent immediately below), since
    /// `apply_layer_blur`'s trait signature takes `device: &dyn
    /// RhiDevice` -- a trait object with no way back to this concrete
    /// field -- not `&VulkanDevice` directly.
    pub(crate) blur_resources: Arc<Mutex<Option<BlurResources>>>,
    /// A single shared sampler used by every bindless-array texture
    /// (IMPLEMENTATION.md Step 2.1) -- baked into
    /// `bindless_descriptor_set_layout` as an immutable sampler, so it is
    /// never itself written via `vkUpdateDescriptorSets`.
    pub(crate) bindless_sampler: vk::Sampler,
    pub(crate) bindless_descriptor_pool: vk::DescriptorPool,
    pub(crate) bindless_descriptor_set_layout: vk::DescriptorSetLayout,
    /// The one persistent descriptor set every pipeline binds (see
    /// `create_pipeline`/`VulkanCommandBuffer::set_pipeline`) -- bindless
    /// means this is bound exactly once and never rebound between draws
    /// that reference different textures, unlike traditional per-texture
    /// descriptor sets.
    pub(crate) bindless_descriptor_set: vk::DescriptorSet,
    /// Which of `bindless_descriptor_set`'s array slots (binding 2, since
    /// Phase 10 Step 10.2 renumbered it from binding 1) are currently
    /// assigned to a live texture. `Mutex`-guarded for the same
    /// forward-looking reason as `transient_pool`; `Arc`-wrapped (like
    /// `frame_sync`) so every `VulkanTexture` created via `create_texture`
    /// can hold a clone and release its own slot on `Drop` without needing
    /// to reach back through a whole `VulkanDevice`.
    pub(crate) bindless_registry: Arc<Mutex<BindlessRegistry>>,
    /// The real, runtime-clamped size of the bindless array (`min(4096,
    /// maxDescriptorSetUpdateAfterBindSampledImages)`), cached here for
    /// `VulkanCommandBuffer::bind_texture` to bounds-check against without
    /// locking `bindless_registry` (Phase 2 Code Review finding #69).
    pub(crate) bindless_capacity: u32,
    /// Phase 10 Step 10.2: the bindless set's binding-2 `STORAGE_BUFFER`
    /// -- a `VulkanRingBuffer` reused for a new purpose (per-shape
    /// `GpuShapeStyle` records: non-uniform corner radii, border,
    /// gradient stops, etc.) rather than vertex/index data, since
    /// `UiVertex`'s hard 32-byte layout (`params: [f32; 3]`) has no room
    /// for it -- see `documentation/ARCHITECTURE.md` Section 7's "Shape
    /// Style Buffer" write-up. Bound to the bindless descriptor set
    /// exactly once, in `new` (like `bindless_sampler`'s immutable
    /// sampler), since the buffer OBJECT never changes across a
    /// `VulkanDevice`'s lifetime -- only its contents, via the same
    /// persistent-mapped `write` every ring buffer already supports.
    /// `Option` only so `Drop for VulkanDevice` can `.take()` it and let
    /// it destroy its own Vulkan resources (via its own cloned
    /// `ash::Device`) BEFORE `destroy_device` runs, the same early-drop
    /// pattern `gc_thread`/`blur_resources` already use here -- unlike
    /// those, this is never `None` while the device is alive.
    pub(crate) shape_style_buffer: Option<VulkanRingBuffer>,
    /// A command pool dedicated to `VulkanTexture::from_pixels`'s one-time
    /// upload command buffers -- deliberately SEPARATE from `command_pool`
    /// above (the per-frame render loop's pool). Vulkan requires external
    /// synchronization on a command pool for `vkAllocateCommandBuffers`/
    /// `vkFreeCommandBuffers`; sharing one pool between the frame loop and
    /// texture uploads would need its own synchronization, which nothing
    /// provided (Phase 2 Code Review finding #72). `Mutex`-guarded so
    /// concurrent `create_texture` calls from multiple threads serialize
    /// safely instead of racing each other.
    pub(crate) upload_command_pool: Mutex<vk::CommandPool>,
    /// Evicted transient textures awaiting their 3-frame grace period
    /// (IMPLEMENTATION.md Step 2.3) before the main thread actually
    /// destroys them in `begin_frame`. A plain `Mutex<VecDeque<_>>`, not a
    /// lock-free structure -- see PLAN.md's "deliberate deviation from
    /// lock-free queue" for why (peeking the front without consuming it is
    /// needed here, and contention at this call frequency is negligible).
    /// `Arc`-wrapped so the GC thread (sole producer) and the main thread
    /// (sole consumer) each hold their own clone.
    pub(crate) deferred_release: Arc<Mutex<VecDeque<DeferredRelease>>>,
    /// Set to `false` by `Drop for VulkanDevice` to tell the GC thread to
    /// exit its scan loop; joined immediately after.
    pub(crate) gc_running: Arc<AtomicBool>,
    /// The background GC thread's handle (IMPLEMENTATION.md Step 2.3) --
    /// the engine's first genuine OS thread. `Option` only so `Drop` can
    /// `.take()` it to call `.join()`, which consumes the handle.
    pub(crate) gc_thread: Option<JoinHandle<()>>,
    /// `VK_EXT_debug_utils` messenger (TECHNICAL.md Section 9.2,
    /// IMPLEMENTATION.md Step 2.4), `None` if the validation
    /// layer/extension weren't both available at instance-creation time.
    /// The field itself doesn't exist in release builds -- compiled out
    /// entirely, matching TECHNICAL.md Section 3.4's zero-allocation
    /// guard's own release-build behavior, so there is no cost (not even
    /// an unused `Option`) in a shipped binary.
    #[cfg(debug_assertions)]
    pub(crate) debug_utils: Option<(ash::ext::debug_utils::Instance, vk::DebugUtilsMessengerEXT)>,
    /// Backing storage for `debug_utils`'s messenger's own `pUserData` --
    /// see `DebugCallbackState`'s doc comment. Always allocated in a debug
    /// build (even when `debug_utils` ends up `None`, i.e. validation
    /// wasn't available -- cheap, and keeps this field's type simple), but
    /// only ever actually pointed to by Vulkan when `debug_utils` is
    /// `Some`. Kept alive here for exactly as long as `debug_utils` itself
    /// would need it, since Vulkan holds the raw pointer into this box for
    /// the messenger's whole lifetime, not just for the `vkCreateDevice`
    /// call that first needed it. Never read after `new` returns -- it
    /// exists purely so `vulkan_debug_callback` has somewhere to report
    /// back to during that one call.
    #[cfg(debug_assertions)]
    #[allow(
        dead_code,
        reason = "never read back -- kept solely so its heap allocation (and the raw pointer \
                  Vulkan holds into it) stays alive for exactly as long as `debug_utils`'s \
                  messenger does, matching that field's own lifetime discipline"
    )]
    debug_callback_state: Box<DebugCallbackState>,
    /// Phase 10 Step 10.2.3: `true` only when this physical device
    /// actually advertises `VK_KHR_dynamic_rendering_local_read`
    /// (queried once in `new`, mirroring `debug_validation_available`'s
    /// own optional-extension pattern). `RhiDevice::
    /// local_read_blend_supported` reports this value so `shapes.rs`'s
    /// dispatch fails closed to ordinary `Normal` blending on any
    /// driver that lacks it -- `VK_EXT_blend_operation_advanced` (this
    /// step's originally-planned primary path) turned out to be one
    /// such driver (RADV, this project's own real dev GPU); see
    /// `documentation/REVIEW.md` for the full account.
    pub(crate) local_read_supported: bool,
    /// Phase 10 Step 10.2.3: the dedicated set-1 resources every
    /// `PipelineKind::FlatColorBlend` pipeline's layout declares --
    /// `None` when `local_read_supported` is `false`, so nothing here
    /// is ever built, bound, or dereferenced on hardware without the
    /// capability. The descriptor SET's contents (which image view it
    /// points at) are rewritten every frame in `begin_frame`, since the
    /// color attachment view can change frame to frame.
    blend_read: Option<BlendReadResources>,
}

/// See `VulkanDevice::blend_read`'s own doc comment.
struct BlendReadResources {
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set: vk::DescriptorSet,
}

/// Per-`VulkanDevice::new` state threaded through `vulkan_debug_callback`
/// via `VkDebugUtilsMessengerCreateInfoEXT::pUserData` -- deliberately not
/// a global/static flag, since a global would race across concurrent
/// `VulkanDevice::new()` calls in the same process (`cargo test` runs test
/// threads in parallel, and more than one test creates a `VulkanDevice`).
/// Boxed by `new` and kept alive on `VulkanDevice::debug_callback_state`
/// for as long as the messenger itself exists, since Vulkan holds the raw
/// pointer for the messenger's whole lifetime, not just for the one call
/// that first needed it.
#[cfg(debug_assertions)]
struct DebugCallbackState {
    /// Set the moment this callback sees the validation layer's own
    /// self-diagnostic for not recognizing
    /// `VK_KHR_dynamic_rendering_local_read` (see
    /// `is_local_read_rejected_by_layer`'s doc comment) -- checked once,
    /// immediately after `create_device` returns, to decide whether the
    /// rest of `new` may actually use the extension it just speculatively
    /// requested.
    local_read_rejected: AtomicBool,
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
    user_data: *mut std::ffi::c_void,
) -> vk::Bool32 {
    // SAFETY: `callback_data` is supplied by the Vulkan loader for the
    // duration of this call only, per `VK_EXT_debug_utils`'s contract,
    // and its `p_message` is always a valid, NUL-terminated C string when
    // this callback fires.
    let message = unsafe { CStr::from_ptr((*callback_data).p_message) }.to_string_lossy();
    eprintln!("[Vulkan {message_severity:?} {message_type:?}] {message}");

    if is_local_read_rejected_by_layer(&message) {
        // SAFETY: `user_data`, when non-null, is the `DebugCallbackState`
        // this same `VulkanDevice::new` boxed and registered as
        // `pUserData` when it created this messenger; it is kept alive on
        // `VulkanDevice::debug_callback_state` for the messenger's whole
        // lifetime, which is exactly when this callback can fire.
        if let Some(state) = unsafe { user_data.cast::<DebugCallbackState>().as_ref() } {
            state.local_read_rejected.store(true, Ordering::Relaxed);
        }
    }

    if message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR)
        && !is_known_false_positive(&message)
    {
        std::process::abort();
    }
    vk::FALSE
}

/// Detects the validation layer's own two self-diagnostics for not
/// recognizing `VK_KHR_dynamic_rendering_local_read` (Step 10.2.3's real,
/// legitimate, hardware-gated extension -- see REVIEW.md finding #170),
/// confirmed on GitHub Actions' `ubuntu-latest` runner where Ubuntu's
/// `vulkan-validationlayers` apt package (pinned to an older Vulkan
/// header revision) and its separately-versioned `mesa-vulkan-drivers`
/// package drift out of lockstep as Ubuntu updates each on its own
/// schedule. Both diagnostics fire during the very same `vkCreateDevice`
/// call that speculatively chains in the extension's feature struct:
///
/// - The layer's own plain-language `WARNING`: "vkCreateDevice():
///   pCreateInfo->ppEnabledExtensionNames[3] VK_KHR_dynamic_rendering
///   _local_read is not supported by this layer."
/// - The resulting `ERROR` (`VUID-VkDeviceCreateInfo-pNext-pNext`):
///   "pCreateInfo->pNext chain includes a structure with unknown
///   VkStructureType (1000232000)... It is possible that you are using a
///   struct from a private extension or an extension that was added to a
///   later version of the Vulkan header."
///
/// Either one, alone, is a reliable, real signal straight from the layer
/// that it cannot validate this extension -- used by `VulkanDevice::new`
/// to disable the extension for the rest of the process instead of using
/// it and then chasing each new downstream VUID this same root cause
/// produces once the extension is actually exercised (REVIEW.md finding
/// #208's own multi-round history of that approach).
#[cfg(debug_assertions)]
fn is_local_read_rejected_by_layer(message: &str) -> bool {
    (message.contains("VK_KHR_dynamic_rendering_local_read")
        && message.contains("not supported by this layer"))
        || (message.contains("(1000232000)") && message.contains("unknown VkStructureType"))
}

/// A real, narrowly-scoped exception to `vulkan_debug_callback`'s own
/// "abort on any ERROR-severity message" policy, covering exactly the one
/// `ERROR` `is_local_read_rejected_by_layer` also reacts to. This ERROR is
/// unavoidable: the validation layer only ever reveals it doesn't
/// recognize `VK_KHR_dynamic_rendering_local_read` by complaining once,
/// during the same `vkCreateDevice` call that speculatively requests it
/// -- so this one call's own false-positive must survive long enough for
/// `new` to read `DebugCallbackState::local_read_rejected` and stop using
/// the extension afterward. No other exception is needed: once the
/// extension is never actually exercised again, none of the further
/// downstream VUIDs this same root cause used to cascade into (REVIEW.md
/// finding #208's earlier iterations) can fire at all.
#[cfg(debug_assertions)]
fn is_known_false_positive(message: &str) -> bool {
    message.contains("(1000232000)") && message.contains("unknown VkStructureType")
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

        #[allow(
            unused_mut,
            reason = "only mutated inside the #[cfg(debug_assertions)] validation-layer setup \
                      just below -- a release build never pushes to this Vec, a real, \
                      cfg-dependent asymmetry only visible when actually built in --release \
                      (debug builds' own clippy/build runs never see this warning)"
        )]
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

        // Boxed (not stack-local) since Vulkan keeps this raw pointer alive
        // for the whole messenger lifetime below, not just for this
        // function call -- the `Box` itself is threaded out and stored on
        // `Self.debug_callback_state` so its heap address stays valid for
        // exactly as long as `debug_utils`'s messenger does.
        #[cfg(debug_assertions)]
        let debug_callback_state = Box::new(DebugCallbackState {
            local_read_rejected: AtomicBool::new(false),
        });
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
                    .pfn_user_callback(Some(vulkan_debug_callback))
                    .user_data(
                        (debug_callback_state.as_ref() as *const DebugCallbackState)
                            .cast_mut()
                            .cast(),
                    );
                // SAFETY: `debug_utils_loader` was just created from this
                // valid `instance`/`entry`; `messenger_info` (and the
                // `'static` callback function it references) is a local
                // borrowed only for the duration of this call, which is all
                // `create_debug_utils_messenger` requires. Its `user_data`
                // points into `debug_callback_state`, which outlives this
                // call (stored on `Self` right alongside the messenger
                // this creates).
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

        // IMPLEMENTATION.md Step 3.3.3: at least one of these two combined
        // depth/stencil formats is guaranteed by the Vulkan spec to support
        // `DEPTH_STENCIL_ATTACHMENT_OPTIMAL` tiling -- queried here once,
        // not assumed, since a pure `VK_FORMAT_S8_UINT`-only stencil format
        // is NOT guaranteed to be supported at all.
        let stencil_format = [
            vk::Format::D24_UNORM_S8_UINT,
            vk::Format::D32_SFLOAT_S8_UINT,
        ]
        .into_iter()
        .find(|&format| {
            // SAFETY: `physical_device` was chosen above from this
            // instance's own enumeration and is still valid; `format`
            // is a plain enum value with no further preconditions.
            let props =
                unsafe { instance.get_physical_device_format_properties(physical_device, format) };
            props
                .optimal_tiling_features
                .contains(vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT)
        })
        .ok_or(EngineError::DeviceLost)?;

        let queue_priorities = [1.0f32];
        let queue_create_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family_index)
            .queue_priorities(&queue_priorities);
        let queue_create_infos = [queue_create_info];

        // Phase 10 Step 10.2.3: a real, disclosed capability query, not
        // an assumption -- `VK_EXT_blend_operation_advanced` (this
        // project's own original plan for non-`Normal` `BlendMode`
        // rendering) is not implemented by RADV, the driver on this
        // project's own real dev GPU, confirmed both via `vulkaninfo`
        // and Mesa's own release notes (REVIEW.md has the full account).
        // `VK_KHR_dynamic_rendering_local_read` -- confirmed present on
        // this same real GPU -- is the real, portable alternative:
        // reading the destination pixel a preceding draw already wrote,
        // via a real input attachment, computing the blend formula in
        // the shader itself. Queried here, once, exactly like
        // `debug_validation_available`'s own identical pattern for an
        // optional instance layer/extension.
        //
        // SAFETY: `physical_device` was chosen above from this
        // instance's own enumeration and is still valid; `None` queries
        // every extension the base driver implementation exposes
        // (rather than a specific layer's own).
        #[allow(
            unused_mut,
            reason = "only downgraded below inside the #[cfg(debug_assertions)] block right \
                      after `create_device` -- a release build never mutates this, a real, \
                      cfg-dependent asymmetry matching `enabled_layers`'s own identical pattern \
                      above"
        )]
        let mut local_read_supported =
            unsafe { instance.enumerate_device_extension_properties(physical_device) }
                .unwrap_or_default()
                .iter()
                .any(|ext| {
                    // SAFETY: `ext.extension_name` is a fixed-size buffer the
                    // Vulkan implementation NUL-terminates.
                    (unsafe { CStr::from_ptr(ext.extension_name.as_ptr()) })
                        == ash::khr::dynamic_rendering_local_read::NAME
                });

        let mut device_extension_names: Vec<*const c_char> = REQUIRED_DEVICE_EXTENSIONS
            .iter()
            .map(|e| e.as_ptr())
            .collect();
        if local_read_supported {
            device_extension_names.push(ash::khr::dynamic_rendering_local_read::NAME.as_ptr());
        }

        let mut dynamic_rendering_feature =
            vk::PhysicalDeviceDynamicRenderingFeatures::default().dynamic_rendering(true);

        // IMPLEMENTATION.md Step 3.3.3: core in Vulkan 1.2 (this device
        // already targets 1.2), needed so a stencil-only image view/aspect
        // mask/layout (`STENCIL_ATTACHMENT_OPTIMAL`) is valid on a combined
        // depth+stencil image -- without this feature enabled, Vulkan
        // requires every barrier/attachment on such an image to reference
        // BOTH aspects together, confirmed by a real validation error
        // during this step's own development (`VUID-VkImageMemoryBarrier-
        // image-03320`), not assumed from reading the spec.
        let mut separate_depth_stencil_layouts_feature =
            vk::PhysicalDeviceSeparateDepthStencilLayoutsFeatures::default()
                .separate_depth_stencil_layouts(true);

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

        // Phase 10 Step 10.2.3: only chained in when `local_read_supported`
        // is true above -- requesting a feature struct for an extension the
        // device didn't advertise is invalid per the Vulkan spec, so this
        // must stay conditional rather than always-on like the other
        // features above (all of which are hard requirements).
        let mut local_read_feature =
            vk::PhysicalDeviceDynamicRenderingLocalReadFeaturesKHR::default()
                .dynamic_rendering_local_read(true);

        let mut device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&device_extension_names)
            .push_next(&mut dynamic_rendering_feature)
            .push_next(&mut descriptor_indexing_feature)
            .push_next(&mut separate_depth_stencil_layouts_feature);
        if local_read_supported {
            device_create_info = device_create_info.push_next(&mut local_read_feature);
        }

        // SAFETY: `physical_device` was chosen above from this instance's
        // own enumeration, and `device_create_info`'s borrowed
        // `queue_create_infos`/`device_extension_names`/
        // `dynamic_rendering_feature` are all locals that outlive this
        // call.
        let device = unsafe { instance.create_device(physical_device, &device_create_info, None) }
            .map_err(|_| EngineError::DeviceLost)?;

        // `vulkan_debug_callback` -- via `debug_callback_state`, only wired
        // up when `debug_utils` actually created a messenger above -- would
        // already have recorded a rejection synchronously during the
        // `create_device` call just above, if this validation layer
        // doesn't recognize `VK_KHR_dynamic_rendering_local_read`. Checked
        // here, before anything below reads `local_read_supported`, so the
        // rest of `new` never actually exercises an extension the device
        // was created with but the active validation layer can't validate
        // -- see `is_local_read_rejected_by_layer`'s doc comment for why
        // this beats chasing each further downstream VUID that extension's
        // real use would otherwise cascade into under this layer.
        #[cfg(debug_assertions)]
        if debug_callback_state
            .local_read_rejected
            .load(Ordering::Relaxed)
        {
            local_read_supported = false;
        }

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

        // Phase 10 Step 10.2: the shape-style storage buffer, built here
        // (rather than inside the `Self { .. }` literal below, like
        // `transient_pool`/`deferred_release`) so its raw buffer handle is
        // available for the binding-1 descriptor write further down, once
        // `bindless_descriptor_set` exists.
        let shape_style_buffer = VulkanRingBuffer::new(
            &device,
            physical_device,
            &instance,
            frame_sync.clone(),
            SHAPE_STYLE_BUFFER_CAPACITY,
        )?;

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

        // Binding 2 (the HIGHEST-numbered binding -- required, per spec,
        // since `VARIABLE_DESCRIPTOR_COUNT` may only be set on the binding
        // with the highest binding number in the layout): the unbounded
        // `texture2D textures[]` array IMPLEMENTATION.md Step 2.1 describes
        // -- `SAMPLED_IMAGE`, not `COMBINED_IMAGE_SAMPLER`, per that same
        // wording (a separate, single shared sampler at binding 0 instead).
        // Binding 0's `immutable_samplers` bakes `bindless_sampler` into
        // the layout itself, so that binding is never written via
        // `vkUpdateDescriptorSets`. Binding 1 (Phase 10 Step 10.2) is the
        // new `shape_style_buffer` storage buffer -- deliberately placed
        // BEFORE the texture array (not appended after it) specifically so
        // the array keeps the highest binding number the spec requires for
        // `VARIABLE_DESCRIPTOR_COUNT`; every shader referencing the
        // texture array was renumbered from `binding = 1` to `binding = 2`
        // to match (`bindless_textured.frag`, `msdf.frag`,
        // `kawase_downsample.frag`, `kawase_upsample.frag`).
        let bindless_layout_bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT)
                .immutable_samplers(std::slice::from_ref(&bindless_sampler)),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(2)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(bindless_capacity)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        // Binding 2 (the texture array) needs all four flags:
        // `UPDATE_AFTER_BIND` (textures are registered after the set is
        // bound elsewhere in a frame's lifetime), `PARTIALLY_BOUND` (most
        // of a 4,096-slot array is unused at any given moment),
        // `VARIABLE_DESCRIPTOR_COUNT` (the array's real size is
        // `bindless_capacity`, decided at runtime, not
        // `BINDLESS_TEXTURE_CAPACITY_TARGET` unconditionally), and
        // `UPDATE_UNUSED_WHILE_PENDING` (registering a new texture must not
        // require waiting for in-flight draws that don't reference it).
        // Binding 0's immutable sampler and binding 1's storage buffer
        // (written exactly once, right after this set is allocated below,
        // and never again -- its CONTENTS change every frame via the same
        // persistent `mapped_ptr` write every ring buffer already uses,
        // not via a second `vkUpdateDescriptorSets`) need none of them.
        let bindless_binding_flags = [
            vk::DescriptorBindingFlags::empty(),
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
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::STORAGE_BUFFER)
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

        // Phase 10 Step 10.2: binding 1's one-and-only descriptor write --
        // `shape_style_buffer`'s underlying `VkBuffer` never changes for
        // the life of this `VulkanDevice`, so the descriptor is pointed at
        // it exactly once here; every later "update" is a plain memory
        // write through `shape_style_buffer`'s own persistent `mapped_ptr`
        // (`RhiDynamicRingBuffer::write`), not a second
        // `vkUpdateDescriptorSets` call.
        let shape_style_buffer_info = vk::DescriptorBufferInfo::default()
            .buffer(shape_style_buffer.buffer)
            .offset(0)
            .range(vk::WHOLE_SIZE);
        // SAFETY: `device` is valid; `bindless_descriptor_set` was just
        // allocated above from this same device; `shape_style_buffer_info`
        // references `shape_style_buffer.buffer`, created earlier in this
        // same function and not yet moved anywhere -- both outlive this
        // call.
        unsafe {
            device.update_descriptor_sets(
                &[vk::WriteDescriptorSet::default()
                    .dst_set(bindless_descriptor_set)
                    .dst_binding(1)
                    .dst_array_element(0)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .buffer_info(std::slice::from_ref(&shape_style_buffer_info))],
                &[],
            );
        }

        // Phase 10 Step 10.2.3: a SEPARATE descriptor set (set 1), never
        // folded into the bindless set 0 above -- extending that set
        // would require renumbering every other shader's bindings 1/2,
        // since `VARIABLE_DESCRIPTOR_COUNT` must stay on the
        // highest-numbered binding in a layout. Built once here (even
        // though `local_read_supported` is checked first) purely so its
        // handles exist as plain fields rather than needing a `Result`
        // returned from inside the `local_read_supported`-guarded branch
        // below -- nothing here is created unless that flag is `true`.
        let blend_read = if local_read_supported {
            let input_attachment_bindings = [vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::INPUT_ATTACHMENT)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT)];
            // SAFETY: `device` is valid, and `input_attachment_bindings`
            // is a local that outlives this call.
            let descriptor_set_layout = unsafe {
                device.create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default()
                        .bindings(&input_attachment_bindings),
                    None,
                )
            }
            .map_err(|_| EngineError::DeviceLost)?;

            let pool_sizes = [vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::INPUT_ATTACHMENT)
                .descriptor_count(1)];
            // SAFETY: `device` is valid, and `pool_sizes` is a local
            // that outlives this call.
            let descriptor_pool = unsafe {
                device.create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default()
                        .max_sets(1)
                        .pool_sizes(&pool_sizes),
                    None,
                )
            }
            .map_err(|_| EngineError::DeviceLost)?;

            let set_layouts = [descriptor_set_layout];
            // SAFETY: `device` is valid; `descriptor_pool` and
            // `descriptor_set_layout` were both just created above on
            // this same device, and `set_layouts` is a local that
            // outlives this call. This descriptor is left unwritten
            // here -- `begin_frame` writes it fresh every frame, before
            // any draw could bind it, once a real color attachment view
            // exists to point it at.
            let descriptor_set = unsafe {
                device.allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default()
                        .descriptor_pool(descriptor_pool)
                        .set_layouts(&set_layouts),
                )
            }
            .map_err(|_| EngineError::DeviceLost)?[0];

            Some(BlendReadResources {
                descriptor_set_layout,
                descriptor_pool,
                descriptor_set,
            })
        } else {
            None
        };

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
                stencil_format,
                device,
                queue_family_index,
                graphics_queue,
                command_pool,
                command_buffer,
                upload_command_pool: Mutex::new(upload_command_pool),
                dynamic_rendering,
                frame_sync,
                transient_pool,
                blur_resources: Arc::new(Mutex::new(None)),
                deferred_release,
                gc_running,
                gc_thread: Some(gc_thread),
                bindless_sampler,
                bindless_descriptor_pool,
                bindless_descriptor_set_layout,
                bindless_descriptor_set,
                bindless_registry: Arc::new(Mutex::new(BindlessRegistry::new(bindless_capacity))),
                bindless_capacity,
                shape_style_buffer: Some(shape_style_buffer),
                #[cfg(debug_assertions)]
                debug_utils,
                #[cfg(debug_assertions)]
                debug_callback_state,
                local_read_supported,
                blend_read,
            },
            surface_loader,
            surface,
        ))
    }

    /// The real `VkQueue` handle for `queue_family_index`.
    #[must_use]
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

    pub(crate) fn create_surface_raw(
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

    /// Builds a `Normal`-blend-only graphics pipeline from `vertex_spv`/
    /// `fragment_spv` (SPIR-V bytecode), bound to `color_format`, against
    /// this device's own single universal pipeline layout (the bindless
    /// descriptor set plus the `texture_index` push constant). For a
    /// non-`Normal`-blend-capable pipeline, see
    /// [`VulkanDevice::create_blend_mode_pipeline`].
    ///
    /// # Errors
    /// Returns [`EngineError::PipelineCreationFailed`] if shader module
    /// or pipeline creation fails.
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

        let bindings = [ui_vertex_binding()];
        let attribute_descriptions = ui_vertex_attributes();
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

        let layout = self.create_universal_pipeline_layout()?;

        let color_formats = [color_format];
        // IMPLEMENTATION.md Step 3.3.3: `begin_frame` now always attaches
        // a stencil buffer (every swapchain owns one), so every pipeline's
        // declared attachment formats must stay compatible with that --
        // matching the same "declared everywhere, unused by pipelines
        // that don't reference it" precedent already used for the
        // bindless descriptor set and push-constant range. This pipeline
        // still has `stencil_test_enable(false)` (via `depth_stencil`
        // above), so declaring the format changes nothing about its
        // actual behavior.
        let mut rendering_info = vk::PipelineRenderingCreateInfo::default()
            .color_attachment_formats(&color_formats)
            .stencil_attachment_format(self.stencil_format);

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

    /// IMPLEMENTATION.md Step 2.1's "ONE universal pipeline layout every
    /// pipeline gets" (the bindless descriptor set plus the 4-byte
    /// `texture_index` push constant), factored out of `create_pipeline`
    /// so more than one pipeline-creation function could share it without
    /// duplicating this construction. (Originally factored out for a
    /// second caller, `create_stencil_and_cover_pipelines` from Step
    /// 3.3.3's stencil-and-cover fill technique -- that function and the
    /// technique it built were retired 2026-09-09, REVIEW.md finding
    /// #165, in favor of `lyon`'s real sweep-line fill tessellator; this
    /// helper stayed factored out since `create_blend_mode_pipeline`,
    /// Step 10.2.3, is now its second real caller.) Each pipeline still
    /// gets its OWN `VkPipelineLayout` object (not a shared one), matching
    /// `create_pipeline`'s existing one-layout-per-`VulkanPipelineState`
    /// ownership/destruction model.
    pub(crate) fn create_universal_pipeline_layout(
        &self,
    ) -> Result<vk::PipelineLayout, EngineError> {
        let bindless_set_layouts = [self.bindless_descriptor_set_layout];
        // SAFETY: `self.device` is the valid logical device owned by this
        // `VulkanDevice`; `bindless_set_layouts` (referencing this
        // device's own `bindless_descriptor_set_layout`, created in
        // `new`) and `push_constant_ranges`'s slice are local temporaries
        // that outlive this call.
        unsafe {
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
        .map_err(|_| EngineError::PipelineCreationFailed)
    }

    /// Phase 10 Step 10.2.3's own two-set variant of
    /// `create_universal_pipeline_layout`, for `PipelineKind::
    /// FlatColorBlend` alone: the same bindless set 0 plus the new
    /// `blend_read`'s input-attachment set 1, so a blend-mode fragment
    /// shader can `subpassLoad` the destination pixel a preceding draw
    /// already wrote. Callers must already know `local_read_blend_
    /// supported()` is `true` -- see `create_blend_mode_pipeline`.
    pub(crate) fn create_blend_pipeline_layout(&self) -> Result<vk::PipelineLayout, EngineError> {
        let blend_read = self
            .blend_read
            .as_ref()
            .expect("create_blend_pipeline_layout requires framebuffer_fetch_blend_supported()");
        let set_layouts = [
            self.bindless_descriptor_set_layout,
            blend_read.descriptor_set_layout,
        ];
        // SAFETY: `self.device` is valid; `set_layouts` references this
        // device's own `bindless_descriptor_set_layout` and `blend_read
        // .descriptor_set_layout`, both created in `new` and still
        // alive, and `set_layouts`/`push_constant_ranges`'s slice are
        // local temporaries that outlive this call.
        unsafe {
            self.device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(&set_layouts)
                    .push_constant_ranges(&[
                        vk::PushConstantRange::default()
                            .stage_flags(
                                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            )
                            .offset(0)
                            .size(12), // vec2 screen_size, uint blend_mode
                    ]),
                None,
            )
        }
        .map_err(|_| EngineError::PipelineCreationFailed)
    }

    /// Phase 10 Step 10.2.3's dedicated pipeline-creation function for
    /// `PipelineKind::FlatColorBlend` -- a near-duplicate of
    /// `create_pipeline` (matching `create_blur_pipeline`'s own existing
    /// precedent of a fully separate function rather than a shared
    /// parameterized helper for a differently-blended pipeline), with
    /// exactly two real differences: `blend_enable(false)` (the
    /// fragment shader computes the fully-composited color itself via
    /// `subpassLoad` and writes it directly, rather than letting fixed-
    /// function hardware blending combine it with the destination), and
    /// `create_blend_pipeline_layout` instead of `create_universal_
    /// pipeline_layout` for the extra input-attachment set.
    ///
    /// # Panics
    /// Panics if `framebuffer_fetch_blend_supported()` is `false` -- callers
    /// must check that capability before ever calling this.
    pub fn create_blend_mode_pipeline(
        &self,
        vertex_spv: &[u8],
        fragment_spv: &[u8],
        color_format: vk::Format,
    ) -> Result<VulkanPipelineState, EngineError> {
        assert!(
            self.local_read_supported,
            "create_blend_mode_pipeline requires framebuffer_fetch_blend_supported()"
        );

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

        let bindings = [ui_vertex_binding()];
        let attribute_descriptions = ui_vertex_attributes();
        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&bindings)
            .vertex_attribute_descriptions(&attribute_descriptions);

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

        // `blend_enable(false)`: unlike every other pipeline in this
        // codebase, the shader itself computes the fully-composited
        // final color (via `subpassLoad` plus the blend formula) and
        // writes it directly -- fixed-function hardware blending must
        // stay off, or the hardware would blend this already-blended
        // result AGAIN against the destination.
        let color_blend_attachment = vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(false)
            .color_write_mask(vk::ColorComponentFlags::RGBA);
        let attachments = [color_blend_attachment];
        let color_blend =
            vk::PipelineColorBlendStateCreateInfo::default().attachments(&attachments);

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        let layout = self.create_blend_pipeline_layout()?;

        let color_formats = [color_format];
        let mut rendering_info = vk::PipelineRenderingCreateInfo::default()
            .color_attachment_formats(&color_formats)
            .stencil_attachment_format(self.stencil_format);

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

    pub(crate) fn create_shader_module(&self, spv: &[u8]) -> Result<vk::ShaderModule, EngineError> {
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
    pub(crate) fn grow_pending_transient_targets(&self) {
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
    pub(crate) fn drain_deferred_release_queue(&self) {
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

/// `UiVertex`'s per-vertex binding, shared by every pipeline-creation
/// function in this crate so each builds the exact same vertex input
/// layout from one definition.
fn ui_vertex_binding() -> vk::VertexInputBindingDescription {
    vk::VertexInputBindingDescription::default()
        .binding(0)
        .stride(std::mem::size_of::<UiVertex>() as u32)
        .input_rate(vk::VertexInputRate::VERTEX)
}

/// `UiVertex`'s four attributes (position/uv/color/params), shared by
/// every pipeline-creation function in this crate. See `create_pipeline`'s
/// original inline version (IMPLEMENTATION.md Step 3.2) for why `params`
/// (location 3) is declared on every pipeline uniformly even though most
/// shaders don't read it.
fn ui_vertex_attributes() -> [vk::VertexInputAttributeDescription; 4] {
    [
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
    ]
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
            // Phase 2 Step 2.3 Code Review finding #79: a silently
            // discarded `Err` here would mean a GC-thread panic (which
            // also poisons `transient_pool`'s shared mutex, per that
            // finding) leaves no trace anywhere, including at final
            // teardown -- the one place guaranteed to still be reachable
            // even if every main-thread pool call has been panicking on
            // the poisoned lock since. `eprintln!`, not a panic: `Drop`
            // is already mid-teardown and must still finish.
            if let Err(payload) = handle.join() {
                // `Box<dyn Any + Send>` isn't `Debug`; a panic payload is
                // almost always a `&str` (a string-literal panic message)
                // or `String` (a formatted one) -- downcast to whichever
                // matches rather than printing nothing useful.
                let message = payload
                    .downcast_ref::<&str>()
                    .copied()
                    .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                    .unwrap_or("<non-string panic payload>");
                eprintln!("tre-rhi-vulkan: GC thread panicked: {message}");
            }
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
        // Phase 2 Step 2.3 Code Review finding #78: the exact same hazard
        // as the transient-pool clear immediately above, for a field that
        // clear predates -- `deferred_release` holds real `VulkanTexture`s
        // awaiting their 3-frame grace period, and Rust would otherwise
        // drop them (running their own real `vkDestroy*` calls) AFTER
        // `destroy_device` below already ran. The grace period itself is
        // moot here: `device_wait_idle()` above already guarantees nothing
        // is submitting or reading from these images anymore, so there is
        // no reason to wait it out before clearing.
        if let Ok(mut queue) = self.deferred_release.lock() {
            queue.clear();
        }
        // Phase 10 Step 10.2: `shape_style_buffer` is a real
        // `VulkanRingBuffer`, not a container of Vulkan-owning items like
        // `transient_pool`/`deferred_release` above -- its own `Drop`
        // impl makes real `unmap_memory`/`destroy_buffer`/`free_memory`
        // calls (through its own cloned `ash::Device`, functionally as
        // valid as `self.device` until `destroy_device` below actually
        // runs). Rust only drops a struct's other fields AFTER this
        // function's body returns, which would run that Drop AFTER
        // `destroy_device` -- a real use-after-free -- unless taken and
        // dropped explicitly here first, same reasoning as `gc_thread`'s
        // early `.take()`-and-`.join()` above.
        drop(self.shape_style_buffer.take());
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
            // IMPLEMENTATION.md Step 7.2.2: `apply_layer_blur`'s own
            // lazily-created resources, if this process ever actually
            // called it -- `descriptor_pool` frees `descriptor_set` too
            // (destroying a pool frees every set allocated from it), the
            // same reasoning `bindless_descriptor_pool` below relies on.
            if let Ok(mut blur) = self.blur_resources.lock() {
                if let Some(blur) = blur.take() {
                    self.device.destroy_pipeline(blur.downsample_pipeline, None);
                    self.device.destroy_pipeline(blur.upsample_pipeline, None);
                    self.device
                        .destroy_pipeline_layout(blur.pipeline_layout, None);
                    self.device.destroy_sampler(blur.sampler, None);
                    self.device
                        .destroy_descriptor_pool(blur.descriptor_pool, None);
                    self.device
                        .destroy_descriptor_set_layout(blur.descriptor_set_layout, None);
                    self.device
                        .destroy_buffer(blur.unit_quad_vertex_buffer, None);
                    self.device.free_memory(blur.unit_quad_vertex_memory, None);
                    self.device
                        .destroy_buffer(blur.unit_quad_index_buffer, None);
                    self.device.free_memory(blur.unit_quad_index_memory, None);
                }
            }
            self.device
                .destroy_descriptor_pool(self.bindless_descriptor_pool, None);
            self.device
                .destroy_descriptor_set_layout(self.bindless_descriptor_set_layout, None);
            self.device.destroy_sampler(self.bindless_sampler, None);
            // Phase 10 Step 10.2.3: destroying the pool frees
            // `blend_read.descriptor_set` too, same reasoning as
            // `bindless_descriptor_pool` above.
            if let Some(blend_read) = &self.blend_read {
                self.device
                    .destroy_descriptor_pool(blend_read.descriptor_pool, None);
                self.device
                    .destroy_descriptor_set_layout(blend_read.descriptor_set_layout, None);
            }
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

impl VulkanDevice {
    /// Allocates a real bindless slot and points it at `view` -- the
    /// shared core of `VulkanTexture::from_pixels`'s own registration
    /// step and the new `RhiDevice::register_bindless` (IMPLEMENTATION.md
    /// Phase 6 Step 6.4.1), factored out so a texture that's already
    /// GPU-resident (rendered into directly, not freshly uploaded) can
    /// register its own existing view without duplicating this logic.
    ///
    /// # Errors
    /// Returns [`EngineError::BindlessArrayExhausted`] if the bindless
    /// texture array has no free slots left.
    pub(crate) fn allocate_bindless_slot(&self, view: vk::ImageView) -> Result<u32, EngineError> {
        let bindless_index = self
            .bindless_registry
            .lock()
            .expect("bindless registry poisoned")
            .allocate()
            .ok_or(EngineError::BindlessArrayExhausted)?;

        let image_info = vk::DescriptorImageInfo::default()
            .image_view(view)
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        let write = vk::WriteDescriptorSet::default()
            .dst_set(self.bindless_descriptor_set)
            .dst_binding(2)
            .dst_array_element(bindless_index)
            .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
            .image_info(std::slice::from_ref(&image_info));
        // SAFETY: `self.device` is valid; `self.bindless_descriptor_set`
        // was allocated in `VulkanDevice::new` from a layout whose binding
        // 2 (the texture array -- renumbered from binding 1 at Phase 10
        // Step 10.2, when the new shape-style storage buffer took binding
        // 1) has `UPDATE_AFTER_BIND`, so writing to it here (potentially
        // while other draws using this same set are in flight, though
        // this step's scope keeps submission fully synchronous anyway) is
        // explicitly permitted; `bindless_index` was just allocated above
        // so it is `< bindless_capacity`, and `view` is the caller's own
        // responsibility to have created on this same device.
        unsafe {
            self.device.update_descriptor_sets(&[write], &[]);
        }
        Ok(bindless_index)
    }

    pub(crate) fn begin_frame_impl(
        &self,
        swapchain: &dyn RhiSwapchain,
        timeout_ns: u64,
        logical_size: Option<(u32, u32)>,
        viewport_crop: Option<(u32, u32)>,
    ) -> Result<(Box<dyn RhiCommandBuffer>, AcquiredImage), EngineError> {
        // SAFETY: `self.device` is valid and `self.frame_sync.fence` was
        // created signaled in `new`; under the single-frame-in-flight
        // model it is only ever waited on here and reset (below, after a
        // successful acquire) once per real frame.
        unsafe {
            self.device
                .wait_for_fences(&[self.frame_sync.fence], true, u64::MAX)
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

        let image = swapchain.acquire_next_image_with_timeout(timeout_ns)?;

        // REVIEW.md finding #235, Option 3: the fence is reset only now,
        // after a successful acquire, not immediately after the wait
        // above. If `acquire_next_image_with_timeout` returns
        // `Err(EngineError::AcquireTimedOut)` (or any other error)
        // before reaching here, the fence is left exactly as the wait
        // above found it (signaled), so the *next* call's own
        // `wait_for_fences` above returns immediately instead of
        // blocking forever waiting for a submit that this skipped frame
        // never made.
        unsafe {
            self.device
                .reset_fences(&[self.frame_sync.fence])
                .map_err(|_| EngineError::DeviceLost)?;
        }

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
        // REVIEW.md finding #235: `ndc_width`/`ndc_height` are what
        // `VulkanCommandBuffer::width`/`height` -- and therefore
        // `draw_indexed`'s `screen_size` push constant -- get set to.
        // `render_width`/`render_height` are the GPU viewport/scissor/
        // render area extent. `swapchain_width`/`swapchain_height`
        // (assigned into the returned `VulkanCommandBuffer` further below)
        // always stay the swapchain's own real, physical extent
        // regardless of either mode -- unrelated to this choice, they
        // exist only so a render-to-texture layer can restore the main
        // swapchain's own true dimensions after popping.
        //
        // `logical_size`/`viewport_crop` are only ever `Some` while a
        // caller is deliberately holding the swapchain at a coarser size
        // than the real window during an active resize drag -- and are
        // mutually exclusive strategies for what to do with that mismatch
        // (see `RhiDevice::begin_frame_with_viewport_crop`'s own doc
        // comment): `logical_size`'s `Stretched` mode below renders across
        // the FULL buffer so a compositor-side scale-to-fit cancels the
        // stretch; `viewport_crop`'s `Cropped` mode instead renders 1:1
        // into just that sub-rectangle, for a caller pairing it with a
        // real compositor-side viewport source crop of the identical
        // size -- zero resample, instead of one. `viewport_crop` is
        // checked first and, if present, takes priority (a caller should
        // never pass both `Some` for the same frame; this ordering just
        // makes the precedence explicit rather than leaving it
        // unspecified).
        let (render_width, render_height, ndc_width, ndc_height) =
            if let Some((crop_width, crop_height)) = viewport_crop {
                // Clamped defensively to the buffer's own real extent --
                // a caller's `real_size` and this swapchain's own extent
                // are expected to already agree (the coarse-bucket resize
                // always rounds up to at least cover it), but an
                // out-of-bounds render area/viewport is a Vulkan validation
                // error, not just a visual glitch, so this is cheap
                // insurance against a transient one-frame race between the
                // two, not a mechanism this design otherwise relies on.
                let crop_width = crop_width.min(width);
                let crop_height = crop_height.min(height);
                (crop_width, crop_height, crop_width, crop_height)
            } else {
                let (logical_width, logical_height) = logical_size.unwrap_or((width, height));
                (width, height, logical_width, logical_height)
            };

        // Phase 10 Step 10.2.3: when this device supports it, the
        // swapchain's color attachment lives in `RENDERING_LOCAL_READ_KHR`
        // for the whole frame instead of the ordinary
        // `COLOR_ATTACHMENT_OPTIMAL` -- a layout usable for a color
        // attachment AND an input attachment simultaneously, so a later
        // `PipelineKind::FlatColorBlend` draw's `subpassLoad` can read
        // back whatever an earlier draw in this same frame already wrote,
        // with no extra layout transitions between ordinary and
        // blend-mode draws. Every ordinary (non-blend) draw this frame
        // still writes through it exactly as before -- this layout has no
        // effect on a shader that never reads the new input attachment.
        // Out of scope for this pass: a layer's own render-to-texture
        // target (`begin_render_to_texture`) always stays in
        // `COLOR_ATTACHMENT_OPTIMAL` and never gets its own input-
        // attachment descriptor written -- `PipelineKind::FlatColorBlend`
        // is only correct against the swapchain target set up here, not
        // while a `PushLayer` is active (see `insert_framebuffer_fetch_barrier`'s
        // own doc comment).
        //
        // Also requires `swapchain.supports_framebuffer_fetch()`
        // -- a real windowed swapchain's images might not support
        // `INPUT_ATTACHMENT` usage even when the device extension itself
        // is present (see that method's own doc comment); every real GPU
        // demo in this codebase uses `HeadlessSwapchain`, which always
        // returns `true` here, so this can only ever fail closed for a
        // windowed surface, never change behavior for an existing demo.
        let local_read_active = self.local_read_supported && swapchain.supports_framebuffer_fetch();
        let color_attachment_layout = if local_read_active {
            vk::ImageLayout::RENDERING_LOCAL_READ_KHR
        } else {
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL
        };

        // Undefined -> `color_attachment_layout`: dynamic rendering has no
        // render pass to do this transition implicitly.
        let barrier = vk::ImageMemoryBarrier::default()
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(color_attachment_layout)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .image(target_image)
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .level_count(1)
                    .layer_count(1),
            );
        // IMPLEMENTATION.md Step 3.3.3: this swapchain's own stencil image
        // gets the exact same "always transition from UNDEFINED, every
        // frame" treatment as the color image above -- valid because it's
        // always paired with `LOAD_OP_CLEAR` below, which discards
        // whatever the image's actual prior contents/layout were anyway.
        let stencil_image = vk::Image::from_raw(swapchain.stencil_image_handle());
        let stencil_view = vk::ImageView::from_raw(swapchain.stencil_view_handle());
        let stencil_barrier = vk::ImageMemoryBarrier::default()
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::STENCIL_ATTACHMENT_OPTIMAL)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(
                vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
            )
            .image(stencil_image)
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::STENCIL)
                    .level_count(1)
                    .layer_count(1),
            );
        // SAFETY: `command_buffer` is in the recording state (just begun
        // above); `target_image` is the swapchain image acquired this
        // frame, and `stencil_image` is this same swapchain's own stencil
        // image (created alongside it) -- both whose layouts are being
        // transitioned before any rendering uses them.
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
            self.device.cmd_pipeline_barrier(
                command_buffer,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                    | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[stencil_barrier],
            );
        }

        let color_attachment = vk::RenderingAttachmentInfo::default()
            .image_view(target_view)
            .image_layout(color_attachment_layout)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.05, 0.05, 0.08, 1.0],
                },
            });
        let color_attachments = [color_attachment];
        // Cleared to 0 every frame. Historical note: this once also
        // relied on the stencil-and-cover technique's own cover-pass
        // `pass_op = ZERO` to reset the buffer to a clean 0 between
        // shapes within a frame, so no shape needed its own mid-frame
        // clear -- that technique (and `create_stencil_and_cover_
        // pipelines`, which built its pipelines) was retired 2026-09-09,
        // REVIEW.md finding #165, in favor of `lyon`'s fill tessellator.
        let stencil_attachment = vk::RenderingAttachmentInfo::default()
            .image_view(stencil_view)
            .image_layout(vk::ImageLayout::STENCIL_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .clear_value(vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 0.0,
                    stencil: 0,
                },
            });
        let rendering_info = vk::RenderingInfo::default()
            .render_area(vk::Rect2D {
                offset: vk::Offset2D::default(),
                extent: vk::Extent2D {
                    width: render_width,
                    height: render_height,
                },
            })
            .layer_count(1)
            .color_attachments(&color_attachments)
            .stencil_attachment(&stencil_attachment);

        // SAFETY: `command_buffer` is still recording, and `target_view`
        // (via `color_attachment`/`rendering_info`) is the same acquired
        // image the barrier above just transitioned to
        // `COLOR_ATTACHMENT_OPTIMAL`; `render_width`/`render_height` never
        // exceed the swapchain's own reported extent (clamped above).
        unsafe {
            self.dynamic_rendering
                .cmd_begin_rendering(command_buffer, &rendering_info);
            self.device.cmd_set_viewport(
                command_buffer,
                0,
                &[vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: render_width as f32,
                    height: render_height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            );
            self.device.cmd_set_scissor(
                command_buffer,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D::default(),
                    extent: vk::Extent2D {
                        width: render_width,
                        height: render_height,
                    },
                }],
            );
        }

        // Phase 10 Step 10.2.3: the input-attachment descriptor is
        // rewritten every frame (not just once) to point at THIS frame's
        // real color attachment view -- safe to do unconditionally here,
        // with no extra synchronization, because the fence wait at the
        // very top of `begin_frame` already guarantees the GPU is done
        // with whatever the prior frame bound it to (the single-frame-
        // in-flight model this whole device is built around). Gated on
        // `local_read_active`, not just `self.blend_read.is_some()`, so a
        // windowed swapchain whose surface doesn't support `INPUT_
        // ATTACHMENT` usage never gets a descriptor pointed at an image
        // that wasn't created with that usage flag.
        if local_read_active {
            if let Some(blend_read) = &self.blend_read {
                let image_info = vk::DescriptorImageInfo::default()
                    .image_view(target_view)
                    .image_layout(color_attachment_layout);
                // SAFETY: `self.device` is valid; `blend_read.
                // descriptor_set` was allocated in `new` from a layout
                // with exactly one `INPUT_ATTACHMENT` binding at 0;
                // `image_info` (referencing `target_view`, this frame's
                // own acquired image view) is a local that outlives this
                // call.
                unsafe {
                    self.device.update_descriptor_sets(
                        &[vk::WriteDescriptorSet::default()
                            .dst_set(blend_read.descriptor_set)
                            .dst_binding(0)
                            .dst_array_element(0)
                            .descriptor_type(vk::DescriptorType::INPUT_ATTACHMENT)
                            .image_info(std::slice::from_ref(&image_info))],
                        &[],
                    );
                }
            }
        }

        Ok((
            Box::new(VulkanCommandBuffer {
                device: self.device.clone(),
                command_buffer,
                width: ndc_width,
                height: ndc_height,
                pipeline_layout: None,
                blur_resources: Arc::clone(&self.blur_resources),
                instance: self.instance.clone(),
                physical_device: self.physical_device,
                stencil_format: self.stencil_format,
                bindless_descriptor_set: self.bindless_descriptor_set,
                bindless_capacity: self.bindless_capacity,
                texture_index: BINDLESS_TEXTURE_SENTINEL,
                dynamic_rendering: self.dynamic_rendering.clone(),
                swapchain_color_view: target_view,
                swapchain_stencil_view: stencil_view,
                swapchain_width: width,
                swapchain_height: height,
                swapchain_color_layout: color_attachment_layout,
                blend_input_descriptor_set: if local_read_active {
                    self.blend_read.as_ref().map(|b| b.descriptor_set)
                } else {
                    None
                },
            }),
            image,
        ))
    }
}

impl RhiDevice for VulkanDevice {
    fn create_dynamic_ring_buffer(
        &self,
        capacity: usize,
    ) -> Result<Box<dyn RhiDynamicRingBuffer>, EngineError> {
        Ok(Box::new(VulkanRingBuffer::new(
            &self.device,
            self.physical_device,
            &self.instance,
            self.frame_sync.clone(),
            capacity,
        )?))
    }

    /// Phase 10 Step 10.2: the bindless binding-2 storage buffer
    /// `GpuShapeStyle` records are bump-allocated into (see
    /// `shape_style_buffer`'s own field doc comment). Reuses the exact
    /// same `RhiDynamicRingBuffer::write` bump-allocate contract the
    /// vertex/index ring buffer already implements -- a style record is
    /// written once per styled shape per frame, never re-read after
    /// upload, the same lifecycle.
    ///
    /// # Panics
    /// Never in practice -- `shape_style_buffer` is `Some` from the moment
    /// `VulkanDevice::new` returns until `Drop` takes it, and nothing
    /// outside `Drop` ever calls `.take()`.
    fn shape_style_buffer(&self) -> &dyn RhiDynamicRingBuffer {
        self.shape_style_buffer
            .as_ref()
            .expect("shape_style_buffer is Some for the entire lifetime of a live VulkanDevice")
    }

    fn framebuffer_fetch_blend_supported(&self) -> bool {
        self.local_read_supported
    }

    fn acquire_transient_target(
        &self,
        width: u32,
        height: u32,
        format: TextureFormat,
    ) -> Result<Box<dyn RhiTexture>, EngineError> {
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
            // counts toward the GC trigger. `saturating_sub` (Phase 2
            // Step 2.3 Code Review finding #79): a bare `-=` would panic
            // on underflow in debug builds -- poisoning `transient_pool`'s
            // mutex and cascading into every future caller on this same
            // lock -- or silently wrap to near-`u64::MAX` in release. A
            // future accounting bug here should degrade to a
            // bounded-but-wrong value, not either of those.
            pool.total_free_bytes = pool.total_free_bytes.saturating_sub(texture.size_bytes);
            return Ok(Box::new(texture));
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
            // `saturating_sub`: see the exact-hit path's comment above.
            pool.total_free_bytes = pool.total_free_bytes.saturating_sub(texture.size_bytes);
            return Ok(Box::new(texture));
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
        //
        // Phase 2 Step 2.3 Code Review finding #80: unlike the two hit
        // paths above, this one requests genuinely NEW GPU memory, not a
        // reuse of already-idle bytes -- so it's the one place that needs
        // an admission check against the budget the generational GC
        // (Step 2.3) only ever reclaims *into*, never gates *out of*.
        // Known limitation, stated plainly: this compares against
        // `total_free_bytes` (idle bytes only), not total bytes including
        // whatever's currently checked out, so it catches "many distinct
        // sizes cycling through mostly-idle" but not "many sizes
        // permanently checked out simultaneously" -- a real, if imperfect,
        // gate using the accounting this step already maintains, not a
        // claim of exact enforcement.
        if pool.total_free_bytes >= DYNAMIC_VRAM_BUDGET_BYTES {
            return Err(EngineError::TransientPoolBudgetExceeded);
        }
        drop(pool);
        Ok(Box::new(
            VulkanTexture::new(self, width, height, format)
                .expect("failed to create transient render target"),
        ))
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

    fn register_bindless(&self, texture: &dyn RhiTexture) -> Result<u32, EngineError> {
        // SAFETY: reconstructing a `vk::ImageView` from `texture`'s own
        // opaque `raw_handle()` -- the same opaque-handle pattern this
        // codebase uses everywhere a trait object needs to hand a
        // concrete Vulkan object to backend-specific code, not a
        // downcast. `RhiTexture`'s own contract guarantees this handle is
        // a live view this same device created.
        let view = vk::ImageView::from_raw(texture.raw_handle());
        self.allocate_bindless_slot(view)
    }

    fn deregister_bindless(&self, bindless_index: u32) {
        self.bindless_registry
            .lock()
            .expect("bindless registry poisoned")
            .release(bindless_index);
    }

    fn begin_frame(
        &self,
        swapchain: &dyn RhiSwapchain,
    ) -> Result<(Box<dyn RhiCommandBuffer>, AcquiredImage), EngineError> {
        self.begin_frame_impl(swapchain, u64::MAX, None, None)
    }

    fn begin_frame_with_logical_size(
        &self,
        swapchain: &dyn RhiSwapchain,
        logical_size: (u32, u32),
    ) -> Result<(Box<dyn RhiCommandBuffer>, AcquiredImage), EngineError> {
        self.begin_frame_impl(swapchain, u64::MAX, Some(logical_size), None)
    }

    /// `/review-project` Architecture finding (2026-09-13): replaces the
    /// former separate `begin_frame_with_timeout`/`begin_frame_with_
    /// viewport_crop` trait methods -- see `Self::begin_frame_impl`'s own
    /// doc comment for the real fence-reset-ordering hazard a bounded
    /// acquire must avoid, and `BeginFrameOptions`'s own field docs for
    /// what each option does. `options.timeout_ns.unwrap_or(u64::MAX)`
    /// and `options.crop_size` map directly onto `begin_frame_impl`'s own
    /// already-orthogonal parameters -- this method adds no new
    /// mechanism, only a single, composable entry point onto one that
    /// already existed.
    fn begin_frame_with_options(
        &self,
        swapchain: &dyn RhiSwapchain,
        options: BeginFrameOptions,
    ) -> Result<(Box<dyn RhiCommandBuffer>, AcquiredImage), EngineError> {
        self.begin_frame_impl(
            swapchain,
            options.timeout_ns.unwrap_or(u64::MAX),
            None,
            options.crop_size,
        )
    }

    fn submit_and_present(
        &self,
        cmd_buffer: Box<dyn RhiCommandBuffer>,
        swapchain: &dyn RhiSwapchain,
        image: AcquiredImage,
    ) -> Result<(), EngineError> {
        let raw_cmd = vk::CommandBuffer::from_raw(cmd_buffer.raw_handle());
        let target_image = vk::Image::from_raw(image.target_image_handle);

        // Must match whatever layout `begin_frame` transitioned this same
        // image into -- `RENDERING_LOCAL_READ_KHR` when this device AND
        // this swapchain both support it, `COLOR_ATTACHMENT_OPTIMAL`
        // otherwise (see `begin_frame`'s own `local_read_active`).
        let old_layout = if self.local_read_supported && swapchain.supports_framebuffer_fetch() {
            vk::ImageLayout::RENDERING_LOCAL_READ_KHR
        } else {
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL
        };
        let barrier = vk::ImageMemoryBarrier::default()
            .old_layout(old_layout)
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

    /// Real user-facing custom shader API (Phase 13 Step 13.8, Q13):
    /// compiles `fragment_source` (real GLSL) to SPIR-V at runtime via
    /// `shaderc` -- the same real library `build.rs` already wraps
    /// through its `glslc` CLI invocation for this crate's own
    /// compile-time shaders, kept identical rather than introducing a
    /// second, potentially-divergent compiler -- then pairs it with
    /// [`crate::shape_pipelines::SDF_ROUNDED_RECT_VERT`] via
    /// [`Self::create_pipeline`] (unchanged -- no new pipeline-creation
    /// code path, since `create_pipeline` already accepts any vertex/
    /// fragment SPIR-V pair). Moved onto this trait impl from a
    /// `VulkanDevice`-only inherent method (Architecture review: RHI
    /// trait-object generalization, REVIEW.md finding #216) -- see the
    /// trait method's own doc comment (`tre_engine::RhiDevice::
    /// create_custom_pipeline`) for the full, unchanged v1 scope
    /// contract.
    fn create_custom_pipeline(
        &self,
        fragment_source: &str,
        color_format: TextureFormat,
    ) -> Result<Box<dyn RhiPipelineState>, EngineError> {
        let compiler = shaderc::Compiler::new().ok_or_else(|| {
            EngineError::ShaderCompilationFailed(
                "shaderc::Compiler::new() failed to initialize -- no usable shaderc backend on \
                 this system"
                    .to_string(),
            )
        })?;
        let mut options = shaderc::CompileOptions::new().ok_or_else(|| {
            EngineError::ShaderCompilationFailed(
                "shaderc::CompileOptions::new() failed to initialize".to_string(),
            )
        })?;
        options.set_target_env(
            shaderc::TargetEnv::Vulkan,
            shaderc::EnvVersion::Vulkan1_2 as u32,
        );
        let artifact = compiler
            .compile_into_spirv(
                fragment_source,
                shaderc::ShaderKind::Fragment,
                "custom_shader.frag",
                "main",
                Some(&options),
            )
            .map_err(|e| EngineError::ShaderCompilationFailed(e.to_string()))?;
        let pipeline = self.create_pipeline(
            crate::shape_pipelines::SDF_ROUNDED_RECT_VERT,
            artifact.as_binary_u8(),
            crate::texture_format_to_vk(color_format),
        )?;
        Ok(Box::new(pipeline))
    }
}

// `/review-project` Architecture finding #247 (2026-09-13): the two
// string predicates below decide whether the validation-layer callback
// aborts the process -- they gate whether CI ever catches a real Vulkan
// error -- and were added as REVIEW.md finding #208's own root-cause fix
// without a single test. They are pure functions of the message text,
// so they need no device. Gated on `debug_assertions` exactly as the
// functions themselves are.
#[cfg(all(test, debug_assertions))]
mod tests {
    use super::*;

    const LAYER_REJECTS_EXTENSION: &str =
        "Validation Error: [ VUID-vkCreateDevice-ppEnabledExtensionNames-01387 ] \
         extension VK_KHR_dynamic_rendering_local_read is not supported by this layer";
    const UNKNOWN_STRUCT_TYPE: &str = "Validation Error: vkCreateDevice(): pCreateInfo->pNext \
         contains an unknown VkStructureType (1000232000)";

    #[test]
    fn is_local_read_rejected_by_layer_matches_both_disclosed_forms_of_the_rejection() {
        assert!(is_local_read_rejected_by_layer(LAYER_REJECTS_EXTENSION));
        assert!(is_local_read_rejected_by_layer(UNKNOWN_STRUCT_TYPE));
    }

    #[test]
    fn is_local_read_rejected_by_layer_needs_both_halves_of_each_form() {
        // Half of each pattern alone must not match -- a genuine, unrelated
        // error that merely mentions the extension name or the enum value
        // must still abort.
        assert!(!is_local_read_rejected_by_layer(
            "VK_KHR_dynamic_rendering_local_read enabled successfully"
        ));
        assert!(!is_local_read_rejected_by_layer(
            "feature X is not supported by this layer"
        ));
        assert!(!is_local_read_rejected_by_layer("(1000232000)"));
        assert!(!is_local_read_rejected_by_layer("unknown VkStructureType"));
        assert!(!is_local_read_rejected_by_layer(""));
    }

    #[test]
    fn is_known_false_positive_covers_exactly_the_one_unknown_struct_type_message() {
        assert!(is_known_false_positive(UNKNOWN_STRUCT_TYPE));
        // Narrower than `is_local_read_rejected_by_layer` by design: the
        // "not supported by this layer" form is NOT a survivable
        // false-positive, only the pNext enum complaint is.
        assert!(!is_known_false_positive(LAYER_REJECTS_EXTENSION));
        assert!(!is_known_false_positive(
            "Validation Error: [ VUID-VkImageCreateInfo-extent-00944 ] extent.width must be > 0"
        ));
    }

    #[test]
    fn every_known_false_positive_is_also_a_layer_rejection() {
        // The invariant `vulkan_debug_callback` relies on: the one message
        // allowed to survive is a subset of the ones that switch the
        // extension off, never something that would leave it enabled.
        for message in [
            UNKNOWN_STRUCT_TYPE,
            LAYER_REJECTS_EXTENSION,
            "",
            "unrelated",
        ] {
            if is_known_false_positive(message) {
                assert!(is_local_read_rejected_by_layer(message));
            }
        }
    }
}
