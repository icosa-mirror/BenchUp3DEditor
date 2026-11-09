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
@group(2) @binding(0) var<storage,read> owner_slots: array<u32>;
@group(2) @binding(1) var visibility: texture_2d<u32>;
struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) owners: vec2<u32>,
};
@vertex fn vertex(@location(0) anchor: vec3<f32>, @location(1) offset: vec2<f32>,
                  @location(2) uv: vec2<f32>, @location(3) color: vec4<f32>, @location(4) owners: vec2<u32>) -> Output {
    let center = (view.model * vec4<f32>(anchor, 1.0)).xyz;
    // A single head basis serves both eyes. Layout is retained in egui points;
    // the GPU keeps a readable angular size without per-eye CPU projection.
    let scale = clamp(distance(center, view.head.xyz), 0.3, 8.0) * 0.0011;
    let world = center + (view.right.xyz * offset.x - view.up.xyz * offset.y) * scale;
    var out: Output;
    out.position = view.projection * vec4<f32>(world, 1.0);
    out.uv = uv;
    out.color = color;
    out.owners = owners;
    return out;
}
@fragment fn fragment(in: Output) -> @location(0) vec4<f32> {
    let face=textureLoad(visibility,vec2<i32>(in.position.xy),0).xy;
    if in.owners.x!=0xffffffffu && face.x!=0u && bitcast<f32>(face.y)<in.position.z-0.00000024 {
        // Sorted slots avoid a per-fragment linear scan of large group contents.
        var lo=in.owners.x; var hi=in.owners.y;
        loop {if lo>=hi {break;} let mid=lo+(hi-lo)/2u;
            if owner_slots[mid]<face.x {lo=mid+1u;} else {hi=mid;}}
        if lo>=in.owners.y || owner_slots[lo]!=face.x {discard;}
    }
    let gamma = textureSample(atlas, atlas_sampler, in.uv) * in.color;
    let linear = select(gamma.rgb / 12.92, pow((gamma.rgb + 0.055) / 1.055, vec3<f32>(2.4)), gamma.rgb > vec3<f32>(0.04045));
    return vec4<f32>(linear, gamma.a);
}
