//! 0.5.4 (#135): how long the GPU spent on a frame, from `wgpu` timestamp
//! queries, where the adapter has them.
//!
//! Two timestamps are written into the frame's command encoder, one before
//! its commands and one after, and resolved into a buffer that is copied to a
//! mappable one. Reading that back waits for the GPU, so it is never done
//! the frame it was written: each frame uses one of a few slots, is mapped
//! once submitted, and is collected on a later frame, without blocking. A
//! frame whose slot is still in flight when its turn comes round goes
//! unmeasured rather than stalling the loop.
//!
//! What it measures is the encoder's own work between the two writes: the
//! scene's render, the copies, the scroll shift. A shader pass or an effect's
//! offscreen render is submitted separately, before it (see
//! `ShaderPasses`), and is not counted.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Frames in flight at once.
const SLOTS: usize = 4;

/// Only every this-many-th frame is timed. Timing a frame costs the CPU a
/// resolve, a copy, a map request and a device poll (measured: about 0.3 ms
/// a frame when every frame is timed, as much as a small frame's whole cost;
/// about 0.02 ms a frame at one in sixteen), so it is sampled rather than done
/// every frame. The mean over the kept frames is what it is for.
const SAMPLE_EVERY: u64 = 16;

/// The features a device needs for [`GpuTimer`].
pub fn required_features() -> wgpu::Features {
    wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Free,
    /// Written this frame, not yet submitted.
    Recording,
    /// Submitted; waiting for the read-back buffer to map.
    Pending,
}

struct Slot {
    readback: wgpu::Buffer,
    state: State,
    /// The caller's number for the frame in this slot.
    frame: u64,
    mapped: Arc<AtomicBool>,
}

/// Measures GPU time per frame. See the module docs.
pub struct GpuTimer {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    slots: Vec<Slot>,
    /// Nanoseconds per timestamp tick.
    period: f32,
    /// The slot the frame being recorded uses.
    current: Option<usize>,
}

impl GpuTimer {
    /// A timer for `device`, or `None` when it was not created with
    /// [`required_features`].
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        if !device.features().contains(required_features()) {
            return None;
        }
        let queries = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("tre frame timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: (SLOTS * 2) as u32,
        });
        // Each slot's results sit at an aligned offset in the resolve buffer.
        let size = SLOTS as u64 * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
        let resolve = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tre frame timestamps (resolved)"),
            size,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let slots = (0..SLOTS)
            .map(|_| Slot {
                readback: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("tre frame timestamps (read back)"),
                    size: 2 * std::mem::size_of::<u64>() as u64,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                state: State::Free,
                frame: 0,
                mapped: Arc::new(AtomicBool::new(false)),
            })
            .collect();
        Some(Self {
            queries,
            resolve,
            slots,
            period: queue.get_timestamp_period(),
            current: None,
        })
    }

    /// Starts timing `frame`'s commands: call before recording them. Does
    /// nothing if every slot is still in flight.
    pub fn begin(&mut self, encoder: &mut wgpu::CommandEncoder, frame: u64) {
        self.current = None;
        if !frame.is_multiple_of(SAMPLE_EVERY) {
            return;
        }
        let Some(index) = self.slots.iter().position(|s| s.state == State::Free) else {
            return;
        };
        encoder.write_timestamp(&self.queries, (index * 2) as u32);
        self.slots[index].state = State::Recording;
        self.slots[index].frame = frame;
        self.current = Some(index);
    }

    /// Ends the frame's timing: call after recording its last command, before
    /// finishing the encoder.
    pub fn end(&mut self, encoder: &mut wgpu::CommandEncoder) {
        let Some(index) = self.current else { return };
        let first = (index * 2) as u32;
        encoder.write_timestamp(&self.queries, first + 1);
        let offset = index as u64 * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
        encoder.resolve_query_set(&self.queries, first..first + 2, &self.resolve, offset);
        encoder.copy_buffer_to_buffer(
            &self.resolve,
            offset,
            &self.slots[index].readback,
            0,
            2 * std::mem::size_of::<u64>() as u64,
        );
    }

    /// The frame's encoder was submitted: ask for its result.
    pub fn submitted(&mut self) {
        let Some(index) = self.current.take() else {
            return;
        };
        let slot = &mut self.slots[index];
        slot.mapped.store(false, Ordering::Release);
        let flag = slot.mapped.clone();
        slot.readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                flag.store(result.is_ok(), Ordering::Release);
            });
        slot.state = State::Pending;
    }

    /// The results that have arrived since the last call: `(frame, GPU time)`.
    /// Never blocks.
    pub fn collect(&mut self, device: &wgpu::Device) -> Vec<(u64, Duration)> {
        if !self.slots.iter().any(|s| s.state == State::Pending) {
            return Vec::new();
        }
        let _ = device.poll(wgpu::PollType::Poll);
        let mut done = Vec::new();
        for slot in &mut self.slots {
            if slot.state != State::Pending || !slot.mapped.load(Ordering::Acquire) {
                continue;
            }
            {
                let Ok(view) = slot.readback.slice(..).get_mapped_range() else {
                    slot.state = State::Free;
                    continue;
                };
                let word = |i: usize| {
                    let mut bytes = [0u8; 8];
                    bytes.copy_from_slice(&view[i * 8..i * 8 + 8]);
                    u64::from_le_bytes(bytes)
                };
                let ticks = word(1).saturating_sub(word(0));
                let nanos = ticks as f64 * f64::from(self.period);
                done.push((slot.frame, Duration::from_nanos(nanos as u64)));
            }
            slot.readback.unmap();
            slot.mapped.store(false, Ordering::Release);
            slot.state = State::Free;
        }
        done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A device with the timestamp features, if this machine's adapter has them.
    fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
        pollster::block_on(async {
            let adapter = wgpu::Instance::default()
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .ok()?;
            if !adapter.features().contains(required_features()) {
                return None;
            }
            adapter
                .request_device(&wgpu::DeviceDescriptor {
                    required_features: required_features(),
                    ..Default::default()
                })
                .await
                .ok()
        })
    }

    #[test]
    fn a_device_without_the_features_has_no_timer() {
        let (device, queue) = pollster::block_on(async {
            let adapter = wgpu::Instance::default()
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .expect("an adapter");
            adapter
                .request_device(&wgpu::DeviceDescriptor::default())
                .await
                .expect("a device")
        });
        assert!(GpuTimer::new(&device, &queue).is_none());
    }

    #[test]
    fn frames_are_measured_later_and_in_order_without_blocking() {
        let Some((device, queue)) = device() else {
            eprintln!("this adapter has no timestamp queries: skipped");
            return;
        };
        let mut timer = GpuTimer::new(&device, &queue).expect("a timer");
        let target = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 4 << 20,
            usage: wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut seen = Vec::new();
        for frame in 1..=40u64 {
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            timer.begin(&mut encoder, frame);
            // Something for the GPU to do between the two timestamps.
            for _ in 0..32 {
                encoder.clear_buffer(&target, 0, None);
            }
            timer.end(&mut encoder);
            queue.submit([encoder.finish()]);
            timer.submitted();
            seen.extend(timer.collect(&device));
        }
        // The rest arrive once the GPU is done.
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        seen.extend(timer.collect(&device));
        assert!(!seen.is_empty(), "some frames were measured");
        let frames: Vec<u64> = seen.iter().map(|(f, _)| *f).collect();
        let mut sorted = frames.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), frames.len(), "no frame twice: {frames:?}");
        assert!(
            frames
                .iter()
                .all(|f| f % SAMPLE_EVERY == 0 && (1..=40).contains(f)),
            "only every {SAMPLE_EVERY}th frame is timed: {frames:?}"
        );
        assert!(
            seen.iter().any(|(_, d)| *d > Duration::ZERO),
            "the GPU took measurable time: {seen:?}"
        );
    }
}
