// A fill: the app paints a node's box itself, here an animated Julia set.
// `p.uv` runs 0 to 1 over the box; `frame.size` is the box in pixels, so the
// picture keeps its proportions at any size.
fn shade(p: Pixel) -> vec4<f32> {
    let aspect = frame.size.x / frame.size.y;
    var z = (p.uv - vec2<f32>(0.5)) * vec2<f32>(3.0 * aspect, 3.0) / u.zoom;
    let t = frame.time * u.speed;
    let c = vec2<f32>(0.7885 * cos(t), 0.7885 * sin(t));
    var steps = 0.0;
    for (var k = 0; k < 80; k++) {
        if (dot(z, z) > 16.0) {
            break;
        }
        z = vec2<f32>(z.x * z.x - z.y * z.y, 2.0 * z.x * z.y) + c;
        steps += 1.0;
    }
    if (steps >= 80.0) {
        return vec4<f32>(u.inside.rgb, 1.0);
    }
    let m = steps / 80.0;
    let wave = 0.5 + 0.5 * cos(6.28318 * (vec3<f32>(m * 2.5) + vec3<f32>(0.0, 0.33, 0.67)));
    return vec4<f32>(wave * 0.9 + vec3<f32>(0.05), 1.0);
}
