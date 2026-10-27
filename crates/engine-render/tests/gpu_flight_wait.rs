//! 0.5.9 (#238): `wait_while_idle`, the wait a host's GPU helper thread sleeps in while nothing
//! is in flight. Its own test file because the in-flight count is process-wide: next to other
//! tests that submit work, "nothing in flight" would not hold.

mod support;

use engine_render::{GpuWatch, any_in_flight, notify_flight, wait_while_idle};

fn empty_submit(device: &wgpu::Device, queue: &wgpu::Queue) {
    let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    queue.submit([encoder.finish()]);
}

/// 0.5.9 (#238): the host's GPU helper thread sleeps with no timer while nothing is in flight,
/// is woken by a submission, and can be told to give up.
#[test]
fn the_flight_wait_sleeps_until_a_submission_or_a_give_up() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    // A thread told to give up is let go by `notify_flight`, with nothing in flight.
    let give_up = Arc::new(AtomicBool::new(false));
    let waiting = {
        let give_up = give_up.clone();
        std::thread::spawn(move || wait_while_idle(&give_up))
    };
    std::thread::sleep(Duration::from_millis(150));
    assert!(!any_in_flight());
    assert!(
        !waiting.is_finished(),
        "an idle wait polls nothing and returns on its own"
    );
    give_up.store(true, Ordering::Relaxed);
    notify_flight();
    waiting
        .join()
        .expect("the waiting thread ends when told to give up");

    // A submission lets a waiting thread go, without anyone asking it to give up.
    pollster::block_on(async {
        let (device, queue) = support::device("gpu-watch flight wait").await;
        let mut watch = GpuWatch::install(&device);
        let never = Arc::new(AtomicBool::new(false));
        let waiting = {
            let never = never.clone();
            std::thread::spawn(move || wait_while_idle(&never))
        };
        std::thread::sleep(Duration::from_millis(100));
        empty_submit(&device, &queue);
        watch.submitted(&queue);
        let began = std::time::Instant::now();
        while !waiting.is_finished() && began.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            waiting.is_finished(),
            "a submission wakes the waiting thread"
        );
        waiting.join().unwrap();
        let _ = watch.poll(&device);
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
    });
}
