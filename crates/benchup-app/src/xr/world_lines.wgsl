struct View { matrix: mat4x4<f32>, viewport: vec4<f32>,limits:vec4<f32> }
@group(0) @binding(0) var<uniform> view: View;
@group(1) @binding(0) var<storage,read> owner_slots:array<u32>;
@group(1) @binding(1) var visibility:texture_2d<u32>;
struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) @interpolate(linear) local: vec2<f32>,
    @location(2) @interpolate(flat) style: vec4<f32>,
    @location(3) @interpolate(flat) owners:vec2<u32>,
    @location(4) @interpolate(linear) depth:f32,
}

@vertex
fn vertex(@builtin(vertex_index) index: u32, @location(0) start: vec3<f32>,
    @location(1) end: vec3<f32>, @location(2) color: vec4<f32>, @location(3) width: f32,
    @location(4) style: vec4<f32>,@location(5) owners:vec2<u32>) -> Output {
    var out: Output;
    out.position = vec4(2.0, 2.0, 0.0, 1.0);
    var world_a=start;var world_b=end;
    if view.viewport.z>0.0 && style.w==0.0 {
        for(var axis=0u;axis<3u;axis++) {
            let limit=view.limits[axis];
            if world_a[axis]>limit && world_b[axis]>limit {return out;}
            if world_a[axis]>limit {world_a=mix(world_a,world_b,(limit-world_a[axis])/(world_b[axis]-world_a[axis]));}
            if world_b[axis]>limit {world_b=mix(world_b,world_a,(limit-world_b[axis])/(world_a[axis]-world_b[axis]));}
        }
    }
    var a = view.matrix * vec4(world_a, 1.0);
    var b = view.matrix * vec4(world_b, 1.0);
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
    } else if style.x==3.0 {
        clip=b;
        offset=direction*select(-style.y,0.0,at_end)+normal*side*style.y*0.5;
    }
    out.position = vec4(clip.xy + offset * 2.0 * clip.w / view.viewport.xy, clip.zw);
    out.color = color;
    out.local = select(vec2(0.0), vec2(length(delta), 0.0), at_end);
    if style.x == 1.0 { out.local = vec2(along, side) * style.y; }
    if style.x==3.0 {out.local=vec2(select(-style.y,0.0,at_end),side*style.y*0.5);}
    out.style = style;
    out.owners=owners;out.depth=clip.z/clip.w;
    return out;
}

@fragment
fn fragment(input: Output) -> @location(0) vec4<f32> {
    let face=textureLoad(visibility,vec2<i32>(input.position.xy),0).xy;
    if input.style.w==0.0 && face.x!=0u && bitcast<f32>(face.y)<input.depth-0.000001 {
        var lo=input.owners.x;var hi=input.owners.y;
        loop {if lo>=hi {break;}let mid=lo+(hi-lo)/2u;if owner_slots[mid]<face.x {lo=mid+1u;} else {hi=mid;}}
        if lo>=input.owners.y || owner_slots[lo]!=face.x {discard;}
    }
    if input.style.x == 0.0 && input.style.y > 0.0 && input.local.x % input.style.y > input.style.z { discard; }
    if input.style.x == 1.0 && min(abs(input.local.x - input.local.y), abs(input.local.x + input.local.y)) > 1.2 { discard; }
    if input.style.x==3.0 && abs(abs(input.local.y)+input.local.x*0.5)>input.style.z*0.5 {discard;}
    let gamma = input.color;
    let linear = select(gamma.rgb / 12.92, pow((gamma.rgb + 0.055) / 1.055, vec3<f32>(2.4)), gamma.rgb > vec3<f32>(0.04045));
    return vec4(linear, gamma.a);
}
