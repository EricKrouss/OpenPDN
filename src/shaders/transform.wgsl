struct Params { size: vec4<u32>, row0: vec4<f32>, row1: vec4<f32>, flags: vec4<u32> }
@group(0) @binding(0) var<storage, read> source: array<u32>;
@group(0) @binding(1) var<storage, read> mask: array<u32>;
@group(0) @binding(2) var<storage, read> background: array<u32>;
@group(0) @binding(3) var<storage, read_write> output: array<u32>;
@group(0) @binding(4) var<storage, read_write> selection: array<u32>;
@group(0) @binding(5) var<uniform> params: Params;
struct Sample { color: vec4<f32>, coverage: f32 }
fn scale(a: u32, b: u32) -> u32 { let t = a*b+128u; return (t+(t>>8u))>>8u; }
fn unpack(p: u32) -> vec4<u32> { return vec4(p&255u,(p>>8u)&255u,(p>>16u)&255u,p>>24u); }
fn pack(p: vec4<u32>) -> u32 { return p.r|(p.g<<8u)|(p.b<<16u)|(p.a<<24u); }
fn decode(c: vec3<f32>) -> vec3<f32> { return select(pow((c+0.055)/1.055,vec3(2.4)),c/12.92,c<=vec3(0.04045)); }
fn encode(c: vec3<f32>) -> vec3<f32> { return select(1.055*pow(max(c,vec3(0.)),vec3(1./2.4))-0.055,c*12.92,c<=vec3(0.0031308)); }
fn fetch(at: vec2<i32>) -> Sample {
    if any(at < vec2(0)) || at.x >= i32(params.size.z) || at.y >= i32(params.size.w) { return Sample(vec4(0.),0.); }
    let i = u32(at.y)*params.size.z+u32(at.x);
    let p = unpack(source[i]);
    let alpha = f32(scale(p.a,mask[i]))/255.;
    var rgb = vec3<f32>(p.rgb)/255.;
    if params.flags.z != 0u { rgb=decode(rgb); }
    return Sample(vec4(rgb*alpha,alpha),f32(mask[i]));
}
fn sample_at(at: vec2<f32>) -> Sample {
    if params.flags.x == 0u { return fetch(vec2<i32>(floor(at))); }
    let p = at-0.5;
    let base = vec2<i32>(floor(p));
    let f = fract(p);
    let a=fetch(base); let b=fetch(base+vec2(1,0)); let c=fetch(base+vec2(0,1)); let d=fetch(base+vec2(1,1));
    return Sample(mix(mix(a.color,b.color,f.x),mix(c.color,d.color,f.x),f.y),mix(mix(a.coverage,b.coverage,f.x),mix(c.coverage,d.coverage,f.x),f.y));
}
fn blend(dst: vec4<u32>, src: vec4<u32>) -> vec4<u32> {
    if src.a==0u { return dst; }
    let remaining=scale(dst.a,255u-src.a); let alpha=remaining+src.a;
    return vec4((dst.rgb*remaining+src.rgb*src.a)/alpha,alpha);
}
@compute @workgroup_size(16,16)
fn main(@builtin(global_invocation_id) pos: vec3<u32>) {
    if any(pos.xy >= params.size.xy) { return; }
    let i=pos.y*params.size.x+pos.x;
    let p=vec3(vec2<f32>(pos.xy)+0.5,1.);
    let value=sample_at(vec2(dot(params.row0.xyz,p),dot(params.row1.xyz,p)));
    selection[i]=u32(clamp(floor(value.coverage+0.5),0.,255.));
    var result=unpack(background[i]);
    if params.flags.y != 0u && value.color.a > 0. {
        var rgb=value.color.rgb/value.color.a;
        if params.flags.z != 0u { rgb=encode(rgb); }
        let color=vec4<u32>(clamp(floor(vec4(rgb,value.color.a)*255.+0.5),vec4(0.),vec4(255.)));
        result=blend(result,color);
    }
    output[i]=pack(result);
}
