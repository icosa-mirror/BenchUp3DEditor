struct View { clip_from_tracking: mat4x4<f32> }
@group(0) @binding(0) var<uniform> view: View;
struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
}
@vertex fn vertex(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) x: vec4<f32>,
    @location(4) y: vec4<f32>,
    @location(5) z: vec4<f32>,
    @location(6) translation: vec4<f32>,
    @location(7) tint: vec4<f32>,
) -> Output {
    let tracking_from_grip = mat4x4<f32>(x, y, z, translation);
    let world_normal = normalize((tracking_from_grip * vec4<f32>(normal, 0.)).xyz);
    let light = 0.45 + 0.55 * max(dot(world_normal, normalize(vec3<f32>(0.3, 0.8, 0.5))), 0.);
    var output: Output;
    output.position = view.clip_from_tracking * tracking_from_grip * vec4<f32>(position, 1.);
    output.color = mix(color.rgb, tint.rgb, color.a) * light;
    return output;
}
@fragment fn fragment(input: Output) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color, 1.);
}
