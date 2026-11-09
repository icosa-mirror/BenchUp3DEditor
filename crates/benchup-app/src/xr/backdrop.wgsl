struct View {
    inverse:mat4x4<f32>, eye:vec4<f32>, background:vec4<f32>, sky:vec4<f32>,
    ground:vec4<f32>, horizon:vec4<f32>, sun:vec4<f32>, options:vec4<f32>,
}
@group(0) @binding(0) var<uniform> view:View;
struct Output {@builtin(position) position:vec4<f32>, @location(0) ndc:vec2<f32>}
@vertex fn vertex(@builtin(vertex_index) index:u32)->Output {
    let point=array<vec2<f32>,3>(vec2<f32>(-1.0,-1.0),vec2<f32>(3.0,-1.0),vec2<f32>(-1.0,3.0))[index];
    return Output(vec4<f32>(point,1.0,1.0),point);
}
@fragment fn fragment(input:Output)->@location(0) vec4<f32> {
    let far=view.inverse*vec4<f32>(input.ndc,1.0,1.0);
    let ray=normalize(far.xyz/far.w-view.eye.xyz);
    var color=view.background.rgb;
    if ray.z>=0.0 && view.options.x>0.0 {
        let base=select(vec3<f32>(1.0),view.horizon.rgb,view.options.z>0.0);
        color=mix(base,view.sky.rgb,clamp(ray.z/0.45,0.0,1.0));
    } else if ray.z<0.0 && view.options.y>0.0 {color=view.ground.rgb;}
    if view.options.x+view.options.y>0.0 {
        let line=1.0-smoothstep(0.0,max(fwidth(ray.z),0.000001),abs(ray.z));
        color=mix(color,view.horizon.rgb,line);
    }
    if view.options.w>0.0 {
        let angle=acos(clamp(dot(ray,view.sun.xyz),-1.0,1.0));
        let glow=(1.0-smoothstep(0.012,0.045,angle))*0.2;
        color=mix(color,vec3<f32>(1.0,0.812,0.353),glow);
        color=mix(color,vec3<f32>(1.0,0.871,0.451),1.0-smoothstep(0.014,0.016,angle));
        color=mix(color,vec3<f32>(1.0,0.961,0.761),1.0-smoothstep(0.009,0.011,angle));
    }
    let linear=select(color/12.92,pow((color+0.055)/1.055,vec3<f32>(2.4)),color>vec3<f32>(0.04045));
    return vec4<f32>(linear,1.0);
}
