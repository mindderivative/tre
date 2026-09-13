//! `VulkanSwapchain` -- the real, on-screen `RhiSwapchain` implementation
//! (as opposed to [`crate::HeadlessSwapchain`]'s manually-backed
//! off-screen twin). Split out of `lib.rs` as one of its ten separable
//! concerns (Architecture review finding).

use ash::vk;
use ash::vk::Handle;
use tre_engine::{AcquiredImage, EngineError, RhiSwapchain};

use crate::{headless, VulkanDevice};

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
    /// This swapchain's own stencil image (IMPLEMENTATION.md Step 3.3.3),
    /// sized to `width`/`height` -- `VkSwapchainKHR` only provides color
    /// images, so unlike `images` above, this one is allocated and owned
    /// manually, the same way `HeadlessSwapchain`'s always is.
    stencil_image: vk::Image,
    stencil_image_view: vk::ImageView,
    stencil_image_memory: vk::DeviceMemory,
    width: u32,
    height: u32,
    image_available_semaphore: vk::Semaphore,
    /// One per swapchain image, indexed by acquired image index -- see
    /// `AcquiredImage::render_finished_semaphore_handle`'s doc comment in
    /// tre-engine for why this can't be a single shared instance.
    render_finished_semaphores: Vec<vk::Semaphore>,
    device: ash::Device,
    present_queue: vk::Queue,
    /// See `RhiSwapchain::supports_local_read_input_attachment`'s own doc
    /// comment -- queried once above, against this real surface's own
    /// `VkSurfaceCapabilitiesKHR::supportedUsageFlags`.
    supports_local_read_input_attachment: bool,
}

/// Everything [`VulkanSwapchain::new`]/[`VulkanSwapchain::recreate`] both
/// need to build fresh against a live surface -- factored out of `new`'s
/// own original body so `recreate` can run the identical sequence
/// against the *same* `vk::SurfaceKHR` a resize must never touch, rather
/// than duplicating it. See [`VulkanSwapchain::recreate`]'s own doc
/// comment for why that distinction is load-bearing (REVIEW.md finding
/// #232).
struct BuiltSwapchain {
    swapchain_loader: ash::khr::swapchain::Device,
    swapchain: vk::SwapchainKHR,
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    format: vk::Format,
    stencil_image: vk::Image,
    stencil_image_view: vk::ImageView,
    stencil_image_memory: vk::DeviceMemory,
    image_available_semaphore: vk::Semaphore,
    render_finished_semaphores: Vec<vk::Semaphore>,
    supports_local_read_input_attachment: bool,
}

fn build_swapchain(
    device: &VulkanDevice,
    surface_loader: &ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
    width: u32,
    height: u32,
) -> Result<BuiltSwapchain, EngineError> {
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
    // IMPLEMENTATION.md Step 7.1 task 1 / TECHNICAL.md Section 6.1: real,
    // observable capability reporting, not yet an actual format switch --
    // a genuine attempt to select `R16G16B16A16_SFLOAT` here was tried
    // and reverted during this step's own implementation, once running it
    // for real against this project's own dev machine surface showed *why*
    // that's unsafe as a bare format-only match: the surface reports
    // `R16G16B16A16_SFLOAT` paired only with `colorspace ==
    // SRGB_NONLINEAR`, not a genuine wide-gamut/extended-linear
    // colorspace (this crate's own `ash` dependency doesn't even expose
    // `VK_EXT_swapchain_colorspace`'s extended constants, e.g.
    // `EXTENDED_SRGB_LINEAR_EXT`, as named symbols, so distinguishing the
    // two isn't possible here anyway). A float format has no implicit
    // hardware encode-on-store the way an `_SRGB` format does, so
    // presenting Step 7.1's own now-genuinely-linear shader output through
    // an `SRGB_NONLINEAR`-tagged float surface would very likely display
    // too dark -- a real, disclosed correctness risk, not a hypothetical
    // one. The SDR search below stays exactly as before; this log line
    // only reports what the real surface *also* offers, so real future
    // HDR work has a genuine, observed starting fact instead of an
    // assumption.
    if let Some(hdr_candidate) = formats
        .iter()
        .find(|f| f.format == vk::Format::R16G16B16A16_SFLOAT)
    {
        eprintln!(
            "[tre-rhi-vulkan] surface also reports {:?} (colorspace {:?}) -- not selected \
             yet: see this call site's own comment for why",
            hdr_candidate.format, hdr_candidate.color_space
        );
    }
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

    // Phase 10 Step 10.2.3: unlike a manually allocated image (e.g.
    // `HeadlessSwapchain`'s own, which always safely declares
    // `INPUT_ATTACHMENT_BIT`), a presentable surface's own supported
    // usage flags are platform/driver-defined -- `vkCreateSwapchainKHR`
    // requires `imageUsage` be a subset of `capabilities.
    // supportedUsageFlags`, so this is queried for real, not assumed.
    // See `RhiSwapchain::supports_local_read_input_attachment`'s own
    // doc comment for how `VulkanDevice::begin_frame` uses this.
    let supports_local_read_input_attachment = capabilities
        .supported_usage_flags
        .contains(vk::ImageUsageFlags::INPUT_ATTACHMENT);
    let mut image_usage = vk::ImageUsageFlags::COLOR_ATTACHMENT;
    if supports_local_read_input_attachment {
        image_usage |= vk::ImageUsageFlags::INPUT_ATTACHMENT;
    }

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
                .image_usage(image_usage)
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

    // IMPLEMENTATION.md Step 3.3.3: this swapchain's own stencil
    // image, sized to match its own extent (`VkSwapchainKHR` only
    // ever provides color images) -- mirrors `HeadlessSwapchain`'s
    // identical stencil-image creation, reusing the same
    // allocate-and-bind helper.
    //
    // SAFETY: `device.device` is valid, and `ImageCreateInfo` only
    // references locals (`device.stencil_format`, `width`/`height`)
    // that outlive this call.
    let stencil_image = unsafe {
        device.device.create_image(
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
    let stencil_image_memory =
        headless::allocate_and_bind_image(device, &device.device, stencil_image)?;
    // SAFETY: `device.device` is valid, and `stencil_image` was just
    // created and bound to memory above.
    let stencil_image_view = unsafe {
        device.device.create_image_view(
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

    Ok(BuiltSwapchain {
        swapchain_loader,
        swapchain,
        images,
        image_views,
        format: surface_format.format,
        stencil_image,
        stencil_image_view,
        stencil_image_memory,
        image_available_semaphore,
        render_finished_semaphores,
        supports_local_read_input_attachment,
    })
}

impl VulkanSwapchain {
    /// Creates a real `VkSwapchainKHR` and its own dedicated stencil
    /// image against `surface` (already created against `device`, e.g.
    /// via `VulkanDevice::new`'s own probe surface or a later
    /// `VulkanDevice::create_surface` call), sized `width x height`.
    ///
    /// # Errors
    /// Returns [`EngineError::DeviceLost`] if any step of swapchain,
    /// image view, semaphore, or stencil-image creation fails.
    pub fn new(
        device: &VulkanDevice,
        surface_loader: ash::khr::surface::Instance,
        surface: vk::SurfaceKHR,
        width: u32,
        height: u32,
    ) -> Result<Self, EngineError> {
        let built = build_swapchain(device, &surface_loader, surface, width, height)?;
        Ok(Self {
            surface_loader,
            surface,
            swapchain_loader: built.swapchain_loader,
            swapchain: built.swapchain,
            images: built.images,
            image_views: built.image_views,
            format: built.format,
            stencil_image: built.stencil_image,
            stencil_image_view: built.stencil_image_view,
            stencil_image_memory: built.stencil_image_memory,
            width,
            height,
            image_available_semaphore: built.image_available_semaphore,
            render_finished_semaphores: built.render_finished_semaphores,
            device: device.device.clone(),
            present_queue: device.graphics_queue(),
            supports_local_read_input_attachment: built.supports_local_read_input_attachment,
        })
    }

    /// This swapchain's own real, selected surface format.
    #[must_use]
    pub fn format(&self) -> vk::Format {
        self.format
    }

    /// Recreates this swapchain's own `VkSwapchainKHR` and every resource
    /// that depends on it (images/views, stencil image, semaphores) at a
    /// new `width`x`height`, reusing the *same* `VkSurfaceKHR` this
    /// swapchain was originally created against -- unlike tearing down
    /// this whole `VulkanSwapchain` and building a brand-new one from a
    /// brand-new `VulkanDevice::create_surface` call, this never touches
    /// the underlying surface at all.
    ///
    /// REVIEW.md finding #232: recreating the surface itself on every
    /// resize (the only approach available before this method existed)
    /// is what caused finding #230's real Wayland `wp_fifo_manager_v1`
    /// crash -- that protocol's fifo-surface binding lives on the
    /// *surface*, not the swapchain, so a resize path that never
    /// recreates the surface can never trigger that conflict in the
    /// first place, rather than working around it after the fact. It
    /// also means a caller's own shape-pipeline registry never needs
    /// re-registering on resize: a swapchain's color format is a
    /// property of the surface (queried from `vkGetPhysicalDeviceSurface
    /// FormatsKHR` against `surface`), which this method never touches,
    /// so the format -- and therefore every pipeline compiled against it
    /// -- cannot change across a resize.
    ///
    /// Waits for the device to go idle first, then destroys every old
    /// dependent resource before building the new ones -- simpler than
    /// juggling `VkSwapchainCreateInfoKHR::oldSwapchain`'s own retire-
    /// but-still-must-destroy semantics for a case (a resize on this
    /// project's own fully-synchronous, single-frame-in-flight render
    /// loop) that has no in-flight work needing the smoother handoff
    /// `oldSwapchain` exists for in the first place.
    ///
    /// # Errors
    /// Returns [`EngineError::DeviceLost`] under the same conditions
    /// [`VulkanSwapchain::new`] does. On error, this swapchain's own
    /// prior resources have already been destroyed and not replaced --
    /// every real caller already treats a failed resize as fatal (the
    /// same way a failed `new` already is), so this is not a new,
    /// silently-swallowed risk.
    pub fn recreate(
        &mut self,
        device: &VulkanDevice,
        width: u32,
        height: u32,
    ) -> Result<(), EngineError> {
        // SAFETY: waiting for the device to go idle first guarantees no
        // GPU work still references this swapchain's own images/views/
        // semaphores/stencil resources before they're destroyed below --
        // matches this project's own established "wait idle before
        // teardown" convention (e.g. every real `Renderer::drop` that
        // owns a `VulkanDevice`, `HeadlessSwapchain`'s own `Drop`).
        unsafe {
            let _ = self.device.device_wait_idle();
        }
        // SAFETY: destroying every one of this swapchain's own current
        // resources -- `device_wait_idle` above guarantees nothing on
        // the GPU still references any of them -- in the same child-
        // before-parent order `Drop` already uses. `self.surface` is
        // deliberately NOT destroyed here -- reusing it unchanged is
        // this method's entire point.
        unsafe {
            self.device
                .destroy_semaphore(self.image_available_semaphore, None);
            for &sem in &self.render_finished_semaphores {
                self.device.destroy_semaphore(sem, None);
            }
            for &view in &self.image_views {
                self.device.destroy_image_view(view, None);
            }
            self.device
                .destroy_image_view(self.stencil_image_view, None);
            self.device.destroy_image(self.stencil_image, None);
            self.device.free_memory(self.stencil_image_memory, None);
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
        }

        let built = build_swapchain(device, &self.surface_loader, self.surface, width, height)?;
        self.swapchain_loader = built.swapchain_loader;
        self.swapchain = built.swapchain;
        self.images = built.images;
        self.image_views = built.image_views;
        self.format = built.format;
        self.stencil_image = built.stencil_image;
        self.stencil_image_view = built.stencil_image_view;
        self.stencil_image_memory = built.stencil_image_memory;
        self.width = width;
        self.height = height;
        self.image_available_semaphore = built.image_available_semaphore;
        self.render_finished_semaphores = built.render_finished_semaphores;
        self.supports_local_read_input_attachment = built.supports_local_read_input_attachment;
        Ok(())
    }
}

impl RhiSwapchain for VulkanSwapchain {
    fn extent(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn stencil_view_handle(&self) -> u64 {
        self.stencil_image_view.as_raw()
    }

    fn stencil_image_handle(&self) -> u64 {
        self.stencil_image.as_raw()
    }

    fn supports_local_read_input_attachment(&self) -> bool {
        self.supports_local_read_input_attachment
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

    /// A real windowed swapchain presents straight to the window surface
    /// and was never given a CPU-visible staging buffer -- only
    /// `HeadlessSwapchain` was built with readback in mind (Architecture
    /// review: RHI trait-object generalization, REVIEW.md finding #216).
    fn read_pixels_bgra8(&self) -> Result<Vec<u8>, EngineError> {
        Err(EngineError::PixelReadbackUnsupported)
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
            self.device
                .destroy_image_view(self.stencil_image_view, None);
            self.device.destroy_image(self.stencil_image, None);
            self.device.free_memory(self.stencil_image_memory, None);
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
            self.surface_loader.destroy_surface(self.surface, None);
        }
    }
}
