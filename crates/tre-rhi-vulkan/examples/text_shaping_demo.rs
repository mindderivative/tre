//! Phase 4 Step 4.1 proof: real bidi + script run segmentation and
//! shaping (`rustybuzz`), a real `fontconfig`-driven font fallback
//! cascade, and real glyph outline extraction (`skrifa`) -- all against
//! whatever fonts are actually installed on this machine, not
//! hand-authored test data. No MSDF, no atlas, no new shader: outline
//! extraction is verified by flattening a real glyph's outline (via
//! `tre-svg`'s now-`pub` curve flatteners) and rendering it through the
//! pre-existing ear-clipping + flat-color pipeline, same as
//! `svg_tessellation_demo`.

use ash::vk;
use rustybuzz::{Direction, Face};
use skrifa::MetadataProvider;
use tre_engine::{rgba8, RhiDevice};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};
use tre_svg::Polygon;
use tre_text::OutlineSegment;

const WIDTH: u32 = 300;
const HEIGHT: u32 = 300;

/// Standard even-odd ray-casting point-in-polygon test, computed directly
/// against the real flattened glyph outline this demo just extracted --
/// this step's version of the "independently verify the expected pixel
/// outcome before trusting the GPU render" methodology every prior step
/// used (a hand-computed Python script for a hand-authored shape). A real
/// font's glyph shape isn't known in advance the way a hand-authored SVG
/// is, so the independent ground truth is computed here, in Rust, against
/// the same point list the renderer consumes, rather than pre-verified
/// externally.
fn point_in_polygon(point: [f32; 2], points: &[[f32; 2]]) -> bool {
    let mut inside = false;
    let n = points.len();
    for i in 0..n {
        let a = points[i];
        let b = points[(i + 1) % n];
        if (a[1] > point[1]) != (b[1] > point[1])
            && point[0] < (b[0] - a[0]) * (point[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}

/// Flattens a `tre_text::Contour` (already-decoded `skrifa` outline
/// commands) into a flat polyline, reusing `tre-svg`'s cubic/quadratic
/// Bezier flatteners (Phase 3 Step 3.3.1) rather than a second hand-rolled
/// flattener -- a glyph outline's curves are the same Bezier types an SVG
/// path's curves are.
fn flatten_contour(contour: &[OutlineSegment]) -> Vec<[f32; 2]> {
    let mut points: Vec<[f32; 2]> = Vec::new();
    for segment in contour {
        match *segment {
            OutlineSegment::MoveTo(p) | OutlineSegment::LineTo(p) => points.push(p),
            OutlineSegment::QuadTo { control, end } => {
                let p0 = *points.last().expect("QuadTo must follow a MoveTo");
                tre_svg::flatten_quad(p0, control, end, &mut points);
            }
            OutlineSegment::CubicTo {
                control1,
                control2,
                end,
            } => {
                let p0 = *points.last().expect("CubicTo must follow a MoveTo");
                tre_svg::flatten_cubic(p0, control1, control2, end, &mut points);
            }
            // `close()` signals "connect back to the start point" without
            // its own coordinates -- exactly `tre_svg::Polygon`'s own
            // convention (last point implicitly connects to the first,
            // never duplicated), so this needs no extra point.
            OutlineSegment::Close => {}
        }
    }
    points
}

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre text shaping probe (never shown)", 1, 1)
        .expect("failed to open probe window");
    use raw_window_handle::HasDisplayHandle;
    let display_handle = probe_connection.display_handle().unwrap().as_raw();
    let window_handle = probe_connection
        .window_handle(probe_window)
        .unwrap()
        .as_raw();
    let (device, surface_loader, surface) =
        VulkanDevice::new(display_handle, window_handle).expect("failed to create VulkanDevice");
    unsafe {
        surface_loader.destroy_surface(surface, None);
    }

    let swapchain =
        HeadlessSwapchain::new(&device, WIDTH, HEIGHT).expect("failed to create HeadlessSwapchain");

    let out_dir = env!("OUT_DIR");
    let vertex_spv = std::fs::read(format!("{out_dir}/walking_skeleton.vert.spv"))
        .expect("failed to read compiled vertex shader");
    let fragment_spv = std::fs::read(format!("{out_dir}/walking_skeleton.frag.spv"))
        .expect("failed to read compiled fragment shader");
    let pipeline = device
        .create_pipeline(&vertex_spv, &fragment_spv, tre_rhi_vulkan::HEADLESS_FORMAT)
        .expect("failed to create pipeline");

    // --- Task 2: real fontconfig-driven fallback cascade ---
    let cascade = tre_text::FontCascade::discover().expect("fontconfig cascade discovery failed");
    eprintln!("resolved font cascade: {:?}", cascade.entries);
    assert!(
        cascade.entries.len() >= 2,
        "this demo needs at least a primary and one distinct fallback font"
    );
    let font_bytes: Vec<Vec<u8>> = cascade
        .entries
        .iter()
        .map(|path| {
            std::fs::read(path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
        })
        .collect();
    let font_slices: Vec<&[u8]> = font_bytes.iter().map(Vec::as_slice).collect();
    let primary_face =
        Face::from_slice(&font_bytes[0], 0).expect("primary cascade font invalid for shaping");

    // --- Task 1: bidi + script segmentation and shaping ---
    // Latin "he" followed by two Hebrew letters (Alef, Bet) -- the same
    // real mixed-direction case `tre-text`'s own unit tests verify the
    // run *segmentation* boundaries for; this demo additionally shapes it
    // through a real font and checks the *glyph order* consequence of
    // that segmentation.
    let mixed_text = "he\u{5D0}\u{5D1}";
    let shaped_runs = tre_text::shape_text(&primary_face, mixed_text).expect("shape_text failed");
    assert_eq!(
        shaped_runs.len(),
        2,
        "expected exactly one Latin run and one Hebrew run: {shaped_runs:?}"
    );
    assert_eq!(shaped_runs[0].direction, Direction::LeftToRight);
    assert_eq!(shaped_runs[1].direction, Direction::RightToLeft);
    for glyph in &shaped_runs[0].glyphs {
        assert_ne!(
            glyph.glyph_id, 0,
            "the primary font must have real (non-notdef) glyphs for plain Latin text"
        );
    }
    // An RTL run's glyphs come back in *visual* order, i.e. the reverse of
    // logical reading order -- the Hebrew run's cluster values (byte
    // offsets into the original string) must therefore be strictly
    // descending, not ascending like the Latin run's.
    let hebrew_clusters: Vec<u32> = shaped_runs[1].glyphs.iter().map(|g| g.cluster).collect();
    let mut expected_order = hebrew_clusters.clone();
    expected_order.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(
        hebrew_clusters, expected_order,
        "an RTL run's shaped glyphs must be in descending (visually reversed) cluster order"
    );
    eprintln!(
        "bidi + script shaping: OK (2 runs, LTR then RTL, RTL glyphs in reversed cluster order)"
    );

    // --- Task 2 continued: real fallback resolution ---
    // U+1F9E0 (the "brain" emoji) -- confirmed via `fc-query`'s charset
    // dump to be absent from both DejaVu Sans and Noto Sans (this
    // project's two most likely `sans-serif` resolutions) and present in
    // Noto Color Emoji, the same codepoint `tre-text`'s own fallback unit
    // tests use, verified there against real installed font files rather
    // than assumed.
    let emoji_text = "\u{1F9E0}";
    let emoji_runs = tre_text::segment_runs(emoji_text);
    assert_eq!(emoji_runs.len(), 1);
    let (resolved_index, resolved_run) =
        tre_text::resolve_run(&font_slices, emoji_text, &emoji_runs[0])
            .expect("resolve_run failed");
    assert_eq!(
        resolved_index,
        font_slices.len() - 1,
        "an emoji codepoint absent from every non-emoji cascade entry must resolve to the last \
         (emoji) entry"
    );
    assert!(
        resolved_run.glyphs.iter().all(|g| g.glyph_id != 0),
        "the resolved fallback font must have a real (non-notdef) glyph for the emoji codepoint"
    );
    eprintln!("font fallback resolution: OK (resolved to cascade entry {resolved_index})");

    // --- Task 3: real glyph outline extraction, rendered as proof ---
    let skrifa_font =
        skrifa::FontRef::new(&font_bytes[0]).expect("primary cascade font invalid for skrifa");
    let glyph_id = skrifa_font
        .charmap()
        .map('L')
        .expect("the primary cascade font must cover 'L'");
    let contours =
        tre_text::glyph_outline(&skrifa_font, glyph_id).expect("outline extraction failed");
    assert_eq!(
        contours.len(),
        1,
        "'L' must be a single hole-free contour -- this demo deliberately avoids glyphs with \
         counters (e.g. 'O'), which need multi-contour winding this step doesn't build"
    );
    let raw_points = flatten_contour(&contours[0]);

    // `skrifa`'s unscaled outline space is Y-up (baseline at y=0,
    // ascenders positive) in font design units (`units_per_em` per em) --
    // this project's screen/canvas space is Y-down, matching every other
    // demo's SVG-style convention, so flip Y and scale/offset the glyph
    // into the canvas.
    let units_per_em =
        f32::from(u16::try_from(primary_face.units_per_em()).expect("units_per_em fits in u16"));
    let scale = 200.0 / units_per_em;
    const OFFSET_X: f32 = 60.0;
    const OFFSET_Y: f32 = 220.0;
    let screen_points: Vec<[f32; 2]> = raw_points
        .iter()
        .map(|&[x, y]| [x.mul_add(scale, OFFSET_X), OFFSET_Y - y * scale])
        .collect();
    let polygon = Polygon {
        points: screen_points,
    };

    let bbox = tre_svg::bounding_box(&polygon);
    // Inside the vertical stroke every 'L' has along its left edge.
    let inside_probe = [
        bbox.0[0] + 0.15 * (bbox.1[0] - bbox.0[0]),
        bbox.0[1] + 0.5 * (bbox.1[1] - bbox.0[1]),
    ];
    // The bounding box's own center: for an 'L', this falls in the empty
    // upper-right region the letterform doesn't cover.
    let outside_probe = [(bbox.0[0] + bbox.1[0]) / 2.0, (bbox.0[1] + bbox.1[1]) / 2.0];
    assert!(
        point_in_polygon(inside_probe, &polygon.points),
        "chosen 'inside' probe {inside_probe:?} is unexpectedly outside the extracted 'L' \
         outline -- the outline extraction or flattening is wrong"
    );
    assert!(
        !point_in_polygon(outside_probe, &polygon.points),
        "chosen 'outside' probe {outside_probe:?} (bbox center) is unexpectedly inside the \
         extracted 'L' outline -- the outline extraction or flattening is wrong"
    );

    let triangles = tre_svg::triangulate(&polygon)
        .expect("a real font's 'L' outline must be a simple (non-self-intersecting) polygon");
    let white = rgba8(255, 255, 255, 255);
    let (vertices, indices) = tre_svg::to_ui_vertices(&polygon, &triangles, white);

    let vertex_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&vertices),
            vk::BufferUsageFlags::VERTEX_BUFFER,
        )
        .expect("failed to upload vertex buffer");
    let index_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&indices),
            vk::BufferUsageFlags::INDEX_BUFFER,
        )
        .expect("failed to upload index buffer");

    let (mut cmd_buffer, image) = device.begin_frame(&swapchain).expect("begin_frame failed");
    cmd_buffer.set_pipeline(&pipeline);
    cmd_buffer.bind_vertex_buffer(&vertex_buffer, 0);
    cmd_buffer.bind_index_buffer(&index_buffer, 0);
    cmd_buffer.draw_indexed(indices.len() as u32, 0, 0);
    device
        .submit_and_present(cmd_buffer, &swapchain, image)
        .expect("submit_and_present failed");

    let bgra = swapchain
        .read_pixels_bgra8()
        .expect("failed to read back pixels");
    let pixel_at = |x: u32, y: u32| -> [u8; 4] {
        let idx = ((y * WIDTH + x) * 4) as usize;
        [bgra[idx], bgra[idx + 1], bgra[idx + 2], bgra[idx + 3]]
    };
    let background = pixel_at(0, 0);
    eprintln!("background (clear color): {background:?}");

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "both probes are computed from a 300x300 canvas's own polygon bounds"
    )]
    let inside_pixel = pixel_at(inside_probe[0] as u32, inside_probe[1] as u32);
    assert_eq!(
        inside_pixel,
        [255, 255, 255, 255],
        "the 'inside' probe must render as exactly the fill color, got {inside_pixel:?}"
    );
    eprintln!("glyph outline render, inside probe: OK ({inside_pixel:?})");

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "both probes are computed from a 300x300 canvas's own polygon bounds"
    )]
    let outside_pixel = pixel_at(outside_probe[0] as u32, outside_probe[1] as u32);
    assert_eq!(
        outside_pixel, background,
        "the 'outside' probe (bbox center) must render as exactly the background color, got \
         {outside_pixel:?}"
    );
    eprintln!("glyph outline render, outside probe: OK ({outside_pixel:?})");

    write_png(&bgra, "TRE_TEXT_SHAPING_OUTPUT", "text_shaping_output.png");

    // --- Bonus: a real shaped *word*, positioned by real shaping
    // advances -- a single letter proves outline extraction, but not that
    // shaping actually lays out a sequence of glyphs correctly. "TEXT" --
    // T, E, and X independently confirmed single-contour (hole-free)
    // glyphs in this machine's real primary cascade font via a standalone
    // probe before writing this section, the same discipline used to
    // pick 'L' above -- lets the whole word render through the same
    // hole-free-only pipeline this step's scope allows.
    let word = "TEXT";
    let word_runs = tre_text::shape_text(&primary_face, word).expect("shape_text failed for word");
    assert_eq!(
        word_runs.len(),
        1,
        "\"TEXT\" is pure Latin, must shape as a single run"
    );
    let word_glyphs = &word_runs[0].glyphs;
    assert_eq!(word_glyphs.len(), word.chars().count());
    for glyph in word_glyphs {
        assert_ne!(
            glyph.glyph_id, 0,
            "the primary font must have real (non-notdef) glyphs for \"TEXT\""
        );
    }

    const WORD_OFFSET_X: f32 = 20.0;
    const WORD_OFFSET_Y: f32 = 180.0;
    // A separate, smaller scale than the single-glyph 'L' proof above --
    // that scale (a ~200px-tall single letter) would run a whole 4-letter
    // word well past this 300px-wide canvas. Sized from the word's own
    // real total advance (not guessed), leaving margin on both sides.
    #[allow(
        clippy::cast_precision_loss,
        reason = "a 4-letter word's total advance is far below f32's exact-integer range"
    )]
    let total_advance_units: f32 = word_glyphs.iter().map(|g| g.x_advance as f32).sum();
    let word_scale = (WIDTH as f32 - 2.0 * WORD_OFFSET_X) / total_advance_units;
    let mut pen_x = WORD_OFFSET_X;
    let mut glyph_polygons: Vec<Polygon> = Vec::new();
    for glyph in word_glyphs {
        let glyph_id = skrifa::GlyphId::from(glyph.glyph_id);
        let contours = tre_text::glyph_outline(&skrifa_font, glyph_id)
            .expect("outline extraction failed for a word glyph");
        assert_eq!(
            contours.len(),
            1,
            "every letter in \"TEXT\" must be a hole-free single contour"
        );
        let raw = flatten_contour(&contours[0]);
        #[allow(
            clippy::cast_precision_loss,
            reason = "a 4-letter word's offsets/advances are far below f32's exact-integer range"
        )]
        let (x_off, y_off) = (
            glyph.x_offset as f32 * word_scale,
            glyph.y_offset as f32 * word_scale,
        );
        let glyph_origin_x = pen_x + x_off;
        let points: Vec<[f32; 2]> = raw
            .iter()
            .map(|&[x, y]| {
                [
                    x.mul_add(word_scale, glyph_origin_x),
                    WORD_OFFSET_Y - y.mul_add(word_scale, y_off),
                ]
            })
            .collect();
        glyph_polygons.push(Polygon { points });
        #[allow(
            clippy::cast_precision_loss,
            reason = "a 4-letter word's offsets/advances are far below f32's exact-integer range"
        )]
        let advance = glyph.x_advance as f32 * word_scale;
        pen_x += advance;
    }

    // Ground truth computed independently against the extracted-and-
    // positioned polygons themselves (the same even-odd ray-casting test
    // used above), *before* looking at any rendered pixel -- for each
    // letter, its own bounding-box center; between each adjacent pair of
    // letters, the horizontal midpoint of their gap (checked against
    // *every* letter's polygon, not just its neighbors, so an advance bug
    // that made two letters overlap would also be caught here).
    let mut probes: Vec<([f32; 2], bool)> = Vec::new();
    for polygon in &glyph_polygons {
        let bbox = tre_svg::bounding_box(polygon);
        let center = [(bbox.0[0] + bbox.1[0]) / 2.0, (bbox.0[1] + bbox.1[1]) / 2.0];
        let expected = point_in_polygon(center, &polygon.points);
        probes.push((center, expected));
    }
    for pair in glyph_polygons.windows(2) {
        let left_bbox = tre_svg::bounding_box(&pair[0]);
        let right_bbox = tre_svg::bounding_box(&pair[1]);
        let gap = [
            (left_bbox.1[0] + right_bbox.0[0]) / 2.0,
            (left_bbox.0[1] + left_bbox.1[1]) / 2.0,
        ];
        let inside_any = glyph_polygons
            .iter()
            .any(|polygon| point_in_polygon(gap, &polygon.points));
        assert!(
            !inside_any,
            "the gap between two adjacent letters in \"TEXT\" must not fall inside any letter's \
             own outline -- a real shaping-advance bug would show up here as overlapping glyphs"
        );
        probes.push((gap, false));
    }

    let mut word_vertex_buffers = Vec::new();
    let mut word_index_buffers = Vec::new();
    for polygon in &glyph_polygons {
        let triangles = tre_svg::triangulate(polygon)
            .expect("every letter in \"TEXT\" must be a simple (non-self-intersecting) polygon");
        let (vertices, indices) = tre_svg::to_ui_vertices(polygon, &triangles, white);
        word_vertex_buffers.push(
            device
                .upload_buffer(
                    bytemuck::cast_slice(&vertices),
                    vk::BufferUsageFlags::VERTEX_BUFFER,
                )
                .expect("failed to upload word vertex buffer"),
        );
        word_index_buffers.push((
            device
                .upload_buffer(
                    bytemuck::cast_slice(&indices),
                    vk::BufferUsageFlags::INDEX_BUFFER,
                )
                .expect("failed to upload word index buffer"),
            indices.len() as u32,
        ));
    }

    let (mut word_cmd_buffer, word_image) = device
        .begin_frame(&swapchain)
        .expect("begin_frame failed for word render");
    word_cmd_buffer.set_pipeline(&pipeline);
    for (vertex_buffer, (index_buffer, index_count)) in
        word_vertex_buffers.iter().zip(&word_index_buffers)
    {
        word_cmd_buffer.bind_vertex_buffer(vertex_buffer, 0);
        word_cmd_buffer.bind_index_buffer(index_buffer, 0);
        word_cmd_buffer.draw_indexed(*index_count, 0, 0);
    }
    device
        .submit_and_present(word_cmd_buffer, &swapchain, word_image)
        .expect("submit_and_present failed for word render");

    let word_bgra = swapchain
        .read_pixels_bgra8()
        .expect("failed to read back word render pixels");
    let word_pixel_at = |x: u32, y: u32| -> [u8; 4] {
        let idx = ((y * WIDTH + x) * 4) as usize;
        [
            word_bgra[idx],
            word_bgra[idx + 1],
            word_bgra[idx + 2],
            word_bgra[idx + 3],
        ]
    };
    for (probe, expected_inside) in probes {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "every probe is computed from this 300x300 canvas's own glyph placements"
        )]
        let pixel = word_pixel_at(probe[0] as u32, probe[1] as u32);
        let expected_pixel = if expected_inside {
            [255, 255, 255, 255]
        } else {
            background
        };
        assert_eq!(
            pixel,
            expected_pixel,
            "probe {probe:?} (independently computed as {}) rendered as {pixel:?}, expected \
             {expected_pixel:?}",
            if expected_inside { "inside" } else { "outside" }
        );
    }
    eprintln!(
        "shaped word render (\"TEXT\", {} glyphs, real advances): OK ({} probes matched)",
        word_glyphs.len(),
        glyph_polygons.len() + glyph_polygons.len().saturating_sub(1)
    );

    write_png(
        &word_bgra,
        "TRE_TEXT_SHAPING_WORD_OUTPUT",
        "text_shaping_word_output.png",
    );

    eprintln!("all text shaping assertions passed");
}

fn write_png(bgra: &[u8], env_var: &str, default_path: &str) {
    let mut rgba = bgra.to_vec();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var(env_var).unwrap_or_else(|_| default_path.to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba)
        .expect("failed to write PNG image data");
    eprintln!("wrote {WIDTH}x{HEIGHT} render to {out_path}");
}
