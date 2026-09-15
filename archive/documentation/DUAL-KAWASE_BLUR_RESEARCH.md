# **Diagnostic Analysis and Remediation Framework for Bindless Texture Sampling Failures in Offscreen Vulkan Render Passes**

A critical blocker in modern graphics hardware interfaces utilizing Vulkan dynamic rendering and bindless descriptor architectures occurs when offscreen color attachments fail to sample correctly in subsequent render passes1. During the implementation of an offscreen Dual-Kawase blur algorithm within a Rust-based Render Hardware Interface (RHI), an issue was identified where sampling a bindless texture while rendering into an offscreen target consistently returns all-zero data ![][image1]1. This behavior persists on hardware without triggering Vulkan validation layer errors, contrasting sharply with swapchain rendering passes where the identical bindless sampling path functions as intended1.  
This report provides a systematic diagnostic breakdown of the failure mode, synthesizes empirical findings across twelve eliminated hypotheses, establishes Vulkan specification-level root cause mechanisms, and outlines technical remediation procedures for the RHI engine architecture1.

## **1\. Architectural Context and Failure Symptomology**

The failure manifests during multi-pass offscreen filtering chains, specifically during the execution of a Dual-Kawase downsample pass1. The RHI renders initial scene content into a transient offscreen color attachment (![][image2]), transitions ![][image2] to a bindless descriptor slot, and then executes a downsample pass by sampling ![][image2] through the bindless descriptor array while rendering into a second offscreen transient target (![][image3])1.  
During execution, GPU pixel readbacks demonstrate that the shader reads zero values for every texel query across ![][image2], causing the resulting pass ![][image3] to output background clear colors across the target1.

### **Empirical Pipeline Characteristics**

* **Swapchain Isolation Paradox**: Sampling the exact same bindless texture slot while rendering directly into the swapchain backbuffer produces correct visual output1. The failure condition is uniquely triggered when the active color render target is an offscreen image target reached via begin\_render\_to\_texture or begin\_render\_to\_texture\_no\_end1.  
* **Pipeline and Shader Integrity**: Substituting shader output with hardcoded constants confirms that viewport transformation, scissor testing, rasterization, pipeline binding, push constants, vertex streams, and color attachment blend states function correctly1.  
* **Input Parameter Propagation**: Directly outputting interpolated texture coordinates (![][image4]) and push constant texture indices (![][image5]) as color channels confirms that coordinates and indices reach the fragment shader without numerical corruption or truncation1.  
* **Validation Layer Silence**: Standard Vulkan Validation Layers return zero warnings or errors during command buffer recording, queue submission, or queue execution1.

During the initial investigation, a secondary issue involving double vkCmdEndRendering command buffer calls was identified when directly chaining end\_render\_to\_texture into begin\_render\_to\_texture1. This issue was resolved by introducing RhiCommandBuffer::begin\_render\_to\_texture\_no\_end, ensuring that render pass end calls are decoupled from barrier insertions and subsequent pass initialization1. While necessary for driver compliance, resolving this double-end error did not alter the all-zero sampling behavior1.

## **2\. Empirical Hypothesis Elimination Matrix**

An extensive diagnostic elimination process was executed to isolate the failure mechanism, testing twelve independent variables across the host RHI and GPU execution state1.

| Hypothesis ID | Investigated Variable | Diagnostic Methodology | Observed Result | Elimination Conclusion |
| :---- | :---- | :---- | :---- | :---- |
| **H1** | Redundant cmd\_end\_rendering Calls | Introduced begin\_render\_to\_texture\_no\_end to prevent double-ending active passes1. | Validation error eliminated; sampling failure persisted1. | Independent bug resolved; not the root cause of zero reads1. |
| **H2** | Mid-Pass Bindless Registration | Moved register\_bindless outside active render passes1. | Execution order aligned with swapchain pass structures1. | Registration timing relative to pass boundaries ruled out1. |
| **H3** | Loss of nonuniformEXT Specifier | Fully inlined descriptor indices at every sampling tap in GLSL1. | Layout matched working reference shaders byte-for-byte1. | Decorator stripping in GLSL compilation ruled out1. |
| **H4** | Multi-Tap Filter Offset Math | Reduced multi-tap Kawase kernel to single unweighted texture() call1. | Shader reads returned zero across all UV coordinates1. | Sample offset math and kernel scaling ruled out1. |
| **H5** | Descriptor Slot Overwrite | Maintained distinct descriptor indices (![][image6]) across frames1. | Verified slot separation (![][image7] vs ![][image8]) in host tracking1. | Descriptor index collision or write race ruled out1. |
| **H6** | Render Target Format Mismatch | Evaluated floating-point (Rgba16Float) against Bgra8Srgb swapchain formats1. | Both formats failed identically during offscreen passes1. | Format-specific sampler incompatibility ruled out1. |
| **H7** | Render Target Scale Delta | Tested 1:1 matching dimensions versus 1:2 downsample resolutions1. | Both target size configurations failed identically1. | Scale mismatch or mipmap generation errors ruled out1. |
| **H8** | Shader Source Code Defects | Replaced Kawase fragment shader with unmodified, proven bindless\_textured.frag1. | Proven shader failed when targeted offscreen1. | Shader source code or compilation logic ruled out1. |
| **H9** | Push Constant / UV Corruption | Visualized frag\_uv and pc.texture\_index directly as RGB output channels1. | Colors displayed expected gradients and index values1. | Pipeline input assembly and stage routing ruled out1. |
| **H10** | Missing Device Feature Flags | Audited descriptor indexing features (runtime\_descriptor\_array, partially\_bound)1. | All required device features confirmed active1. | Device initialization or missing extensions ruled out1. |
| **H11** | Pipeline / Render Pass Setup | Emitted hardcoded ![][image9] color output1. | Red pixels rasterized successfully across composite targets1. | Rasterization, blending, viewport, and scissor ruled out1. |
| **H12** | Render Target Context | Tested identical bindless sampling calls against swapchain versus offscreen targets1. | Swapchain succeeds; offscreen target reads zero1. | Primary root cause dependency established1. |

## **3\. Analysis of Unresolved Root Cause Candidates**

Because basic pipeline parameters, GLSL shader source code, and device feature flags have been ruled out, the failure points to specific interactions within the Vulkan specification state machine1. Four technical mechanisms explain why bindless texture sampling returns zero exclusively during offscreen render passes.

### **3.1. Pipeline Barrier Memory Coherency and Layout Synchronization**

When transitioning an offscreen image (![][image2]) from a color attachment output to a fragment shader input for pass ![][image3], explicit memory barriers and pipeline stage dependencies must be recorded. Swapchain backbuffers mask missing barriers because presentation engines force synchronization during image acquisition and queue submission. Offscreen transient targets lack presentation boundaries.  
If the RHI omits an explicit VkImageMemoryBarrier2 between the end of pass ![][image2] and the start of pass ![][image3], two hardware-level failures occur. First, the GPU L2 cache holds the written color attachment data, but the L1 texture cache utilized by the Execution Units has not been invalidated, causing texture units to read stale memory segments containing zero. Second, if ![][image2] remains in VK\_IMAGE\_LAYOUT\_COLOR\_ATTACHMENT\_OPTIMAL without transitioning to VK\_IMAGE\_LAYOUT\_SHADER\_READ\_ONLY\_OPTIMAL, hardware memory compression units (such as AMD DCC or Nvidia Intel Compressed Formats) do not decompress color blocks prior to sampling, resulting in zeroed reads.

### **3.2. Descriptor Set Compatibility and Binding Scope Under Dynamic Rendering**

Under Vulkan Dynamic Rendering (VK\_KHR\_dynamic\_rendering), descriptor sets bound prior to vkCmdBeginRendering remain valid only if the pipeline layout bound during the offscreen pass is identical to or layout-compatible with the pipeline layout used when binding the descriptor set.  
If the offscreen rendering pipeline uses a distinct VkPipelineLayout handle—or if the RHI executes vkCmdBindPipeline with a layout that redefines Set 0 or Push Constant ranges—Vulkan invalidates previously bound descriptor sets for that set index. Under Vulkan's descriptorBindingPartiallyBound feature, sampling an invalidated descriptor slot does not trigger a driver crash or validation layer error; instead, the hardware returns ![][image10] for all channels.

### **3.3. Descriptor Set Layout Update-After-Bind Flags**

When registering bindless textures dynamically via host writes (vkUpdateDescriptorSets), the underlying descriptor pool and descriptor set layout must explicitly configure VK\_DESCRIPTOR\_SET\_LAYOUT\_CREATE\_UPDATE\_AFTER\_BIND\_POOL\_BIT and VK\_DESCRIPTOR\_BINDING\_UPDATE\_AFTER\_BIND\_BIT. If register\_bindless updates the descriptor array on the host while command buffers are being recorded or executed, but the descriptor set layout was instantiated without update-after-bind flags, host updates execute without raising a validation error while deferring GPU descriptor page updates until device idle. Execution units sampling ![][image2]'s descriptor index consequently read an unpopulated page entry.

### **3.4. Offscreen Image Creation Flags and Sampler Component Swizzle**

Offscreen images constructed for transient passes must explicitly declare sampling usage flags during creation via VkImageCreateInfo, requiring VK\_IMAGE\_USAGE\_SAMPLED\_BIT alongside VK\_IMAGE\_USAGE\_COLOR\_ATTACHMENT\_BIT. Omitted sampling flags allow drivers to place image memory in non-sampled memory banks. Furthermore, if VkImageViewCreateInfo.components is populated with VK\_COMPONENT\_SWIZZLE\_ZERO for any channel during offscreen target initialization, the GPU texture sampler overrides fetched texel channels with absolute zeros regardless of underlying memory content.

## **4\. Systematic Diagnostic Protocol**

To isolate the precise root cause on physical hardware, graphics engineers should execute a diagnostic protocol combining Vulkan synchronization validation and frame capture analysis.

### **Frame Debugging Procedure**

> 1. **Enable Extended Synchronization Validation**: Configure Vulkan validation layers to track pipeline synchronization by enabling VK\_VALIDATION\_FEATURE\_ENABLE\_SYNCHRONIZATION\_VALIDATION\_EXT. This identifies missing pipeline barriers between ![][image2] color writes (VK\_PIPELINE\_STAGE\_COLOR\_ATTACHMENT\_OUTPUT\_BIT) and ![][image3] fragment reads (VK\_PIPELINE\_STAGE\_FRAGMENT\_SHADER\_BIT).  
> 2. **Inspect Pipeline State in RenderDoc**: Capture a frame during the execution of dual\_kawase\_blur\_demo.rs1. Select the downsample draw call in pass ![][image3] and verify that Set 1 lists a valid VkImageView handle for index pc.texture\_index. If the binding slot displays as unbound or null, the pipeline layout bound prior to the draw call is incompatible with the descriptor set layout.  
> 3. **Inspect Subresource Image Layouts**: Verify within the RenderDoc Resource Inspector that image ![][image2] resides in VK\_IMAGE\_LAYOUT\_SHADER\_READ\_ONLY\_OPTIMAL during pass ![][image3]. If it remains in VK\_IMAGE\_LAYOUT\_COLOR\_ATTACHMENT\_OPTIMAL, the RHI barrier engine failed to dispatch an layout transition barrier.  
> 4. **Evaluate Pixel History and Texel Fetch**: Open Pixel History on target ![][image3] and step through the fragment shader execution trace. Evaluate whether the GLSL texture() instruction returns zeros due to an unpopulated descriptor slot versus sampling uninitialized physical memory.

## **5\. Technical Solutions and RHI Architecture Remediation**

To resolve offscreen bindless texture sampling failures across Vulkan drivers, implement the following modifications within the Rust RHI implementation.

### **5.1. Explicit Pipeline Barriers for Transient Target Layout Transitions**

Ensure that the transition from rendering into ![][image2] to sampling ![][image2] in ![][image3] executes an explicit pipeline barrier using vkCmdPipelineBarrier2 via ash.

Rust  
pub fn transition\_attachment\_to\_shader\_read(  
    device: \&ash::Device,  
    command\_buffer: vk::CommandBuffer,  
    image: vk::Image,  
) {  
    let image\_memory\_barrier \= vk::ImageMemoryBarrier2::builder()  
        .src\_stage\_mask(vk::PipelineStageFlags2::COLOR\_ATTACHMENT\_OUTPUT)  
        .src\_access\_mask(vk::AccessFlags2::COLOR\_ATTACHMENT\_WRITE)  
        .dst\_stage\_mask(vk::PipelineStageFlags2::FRAGMENT\_SHADER)  
        .dst\_access\_mask(vk::AccessFlags2::SHADER\_READ)  
        .old\_layout(vk::ImageLayout::COLOR\_ATTACHMENT\_OPTIMAL)  
        .new\_layout(vk::ImageLayout::SHADER\_READ\_ONLY\_OPTIMAL)  
        .subresource\_range(vk::ImageSubresourceRange {  
            aspect\_mask: vk::ImageAspectFlags::COLOR,  
            base\_mip\_level: 0,  
            level\_count: 1,  
            base\_array\_layer: 0,  
            layer\_count: 1,  
        })  
        .image(image)  
        .build();

    let dependency\_info \= vk::DependencyInfo::builder()  
        .image\_memory\_barriers(std::slice::from\_ref(\&image\_memory\_barrier));

    unsafe {  
        device.cmd\_pipeline\_barrier2(command\_buffer, \&dependency\_info);  
    }  
}

### **5.2. Enforcing Descriptor Binding Within Dynamic Passes**

Ensure that descriptor sets are explicitly bound inside active offscreen passes after pipeline binding to guarantee state validity under dynamic rendering scope changes.

Rust  
pub fn begin\_offscreen\_pass\_and\_bind\_state(  
    device: \&ash::Device,  
    cmd\_buf: vk::CommandBuffer,  
    pipeline: vk::Pipeline,  
    pipeline\_layout: vk::PipelineLayout,  
    bindless\_descriptor\_set: vk::DescriptorSet,  
    rendering\_info: \&vk::RenderingInfo,  
) {  
    unsafe {  
        device.cmd\_begin\_rendering(cmd\_buf, rendering\_info);  
        device.cmd\_bind\_pipeline(cmd\_buf, vk::PipelineBindPoint::GRAPHICS, pipeline);

        device.cmd\_bind\_descriptor\_sets(  
            cmd\_buf,  
            vk::PipelineBindPoint::GRAPHICS,  
            pipeline\_layout,  
            1, // Bindless descriptor set index  
            std::slice::from\_ref(\&bindless\_descriptor\_set),  
            &\[\],  
        );  
    }  
}

### **5.3. Correcting Transient Texture Creation Parameters**

When allocating transient textures for offscreen targets, enforce explicit image usage flags and identity component mappings within the RHI memory manager.

Rust  
pub fn create\_transient\_render\_target(  
    device: \&ash::Device,  
    width: u32,  
    height: u32,  
    format: vk::Format,  
) \-\> (vk::Image, vk::ImageView) {  
    let image\_info \= vk::ImageCreateInfo::builder()  
        .image\_type(vk::ImageType::TYPE\_2D)  
        .format(format)  
        .extent(vk::Extent3D { width, height, depth: 1 })  
        .mip\_levels(1)  
        .array\_layers(1)  
        .samples(vk::SampleCountFlags::TYPE\_1)  
        .tiling(vk::ImageTiling::OPTIMAL)  
        .usage(vk::ImageUsageFlags::COLOR\_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)  
        .sharing\_mode(vk::SharingMode::EXCLUSIVE)  
        .initial\_layout(vk::ImageLayout::UNDEFINED);

    let image \= unsafe { device.create\_image(\&image\_info, None).unwrap() };

    let view\_info \= vk::ImageViewCreateInfo::builder()  
        .image(image)  
        .view\_type(vk::ImageViewType::TYPE\_2D)  
        .format(format)  
        .components(vk::ComponentMapping {  
            r: vk::ComponentSwizzle::IDENTITY,  
            g: vk::ComponentSwizzle::IDENTITY,  
            b: vk::ComponentSwizzle::IDENTITY,  
            a: vk::ComponentSwizzle::IDENTITY,  
        })  
        .subresource\_range(vk::ImageSubresourceRange {  
            aspect\_mask: vk::ImageAspectFlags::COLOR,  
            base\_mip\_level: 0,  
            level\_count: 1,  
            base\_array\_layer: 0,  
            layer\_count: 1,  
        });

    let view \= unsafe { device.create\_image\_view(\&view\_info, None).unwrap() };  
    (image, view)  
}

## **6\. Strategic Implementation Roadmap**

To resolve Finding \#130 and unblock the visual filter pipeline (Step 7.2.1 and Step 7.2.2), the RHI implementation must follow a four-phase remediation sequence1.

| Roadmap Phase | Core Objective | RHI Action Items | Success Verification |
| :---- | :---- | :---- | :---- |
| **Phase 1: Synchronization Audit** | Enforce explicit image memory barriers1. | Integrate transition\_attachment\_to\_shader\_read between offscreen pass ends and subsequent pass begins1. | Zero synchronization validation warnings during frame recording1. |
| **Phase 2: Layout Consolidation** | Standardize pipeline layouts across render passes1. | Unify VkPipelineLayout definitions across offscreen and swapchain pipelines; re-bind descriptor sets post begin\_rendering1. | Descriptor Set 1 remains bound and populated across offscreen passes1. |
| **Phase 3: Frame Capture Validation** | Verify texel fetch integrity in RenderDoc1. | Capture dual\_kawase\_blur\_demo.rs and inspect downsample texel reads1. | Pixel debug output confirms non-zero color values fetched from ![][image2]1. |
| **Phase 4: CI Re-integration** | Unblock Step 7.2.1 and Step 7.2.21. | Re-enable pixel assertions in dual\_kawase\_blur\_demo.rs; add demo to ci.yml; proceed to layer blending implementation1. | Automated CI pipeline passes pixel assertions across all hardware targets1. |

[image1]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAJUAAAAZCAYAAAA40GzsAAAE3ElEQVR4Xu2aeahuUxjGH1yZypQkXK4MF5Ey0+XqGkJmmSljV8iYebim8IehLrqZ6bokQ1z8QxmSPyQSf5g7J5ExY1Ek3qd3b2ed59v7u+tbL/uE/aun0372Ps/+9vvtvdZea31AT09Pz7+RZU2Pmabrjp7/NLNMC9T8u7jddJB400xXm14wvWG60bTCpCPaWct0j+lF02umUybvziaSs5PpCdPLppdMu07am09pTqR+KZGcnPpdY7pUzShHw4umPGx6EH5Ry5sWwy9sSaxhett0WrXN1u9D05X1AZlEcnY0fWbavtrew/SLac5fR+QRySmtn1Kak1s/5r5n2ln8YpaGF21v8Q80/WFaM/E2r7wdEq+Jm+AXk3Kq6Rt4N5tLJOd103zxFsG/kFEozYnULyWSM0r9zjc9JV4xB5g+Mi0l/gOmL8XjHf276RLxlXHTI+LxyWYhdhF/GOMoy9kYfswZ4s8z/Qi/jhwiOZH6pURyxpFfvw0rfwvxi1houlVN+B3+rprGd6bn1ExgH84Pd4f421T+ZeK3Eck5An7MseKfU/mzxW8jklNaP6U0p6R+YxjsGot4H4NPIvkeg00n+cL0sZoJ28I/9G3ib1n594nfRiTnXPgxvClSTq/8E8VvI5JTWj+lNKekfmwVn1RzVFaBn4D9tkK/6WL4/vWpmgmz0Xwx9XsAXzhziOTwKWy6Gfg+QX+u+G1Eckrrp5TmlNSPI8Cmc43EVvATNA2Rf0bzCXghY2omcPjddDGbVT6fhhwiORei+WbgTUD/JPHbiOSU1k8pzSmpH3usz9Ucle3gJ2CTqHxiekdN+Ek5V9LGTHgm571S+AJI/xbx24jkcC6GxxwpPofW9A8Rv41ITmn9lNKckvodZ/oNPiNQTN1S8a/yJnxOQ/nB9LyaCZwbYSYn3FLqG/hy8duI5BwKP+Z48c+r/N3EbyOSU1o/pTSnpH7HwPeFbqrp8JCm7u9u+AgjZUX48TeIr3xgely8feD/u6/4wyjNmQE/5mzxr4dPXK4qfhszUJ4TqV9KJGfU+rEF5sAgxErwE+ynO4w94fvWTzxOttHbOvHWgw+xl0u8a+GjynTui5NrbLLTSbdNTfsn20puDudcjkq2CZdU7hJvMXx9M2UvNHf/NTk5XDLhSHGdxMut32qmE0zLJF5Kbk7ke6i52PSWmiWMmc5Ss4JrTDcn25zT0nmPpzH4NHMp4VVMrCWy6HxqdL6HIxgtTkpODgv2EwZzNoAPudeutjnq+Rb+rlFTv7RyeN5UZJKTcwE8R1uFnPpxFKb1U3JyIt9DDVvFRWqWcC+ah5eEfetc+AiCJ2TxtL+9Dj67u7v4q5suMt1vutN08KS9DluBXzGxNtVETs4r8FlnHpvClvAq+NrZAgy+O3Lp42v4Zxg2k7ykHHYprMEV4ufUj60J//ch8VNyciLfQw1HmWzJwrDp/QrtT+o/zWEYPjTvgvnwJZmpgi/VTUP8LlkZPvLjTHwYhrH5P1x3dAQXPTdRs2OeVaNjOPl8spodw5ZQu+8QXHZ4Rs0O4IvtVJw3hQvq2m11Cd8JuYaX/gqha6bBu76NdEcE9s/8PRVHGl1ypmldNTuG7zQs6lQxB4M/O+qaeRi+jlkMRwaPYmKU0/P/YBZ8sNbT09PT09MzOn8Cno+ql/rXOo8AAAAASUVORK5CYII=>

[image2]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABQAAAAXCAYAAAALHW+jAAABH0lEQVR4Xu2TvytGURjHv2wvoSz+BH+ARQYlZbCRMthkUN6JUsrkDzDwWpUUo5RNISWLxPQalTKYlEGU+Jz3Offe4yzqmRSf+tR9zvee554fXemfX88QHuMTfuIrXuF2+pKHA1nDgTzw0C5b4V0eeBmUrW4zD7ysyhpO5oGXc3zHnjzwEJp8yJr+xIjs8rZwOctKwjbDdtfyILKEfdiN99gZx9dxJj5/oyFrOJoH0IsX8XkCL5NsFg+TuqSJb6q+XNCGO7gQ60U8rWJN421St+iXrS78KSk13MAXVRe1giflG9KU7AhaDMua3MgaPsb6Gp/jWHCvmADzeJbUYYVhvpsx2T9eMCe7cTcd+IBdsQ5HUq9iH+N4hLu4Lzvrv8gXF5o32diilcIAAAAASUVORK5CYII=>

[image3]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABYAAAAaCAYAAACzdqxAAAAA80lEQVR4Xu2TMatBYRzGHzYZxOIj3A9wFxlub/kGPoKVzWq4ySfArO7AJh/ApmySjcmVwWBSiijd+7z9j5z+OJ16TTq/+hXP4zzecwSIeBvydEi39I8e6YR2/B9yYQAZ/tSFC3HIiRe6cCUHOW1LF67UIMNFXbgyomea0oULduwCGQ9DmpZ0+Ah7+/Yx1HXhUaVZ73WD9unyVj+nDRku6IJk6FhlBiGH5/REkyqP0R9aVrlBiOEPyGntP89PgjbpHvc/qEHA8BdkbAYZ3njvp3TnZdbe9QIfhv7q8BUYulLZSzB0rUNXKrRLD/Qb8iUREQH8A8O1MFSrqM92AAAAAElFTkSuQmCC>

[image4]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEQAAAAZCAYAAACIA4ibAAAC/ElEQVR4Xu2YWahNURjHP/McumUIkYxPXMmYCKV0UYQXUgrlwVAyZHgwRIRkyFSuUChKkkyJkMxTKeSBB+FBFEqJ//9+3zp77dWxz76HKHv/69c9+//ttfda31nrW+tckVy5cpWpumAZuAUeg/nxcPa0CrwEbcFD8DUezpbqgHeiM4QaB6ZH4eypAfgB5oWBrKqx5AkpaIZoMnzWgWpwCbwFg8FR0drSr6aVSEdwEtwAF8EZMNdivhqJ1qdT4BzYAFaAY+Cwd1+SGkq8f/wCKb7beaPMq/Y8NxbqguftMO+XcjNkpecNAnvN3weG2ufNFuegeD3FrtuDLxKvPfXBXbDHrrmTPQDHwVhwwPw06g6uSTwhTNR681xCqHbgm2hy+E6qA3gPJon2K1HFEkLNNn+y6EMWg14W4wxZDlrZNXXZcOov2n6h520xr7XnpdVWiSeE4gDDhFBnwTPRDYPqCk5H4WSVSohbJqF6gpmiy2AXeA7ueXEmj+2XeB4T8l3iiUyrTZI+IewX/RF2zR10aiFaQqUSwukaaqBojGuzh3n8fL9wR+RdFZ1hFaJxt+xqK7c80iSkuehZyi3X2xJvl6gmkpyQzoFPcZBPQQvPYxF+JNqZbuYdAUvBC3BHfu8EzALJ/rC/TqxhxRJCsWjzfMUZzjqYWs2keELmmN8l8Fuafyjw+e0zIZVgo3n8GfCntEb0veyvE5cCvdGe58TCzdhNMDKIJaqNaMPVgb/I/N6Bz0LF3zwsWm6GjAcfwSswBiwwn7sKiyGTTW+i6PvKUZVof7iLUJyJbudhgecB01c90WPDG4mKa0mx+LABH8pix4EOFx0sPcdn18DUCewHV0QLKgvnMPBJ9MzS1O7jGcR/DuHa3ia16KQnbvfbRRO8G0yT6LncWUKxiK8NzX8lJvu16HTmMnPiLGGCZ4HzKZigzYqK2zeLfjnJ/es6CHaGpumE6DedKfUV/ZdCH8/j6ZI7zQcwwPMzI9aV6wbPJE/s7xD/ply5cuX6H/QToNbEIXO9hJoAAAAASUVORK5CYII=>

[image5]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAJYAAAAZCAYAAADT59fvAAAGCklEQVR4Xu2ZB6gkRRCGyzPnhIrxjKdiwIQopueJoHgIKipiOszombOoPCNGFNOZz4QRA+aIZw6YM4oihvMMGBFREa3vqmunpn2z7+3uW++dOx/8sF3TM9PTXV1d3StSU1NTU1MzvVhKtVhurOkdJqi+Vu2UX2iTK1R/J+2VXftfM69qx9zYBcapFs2NI5DzxZzgyPxCm4wSc6iec6xNVbfkxi7wiGr13DgCmUV1gGrO/EIHjJEedKzTpfuOtbDqJ5kxHKsbrCQ95lirqn6V7jrWTKqrxDp2RnGsBVVL58YOWFFGkGMdp/pOrEGHqs5V3a76VnWH2C4jMpvqeNU7qgdVD6t2K9Uos58USWXUBqHOAqrLVW+oHlNNTDa4W8r38e5tMtuBqs8zG7pPjC2D7elkmyfYEEsTXKd6Qiyx3lB1q+pN1TrpOmynekqsrTxv63BtKBwkxXtPSjae/1GyMQHXVF2qelGsn+P7HcbuXdVksX5iHLg/dyyeda/qOdWTqqPEcrL1pNwHf6T6zwfbV8nWMquoDhZ7yNtSONJyqk9V74kNAsysekbsY3xL+5rqT7FlqBk0cKCINYeYQ52ayrOLOSsd5cynekn1ixTv/UBseV1eLFoBDsZ3DBSx+lTfSOFYQMR4VMqOhcNfmWxEwI3Sb5Jt2EP1iWp0Ko9N19dN5aFAezeWsmMtpNpcNVX1vuoS1azpGgPNZI+cp5oihcMxNvRv7lhrqL4XaycsKTYJj2jUENld7D6eCTjoh2J5sY99WzAQPPiQzL5Psh+byiSblLdv1BA5WXW9FANTRZVjsStipswdbDuLvWeFYGPW/aY6W7WF2KDnNHMseFXKjgUXStmxwKMsu1jsx4hNQNr4peq0ouo03lJNymyDwWSJjuXQxh/FJpxzsRTRBHAW7o3OAR6BomPdIxbRIjjtx5ntZtUPYoGFib1y+XJ7rCYDOxbrP/b7U5mlhXI7B3BVjhXDbq6tQj04JdmJmItk12Awx2JZacWx8uUnX4KjiOSt4Etx7lgvi/VJxNvoHJbKTLBI7lhMhLydUaQ1Dkc0RHREsBgWmI28KHcswit2ljtgQFn22qHKscjl8tlTBbOYMM5SETvFGcyxnpXWHItdVoT+iQPXCXwLz8od6wWpbqMv+RelMjlgJHestVJ5UqNGcyaI1R+f2dumyrHIX7CTpAIhkjJhvFWiY5HHnJV+swkg9HunNaNP9YpYG44uX5pG7lhE2LjEMmB5ZGGZqXKs0cEGuyT7cBxqcn41kGMRraoci4kO3EM5j+i5Yy2RyiyHg0H/PyS2UflC7EC7Y9yx8sHyDva1fM9U3qFRwyD5Wz/9psOoT5IY+Ux1W/q9iRThtl/smeNS2TlByksRzkynk+SSX3Fele9Y9xV7FvkYMBF8MGCy2M4oQmdyjyfKsH+yLRtswE6VPIR2jAp2vpk8sxV8mcodi6WwyrHc+T0n3rtRw2AMomMBO9yfVYsHGzAWcTKxgWPHTW5FLuublY5wx6LT/BiAwWGZYjfGTg1oyI1iu0XyL6DBN0mRbJLo8qw7U9kherDDZADZzfnfO8wM7DieLz28+670G+ZS3aA6MZUZYJZEnIKdnUPbefd4sV0qOVXkHLFE2xkjxVHL2GBnO46N87ccn2xniC3HONhlUr5/KJCn8pz+YCNqsMy/HmxwtVjd+YON9xNdPCIzRvQZ9YjCfqLPcsgZYuyrXaWcR7FDpR98cvkYbtuo0SbuWPyRSR7Fh00Vc4A8JPLx7BKZVXg9ZynxbwnOdP6SfyeAa4vtqMinuC/OeJyUv2LI3x4XO7/xTojJPSEafMlz+VINnIf9LuasRMYI33KN6kyxnR07TH77c5ixbLPjsxmUHJLnKWKJLt9CUt8KRBRSA57PN18rFm1wKn8v40AOxeRwG/3nZ4b03+GqB1QXpHr9oS7f72wmNqb0C8si48c45psRIjV421x9yd4yVTlWL7CMWC4yHHDGxblYM3Ho3C2YOIxlXP6nK1XHDTU1HeFJH0tBTc2wQKLGdh/HIjcayta0pqampqampqampuY/5h/TQriDmt+c1wAAAABJRU5ErkJggg==>

[image6]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAGcAAAAXCAYAAAAWY1E4AAADBElEQVR4Xu2YWahNURjHP7OEkCkUD+byYIgHcmROCmUqXWR4MT4oMmbIg2RKhjI8kAdDKRGSROaZDE8kkjHEExL/v2/ts9f57tE9+3ZO3X2tX/1qf9/a+9xz19prrW8dkUAgEAiklJE2EagajIYzbTKtZOBleAteg0dgU3gINoaL4Ef4Gy5wzySlD+xnkyWgJjwF69qGNLIE3oO9vVwP+Axe93JzRQenv5crlFrwHdxmG0rAJDjHJtPIYNEO72YbwF64yYuPwg+iHZ2UAaJ/Z6xtKDKcNedgHduQRnaLdlq+JWArHOqua8NP8GDcnIil8CdsYhuKTBmcZpNpZYvo4HCQuou+efkYKHrfFNsg2uEb4FX4FD6SuFLaIfomf4Nv3PVh11YZWot+jyQO//tkCukK30v8j3yGp6V8GbpGtL2ZyfeCz+FqiQd2rehgtHRxA/gDrndxqVgIp9tk2mE1xr2AnXpHdBC+wvbePVfgJS8mbeErKb/U9RT9jBkuHuLiEdk7lHqiM5azax/slNuciEbwhujyWy3wOz+Cb/92ye1cLlu/4KroJgcHk/dlTH6My892MWcdZ07D7B3KcrjCXTeHD6RyxQZZJtXoXMMO51kgH1FlxZKUTHaxLaF5JuLyZd9WzgTe38XFF+GFuDnLbdHDYgRL975enI/K7DlRUZMaxsObNunYCZ/A+i7eI1qp8a3mG9rC5XlY5VLiw6WO+9YJF3O2cNZEs64DPOCuv0huxz2EE724UNbBqTaZZnaJds4sL8c9gEvQW9jRy3OGUe5NrMgi5otWYFEZzqXpPHwM27kc9xG+vRNEi4mzsLNr+y66H0XwIOx/n0LgZ96Xf1eZqYSd2AYuFt3QT4pu+JtFKzifQfCM6CyxS8Q8eAweFz2kcp+xZ6b98CW8K/oTTsRrOMyLOXM4iEnYKMmfCRQA96JxXvxCcgevIlqJLq2BEsD9a6W75kbPJbJG3FwhnOWjbDJQHLjH8VdvFiCs5liCFwqLEu5fgSoIz0gZmwwEAv8jfwACAaTypY7oWwAAAABJRU5ErkJggg==>

[image7]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAkAAAAWCAYAAAASEbZeAAAAzklEQVR4XtWQMQtBYRiFX4NilcFiFjFhMEjZmNitSnZloAxWWRgooSwGv8FksNqw2Cz+A+ftnlffvX9ATj3Dc77T7faK/CwJsAJHcAZt/7NIHFxAl54EdzCygWYi3shNB7xA2IoH2JswVfAGZRX9F5WFu0Dy7AcqBcrMXSA59muVCiU4yrDfqZQowVGa/VYlRZm7CyTLfqqiN1LRQ7opsh9acQOH77OXmnijuhVjcAUhK5AeeIpzzAg4gQY9Kt7XWzawxEAfbMASNH2v/5gPBMsqbStVx94AAAAASUVORK5CYII=>

[image8]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAkAAAAWCAYAAAASEbZeAAAAY0lEQVR4XmNgGHDACMSK6IIwIAXEAUC8G4jXo8mBQR4QXwLiKUD8iwGHImTwlWEIK9qALogOiFa0EV0QHYAUbUIXRAbsQPwdiPcyQOIQBTgB8QkGiIL/UHyXARKPPEjqhhYAAL9fG55PO7fqAAAAAElFTkSuQmCC>

[image9]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAKYAAAAXCAYAAACBHjSnAAAGSElEQVR4Xu2ZB6gdRRSGjz32FoPYEnsUxR4VW4wRrGgUUUGwY++9xojYsBF7TWILBgv2BsYSe++oSIK9YYmgqIie752dvLnn7u6bvZt3n8h+cHj3/jP3vN2ZszNnzoo0NDQ0NPy/GKe2kxcbGvqT0WrTvRgxRu1qL0YsqraEF0uYW+1ctafUXle7RG3+lh5p1PGztNrNalPVXlY7uLU5mTp+NlW7V+0ZtafVtmxprc4KanN5sYQ649fvzKP2ltrnviFjuNpHaoOcPofaqmIT8YHa8a3NpUxWu11sYPD7gNjgVKVTP4PV3lE7PPu+vNonaueEDonU8bOJ2pdqI7LvLA6/q42a1SONedXWE/ufP6ht2NJaTqfj1xWOU/tDigNzitr5XlTuUpumdpvaP5IemLuI9R8SaWtm2saR1hd1/FwqFlAxh4pNLA9qKnX8vKo23ml3iAVHFZi3R9SeFLv31MCsM379zlJiA/Sw2meuDRZT+01sJSiCJ79KYE5S+9ZpPLF/q53u9DLq+Jkh9mDFsFJxH1s4vYwZ0pkfdhr6HOn0sWozxe6jKidLtcCsM379DnnjtmoPSv6KyTb9thcdVQOTFeZDLyo/qT3hxRI69UNOyPVe7/QNMv1MpxdRx8+eYn32cTq7F/pWTk+hamB2On79Dss2iTcUBeaN0r4ieKoG5s/Svv3BN2qferGETv0wcVzvVU5fO9MnOL2IOn4YK/oQoDFHZPoBTk+hamAmjx/7+q1qL6ltlGkhmT4mdMogfyGXIR95Vu0FsSc1hm34MrX3xBLaiWqbRe2Pqa2SfS4KzNfE8qgyqgYmffMGhIPAF14soVM/rEZ5ARXyKw4DKdTxw2qaF5jMKfohTk+hamAmjd9yYsnrgmqviB0ogBzwK7FSQmCo2PZ6jdqcmbarWOIcWFFsmebUtaRYKYd/RjDCjmJBGygKzO/UzvCio2pgkrPmDQjXN92LJXTqhxJNXkCtkenkXinU8XOK5AcmAYl+oNNTqBqYSeNHsjlabWVpn+TD1G7IPpOcsgK+KL1BuZpYrrhz9p0yDqsu/SglAIHJAWdvsTrXNLXFszYoCsxfxLaXMkJgnuAbCuCQ9a4Xla/FammpdOpndbHr9XXZtTL9cqcXUccPuTt99nI6ZSf03ZyeQgjMsNv2RaXxO0/M+bKRtq/YEg+sjLS/oXaW2hVi9Su2j8AeWR8uNI+jpD3YigKTFZP+ZVQNTK6d9MTDQ8CukUqnfgaLXS9F8RgmFJ1xTaGOn93F+uzndMYQfaTTU6gamJXGjwie6jRqVGz1cJr0/c/JCelD0TUPgpD2Ips4q6elA6lb+Ym+oYCbxE5+MQuI+bjQ6WXU8fOx2j1O217stzs4vYxO/QwT63Os0y8QK7JzPqhKCMwRvqGA5PEjv0QcG2lsFxOi75zW6DM00oDte/PsM07ps3Bvcw9xHw9PT96K+bzaxV50hMA8yTeIvd4iLYl3AMpT/h44+KGtH2mkGvtL8Su2VD+8pqMMM1+ksTPxNosxCXD9bGNxYXy49KZIeaT6oaZJGhXDa0iqHjEcZu+OvnPvlJQ4a/RFCMy84nideeiBo3rIWZiYKdnfABc4U1rfxy4i9tSGYi2r6V/S+sQOU7tf2rfwAPkog+m5RVoHKo+txW5mnG+Q3sHyqwrvZOPDF4c9Xw/kVMtv/aoSk+In7BCxn0FiDx2pETBxrH6+rsgJNXeiMlL8ELS/SrsfDqiUZZbJvpOO/Si2GAXCYciPXx5ni/XdxjdIvXnogddE34sdXq5VW7e1uQeePG70TbFC6H3SXipiYHj//bhYvkAetE5LD4NteobYRWN8Jl0IsGJRFciDiX5f7E1B+D3lpeuiPmxrtDNoMRzcGHROtGwpDFw4zAVYjfjtnU6PSfHD61T8+AnjQHiqWOrC4XJMS6vBqvan9L4LzyPFz3Nib1noG8OKzAM9WWy+/RxtJ3btebtZYKK0ziHXy3XH11xnHv6TrCR2s6l5y+yGA8YkL3YZDpOdlG9mJ496ocFW3Su92CXYQQ7yYpfhMElJbqBglWWLbXBQlyP3IZ/qJuRmpCpDfEMX4bDwkBe7DA/GSC82GJSCLvJiPzNKLMcaSI6W3lLdQLCQlNdDG8TKUCTRDQ0NDQPHv/H93WxOaY1DAAAAAElFTkSuQmCC>

[image10]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABoAAAAZCAYAAAAv3j5gAAABmklEQVR4Xu2UvSuGYRSHj6LYJIOUYpCPSPmKwUeyMPkYUAaRCIOPxEAoZZISCQlFUhj8A0gG2WxIWSQlA4OR33nPofOcnt7J+F51Dfd1n7qfp+e9X6IY/0Aa3IJn8Br2BrejUglP4AU8h9WBXUMqvIUDus6AD3D2dyAKFfAZluu6Hn7Bur8JwyLJQZZ++AYTXPfcwGXX9uGpaxGe4KFr/ETfsMp1SzbJzJDrM/ADxtvI34aH120EJdqnXLe0kcx0uj6ivcbGUo0rNoJC7duuW0ZJZvhAy6D2bhv51LCD8rXvuW7htw07iL8v9z4b+acZdlCe9l3XLRMUfhAfwL3HxhyNqzaCAu1Lrlv4rvFMu+t8Tbi32Mh3iCNfVkuZ9mnXLa0kM12uj2mvdZ3u4bFrDSTDja5bMklmhl1fILm0ya7TPLyDcaaNwxcKXli+Ux1mzfDfzqZrfFmPXIuQCK9gk66TSN7S3g9+iE+SNyg2PQs+wnRd86/1neTbh5ICJ+EO3IDNgV3hEr6SzFpy4Rw8gGuwKLgdI0aM/+AH681ZX+W01h4AAAAASUVORK5CYII=>