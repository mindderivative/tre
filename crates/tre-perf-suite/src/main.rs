//! Phase 20: Rendering Engine Performance & Optimization Test Suite --
//! a manually triggered (never CI-gated) tool that profiles `tre` under
//! simulated production load: a real, on-screen window, the `--release`
//! build's own real optimizations, and two test profiles (`ramp`,
//! `resize`) collecting FPS/CPU/memory/GPU telemetry to both the live
//! console and a structured `.jsonl` log file.
//!
//! # GPU utilization, disclosed
//!
//! There is no fully portable, dependency-free way to read a literal
//! "GPU utilization %" across arbitrary hardware and platforms.
//! Confirmed with the project owner before implementing
//! ([`telemetry::read_gpu_busy_percent`]): this tool reads the Linux
//! `amdgpu` kernel driver's own `gpu_busy_percent` sysfs file, matching
//! this project's actual real development GPU (RADV/AMD) and its own
//! established precedent for disclosed, platform-specific
//! implementations (TECHNICAL.md's DirectX 12/Metal backends are
//! "unimplemented, deferred" for the identical reason: build the real
//! thing for the real hardware in front of you, disclose the boundary,
//! don't fake portability). On any other GPU vendor or OS, this reports
//! "unavailable" -- never a fabricated number.
//!
//! # Usage
//!
//! ```text
//! cargo run -p tre-perf-suite --release -- --profile ramp \
//!     [--workload shapes|textures|shaders|all] [--duration 30] \
//!     [--tier-seconds 2] [--out path.jsonl]
//!
//! cargo run -p tre-perf-suite --release -- --profile resize [--out path.jsonl]
//! ```
//!
//! `--release` matters: the spec's "production configuration/
//! optimizations" requirement is satisfied by the release profile, the
//! same real-hardware verification standard already applied elsewhere
//! in this workspace.

mod ramp_test;
mod resize_test;
mod telemetry;
mod workload;

use ramp_test::RampConfig;
use workload::Workload;

const USAGE: &str = "\
tre-perf-suite -- Phase 20 rendering engine performance test suite

USAGE:
    tre-perf-suite --profile ramp [--workload shapes|textures|shaders|all] [--duration SECS] [--tier-seconds SECS] [--out PATH]
    tre-perf-suite --profile resize [--out PATH]

OPTIONS:
    --profile <ramp|resize>   Required. Which test to run.
    --workload <name>         ramp only. One of shapes/textures/shaders/all. Default: all.
    --duration <secs>         ramp only. Total seconds per workload. Default: 30.
    --tier-seconds <secs>     ramp only. Seconds between each doubling of primitive count. Default: 2.
    --out <path>              Structured JSON-Lines log path. Default: perf_results/<profile>_<workload>_<unix-ts>.jsonl
    --help                    Print this message.";

enum Command {
    Ramp(RampConfig),
    Resize(Option<std::path::PathBuf>),
}

fn parse_args(args: &[String]) -> Result<Command, String> {
    let mut profile: Option<String> = None;
    let mut workload: Option<String> = None;
    let mut duration_s: f64 = 30.0;
    let mut tier_seconds: f64 = 2.0;
    let mut out: Option<std::path::PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "--help" | "-h" => return Err(String::new()),
            "--profile" => {
                i += 1;
                profile = Some(
                    args.get(i)
                        .cloned()
                        .ok_or_else(|| "--profile requires a value".to_string())?,
                );
            }
            "--workload" => {
                i += 1;
                workload = Some(
                    args.get(i)
                        .cloned()
                        .ok_or_else(|| "--workload requires a value".to_string())?,
                );
            }
            "--duration" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| "--duration requires a value".to_string())?;
                duration_s = value.parse().map_err(|_| {
                    format!("--duration expects a number of seconds, got {value:?}")
                })?;
            }
            "--tier-seconds" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| "--tier-seconds requires a value".to_string())?;
                tier_seconds = value.parse().map_err(|_| {
                    format!("--tier-seconds expects a number of seconds, got {value:?}")
                })?;
            }
            "--out" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| "--out requires a value".to_string())?;
                out = Some(std::path::PathBuf::from(value));
            }
            other => return Err(format!("unrecognized argument: {other}")),
        }
        i += 1;
    }

    match profile.as_deref() {
        Some("ramp") => {
            let workloads = match workload.as_deref().unwrap_or("all") {
                "all" => Workload::ALL.to_vec(),
                name => vec![Workload::parse(name)
                    .ok_or_else(|| format!("unrecognized --workload: {name}"))?],
            };
            if duration_s <= 0.0 {
                return Err("--duration must be positive".to_string());
            }
            if tier_seconds <= 0.0 {
                return Err("--tier-seconds must be positive".to_string());
            }
            Ok(Command::Ramp(RampConfig {
                workloads,
                duration_s,
                tier_seconds,
                out,
            }))
        }
        Some("resize") => Ok(Command::Resize(out)),
        Some(other) => Err(format!(
            "unrecognized --profile: {other} (expected ramp or resize)"
        )),
        None => Err("--profile is required (ramp or resize)".to_string()),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse_args(&args) {
        Ok(Command::Ramp(config)) => ramp_test::run(config),
        Ok(Command::Resize(out)) => resize_test::run(out),
        Err(message) => {
            if !message.is_empty() {
                eprintln!("error: {message}\n");
            }
            eprintln!("{USAGE}");
            std::process::exit(1);
        }
    }
}
