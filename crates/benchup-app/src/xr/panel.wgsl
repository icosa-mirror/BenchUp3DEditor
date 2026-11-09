@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4(corner * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fragment(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    if pixel.y < 450.0 {
        if pixel.x < 720.0 { return vec4(1.0, 0.0, 0.0, 1.0); }
        return vec4(0.0, 1.0, 0.0, 1.0);
    }
    if pixel.x < 720.0 { return vec4(0.0, 0.0, 1.0, 1.0); }
    return vec4(1.0);
}
