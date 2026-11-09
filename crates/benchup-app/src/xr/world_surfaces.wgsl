@group(0) @binding(0) var<uniform> matrix: mat4x4<f32>;
struct Output { @builtin(position) position: vec4<f32>, @location(0) color: vec4<f32> }
@vertex fn vertex(@location(0) point: vec3<f32>, @location(1) color: vec4<f32>) -> Output {
    return Output(matrix * vec4<f32>(point,1.0),color);
}
@fragment fn fragment(input: Output) -> @location(0) vec4<f32> {
    let color=input.color;
    let linear=select(color.rgb/12.92,pow((color.rgb+0.055)/1.055,vec3<f32>(2.4)),color.rgb>vec3<f32>(0.04045));
    return vec4<f32>(linear,color.a);
}
@fragment fn visibility(input: Output) -> @location(0) vec2<u32> {
    return vec2<u32>(0xffffffffu,bitcast<u32>(input.position.z));
}
