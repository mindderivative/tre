//! Generates `include/tre_ffi.h` from this crate's own `extern "C"`
//! items on every build, via `cbindgen` -- IMPLEMENTATION.md Phase 10
//! Step 10.3 task 4's "auditable in a single place" rationale depends on
//! the header never drifting from the real signatures by hand. Checked
//! into the repo (so a consumer has it without running a build first)
//! but always regenerated here, never hand-edited.

fn main() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=cbindgen.toml");

    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").expect("set by Cargo");
    let config = cbindgen::Config::from_root_or_default(&crate_dir);
    match cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(config)
        .generate()
    {
        Ok(bindings) => {
            bindings.write_to_file(format!("{crate_dir}/include/tre_ffi.h"));
        }
        Err(err) => {
            // A parse/config error here means the crate's own `extern
            // "C"` surface (or `cbindgen.toml`) is malformed -- fail the
            // build loudly rather than shipping a stale or partial
            // header silently.
            panic!("cbindgen failed to generate include/tre_ffi.h: {err}");
        }
    }
}
