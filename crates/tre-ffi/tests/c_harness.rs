//! Compiles and runs `tests/c/harness.c` -- the real, dedicated
//! non-Python C test harness (IMPLEMENTATION.md Phase 10 Step 10.3 task
//! 4) -- against this crate's own just-built `cdylib`, satisfying
//! TECHNICAL.md Section 9.4.1's literal "`cargo test`'s FFI test target"
//! requirement rather than a separate, CI-only shell step.
//!
//! Links against `libtre_ffi.so` (the `cdylib` target), not
//! `libtre_ffi.a` (the `staticlib` target), even though this crate
//! builds both (confirmed separately: both artifacts exist after `cargo
//! build -p tre-ffi`). The `.a` archive alone is not enough to link a
//! plain `cc` invocation against directly -- `tre-platform`'s real GTK/
//! GLib/GDK/GIO system-library dependencies (`tray-icon`/`rfd`) are only
//! resolved as `cargo:rustc-link-lib` directives Cargo itself feeds to
//! `rustc` when producing a *final* linked artifact, and replicating
//! that exact transitive library list by hand (confirmed empirically:
//! linking the `.a` directly produces dozens of undefined `g_*`/`gdk_*`
//! symbol errors) is real, unnecessary work `cdylib` sidesteps entirely
//! -- `libtre_ffi.so` is already a fully self-contained shared object
//! with every one of those dependencies linked in.

use std::path::PathBuf;

/// Cargo places a crate's own `staticlib`/`cdylib` output directly in
/// `target/<profile>/` (unlike the hashed `rlib`s under `target/
/// <profile>/deps/`, where a `cargo test` binary itself lives) -- so
/// this test's own binary path, two directories up, is where
/// `libtre_ffi.so` sits, since both are produced by the same `cargo
/// test` invocation.
fn target_profile_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("a running test binary has its own path");
    exe.parent()
        .and_then(std::path::Path::parent)
        .expect("test binaries live two directories under target/<profile>/")
        .to_path_buf()
}

#[test]
fn c_harness_exercises_the_real_ffi_surface() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let profile_dir = target_profile_dir();

    // Both real build-target artifacts (Step 10.3 task 3) must exist --
    // asserted here even though only the cdylib is actually linked
    // against below, so a regression that silently drops "staticlib"
    // from `[lib] crate-type` still fails this test.
    let staticlib = profile_dir.join("libtre_ffi.a");
    assert!(
        staticlib.is_file(),
        "expected {} to exist -- tre-ffi's [lib] crate-type must still include \"staticlib\"",
        staticlib.display()
    );
    let cdylib = profile_dir.join("libtre_ffi.so");
    assert!(
        cdylib.is_file(),
        "expected {} to exist -- tre-ffi's [lib] crate-type must still include \"cdylib\"",
        cdylib.display()
    );

    let harness_c = manifest_dir.join("tests/c/harness.c");
    let include_dir = manifest_dir.join("include");
    let exe_path = profile_dir.join("tre_ffi_c_harness");

    // Plain `std::process::Command`, not the `cc` crate's `Build`: that
    // crate's compiler-detection logic reads Cargo-set environment
    // variables (`OPT_LEVEL`, `TARGET`, `HOST`, ...) that only exist
    // inside a `build.rs` invocation, not a regular `cargo test` binary
    // -- confirmed by actually running this the `cc`-crate way first and
    // hitting exactly that missing-`OPT_LEVEL` error. Respecting `$CC`
    // (the same override a real C build would) is enough here.
    let compiler = std::env::var("CC").unwrap_or_else(|_| "cc".to_string());
    let mut cmd = std::process::Command::new(compiler);
    cmd.arg(&harness_c)
        .arg("-I")
        .arg(&include_dir)
        .arg("-o")
        .arg(&exe_path)
        .arg("-L")
        .arg(&profile_dir)
        .arg("-ltre_ffi")
        // Embeds the profile dir as an rpath so the resulting executable
        // finds `libtre_ffi.so` at run time without needing
        // `LD_LIBRARY_PATH` set.
        .arg(format!("-Wl,-rpath,{}", profile_dir.display()));

    let compile_status = cmd
        .status()
        .expect("failed to invoke the system C compiler -- is one installed?");
    assert!(
        compile_status.success(),
        "compiling/linking tests/c/harness.c against libtre_ffi.so failed"
    );

    let run_status = std::process::Command::new(&exe_path)
        .status()
        .unwrap_or_else(|e| {
            panic!(
                "failed to run the compiled C harness at {}: {e}",
                exe_path.display()
            )
        });
    assert!(
        run_status.success(),
        "the C harness itself reported a failure -- see its stderr output above"
    );
}
