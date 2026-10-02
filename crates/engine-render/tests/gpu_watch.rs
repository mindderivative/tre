//! 0.5.1 (#65): `GpuWatch` against a real device. A lost device is driven
//! with `Device::destroy()`, which is safe and deterministic: a real hang
//! can't be provoked here (and is never run on a real GPU). The stall
//! watchdog's own timing is unit-tested in the module.

mod support;

use engine_render::{GpuReport, GpuWatch, any_in_flight};

fn too_big_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    // Larger than any device's maximum buffer size: a validation error.
    device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 1 << 60,
        usage: wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn empty_submit(device: &wgpu::Device, queue: &wgpu::Queue) {
    let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    queue.submit([encoder.finish()]);
}

/// The device-lost callback fires only when the device is polled, so
/// nothing is known until `poll` -- and then it is reported, once.
#[test]
fn a_destroyed_device_is_reported_lost_on_the_first_poll() {
    pollster::block_on(async {
        let (device, _queue) = support::device("gpu-watch lost").await;
        let mut watch = GpuWatch::install(&device);
        device.destroy();
        assert!(
            !watch.is_lost(),
            "nothing is known before the device is polled"
        );
        let reports = watch.poll(&device);
        assert_eq!(
            reports,
            vec![GpuReport::Lost {
                destroyed: true,
                message: String::new()
            }]
        );
        assert!(watch.is_lost());
        assert!(watch.poll(&device).is_empty(), "a loss is reported once");
    });
}

/// `wgpu`'s default handler panics on any GPU error. With the watch
/// installed the error is reported, once per distinct message, and the
/// process survives (this test returning is the proof).
#[test]
fn a_gpu_error_is_reported_once_and_does_not_panic() {
    pollster::block_on(async {
        let (device, _queue) = support::device("gpu-watch error").await;
        let mut watch = GpuWatch::install(&device);
        let _a = too_big_buffer(&device);
        let _b = too_big_buffer(&device); // the same error again
        let reports = watch.poll(&device);
        assert_eq!(reports.len(), 1, "one distinct error, once: {reports:?}");
        let GpuReport::Error { message } = &reports[0] else {
            panic!("expected an error report, got {reports:?}");
        };
        assert!(
            message.contains("create_buffer"),
            "says what failed: {message}"
        );
        let _c = too_big_buffer(&device);
        assert!(watch.poll(&device).is_empty(), "already reported");
        assert!(!watch.is_lost(), "an error is not a lost device");
    });
}

/// A submission counts as running until the GPU reports its work done,
/// which also takes a poll.
#[test]
fn a_submission_is_in_flight_until_the_device_is_polled_and_done() {
    pollster::block_on(async {
        let (device, queue) = support::device("gpu-watch in flight").await;
        let mut watch = GpuWatch::install(&device);
        assert!(!watch.in_flight());
        empty_submit(&device, &queue);
        watch.submitted(&queue);
        assert!(watch.in_flight(), "submitted, not yet polled");
        assert!(any_in_flight(), "and the loop is told to keep waking");
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("the device finishes its work");
        assert!(!watch.in_flight(), "done");
        assert!(
            watch.poll(&device).is_empty(),
            "a healthy frame reports nothing"
        );
    });
}

/// With the watchdog on, a frame that finishes is never reported, and an
/// unusable limit is ignored rather than a panic.
#[test]
fn the_watchdog_stays_quiet_for_a_healthy_frame() {
    pollster::block_on(async {
        let (device, queue) = support::device("gpu-watch watchdog").await;
        let mut watch = GpuWatch::install(&device);
        watch.set_watchdog(Some(5.0));
        empty_submit(&device, &queue);
        watch.submitted(&queue);
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("the device finishes its work");
        assert!(watch.poll(&device).is_empty());
        watch.set_watchdog(Some(f64::NAN)); // ignored, not a panic
        watch.set_watchdog(None);
        assert!(watch.poll(&device).is_empty());
    });
}
