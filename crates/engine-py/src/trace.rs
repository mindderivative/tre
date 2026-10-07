//! 0.5.4 (#135): the frame stream as a Chrome / Perfetto trace.
//!
//! `Window.start_trace(path)` opens a file and every drawn frame after it adds
//! slices in the Trace Event Format (`chrome://tracing`, ui.perfetto.dev,
//! `speedscope`): one slice for the frame with its stages inside it, on the
//! loop's own track, and the GPU's time on a second track once the adapter
//! has read it back (a few frames later). The file is a JSON array that is
//! written as it goes and closed by `stop_trace`; viewers read one that was
//! never closed (a crash, a window closed first) as well.

use std::io::{self, Write};
use std::time::{Duration, Instant};

use crate::frame_stats::{FrameRecord, Redraw};

/// Frames between flushes, so a crash loses little.
const FLUSH_EVERY: u64 = 30;

/// A trace being written.
pub(crate) struct Trace {
    out: Box<dyn Write + Send>,
    began: Instant,
    /// Whether nothing but the header is written yet (no comma needed).
    first: bool,
    frames: u64,
}

fn us(d: Duration) -> f64 {
    d.as_secs_f64() * 1e6
}

impl Trace {
    /// Starts a trace on `out`: the array's opening and the track names.
    pub(crate) fn begin(mut out: Box<dyn Write + Send>) -> io::Result<Self> {
        out.write_all(b"[\n")?;
        let mut trace = Self {
            out,
            began: Instant::now(),
            first: true,
            frames: 0,
        };
        trace.event(r#"{"name":"process_name","ph":"M","pid":1,"args":{"name":"tre window"}}"#)?;
        trace.event(
            r#"{"name":"thread_name","ph":"M","pid":1,"tid":1,"args":{"name":"event loop (CPU)"}}"#,
        )?;
        trace.event(r#"{"name":"thread_name","ph":"M","pid":1,"tid":2,"args":{"name":"GPU"}}"#)?;
        Ok(trace)
    }

    fn event(&mut self, json: &str) -> io::Result<()> {
        if !self.first {
            self.out.write_all(b",\n")?;
        }
        self.first = false;
        self.out.write_all(json.as_bytes())
    }

    fn slice(
        &mut self,
        name: &str,
        tid: u8,
        start: Duration,
        took: Duration,
        args: &str,
    ) -> io::Result<()> {
        let json = format!(
            r#"{{"name":"{name}","cat":"tre","ph":"X","pid":1,"tid":{tid},"ts":{:.3},"dur":{:.3},"args":{{{args}}}}}"#,
            us(start),
            us(took),
        );
        self.event(&json)
    }

    /// How long after the trace began `at` was.
    fn since_start(&self, at: Instant) -> Duration {
        at.saturating_duration_since(self.began)
    }

    /// Adds a drawn frame: its slice and the stages in it. Frames that began
    /// before the trace did are left out.
    pub(crate) fn frame(&mut self, r: &FrameRecord) -> io::Result<()> {
        if r.at < self.began {
            return Ok(());
        }
        self.frames += 1;
        let (redraw, rects, area) = match r.redraw {
            Redraw::Nothing => ("nothing", 0, 0.0),
            Redraw::Full => ("full", 1, 1.0),
            Redraw::Partial { rects, area } => ("partial", rects, area),
        };
        let start = self.since_start(r.at);
        self.slice(
            &format!("frame {}", r.index),
            1,
            start,
            r.total,
            &format!(
                r#""redraw":"{redraw}","damage_rects":{rects},"damage_area":{area:.4},"nodes":{},"shader_passes":{}"#,
                r.nodes, r.shader_passes
            ),
        )?;
        let mut cursor = start;
        for (name, took) in [
            ("tick", r.tick),
            ("layout", r.layout),
            ("configure", r.configure),
            ("prepare", r.prepare),
            ("acquire", r.acquire),
            ("draw", r.draw),
            ("present", r.present),
        ] {
            if took > Duration::ZERO {
                self.slice(name, 1, cursor, took, "")?;
            }
            cursor += took;
        }
        if self.frames.is_multiple_of(FLUSH_EVERY) {
            self.out.flush()?;
        }
        Ok(())
    }

    /// Adds the GPU's time for `r`, read back after the frame was written: on
    /// its own track, starting where the frame's drawing did.
    pub(crate) fn gpu(&mut self, r: &FrameRecord, took: Duration) -> io::Result<()> {
        if r.at < self.began {
            return Ok(());
        }
        let draw_start =
            self.since_start(r.at) + r.tick + r.layout + r.configure + r.prepare + r.acquire;
        self.slice(&format!("gpu frame {}", r.index), 2, draw_start, took, "")
    }

    /// Closes the array and flushes; returns how many frames were written.
    pub(crate) fn finish(mut self) -> io::Result<u64> {
        self.out.write_all(b"\n]\n")?;
        self.out.flush()?;
        Ok(self.frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// A `Write` that keeps what it is given where the test can read it.
    #[derive(Clone, Default)]
    struct Sink(Arc<Mutex<Vec<u8>>>);

    impl Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn record(at: Instant, index: u64) -> FrameRecord {
        let ms = Duration::from_millis;
        FrameRecord {
            index,
            at,
            tick: ms(1),
            layout: ms(2),
            configure: ms(0),
            prepare: ms(0),
            acquire: ms(3),
            draw: ms(4),
            present: ms(1),
            total: ms(11),
            redraw: Redraw::Partial {
                rects: 2,
                area: 0.25,
            },
            shader_passes: 0,
            nodes: 7,
            size: (10, 10),
            gpu: None,
        }
    }

    #[test]
    fn a_trace_is_a_json_array_of_slices_with_the_stages_inside_the_frame() {
        let sink = Sink::default();
        let mut trace = Trace::begin(Box::new(sink.clone())).unwrap();
        let at = Instant::now() + Duration::from_millis(5);
        let r = record(at, 1);
        trace.frame(&r).unwrap();
        trace.gpu(&r, Duration::from_micros(1500)).unwrap();
        assert_eq!(trace.finish().unwrap(), 1);
        let text = String::from_utf8(sink.0.lock().unwrap().clone()).unwrap();
        assert!(
            text.starts_with("[\n") && text.trim_end().ends_with(']'),
            "{text}"
        );
        for needle in [
            r#""name":"frame 1""#,
            r#""name":"draw""#,
            r#""name":"gpu frame 1""#,
            r#""redraw":"partial""#,
            r#""nodes":7"#,
            r#""tid":2"#,
        ] {
            assert!(text.contains(needle), "{needle} in {text}");
        }
        // A stage that took no time is not a slice.
        assert!(!text.contains(r#""name":"prepare""#));
        // Events are separated by commas and none trails the last.
        assert!(!text.contains(",\n]"));
    }

    #[test]
    fn frames_that_began_before_the_trace_are_left_out() {
        let sink = Sink::default();
        let early = Instant::now();
        std::thread::sleep(Duration::from_millis(2));
        let mut trace = Trace::begin(Box::new(sink.clone())).unwrap();
        trace.frame(&record(early, 1)).unwrap();
        assert_eq!(trace.finish().unwrap(), 0);
        let text = String::from_utf8(sink.0.lock().unwrap().clone()).unwrap();
        assert!(!text.contains("frame 1"), "{text}");
    }
}
