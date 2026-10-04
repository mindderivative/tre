//! 0.5.4 (#110): what the pinned vello_gpu can render as layer effects, probed
//! directly (no tre node involved). `cargo test -p engine-render --release --test
//! vello_effects -- --ignored --nocapture`

mod support;

use std::panic::{AssertUnwindSafe, catch_unwind};

use engine_render::FrameRenderer;
use peniko::kurbo::{Affine, BezPath, Rect, Shape};
use peniko::{BlendMode, Color, Compose, Mix};
use vello_common::filter_effects::{Filter, FilterFunction, FilterPrimitive};
use vello_gpu::{RenderSize, RenderTargetConfig, Scene};

const S: u16 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn render(device: &wgpu::Device, queue: &wgpu::Queue, paint: impl Fn(&mut Scene)) -> Vec<u8> {
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

fn px(data: &[u8], x: usize, y: usize) -> [u8; 4] {
    let row = (usize::from(S) * 4).next_multiple_of(256);
    let i = y * row + x * 4;
    [data[i], data[i + 1], data[i + 2], data[i + 3]]
}

fn bg(s: &mut Scene) {
    s.set_paint(Color::from_rgba8(40, 90, 200, 255));
    s.fill_rect(&Rect::new(0.0, 0.0, 64.0, 64.0));
}

fn red_square(s: &mut Scene) {
    s.set_paint(Color::from_rgba8(255, 0, 0, 255));
    s.fill_rect(&Rect::new(20.0, 20.0, 44.0, 44.0));
}

fn probe(name: &str, device: &wgpu::Device, queue: &wgpu::Queue, f: impl Fn(&mut Scene)) {
    match catch_unwind(AssertUnwindSafe(|| render(device, queue, &f))) {
        Ok(d) => println!(
            "{name:34} ok   centre {:?} edge(18,32) {:?} outside(5,5) {:?}",
            px(&d, 32, 32),
            px(&d, 18, 32),
            px(&d, 5, 5)
        ),
        Err(e) => {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            println!("{name:34} PANIC {msg}");
        }
    }
}

#[test]
#[ignore = "probe"]
fn effects() {
    let (device, queue) = pollster::block_on(support::device("effects"));
    let (d, q) = (&device, &queue);
    probe("plain red square", d, q, |s| {
        bg(s);
        red_square(s);
    });
    probe("blur layer sigma 3", d, q, |s| {
        bg(s);
        s.push_filter_layer(Filter::from_function(FilterFunction::Blur { radius: 3.0 }));
        red_square(s);
        s.pop_layer();
    });
    probe("blur layer sigma 8", d, q, |s| {
        bg(s);
        s.push_filter_layer(Filter::from_function(FilterFunction::Blur { radius: 8.0 }));
        red_square(s);
        s.pop_layer();
    });
    probe("saturate function", d, q, |s| {
        bg(s);
        s.push_filter_layer(Filter::from_function(FilterFunction::Saturate {
            amount: 0.0,
        }));
        red_square(s);
        s.pop_layer();
    });
    probe("drop shadow primitive", d, q, |s| {
        bg(s);
        s.push_filter_layer(Filter::from_primitive(FilterPrimitive::DropShadow {
            dx: 4.0,
            dy: 4.0,
            std_deviation: 2.0,
            color: Color::from_rgba8(0, 0, 0, 200),
            edge_mode: Default::default(),
        }));
        red_square(s);
        s.pop_layer();
    });
    for (name, mix) in [
        ("blend multiply", Mix::Multiply),
        ("blend screen", Mix::Screen),
        ("blend overlay", Mix::Overlay),
        ("blend difference", Mix::Difference),
    ] {
        probe(name, d, q, |s| {
            bg(s);
            s.push_blend_layer(BlendMode::new(mix, Compose::SrcOver));
            s.set_paint(Color::from_rgba8(255, 200, 0, 255));
            s.fill_rect(&Rect::new(20.0, 20.0, 44.0, 44.0));
            s.pop_layer();
        });
    }
    probe("clip layer (rounded)", d, q, |s| {
        bg(s);
        let path: BezPath =
            peniko::kurbo::RoundedRect::new(20.0, 20.0, 44.0, 44.0, 12.0).to_path(0.1);
        s.push_clip_layer(&path);
        s.set_paint(Color::from_rgba8(255, 0, 0, 255));
        s.fill_rect(&Rect::new(0.0, 0.0, 64.0, 64.0));
        s.pop_layer();
    });
    probe("backdrop blur by re-drawing", d, q, |s| {
        bg(s);
        red_square(s);
        // A blurred copy of what is behind, clipped to a panel on top.
        let panel: BezPath =
            peniko::kurbo::RoundedRect::new(10.0, 10.0, 54.0, 54.0, 8.0).to_path(0.1);
        s.push_clip_layer(&panel);
        s.push_filter_layer(Filter::from_function(FilterFunction::Blur { radius: 4.0 }));
        bg(s);
        red_square(s);
        s.pop_layer();
        s.pop_layer();
    });
    probe("mask layer", d, q, |s| {
        bg(s);
        s.push_mask_layer(vello_common::mask::Mask::new_luminance(
            &vello_common::pixmap::Pixmap::new(8, 8),
        ));
        red_square(s);
        s.pop_layer();
    });
}
