struct Transform { clip_from_quad: mat4x4<f32> }
@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
@group(0) @binding(2) var<uniform> transform: Transform;
struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
@vertex fn vertex(@builtin(vertex_index) index: u32) -> Output {
    let point = array<vec2<f32>, 6>(
        vec2<f32>(-1., -1.), vec2<f32>(1., -1.), vec2<f32>(1., 1.),
        vec2<f32>(-1., -1.), vec2<f32>(1., 1.), vec2<f32>(-1., 1.),
    )[index];
    var output: Output;
    output.position = transform.clip_from_quad * vec4<f32>(point, 0., 1.);
    output.uv = vec2<f32>(point.x * 0.5 + 0.5, 0.5 - point.y * 0.5);
    return output;
}
@fragment fn fragment(input: Output) -> @location(0) vec4<f32> {
    return textureSample(image, image_sampler, input.uv);
}
