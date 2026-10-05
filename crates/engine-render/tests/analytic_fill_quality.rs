//! 0.5.4 (#107): how close vello's analytic rounded-rect primitive
//! (`fill_blurred_rounded_rect`) gets to a filled path, for several blur
//! widths. `cargo test -p engine-render --release --test analytic_fill_quality -- --ignored --nocapture`

mod support;

use engine_render::FrameRenderer;
use peniko::Color;
use peniko::kurbo::{Affine, Rect, RoundedRect, Shape};
use vello_gpu::{RenderSize, RenderTargetConfig, Scene};

const S: u16 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn draw(device: &wgpu::Device, queue: &wgpu::Queue, paint: impl Fn(&mut Scene)) -> Vec<u8> {
    let mut renderer = FrameRenderer::new(
        device,
        &RenderTargetConfig {
            format: FORMAT,
            width: S,
            height: S,
        },
    );
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: u32::from(S),
            height: u32::from(S),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut scene = Scene::new(S, S);
    scene.set_transform(Affine::IDENTITY);
    scene.set_paint(Color::from_rgba8(0, 0, 0, 255));
    scene.fill_rect(&Rect::new(0.0, 0.0, f64::from(S), f64::from(S)));
    paint(&mut scene);
    let mut encoder = device.create_command_encoder(&Default::default());
    renderer.render_into(
        &scene,
        device,
        queue,
        &mut encoder,
        &RenderSize {
            width: S,
            height: S,
        },
        &view,
        None,
    );
    queue.submit([encoder.finish()]);
    support::read_texture(device, queue, &target)
}

#[test]
#[ignore = "measurement, not correctness"]
fn analytic_fill_vs_path_fill() {
    let (device, queue) = pollster::block_on(support::device("quality"));
    let white = Color::from_rgba8(255, 255, 255, 255);
    println!("sigma  radius  offset   max|diff|  mean|diff| over edge pixels");
    for sigma in [0.25f32, 0.3, 0.35, 0.4, 0.5] {
        for radius in [0.0f64, 6.0, 16.0] {
            for off in [0.0f64, 0.5, 0.3] {
                let rect = Rect::new(10.0 + off, 10.0 + off, 50.0 + off, 40.0 + off);
                let a = draw(&device, &queue, |s| {
                    s.set_paint(white);
                    s.fill_path(&RoundedRect::from_rect(rect, radius).to_path(0.1));
                });
                let b = draw(&device, &queue, |s| {
                    s.set_paint(white);
                    s.fill_blurred_rounded_rect(&rect, radius as f32, sigma, false);
                });
                let row = (u32::from(S) * 4).next_multiple_of(256) as usize;
                let (mut max, mut sum, mut n) = (0i32, 0i64, 0i64);
                for y in 0..S as usize {
                    for x in 0..S as usize {
                        let (p, q) = (a[y * row + x * 4], b[y * row + x * 4]);
                        let d = (i32::from(p) - i32::from(q)).abs();
                        if p != 0 && p != 255 || q != 0 && q != 255 {
                            sum += i64::from(d);
                            n += 1;
                        }
                        max = max.max(d);
                    }
                }
                println!(
                    "{sigma:5}  {radius:6}  {off:5}   {max:8}  {:8.2}",
                    sum as f64 / n.max(1) as f64
                );
            }
        }
    }
}
