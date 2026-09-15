//! `VulkanCommandBuffer` -- the `RhiCommandBuffer` implementation. Split
//! out of `lib.rs` as one of its ten separable concerns (Architecture
//! review finding); its own `apply_layer_blur` method is the real caller
//! of `crate::blur`'s Dual-Kawase setup.

use std::sync::{Arc, Mutex};

use ash::vk;
use ash::vk::Handle;
use tre_engine::{
    EngineError, RhiBuffer, RhiCommandBuffer, RhiDevice, RhiPipelineState, RhiTexture, ScissorRect,
    TextureFormat,
};

use crate::blur::{create_blur_resources, BlurResources, PushConstants};
use crate::BINDLESS_TEXTURE_SENTINEL;

/// The Vulkan `RhiCommandBuffer` implementation -- see the `RhiCommandBuffer`
/// trait (`tre_engine`) for the full method contract this type provides.
/// Returned (as `Box<dyn RhiCommandBuffer>`) from `RhiDevice::begin_frame`;
/// owns no exclusively-its-own GPU resources beyond shared, cloned handles,
/// so it implements no `Drop` of its own.
pub struct VulkanCommandBuffer {
    pub(crate) device: ash::Device,
    pub(crate) command_buffer: vk::CommandBuffer,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pipeline_layout: Option<vk::PipelineLayout>,
    /// `VulkanDevice::blur_resources`'s own `Arc` clone -- see that
    /// field's own doc comment for why `apply_layer_blur` needs this
    /// copied in at construction rather than reached through its own
    /// `device: &dyn RhiDevice` parameter.
    pub(crate) blur_resources: Arc<Mutex<Option<BlurResources>>>,
    /// `VulkanDevice::instance`/`physical_device`, copied in for the same
    /// reason `blur_resources` is -- `apply_layer_blur`'s own lazy
    /// pipeline/buffer setup (`create_blur_resources`) needs both to
    /// select a real memory type, and neither is reachable through the
    /// `device: &dyn RhiDevice` trait object its signature is given.
    pub(crate) instance: ash::Instance,
    pub(crate) physical_device: vk::PhysicalDevice,
    /// `VulkanDevice::stencil_format`, copied in for `create_blur_
    /// resources`'s own pipeline creation -- the same reason `instance`/
    /// `physical_device` above are.
    pub(crate) stencil_format: vk::Format,
    /// The one persistent bindless descriptor set (`VulkanDevice::
    /// bindless_descriptor_set`), bound once per `set_pipeline` call.
    pub(crate) bindless_descriptor_set: vk::DescriptorSet,
    /// The real, runtime-clamped bindless array size (`VulkanDevice::
    /// bindless_capacity`), used by `bind_texture` to bounds-check its
    /// `bindless_index` argument (Phase 2 Code Review finding #69) before
    /// it can ever reach the GPU as an out-of-range descriptor index.
    pub(crate) bindless_capacity: u32,
    /// The bindless array index `draw_indexed` will push next, set by
    /// `bind_texture`. Starts at `BINDLESS_TEXTURE_SENTINEL` ("no texture,
    /// use vertex color") so a draw that never calls `bind_texture` keeps
    /// behaving exactly like Phase 0's flat-color path.
    pub(crate) texture_index: u32,
    /// IMPLEMENTATION.md Phase 6 Step 6.4.1: cloned from `VulkanDevice`
    /// (which owns the real loader) so `begin_render_to_texture`/
    /// `end_render_to_texture`/`resume_swapchain_rendering` -- all
    /// `RhiCommandBuffer` methods, called on this struct, not on
    /// `VulkanDevice` -- can call `cmd_begin_rendering`/`cmd_end_rendering`
    /// themselves; cheap to clone, the same pattern `device: ash::Device`
    /// above already uses.
    pub(crate) dynamic_rendering: ash::khr::dynamic_rendering::Device,
    /// The swapchain image view/extent `VulkanDevice::begin_frame`
    /// originally began rendering into, stashed here (nothing previously
    /// persisted it past that function's own local scope) so
    /// `resume_swapchain_rendering` can re-begin an equivalent rendering
    /// scope after one or more `begin_render_to_texture`/
    /// `end_render_to_texture` pairs redirected rendering elsewhere --
    /// with `LOAD_OP_LOAD`, not `CLEAR`, since whatever was already drawn
    /// (and the frame's own initial clear, from `begin_frame` itself)
    /// must be preserved, not erased.
    pub(crate) swapchain_color_view: vk::ImageView,
    /// This same swapchain's own stencil view, stashed for the same
    /// reason -- `resume_swapchain_rendering`'s own `RenderingInfo` must
    /// still pair a stencil attachment, exactly as `begin_frame`'s own
    /// did, so a stencil-and-cover draw recorded after a layer redirect
    /// still has one to write into.
    pub(crate) swapchain_stencil_view: vk::ImageView,
    /// The swapchain's own real extent, stashed alongside its views for
    /// the same reason: `self.width`/`self.height` above are repurposed
    /// by `begin_render_to_texture` to mean "the currently active render
    /// target's own dimensions" (`draw_indexed`'s push constants need the
    /// *active* target's size to map pixel-space positions to NDC
    /// correctly, not always the swapchain's) -- `resume_swapchain_
    /// rendering` restores `self.width`/`self.height` from these two
    /// fields, which never change once `begin_frame` sets them.
    pub(crate) swapchain_width: u32,
    pub(crate) swapchain_height: u32,
    /// Phase 10 Step 10.2.3: the layout `VulkanDevice::begin_frame`
    /// actually transitioned `swapchain_color_view` into this frame --
    /// `RENDERING_LOCAL_READ_KHR` when `local_read_active` was true,
    /// `COLOR_ATTACHMENT_OPTIMAL` otherwise. `resume_swapchain_rendering`
    /// must declare this SAME layout (not hardcode `COLOR_ATTACHMENT_
    /// OPTIMAL`) when re-beginning rendering into the swapchain after a
    /// `PushLayer`/`PopLayer` redirect -- a real regression this step's
    /// own first full demo regression sweep caught: `vkCmdBeginRendering`
    /// validation fails outright if the declared `imageLayout` doesn't
    /// match the image's actual current layout, since nothing transitions
    /// the swapchain image back to `COLOR_ATTACHMENT_OPTIMAL` in between
    /// (`begin_render_to_texture`/`end_render_to_texture` only ever touch
    /// a *texture's* own image, never the swapchain's).
    pub(crate) swapchain_color_layout: vk::ImageLayout,
    /// `VulkanDevice::blend_read`'s descriptor set, copied in at
    /// construction (`VulkanDevice::begin_frame`) for the same reason
    /// `bindless_descriptor_set` above is -- `insert_framebuffer_fetch_barrier`
    /// needs to bind it directly, with no way back to a `VulkanDevice`
    /// through this struct's `RhiCommandBuffer` trait methods. `None`
    /// when `framebuffer_fetch_blend_supported()` is `false`.
    pub(crate) blend_input_descriptor_set: Option<vk::DescriptorSet>,
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
            self.device.cmd_bind_vertex_buffers(
                self.command_buffer,
                0,
                &[raw],
                &[u64::from(offset)],
            );
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
                u64::from(offset),
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

    /// Phase 10 Step 10.2.3: called before every `PipelineKind::
    /// FlatColorBlend` draw (`execute_frame`'s own special case), never
    /// once per frame -- each such draw must see whatever the LATEST
    /// framebuffer state is, including ordinary draws that ran since the
    /// last blend-mode draw. Inserts the by-region barrier the spec
    /// requires between a color-attachment WRITE and a later input-
    /// attachment READ of the same pixels, then binds set 1 (the
    /// input-attachment descriptor `VulkanDevice::begin_frame` rewrote
    /// this frame) against `self.pipeline_layout` -- safe to bind here,
    /// immediately after `set_pipeline`, since `execute_frame` always
    /// calls `set_pipeline` with the `FlatColorBlend` pipeline (whose
    /// layout is `create_blend_pipeline_layout`'s own two-set layout)
    /// right before this.
    ///
    /// Classic (non-`_2`) `vkCmdPipelineBarrier`, not `VK_KHR_
    /// synchronization2`'s `vkCmdPipelineBarrier2`: `INPUT_ATTACHMENT_
    /// READ`/`BY_REGION` are both core (non-KHR) enum values, so nothing
    /// here needed that extension as a new dependency.
    ///
    /// Out of scope for this pass: only correct when the active render
    /// target is the swapchain/headless attachment `begin_frame` set up
    /// -- a `PushLayer` render-to-texture target never gets its own
    /// `RENDERING_LOCAL_READ_KHR` layout or input-attachment descriptor
    /// write, so a `FlatColorBlend` draw issued while one is active would
    /// bind this same (stale, swapchain-pointing) descriptor set instead
    /// of the layer's own texture.
    ///
    /// # Panics
    /// Panics if called with no pipeline yet bound, or on a device
    /// without `framebuffer_fetch_blend_supported()` -- both are caller
    /// contract violations: `execute_frame` only calls this immediately
    /// after `set_pipeline` for a `FlatColorBlend` draw, and `shapes.rs`
    /// never dispatches to that pipeline kind unless the capability was
    /// already checked.
    fn insert_framebuffer_fetch_barrier(&mut self) {
        let barrier = vk::MemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_access_mask(vk::AccessFlags::INPUT_ATTACHMENT_READ);
        let descriptor_set = self.blend_input_descriptor_set.expect(
            "insert_framebuffer_fetch_barrier requires framebuffer_fetch_blend_supported()",
        );
        let layout = self
            .pipeline_layout
            .expect("set_pipeline must be called before insert_framebuffer_fetch_barrier");
        // SAFETY: `self.command_buffer` is recording; `descriptor_set`
        // was allocated by this same device in `VulkanDevice::new` and
        // rewritten fresh this frame in `begin_frame`; `layout` is the
        // `FlatColorBlend` pipeline's own two-set layout, whose set 1
        // matches `descriptor_set`'s own layout exactly
        // (`create_blend_pipeline_layout`).
        unsafe {
            self.device.cmd_pipeline_barrier(
                self.command_buffer,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::BY_REGION,
                &[barrier],
                &[],
                &[],
            );
            self.device.cmd_bind_descriptor_sets(
                self.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                layout,
                1,
                &[descriptor_set],
                &[],
            );
        }
    }

    fn begin_render_to_texture(
        &mut self,
        texture: &dyn RhiTexture,
        logical_width: u32,
        logical_height: u32,
    ) {
        let image = vk::Image::from_raw(texture.image_handle());
        let view = vk::ImageView::from_raw(texture.raw_handle());
        let (width, height) = texture.dimensions();

        // Undefined -> COLOR_ATTACHMENT_OPTIMAL, the same reasoning as
        // `VulkanDevice::begin_frame`'s own swapchain-image barrier:
        // dynamic rendering has no render pass to do this transition
        // implicitly. Always UNDEFINED as the old layout regardless of
        // what a reused pooled texture's own prior layout actually was --
        // correct and intentional, since this scope clears to transparent
        // immediately after regardless of any prior content.
        let barrier = vk::ImageMemoryBarrier::default()
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .image(image)
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .level_count(1)
                    .layer_count(1),
            );

        let color_attachment = vk::RenderingAttachmentInfo::default()
            .image_view(view)
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 0.0],
                },
            });
        let color_attachments = [color_attachment];
        // No stencil attachment -- transient targets don't have one
        // (IMPLEMENTATION.md Phase 6 Step 6.4.1's own scope decision);
        // stencil-and-cover draws are not supported inside a layer yet.
        let rendering_info = vk::RenderingInfo::default()
            .render_area(vk::Rect2D {
                offset: vk::Offset2D::default(),
                extent: vk::Extent2D { width, height },
            })
            .layer_count(1)
            .color_attachments(&color_attachments);

        // SAFETY: `self.command_buffer` is recording (begun by
        // `VulkanDevice::begin_frame`); ending whatever rendering scope is
        // currently active before beginning a new one is always valid --
        // dynamic rendering permits any number of begin/end pairs within
        // one command buffer, just never nested. `image`/`view` come from
        // `texture`, whose `RhiTexture` contract guarantees they are live
        // Vulkan objects this same device created with `COLOR_ATTACHMENT`
        // usage (`RhiDevice::acquire_transient_target`).
        unsafe {
            self.dynamic_rendering
                .cmd_end_rendering(self.command_buffer);
            self.device.cmd_pipeline_barrier(
                self.command_buffer,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
            self.dynamic_rendering
                .cmd_begin_rendering(self.command_buffer, &rendering_info);
            self.device.cmd_set_viewport(
                self.command_buffer,
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
                self.command_buffer,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D::default(),
                    extent: vk::Extent2D { width, height },
                }],
            );
        }

        // `draw_indexed`'s push constants map pixel-space positions to
        // NDC using `self.width`/`self.height` -- REVIEW.md finding #152:
        // this must be the caller's own intended/logical size, not
        // `texture`'s own real physical size. `RhiDevice::acquire_
        // transient_target`'s documented "oversized borrow" fallback can
        // hand back a texture larger than requested, and any command
        // recorded against the smaller, intended size (every `PushLayer`
        // inner draw, baked at `Canvas` record time before the real
        // texture is ever acquired) would otherwise get its NDC mapping
        // computed against the wrong, larger size, confining it to a
        // small corner of the oversized image. Using the intended size
        // here instead is provably equivalent to sizing the texture
        // exactly right: the viewport/scissor/render area below are still
        // the texture's own real (possibly larger) extent, so content
        // simply draws "stretched" to fill it -- and that stretch is
        // exactly undone later when it's sampled back through a
        // normalized `(0,0)`-`(1,1)` UV read (`PopLayer`'s own composite
        // quad) and redrawn at its own real, requested on-screen size.
        self.width = logical_width;
        self.height = logical_height;
    }

    fn end_render_to_texture(&mut self, texture: &dyn RhiTexture) {
        let image = vk::Image::from_raw(texture.image_handle());

        // COLOR_ATTACHMENT_OPTIMAL -> SHADER_READ_ONLY_OPTIMAL: the old
        // layout is statically known, not queried -- nothing but
        // `begin_render_to_texture`'s own barrier above can have touched
        // this image's layout in between, within one command buffer.
        let barrier = vk::ImageMemoryBarrier::default()
            .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_access_mask(vk::AccessFlags::SHADER_READ)
            .image(image)
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .level_count(1)
                    .layer_count(1),
            );

        // SAFETY: `self.command_buffer` is recording, with a rendering
        // scope `begin_render_to_texture` began still active (`RhiDevice`/
        // `RhiCommandBuffer`'s own contract: every `begin_render_to_texture`
        // call is paired with exactly one matching `end_render_to_texture`
        // before anything else touches this command buffer's rendering
        // state); `image` is that same call's own `texture`.
        unsafe {
            self.dynamic_rendering
                .cmd_end_rendering(self.command_buffer);
            self.device.cmd_pipeline_barrier(
                self.command_buffer,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
        }
    }

    fn begin_render_to_texture_no_end(
        &mut self,
        texture: &dyn RhiTexture,
        logical_width: u32,
        logical_height: u32,
    ) {
        // IMPLEMENTATION.md Step 7.2.1: identical to `begin_render_to_
        // texture` above except it never calls `cmd_end_rendering` first
        // -- the caller's own contract (that method's doc comment) is
        // that nothing is currently active, having just called `end_
        // render_to_texture` on the previous level. A first design tried
        // combining that end-and-barrier step with this begin into one
        // call (`chain_render_to_texture`) but left no point to call
        // `RhiDevice::register_bindless` with no render pass active --
        // every proven-working caller of that method needs exactly that,
        // and calling it while a pass *was* active produced fully wrong
        // sampled output on real hardware. Reverted in favor of this
        // smaller, separate method, used together with a plain `end_
        // render_to_texture` call immediately before it.
        let image = vk::Image::from_raw(texture.image_handle());
        let view = vk::ImageView::from_raw(texture.raw_handle());
        let (width, height) = texture.dimensions();

        let barrier = vk::ImageMemoryBarrier::default()
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .image(image)
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .level_count(1)
                    .layer_count(1),
            );

        let color_attachment = vk::RenderingAttachmentInfo::default()
            .image_view(view)
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 0.0],
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

        // SAFETY: `self.command_buffer` is recording, with no rendering
        // scope currently active (this method's own contract, above);
        // `image`/`view` come from `texture`, guaranteed live by
        // `RhiTexture`'s own contract, same as `begin_render_to_texture`.
        unsafe {
            self.device.cmd_pipeline_barrier(
                self.command_buffer,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
            self.dynamic_rendering
                .cmd_begin_rendering(self.command_buffer, &rendering_info);
            self.device.cmd_set_viewport(
                self.command_buffer,
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
                self.command_buffer,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D::default(),
                    extent: vk::Extent2D { width, height },
                }],
            );
        }

        // REVIEW.md finding #152: same reasoning as `begin_render_to_
        // texture`'s own comment above -- the caller's intended/logical
        // size, not the texture's real (possibly oversized) one.
        self.width = logical_width;
        self.height = logical_height;
    }

    fn resume_swapchain_rendering(&mut self) {
        let color_attachment = vk::RenderingAttachmentInfo::default()
            .image_view(self.swapchain_color_view)
            .image_layout(self.swapchain_color_layout)
            // LOAD, not CLEAR: whatever was already drawn into the
            // swapchain before the redirect (plus `begin_frame`'s own
            // initial clear) must be preserved, not erased.
            .load_op(vk::AttachmentLoadOp::LOAD)
            .store_op(vk::AttachmentStoreOp::STORE);
        let color_attachments = [color_attachment];
        let stencil_attachment = vk::RenderingAttachmentInfo::default()
            .image_view(self.swapchain_stencil_view)
            .image_layout(vk::ImageLayout::STENCIL_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::LOAD)
            .store_op(vk::AttachmentStoreOp::DONT_CARE);
        let rendering_info = vk::RenderingInfo::default()
            .render_area(vk::Rect2D {
                offset: vk::Offset2D::default(),
                extent: vk::Extent2D {
                    width: self.swapchain_width,
                    height: self.swapchain_height,
                },
            })
            .layer_count(1)
            .color_attachments(&color_attachments)
            .stencil_attachment(&stencil_attachment);

        // SAFETY: `self.command_buffer` is recording, with no rendering
        // scope currently active (the last `end_render_to_texture` call
        // ended one); `self.swapchain_color_view`/`swapchain_stencil_view`
        // are the same views `VulkanDevice::begin_frame` already
        // transitioned to `self.swapchain_color_layout`/
        // `STENCIL_ATTACHMENT_OPTIMAL` this frame, and nothing since has
        // changed either layout (`begin_render_to_texture`/`end_render_
        // to_texture` only ever touch a *texture's* own image, never the
        // swapchain's), so no barrier is needed here -- only ending/
        // beginning is. Viewport
        // and scissor are separate, persistent command-buffer state --
        // `cmd_begin_rendering` does not reset them on its own -- so both
        // must be explicitly restored to the swapchain's own real extent
        // here, or a draw recorded after resuming would still render
        // through whatever a preceding `begin_render_to_texture` last set
        // (a real bug this step's own first real run caught: the
        // composite draw silently rendered through the layer's own
        // smaller viewport instead of the swapchain's).
        unsafe {
            self.dynamic_rendering
                .cmd_begin_rendering(self.command_buffer, &rendering_info);
            self.device.cmd_set_viewport(
                self.command_buffer,
                0,
                &[vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: self.swapchain_width as f32,
                    height: self.swapchain_height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            );
            self.device.cmd_set_scissor(
                self.command_buffer,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D::default(),
                    extent: vk::Extent2D {
                        width: self.swapchain_width,
                        height: self.swapchain_height,
                    },
                }],
            );
        }

        // Restore the real, active-target dimensions `draw_indexed`'s push
        // constants need -- see `begin_render_to_texture`'s own comment on
        // `self.width`/`self.height`.
        self.width = self.swapchain_width;
        self.height = self.swapchain_height;
    }

    fn apply_layer_blur(
        &mut self,
        device: &dyn RhiDevice,
        source: &dyn RhiTexture,
        width: u32,
        height: u32,
    ) -> Result<Box<dyn RhiTexture>, EngineError> {
        // Lazy, once-per-process setup (IMPLEMENTATION.md Step 7.2.2) --
        // see `BlurResources`'s own doc comment for the full design.
        let blur = {
            let mut guard = self.blur_resources.lock().expect("blur resources poisoned");
            *guard.get_or_insert_with(|| {
                create_blur_resources(
                    &self.instance,
                    self.physical_device,
                    &self.device,
                    self.stencil_format,
                )
            })
        };

        let half_size = ((width / 2).max(1), (height / 2).max(1));
        let quarter_size = ((width / 4).max(1), (height / 4).max(1));

        let raw_device = self.device.clone();
        let raw_cmd_buffer = self.command_buffer;
        // `set_index` selects one of `blur.descriptor_sets`'s own 4 real
        // hop sets -- see that field's own doc comment for why this
        // demo-proven "one set per hop" shape is used instead of
        // reusing (and repeatedly updating) a single one.
        let point_at = |set_index: usize, view: vk::ImageView| {
            let image_info = vk::DescriptorImageInfo::default()
                .image_view(view)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .sampler(blur.sampler);
            // SAFETY: `raw_device` is valid for this whole method's
            // duration; `blur.sampler`/`blur.descriptor_sets` were
            // created once in `create_blur_resources` and remain valid
            // for the process's lifetime; `image_info` is a local
            // borrowed only for this call.
            unsafe {
                raw_device.update_descriptor_sets(
                    &[vk::WriteDescriptorSet::default()
                        .dst_set(blur.descriptor_sets[set_index])
                        .dst_binding(0)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .image_info(std::slice::from_ref(&image_info))],
                    &[],
                );
            }
        };
        // REVIEW.md finding #130's own real fix: a raw `cmd_draw_indexed`
        // call, never `RhiCommandBuffer::draw_indexed` -- that wrapper's
        // own second, unconditional `cmd_push_constants` call would
        // silently clobber `dest_size` below with `self.width`/`self.
        // height` instead.
        let draw_hop = |pipeline: vk::Pipeline, set_index: usize, dest_size: (u32, u32)| {
            let push_constants: [f32; 2] = [dest_size.0 as f32, dest_size.1 as f32];
            // SAFETY: `raw_device`/`raw_cmd_buffer` are valid for this
            // whole method's duration and `raw_cmd_buffer` is currently
            // in a recording state; `pipeline` (caller-supplied) and
            // every `blur.*` handle referenced below were created once
            // in `create_blur_resources` and remain valid for the
            // process's lifetime; `push_constants` is a local borrowed
            // only for its own call.
            unsafe {
                raw_device.cmd_bind_pipeline(
                    raw_cmd_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipeline,
                );
                raw_device.cmd_bind_descriptor_sets(
                    raw_cmd_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    blur.pipeline_layout,
                    0,
                    &[blur.descriptor_sets[set_index]],
                    &[],
                );
                raw_device.cmd_push_constants(
                    raw_cmd_buffer,
                    blur.pipeline_layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    bytemuck::cast_slice(&push_constants),
                );
                raw_device.cmd_bind_vertex_buffers(
                    raw_cmd_buffer,
                    0,
                    &[blur.unit_quad_vertex_buffer],
                    &[0],
                );
                raw_device.cmd_bind_index_buffer(
                    raw_cmd_buffer,
                    blur.unit_quad_index_buffer,
                    0,
                    vk::IndexType::UINT32,
                );
                raw_device.cmd_draw_indexed(raw_cmd_buffer, 6, 1, 0, 0, 0);
            }
        };

        // L0 (`source`) -> L1: downsample to half size. Set 0 reads L0.
        // REVIEW.md finding #189: every `acquire_transient_target` call in
        // this chain now propagates a real `Err` (pool exhaustion under
        // real VRAM pressure is a genuine, recoverable condition, not a
        // programmer-error panic) instead of `.expect()`-ing it. Nothing
        // has been acquired yet at this first hop, so there is nothing to
        // release on failure.
        point_at(0, vk::ImageView::from_raw(source.raw_handle()));
        let l1 = device.acquire_transient_target(
            half_size.0,
            half_size.1,
            TextureFormat::Rgba16Float,
        )?;
        self.begin_render_to_texture_no_end(&*l1, half_size.0, half_size.1);
        draw_hop(blur.downsample_pipeline, 0, half_size);
        self.end_render_to_texture(&*l1);

        // L1 -> L2: downsample to quarter size. Set 1 reads L1.
        point_at(1, vk::ImageView::from_raw(l1.raw_handle()));
        let l2 = match device.acquire_transient_target(
            quarter_size.0,
            quarter_size.1,
            TextureFormat::Rgba16Float,
        ) {
            Ok(target) => target,
            // `l1` is still held (not yet released) at this point -- must
            // go back to the pool before propagating, or this failure
            // path leaks it.
            Err(e) => {
                device.release_transient_target(l1);
                return Err(e);
            }
        };
        self.begin_render_to_texture_no_end(&*l2, quarter_size.0, quarter_size.1);
        draw_hop(blur.downsample_pipeline, 1, quarter_size);
        self.end_render_to_texture(&*l2);
        device.release_transient_target(l1);

        // L2 -> U1: upsample back to half size. Set 2 reads L2.
        point_at(2, vk::ImageView::from_raw(l2.raw_handle()));
        let u1 = match device.acquire_transient_target(
            half_size.0,
            half_size.1,
            TextureFormat::Rgba16Float,
        ) {
            Ok(target) => target,
            Err(e) => {
                device.release_transient_target(l2);
                return Err(e);
            }
        };
        self.begin_render_to_texture_no_end(&*u1, half_size.0, half_size.1);
        draw_hop(blur.upsample_pipeline, 2, half_size);
        self.end_render_to_texture(&*u1);
        device.release_transient_target(l2);

        // U1 -> U0: upsample back to full size -- the real result. Set 3
        // reads U1.
        point_at(3, vk::ImageView::from_raw(u1.raw_handle()));
        let u0 = match device.acquire_transient_target(width, height, TextureFormat::Rgba16Float) {
            Ok(target) => target,
            Err(e) => {
                device.release_transient_target(u1);
                return Err(e);
            }
        };
        self.begin_render_to_texture_no_end(&*u0, width, height);
        draw_hop(blur.upsample_pipeline, 3, (width, height));
        self.end_render_to_texture(&*u0);
        device.release_transient_target(u1);

        Ok(u0)
    }

    fn raw_handle(&self) -> u64 {
        self.command_buffer.as_raw()
    }
}
