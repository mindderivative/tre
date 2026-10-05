//! 0.5.4 (#108): one frame of a tree, rendered offscreen and read back.
//!
//! The same `WindowRenderer` sequence a window runs (`prepare`, then `draw`),
//! into a texture nothing presents, then copied to the CPU. It needs a GPU
//! device and no window, so tests and tools can read what a tree draws.

use engine_core::{NodeId, Tree};

use crate::WindowRenderer;

/// The format snapshots are rendered in: straight-alpha RGBA, 8 bits a channel.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// A rendered frame: `width * height` pixels, row by row, four bytes
/// (R, G, B, A) each, with no row padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Renders `tree` (already laid out) as `root`'s window of `width` x `height`
/// physical pixels at display `scale`, with shaders seeing `time` seconds.
/// Errors, rather than panics, on a size the renderer or the device can't do.
#[allow(clippy::too_many_arguments)]
pub fn snapshot(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    tree: &Tree,
    root: NodeId,
    width: u32,
    height: u32,
    scale: f64,
    time: f32,
) -> Result<Snapshot, String> {
    snapshot_with(device, queue, tree, root, width, height, scale, time, false)
}

/// `snapshot`, drawing text from the glyph cache when `glyph_cache` (0.5.4,
/// #127), as a window that has it on does.
#[allow(clippy::too_many_arguments)]
pub fn snapshot_with(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    tree: &Tree,
    root: NodeId,
    width: u32,
    height: u32,
    scale: f64,
    time: f32,
    glyph_cache: bool,
) -> Result<Snapshot, String> {
    let limit = device
        .limits()
        .max_texture_dimension_2d
        .min(u32::from(u16::MAX));
    if width == 0 || height == 0 || width > limit || height > limit {
        return Err(format!(
            "a snapshot of {width} x {height} pixels is outside what this GPU can render \
             (1 to {limit} on each side)"
        ));
    }
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("snapshot target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut renderer = WindowRenderer::new(device, FORMAT, width, height, true);
    renderer.set_scale(scale);
    renderer.set_time(time);
    renderer.set_glyph_cache(glyph_cache);
    renderer.text().sync_registered_fonts();
    let (w, h) = (width as u16, height as u16);
    let damage = renderer.prepare(tree, root, w, h, true, device, queue);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("snapshot"),
    });
    renderer.draw(
        tree,
        root,
        w,
        h,
        &damage,
        device,
        queue,
        &mut encoder,
        &texture,
        &view,
    );
    queue.submit([encoder.finish()]);
    read_back(device, queue, &texture)
}

/// Premultiplied RGBA8 to straight alpha, in place: each colour channel divided
/// by alpha, rounded, for pixels that are neither opaque nor clear.
fn unpremultiply(rgba: &mut [u8]) {
    for px in rgba.as_chunks_mut::<4>().0 {
        let a = u32::from(px[3]);
        if a == 0 || a == 255 {
            continue;
        }
        for c in &mut px[..3] {
            *c = ((u32::from(*c) * 255 + a / 2) / a).min(255) as u8;
        }
    }
}

/// `texture`'s pixels, rows unpadded.
fn read_back(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
) -> Result<Snapshot, String> {
    let (width, height) = (texture.width(), texture.height());
    let row = (width * 4) as usize;
    let padded = row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("snapshot readback"),
        size: (padded * height as usize) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("snapshot copy"),
    });
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded as u32),
                rows_per_image: None,
            },
        },
        texture.size(),
    );
    queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|err| format!("the GPU didn't finish the snapshot: {err}"))?;
    receiver
        .recv()
        .map_err(|_| "the snapshot readback never completed".to_string())?
        .map_err(|err| format!("couldn't read the snapshot back from the GPU: {err}"))?;
    let data = slice
        .get_mapped_range()
        .map_err(|err| format!("couldn't map the snapshot: {err}"))?;
    let mut rgba = Vec::with_capacity(row * height as usize);
    for y in 0..height as usize {
        rgba.extend_from_slice(&data[y * padded..y * padded + row]);
    }
    // The renderer writes premultiplied alpha; a snapshot is straight alpha, as
    // an image file and every decoder expect.
    unpremultiply(&mut rgba);
    Ok(Snapshot {
        width,
        height,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::unpremultiply;

    #[test]
    fn opaque_and_clear_pixels_are_untouched_and_translucent_ones_are_divided() {
        let mut px = vec![
            10, 20, 30, 255, // opaque
            0, 0, 0, 0, // clear
            128, 0, 0, 128, // half red, premultiplied
            50, 50, 50, 200, // dim
            1, 0, 0, 1, // alpha 1: a rounding extreme
        ];
        unpremultiply(&mut px);
        assert_eq!(&px[0..4], &[10, 20, 30, 255]);
        assert_eq!(&px[4..8], &[0, 0, 0, 0]);
        assert_eq!(&px[8..12], &[255, 0, 0, 128]);
        assert_eq!(&px[12..16], &[64, 64, 64, 200]);
        assert_eq!(&px[16..20], &[255, 0, 0, 1]);
    }
}
