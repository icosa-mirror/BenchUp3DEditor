struct View {
    projection: mat4x4<f32>,
    model: mat4x4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
    head: vec4<f32>,
};
@group(0) @binding(0) var<uniform> view: View;
@group(1) @binding(0) var atlas: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;
struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};
@vertex fn vertex(@location(0) anchor: vec3<f32>, @location(1) offset: vec2<f32>,
                  @location(2) uv: vec2<f32>, @location(3) color: vec4<f32>) -> Output {
    let center = (view.model * vec4<f32>(anchor, 1.0)).xyz;
    // A single head basis serves both eyes. Layout is retained in egui points;
    // the GPU keeps a readable angular size without per-eye CPU projection.
    let scale = clamp(distance(center, view.head.xyz), 0.3, 8.0) * 0.0011;
    let world = center + (view.right.xyz * offset.x - view.up.xyz * offset.y) * scale;
    var out: Output;
    out.position = view.projection * vec4<f32>(world, 1.0);
    out.uv = uv;
    out.color = color;
    return out;
}
@fragment fn fragment(in: Output) -> @location(0) vec4<f32> {
    let gamma = textureSample(atlas, atlas_sampler, in.uv) * in.color;
    let linear = select(gamma.rgb / 12.92, pow((gamma.rgb + 0.055) / 1.055, vec3<f32>(2.4)), gamma.rgb > vec3<f32>(0.04045));
    return vec4<f32>(linear, gamma.a);
}
