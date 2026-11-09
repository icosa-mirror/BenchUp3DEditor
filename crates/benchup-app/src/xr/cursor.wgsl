struct Pointer { geometry: vec4<f32>, state: vec4<f32> }
@group(0) @binding(0) var<uniform> pointer: Pointer;

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    let pixel = pointer.geometry.xy + corner * 11.0;
    return vec4(pixel.x * 2.0 / pointer.geometry.z - 1.0, 1.0 - pixel.y * 2.0 / pointer.geometry.w, 0.0, 1.0);
}

@fragment
fn fragment(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    let distance = length(pixel.xy - pointer.geometry.xy);
    if distance > 9.0 { discard; }
    let alpha = 1.0 - smoothstep(8.0, 9.0, distance);
    var color = vec3<f32>(1.0);
    if pointer.state.x > 0.5 { color = vec3<f32>(1.0, 0.55, 0.05); }
    if distance > 6.0 { color = vec3<f32>(0.0); }
    return vec4(color * alpha, alpha);
}
