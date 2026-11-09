struct View { matrix: mat4x4<f32>, viewport: vec4<f32> }
@group(0) @binding(0) var<uniform> view: View;
struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) @interpolate(linear) local: vec2<f32>,
    @location(2) @interpolate(flat) style: vec4<f32>,
}

@vertex
fn vertex(@builtin(vertex_index) index: u32, @location(0) start: vec3<f32>,
    @location(1) end: vec3<f32>, @location(2) color: vec4<f32>, @location(3) width: f32,
    @location(4) style: vec4<f32>) -> Output {
    var out: Output;
    out.position = vec4(2.0, 2.0, 0.0, 1.0);
    var a = view.matrix * vec4(start, 1.0);
    var b = view.matrix * vec4(end, 1.0);
    if (a.z < 0.0 && b.z < 0.0) { return out; }
    if a.z < 0.0 { a = mix(a, b, -a.z / (b.z - a.z)); }
    if b.z < 0.0 { b = mix(b, a, -b.z / (a.z - b.z)); }
    if a.w <= 0.00001 || b.w <= 0.00001 { return out; }
    let delta = (b.xy / b.w - a.xy / a.w) * view.viewport.xy * 0.5;
    let direction = delta / max(length(delta), 0.0001);
    let normal = vec2(-direction.y, direction.x);
    let at_end = index == 2u || index == 4u || index == 5u;
    let positive = index == 1u || index == 3u || index == 4u;
    let along = select(-1.0, 1.0, at_end);
    var clip = select(a, b, at_end);
    let side = select(-1.0, 1.0, positive);
    var offset = normal * width * side * 0.5;
    if style.x == 1.0 {
        clip = a;
        offset = vec2(along, side) * style.y;
    } else if style.x == 2.0 {
        clip = a;
        let tick = direction * 3.0 + normal * 4.0;
        let cross_tick = vec2(-tick.y, tick.x) / 5.0;
        offset = tick * along + cross_tick * width * side * 0.5;
    }
    out.position = vec4(clip.xy + offset * 2.0 * clip.w / view.viewport.xy, clip.zw);
    out.color = color;
    out.local = select(vec2(0.0), vec2(length(delta), 0.0), at_end);
    if style.x == 1.0 { out.local = vec2(along, side) * style.y; }
    out.style = style;
    return out;
}

@fragment
fn fragment(input: Output) -> @location(0) vec4<f32> {
    if input.style.x == 0.0 && input.style.y > 0.0 && input.local.x % input.style.y > input.style.z { discard; }
    if input.style.x == 1.0 && min(abs(input.local.x - input.local.y), abs(input.local.x + input.local.y)) > 1.2 { discard; }
    let gamma = input.color;
    let linear = select(gamma.rgb / 12.92, pow((gamma.rgb + 0.055) / 1.055, vec3<f32>(2.4)), gamma.rgb > vec3<f32>(0.04045));
    return vec4(linear, gamma.a);
}
