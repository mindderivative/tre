//! Compiles the Phase 0 placeholder shaders to SPIR-V at build time
//! (TECHNICAL.md Section 9.3's "Build Integration" bullet: shader
//! compilation happens in a Cargo build script, never at runtime in a
//! shipping build). This uses GLSL + `glslc`, not the documented
//! HLSL + DXC pipeline -- that toolchain is IMPLEMENTATION.md Phase 3.2's
//! concern (the real SDF rounded-rect shader); this placeholder flat-color
//! shader exists only to validate the Canvas -> IR -> RHI pipeline shape
//! Phase 0 is scoped to, so standing up the full HLSL/DXC/SPIRV-Cross
//! cross-compilation build step here would be scope creep ahead of need.

use std::path::Path;
use std::process::Command;

fn compile_shader(src: &str, out_name: &str, out_dir: &str) {
    println!("cargo:rerun-if-changed={src}");
    let out_path = Path::new(out_dir).join(out_name);
    let status = Command::new("glslc")
        // IMPLEMENTATION.md Step 2.1's bindless shaders use
        // `GL_EXT_nonuniform_qualifier` and an unbounded (runtime-sized)
        // descriptor array, which need SPIR-V's descriptor-indexing
        // capabilities -- not available under glslc's default `vulkan1.0`
        // target environment. Harmless for the older flat-color shaders,
        // which don't use anything version-gated.
        .arg("--target-env=vulkan1.2")
        .arg(src)
        .arg("-o")
        .arg(&out_path)
        .status()
        .unwrap_or_else(|e| panic!("failed to run glslc (is it installed?): {e}"));
    assert!(status.success(), "glslc failed to compile {src}");
}

fn main() {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set by Cargo");
    compile_shader(
        "shaders/walking_skeleton.vert",
        "walking_skeleton.vert.spv",
        &out_dir,
    );
    compile_shader(
        "shaders/walking_skeleton.frag",
        "walking_skeleton.frag.spv",
        &out_dir,
    );
    compile_shader(
        "shaders/bindless_textured.vert",
        "bindless_textured.vert.spv",
        &out_dir,
    );
    compile_shader(
        "shaders/bindless_textured.frag",
        "bindless_textured.frag.spv",
        &out_dir,
    );
    compile_shader(
        "shaders/sdf_rounded_rect.vert",
        "sdf_rounded_rect.vert.spv",
        &out_dir,
    );
    compile_shader(
        "shaders/sdf_rounded_rect.frag",
        "sdf_rounded_rect.frag.spv",
        &out_dir,
    );
    // Phase 4 Step 4.2.3: the real MSDF evaluation shader. Deliberately
    // paired at pipeline-creation time with `bindless_textured.vert`, not
    // a new vertex shader -- its inputs/outputs are already exactly what
    // MSDF sampling needs, and this build script already compiles every
    // shader file independently rather than as fixed vert/frag pairs.
    compile_shader("shaders/msdf.frag", "msdf.frag.spv", &out_dir);
}
