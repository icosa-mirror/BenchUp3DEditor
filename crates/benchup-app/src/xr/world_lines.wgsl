struct View { matrix: mat4x4<f32>, viewport: vec4<f32> }
@group(0) @binding(0) var<uniform> view: View;
struct Output { @builtin(position) position: vec4<f32>, @location(0) color: vec4<f32> }

@vertex
fn vertex(@builtin(vertex_index) index: u32, @location(0) start: vec3<f32>,
    @location(1) end: vec3<f32>, @location(2) color: vec4<f32>, @location(3) width: f32) -> Output {
    var a = view.matrix * vec4(start, 1.0);
    var b = view.matrix * vec4(end, 1.0);
    if (a.z < 0.0 && b.z < 0.0) { return Output(vec4(2.0, 2.0, 0.0, 1.0), vec4(0.0)); }
    if a.z < 0.0 { a = mix(a, b, -a.z / (b.z - a.z)); }
    if b.z < 0.0 { b = mix(b, a, -b.z / (a.z - b.z)); }
    if a.w <= 0.00001 || b.w <= 0.00001 { return Output(vec4(2.0, 2.0, 0.0, 1.0), vec4(0.0)); }
    let delta = (b.xy / b.w - a.xy / a.w) * view.viewport.xy;
    let normal = vec2(-delta.y, delta.x) / max(length(delta), 0.0001);
    let at_end = index == 2u || index == 4u || index == 5u;
    let positive = index == 1u || index == 3u || index == 4u;
    var clip = select(a, b, at_end);
    let side = select(-1.0, 1.0, positive);
    clip = vec4(clip.xy + normal * width * side * clip.w / view.viewport.xy, clip.zw);
    return Output(clip, color);
}

@fragment
fn fragment(input: Output) -> @location(0) vec4<f32> { return input.color; }
