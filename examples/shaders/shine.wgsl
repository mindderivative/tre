// An effect: the node's own pixels come in as `content(uv)`, and a band of
// light sweeps across wherever the node painted something (`c.a`).
//
// `frame.time` is the seconds since the window opened; `u.speed` and
// `u.glow` are the app's own uniforms.
fn shade(p: Pixel) -> vec4<f32> {
    let c = content(p.uv);
    // A diagonal band, drawn left to right and repeating.
    let along = fract(frame.time * u.speed) * 2.4 - 0.7;
    let d = (p.uv.x + p.uv.y * 0.3) - along;
    let band = exp(-d * d * 40.0);
    // Add light only where there is text (c.a), never in the gaps.
    return vec4<f32>(c.rgb + u.glow * band * c.a, c.a);
}
