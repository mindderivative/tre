//! Phase 4 Step 4.2.2 proof: a real MSDF (Multi-channel Signed Distance
//! Field) generated via `fdsm` from a real glyph's real outline -- and,
//! deliberately, a glyph with a true hole (`'O'`), the exact case
//! Step 4.1's own ear-clipping-based rendering explicitly couldn't
//! handle. No GPU, no shader, no Vulkan at all this sub-step (Step
//! 4.2.3's job) -- verified entirely on the CPU: an independently
//! computed median-of-3 scan proves the hole is real (two separate
//! inside-regions along one scanline, impossible for a solid shape), and
//! `fdsm`'s own CPU-side preview renderer produces a human-viewable PNG.

use fdsm::render::render_msdf;
use image::{GrayImage, Rgb, RgbImage};
use skrifa::MetadataProvider;

const SIZE: u32 = 32;
const RANGE_PX: f64 = 4.0;
const PREVIEW_SCALE: u32 = 8;

/// Standard median-of-3 evaluation of a stored MSDF pixel -- computed
/// independently here (`fdsm`'s own `median`/`median3` are private to
/// that crate), matching TECHNICAL.md Section 5.3's canonical formula's
/// own first step, not reusing the library's internal implementation of
/// the very thing this demo is trying to verify.
fn median_at(bitmap: &tre_text::MsdfBitmap, x: u32, y: u32) -> u8 {
    let idx = ((y * bitmap.width + x) * 3) as usize;
    let mut channels = [
        bitmap.pixels[idx],
        bitmap.pixels[idx + 1],
        bitmap.pixels[idx + 2],
    ];
    channels.sort_unstable();
    channels[1]
}

fn main() {
    let cascade = tre_text::FontCascade::discover().expect("fontconfig cascade discovery failed");
    let font_bytes =
        std::fs::read(&cascade.entries[0]).expect("failed to read the primary cascade font");
    let font = skrifa::FontRef::new(&font_bytes).expect("primary cascade font invalid for skrifa");
    let glyph_id = font
        .charmap()
        .map('O')
        .expect("the primary cascade font must cover 'O'");

    let contours = tre_text::glyph_outline(&font, glyph_id).expect("outline extraction failed");
    assert_eq!(
        contours.len(),
        2,
        "'O' must be exactly two contours (an outer boundary and an inner hole) -- got {}",
        contours.len()
    );
    eprintln!("'O' extracted as {} contours", contours.len());

    let bitmap = tre_text::generate_msdf(&contours, SIZE, RANGE_PX);
    assert_eq!(bitmap.pixels.len(), (SIZE * SIZE * 3) as usize);

    // Scan the vertical-center row and classify each pixel as inside
    // (median > 127) or outside (median <= 127) -- computed directly
    // against the raw MSDF bytes, independent of `fdsm`'s own preview
    // renderer below. A solid shape crossed by this scanline produces
    // exactly one outside->inside transition (entering) and one
    // inside->outside transition (leaving): two transitions total. A
    // genuine ring -- outside, then the left wall, then the hole, then
    // the right wall, then outside again -- produces two *separate*
    // outside->inside (rising) transitions, something no solid shape can
    // produce. This is the real, self-verifying proof that the hole
    // survived MSDF generation, not a hardcoded pixel-coordinate guess.
    let center_row = SIZE / 2;
    let classifications: Vec<bool> = (0..SIZE)
        .map(|x| median_at(&bitmap, x, center_row) > 127)
        .collect();
    let rising_edges = classifications
        .windows(2)
        .filter(|pair| !pair[0] && pair[1])
        .count();
    eprintln!(
        "center-row inside/outside sequence: {}",
        classifications
            .iter()
            .map(|&inside| if inside { '#' } else { '.' })
            .collect::<String>()
    );
    assert_eq!(
        rising_edges, 2,
        "a genuine ring crossed through its own vertical center must show exactly 2 \
         outside->inside transitions (the left and right walls) -- a solid shape (no hole) or a \
         failed/collapsed hole would show only 1"
    );
    eprintln!("hole verification: OK (2 separate inside-regions found along the center scanline)");

    // Also confirm the raw MSDF buffer round-trips into a real
    // `image::RgbImage` cleanly (the exact type `fdsm::render::render_msdf`
    // below, and Step 4.2.3's eventual texture upload, both need).
    let msdf_image = RgbImage::from_raw(bitmap.width, bitmap.height, bitmap.pixels.clone())
        .expect("MsdfBitmap's own width/height/pixels must be internally consistent");

    let preview_size = SIZE * PREVIEW_SCALE;
    let mut preview = GrayImage::new(preview_size, preview_size);
    render_msdf(&msdf_image, &mut preview, RANGE_PX);

    let out_path = std::env::var("TRE_MSDF_GENERATION_OUTPUT")
        .unwrap_or_else(|_| "msdf_generation_output.png".to_string());
    write_gray_png(&preview, preview_size, &out_path);
    eprintln!("wrote {preview_size}x{preview_size} MSDF preview render to {out_path}");

    // A second, tiny raw-channel dump of the unscaled 32x32 MSDF itself,
    // upscaled with nearest-neighbor (not `render_msdf`'s own bilinear
    // preview) so the actual stored RGB channel values -- not their
    // rendered coverage -- can be inspected directly.
    let raw_out_path =
        std::env::var("TRE_MSDF_RAW_OUTPUT").unwrap_or_else(|_| "msdf_raw_output.png".to_string());
    let mut raw_upscaled = RgbImage::new(preview_size, preview_size);
    for (x, y, pixel) in raw_upscaled.enumerate_pixels_mut() {
        let sample: Rgb<u8> = msdf_image
            .get_pixel(x / PREVIEW_SCALE, y / PREVIEW_SCALE)
            .to_owned();
        *pixel = sample;
    }
    write_rgb_png(&raw_upscaled, preview_size, &raw_out_path);
    eprintln!("wrote {preview_size}x{preview_size} raw MSDF channel dump to {raw_out_path}");

    eprintln!("all MSDF generation assertions passed");
}

fn write_gray_png(image: &GrayImage, size: u32, out_path: &str) {
    let file = std::fs::File::create(out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size, size);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(image.as_raw())
        .expect("failed to write PNG image data");
}

fn write_rgb_png(image: &RgbImage, size: u32, out_path: &str) {
    let file = std::fs::File::create(out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size, size);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(image.as_raw())
        .expect("failed to write PNG image data");
}
