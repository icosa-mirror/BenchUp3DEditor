struct View {matrix:mat4x4<f32>,inverse:mat4x4<f32>,eye:vec4<f32>,params:vec4<f32>,origin:vec4<f32>,minor:vec4<f32>,major:vec4<f32>,limits:vec4<f32>,forward:vec4<f32>}
@group(0) @binding(0) var<uniform> view:View;
struct Output {@builtin(position) position:vec4<f32>, @location(0) ndc:vec2<f32>}
struct Fragment {@location(0) color:vec4<f32>, @builtin(frag_depth) depth:f32}
@vertex fn vertex(@builtin(vertex_index) index:u32)->Output {
    let point=array<vec2<f32>,3>(vec2<f32>(-1.0,-1.0),vec2<f32>(3.0,-1.0),vec2<f32>(-1.0,3.0))[index];
    return Output(vec4<f32>(point,1.0,1.0),point);
}
fn lattice(point:vec2<f32>,step:f32,footprint:vec2<f32>)->f32 {
    let phase=fract(point/step+0.5);
    let distance=abs(phase-0.5)*step/max(footprint,vec2<f32>(0.0000001));
    return 1.0-clamp(min(distance.x,distance.y),0.0,1.0);
}
@fragment fn fragment(input:Output)->Fragment {
    let far=view.inverse*vec4<f32>(input.ndc,1.0,1.0);
    let ray=far.xyz/far.w-view.eye.xyz;
    let t=(-view.origin.z-view.eye.z)/ray.z;
    let point=view.eye.xyz+ray*t;
    let world=point+view.origin.xyz;
    let footprint=abs(dpdx(world.xy))+abs(dpdy(world.xy));
    let separation=view.params.x/max(length(dpdx(world.xy)),length(dpdy(world.xy)));
    let minor=lattice(world.xy,view.params.x,footprint)*smoothstep(0.8,3.0,separation);
    let level=max(floor(log2(max(3.0/max(separation*5.0,0.000001),1.0))),0.0);
    let step=view.params.x*5.0*exp2(level);
    let major=lattice(world.xy,step,footprint)*smoothstep(0.8,3.0,separation*5.0*exp2(level));
    let coarse=lattice(world.xy,step*2.0,footprint)*smoothstep(0.8,3.0,separation*10.0*exp2(level));
    let depth=dot(view.eye.xyz-point,view.forward.xyz);
    let fade=clamp(view.params.w/max(depth,0.00001),0.0,1.0);
    let clip=view.matrix*vec4<f32>(point,1.0);
    if view.origin.w==0.0 || abs(ray.z)<0.000001 || t<=0.0 || clip.z<=0.0 || clip.z>=clip.w {discard;}
    if view.params.z==0.0 && any(abs(world.xy)>vec2<f32>(view.params.y*0.5)) {discard;}
    if any(point>view.limits.xyz) {discard;}
    let strong=max(major,coarse);
    let alpha=max(minor,strong)*fade;
    if alpha<0.0039 {discard;}
    let color=mix(view.minor.rgb,view.major.rgb,clamp(strong/max(minor,0.0001),0.0,1.0));
    let linear=select(color/12.92,pow((color+0.055)/1.055,vec3<f32>(2.4)),color>vec3<f32>(0.04045));
    return Fragment(vec4<f32>(linear,alpha),clip.z/clip.w);
}
