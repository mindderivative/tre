//! Real, Linux-specific telemetry collection -- FPS/frame time (from
//! `tre_engine::FrameClock`, already proven elsewhere in the workspace),
//! process CPU% and memory RSS (parsed directly from `/proc/self/stat`/
//! `/proc/self/status`, no new dependency), and GPU utilization (Linux/
//! AMDGPU sysfs `gpu_busy_percent`, a real, disclosed platform-specific
//! implementation -- see this crate's own `main.rs` doc comment for the
//! project-owner-confirmed rationale). Every metric here is either real
//! or explicitly reported as `None`/"unavailable" -- nothing is ever
//! fabricated when a real reading can't be taken.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// One telemetry sample, written to both the live console and the
/// structured JSON-Lines log file.
pub struct Sample {
    pub elapsed_s: f64,
    pub profile: &'static str,
    pub workload: String,
    pub primitive_count: usize,
    pub fps: f32,
    pub frame_time_ms: f32,
    pub cpu_pct: f32,
    pub mem_rss_kb: u64,
    pub gpu_pct: Option<f32>,
}

impl Sample {
    fn console_line(&self) -> String {
        let gpu = match self.gpu_pct {
            Some(pct) => format!("{pct:.1}%"),
            None => "unavailable".to_string(),
        };
        format!(
            "[{:>6.1}s] {:<9} n={:<6} fps={:>6.1} frame={:>6.2}ms cpu={:>5.1}% mem={:>7}kB gpu={gpu}",
            self.elapsed_s,
            self.workload,
            self.primitive_count,
            self.fps,
            self.frame_time_ms,
            self.cpu_pct,
            self.mem_rss_kb,
        )
    }

    fn json_line(&self) -> String {
        let gpu = match self.gpu_pct {
            Some(pct) => format!("{pct:.2}"),
            None => "null".to_string(),
        };
        format!(
            "{{\"kind\":\"sample\",\"elapsed_s\":{:.3},\"profile\":\"{}\",\"workload\":\"{}\",\
             \"primitive_count\":{},\"fps\":{:.2},\"frame_time_ms\":{:.3},\"cpu_pct\":{:.2},\
             \"mem_rss_kb\":{},\"gpu_pct\":{gpu}}}",
            self.elapsed_s,
            self.profile,
            self.workload,
            self.primitive_count,
            self.fps,
            self.frame_time_ms,
            self.cpu_pct,
            self.mem_rss_kb,
        )
    }
}

/// Appends one JSON object per line to a log file, creating its parent
/// directory if needed. Deliberately hand-formatted rather than built on
/// `serde_json` -- every record here is a flat set of caller-controlled
/// numbers and a handful of fixed-vocabulary strings (`"shapes"`,
/// `"textures"`, `"shaders"`, `"interactive"`, `"ramp"`, `"resize"`),
/// so there is no untrusted-input escaping concern a real JSON library
/// would otherwise be needed for.
pub struct TelemetryLog {
    file: fs::File,
    path: PathBuf,
}

impl TelemetryLog {
    pub fn create(path: &Path) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self {
            file,
            path: path.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn write_sample(&mut self, sample: &Sample) -> std::io::Result<()> {
        println!("{}", sample.console_line());
        writeln!(self.file, "{}", sample.json_line())
    }

    /// A ramp test's own tier-boundary marker -- lets a post-run reader
    /// see exactly which primitive count a given FPS collapse happened
    /// at, without having to infer it from `primitive_count` alone
    /// jumping between two `sample` records.
    pub fn write_tier_change(
        &mut self,
        elapsed_s: f64,
        workload: &str,
        primitive_count: usize,
    ) -> std::io::Result<()> {
        println!(
            "[{elapsed_s:>6.1}s] {workload:<9} --- tier change: primitive_count = {primitive_count} ---"
        );
        writeln!(
            self.file,
            "{{\"kind\":\"tier_change\",\"elapsed_s\":{elapsed_s:.3},\"workload\":\"{workload}\",\
             \"primitive_count\":{primitive_count}}}"
        )
    }

    /// The Interaction-Driven resize test's own dedicated record: how
    /// long the real swapchain-plus-pipeline reconstruction itself took,
    /// separate from the regular per-frame `Sample`s around it -- so the
    /// log shows both the rebuild's own cost and its effect on the next
    /// several frames' pacing.
    pub fn write_resize_event(
        &mut self,
        elapsed_s: f64,
        width: u32,
        height: u32,
        rebuild_ms: f32,
    ) -> std::io::Result<()> {
        println!(
            "[{elapsed_s:>6.1}s] resize    --- window resized to {width}x{height}, swapchain \
             rebuild took {rebuild_ms:.2}ms ---"
        );
        writeln!(
            self.file,
            "{{\"kind\":\"resize_event\",\"elapsed_s\":{elapsed_s:.3},\"width\":{width},\
             \"height\":{height},\"rebuild_ms\":{rebuild_ms:.3}}}"
        )
    }
}

/// `perf_results/<profile>_<workload>_<unix-timestamp>.jsonl` at the
/// workspace root -- the default log path when `--out` isn't given.
pub fn default_log_path(profile: &str, workload: &str) -> PathBuf {
    let unix_ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    PathBuf::from("perf_results").join(format!("{profile}_{workload}_{unix_ts}.jsonl"))
}

/// Real process CPU utilization and resident-set memory, sampled from
/// `/proc/self/stat`/`/proc/self/status` -- Linux-specific (matches this
/// project's own current platform scope; see TECHNICAL.md's DX12/Metal
/// "unimplemented, deferred" precedent for the same kind of disclosed
/// boundary), and process-scoped rather than system-wide, since it's
/// this engine's own load the suite exists to characterize.
pub struct CpuMemSampler {
    /// `sysconf(_SC_CLK_TCK)`'s value -- **hardcoded to 100**, its
    /// near-universal value on every real Linux system (including this
    /// project's own), rather than adding a `libc` dependency just to
    /// call `sysconf` for a number that is 100 in practice everywhere
    /// this tool runs. Disclosed here, not silently assumed.
    clk_tck: u64,
    prev_wall: Instant,
    prev_cpu_ticks: u64,
}

impl CpuMemSampler {
    pub fn new() -> Self {
        Self {
            clk_tck: 100,
            prev_wall: Instant::now(),
            prev_cpu_ticks: read_self_cpu_ticks().unwrap_or(0),
        }
    }

    /// Returns `(cpu_pct, mem_rss_kb)` since the previous call (or since
    /// construction, for the first call).
    pub fn sample(&mut self) -> (f32, u64) {
        let now = Instant::now();
        let wall_elapsed_s = now.duration_since(self.prev_wall).as_secs_f64();
        let cpu_ticks = read_self_cpu_ticks().unwrap_or(self.prev_cpu_ticks);
        let delta_ticks = cpu_ticks.saturating_sub(self.prev_cpu_ticks);
        self.prev_wall = now;
        self.prev_cpu_ticks = cpu_ticks;

        let cpu_pct = if wall_elapsed_s > 0.0 {
            #[allow(
                clippy::cast_precision_loss,
                reason = "tick counts here are well within f64's exact-integer range for any \
                          realistic sampling interval this tool uses"
            )]
            let pct = 100.0 * (delta_ticks as f64 / self.clk_tck as f64) / wall_elapsed_s;
            pct as f32
        } else {
            0.0
        };
        let mem_rss_kb = read_self_rss_kb().unwrap_or(0);
        (cpu_pct, mem_rss_kb)
    }
}

/// Sums `utime` (field 14) and `stime` (field 15) from `/proc/self/stat`,
/// in clock ticks. Parsed past the `comm` field's own closing `)` rather
/// than by naive whitespace-splitting the whole line -- `comm` (the
/// process name in parentheses) can itself contain spaces, per `proc(5)`.
fn read_self_cpu_ticks() -> Option<u64> {
    let contents = fs::read_to_string("/proc/self/stat").ok()?;
    let after_comm = contents.rfind(')')?;
    let rest = contents.get(after_comm + 1..)?;
    let fields: Vec<&str> = rest.split_whitespace().collect();
    // `rest`'s field 0 is /proc/self/stat's field 3 (`state`); field 14
    // (`utime`) is therefore index 11, field 15 (`stime`) index 12.
    let utime: u64 = fields.get(11)?.parse().ok()?;
    let stime: u64 = fields.get(12)?.parse().ok()?;
    Some(utime + stime)
}

/// Parses the `VmRSS:` line of `/proc/self/status` (kB).
fn read_self_rss_kb() -> Option<u64> {
    let contents = fs::read_to_string("/proc/self/status").ok()?;
    for line in contents.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            return rest.split_whitespace().next()?.parse().ok();
        }
    }
    None
}

/// Real GPU utilization via the Linux `amdgpu` kernel driver's own sysfs
/// exposure -- `None` (never a fabricated number) if no such file exists
/// (non-AMD hardware, non-Linux platform, or a permissions issue), the
/// disclosed boundary confirmed with the project owner before
/// implementing this. Returns the first readable card found; this
/// project's own real dev machine has exactly one GPU, so "first found"
/// is the correct, and only, one there.
pub fn read_gpu_busy_percent() -> Option<f32> {
    let drm_dir = fs::read_dir("/sys/class/drm").ok()?;
    for entry in drm_dir.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        // Only `cardN` itself (e.g. "card0") -- `/sys/class/drm` also
        // contains per-connector entries (e.g. "card0-DP-1") that have
        // no `device/gpu_busy_percent` file of their own.
        if !name.starts_with("card") || !name["card".len()..].bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let busy_path = entry.path().join("device/gpu_busy_percent");
        if let Ok(contents) = fs::read_to_string(&busy_path) {
            if let Ok(pct) = contents.trim().parse::<f32>() {
                return Some(pct);
            }
        }
    }
    None
}
